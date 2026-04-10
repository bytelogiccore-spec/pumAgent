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
            for (key, _) in entries {
                if let Ok(name) = String::from_utf8(key) {
                    list_md.push_str(&format!("- {}\n", name));
                    count += 1;
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
}
