use app_lib::tools::search::SearchTool;
use app_lib::config::AppConfig;
use std::path::PathBuf;
use tokio;

#[tokio::test]
async fn test_search_connectivity_diagnostic() {
    println!("\n=== [Search Connectivity Diagnostic Starting] ===");
    
    // 1. Setup minimal base_dir for config
    let base_dir = std::env::current_dir().unwrap().join("tests_data_search");
    if !base_dir.exists() {
        std::fs::create_dir_all(&base_dir).unwrap();
    }
    
    // Create a dummy config (search_provider=duckduckgo by default)
    let config_path = base_dir.join("agent_config.json");
    if !config_path.exists() {
        let default_config = AppConfig::load(&base_dir); // returns default
        let _ = default_config.save(&base_dir);
    }
    
    let search_tool = SearchTool::new("".to_string(), "".to_string(), base_dir.clone());
    
    println!("--- Running Stress Test (10 consecutive queries) ---");
    for i in 1..=10 {
        let q = format!("news test query {}", i);
        match search_tool.search(&q, None, 5).await {
            Ok(results) => {
                println!("[{}/10] SUCCESS: Found {} results.", i, results.len());
            },
            Err(e) => {
                println!("[{}/10] FAILURE: {}", i, e);
            }
        }
    }
    
    println!("\n=== Diagnostic Finished ===\n");
    
    // Cleanup
    let _ = std::fs::remove_dir_all(&base_dir);
}
