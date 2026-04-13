use dbx_core::Database;
use std::sync::Arc;

pub struct BrainTool {
    db: Arc<Database>,
}

impl BrainTool {
    pub fn new(db: Arc<Database>) -> Self {
        BrainTool { db }
    }

    pub fn list_artifacts(&self) -> Result<String, String> {
        let mut list_md = String::from("### Available Brain Artifacts:\n");
        let mut count = 0;

        if let Ok(entries) = self.db.scan("brain_artifacts") {
            for (key, val) in entries {
                if val != b"__PUM_DELETED__" {
                    if let Ok(name) = String::from_utf8(key) {
                        let content = String::from_utf8_lossy(&val);
                        let mut preview = content.trim().replace('\n', " ");
                        if preview.chars().count() > 100 {
                            let snippet: String = preview.chars().take(100).collect();
                            preview = format!("{}...", snippet);
                        }
                        list_md.push_str(&format!("- **{}** (Preview: {})\n", name, preview));
                        count += 1;
                    }
                }
            }
        }

        if count == 0 {
            return Ok("No brain artifacts stored yet.".to_string());
        }

        Ok(list_md)
    }

    pub fn read_artifact(&self, name: &str) -> Result<String, String> {
        match self.db.get("brain_artifacts", name.as_bytes()) {
            Ok(Some(content_bytes)) => match String::from_utf8(content_bytes) {
                Ok(content) => Ok(content),
                Err(e) => Err(format!("UTF-8 parsing error: {}", e)),
            },
            Ok(None) => Err(format!("Artifact '{}' not found.", name)),
            Err(e) => Err(format!("Database error reading '{}': {}", name, e)),
        }
    }

    pub fn write_artifact(&self, name: &str, content: &str) -> Result<String, String> {
        let safe_name = if !name.ends_with(".md") {
            format!("{}.md", name)
        } else {
            name.to_string()
        };

        match self
            .db
            .insert("brain_artifacts", safe_name.as_bytes(), content.as_bytes())
        {
            Ok(_) => {
                let _ = self.db.flush();
                Ok(format!(
                    "Brain artifact '{}' successfully written/updated in DB.",
                    safe_name
                ))
            }
            Err(e) => Err(format!("Failed to write artifact '{}': {}", safe_name, e)),
        }
    }

    pub fn delete_artifact(&self, name: &str) -> Result<(), String> {
        match self.db.delete("brain_artifacts", name.as_bytes()) {
            Ok(_) => {
                let _ = self.db.flush();
                Ok(())
            }
            Err(e) => Err(format!("Failed to delete artifact '{}': {}", name, e)),
        }
    }

    pub fn execute_action(
        &self,
        tool: String,
        action: String,
        args: serde_json::Value,
    ) -> crate::agent::multi_agent::ToolResult {
        let name = args.get("name").and_then(|n| n.as_str()).unwrap_or("");
        match action.as_str() {
            "list" => {
                let result = self.list_artifacts();
                crate::agent::multi_agent::ToolResult {
                    tool_name: tool,
                    action,
                    ok: result.is_ok(),
                    output: result.unwrap_or_else(|e| e),
                }
            }
            "read" => {
                let result = self.read_artifact(name);
                crate::agent::multi_agent::ToolResult {
                    tool_name: tool,
                    action,
                    ok: result.is_ok(),
                    output: result.unwrap_or_else(|e| e),
                }
            }
            "write_artifact" => {
                let content = args.get("content").and_then(|c| c.as_str()).unwrap_or("");
                let result = self.write_artifact(name, content);
                crate::agent::multi_agent::ToolResult {
                    tool_name: tool,
                    action,
                    ok: result.is_ok(),
                    output: result.unwrap_or_else(|e| e),
                }
            }
            _ => crate::agent::multi_agent::ToolResult {
                tool_name: tool,
                action,
                ok: false,
                output: "Unsupported action for brain tool".into(),
            },
        }
    }
}
