use dbx_core::Database;
use std::sync::Arc;

pub struct KnowledgeTool {
    db: Arc<Database>,
}

impl KnowledgeTool {
    pub fn new(db: Arc<Database>) -> Self {
        KnowledgeTool { db }
    }

    fn resolve_key(&self, domain: &str, name: &str) -> Result<String, String> {
        if !["skills", "rules", "workflows", "schedules", "locales"].contains(&domain) {
            return Err("Invalid domain. Must be 'skills', 'rules', 'workflows', 'schedules', or 'locales'.".to_string());
        }
        let ext_name =
            if !name.ends_with(".md") && !name.ends_with(".json") && !name.ends_with(".txt") {
                format!("{}.md", name)
            } else {
                name.to_string()
            };
        Ok(format!("{}:{}", domain, ext_name))
    }

    pub fn list(&self, domain: &str) -> Result<String, String> {
        if !["skills", "rules", "workflows", "schedules", "locales"].contains(&domain) {
            return Err("Invalid domain.".to_string());
        }
        let prefix = format!("{}:", domain);
        let mut files = Vec::new();

        if let Ok(entries) = self.db.scan("knowledge_base") {
            for (key, _) in entries {
                if let Ok(key_str) = String::from_utf8(key) {
                    if key_str.starts_with(&prefix) {
                        files.push(key_str.replace(&prefix, ""));
                    }
                }
            }
        }

        if files.is_empty() {
            return Ok(format!("No items found in {}.", domain));
        }
        Ok(files.join("\n"))
    }

    pub fn read(&self, domain: &str, name: &str) -> Result<String, String> {
        let key = self.resolve_key(domain, name)?;
        match self.db.get("knowledge_base", key.as_bytes()) {
            Ok(Some(content_bytes)) => match String::from_utf8(content_bytes) {
                Ok(content) => Ok(content),
                Err(e) => Err(format!("UTF-8 parsing error: {}", e)),
            },
            Ok(None) => Err(format!("Item {} not found in {}.", name, domain)),
            Err(e) => Err(format!("Database error reading '{}': {}", name, e)),
        }
    }

    pub fn write(&self, domain: &str, name: &str, content: &str) -> Result<String, String> {
        let key = self.resolve_key(domain, name)?;
        match self
            .db
            .insert("knowledge_base", key.as_bytes(), content.as_bytes())
        {
            Ok(_) => {
                let _ = self.db.flush();
                Ok(format!("Successfully wrote {} to {}.", name, domain))
            }
            Err(e) => Err(format!("Failed to write {}: {}", key, e)),
        }
    }

    pub fn delete(&self, domain: &str, name: &str) -> Result<String, String> {
        let key = self.resolve_key(domain, name)?;
        match self.db.delete("knowledge_base", key.as_bytes()) {
            Ok(_) => {
                let _ = self.db.flush();
                Ok(format!("Successfully deleted {} from {}.", name, domain))
            }
            Err(e) => Err(format!("Failed to delete {}: {}", key, e)),
        }
    }

    pub fn read_all_schedules(&self) -> String {
        let mut all_schedules = Vec::new();
        let prefix = "schedules:";

        if let Ok(entries) = self.db.scan("knowledge_base") {
            for (key, val) in entries {
                if let Ok(key_str) = String::from_utf8(key) {
                    if key_str.starts_with(prefix) {
                        let file_name = key_str.replace(prefix, "");
                        if let Ok(content) = String::from_utf8(val) {
                            all_schedules.push(format!("---\n[{}]\n{}\n", file_name, content));
                        }
                    }
                }
            }
        }

        if all_schedules.is_empty() {
            "No schedules registered yet.".to_string()
        } else {
            all_schedules.join("\n")
        }
    }
}
