use crate::agent::llm_client::ChatMessage;
use crate::agent::parser::extract_json_blocks;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

impl super::Orchestrator {
    #[allow(clippy::too_many_arguments)]
    pub async fn run_loop(
        &self,
        session_id: Option<String>,
        system_prompt: &str,
        planner_prompt_opt: Option<&str>,
        critic_prompt_opt: Option<&str>,
        writer_prompt_opt: Option<&str>,
        user_messages: Vec<ChatMessage>,
        language: &str,
        max_loops: u32,
        use_multi_agent_workflow: bool,
        registry_prompt_opt: Option<&str>,
        log_tx: mpsc::Sender<String>,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<(String, Vec<ChatMessage>), String> {
        let is_background_run = user_messages.is_empty();

        if use_multi_agent_workflow {
            return self
                .run_multi_agent_pipeline(
                    session_id.clone(),
                    system_prompt,
                    planner_prompt_opt,
                    critic_prompt_opt,
                    writer_prompt_opt,
                    user_messages,
                    language,
                    max_loops,
                    registry_prompt_opt,
                    log_tx,
                    cancel_flag,
                )
                .await;
        }

        let (lang_name, lang_native) = match language {
            "en" => ("ENGLISH".to_string(), "English".to_string()),
            "ko" => ("KOREAN".to_string(), "한국어".to_string()),
            _ => (language.to_uppercase(), language.to_string()),
        };

        let (current_time, brain_files_md, schedule_files_md, mut pending_tasks, skills_rules) =
            self.build_context();

        let force_tool_prompt = format!(
            r#"[USER DEFINED SYSTEM PROMPT]
{sys}

{skills}

[SYSTEM TIME ANCHOR]
Current System Time: {current_time} (You live in this exact present moment. Use this to accurately calculate "recent", "upcoming", "past", and assess dates from search results).

[CRITICAL TOOL INSTRUCTIONS & WORKFLOW RULES]
1. IMPORTANT: ALWAYS check the [LONG TERM MEMORY (BRAIN)] file list at the bottom of this prompt FIRST. If a file seems relevant to the user's query, you MUST use the `brain` tool (with action "read") to read it before attempting an external external search!
2. You have the following tools:
   - "search": (action: "query", args: {{"query": "string", "time_range": "d|w|m|y"}}) -> Google Search for finding external information.
   - "crawl4ai": (action: "scrape", args: {{"url": "string"}}) -> Read webpage content. **CRITICAL: If the user provides a direct URL (like a github link, article, etc.), ALWAYS use `crawl4ai` FIRST to scrape the exact page instead of searching!**
   - "brain": (action: "list", args: {{}}) -> List all your long-term memory artifact files.
   - "brain": (action: "read", args: {{"name": "filename.md"}}) -> Read the precise content of a specific memory artifact file.
   - "brain": (action: "write_artifact", args: {{"name": "filename.md", "content": "markdown string"}}) -> Create or overwrite a semantic long-term memory document. CRITICAL: To prevent memory splintering, if a file for a similar topic already exists in [LONG TERM MEMORY], you MUST `read` it first, merge the new data, and overwite it using the same filename! DO NOT create `topic_2.md` or similar fragmented files.
   - "terminal": (action: "execute", args: {{"command": "string"}}) -> Execute System Shell commands. *WARNING: You are sandboxed to the `./Work/` folder. Use this to run scripts, curl data, schedule tasks, etc.*
   - "knowledge": (action: "read"|"write"|"list"|"delete", args: {{"domain": "skills"|"rules"|"workflows"|"schedules", "name": "string", "content": "string"}}) -> Manage your own logic and rules by writing to the knowledge base! ANTI-SPLINTERING: Always read and overwrite existing items instead of creating _v2.
     *CRITICAL: If the user asks to schedule a periodic repeating task (e.g. "every 5 minutes"), YOU MUST set `domain="schedules"`. DO NOT use `workflows` for periodic tasks.*
     *CRITICAL: If domain='schedules', `content` MUST be a JSON string EXACTLY matching this schema: `{{\"name\": \"str\", \"interval_seconds\": num (optional), \"cron_expression\": \"str (optional)\", \"description\": \"str\", \"task_prompt\": \"Exact prompt to execute\", \"end_date\": \"ISO8601 string or null\"}}`*

[REGISTERED SCHEDULES]
{schedules}

2. To use a tool, output ONLY the following JSON block format (you may use multiple blocks):
```json
{{
  "tool": "search",
  "action": "query",
  "args": {{ "query": "your search query here" }}
}}
```

3. **[LANGUAGE POLICY]**
   - THINKING & TOOL EXECUTION: During intermediate steps where you are gathering data and planning, you MUST think, output tool JSON, and reason in **ENGLISH** to maximize token efficiency and cognitive accuracy.
   - FINAL OUTPUT: ONLY when you have resolved the user's task and do not need any more tools, provide your FINAL direct response to the user translated entirely into natural **{lang_name}** ({lang_native}).

[LONG TERM MEMORY (BRAIN)]
{brain}
"#,
            sys = system_prompt,
            skills = skills_rules,
            current_time = current_time,
            schedules = schedule_files_md,
            lang_name = lang_name,
            lang_native = lang_native,
            brain = brain_files_md
        );

        // Construct conversation history with System Prompt at the head
        let mut history = vec![ChatMessage {
            role: "system".to_string(),
            content: force_tool_prompt.clone(),
            images_base64: None,
        }];
        history.extend(user_messages);

        let mut loop_count = 0;
        let mut active_worker_id = self.routing.worker_id.clone();

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

            // At step 1, if there are pending scheduled tasks, we auto-inject their prompts
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
                tasks_md.push_str("\nAGENT, please execute the required tool calls to satisfy these scheduled tasks.");

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
                .send(format!("[Step {}/LLM] AI 응답 대기 중...", loop_count))
                .await;

            let mut response_res = self
                .get_llm_client_by_id(&active_worker_id)
                .chat(&history, 0.7)
                .await;
            if response_res.is_err() {
                let primary_err = response_res.as_ref().unwrap_err(); /* get the error clone or format string */
                let _ = log_tx.send(format!("[경고] 주력 LLM 통신 실패: {}. 예비 모델들로 폴백(Fallback) 라우팅을 시도합니다...", primary_err)).await;

                for fallback_ep in self.get_fallback_endpoints(&active_worker_id) {
                    response_res = crate::agent::llm_client::LLMClient::new(
                        fallback_ep.api_url.clone(),
                        fallback_ep.model.clone(),
                        fallback_ep.api_key.clone(),
                    )
                    .chat(&history, 0.7)
                    .await;
                    if response_res.is_ok() {
                        active_worker_id = fallback_ep.id.clone();
                        let _ = log_tx.send(format!("[안내] 예비 LLM 통신 성공. 현재 세션 동안 {} 모델로 대체 작동합니다.", fallback_ep.name)).await;
                        break;
                    }
                }
            }

            let response = match response_res {
                Ok(res) => res,
                Err(e) => {
                    let err_msg = format!("최종 LLM 통신 불가 (활성화된 모든 모델 실패): {}", e);
                    let _ = log_tx.send(format!("[치명적 오류] {}", err_msg)).await;
                    return Err(err_msg);
                }
            };

            let ai_text = response.content;

            // Log a snippet of AI reasoning safely handling Unicode boundaries
            let mut chars = ai_text.chars();
            let snippet: String = chars.by_ref().take(100).collect();
            let preview = if chars.next().is_some() {
                format!("{}...", snippet)
            } else {
                snippet
            };
            let preview_clean = preview.replace("\n", " ");
            let _ = log_tx.send(format!("[AI 추론] {}", preview_clean)).await;

            // Clean ai_text to prevent injecting hallucinated tool_responses into history
            let mut clean_ai_text = ai_text.clone();
            if let Some(cutoff) = clean_ai_text.find("<|tool_response>") {
                clean_ai_text.truncate(cutoff);
            }

            // Append assistant response to history
            history.push(ChatMessage {
                role: "assistant".to_string(),
                content: clean_ai_text,
                images_base64: None,
            });

            // Parse any Tool invocations
            let tool_calls = extract_json_blocks(&ai_text);

            if tool_calls.is_empty() {
                // Goal Achieved or Natural Chat
                let _ = log_tx
                    .send("[완료] 최종 라운드가 성공적으로 종료되었습니다.".to_string())
                    .await;
                self.save_transcript(session_id, &history);
                return Ok((Self::sanitize_output(&ai_text), history));
            }

            let _ = log_tx
                .send(format!(
                    "[플래너] {}개의 툴 작업을 식별하고 병렬 트리거합니다.",
                    tool_calls.len()
                ))
                .await;

            // Execute Tools
            let results = self
                .multi_agent
                .execute_tools(
                    tool_calls,
                    Some(self.get_llm_client_by_id(&active_worker_id)),
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

            let _ = log_tx
                .send("[관찰] 툴 데이터를 확보하여 LLM에 재주입합니다.".to_string())
                .await;

            // Inject the result as user system feedback
            history.push(ChatMessage {
                role: "user".to_string(),
                content: format!("SYSTEM OBSERVATION RESULTS:\n{}\n\nINSTRUCTION: If these results fulfill the user's most recent request completely, provide your FINAL direct response to the user. Explain ONLY what was just accomplished. Do NOT artificially repeat conversational history. If you need more data or intermediate steps, respond ONLY with JSON tool blocks.", result_summary_md),
                images_base64: None,
            });
        }

        // Exhausted max loops, return latest assistant response if any
        if let Some(ChatMessage {
            role: r,
            content: c,
            images_base64: _images,
        }) = history.last()
        {
            if r == "assistant" {
                self.save_transcript(session_id, &history);
                return Ok((Self::sanitize_output(c), history));
            }
        }

        self.save_transcript(session_id, &history);
        Ok((
            "에이전트가 결론을 짓지 못하고 종료되었습니다.".to_string(),
            history,
        ))
    }
}
