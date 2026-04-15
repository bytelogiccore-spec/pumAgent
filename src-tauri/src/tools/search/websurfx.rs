use super::models::SearchResultItem;
use serde::Deserialize;
use std::error::Error;
use tokio::time::{sleep, Duration};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct WebsurfxResponse {
    results: Vec<WebsurfxResult>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct WebsurfxResult {
    title: Option<String>,
    url: Option<String>,
    description: Option<String>,
}

pub async fn search_websurfx(
    client: &rquest::Client,
    query: &str,
    _time_range: Option<&str>,
    num: u32,
) -> Result<Vec<SearchResultItem>, Box<dyn Error + Send + Sync>> {
    let local_instance = "http://127.0.0.1:8080";
    let max_retries = 3;
    let mut last_error = String::new();

    for attempt in 0..max_retries {
        let url = format!(
            "{}/search?q={}&json=true",
            local_instance,
            urlencoding::encode(query)
        );

        log::info!(
            "Websurfx local API polling (attempt {}): {}",
            attempt + 1,
            url
        );
        let resp = client.get(&url).send().await;

        match resp {
            Ok(response) => {
                if !response.status().is_success() {
                    last_error = format!("Status: {}", response.status());
                    log::warn!("Websurfx local instance failed: {}", last_error);
                } else {
                    let text = response.text().await.unwrap_or_default();
                    match serde_json::from_str::<WebsurfxResponse>(&text) {
                        Ok(parsed) => {
                            let mut items = Vec::new();
                            for result in parsed.results.into_iter().take(num as usize) {
                                let title = result.title.unwrap_or_default();
                                let link = result.url.unwrap_or_default();
                                let mut snippet = result.description.unwrap_or_default();

                                snippet = snippet
                                    .replace("<span class=\"searchmatch\">", "")
                                    .replace("</span>", "")
                                    .replace("<b>", "")
                                    .replace("</b>", "");
                                let title = title
                                    .replace("<span class=\"searchmatch\">", "")
                                    .replace("</span>", "")
                                    .replace("<b>", "")
                                    .replace("</b>", "");

                                if title.is_empty() || link.is_empty() {
                                    continue;
                                }

                                let mut item = SearchResultItem {
                                    title,
                                    link,
                                    snippet: snippet.clone(),
                                    recency_score: 0,
                                    source: "websurfx".to_string(),
                                    query_used: query.to_string(),
                                    reliability_score: 0,
                                    query_match_score: 0,
                                    cross_check_score: 0,
                                    final_score: 0,
                                };
                                item.recency_score =
                                    crate::tools::search::models::assign_recency_score(&snippet);
                                items.push(item);
                            }

                            if !items.is_empty() {
                                return Ok(items);
                            } else {
                                last_error =
                                    "Websurfx API loaded but no valid results found in JSON"
                                        .to_string();
                            }
                        }
                        Err(e) => {
                            last_error = format!("Failed to parse Websurfx JSON response: {}", e);
                            log::error!(
                                "Websurfx parsing error: {}. Raw response excerpt: {:.200}",
                                e,
                                text
                            );
                        }
                    }
                }
            }
            Err(e) => {
                last_error = e.to_string();
            }
        }
        sleep(Duration::from_millis(1500)).await;
    }

    Err(format!(
        "Websurfx local instance failed after {} retries. Last error: {}",
        max_retries, last_error
    )
    .into())
}
