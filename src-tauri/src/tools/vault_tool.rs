use crate::agent::multi_agent::ToolResult;
use keyring::Entry;

pub struct VaultTool;

impl VaultTool {
    pub fn new() -> Self {
        VaultTool
    }

    pub fn set_secret(&self, key: &str, value: &str) -> Result<(), String> {
        let entry = Entry::new("PumAgentVault", key).map_err(|e| e.to_string())?;
        entry.set_password(value).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_secret(&self, key: &str) -> Result<(), String> {
        let entry = Entry::new("PumAgentVault", key).map_err(|e| e.to_string())?;
        entry.delete_credential().map_err(|e: keyring::Error| e.to_string())?;
        Ok(())
    }

    pub fn get_secret(key: &str) -> Result<String, String> {
        let entry = Entry::new("PumAgentVault", key).map_err(|e| e.to_string())?;
        entry.get_password().map_err(|e| e.to_string())
    }

    pub fn execute_action(
        &self,
        tool: String,
        action: String,
        args: serde_json::Value,
    ) -> ToolResult {
        let ok;
        let mut output = String::new();

        match action.as_str() {
            "set" => {
                if let (Some(key), Some(value)) = (
                    args.get("key").and_then(|v| v.as_str()),
                    args.get("value").and_then(|v| v.as_str()),
                ) {
                    match self.set_secret(key, value) {
                        Ok(_) => {
                            ok = true;
                            output = format!("Secret '{}' securely saved in OS Keyring. You can use it in other tools via {{{{vault:{}}}}}", key, key);
                        }
                        Err(e) => {
                            ok = false;
                            output = format!("Failed to save secret: {}", e);
                        }
                    }
                } else {
                    ok = false;
                    output = "Missing 'key' or 'value' arguments.".to_string();
                }
            }
            "delete" => {
                if let Some(key) = args.get("key").and_then(|v| v.as_str()) {
                    match self.delete_secret(key) {
                        Ok(_) => {
                            ok = true;
                            output = format!("Secret '{}' removed from Keyring.", key);
                        }
                        Err(e) => {
                            ok = false;
                            output = format!("Failed to delete secret: {}", e);
                        }
                    }
                } else {
                    ok = false;
                    output = "Missing 'key' argument.".to_string();
                }
            }
            _ => {
                ok = false;
                output = "Unknown action for vault tool. Allowed: set, delete.".to_string();
            }
        }

        ToolResult {
            tool_name: tool,
            action,
            ok,
            output,
        }
    }
}
