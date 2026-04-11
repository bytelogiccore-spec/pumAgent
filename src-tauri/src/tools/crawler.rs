use html2md::parse_html;
use regex::Regex;
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

        // 1. Strip massive heavy tags (scripts, styles, images, SVGs) that blow up context limits
        let re_script = Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
        let re_style = Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
        let re_svg = Regex::new(r"(?is)<svg[^>]*>.*?</svg>").unwrap();
        let re_img = Regex::new(r"(?i)<img[^>]*>").unwrap();
        let re_noscript = Regex::new(r"(?is)<noscript[^>]*>.*?</noscript>").unwrap();
        let re_footer = Regex::new(r"(?is)<footer[^>]*>.*?</footer>").unwrap();

        let mut clean_html = re_script.replace_all(&html_content, "").to_string();
        clean_html = re_style.replace_all(&clean_html, "").to_string();
        clean_html = re_svg.replace_all(&clean_html, "").to_string();
        clean_html = re_img.replace_all(&clean_html, "").to_string();
        clean_html = re_noscript.replace_all(&clean_html, "").to_string();
        clean_html = re_footer.replace_all(&clean_html, "").to_string();

        // 2. Use scraper to get the body
        let document = Html::parse_document(&clean_html);
        let body_selector = Selector::parse("body").unwrap();

        let focused_html = if let Some(body) = document.select(&body_selector).next() {
            body.html()
        } else {
            clean_html
        };

        // 3. Convert cleaned HTML into structured Markdown for LLMs
        let mut markdown = parse_html(&focused_html);

        // 4. Strip any remaining markdown images (e.g. from picture tags) and massive data links
        let re_md_img = Regex::new(r"!\[.*?\]\(.*?\)").unwrap();
        let re_data_link = Regex::new(r"\[([^\]]*)\]\(data:[^)]+\)").unwrap();
        markdown = re_md_img.replace_all(&markdown, "").to_string();
        markdown = re_data_link.replace_all(&markdown, "$1").to_string();

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

    pub async fn execute_action(
        &self,
        tool: String,
        action: String,
        args: serde_json::Value,
    ) -> crate::agent::multi_agent::ToolResult {
        if action == "scrape" {
            let url = args.get("url").and_then(|u| u.as_str()).unwrap_or("");
            match self.scrape(url).await {
                Ok(content) => crate::agent::multi_agent::ToolResult {
                    tool_name: tool,
                    action,
                    ok: true,
                    output: content,
                },
                Err(e) => crate::agent::multi_agent::ToolResult {
                    tool_name: tool,
                    action,
                    ok: false,
                    output: format!("Scrape error: {}", e),
                },
            }
        } else {
            crate::agent::multi_agent::ToolResult {
                tool_name: tool,
                action,
                ok: false,
                output: "Unsupported action".into(),
            }
        }
    }
}
