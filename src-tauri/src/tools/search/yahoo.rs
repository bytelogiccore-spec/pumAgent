use super::models::{assign_recency_score, SearchResultItem};
use scraper::{Html, Selector};
use std::error::Error;

pub async fn scrape_yahoo(
    client: &rquest::Client,
    query: &str,
    num: u32,
) -> Result<Vec<SearchResultItem>, Box<dyn Error + Send + Sync>> {
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
                recency_score: assign_recency_score(&snippet),
            });
        }
    }
    Ok(items)
}
