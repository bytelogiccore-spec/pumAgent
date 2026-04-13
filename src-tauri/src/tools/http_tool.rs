use rquest::Client;
use std::error::Error;
use std::collections::HashMap;

pub struct HttpTool {}

impl Default for HttpTool {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpTool {
    pub fn new() -> Self {
        HttpTool {}
    }

    pub async fn execute(
        &self,
        method: &str,
        url: &str,
        headers: Option<HashMap<String, String>>,
        body: Option<String>,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let mut request = match method.to_uppercase().as_str() {
            "GET" => client.get(url),
            "POST" => client.post(url),
            "PUT" => client.put(url),
            "DELETE" => client.delete(url),
            "PATCH" => client.patch(url),
            _ => return Err(format!("Unsupported HTTP method: {}", method).into()),
        };

        if let Some(h) = headers {
            for (k, v) in h {
                request = request.header(k, v);
            }
        }

        if let Some(b) = body {
            if !b.trim().is_empty() {
                request = request.body(b);
            }
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
    ) -> crate::agent::multi_agent::ToolResult {
        if action != "request" {
            return crate::agent::multi_agent::ToolResult {
                tool_name: tool,
                action,
                ok: false,
                output: "Unknown action for http tool.".into(),
            };
        }

        let method = args.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
        let url = match args.get("url").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => {
                return crate::agent::multi_agent::ToolResult {
                    tool_name: tool,
                    action,
                    ok: false,
                    output: "Missing required arg: url".into(),
                }
            }
        };

        let headers = args.get("headers").and_then(|v| v.as_object()).map(|obj| {
            let mut map = HashMap::new();
            for (k, v) in obj {
                if let Some(s) = v.as_str() {
                    map.insert(k.clone(), s.to_string());
                }
            }
            map
        });

        let body = args.get("body").map(|v| {
            if v.is_string() {
                v.as_str().unwrap().to_string()
            } else {
                v.to_string()
            }
        });

        match self.execute(method, url, headers, body).await {
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
                output: format!("HTTP Error: {}", e),
            },
        }
    }
}
