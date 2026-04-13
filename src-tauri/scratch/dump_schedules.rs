use dbx_core::Database;

fn main() {
    let db = Database::new("../knowledge_base").expect("Failed to open DB");
    if let Ok(entries) = db.scan("knowledge_base") {
        for (key, val) in entries {
            let key_str = String::from_utf8_lossy(&key);
            if key_str.starts_with("schedules:") {
                let content = String::from_utf8_lossy(&val);
                println!("--- {} ---", key_str);
                println!("{}", content);
            }
        }
    }
}
