use rquest_util::Emulation;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchResultItem {
    pub title: String,
    pub link: String,
    pub snippet: String,
    pub recency_score: u32,
}

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

    fn assign_recency_score(snippet: &str) -> u32 {
        let snippet_lower = snippet.to_lowercase();
        // Simple heuristic: higher score = more recent
        if snippet_lower.contains("분 전") || snippet_lower.contains("mins ago") {
            return 100;
        } else if snippet_lower.contains("시간 전") || snippet_lower.contains("hours ago") {
            return 80;
        } else if snippet_lower.contains("일 전")
            || snippet_lower.contains("days ago")
            || snippet_lower.contains("어제")
        {
            return 50;
        } else if snippet_lower.contains("주 전") || snippet_lower.contains("weeks ago") {
            return 30;
        } else if snippet_lower.contains("2026.") || snippet_lower.contains("2026년") {
            return 60; // Explicitly 2026 mentions are good contextually
        }
        0 // No date or old
    }

    async fn scrape_duckduckgo(
        &self,
        query: &str,
        time_range: Option<&str>,
        num: u32,
    ) -> Result<Vec<SearchResultItem>, Box<dyn Error + Send + Sync>> {
        let client = Self::get_spoofed_client()?;
        let url = "https://lite.duckduckgo.com/lite/";

        let mut params = std::collections::HashMap::new();
        params.insert("q", query);
        params.insert("v", "l");
        if let Some(tr) = time_range {
            if ["d", "w", "m", "y"].contains(&tr) {
                params.insert("df", tr);
            }
        }

        let resp = client.post(url).form(&params).send().await?;
        if !resp.status().is_success() {
            return Err(format!("DDG Error: {}", resp.status()).into());
        }

        let body = resp.text().await?;
        let document = Html::parse_document(&body);
        let result_link_selector = Selector::parse(".result-link").unwrap();
        let result_snippet_selector = Selector::parse(".result-snippet").unwrap();

        let mut items = Vec::new();
        let links: Vec<_> = document.select(&result_link_selector).collect();
        let snippets: Vec<_> = document.select(&result_snippet_selector).collect();

        for i in 0..std::cmp::min(links.len(), snippets.len()) {
            if i >= num as usize + 5 {
                break;
            }
            let a = links[i];
            let snip = snippets[i];

            let title = a.text().collect::<Vec<_>>().join(" ").trim().to_string();
            let mut link = String::new();
            if let Some(href) = a.value().attr("href") {
                link = href.to_string();
                if link.starts_with("//") {
                    link = format!("https:{}", link);
                } else if link.starts_with("/lite/") {
                    link = format!("https://lite.duckduckgo.com{}", link);
                }
            }

            let snippet = snip.text().collect::<Vec<_>>().join(" ").trim().to_string();

            if !title.is_empty() && !link.is_empty() {
                items.push(SearchResultItem {
                    title,
                    link,
                    snippet: snippet.clone(),
                    recency_score: Self::assign_recency_score(&snippet),
                });
            }
        }
        Ok(items)
    }

    async fn scrape_yahoo(
        &self,
        query: &str,
        num: u32,
    ) -> Result<Vec<SearchResultItem>, Box<dyn Error + Send + Sync>> {
        let client = Self::get_spoofed_client()?;
        let url = format!(
            "https://search.yahoo.com/search?p={}",
            urlencoding::encode(query)
        );

        // Let's just do a basic GET
        let resp = client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(format!("Yahoo Error: {}", resp.status()).into());
        }

        let body = resp.text().await?;
        let document = Html::parse_document(&body);

        let algo_selector = Selector::parse(".algo").unwrap();
        let title_selector = Selector::parse(".title a").unwrap();
        let snippet_selector = Selector::parse(".compText").unwrap();

        let mut items = Vec::new();
        for element in document.select(&algo_selector).take(num as usize + 5) {
            let mut title = String::new();
            let mut link = String::new();
            if let Some(a) = element.select(&title_selector).next() {
                title = a.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if let Some(href) = a.value().attr("href") {
                    link = href.to_string();
                }
            }

            let mut snippet = String::new();
            if let Some(snip) = element.select(&snippet_selector).next() {
                snippet = snip.text().collect::<Vec<_>>().join(" ").trim().to_string();
            }

            // Ignore weird ad links without titles
            if !title.is_empty() && !link.is_empty() && snippet.len() > 10 {
                items.push(SearchResultItem {
                    title,
                    link,
                    snippet: snippet.clone(),
                    recency_score: Self::assign_recency_score(&snippet),
                });
            }
        }
        Ok(items)
    }

    async fn search_tavily(
        &self,
        query: &str,
        api_key: &str,
    ) -> Result<Vec<SearchResultItem>, Box<dyn Error + Send + Sync>> {
        let client = Self::get_spoofed_client()?;
        let url = "https://api.tavily.com/search";
        let mut body = serde_json::Map::new();
        body.insert(
            "api_key".to_string(),
            serde_json::Value::String(api_key.to_string()),
        );
        body.insert(
            "query".to_string(),
            serde_json::Value::String(query.to_string()),
        );
        body.insert(
            "search_depth".to_string(),
            serde_json::Value::String("basic".to_string()),
        );

        let resp = client.post(url).json(&body).send().await?;
        if !resp.status().is_success() {
            return Err(format!("Tavily Error: {}", resp.status()).into());
        }

        let json_resp: serde_json::Value = resp.json().await?;
        let mut items = Vec::new();

        if let Some(results) = json_resp.get("results").and_then(|r| r.as_array()) {
            for res in results {
                let title = res
                    .get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                let link = res
                    .get("url")
                    .and_then(|u| u.as_str())
                    .unwrap_or("")
                    .to_string();
                let snippet = res
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();

                if !title.is_empty() && !link.is_empty() {
                    items.push(SearchResultItem {
                        title,
                        link,
                        snippet: snippet.clone(),
                        recency_score: Self::assign_recency_score(&snippet),
                    });
                }
            }
        }
        Ok(items)
    }

    async fn search_google(
        &self,
        query: &str,
        api_key: &str,
        cx: &str,
    ) -> Result<Vec<SearchResultItem>, Box<dyn Error + Send + Sync>> {
        let client = Self::get_spoofed_client()?;
        let url = format!(
            "https://customsearch.googleapis.com/customsearch/v1?key={}&cx={}&q={}",
            api_key,
            cx,
            urlencoding::encode(query)
        );

        let resp = client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(format!("Google Error: {}", resp.status()).into());
        }

        let json_resp: serde_json::Value = resp.json().await?;
        let mut items = Vec::new();

        if let Some(results) = json_resp.get("items").and_then(|i| i.as_array()) {
            for res in results {
                let title = res
                    .get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                let link = res
                    .get("link")
                    .and_then(|l| l.as_str())
                    .unwrap_or("")
                    .to_string();
                let snippet = res
                    .get("snippet")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();

                if !title.is_empty() && !link.is_empty() {
                    items.push(SearchResultItem {
                        title,
                        link,
                        snippet: snippet.clone(),
                        recency_score: Self::assign_recency_score(&snippet),
                    });
                }
            }
        }
        Ok(items)
    }

    pub async fn search(
        &self,
        query: &str,
        time_range: Option<&str>,
        num: u32,
    ) -> Result<Vec<SearchResultItem>, Box<dyn Error + Send + Sync>> {
        self.rate_limit().await; // Global IP Rate Limit / Token Bucket protection

        let config = crate::AppConfig::load(&self.base_dir);

        let mut results = vec![];
        let provider = config.search_provider.as_str();

        if provider == "tavily" && !config.tavily_api_key.is_empty() {
            match self.search_tavily(query, &config.tavily_api_key).await {
                Ok(api_results) => results.extend(api_results),
                Err(e) => log::error!("Tavily failed: {}", e),
            }
        } else if provider == "google"
            && !config.google_api_key.is_empty()
            && !config.google_cx.is_empty()
        {
            match self
                .search_google(query, &config.google_api_key, &config.google_cx)
                .await
            {
                Ok(api_results) => results.extend(api_results),
                Err(e) => log::error!("Google API failed: {}", e),
            }
        }

        // If specific provider API failed or returned 0 results, OR provider is duckduckgo -> fallback to Web Scraper
        if results.is_empty() && results.len() < (num as usize) / 2 {
            match self.scrape_duckduckgo(query, time_range, num).await {
                Ok(ddg_results) => results.extend(ddg_results),
                Err(e) => log::error!("Meta-Search: DuckDuckGo failed! {} - Falling back...", e),
            }

            if results.len() < (num as usize) / 2 {
                self.rate_limit().await;
                match self.scrape_yahoo(query, num).await {
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
}
