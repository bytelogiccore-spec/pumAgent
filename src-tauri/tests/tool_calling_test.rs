use app_lib::agent::llm_client::{ChatMessage, LLMClient};

#[tokio::test]
async fn test_comprehensive_native_tool_calling() {
    println!("==================================================");
    println!("🚀 [NEW SCENARIO] Comprehensive Native Tool Calling Test 🚀");
    println!("목표: LLM이 다양한 유저 요청에 따라 알맞은 Native 툴(crawler, telegram, terminal 등)을 정확히 호출하는지 검증");
    println!("==================================================");

    // LLM 엔드포인트 세팅 (이전 시나리오와 동일한 로컬 llama-server IP 참조)
    let endpoint = "http://192.168.219.111:8000/v1/chat/completions";
    let model = "auto";
    let api_key = "local";
    let client = LLMClient::new(endpoint.into(), model.into(), api_key.into());

    // 누락되었던 툴들에 대한 스키마 정의
    let tools_schema = vec![
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "telegram",
                "description": "Send a proactive push notification to the user's mobile Telegram.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["send_message"] },
                        "args": {
                            "type": "object",
                            "properties": {
                                "message": { "type": "string" }
                            }
                        }
                    },
                    "required": ["action", "args"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "crawler",
                "description": "Scrape and extract raw text from a website HTML url.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["scrape"] },
                        "args": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "The valid web URL to parse." }
                            }
                        }
                    },
                    "required": ["action", "args"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "terminal",
                "description": "Run standard local CLI commands (PowerShell).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["execute"] },
                        "args": {
                            "type": "object",
                            "properties": {
                                "command": { "type": "string", "description": "The PowerShell command to run." }
                            }
                        }
                    },
                    "required": ["action", "args"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "scripting",
                "description": "Run a dynamically generated scripting code block (Node.js/Rhia).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["run_node", "run_rhai"] },
                        "args": {
                            "type": "object",
                            "properties": {
                                "code": { "type": "string", "description": "The logic code to execute natively." }
                            }
                        }
                    },
                    "required": ["action", "args"]
                }
            }
        }),
    ];

    let system_prompt = ChatMessage {
        role: "system".into(),
        content: "You are a robust and helpful AI. Choose exactly ONE appropriate tool when you are explicitly asked or need it. If no tool is heavily needed, just respond via text.".into(),
        images_base64: None,
    };

    // 시나리오 1: 텔레그램(Telegram) 툴 호출 테스트
    println!("\n[Scenario 1] Telegram Push Notification");
    {
        let messages = vec![
            system_prompt.clone(),
            ChatMessage {
                role: "user".into(),
                content: "작업이 다 끝났다고 텔레그램으로 메시지 하나만 쏴줘!".into(),
                images_base64: None,
            },
        ];

        let response = client
            .clone()
            .with_tools(tools_schema.clone())
            .chat(&messages, 0.7)
            .await
            .expect("LLM 통신 실패");
        let has_tool = response
            .native_tool_calls
            .iter()
            .any(|call| call.get("tool").and_then(|v| v.as_str()) == Some("telegram"));
        assert!(
            has_tool,
            "LLM failed to invoke 'telegram'. Raw native calls: {:?}",
            response.native_tool_calls
        );
        println!("✅ Telegram 툴 콜링 확인 완료.");
    }

    // 시나리오 2: 웹 크롤러(Crawler) 툴 호출 테스트
    println!("\n[Scenario 2] Web Crawler Execution");
    {
        let messages = vec![
            system_prompt.clone(),
            ChatMessage {
                role: "user".into(),
                content: "https://news.ycombinator.com 사이트를 긁어와서 내용을 파악해볼래?".into(),
                images_base64: None,
            },
        ];

        let response = client
            .clone()
            .with_tools(tools_schema.clone())
            .chat(&messages, 0.7)
            .await
            .expect("LLM 통신 실패");
        let has_tool = response
            .native_tool_calls
            .iter()
            .any(|call| call.get("tool").and_then(|v| v.as_str()) == Some("crawler"));
        assert!(
            has_tool,
            "LLM failed to invoke 'crawler'. Raw native calls: {:?}",
            response.native_tool_calls
        );
        println!("✅ Crawler 툴 콜링 확인 완료.");
    }

    // 시나리오 3: 터미널(Terminal) 툴 호출 테스트
    println!("\n[Scenario 3] Terminal CLI Execution");
    {
        let messages = vec![
            system_prompt.clone(),
            ChatMessage { role: "user".into(), content: "로컬 콘솔에 `ipconfig` 명령어를 치고, 반환되는 내역을 바탕으로 내 IP 주소를 찾아줘.".into(), images_base64: None }
        ];

        let response = client
            .clone()
            .with_tools(tools_schema.clone())
            .chat(&messages, 0.7)
            .await
            .expect("LLM 통신 실패");
        let has_tool = response
            .native_tool_calls
            .iter()
            .any(|call| call.get("tool").and_then(|v| v.as_str()) == Some("terminal"));
        assert!(
            has_tool,
            "LLM failed to invoke 'terminal'. Raw native calls: {:?}",
            response.native_tool_calls
        );
        println!("✅ Terminal 툴 콜링 확인 완료.");
    }

    // 시나리오 4: 스크립트 실행(Scripting) 툴 호출 테스트
    println!("\n[Scenario 4] Scripting engine invocation");
    {
        let messages = vec![
            system_prompt.clone(),
            ChatMessage { role: "user".into(), content: "Node.js 엔진을 써서 1부터 100까지 더하는 스크립트를 즉석에서 실행하고 결과를 알려줘.".into(), images_base64: None }
        ];

        let response = client
            .clone()
            .with_tools(tools_schema.clone())
            .chat(&messages, 0.7)
            .await
            .expect("LLM 통신 실패");
        let has_tool = response
            .native_tool_calls
            .iter()
            .any(|call| call.get("tool").and_then(|v| v.as_str()) == Some("scripting"));
        assert!(
            has_tool,
            "LLM failed to invoke 'scripting'. Raw native calls: {:?}",
            response.native_tool_calls
        );
        println!("✅ Scripting 툴 콜링 확인 완료.");
    }

    println!("\n==================================================");
    println!("✅ ALL TOOL CALLING ROUTING SCENARIOS PASSED!");
    println!("==================================================");
}
