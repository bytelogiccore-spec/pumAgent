use crate::AgentState;
use std::fs;
use tauri::State;

#[tauri::command]
pub fn list_logs(state: State<'_, AgentState>) -> Result<Vec<String>, String> {
    let mut logs = vec![];
    if let Ok(entries) = fs::read_dir(state.base_dir.join("logs")) {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                logs.push(name);
            }
        }
    }
    logs.sort_by(|a, b| b.cmp(a)); // Descending sorting
    Ok(logs)
}

#[tauri::command]
pub fn read_log(name: String, state: State<'_, AgentState>) -> Result<String, String> {
    fs::read_to_string(state.base_dir.join("logs").join(name)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_logs(names: Vec<String>, state: State<'_, AgentState>) -> Result<(), String> {
    for name in names {
        let path = state.base_dir.join("logs").join(&name);
        if let Err(e) = fs::remove_file(path) {
            eprintln!("Failed to delete log {}: {}", name, e);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn list_knowledge(domain: String, state: State<'_, AgentState>) -> Result<Vec<String>, String> {
    let mut files = vec![];
    let prefix = format!("{}:", domain);
    if let Ok(entries) = state.db.scan("knowledge_base") {
        for (key, val) in entries {
            if val != b"__PUM_DELETED__" {
                if let Ok(name) = String::from_utf8(key) {
                    if name.starts_with(&prefix) {
                        files.push(name.replace(&prefix, ""));
                    }
                }
            }
        }
    }
    Ok(files)
}

#[tauri::command]
pub fn read_knowledge(
    domain: String,
    name: String,
    state: State<'_, AgentState>,
) -> Result<String, String> {
    let key = format!("{}:{}", domain, name);
    match state.db.get("knowledge_base", key.as_bytes()) {
        Ok(Some(bytes)) => {
            if bytes == b"__PUM_DELETED__" {
                return Err("Not found".into());
            }
            String::from_utf8(bytes).map_err(|e| e.to_string())
        }
        Ok(None) => Err("Not found".into()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn write_knowledge(
    domain: String,
    name: String,
    content: String,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    let key = format!("{}:{}", domain, name);
    let res = state
        .db
        .insert("knowledge_base", key.as_bytes(), content.as_bytes())
        .map_err(|e| e.to_string());
    let _ = state.db.flush();
    res
}

#[tauri::command]
pub fn delete_knowledge(
    domain: String,
    name: String,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    let key = format!("{}:{}", domain, name);
    let res = state
        .db
        .insert("knowledge_base", key.as_bytes(), b"__PUM_DELETED__")
        .map_err(|e| e.to_string());
    let _ = state.db.flush();
    res
}

#[tauri::command]
pub fn list_brain_artifacts(state: State<'_, AgentState>) -> Result<Vec<String>, String> {
    let mut files = vec![];
    if let Ok(entries) = state.db.scan("brain_artifacts") {
        for (key, val) in entries {
            if val != b"__PUM_DELETED__" {
                if let Ok(name) = String::from_utf8(key) {
                    files.push(name);
                }
            }
        }
    }
    Ok(files)
}

#[tauri::command]
pub fn read_brain_artifact(name: String, state: State<'_, AgentState>) -> Result<String, String> {
    match state.db.get("brain_artifacts", name.as_bytes()) {
        Ok(Some(bytes)) => {
            if bytes == b"__PUM_DELETED__" {
                return Err("Not found".to_string());
            }
            String::from_utf8(bytes).map_err(|e| e.to_string())
        }
        Ok(None) => Err("Not found".to_string()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn write_brain_artifact(
    name: String,
    content: String,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    let res = state
        .db
        .insert("brain_artifacts", name.as_bytes(), content.as_bytes())
        .map_err(|e| e.to_string());
    let _ = state.db.flush();
    res
}

#[tauri::command]
pub fn delete_brain_artifact(name: String, state: State<'_, AgentState>) -> Result<(), String> {
    let res = state
        .db
        .insert("brain_artifacts", name.as_bytes(), b"__PUM_DELETED__")
        .map_err(|e| e.to_string());
    let _ = state.db.flush();
    res
}
