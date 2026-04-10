use crate::agent::llm_client::{ChatMessage, LLMClient, LLMResult};
use crate::agent::multi_agent::MultiAgent;
use crate::agent::parser::extract_json_blocks;
use crate::agent::scheduler::Scheduler;
use chrono::Local;
use dbx_core::Database;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct Orchestrator {
    llm: LLMClient,
    multi_agent: Arc<MultiAgent>,
    base_dir: std::path::PathBuf,
    db: Arc<Database>,
}

impl Orchestrator {
    pub fn new(
        llm: LLMClient,
        multi_agent: Arc<MultiAgent>,
        base_dir: std::path::PathBuf,
        db: Arc<Database>,
    ) -> Self {
        Self {
            llm,
            multi_agent,
            base_dir,
            db,
        }
    }

    pub async fn run_loop(
        &self,
        system_prompt: &str,
        planner_prompt_opt: Option<&str>,
        critic_prompt_opt: Option<&str>,
        writer_prompt_opt: Option<&str>,
        user_messages: Vec<ChatMessage>,
        language: &str,
        max_loops: u32,
        use_multi_agent_workflow: bool,
        log_tx: mpsc::Sender<String>,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<(String, Vec<ChatMessage>), String> {
        if use_multi_agent_workflow {
            return self
                .run_multi_agent_pipeline(
                    system_prompt,
                    planner_prompt_opt,
                    critic_prompt_opt,
                    writer_prompt_opt,
                    user_messages,
                    language,
                    max_loops,
                    log_tx,
                    cancel_flag,
                )
                .await;
        }

        let (lang_name, lang_native) = match language {
            "en" => ("ENGLISH", "English"),
            "ja" => ("JAPANESE", "日本語"),
            "zh" => ("CHINESE", "中文"),
            _ => ("KOREAN", "한국어"),
        };

        let current_time = Local::now().format("%Y-%m-%d %A %H:%M:%S").to_string();

        let brain_tool = crate::tools::brain::BrainTool::new(self.db.clone());
        let brain_files_md = brain_tool
            .list_artifacts()
            .unwrap_or_else(|_| "No brain artifacts stored yet.".to_string());

        let scheduler = Scheduler::new(self.db.clone());
        let (mut pending_tasks, schedules_summary, status_summary) = scheduler.evaluate_schedules();
        let schedule_files_md = format!(
            "{}\n\n[SCHEDULE STATUS]\n{}",
            schedules_summary, status_summary
        );

        let force_tool_prompt = format!(
            r#"[USER DEFINED SYSTEM PROMPT]
{}

[SYSTEM TIME ANCHOR]
Current System Time: {} (You live in this exact present moment. Use this to accurately calculate "recent", "upcoming", "past", and assess dates from search results).

[CRITICAL TOOL INSTRUCTIONS & WORKFLOW RULES]
1. IMPORTANT: ALWAYS check the [LONG TERM MEMORY (BRAIN)] file list at the bottom of this prompt FIRST. If a file seems relevant to the user's query, you MUST use the `brain` tool (with action "read") to read it before attempting an external external search!
2. You have the following tools:
   - "search": (action: "query", args: {{"query": "string", "time_range": "d|w|m|y"}}) -> Google Search.
   - "crawl4ai": (action: "scrape", args: {{"url": "string"}}) -> Read webpage content.
   - "brain": (action: "list", args: {{}}) -> List all your long-term memory artifact files.
   - "brain": (action: "read", args: {{"name": "filename.md"}}) -> Read the precise content of a specific memory artifact file.
   - "brain": (action: "write_artifact", args: {{"name": "filename.md", "content": "markdown string"}}) -> Create or overwrite a semantic long-term memory document. USE THIS to persist specific user data, personas, project details, etc.
   - "terminal": (action: "execute", args: {{"command": "string"}}) -> Execute Powershell commands. *WARNING: You are sandboxed to the `./Work/` folder. Use this to run scripts, curl data, schedule tasks, etc.*
   - "knowledge": (action: "read"|"write"|"list"|"delete", args: {{"domain": "skills"|"rules"|"workflows"|"schedules", "name": "string", "content": "string"}}) -> Manage your own logic and rules by writing to the knowledge base!
     *CRITICAL: If the user asks to schedule a periodic repeating task (e.g. "every 5 minutes"), YOU MUST set `domain="schedules"`. DO NOT use `workflows` for periodic tasks.*
     *CRITICAL: If domain='schedules', `content` MUST be a JSON string EXACTLY matching this schema: `{{\"name\": \"str\", \"interval_seconds\": num, \"description\": \"str\", \"task_prompt\": \"Exact prompt to execute per interval\", \"end_date\": \"ISO8601 string or null for infinite\"}}`*

[REGISTERED SCHEDULES]
{}

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
   - FINAL OUTPUT: ONLY when you have resolved the user's task and do not need any more tools, provide your FINAL direct response to the user translated entirely into natural **{}** ({}).

[LONG TERM MEMORY (BRAIN)]
{}
"#,
            system_prompt, current_time, schedule_files_md, lang_name, lang_native, brain_files_md
        );

        // Construct conversation history with System Prompt at the head
        let mut history = vec![ChatMessage {
            role: "system".to_string(),
            content: force_tool_prompt.clone(),
            images_base64: None,
        }];
        history.extend(user_messages);

        let mut loop_count = 0;

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
            if loop_count == 1 && !pending_tasks.is_empty() {
                let mut tasks_md = String::from("[SYSTEM: PENDING SCHEDULED TASKS DETECTED]\nThe following scheduled tasks must be executed immediately because their scheduled time has arrived:\n");
                for (path, sched) in pending_tasks.drain(..) {
                    tasks_md.push_str(&format!(
                        "- Task Name: {}\n  Instruction: {}\n",
                        sched.name, sched.task_prompt
                    ));
                    // Update last run time so we don't run it again next cycle
                    scheduler.update_last_run(&path, sched.clone());
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

            let response = match self.llm.chat(&history, 0.7).await {
                Ok(res) => res,
                Err(e) => {
                    let err_msg = format!("LLM 통신 에러: {}", e);
                    let _ = log_tx.send(format!("[오류] {}", err_msg)).await;
                    return Err(err_msg);
                }
            };

            let ai_text = response.content;

            // Log a snippet of AI reasoning safely handling Unicode boundaries
            let chars: Vec<char> = ai_text.chars().collect();
            let preview = if chars.len() > 100 {
                let snippet: String = chars[..100].iter().collect();
                format!("{}...", snippet)
            } else {
                ai_text.clone()
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
                self.save_transcript(&history);
                return Ok((Self::sanitize_output(&ai_text), history));
            }

            let _ = log_tx
                .send(format!(
                    "[플래너] {}개의 툴 작업을 식별하고 병렬 트리거합니다.",
                    tool_calls.len()
                ))
                .await;

            // Execute Tools
            let results = self.multi_agent.execute_tools(tool_calls).await;

            let mut result_summary_md = String::from("### Tool Execution Results\n");
            for r in results {
                let status = if r.ok { "성공" } else { "실패" };
                let _ = log_tx
                    .send(format!(" -> [{}.{}] {}", r.tool_name, r.action, status))
                    .await;

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
                content: format!("여기는 시스템(또는 관찰 도구)의 결과입니다. 이 정보를 바탕으로 다음 단계 추론을 계속하거나 사용자에게 답변해 주시기 바랍니다.\n\n{}", result_summary_md),
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
                self.save_transcript(&history);
                return Ok((Self::sanitize_output(c), history));
            }
        }

        self.save_transcript(&history);
        Ok((
            "에이전트가 결론을 짓지 못하고 종료되었습니다.".to_string(),
            history,
        ))
    }

    /// Saves the complete session transcript into the logs/ directory
    fn save_transcript(&self, history: &[ChatMessage]) {
        let logs_dir = self.base_dir.join("logs");
        if !logs_dir.exists() {
            let _ = fs::create_dir_all(&logs_dir);
        }
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let filepath = logs_dir.join(format!("session_{}.md", timestamp));

        let mut out = String::new();
        out.push_str(&format!("# PumAgent Session Log - {}\n\n", timestamp));
        for msg in history {
            out.push_str(&format!("### Role: {}\n", msg.role.to_uppercase()));
            out.push_str(&format!("{}\n\n---\n", msg.content));
        }

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&filepath)
        {
            let _ = file.write_all(out.as_bytes());
        }
    }

    /// Strips internal Gemma chain-of-thought tags like `<channel|>` from final user output
    fn sanitize_output(text: &str) -> String {
        let mut clean = text.to_string();
        if let Some(pos) = clean.rfind("<channel|>") {
            clean = clean[pos + "<channel|>".len()..].trim().to_string();
        }
        // Also strip self correction checks sometimes emitted by models
        if let Some(pos) = clean.find("*(Self-Correction Check:") {
            clean = clean[..pos].trim().to_string();
        }
        if let Some(pos) = clean.find("*(Self-Correction Check") {
            clean = clean[..pos].trim().to_string();
        }
        clean
    }

    pub async fn run_multi_agent_pipeline(
        &self,
        system_prompt: &str,
        planner_prompt_opt: Option<&str>,
        critic_prompt_opt: Option<&str>,
        writer_prompt_opt: Option<&str>,
        user_messages: Vec<ChatMessage>,
        language: &str,
        max_loops: u32,
        log_tx: mpsc::Sender<String>,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<(String, Vec<ChatMessage>), String> {
        let current_time = Local::now().format("%Y-%m-%d %A %H:%M:%S").to_string();

        let (lang_name, lang_native) = match language {
            "en" => ("ENGLISH", "English"),
            "ja" => ("JAPANESE", "日本語"),
            "zh" => ("CHINESE", "中文"),
            _ => ("KOREAN", "한국어"),
        };

        let brain_tool = crate::tools::brain::BrainTool::new(self.db.clone());
        let brain_files_md = brain_tool
            .list_artifacts()
            .unwrap_or_else(|_| "No brain artifacts stored yet.".to_string());

        // Use the new Scheduler to evaluate schedules and inject execution status
        let scheduler = Scheduler::new(self.db.clone());
        let (mut pending_tasks, schedules_summary, status_summary) = scheduler.evaluate_schedules();
        let schedule_files_md = format!(
            "{}\n\n[SCHEDULE STATUS]\n{}",
            schedules_summary, status_summary
        );

        let fallback_planner_prompt = r#"[PLANNER SYSTEM PROMPT]
{SYSTEM_PROMPT}

[SYSTEM TIME ANCHOR]
Current System Time: {CURRENT_TIME} (You live in this exact present moment. Use this to accurately calculate "recent", "upcoming", "past", and assess dates from search results).

[CRITICAL TOOL INSTRUCTIONS & WORKFLOW RULES]
1. You are the PLANNER and RESEARCHER. Your job is to gather data using tools.
2. You have the following tools:
   - "search": (action: "query", args: {"query": "string", "time_range": "d|w|m|y"}) -> Google Search.
   - "crawl4ai": (action: "scrape", args: {"url": "string"}) -> Read webpage content.
   - "brain": (action: "list", args: {}) -> List all your long-term memory artifact files.
   - "brain": (action: "read", args: {"name": "filename.md"}) -> Read the precise content of a specific memory artifact file.
   - "brain": (action: "write_artifact", args: {"name": "filename.md", "content": "markdown string"}) -> Create or overwrite a semantic long-term memory document.
   - "terminal": (action: "execute", args: {"command": "string"}) -> Execute Powershell commands. *WARNING: You are sandboxed to the `./Work/` folder.*
   - "knowledge": (action: "read"|"write"|"list"|"delete", args: {"domain": "skills"|"rules"|"workflows"|"schedules", "name": "string", "content": "string"}) -> Manage your own logic and rules by writing to the knowledge base!
     * IF the user asks to save a rule, skill, workflow, or schedule task, YOU MUST use the `knowledge` tool to `write` it!
     * CRITICAL: If the user asks to schedule a periodic repeating task (e.g. "every 5 minutes"), YOU MUST set `domain="schedules"`. DO NOT use `workflows` for periodic tasks.
     * CRITICAL: If domain='schedules', `content` MUST be a JSON string EXACTLY matching this schema: `{"name": "str", "interval_seconds": num, "description": "str", "task_prompt": "Exact prompt to execute per interval", "end_date": "ISO8601 string or null for infinite"}`
   - "telegram": (action: "send_message", args: {"message": "string"}) -> Send a proactive push notification to the user's mobile Telegram. Use this immediately if you discover pending tasks or important alerts during a heartbeat/background loop.
3. To use a tool, output ONLY the following JSON block format (you may use multiple blocks):
```json
{
  "tool": "search",
  "action": "query",
  "args": { "query": "your search query here" }
}
```
4. ONLY output JSON blocks when you need more information.
   - If you do not need tools (e.g., simple greetings, casual chat), simply reply naturally to the user and DO NOT output JSON and DO NOT output "DONE".
   - If you have finished gathering all necessary information via tools from previous steps, reply ONLY with the exact single word "DONE".

[LONG TERM MEMORY (BRAIN)]
{BRAIN_FILES}

[REGISTERED SCHEDULES]
{SCHEDULES}
"#;

        let base_planner = planner_prompt_opt.unwrap_or(fallback_planner_prompt);
        let planner_prompt = base_planner
            .replace("{SYSTEM_PROMPT}", system_prompt)
            .replace("{CURRENT_TIME}", &current_time)
            .replace("{BRAIN_FILES}", &brain_files_md)
            .replace("{SCHEDULES}", &schedule_files_md);

        let mut history = vec![ChatMessage {
            role: "system".to_string(),
            content: planner_prompt,
            images_base64: None,
        }];
        history.extend(user_messages.clone());

        let mut loop_count = 0;

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
            if loop_count == 1 && !pending_tasks.is_empty() {
                let mut tasks_md = String::from("[SYSTEM: PENDING SCHEDULED TASKS DETECTED]\nThe following scheduled tasks must be executed immediately because their scheduled time has arrived:\n");
                for (path, sched) in pending_tasks.drain(..) {
                    tasks_md.push_str(&format!(
                        "- Task Name: {}\n  Instruction: {}\n",
                        sched.name, sched.task_prompt
                    ));
                    // Update last run time so we don't run it again next cycle
                    scheduler.update_last_run(&path, sched.clone());
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

            let planner_res = match self.llm.chat(&history, 0.7).await {
                Ok(r) => r,
                Err(e) => return Err(format!("Planner LLM 에러: {}", e)),
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
                    self.save_transcript(&history);
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
                let writer_res = self
                    .llm
                    .chat(&writer_history, 0.7)
                    .await
                    .unwrap_or_else(|_| LLMResult {
                        content: planner_res.content.clone(),
                        raw: serde_json::Value::Null,
                    });

                history.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: writer_res.content.clone(),
                    images_base64: None,
                });
                self.save_transcript(&history);
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
            let results = self.multi_agent.execute_tools(json_blocks).await;

            let mut result_summary_md = String::from("### Tool Execution Results\n");
            for r in results {
                let status = if r.ok { "성공" } else { "실패" };
                let _ = log_tx
                    .send(format!(" -> [{}.{}] {}", r.tool_name, r.action, status))
                    .await;
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

            let critic_res = self
                .llm
                .chat(
                    &[ChatMessage {
                        role: "user".to_string(),
                        content: critic_prompt,
                        images_base64: None,
                    }],
                    0.2,
                )
                .await
                .unwrap_or_else(|_| LLMResult {
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
                writer_history.push(ChatMessage { role: "user".to_string(), content: format!("지금까지의 정보가 완벽히 검증되었습니다. 사용자를 위해 최종 답변을 {}로 작성해주세요.", lang_native), images_base64: None });

                let _ = log_tx
                    .send("[Writer] 최종 답변 정리 중...".to_string())
                    .await;
                let writer_res = self
                    .llm
                    .chat(&writer_history, 0.7)
                    .await
                    .unwrap_or_else(|_| LLMResult {
                        content: "Writer 에러".to_string(),
                        raw: serde_json::Value::Null,
                    });

                history.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: writer_res.content.clone(),
                    images_base64: None,
                });
                self.save_transcript(&history);
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
                    content: format!("여기는 시스템의 결과입니다.\n{}\n\n[Critic Agent Feedback]\nYour last tool executed, but the data is insufficient. Critic feedback: {}\nTry different keywords or tools.", result_summary_md, fb),
                    images_base64: None,
                });
            }
        }

        self.save_transcript(&history);
        Ok((
            "에이전트 파이프라인이 결론을 짓지 못하고 종료되었습니다.".to_string(),
            history,
        ))
    }

    pub async fn run_reflector_pipeline(
        &self,
        reflector_prompt: &str,
        mut history: Vec<ChatMessage>,
        log_tx: mpsc::Sender<String>,
    ) {
        let _ = log_tx
            .send("[Reflector] 백그라운드 회고(Memory/Schedule)를 시작합니다.".to_string())
            .await;

        let system_msg = ChatMessage {
            role: "system".to_string(),
            content: reflector_prompt.to_string(),
            images_base64: None,
        };

        // Insert the system prompt at the beginning of the history so the LLM knows it is the Reflector.
        history.insert(0, system_msg);

        // Append a prompt asking it to execute tools now if needed
        history.push(ChatMessage {
            role: "user".to_string(),
            content: "You have reviewed the conversation. If you need to store anything in the brain or schedule a task, output the appropriate JSON tool blocks now. If no memory or scheduling is needed, reply exactly 'NO_MEMORY_NEEDED'.".to_string(), images_base64: None });

        let reflector_res = self
            .llm
            .chat(&history, 0.7)
            .await
            .unwrap_or_else(|_| LLMResult {
                content: "NO_MEMORY_NEEDED".to_string(),
                raw: serde_json::Value::Null,
            });

        let ai_text = reflector_res.content.clone();
        if ai_text.trim() == "NO_MEMORY_NEEDED" {
            let _ = log_tx
                .send("[Reflector] 추가 기록 항목이 없습니다. 종료합니다.".to_string())
                .await;
            return;
        }

        let tool_calls = extract_json_blocks(&ai_text);
        if tool_calls.is_empty() {
            let _ = log_tx
                .send("[Reflector] 구조화된 툴 호출이 발견되지 않았습니다. 종료합니다.".to_string())
                .await;
            return;
        }

        let _ = log_tx
            .send(format!(
                "[Reflector] {}개의 툴 작업을 식별하여 백그라운드에서 실행합니다.",
                tool_calls.len()
            ))
            .await;

        // Execute the tools
        let _results = self.multi_agent.execute_tools(tool_calls).await;
        // Optionally, log results
        let _ = log_tx
            .send("[Reflector] 백그라운드 회고 저장을 완료했습니다.".to_string())
            .await;
    }
}
