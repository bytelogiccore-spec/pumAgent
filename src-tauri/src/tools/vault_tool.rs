use crate::agent::multi_agent::ToolResult;
use keyring::Entry;

use std::path::PathBuf;
use std::fs;

pub struct VaultTool {
    base_dir: PathBuf,
}

impl VaultTool {
    pub fn new(base_dir: PathBuf) -> Self {
        VaultTool { base_dir }
    }

    fn index_file(&self) -> PathBuf {
        self.base_dir.join("vault_keys.json")
    }

    pub fn list_keys(&self) -> Vec<String> {
        if let Ok(data) = fs::read_to_string(self.index_file()) {
            if let Ok(keys) = serde_json::from_str::<Vec<String>>(&data) {
                return keys;
            }
        }
        Vec::new()
    }

    fn add_to_index(&self, key: &str) {
        let mut keys = self.list_keys();
        if !keys.contains(&key.to_string()) {
            keys.push(key.to_string());
            if let Ok(data) = serde_json::to_string(&keys) {
                let _ = fs::write(self.index_file(), data);
            }
        }
    }

    fn remove_from_index(&self, key: &str) {
        let mut keys = self.list_keys();
        if let Some(pos) = keys.iter().position(|k| k == key) {
            keys.remove(pos);
            if let Ok(data) = serde_json::to_string(&keys) {
                let _ = fs::write(self.index_file(), data);
            }
        }
    }

    pub fn set_secret(&self, key: &str, value: &str) -> Result<(), String> {
        let entry = Entry::new("PumAgentVault", key).map_err(|e| e.to_string())?;
        entry.set_password(value).map_err(|e| e.to_string())?;
        self.add_to_index(key);
        Ok(())
    }

    pub fn delete_secret(&self, key: &str) -> Result<(), String> {
        let entry = Entry::new("PumAgentVault", key).map_err(|e| e.to_string())?;
        entry.delete_credential().map_err(|e: keyring::Error| e.to_string())?;
        self.remove_from_index(key);
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
