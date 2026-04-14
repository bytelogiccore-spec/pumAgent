pub mod google;
pub mod models;
pub mod searxng;
pub mod tavily;

use google::search_google;
pub use models::SearchResultItem;
use rquest_util::Emulation;
use searxng::search_searxng;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tavily::search_tavily;
use tokio::time::sleep;
use rand::seq::SliceRandom;

pub struct SearchTool {
    db: Arc<dbx_core::Database>,
    base_dir: std::path::PathBuf,
}

impl SearchTool {
    pub fn new(db: Arc<dbx_core::Database>, base_dir: std::path::PathBuf) -> Self {
        SearchTool {
            db,
            base_dir,
        }
    }

    fn get_spoofed_client() -> Result<rquest::Client, Box<dyn Error + Send + Sync>> {
        let emulations = vec![
            Emulation::Chrome120,
            Emulation::Chrome124,
            Emulation::Chrome127,
            Emulation::Safari17_5,
            Emulation::Safari17_4_1,
        ];
        let mut rng = rand::thread_rng();
        let selected_emulation = emulations.choose(&mut rng).unwrap_or(&Emulation::Chrome124);

        let client = rquest::Client::builder()
            .emulation(*selected_emulation)
            .build()?;

        Ok(client)
    }

    async fn rate_limit(&self) {
        let min_delay = 3500; // 3.5 seconds in milliseconds
        let now = chrono::Utc::now().timestamp_millis();
        let mut wait_time = 0;

        let last_req_key = b"search_last_req_ts";
        if let Ok(Some(bytes)) = self.db.get("agent_state", last_req_key) {
            if let Ok(last_req_str) = String::from_utf8(bytes) {
                if let Ok(last_req) = last_req_str.parse::<i64>() {
                    let elapsed = now - last_req;
                    if elapsed >= 0 && elapsed < min_delay {
                        wait_time = min_delay - elapsed;
                    }
                }
            }
        }

        let next_ts = now + wait_time;
        let _ = self.db.insert("agent_state", last_req_key, next_ts.to_string().as_bytes());

        if wait_time > 0 {
            sleep(Duration::from_millis(wait_time as u64)).await;
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

        let mut tasks = vec![];

        // 1. Tavily Task
        if !config.tavily_api_key.is_empty() {
            let client_clone = client.clone();
            let query_clone = query.to_string();
            let api_key = config.tavily_api_key.clone();
            tasks.push(tokio::spawn(async move {
                match search_tavily(&client_clone, &query_clone, &api_key).await {
                    Ok(res) => Ok(res),
                    Err(e) => Err(format!("Tavily: {}", e)),
                }
            }));
        }

        // 2. Google Task
        if !config.google_api_key.is_empty() && !config.google_cx.is_empty() {
            let client_clone = client.clone();
            let query_clone = query.to_string();
            let api_key = config.google_api_key.clone();
            let cx = config.google_cx.clone();
            tasks.push(tokio::spawn(async move {
                match search_google(&client_clone, &query_clone, &api_key, &cx).await {
                    Ok(res) => Ok(res),
                    Err(e) => Err(format!("Google: {}", e)),
                }
            }));
        }

        // 3. SearXNG Aggregator Task
        let client_clone = client.clone();
        let query_clone = query.to_string();
        let tr_clone = time_range.map(|s| s.to_string());
        tasks.push(tokio::spawn(async move {
            match search_searxng(&client_clone, &query_clone, tr_clone.as_deref(), num).await {
                Ok(searxng_results) => {
                    if searxng_results.is_empty() {
                        Err("SearXNG: No organic results found.".to_string())
                    } else {
                        Ok(searxng_results)
                    }
                },
                Err(e) => Err(format!("SearXNG: {}", e)),
            }
        }));

        let mut results = vec![];
        let mut errors = vec![];

        let completed_tasks = futures::future::join_all(tasks).await;
        for task_res in completed_tasks {
            match task_res {
                Ok(Ok(items)) => results.extend(items),
                Ok(Err(err_msg)) => {
                    log::error!("{}", err_msg);
                    errors.push(err_msg);
                }
                Err(e) => {
                    log::error!("Task Join Error: {}", e);
                }
            }
        }

        if results.is_empty() {
            let combined_errors = if errors.is_empty() {
                "No results found from any search provider.".to_string()
            } else {
                errors.join(" | ")
            };
            return Err(format!("Search failed or blocked. Details: {}", combined_errors).into());
        }

        // De-duplicate by link
        let mut unique_links = std::collections::HashSet::new();
        let mut filtered_results = Vec::new();
        for item in results {
            if unique_links.insert(item.link.clone()) {
                filtered_results.push(item);
            }
        }

        // Sort dynamically by recency score (Descending: High score first)
        filtered_results.sort_by(|a, b| b.recency_score.cmp(&a.recency_score));

        // Filter and return Top N
        filtered_results.truncate(num as usize);
        Ok(filtered_results)
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
