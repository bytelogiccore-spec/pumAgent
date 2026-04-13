use crate::agent::multi_agent::ToolResult;
use rquest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Default)]
struct MoltbookCreds {
    pub api_key: Option<String>,
    pub agent_name: Option<String>,
}

pub struct MoltbookTool {
    base_dir: PathBuf,
}

impl MoltbookTool {
    pub fn new(base_dir: PathBuf) -> Self {
        MoltbookTool { base_dir }
    }

    fn credentials_path(&self) -> PathBuf {
        self.base_dir.join("moltbook_credentials.json")
    }

    fn load_credentials(&self) -> MoltbookCreds {
        // Try OS Keyring Vault First
        if let Ok(data) = crate::tools::vault_tool::VaultTool::get_secret("moltbook_creds") {
            if let Ok(creds) = serde_json::from_str(&data) {
                return creds;
            }
        }
        
        // Fallback or legacy file check
        if let Ok(data) = std::fs::read_to_string(self.credentials_path()) {
            if let Ok(creds) = serde_json::from_str(&data) {
                return creds;
            }
        }
        MoltbookCreds::default()
    }

    fn save_credentials(&self, creds: &MoltbookCreds) {
        if let Ok(data) = serde_json::to_string(creds) {
            let vault = crate::tools::vault_tool::VaultTool::new(self.base_dir.clone());
            let _ = vault.set_secret("moltbook_creds", &data);
            
            // For security, remove the plain text credentials file if it exists
            if self.credentials_path().exists() {
                let _ = std::fs::remove_file(self.credentials_path());
            }
        }
    }

    async fn execute_http(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
        use_auth: bool,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let url = format!("https://www.moltbook.com/api/v1{}", path);
        let mut request = match method.to_uppercase().as_str() {
            "GET" => client.get(&url),
            "POST" => client.post(&url),
            "PUT" => client.put(&url),
            "DELETE" => client.delete(&url),
            "PATCH" => client.patch(&url),
            _ => return Err("Invalid method".into()),
        };

        if use_auth {
            let creds = self.load_credentials();
            if let Some(key) = creds.api_key {
                request = request.header("Authorization", format!("Bearer {}", key));
            } else {
                return Err("No Moltbook API key found. Please register first.".into());
            }
        }

        if let Some(b) = body {
            request = request.json(&b);
        }

        let resp = request.send().await?;
        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            return Ok(format!("HTTP Error {}\nBody: {}", status, text));
        }

        Ok(text)
    }

    pub async fn execute_action(
        &self,
        tool: String,
        action: String,
        args: serde_json::Value,
    ) -> ToolResult {
        let result = match action.as_str() {
            "register" => {
                let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("PumAgent");
                let desc = args.get("description").and_then(|v| v.as_str()).unwrap_or("Autonomous Agent");
                let body = serde_json::json!({
                    "name": name,
                    "description": desc
                });
                
                match self.execute_http("POST", "/agents/register", Some(body), false).await {
                    Ok(resp_str) => {
                        // try to extract api_key
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&resp_str) {
                            if let Some(key) = parsed.get("agent").and_then(|a| a.get("api_key")).and_then(|k| k.as_str()) {
                                self.save_credentials(&MoltbookCreds {
                                    api_key: Some(key.to_string()),
                                    agent_name: Some(name.to_string()),
                                });
                            }
                        }
                        Ok(resp_str)
                    },
                    Err(e) => Err(e),
                }
            }
            "home" => self.execute_http("GET", "/home", None, true).await,
            "status" => self.execute_http("GET", "/agents/status", None, true).await,
            "search" => {
                let q = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let path = format!("/search?q={}", urlencoding::encode(q));
                self.execute_http("GET", &path, None, true).await
            }
            "feed" => {
                let sort = args.get("sort").and_then(|v| v.as_str()).unwrap_or("new");
                let path = format!("/feed?sort={}", sort);
                self.execute_http("GET", &path, None, true).await
            }
            "create_post" => {
                self.execute_http("POST", "/posts", Some(args.clone()), true).await
            }
            "create_comment" => {
                let post_id = args.get("post_id").and_then(|v| v.as_str()).unwrap_or("");
                let path = format!("/posts/{}/comments", post_id);
                self.execute_http("POST", &path, Some(args.clone()), true).await
            }
            "verify" => {
                self.execute_http("POST", "/verify", Some(args.clone()), true).await
            }
            "request" => {
                // general authenticated request
                let method = args.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("/");
                let body = args.get("body").cloned();
                self.execute_http(method, path, body, true).await
            }
            _ => Err("Unknown action for moltbook tool".into()),
        };

        match result {
            Ok(output) => ToolResult {
                tool_name: tool,
                action,
                ok: !output.contains("HTTP Error"),
                output,
            },
            Err(e) => ToolResult {
                tool_name: tool,
                action,
                ok: false,
                output: format!("Moltbook Error: {}", e),
            },
        }
    }
}
