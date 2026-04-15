use super::models::{assign_recency_score, SearchResultItem};
use scraper::{Html, Selector};
use std::error::Error;

pub async fn scrape_duckduckgo(
    client: &rquest::Client,
    query: &str,
    time_range: Option<&str>,
    num: u32,
) -> Result<Vec<SearchResultItem>, Box<dyn Error + Send + Sync>> {
    let mut url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        urlencoding::encode(query)
    );
    if let Some(tr) = time_range {
        if ["d", "w", "m", "y"].contains(&tr) {
            url.push_str(&format!("&df={}", tr));
        }
    }

    let resp = client.get(&url).send().await?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("DDG Error: {}", status).into());
    }

    let body = resp.text().await?;

    let document = Html::parse_document(&body);
    let result_selector = Selector::parse(".result").unwrap();
    let title_selector = Selector::parse(".result__title a").unwrap();
    let snippet_selector = Selector::parse(".result__snippet").unwrap();

    let mut items = Vec::new();
    for container in document.select(&result_selector) {
        if items.len() as u32 >= num {
            break;
        }

        let a_elem = match container.select(&title_selector).next() {
            Some(elem) => elem,
            None => continue,
        };

        let title = a_elem
            .text()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();
        let mut link = a_elem.value().attr("href").unwrap_or_default().to_string();

        // Decode DDG proxy links
        if link.starts_with("//duckduckgo.com/l/?uddg=") {
            if let Some(encoded_url) = link.split("uddg=").nth(1) {
                let end = encoded_url.find('&').unwrap_or(encoded_url.len());
                let extracted = &encoded_url[..end];
                if let Ok(decoded) = urlencoding::decode(extracted) {
                    link = decoded.into_owned();
                }
            }
        }

        let snippet = match container.select(&snippet_selector).next() {
            Some(snip) => snip.text().collect::<Vec<_>>().join(" ").trim().to_string(),
            None => "".to_string(),
        };

        if !title.is_empty() && !link.is_empty() {
            items.push(SearchResultItem {
                title,
                link,
                snippet: snippet.clone(),
                recency_score: assign_recency_score(&snippet),
                source: "duckduckgo".to_string(),
                query_used: query.to_string(),
                reliability_score: 0,
                query_match_score: 0,
                cross_check_score: 0,
                final_score: 0,
            });
        }
    }

    if items.is_empty() {
        return Err("No organic results found in DuckDuckGo HTML.".into());
    }

    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rquest_util::Emulation;

    #[tokio::test]
    async fn test_scrape_duckduckgo() {
        let client = rquest::Client::builder()
            .emulation(Emulation::Chrome124)
            .build()
            .unwrap();

        match scrape_duckduckgo(&client, "오늘 서울 날씨", None, 5).await {
            Ok(results) => {
                println!("SUCCESS! Found {} results.", results.len());
                for r in results {
                    println!("T: {}\nL: {}", r.title, r.link);
                }
            }
            Err(e) => {
                println!("ERROR: {}", e);
            }
        }
    }
}
