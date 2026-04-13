use app_lib::agent::llm_client::{ChatMessage, LLMClient};
use app_lib::tools::moltbook_tool::MoltbookTool;
use std::path::PathBuf;

#[tokio::test]
async fn test_moltbook_feed_scenario() {
    // 1. LLM Endpoint 설정 (사용자 로컬 llama-server)
    let endpoint = "http://192.168.219.111:8000/v1/chat/completions";
    let model = "auto";
    let api_key = "local"; // 로컬 LLM은 키 무시
    let mut client = LLMClient::new(endpoint.into(), model.into(), api_key.into());

    println!("--------------------------------------------------");
    println!("[TEST PHASE 1] Moltbook Tool 자체 통신 테스트 (자동 가입 및 예외처리)");
    let data_dir = PathBuf::from("d:/ByteLogicCore/AI/PumAgentData");
    let molt_tool = MoltbookTool::new(data_dir);
    
    // AI의 개입 없이 실제 Rust 도구가 몰트북 백엔드와 정상 통신 및 자가 치유(Auto-Register) 하는지 테스트
    let result = molt_tool.execute_action(
        "moltbook".into(),
        "feed".into(),
        serde_json::json!({ "sort": "new" })
    ).await;
    
    println!("Moltbook API 응답 결과 (최대 200자): \n{}", result.output.chars().take(200).collect::<String>());
    assert!(result.ok, "Moltbook tool failed: {}", result.output);
    assert!(!result.output.contains("No Moltbook API key found")); // 409나 API 키 오류가 안 나야 함

    println!("--------------------------------------------------");
    println!("[TEST PHASE 2] LLM 파이프라인 (Planner) 도구 호출 시나리오 테스트");
    let prompt = r#"[CRITICAL TOOL INSTRUCTIONS & WORKFLOW RULES]
1. You are the PLANNER and RESEARCHER. Your job is to gather data using tools.
2. You have the following tools:
   - "moltbook": (action: "feed", args: {}) -> Natively interact with Moltbook API (social network for AI agents). Use 'feed' to read the latest global timeline.
3. To use a tool, output ONLY the following JSON block format:
```json
{ "tool": "moltbook", "action": "feed", "args": {} }
```
4. ONLY output JSON blocks when you need more information."#;

    // AI에게 시나리오 던지기
    let response = client.chat(&vec![
        ChatMessage { role: "system".into(), content: prompt.into(), images_base64: None },
        ChatMessage { role: "user".into(), content: "몰트북 최신피드 5개 조사해줘".into(), images_base64: None },
    ], 0.7).await.expect("LLM 서버와 통신 실패 (llama-server가 켜져 있는지 확인하세요)");

    let text = response.content;
    println!("로컬 LLM (Gemma) 응답 분석 중...");
    println!("Raw Reply:\n{}", text);

    // AI가 몰트북 도구를 올바르게 사용하기 위해 JSON 블록을 출력했는지 검증
    let contains_tool = text.contains("\"tool\": \"moltbook\"") || text.contains("\"tool\":\"moltbook\"");
    let contains_action = text.contains("\"action\": \"feed\"") || text.contains("\"action\":\"feed\"");
    
    assert!(contains_tool && contains_action, "LLM이 도구 사용 지침(JSON format)을 준수하지 않았습니다!");

    println!("--------------------------------------------------");
    println!("ALL SCENARIOS PASSED SUCCESSFULLY! 🚀");
}
