use crate::agent::multi_agent::ToolResult;
use crate::tools::http_tool::HttpTool;
use rhai::{Dynamic, Engine, EvalAltResult, Scope};
use std::collections::HashMap;

pub struct ScriptingTool {}

impl Default for ScriptingTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptingTool {
    pub fn new() -> Self {
        ScriptingTool {}
    }

    pub async fn execute_action(
        &self,
        tool: String,
        action: String,
        args: serde_json::Value,
    ) -> ToolResult {
        if action != "run_rhai" {
            return ToolResult {
                tool_name: tool,
                action,
                ok: false,
                output: "Unknown action for scripting tool.".into(),
            };
        }

        let script = match args.get("script").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => {
                return ToolResult {
                    tool_name: tool,
                    action,
                    ok: false,
                    output: "Missing required arg: script".into(),
                }
            }
        };

        // We run rhai in a blocking task to avoid locking up tokio async workers
        let res = tokio::task::spawn_blocking(move || {
            let mut engine = Engine::new();

            // Expose http_get to Rhai
            engine.register_fn(
                "http_get",
                |url: &str| -> Result<String, Box<EvalAltResult>> {
                    tokio::runtime::Handle::current().block_on(async move {
                        let http = HttpTool::new();
                        match http.execute("GET", url, None, None).await {
                            Ok(res) => Ok(res),
                            Err(e) => Err(format!("HTTP Error: {}", e).into()),
                        }
                    })
                },
            );

            // Expose http_post to Rhai
            // Takes url, headers as JSON string (or "{}" for empty), and body as string
            engine.register_fn(
                "http_post",
                |url: &str, headers_json: &str, body: &str| -> Result<String, Box<EvalAltResult>> {
                    // Parse headers map
                    let parsed_headers: Option<HashMap<String, String>> =
                        serde_json::from_str(headers_json).unwrap_or(None);
                    tokio::runtime::Handle::current().block_on(async move {
                        let http = HttpTool::new();
                        match http
                            .execute("POST", url, parsed_headers, Some(body.to_string()))
                            .await
                        {
                            Ok(res) => Ok(res),
                            Err(e) => Err(format!("HTTP Error: {}", e).into()),
                        }
                    })
                },
            );

            // We can add print capability
            engine.on_print(|x| log::info!("Rhai Print: {}", x));

            let mut scope = Scope::new();
            match engine.eval_with_scope::<Dynamic>(&mut scope, &script) {
                Ok(result) => Ok(format!("{}", result)),
                Err(e) => Err(format!("Script Execution Error: {}", e)),
            }
        })
        .await;

        match res {
            Ok(Ok(output)) => ToolResult {
                tool_name: tool,
                action,
                ok: true,
                output: format!("Script result: {}", output),
            },
            Ok(Err(e)) => ToolResult {
                tool_name: tool,
                action,
                ok: false,
                output: e,
            },
            Err(e) => ToolResult {
                tool_name: tool,
                action,
                ok: false,
                output: format!("Task Panic Error: {}", e),
            },
        }
    }
}
