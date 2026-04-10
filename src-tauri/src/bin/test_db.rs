use dbx_core::Database;

fn main() {
    let mut base_dir = std::env::current_dir().unwrap();
    base_dir.push(".."); // PumAgent_Rust
    base_dir.push(".."); // AI
    base_dir.push("PumAgentData");

    let db_path = base_dir.join("pumagent_store.dbx");
    println!("Opening DB at {:?}", db_path);
    let db = Database::open(&db_path).unwrap();

    println!("--- brain_artifacts ---");
    if let Ok(entries) = db.scan("brain_artifacts") {
        for (k, v) in entries {
            println!("Key: {:?} (len: {}) => Val len: {}", k, k.len(), v.len());
            if let Ok(s) = String::from_utf8(k) {
                println!("  String Key: '{}'", s);
                println!(
                    "  Get test: {}",
                    db.get("brain_artifacts", s.as_bytes()).unwrap().is_some()
                );
            }
        }
    }

    println!("--- knowledge_base ---");
    if let Ok(entries) = db.scan("knowledge_base") {
        for (k, v) in entries {
            println!("Key: {:?} (len: {}) => Val len: {}", k, k.len(), v.len());
            if let Ok(s) = String::from_utf8(k) {
                println!("  String Key: '{}'", s);
                println!(
                    "  Get test: {}",
                    db.get("knowledge_base", s.as_bytes()).unwrap().is_some()
                );
            }
        }
    }
}
