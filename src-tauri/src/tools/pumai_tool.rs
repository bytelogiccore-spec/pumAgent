use crate::config::AppConfig;
use rquest::Client;
use std::error::Error;
use std::path::PathBuf;

pub struct PumaiTool {
    base_dir: PathBuf,
    client: Client,
}

impl PumaiTool {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .expect("Failed to create PumAI HTTP client"),
        }
    }

    fn resolve_config(&self) -> (String, String) {
        let config = AppConfig::load(&self.base_dir);
        (config.pumai_base_url, config.pumai_api_key)
    }

    fn build_url(base_url: &str, path: &str) -> String {
        format!(
            "{}/{}",
            base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    async fn get(
        &self,
        base_url: &str,
        api_key: Option<&str>,
        path: &str,
        query_pairs: &[(String, String)],
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let mut url = Self::build_url(base_url, path);
        if !query_pairs.is_empty() {
            let qs = query_pairs
                .iter()
                .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
                .collect::<Vec<String>>()
                .join("&");
            url = format!("{}?{}", url, qs);
        }

        let mut req = self.client.get(url);
        if let Some(key) = api_key {
            if !key.trim().is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key.trim()));
                req = req.header("x-api-key", key.trim());
            }
        }

        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Ok(format!("PumAI HTTP Error {}\nBody: {}", status, text));
        }
        Ok(text)
    }

    pub async fn execute_action(
        &self,
        tool: String,
        action: String,
        args: serde_json::Value,
    ) -> crate::agent::multi_agent::ToolResult {
        let (cfg_base_url, cfg_api_key) = self.resolve_config();
        let base_url = args
            .get("base_url")
            .and_then(|v| v.as_str())
            .unwrap_or(&cfg_base_url);
        let api_key = args.get("api_key").and_then(|v| v.as_str()).or({
            if cfg_api_key.is_empty() {
                None
            } else {
                Some(cfg_api_key.as_str())
            }
        });

        if base_url.trim().is_empty() {
            return crate::agent::multi_agent::ToolResult {
                tool_name: tool,
                action,
                ok: false,
                output: "Missing PumAI base URL. Set config.pumai_base_url or args.base_url."
                    .to_string(),
            };
        }

        let request_result = match action.as_str() {
            "health" => self.get(base_url, api_key, "/health", &[]).await,
            "market_list" => {
                let mut query_pairs = Vec::new();
                if let Some(limit) = args.get("limit").and_then(|v| v.as_u64()) {
                    query_pairs.push(("limit".to_string(), limit.to_string()));
                }
                if let Some(page) = args.get("page").and_then(|v| v.as_u64()) {
                    query_pairs.push(("page".to_string(), page.to_string()));
                }
                if let Some(query) = args.get("query").and_then(|v| v.as_str()) {
                    if !query.trim().is_empty() {
                        query_pairs.push(("query".to_string(), query.trim().to_string()));
                    }
                }
                self.get(base_url, api_key, "/market/items", &query_pairs)
                    .await
            }
            "market_get" => {
                let item_id = match args.get("item_id").and_then(|v| v.as_str()) {
                    Some(id) if !id.trim().is_empty() => id.trim(),
                    _ => {
                        return crate::agent::multi_agent::ToolResult {
                            tool_name: tool,
                            action,
                            ok: false,
                            output: "Missing required arg: item_id".to_string(),
                        };
                    }
                };
                let path = format!("/market/items/{}", item_id);
                self.get(base_url, api_key, &path, &[]).await
            }
            "knowledge_fetch" => {
                let knowledge_id = match args.get("knowledge_id").and_then(|v| v.as_str()) {
                    Some(id) if !id.trim().is_empty() => id.trim(),
                    _ => {
                        return crate::agent::multi_agent::ToolResult {
                            tool_name: tool,
                            action,
                            ok: false,
                            output: "Missing required arg: knowledge_id".to_string(),
                        };
                    }
                };
                let path = format!("/knowledge/{}", knowledge_id);
                self.get(base_url, api_key, &path, &[]).await
            }
            _ => {
                return crate::agent::multi_agent::ToolResult {
                    tool_name: tool,
                    action,
                    ok: false,
                    output: "Unknown action for pumai tool.".to_string(),
                };
            }
        };

        match request_result {
            Ok(output) => crate::agent::multi_agent::ToolResult {
                tool_name: tool,
                action,
                ok: true,
                output,
            },
            Err(e) => crate::agent::multi_agent::ToolResult {
                tool_name: tool,
                action,
                ok: false,
                output: format!("PumAI Error: {}", e),
            },
        }
    }
}
