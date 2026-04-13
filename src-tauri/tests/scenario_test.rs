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
    // 실제 프로덕션에서 사용되는 최신 프롬프트를 프론트엔드 코드(constants.ts)에서 직접 읽어와 테스트합니다.
    let constants_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/lib/constants.ts");
    let constants_content = std::fs::read_to_string(&constants_path).expect("Failed to read constants.ts");
    let start_tag = "export const DEFAULT_PLANNER = `";
    let start_idx = constants_content.find(start_tag).expect("Could not find DEFAULT_PLANNER") + start_tag.len();
    let end_idx = constants_content[start_idx..].find("`;").expect("Could not find end of DEFAULT_PLANNER") + start_idx;
    
    // 치환자를 빈 문자열이나 기본값으로 정리하여 완전한 프롬프트로 만듭니다.
    let actual_prompt = constants_content[start_idx..end_idx]
        .replace("{SYSTEM_PROMPT}", "You are the PumAgent.")
        .replace("{CURRENT_TIME}", "2026-04-13T12:00:00Z")
        .replace("{BRAIN_FILES}", "")
        .replace("{SKILLS_RULES}", "");

    // AI에게 시나리오 던지기
    let response = client.chat(&vec![
        ChatMessage { role: "system".into(), content: actual_prompt.clone(), images_base64: None },
        ChatMessage { role: "user".into(), content: "몰트북 최신피드 5개 조사해줘".into(), images_base64: None },
    ], 0.7).await.expect("LLM 서버와 통신 실패 (llama-server가 켜져 있는지 확인하세요)");

    let text = response.content;
    println!("로컬 LLM (Gemma) 응답 분석 중...");
    println!("Raw Reply:\n{}", text);

    // AI가 몰트북 도구를 올바르게 사용하기 위해 도구 호출을 생성했는지 검증
    // 단순 문자열 매칭 대신, 실제 프로덕션에서 사용하는 parser를 사용하여 검증합니다.
    let tool_calls = app_lib::agent::parser::extract_json_blocks(&text);
    
    let has_moltbook_feed = tool_calls.iter().any(|call| {
        let tool = call.get("tool").and_then(|v| v.as_str()).unwrap_or("");
        let action = call.get("action").and_then(|v| v.as_str()).unwrap_or("");
        tool == "moltbook" && action == "feed"
    });
    
    assert!(has_moltbook_feed, "LLM이 moltbook feed 도구를 호출하지 않았거나 파서가 인식할 수 없는 형식을 사용했습니다! Response: {}", text);

    println!("--------------------------------------------------");
    println!("[TEST PHASE 3] 플래너 도구 에러 발생 시 자가 디버깅(Recovery) 시나리오");
    
    // 만약 실제 툴이 에러가 났다고 가정한 피드백 루프
    let artificial_error = "Tool execution failed! Error: [Moltbook Error: 401 Unauthorized - Invalid Access Token]. Please analyze the cause of this error and output a new JSON block using a different tool (like 'search') to find a solution, or explain the error.";
    
    let mut debug_history = vec![
        ChatMessage { role: "system".into(), content: actual_prompt.clone(), images_base64: None },
        ChatMessage { role: "user".into(), content: "몰트북 최신피드 5개 조사해줘".into(), images_base64: None },
        ChatMessage { role: "assistant".into(), content: text, images_base64: None },
        ChatMessage { role: "user".into(), content: artificial_error.into(), images_base64: None },
    ];

    let debug_response = client.chat(&debug_history, 0.7).await.expect("LLM 통신 실패");
    println!("에러 발생 후 플래너의 분석/복구 반응:\n{}", debug_response.content);

    // 검증: LLM이 에러를 인식하고 search 등의 대안을 탐색하거나 에러를 분석하는 내용이 들어있는지 확인
    let lower_resp = debug_response.content.to_lowercase();
    assert!(
        lower_resp.contains("tool") || 
        lower_resp.contains("401") || 
        lower_resp.contains("unauthorized") ||
        lower_resp.contains("token") ||
        lower_resp.contains("search"),
        "LLM이 에러의 원인을 찾거나 우회하려는 시도를 하지 않았습니다!"
    );

    println!("--------------------------------------------------");
    println!("ALL SCENARIOS PASSED SUCCESSFULLY! 🚀");
}

#[tokio::test]
async fn debug_vault_check() {
    use keyring::Entry;
    println!("--- Vault Diagnostic Start ---");
    let entry = Entry::new("PumAgentVault", "moltbook_creds");
    match entry {
        Ok(e) => {
            match e.get_password() {
                Ok(pw) => {
                    println!("SUCCESS: 'moltbook_creds' found in Vault.");
                    println!("LENGTH: {} characters", pw.len());
                    println!("PREVIEW: {}...", pw.chars().take(5).collect::<String>());
                    if pw.trim().starts_with('{') {
                        println!("FORMAT: Looks like JSON");
                    } else {
                        println!("FORMAT: Looks like RAW STRING");
                    }
                },
                Err(err) => println!("ERROR: Could not retrieve password for 'moltbook_creds': {}", err),
            }
        },
        Err(err) => println!("ERROR: Could not create keyring entry: {}", err),
    }
    println!("--- Vault Diagnostic End ---");
}
