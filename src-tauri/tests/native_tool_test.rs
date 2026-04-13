use app_lib::agent::llm_client::{ChatMessage, LLMClient};

#[tokio::test]
async fn test_native_tool_calling_scenario() {
    println!("==================================================");
    println!("🚀 [NEW SCENARIO] Native Tool Calling API Test 🚀");
    println!(
        "목표: LLM이 텍스트 프롬프트가 아닌 Native Schema를 통해 툴을 인식하고 반환하는지 검증"
    );
    println!("==================================================");

    // LLM 엔드포인트 세팅 (이전 시나리오와 동일한 로컬 llama-server IP 참조)
    let endpoint = "http://192.168.219.111:8000/v1/chat/completions";
    let model = "auto";
    let api_key = "local";
    let client = LLMClient::new(endpoint.into(), model.into(), api_key.into());

    // 테스트용 단일 스키마 (Search)
    let tools_schema = vec![serde_json::json!({
        "type": "function",
        "function": {
            "name": "search",
            "description": "Google Search for finding real-time external information like news or scores.",
            "parameters": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["query"] },
                    "args": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string" },
                            "time_range": { "type": "string" }
                        }
                    }
                },
                "required": ["action", "args"]
            }
        }
    })];

    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: "You are a helpful AI. You must use tools proactively when the user asks about real-time, current events or facts you don't know.".into(),
            images_base64: None,
        },
        ChatMessage {
            role: "user".into(),
            content: "어제 손흥민이 공격포인트를 올렸는지, 경기 결과를 검색해봐.".into(),
            images_base64: None,
        }
    ];

    println!("LLM API (llama-server) 에 Native 요청 전송 중...");

    // Tools schema 파라미터 주입!
    let response = client
        .with_tools(tools_schema)
        .chat(&messages, 0.7)
        .await
        .expect("LLM 서버와 통신 실패 (llama-server 가동 상태와 IP를 확인하세요)");

    println!("\n🔍 [LLM 응답 분석]");
    println!("텍스트 응답 (Fallback Contents): {}", response.content);
    println!(
        "파싱된 네이티브 툴 호출 내역: {}",
        serde_json::to_string_pretty(&response.native_tool_calls).unwrap_or_default()
    );

    // 구형 extract_json_blocks 도 돌려봐서 교차 검증 (아무것도 없어야 정상. 즉 LLM이 마크다운 블록을 뱉은게 아니어야함)
    let old_style_blocks = app_lib::agent::parser::extract_json_blocks(&response.content);
    println!(
        "구형 정규식 파서에 걸린 블록 수: {}",
        old_style_blocks.len()
    );

    let has_native_search = response.native_tool_calls.iter().any(|call| {
        let tool = call.get("tool").and_then(|v| v.as_str()).unwrap_or("");
        tool == "search"
    });

    assert!(
        has_native_search,
        "LLM이 네이티브 툴 'search' 에 대한 호출(tool_calls) 덩어리를 생성하지 못했습니다! 
        모델이 Native Format을 거부했거나 프롬프트를 무시했습니다."
    );

    // Old block length validation (Optional, some models might duplicate it out of habit, but ideally it's 0)
    // if old_style_blocks.len() > 0 { println!("경고: 모델이 네이티브 툴 콜 뿐만 아니라 마크다운 텍스트도 같이 내뱉는 비효율적인 행동을 보입니다."); }

    println!("--------------------------------------------------");
    println!("✅ NATIVE TOOL CALLING SUCCESS! 모델이 JSON Schema를 완벽히 이해했습니다.");
    println!("==================================================");
}
