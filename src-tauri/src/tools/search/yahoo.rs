use scraper::{Html, Selector};
use std::error::Error;
use super::SearchResultItem;

pub async fn scrape_yahoo(
    client: &rquest::Client,
    query: &str,
    time_range: Option<&str>,
    num: u32,
) -> Result<Vec<SearchResultItem>, Box<dyn Error + Send + Sync>> {
    let mut url = format!("https://search.yahoo.com/search?p={}&n={}", urlencoding::encode(query), num + 5);

    // time_range handling a la DDG/Google if needed...
    if let Some(tr) = time_range {
        let age = match tr {
            "d" => "1d",
            "w" => "1w",
            "m" => "1m",
            "y" => "1y",
            _ => "",
        };
        if !age.is_empty() {
             url.push_str(&format!("&age={}", age));
        }
    }

    let response = client
        .get(&url)
        .header("Referer", "https://search.yahoo.com/")
        .header("Accept-Language", "ko-KR,ko;q=0.9,en-US;q=0.8,en;q=0.7")
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8")
        .send()
        .await?;

    let text = response.text().await?;
    let body_lower = text.to_lowercase();

    if body_lower.contains("captcha") || body_lower.contains("will you help us") {
        return Err("Search blocked by bot detection. Please wait or try another provider.".into());
    }

    let document = Html::parse_document(&text);
    
    // Yahoo commonly uses div.algo or div.compTitle for organic search results
    // We will target combinations that are known to work:
    let container_selector = Selector::parse("div.algo, div.algo-sr, div.Ovh\\(h\\)").unwrap();
    let title_selector = Selector::parse("h3.title a, .compTitle a").unwrap();
    let snippet_selector = Selector::parse(".compText, .fz-ms, .fc-falcon").unwrap();

    let mut items = Vec::new();
    let mut count = 0;

    for container in document.select(&container_selector) {
        if count >= num {
            break;
        }

        let a_elem = match container.select(&title_selector).next() {
            Some(elem) => elem,
            None => continue,
        };

        let link = match a_elem.value().attr("href") {
            Some(href) => {
                // Yahoo often wraps links in their own redirect tag like `https://r.search.yahoo.com/_ylt=.../RU=https://actual.link/RK=...`
                // We should clean it if it contains /RU=
                let mut actual_link = href.to_string();
                if let Some(ru_idx) = actual_link.find("/RU=") {
                    if let Some(rk_idx) = actual_link[ru_idx..].find("/RK=") {
                        if let Ok(decoded) = urlencoding::decode(&actual_link[ru_idx + 4..ru_idx + rk_idx]) {
                             actual_link = decoded.to_string();
                        }
                    } else if let Ok(decoded) = urlencoding::decode(&actual_link[ru_idx + 4..]) {
                         actual_link = decoded.to_string();
                    }
                }
                actual_link
            },
            None => continue,
        };

        let title: String = a_elem.text().collect::<Vec<_>>().join(" ").trim().to_string();
        if title.is_empty() {
            continue;
        }

        let snippet: String = match container.select(&snippet_selector).next() {
            Some(desc) => desc.text().collect::<Vec<_>>().join(" ").trim().to_string(),
            None => "".to_string(),
        };

        let mut item = SearchResultItem {
            title,
            link,
            snippet: snippet.clone(),
            recency_score: 0,
        };
        item.recency_score = crate::tools::search::models::assign_recency_score(&snippet);
        items.push(item);
        count += 1;
    }

    Ok(items)
}
