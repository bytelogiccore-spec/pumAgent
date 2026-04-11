use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use app_lib::agent::llm_client::ChatMessage;
use app_lib::agent::multi_agent::MultiAgent;
use app_lib::agent::orchestrator::{Orchestrator, OrchestratorRouting};
use app_lib::config::LlmEndpoint;
use app_lib::tools::brain::BrainTool;
use app_lib::tools::crawler::Crawler;
use app_lib::tools::knowledge::KnowledgeTool;
use app_lib::tools::search::SearchTool;
use app_lib::tools::telegram_tool::TelegramTool;
use app_lib::tools::terminal::TerminalTool;

// A simple mock HTTP server for mimicking OpenAI Chat Completions API
async fn start_mock_llm_server(port: u16) {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .unwrap();
    let mut request_count = 0;

    tokio::spawn(async move {
        loop {
            if let Ok((mut socket, _)) = listener.accept().await {
                // Read the request but ignore it
                let mut buf = [0; 4096];
                let _ = socket.read(&mut buf).await;

                request_count += 1;
                let response_body = match request_count {
                    1 => {
                        // 1. Planner decides to use `brain` tool to list artifacts
                        serde_json::json!({
                            "choices": [{
                                "message": {
                                    "role": "assistant",
                                    "content": "",
                                    "tool_calls": [{
                                        "id": "call_mock123",
                                        "type": "function",
                                        "function": {
                                            "name": "brain",
                                            "arguments": "{\"action\": \"list\"}"
                                        }
                                    }]
                                }
                            }]
                        })
                    }
                    2 => {
                        // 2. Planner reviews tool execution and exits loop
                        serde_json::json!({
                            "choices": [{
                                "message": {
                                    "role": "assistant",
                                    "content": "Tools successfully used. I found the file list.",
                                    "tool_calls": null
                                }
                            }]
                        })
                    }
                    _ => {
                        // 3. Writer synthesizes final output
                        serde_json::json!({
                            "choices": [{
                                "message": {
                                    "role": "assistant",
                                    "content": "FINAL RESPONSE: The brain list was requested successfully.",
                                    "tool_calls": null
                                }
                            }]
                        })
                    }
                };

                // Sleep to allow reqwest to send the full payload to the buffer before we unilaterally close
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;

                let body_str = serde_json::to_string(&response_body).unwrap();
                let http_response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body_str.len(),
                    body_str
                );
                let _ = socket.write_all(http_response.as_bytes()).await;
                let _ = socket.flush().await;
            }
        }
    });
}

#[tokio::test]
async fn test_native_tool_calling_e2e() {
    let mock_port = 8124;
    start_mock_llm_server(mock_port).await;

    // Create Temporary Database for the tools
    let temp_db_path = std::env::temp_dir().join("test_integration.dbx");
    let db = Arc::new(dbx_core::Database::open(&temp_db_path).unwrap());

    // Insert dummy artifact to test the brain tool
    let _ = db.insert("brain_artifacts", b"TestArtifact.md", b"Test Content");

    // Tools Setup
    let crawler = Crawler::new();
    let search_tool = SearchTool::new(
        "".to_string(),
        "".to_string(),
        std::path::PathBuf::from("/"),
    );
    let brain_tool = BrainTool::new(Arc::clone(&db));
    let terminal_tool = TerminalTool::new(std::path::PathBuf::from("/"));
    let knowledge_tool = KnowledgeTool::new(Arc::clone(&db));
    let telegram_tool = TelegramTool::new(std::path::PathBuf::from("/"));

    let agent = Arc::new(MultiAgent::new(
        crawler,
        search_tool,
        brain_tool,
        terminal_tool,
        knowledge_tool,
        telegram_tool,
    ));

    // Orchestrator Setup
    let endpoint = LlmEndpoint {
        id: "mock_model".to_string(),
        name: "Mock Model".to_string(),
        api_url: format!("http://127.0.0.1:{}/v1/chat/completions", mock_port),
        model: "gpt-mock".to_string(),
        api_key: "dummy".to_string(),
        is_enabled: true,
    };

    let routing = OrchestratorRouting {
        endpoints: vec![endpoint],
        planner_id: "mock_model".to_string(),
        critic_id: "mock_model".to_string(),
        writer_id: "mock_model".to_string(),
        worker_id: "mock_model".to_string(),
        reflector_id: "mock_model".to_string(),
        registry_id: "mock_model".to_string(),
    };

    let (log_tx, mut log_rx) = tokio::sync::mpsc::channel(100);

    // Launch log reader task to prevent channel blocking
    tokio::spawn(async move {
        while let Some(msg) = log_rx.recv().await {
            println!("LOG: {}", msg);
        }
    });

    let orchestrator = Orchestrator::new(
        routing,
        agent,
        std::path::PathBuf::from("/"),
        Arc::clone(&db),
    );

    let initial_message = vec![ChatMessage {
        role: "user".to_string(),
        content: "What files do I have in my brain?".to_string(),
        images_base64: None,
        tool_calls: None,
        tool_call_id: None,
    }];

    // Execute the E2E Agent Pipeline
    let (final_response, history) = orchestrator
        .run_multi_agent_pipeline(
            Some("test_session".to_string()),
            "You are a helpful assistant",
            None,
            None,
            None,
            initial_message,
            "en",
            5,
            None,
            log_tx,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .await
        .unwrap();

    println!("E2E Final output: {}", final_response);

    // Assert that the final response generated by the Writer matches the 3rd mock HTTP hook
    assert!(final_response.contains("FINAL RESPONSE"));

    // Check history length
    // History should contain:
    // 1. Initial Prompt (System Planner)
    // 2. User Message
    // 3. Assistant Message (tool_calls invoking 'brain')
    // 4. Tool Message ('brain' result with artifact TestArtifact.md)
    // 5. Assistant Message (Planner loop review empty)
    // 6. System Prompt (Writer config)
    // 7. Writer Request User formatting
    // 8. Assistant Message (FINAL RESPONSE)

    // Check if the history correctly tracked the tool_call_id
    let has_tool_resp = history
        .iter()
        .any(|msg| msg.role == "tool" && msg.content.contains("TestArtifact.md"));
    assert!(
        has_tool_resp,
        "History failed to record role: tool message correctly"
    );
}
