use super::models::{assign_recency_score, SearchResultItem};
use std::error::Error;

pub async fn search_tavily(
    client: &rquest::Client,
    query: &str,
    api_key: &str,
) -> Result<Vec<SearchResultItem>, Box<dyn Error + Send + Sync>> {
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
                    recency_score: assign_recency_score(&snippet),
                });
            }
        }
    }
    Ok(items)
}
