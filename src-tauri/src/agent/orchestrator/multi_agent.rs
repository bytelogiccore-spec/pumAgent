use crate::agent::llm_client::{ChatMessage, LLMResult};
use crate::agent::parser::extract_json_blocks;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

impl super::Orchestrator {
    #[allow(clippy::too_many_arguments)]
    pub async fn run_multi_agent_pipeline(
        &self,
        session_id: Option<String>,
        system_prompt: &str,
        planner_prompt_opt: Option<&str>,
        critic_prompt_opt: Option<&str>,
        writer_prompt_opt: Option<&str>,
        user_messages: Vec<ChatMessage>,
        language: &str,
        max_loops: u32,
        registry_prompt_opt: Option<&str>,
        log_tx: mpsc::Sender<String>,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<(String, Vec<ChatMessage>), String> {
        let is_background_run = user_messages.is_empty();

        let (current_time, brain_files_md, schedule_files_md, mut pending_tasks, skills_rules) =
            self.build_context();

        let (lang_name, _lang_native) = match language {
            "en" => ("ENGLISH".to_string(), "English".to_string()),
            "ko" => ("KOREAN".to_string(), "한국어".to_string()),
            _ => (language.to_uppercase(), language.to_string()),
        };

let fallback_planner_prompt = format!(r#"[PLANNER SYSTEM PROMPT]
{sys}

{skills}

[SYSTEM TIME ANCHOR]
Current System Time: {current_time} (You live in this exact present moment. Use this to accurately calculate "recent", "upcoming", "past", and assess dates from search results).

[CRITICAL TOOL INSTRUCTIONS & WORKFLOW RULES]
1. You are the PLANNER and RESEARCHER. Your job is to gather data using tools.
2. You have the following tools:
   - "search": (action: "query", args: {{"query": "string", "time_range": "d|w|m|y"}}) -> Google Search.
   - "crawl4ai": (action: "scrape", args: {{"url": "string"}}) -> Read webpage content.
   - "brain": (action: "list", args: {{}}) -> List all your long-term memory artifact files.
   - "brain": (action: "read", args: {{"name": "filename.md"}}) -> Read the precise content of a specific memory artifact file.
   - "brain": (action: "write_artifact", args: {{"name": "filename.md", "content": "markdown string"}}) -> Create or overwrite a semantic long-term memory document.
   - "terminal": (action: "execute", args: {{"command": "string"}}) -> Execute Powershell commands. *WARNING: You are sandboxed to the `./Work/` folder.*
   - "knowledge": (action: "read"|"write"|"list"|"delete", args: {{"domain": "skills"|"rules"|"workflows"|"schedules", "name": "string", "content": "string"}}) -> Manage your own logic and rules by writing to the knowledge base!
     * IF the user asks to save a rule, skill, workflow, or schedule task, YOU MUST use the `knowledge` tool to `write` it!
     * CRITICAL: If the user asks to schedule a periodic repeating task (e.g. "every 5 minutes"), YOU MUST set `domain="schedules"`. DO NOT use `workflows` for periodic tasks.
     * CRITICAL: If domain='schedules', `content` MUST be a JSON string EXACTLY matching this schema: `{{\"name\": \"str\", \"interval_seconds\": num, \"description\": \"str\", \"task_prompt\": \"Exact prompt to execute per interval\", \"end_date\": \"ISO8601 string or null for infinite\"}}`
   - "telegram": (action: "send_message", args: {{"message": "string"}}) -> Send a proactive push notification to the user's mobile Telegram. Use this immediately if you discover pending tasks or important alerts during a heartbeat/background loop.
3. To use a tool, output ONLY the following JSON block format (you may use multiple blocks):
```json
{{
  "tool": "search",
  "action": "query",
  "args": {{ "query": "your search query here" }}
}}
```
4. ONLY output JSON blocks when you need more information.
   - If you do not need tools (e.g., simple greetings, casual chat), simply reply naturally to the user and DO NOT output JSON and DO NOT output "DONE".
   - If you have finished gathering all necessary information via tools from previous steps, reply ONLY with the exact single word "DONE".

[LONG TERM MEMORY (BRAIN)]
{brain}

[REGISTERED SCHEDULES]
{schedules}
"#,
            sys = system_prompt,
            skills = skills_rules,
            current_time = current_time,
            brain = brain_files_md,
            schedules = schedule_files_md
        );

        let base_planner = planner_prompt_opt.unwrap_or(&fallback_planner_prompt);
        let mut planner_prompt = base_planner
            .replace("{SYSTEM_PROMPT}", system_prompt)
            .replace("{CURRENT_TIME}", &current_time)
            .replace("{BRAIN_FILES}", &brain_files_md)
            .replace("{SCHEDULES}", &schedule_files_md)
            .replace("{SKILLS_RULES}", &skills_rules);
        
        if !planner_prompt.contains("[AVAILABLE CUSTOM SKILLS & RULES]") {
             planner_prompt.push_str(&format!("\n\n{}", skills_rules));
        }

        let mut history = vec![ChatMessage {
            role: "system".to_string(),
            content: planner_prompt,
            images_base64: None,
        }];
        history.extend(user_messages.clone());

        let mut loop_count = 0;

        // Local state for failover routing caching during this session
        let mut active_planner_id = self.routing.planner_id.clone();
        let mut active_critic_id = self.routing.critic_id.clone();
        let active_writer_id = self.routing.writer_id.clone();
        let _active_worker_id = self.routing.worker_id.clone();

        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                let _ = log_tx
                    .send("[안내] 사용자에 의해 에이전트 실행이 중지되었습니다.".to_string())
                    .await;
                return Ok(("[사용자 중지됨]".to_string(), history));
            }

            loop_count += 1;
            if loop_count > max_loops {
                let _ = log_tx
                    .send(format!(
                        "[경고] 최대 루프 제한({}회)에 도달했습니다.",
                        max_loops
                    ))
                    .await;
                break;
            }

            // At step 1, if there are pending scheduled tasks AND this was spawned as a background heartbeat (empty user messages)
            if loop_count == 1 && !pending_tasks.is_empty() && is_background_run {
                let mut tasks_md = String::from("[SYSTEM: PENDING SCHEDULED TASKS DETECTED]\nThe following scheduled tasks must be executed immediately because their scheduled time has arrived:\n");
                for (path, sched) in pending_tasks.drain(..) {
                    tasks_md.push_str(&format!(
                        "- Task Name: {}\n  Instruction: {}\n",
                        sched.name, sched.task_prompt
                    ));
                    // Update last run time so we don't run it again next cycle
                    crate::agent::scheduler::Scheduler::new(self.db.clone())
                        .update_last_run(&path, sched.clone());
                }
                tasks_md.push_str("\nPLANNER, please execute the required tool calls to satisfy these scheduled tasks.");

                let _ = log_tx
                    .send(
                        "[안내] 백그라운드 예약된 작업(Schedule) 실행 지시문을 주입했습니다."
                            .to_string(),
                    )
                    .await;
                history.push(ChatMessage {
                    role: "user".to_string(),
                    content: tasks_md,
                    images_base64: None,
                });
            }

            let _ = log_tx
                .send(format!(
                    "[Step {}/Planner] 도구 실행 계획 수립 중...",
                    loop_count
                ))
                .await;

            let mut planner_res_result = self
                .get_llm_client_by_id(&active_planner_id)
                .chat(&history, 0.7)
                .await;
            if planner_res_result.is_err() {
                let err = planner_res_result.as_ref().unwrap_err();
                let _ = log_tx.send(format!("[경고] 주력 LLM(Planner) 통신 실패: {}. 예비 모델들로 폴백(Fallback) 라우팅을 시도합니다...", err)).await;

                for fallback_ep in self.get_fallback_endpoints(&active_planner_id) {
                    planner_res_result = crate::agent::llm_client::LLMClient::new(
                        fallback_ep.api_url.clone(),
                        fallback_ep.model.clone(),
                        fallback_ep.api_key.clone(),
                    )
                    .chat(&history, 0.7)
                    .await;
                    if planner_res_result.is_ok() {
                        active_planner_id = fallback_ep.id.clone();
                        let _ = log_tx.send(format!("[안내] 예비 LLM 통신 성공. 현재 세션 동안 {} 모델로 대체 작동합니다.", fallback_ep.name)).await;
                        break;
                    }
                }
            }

            let planner_res = match planner_res_result {
                Ok(r) => r,
                Err(e) => {
                    return Err(format!(
                        "Planner LLM 최종 통신 불가 (주력 및 예비 모델 모두 실패): {}",
                        e
                    ))
                }
            };

            let json_blocks = extract_json_blocks(&planner_res.content);

            if json_blocks.is_empty() {
                let text_res = planner_res.content.trim();

                // If it doesn't say "DONE", it means planner replied directly
                if !text_res.to_uppercase().contains("DONE") {
                    let _ = log_tx
                        .send(
                            "[Planner] 도구 검색이 필요 없는 단순 대화로 판단하여 즉시 답변합니다."
                                .to_string(),
                        )
                        .await;
                    history.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: text_res.to_string(),
                        images_base64: None,
                    });
                    self.save_transcript(session_id.clone(), &history);
                    return Ok((Self::sanitize_output(text_res), history));
                }

                let _ = log_tx
                    .send(
                        "[Planner] 도구 수집이 완료되었습니다. 전문 작성(Writer)으로 넘어갑니다."
                            .to_string(),
                    )
                    .await;
                // Run Writer Agent
                let fallback_writer_prompt_string = format!("You are a WRITER Agent. Synthesize the findings and user conversations into a highly professional response entirely in natural {}. If the user merely sent a standard greeting or simple chatter without requiring tool lookups, just reply naturally and conversationally.", lang_name);
                let writer_system = writer_prompt_opt.unwrap_or(&fallback_writer_prompt_string);

                let writer_prompt = ChatMessage {
                    role: "system".to_string(),
                    content: writer_system.to_string(),
                    images_base64: None,
                };
                let mut writer_history = history.clone();
                writer_history.insert(0, writer_prompt);
                writer_history.push(ChatMessage {
                    role: "user".to_string(),
                    content: format!(
                        "WRITER Agent, please write the final response based on the above tool exploration log (if any) and the conversation history. If it is a simple conversation without tools, just answer naturally in {}.",
                        lang_name
                    ), images_base64: None });

                let _ = log_tx
                    .send("[Writer] 최종 답변 작성 중...".to_string())
                    .await;
                let mut writer_res_result = self
                    .get_llm_client_by_id(&active_writer_id)
                    .chat(&writer_history, 0.7)
                    .await;
                if writer_res_result.is_err() {
                    let _ = log_tx.send("[경고] Writer LLM 에러, 예비 모델들로 폴백(Fallback) 라우팅을 시도합니다...".to_string()).await;
                    for fallback_ep in self.get_fallback_endpoints(&active_writer_id) {
                        writer_res_result = crate::agent::llm_client::LLMClient::new(
                            fallback_ep.api_url.clone(),
                            fallback_ep.model.clone(),
                            fallback_ep.api_key.clone(),
                        )
                        .chat(&writer_history, 0.7)
                        .await;
                        if writer_res_result.is_ok() {
                            let _ = log_tx.send(format!("[안내] 예비 LLM 통신 성공. 현재 세션 동안 {} 모델로 대체 작동합니다.", fallback_ep.name)).await;
                            break;
                        }
                    }
                }
                let writer_res = writer_res_result.unwrap_or_else(|_| LLMResult {
                    content: planner_res.content.clone(),
                    raw: serde_json::Value::Null,
                });

                history.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: writer_res.content.clone(),
                    images_base64: None,
                });
                self.save_transcript(session_id.clone(), &history);
                return Ok((Self::sanitize_output(&writer_res.content), history));
            }

            // Execute Tools
            history.push(ChatMessage {
                role: "assistant".to_string(),
                content: planner_res.content.clone(),
                images_base64: None,
            });

            let _ = log_tx
                .send(format!(
                    "[Planner] {}개의 도구 실행을 요청했습니다.",
                    json_blocks.len()
                ))
                .await;
            let results = self
                .multi_agent
                .execute_tools(
                    json_blocks,
                    Some(self.get_llm_for_planner()),
                    registry_prompt_opt.map(|s| s.to_string()),
                )
                .await;

            let mut result_summary_md = String::from("### Tool Execution Results\n");
            for r in results {
                let status = if r.ok { "성공" } else { "실패" };
                let _ = log_tx
                    .send(format!(" -> [{}.{}] {}", r.tool_name, r.action, status))
                    .await;

                if r.ok
                    && (r.action == "write" || r.action == "write_artifact")
                    && (r.tool_name == "brain" || r.tool_name == "knowledge")
                {
                    let _ = log_tx
                        .send(format!("[DB 저장 이벤트] 💾 {} 도구를 통해 데이터베이스에 저장이 완료되었습니다.", r.tool_name))
                        .await;
                }

                result_summary_md.push_str(&format!(
                    "**Tool**: {}.{}\n**Status**: {}\n**Output**:\n{}\n\n---\n",
                    r.tool_name, r.action, status, r.output
                ));
            }

            // Critic Agent Phase
            let _ = log_tx
                .send("[Critic] 수집된 데이터의 유효성을 검증합니다...".to_string())
                .await;

            let query = user_messages
                .iter()
                .rfind(|m| m.role == "user")
                .map(|m| m.content.clone())
                .unwrap_or_else(|| "".to_string());
            let fallback_critic_prompt = "You are a strict CRITIC Agent. Read the user's main query: '{QUERY}'.\nNow read these tool execution results:\n{RESULT_SUMMARY}\n\nIf the results contain the exact facts needed to fully answer the query, reply exactly: 'STATUS: PASS'. If the results are outdated, irrelevant, or missing info, reply 'STATUS: FAIL' followed by strict feedback instructing the Planner on what to search differently (e.g. search specific year, different keywords). Reply in English.";
            let base_critic = critic_prompt_opt.unwrap_or(fallback_critic_prompt);
            let critic_prompt = base_critic
                .replace("{QUERY}", &query)
                .replace("{RESULT_SUMMARY}", &result_summary_md);

            let mut critic_res_result = self
                .get_llm_for_critic()
                .chat(
                    &[ChatMessage {
                        role: "user".to_string(),
                        content: critic_prompt.clone(),
                        images_base64: None,
                    }],
                    0.2,
                )
                .await;

            if critic_res_result.is_err() {
                let _ = log_tx.send("[경고] Critic LLM 에러, 예비 모델들로 폴백(Fallback) 라우팅을 시도합니다...".to_string()).await;
                for fallback_ep in self.get_fallback_endpoints(&active_critic_id) {
                    critic_res_result = crate::agent::llm_client::LLMClient::new(
                        fallback_ep.api_url.clone(),
                        fallback_ep.model.clone(),
                        fallback_ep.api_key.clone(),
                    )
                    .chat(
                        &[ChatMessage {
                            role: "user".to_string(),
                            content: critic_prompt.clone(),
                            images_base64: None,
                        }],
                        0.2,
                    )
                    .await;
                    if critic_res_result.is_ok() {
                        active_critic_id = fallback_ep.id.clone();
                        let _ = log_tx.send(format!("[안내] 예비 LLM 통신 성공. 현재 세션 동안 {} 모델로 대체 작동합니다.", fallback_ep.name)).await;
                        break;
                    }
                }
            }

            let critic_res = critic_res_result.unwrap_or_else(|_| LLMResult {
                content: "STATUS: PASS".to_string(),
                raw: serde_json::Value::Null,
            });

            if critic_res.content.contains("STATUS: PASS") {
                let _ = log_tx
                    .send(
                        "[Critic] 검증 완료. 데이터가 충분합니다. Writer에게 넘깁니다.".to_string(),
                    )
                    .await;
                history.push(ChatMessage {
                    role: "user".to_string(),
                    content: result_summary_md,
                    images_base64: None,
                });

                // Run Writer
                let fallback_writer_prompt = format!("You are a WRITER Agent. Synthesize the findings and user conversations into a highly professional response entirely in natural {}.", lang_name);
                let writer_prompt = ChatMessage {
                    role: "system".to_string(),
                    content: writer_prompt_opt
                        .unwrap_or(&fallback_writer_prompt)
                        .to_string(),
                    images_base64: None,
                };
                let mut writer_history = history.clone();
                writer_history.insert(0, writer_prompt);
                writer_history.push(ChatMessage { role: "user".to_string(), content: format!("The user's request has been fulfilled or the data is ready. Please provide the final response to the user in {}. DO NOT unnecessarily repeat conversational history. Focus ONLY on what was just done or discovered.", lang_name), images_base64: None });

                let _ = log_tx
                    .send("[Writer] 최종 답변 정리 중...".to_string())
                    .await;
                let mut writer_res_result = self
                    .get_llm_client_by_id(&active_writer_id)
                    .chat(&writer_history, 0.7)
                    .await;
                if writer_res_result.is_err() {
                    let _ = log_tx.send("[경고] Writer LLM 에러, 예비 모델들로 폴백(Fallback) 라우팅을 시도합니다...".to_string()).await;
                    for fallback_ep in self.get_fallback_endpoints(&active_writer_id) {
                        writer_res_result = crate::agent::llm_client::LLMClient::new(
                            fallback_ep.api_url.clone(),
                            fallback_ep.model.clone(),
                            fallback_ep.api_key.clone(),
                        )
                        .chat(&writer_history, 0.7)
                        .await;
                        if writer_res_result.is_ok() {
                            let _ = log_tx.send(format!("[안내] 예비 LLM 통신 성공. 현재 세션 동안 {} 모델로 대체 작동합니다.", fallback_ep.name)).await;
                            break;
                        }
                    }
                }
                let writer_res = writer_res_result.unwrap_or_else(|_| LLMResult {
                    content: "Writer 에러".to_string(),
                    raw: serde_json::Value::Null,
                });

                history.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: writer_res.content.clone(),
                    images_base64: None,
                });
                self.save_transcript(session_id.clone(), &history);
                return Ok((Self::sanitize_output(&writer_res.content), history));
            } else {
                let fb = critic_res
                    .content
                    .replace("STATUS: FAIL", "")
                    .trim()
                    .to_string();
                let _ = log_tx.send(format!("[Critic] 검증 실패: {}", fb)).await;
                history.push(ChatMessage {
                    role: "user".to_string(),
                    content: format!("SYSTEM OBSERVATION RESULTS:\n{}\n\n[Critic Agent Feedback]\nYour last tool executed, but the task is not yet complete. Critic feedback: {}\nPlease use tools again with different strategies to fulfill the task.", result_summary_md, fb),
                    images_base64: None,
                });
            }
        }

        self.save_transcript(session_id.clone(), &history);
        Ok((
            "에이전트 파이프라인이 결론을 짓지 못하고 종료되었습니다.".to_string(),
            history,
        ))
    }
}
