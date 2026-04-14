use app_lib::agent::orchestrator::Orchestrator;
use app_lib::agent::multi_agent::MultiAgent;
use app_lib::config::{AppConfig, LlmEndpoint};
use app_lib::agent::llm_client::ChatMessage;
use std::path::PathBuf;
use std::sync::Arc;
use tokio;

#[tokio::test]
async fn test_telegram_logic_simulation() {
    println!("\n=== [Telegram Simulation Diagnostic Starting] ===");
    
    // 1. Setup a controlled environment
    let base_dir = std::env::current_dir().unwrap().join("tests_data_telegram_sim");
    if !base_dir.exists() {
        std::fs::create_dir_all(&base_dir).unwrap();
    }
    
    // 2. Create a config that MANDATES <think> tags and SEARCH (The latest instructions)
    let mut config = AppConfig::load(&base_dir);
    config.planner_prompt = Some("[PLANNER] You MUST use tools for news. ALWAYS use <think> tags.".to_string());
    config.use_multi_agent_workflow = true;
    config.save(&base_dir).expect("Failed to save config for test");

    println!("- Config updated with mandatory search policy.");

    // 3. Initialize MultiAgent (Mocking DB and handle)
    let db_path = base_dir.join("test_sim.dbx");
    let db = dbx_core::Database::open(&db_path).expect("Failed to open test DB");
    
    // We assume the LLM endpoint is already configured in the user's environment.
    // If not, this test will fail on network, which is also a diagnostic.
    let initial_config = AppConfig::load(&base_dir);
    
    // We need to bypass the actual Telegram bot loop and test the process_telegram_request logic
    // But since that uses Bot and UI handles, we'll simulate the ORCHESTRATOR call part directly
    
    println!("- Simulating Incoming Telegram Message: '최신 뉴스 요약해줘'");
    
    let routing_flags = app_lib::agent::orchestrator::OrchestratorRouting {
        endpoints: initial_config.endpoints.clone(),
        planner_id: initial_config.planner_endpoint_id.clone(),
        critic_id: initial_config.critic_endpoint_id.clone(),
        writer_id: initial_config.writer_endpoint_id.clone(),
        worker_id: initial_config.worker_endpoint_id.clone(),
        reflector_id: initial_config.reflector_endpoint_id.clone(),
        registry_id: initial_config.registry_endpoint_id.clone(),
    };

    // We need to create a MultiAgent instance.
    // Since common tools are needed, we initialize them as in lib.rs
    let crawler = app_lib::tools::crawler::Crawler::new();
    let search_tool = app_lib::tools::search::SearchTool::new("".to_string(), "".to_string(), base_dir.clone());
    let brain_tool = app_lib::tools::brain::BrainTool::new(db.clone());
    let terminal_tool = app_lib::tools::terminal::TerminalTool::new(base_dir.clone(), Some(db.clone()));
    let knowledge_tool = app_lib::tools::knowledge::KnowledgeTool::new(db.clone());
    let telegram_tool = app_lib::tools::telegram_tool::TelegramTool::new(base_dir.clone());
    let http_tool = app_lib::tools::http_tool::HttpTool::new();
    let script_tool = app_lib::tools::scripting_tool::ScriptingTool::new();
    let moltbook_tool = app_lib::tools::moltbook_tool::MoltbookTool::new(base_dir.clone());
    let vault_tool = app_lib::tools::vault_tool::VaultTool::new(base_dir.clone());

    let multi_agent = Arc::new(MultiAgent::new(
        crawler, search_tool, brain_tool, terminal_tool, knowledge_tool, 
        telegram_tool, http_tool, script_tool, moltbook_tool, vault_tool
    ));

    let orchestrator = Orchestrator::new(
        routing_flags,
        multi_agent.clone(),
        base_dir.clone(),
        db.clone(),
    );

    let mut session_history = vec![ChatMessage {
        role: "user".to_string(),
        content: "최신 뉴스 요약해줘".to_string(),
        images_base64: None,
    }];

    let (log_tx, mut log_rx) = tokio::sync::mpsc::channel::<String>(100);
    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    // RUN THE PIPELINE
    println!("- Executing Orchestrator Pipeline...");
    
    let res = orchestrator.run_loop(
        Some("Sim_Test".to_string()),
        &initial_config.system_prompt,
        initial_config.planner_prompt.as_deref(),
        initial_config.critic_prompt.as_deref(),
        initial_config.writer_prompt.as_deref(),
        session_history,
        &initial_config.language,
        3, // max loops
        true, // multi agent
        None,
        log_tx,
        cancel_flag
    ).await;

    match res {
        Ok((final_out, history)) => {
            println!("- Pipeline Finished Successfully.");
            println!("- History Length: {}", history.len());
            
            // Check if any tools were called
            let any_tool_calls = history.iter().any(|m| m.role == "assistant" && m.content.contains("\"tool\":"));
            if any_tool_calls {
                println!("✅ SUCCESS: Planner triggered tool calls as expected!");
            } else {
                println!("❌ FAILURE: Planner ONLY gave a social response. Instructions ignored.");
                println!("Raw output: {}", final_out);
            }
            
            // Log thinking process
            for m in history {
                if m.role == "assistant" && (m.content.contains("<think>") || m.content.contains("<thought>")) {
                    println!("- FOUND THINKING BLOCK: [Thinking Detected]");
                }
            }
        },
        Err(e) => {
            println!("❌ ERROR during pipeline: {}", e);
        }
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&base_dir);
    println!("=== Simulation Finished ===\n");
}
