pub mod duckduckgo;
pub mod google;
pub mod models;
pub mod tavily;
pub mod websurfx;
pub mod yahoo;

use duckduckgo::scrape_duckduckgo;
use google::search_google;
pub use models::SearchResultItem;
use rand::seq::SliceRandom;
use rquest_util::Emulation;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tavily::search_tavily;
use tokio::time::sleep;
use websurfx::search_websurfx;
use yahoo::scrape_yahoo;

pub struct SearchTool {
    db: Arc<dbx_core::Database>,
    base_dir: std::path::PathBuf,
}

impl SearchTool {
    fn record_search_anomaly(
        &self,
        trace_id: Option<&str>,
        action: &str,
        normalized_action: &str,
        details: &[String],
    ) {
        if details.is_empty() {
            return;
        }
        let key = format!("search_anomaly:{}", chrono::Utc::now().timestamp_millis());
        let value = serde_json::json!({
            "trace_id": trace_id.unwrap_or("trace-unknown"),
            "action": action,
            "normalized_action": normalized_action,
            "details": details,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        let _ = self
            .db
            .insert("metrics", key.as_bytes(), value.to_string().as_bytes());
    }

    fn extract_query_with_fallback(args: &serde_json::Value) -> (String, Option<&'static str>) {
        if let Some(q) = args.get("query").and_then(|v| v.as_str()) {
            let query = q.trim().to_string();
            if !query.is_empty() {
                return (query, None);
            }
        }
        for (key, label) in [
            ("action", "args.action"),
            ("q", "args.q"),
            ("keyword", "args.keyword"),
            ("text", "args.text"),
        ] {
            if let Some(v) = args.get(key).and_then(|x| x.as_str()) {
                let query = v.trim().to_string();
                if !query.is_empty() {
                    return (query, Some(label));
                }
            }
        }
        if let Some(raw) = args.as_str() {
            let query = raw.trim().to_string();
            if !query.is_empty() {
                return (query, Some("args(raw_string)"));
            }
        }
        ("".to_string(), None)
    }

    pub fn new(db: Arc<dbx_core::Database>, base_dir: std::path::PathBuf) -> Self {
        SearchTool { db, base_dir }
    }

    fn get_spoofed_client() -> Result<rquest::Client, Box<dyn Error + Send + Sync>> {
        let emulations = [
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
        let _ = self
            .db
            .insert("agent_state", last_req_key, next_ts.to_string().as_bytes());

        if wait_time > 0 {
            sleep(Duration::from_millis(wait_time as u64)).await;
        }
    }

    fn record_metric(&self, trace_id: &str, provider: &str, status: &str, elapsed_ms: i64) {
        let key = format!(
            "search_metric:{}:{}:{}",
            chrono::Utc::now().timestamp_millis(),
            provider,
            status
        );
        let value = serde_json::json!({
            "trace_id": trace_id,
            "provider": provider,
            "status": status,
            "elapsed_ms": elapsed_ms,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        let _ = self
            .db
            .insert("metrics", key.as_bytes(), value.to_string().as_bytes());
    }

    fn rewrite_and_decompose_query(query: &str) -> Vec<String> {
        let mut queries = vec![query.trim().to_string()];
        let normalized = query
            .replace("latest", "recent")
            .replace("news", "updates")
            .replace("please", "")
            .trim()
            .to_string();
        if !normalized.is_empty() && normalized != query {
            queries.push(normalized);
        }
        for split_key in [" and ", ",", " then "] {
            if query.to_lowercase().contains(split_key) {
                for part in query.split(split_key) {
                    let p = part.trim();
                    if p.len() > 3 {
                        queries.push(p.to_string());
                    }
                }
            }
        }
        queries.sort();
        queries.dedup();
        queries
    }

    pub async fn search(
        &self,
        query: &str,
        time_range: Option<&str>,
        num: u32,
        trace_id: Option<&str>,
    ) -> Result<Vec<SearchResultItem>, Box<dyn Error + Send + Sync>> {
        let trace_id = trace_id.unwrap_or("trace-unknown");
        let search_started = chrono::Utc::now().timestamp_millis();
        self.rate_limit().await; // Global IP Rate Limit / Token Bucket protection

        let config = crate::AppConfig::load(&self.base_dir);
        let client = Self::get_spoofed_client()?;

        let mut tasks = vec![];
        let mut errors = vec![];
        let query_variants = Self::rewrite_and_decompose_query(query);
        let primary_query = query_variants
            .first()
            .cloned()
            .unwrap_or_else(|| query.to_string());

        // 1. Primary Engine: Websurfx local sidecar
        let tr_clone_websurfx = time_range.map(|s| s.to_string());
        log::info!("Searching via Primary Engine (Websurfx Local Sidecar)...");
        match search_websurfx(&client, &primary_query, tr_clone_websurfx.as_deref(), num).await {
            Ok(websurfx_results) => {
                if !websurfx_results.is_empty() {
                    let websurfx_len = websurfx_results.len();
                    let mut final_results = websurfx_results;
                    self.enrich_scores(&primary_query, &mut final_results);
                    self.record_metric(
                        trace_id,
                        "websurfx",
                        "success",
                        chrono::Utc::now().timestamp_millis() - search_started,
                    );
                    log::info!("Websurfx successfully returned {} results.", websurfx_len);
                    return Ok(final_results);
                } else {
                    self.record_metric(
                        trace_id,
                        "websurfx",
                        "empty",
                        chrono::Utc::now().timestamp_millis() - search_started,
                    );
                    errors.push("Websurfx: No organic results found.".to_string());
                }
            }
            Err(e) => {
                self.record_metric(
                    trace_id,
                    "websurfx",
                    "error",
                    chrono::Utc::now().timestamp_millis() - search_started,
                );
                let err_msg = format!("Websurfx: {}", e);
                log::warn!("{}", err_msg);
                errors.push(err_msg);
            }
        }

        log::warn!("Primary Engine (Websurfx) failed or returned no results. Falling back to external aggregators...");

        // Fallbacks: Tavily
        for q in query_variants {
            if !config.tavily_api_key.is_empty() {
                let client_clone = client.clone();
                let query_clone = q.clone();
                let api_key = config.tavily_api_key.clone();
                tasks.push(tokio::spawn(async move {
                    match search_tavily(&client_clone, &query_clone, &api_key).await {
                        Ok(res) => Ok(res),
                        Err(e) => Err(format!("Tavily: {}", e)),
                    }
                }));
            }

            if !config.google_api_key.is_empty() && !config.google_cx.is_empty() {
                let client_clone = client.clone();
                let query_clone = q.clone();
                let api_key = config.google_api_key.clone();
                let cx = config.google_cx.clone();
                tasks.push(tokio::spawn(async move {
                    match search_google(&client_clone, &query_clone, &api_key, &cx).await {
                        Ok(res) => Ok(res),
                        Err(e) => Err(format!("Google: {}", e)),
                    }
                }));
            }

            let client_clone = client.clone();
            let query_clone = q.clone();
            let tr_clone = time_range.map(|s| s.to_string());
            tasks.push(tokio::spawn(async move {
                match scrape_duckduckgo(&client_clone, &query_clone, tr_clone.as_deref(), num).await
                {
                    Ok(ddg_results) => {
                        if ddg_results.is_empty() {
                            Err("DDG HTML: No organic results found.".to_string())
                        } else {
                            Ok(ddg_results)
                        }
                    }
                    Err(e) => Err(format!("DDG HTML: {}", e)),
                }
            }));

            let client_clone3 = client.clone();
            let query_clone3 = q.clone();
            let tr_clone3 = time_range.map(|s| s.to_string());
            tasks.push(tokio::spawn(async move {
                match scrape_yahoo(&client_clone3, &query_clone3, tr_clone3.as_deref(), num).await {
                    Ok(yahoo_results) => {
                        if yahoo_results.is_empty() {
                            Err("Yahoo: No organic results found.".to_string())
                        } else {
                            Ok(yahoo_results)
                        }
                    }
                    Err(e) => Err(format!("Yahoo: {}", e)),
                }
            }));
        }

        let mut results = vec![];

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
            self.record_metric(
                trace_id,
                "fallback",
                "failed",
                chrono::Utc::now().timestamp_millis() - search_started,
            );
            return Err(format!("Search failed or blocked. Details: {}", combined_errors).into());
        }

        // De-duplicate by link
        let mut unique_links = std::collections::HashSet::new();
        let mut filtered_results = Vec::new();
        for item in results {
            let canonical = models::canonicalize_link(&item.link);
            if unique_links.insert(canonical) {
                filtered_results.push(item);
            }
        }

        self.enrich_scores(query, &mut filtered_results);
        filtered_results.sort_by(|a, b| b.final_score.cmp(&a.final_score));

        // Filter and return Top N
        filtered_results.truncate(num as usize);
        self.record_metric(
            trace_id,
            "fallback",
            "success",
            chrono::Utc::now().timestamp_millis() - search_started,
        );
        Ok(filtered_results)
    }

    fn enrich_scores(&self, query: &str, items: &mut [SearchResultItem]) {
        let mut domain_counts: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
        for item in items.iter() {
            let domain = models::canonicalize_link(&item.link)
                .split('/')
                .nth(2)
                .unwrap_or("unknown")
                .to_string();
            *domain_counts.entry(domain).or_insert(0) += 1;
        }
        for item in items.iter_mut() {
            item.reliability_score = models::source_reliability_score(&item.link);
            item.query_match_score =
                models::query_match_score(query, &format!("{} {}", item.title, item.snippet));
            let domain = models::canonicalize_link(&item.link)
                .split('/')
                .nth(2)
                .unwrap_or("unknown")
                .to_string();
            let domain_support = domain_counts.get(&domain).copied().unwrap_or(1);
            item.cross_check_score = domain_support.saturating_mul(15).min(100);
            item.final_score = item.recency_score
                + item.reliability_score
                + item.query_match_score
                + item.cross_check_score;
        }
    }

    pub async fn execute_action(
        &self,
        tool: String,
        action: String,
        args: serde_json::Value,
    ) -> crate::agent::multi_agent::ToolResult {
        let mut anomalies = Vec::new();
        let normalized_action = if action == "query" {
            "query".to_string()
        } else {
            anomalies.push(format!(
                "unexpected search action '{}', coercing to 'query'",
                action
            ));
            "query".to_string()
        };

        let (query, fallback_source) = Self::extract_query_with_fallback(&args);
        let trace_id = args.get("trace_id").and_then(|t| t.as_str());
        if let Some(source) = fallback_source {
            anomalies.push(format!("query extracted from {}", source));
        }
        if query.eq_ignore_ascii_case("query") {
            anomalies.push("query value is literal keyword 'query'".to_string());
        }
        self.record_search_anomaly(trace_id, &action, &normalized_action, &anomalies);

        if query.trim().is_empty() {
            return crate::agent::multi_agent::ToolResult {
                tool_name: tool,
                action: normalized_action,
                ok: false,
                output: "Search error: query cannot be empty".into(),
            };
        }

        let time_range = args.get("time_range").and_then(|t| t.as_str());
        match self.search(&query, time_range, 5, trace_id).await {
            Ok(items) => {
                let mut content = items
                    .into_iter()
                    .map(|item| {
                        format!(
                            "Title: {}\nLink: {}\nSnippet: {}\nSource: {}\nScore: {}\nBreakdown(recency/reliability/match/cross): {}/{}/{}/{}\n---",
                            item.title,
                            item.link,
                            item.snippet,
                            item.source,
                            item.final_score,
                            item.recency_score,
                            item.reliability_score,
                            item.query_match_score,
                            item.cross_check_score
                        )
                    })
                    .collect::<Vec<String>>()
                    .join("\n");
                if let Some(source) = fallback_source {
                    content = format!("[search_query_fallback_used: {}]\n{}", source, content);
                }
                crate::agent::multi_agent::ToolResult {
                    tool_name: tool,
                    action: normalized_action,
                    ok: true,
                    output: content,
                }
            }
            Err(e) => crate::agent::multi_agent::ToolResult {
                tool_name: tool,
                action: normalized_action,
                ok: false,
                output: format!("Search error: {}", e),
            },
        }
    }
}
