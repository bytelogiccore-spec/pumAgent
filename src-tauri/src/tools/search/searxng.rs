use serde::Deserialize;
use std::error::Error;
use super::models::SearchResultItem;
use rand::seq::SliceRandom;
use tokio::time::{sleep, Duration};

const SEARXNG_INSTANCES: &[&str] = &[
    "https://searx.be/",
    "https://searx.tiekoetter.com/",
    "https://priv.au/",
    "https://etsi.me/",
    "https://baresearch.org/",
    "https://searx.perennialte.ch/",
];

#[derive(Deserialize, Debug)]
struct SearxngResponse {
    results: Vec<SearxngResult>,
}

#[derive(Deserialize, Debug)]
struct SearxngResult {
    title: String,
    url: String,
    content: Option<String>,
}

pub async fn search_searxng(
    client: &rquest::Client,
    query: &str,
    time_range: Option<&str>,
    num: u32,
) -> Result<Vec<SearchResultItem>, Box<dyn Error + Send + Sync>> {
    let mut instances: Vec<&str> = SEARXNG_INSTANCES.to_vec();
    {
        let mut rng = rand::thread_rng();
        instances.shuffle(&mut rng);
    }

    let max_retries = 3;
    let mut last_error = String::new();

    for instance in instances.iter().take(max_retries) {
        let mut url = format!("{}/search?q={}&format=json", instance, urlencoding::encode(query));
        
        if let Some(tr) = time_range {
            let tr_val = match tr {
                "d" | "w" | "m" | "y" => tr,
                _ => "",
            };
            if !tr_val.is_empty() {
                url.push_str(&format!("&time_range={}", tr_val)); // SearXNG param
            }
        }

        let resp = client.get(&url).send().await;
        
        match resp {
            Ok(response) => {
                if !response.status().is_success() {
                    last_error = format!("Status: {}", response.status());
                    continue;
                }
                
                let text = response.text().await.unwrap_or_default();
                if let Ok(json) = serde_json::from_str::<SearxngResponse>(&text) {
                    let mut items = Vec::new();
                    for (i, r) in json.results.into_iter().enumerate() {
                        if i as u32 >= num { break; }
                        let snippet = r.content.unwrap_or_default();
                        let mut item = SearchResultItem {
                            title: r.title,
                            link: r.url,
                            snippet: snippet.clone(),
                            recency_score: 0,
                        };
                        item.recency_score = crate::tools::search::models::assign_recency_score(&snippet);
                        items.push(item);
                    }
                    if !items.is_empty() {
                        return Ok(items);
                    } else {
                        last_error = "Valid JSON but no organic results".to_string();
                    }
                } else {
                    last_error = "Invalid JSON response".to_string();
                }
            },
            Err(e) => {
                last_error = e.to_string();
            }
        }
        sleep(Duration::from_millis(500)).await;
    }
    
    Err(format!("SearXNG public instances failed after {} retries. Last error: {}", max_retries, last_error).into())
}
