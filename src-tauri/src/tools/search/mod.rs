pub mod duckduckgo;
pub mod google;
pub mod models;
pub mod tavily;
pub mod yahoo;

use duckduckgo::scrape_duckduckgo;
use google::search_google;
pub use models::SearchResultItem;
use rquest_util::Emulation;
use std::error::Error;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tavily::search_tavily;
use tokio::time::sleep;
use yahoo::scrape_yahoo;

pub struct SearchTool {
    last_request: Mutex<Option<Instant>>,
    base_dir: std::path::PathBuf,
}

impl SearchTool {
    pub fn new(_api_key: String, _cx: String, base_dir: std::path::PathBuf) -> Self {
        SearchTool {
            last_request: Mutex::new(None),
            base_dir,
        }
    }

    fn get_spoofed_client() -> Result<rquest::Client, Box<dyn Error + Send + Sync>> {
        let client = rquest::Client::builder()
            .emulation(Emulation::Chrome124)
            .build()?;

        Ok(client)
    }

    async fn rate_limit(&self) {
        let min_delay = Duration::from_millis(2500); // 2.5 seconds minimum delay between requests
        let mut wait_time = Duration::from_secs(0);

        {
            let mut last_req = self.last_request.lock().unwrap();
            let now = Instant::now();
            if let Some(last) = *last_req {
                let elapsed = now.duration_since(last);
                if elapsed < min_delay {
                    wait_time = min_delay - elapsed;
                }
            }
            *last_req = Some(now + wait_time); // Update to expected time
        }

        if wait_time > Duration::from_secs(0) {
            sleep(wait_time).await;
        }
    }

    pub async fn search(
        &self,
        query: &str,
        time_range: Option<&str>,
        num: u32,
    ) -> Result<Vec<SearchResultItem>, Box<dyn Error + Send + Sync>> {
        self.rate_limit().await; // Global IP Rate Limit / Token Bucket protection

        let config = crate::AppConfig::load(&self.base_dir);
        let client = Self::get_spoofed_client()?;

        let mut results = vec![];
        let provider = config.search_provider.as_str();

        if provider == "tavily" && !config.tavily_api_key.is_empty() {
            match search_tavily(&client, query, &config.tavily_api_key).await {
                Ok(api_results) => results.extend(api_results),
                Err(e) => log::error!("Tavily failed: {}", e),
            }
        } else if provider == "google"
            && !config.google_api_key.is_empty()
            && !config.google_cx.is_empty()
        {
            match search_google(&client, query, &config.google_api_key, &config.google_cx).await {
                Ok(api_results) => results.extend(api_results),
                Err(e) => log::error!("Google API failed: {}", e),
            }
        }

        // If specific provider API failed or returned 0 results, OR provider is duckduckgo -> fallback to Web Scraper
        if results.is_empty() && results.len() < (num as usize) / 2 {
            match scrape_duckduckgo(&client, query, time_range, num).await {
                Ok(ddg_results) => results.extend(ddg_results),
                Err(e) => log::error!("Meta-Search: DuckDuckGo failed! {} - Falling back...", e),
            }

            if results.len() < (num as usize) / 2 {
                self.rate_limit().await;
                match scrape_yahoo(&client, query, num).await {
                    Ok(yahoo_results) => results.extend(yahoo_results),
                    Err(e) => log::error!("Meta-Search: Yahoo Fallback failed! {}", e),
                }
            }
        }

        if results.is_empty() {
            return Err("All search engines failed or blocked the request.".into());
        }

        // Sort dynamically by recency score (Descending: High score first)
        results.sort_by(|a, b| b.recency_score.cmp(&a.recency_score));

        // Filter and return Top N
        results.truncate(num as usize);
        Ok(results)
    }

    pub async fn execute_action(
        &self,
        tool: String,
        action: String,
        args: serde_json::Value,
    ) -> crate::agent::multi_agent::ToolResult {
        if action == "query" {
            let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("");
            let time_range = args.get("time_range").and_then(|t| t.as_str());
            match self.search(query, time_range, 5).await {
                Ok(items) => {
                    let content = items
                        .into_iter()
                        .map(|item| {
                            format!(
                                "Title: {}\nLink: {}\nSnippet: {}\n---",
                                item.title, item.link, item.snippet
                            )
                        })
                        .collect::<Vec<String>>()
                        .join("\n");
                    crate::agent::multi_agent::ToolResult {
                        tool_name: tool,
                        action,
                        ok: true,
                        output: content,
                    }
                }
                Err(e) => crate::agent::multi_agent::ToolResult {
                    tool_name: tool,
                    action,
                    ok: false,
                    output: format!("Search error: {}", e),
                },
            }
        } else {
            crate::agent::multi_agent::ToolResult {
                tool_name: tool,
                action,
                ok: false,
                output: "Unsupported action".into(),
            }
        }
    }
}
