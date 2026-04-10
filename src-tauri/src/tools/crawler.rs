use html2md::parse_html;
use rquest::Client;
use rquest_util::Emulation;
use scraper::{Html, Selector};
use std::error::Error;

pub struct Crawler {}

impl Default for Crawler {
    fn default() -> Self {
        Self::new()
    }
}

impl Crawler {
    pub fn new() -> Self {
        Crawler {}
    }

    /// Fetches HTML from url, parses it, extracts body text and converts to markdown.
    pub async fn scrape(&self, url: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let client = Client::builder()
            .emulation(Emulation::Chrome124)
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        let resp = client.get(url).send().await?;
        if !resp.status().is_success() {
            return Err(format!("HTTP Error: {}", resp.status()).into());
        }

        let html_content = resp.text().await?;

        // Use scraper to get the body, dropping <script> and <style>
        let document = Html::parse_document(&html_content);
        let body_selector = Selector::parse("body").unwrap();

        let clean_html = if let Some(body) = document.select(&body_selector).next() {
            // Very naive cleanup before markdown conversion
            body.html()
        } else {
            html_content
        };

        // Convert cleaned HTML into structured Markdown for LLMs
        let markdown = parse_html(&clean_html);

        // Strip excessive blank lines
        let mut compact_md = String::new();
        let mut prev_empty = false;

        for line in markdown.lines() {
            let l = line.trim();
            if l.is_empty() {
                if !prev_empty {
                    compact_md.push('\n');
                    prev_empty = true;
                }
            } else {
                compact_md.push_str(l);
                compact_md.push('\n');
                prev_empty = false;
            }
        }

        Ok(compact_md.trim().to_string())
    }
}
