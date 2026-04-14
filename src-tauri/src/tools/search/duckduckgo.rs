use super::models::{assign_recency_score, SearchResultItem};
use scraper::{Html, Selector};
use std::error::Error;

pub async fn scrape_duckduckgo(
    client: &rquest::Client,
    query: &str,
    time_range: Option<&str>,
    num: u32,
) -> Result<Vec<SearchResultItem>, Box<dyn Error + Send + Sync>> {
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
    let status = resp.status();
    if !status.is_success() {
        let body_preview = resp.text().await.unwrap_or_default();
        let preview = if body_preview.len() > 100 {
            format!("{}...", &body_preview[..100])
        } else {
            body_preview
        };
        return Err(format!("DDG Error: {} - Body: {}", status, preview).into());
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
                recency_score: assign_recency_score(&snippet),
            });
        }
    }
    Ok(items)
}
