use super::models::{assign_recency_score, SearchResultItem};
use std::error::Error;

pub async fn search_google(
    client: &rquest::Client,
    query: &str,
    api_key: &str,
    cx: &str,
) -> Result<Vec<SearchResultItem>, Box<dyn Error + Send + Sync>> {
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
                    recency_score: assign_recency_score(&snippet),
                    source: "google".to_string(),
                    query_used: query.to_string(),
                    reliability_score: 0,
                    query_match_score: 0,
                    cross_check_score: 0,
                    final_score: 0,
                });
            }
        }
    }
    Ok(items)
}
