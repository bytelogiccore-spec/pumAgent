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

        let lang_display = self.get_lang_display(language);

        let (current_time, brain_files_md, schedule_files_md, mut pending_tasks, skills_rules) =
            self.build_context();

        let force_tool_prompt = crate::agent::prompts::build_single_agent_prompt(
            system_prompt,
            &skills_rules,
            &current_time,
            &schedule_files_md,
            &lang_display,
            &brain_files_md,
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
                    .send(format!(
                        "i18n:{}",
                        serde_json::json!({"key": "log.agent_stopped_by_user", "args": {}})
                    ))
                    .await;
                return Ok((
                    format!(
                        "i18n:{}",
                        serde_json::json!({"key": "chat.agent_stopped", "args": {}})
                    ),
                    history,
                ));
            }

            loop_count += 1;
            if loop_count > max_loops {
                let _ = log_tx
                    .send(format!("i18n:{}", serde_json::json!({"key": "log.max_loops_reached", "args": {"loops": max_loops}})))
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
                    .send(format!(
                        "i18n:{}",
                        serde_json::json!({"key": "log.injected_scheduled_task", "args": {}})
                    ))
                    .await;
                history.push(ChatMessage {
                    role: "user".to_string(),
                    content: tasks_md,
                    images_base64: None,
                });
            }

            let _ = log_tx
                .send(format!(
                    "i18n:{}",
                    serde_json::json!({"key": "log.llm_waiting", "args": {"step": loop_count}})
                ))
                .await;

            let tools_schema = self.multi_agent.get_tool_schemas();

            let mut response_res = self
                .get_llm_client_by_id(&active_worker_id)
                .with_tools(tools_schema.clone())
                .chat(&history, 0.7)
                .await;
            if response_res.is_err() {
                let primary_err = response_res.as_ref().unwrap_err(); /* get the error clone or format string */
                let _ = log_tx.send(format!("i18n:{}", serde_json::json!({"key": "log.llm_fallback", "args": {"err": primary_err.to_string()}}))).await;

                for fallback_ep in self.get_fallback_endpoints(&active_worker_id) {
                    response_res = crate::agent::llm_client::LLMClient::new(
                        fallback_ep.api_url.clone(),
                        fallback_ep.model.clone(),
                        fallback_ep.api_key.clone(),
                    )
                    .with_tools(tools_schema.clone())
                    .chat(&history, 0.7)
                    .await;
                    if response_res.is_ok() {
                        active_worker_id = fallback_ep.id.clone();
                        let _ = log_tx.send(format!("i18n:{}", serde_json::json!({"key": "log.fallback_success", "args": {"model": fallback_ep.name}}))).await;
                        break;
                    }
                }
            }

            let response = match response_res {
                Ok(res) => res,
                Err(e) => {
                    let err_msg = e.to_string();
                    let _ = log_tx
                        .send(format!(
                            "i18n:{}",
                            serde_json::json!({"key": "log.llm_fatal", "args": {"err": err_msg}})
                        ))
                        .await;
                    return Err(format!(
                        "i18n:{}",
                        serde_json::json!({"key": "chat.agent_llm_fatal", "args": {"err": err_msg}})
                    ));
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
            let _ = log_tx.send(format!("i18n:{}", serde_json::json!({"key": "log.ai_reasoning", "args": {"preview": preview_clean}}))).await;

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
                    .send(format!(
                        "i18n:{}",
                        serde_json::json!({"key": "log.final_round_success", "args": {}})
                    ))
                    .await;
                self.save_transcript(session_id, &history);
                return Ok((Self::sanitize_output(&ai_text), history));
            }

            let _ = log_tx.send(format!("i18n:{}", serde_json::json!({"key": "log.planner_tools_requested", "args": {"count": tool_calls.len()}}))).await;

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
                let status = if r.ok { "SUCCESS" } else { "FAIL" };
                let _ = log_tx.send(format!("i18n:{}", serde_json::json!({"key": "log.tool_result", "args": {"tool": r.tool_name, "action": r.action, "status": status}}))).await;

                if r.ok
                    && (r.action == "write" || r.action == "write_artifact")
                    && (r.tool_name == "brain" || r.tool_name == "knowledge")
                {
                    let _ = log_tx.send(format!("i18n:{}", serde_json::json!({"key": "log.db_saved", "args": {"tool": r.tool_name}}))).await;
                }

                result_summary_md.push_str(&format!(
                    "**Tool**: {}.{}\n**Status**: {}\n**Output**:\n{}\n\n---\n",
                    r.tool_name, r.action, status, r.output
                ));
            }

            let _ = log_tx
                .send(format!(
                    "i18n:{}",
                    serde_json::json!({"key": "log.tools_injected", "args": {}})
                ))
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
            format!(
                "i18n:{}",
                serde_json::json!({"key": "chat.agent_terminated", "args": {}})
            ),
            history,
        ))
    }
}
