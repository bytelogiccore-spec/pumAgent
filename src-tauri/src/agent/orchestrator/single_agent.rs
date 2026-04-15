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
        trace_id: &str,
        system_prompt: &str,
        planner_prompt_opt: Option<&str>,
        critic_prompt_opt: Option<&str>,
        writer_prompt_opt: Option<&str>,
        user_messages: Vec<ChatMessage>,
        language: &str,
        max_loops: u32,
        use_multi_agent_workflow: bool,
        use_think_mode: bool,
        registry_prompt_opt: Option<&str>,
        log_tx: mpsc::Sender<String>,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<(String, Vec<ChatMessage>), String> {
        let _is_background_run = user_messages.is_empty();
        let working_messages = self.maybe_compact_messages(user_messages).await;

        let (final_out, history) = if use_multi_agent_workflow {
            self.run_multi_agent_pipeline(
                session_id.clone(),
                trace_id,
                system_prompt,
                planner_prompt_opt,
                critic_prompt_opt,
                writer_prompt_opt,
                working_messages,
                language,
                max_loops,
                use_think_mode,
                registry_prompt_opt,
                log_tx.clone(),
                cancel_flag,
            )
            .await?
        } else {
            self.run_single_agent_internal(
                session_id.clone(),
                trace_id,
                system_prompt,
                working_messages,
                language,
                max_loops,
                use_think_mode,
                registry_prompt_opt,
                log_tx.clone(),
                cancel_flag,
            )
            .await?
        };

        // --- Consolidated Background Tasks (Reflector & Transcript) ---
        let orchestrator_clone = self.clone();
        let history_clone = history.clone();
        let log_tx_clone = log_tx.clone();
        let session_id_clone = session_id.clone();
        let trace_id_owned = trace_id.to_string();

        tokio::spawn(async move {
            // Auto-Memory Consolidation (Reflector Phase)
            // Trigger reflector if there's significant history (not just 1 system + 1 user + 1 assistant)
            // Or if it was a multi-agent run which is inherently complex
            if history_clone.len() > 3 {
                let ref_prompt = crate::agent::prompts::get_fallback_reflector_prompt();
                orchestrator_clone
                    .run_reflector_pipeline(
                        &trace_id_owned,
                        ref_prompt,
                        history_clone.clone(),
                        log_tx_clone,
                    )
                    .await;
            }
            orchestrator_clone.save_transcript(session_id_clone, &history_clone);
        });

        Ok((final_out, history))
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_single_agent_internal(
        &self,
        _session_id: Option<String>,
        trace_id: &str,
        system_prompt: &str,
        user_messages: Vec<ChatMessage>,
        language: &str,
        max_loops: u32,
        use_think_mode: bool,
        registry_prompt_opt: Option<&str>,
        log_tx: mpsc::Sender<String>,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<(String, Vec<ChatMessage>), String> {
        let is_background_run = user_messages.is_empty();
        let lang_display = self.get_lang_display(language);

        let (current_time, brain_files_md, schedule_files_md, mut pending_tasks, skills_rules) =
            self.build_context();

        let mut force_tool_prompt = crate::agent::prompts::build_single_agent_prompt(
            system_prompt,
            &skills_rules,
            &current_time,
            &schedule_files_md,
            &lang_display,
            &brain_files_md,
        );

        if use_think_mode {
            force_tool_prompt = force_tool_prompt.replace(
                "{THINK_MODE_RULE}",
                "- THINKING & TOOL EXECUTION: During intermediate steps where you are gathering data and planning, you MUST reason in **ENGLISH** inside <think>...</think> tags to maximize cognitive accuracy."
            );
        } else {
            force_tool_prompt = force_tool_prompt.replace(
                "{THINK_MODE_RULE}",
                "- THINKING & TOOL EXECUTION: DO NOT output any internal thought processes or reasoning. Provide only the direct tool call blocks."
            );
        }

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

            if loop_count == 1 && !pending_tasks.is_empty() && is_background_run {
                let mut tasks_md = String::from("[SYSTEM: PENDING SCHEDULED TASKS DETECTED]\nThe following scheduled tasks must be executed immediately because their scheduled time has arrived:\n");
                for (path, sched) in pending_tasks.drain(..) {
                    tasks_md.push_str(&format!(
                        "- Task Name: {}\n  Instruction: {}\n",
                        sched.name, sched.task_prompt
                    ));
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
                let primary_err = response_res.as_ref().unwrap_err();
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
            let mut chars = ai_text.chars();
            let snippet: String = chars.by_ref().take(100).collect();
            let preview = if chars.next().is_some() {
                format!("{}...", snippet)
            } else {
                snippet
            };
            let preview_clean = preview.replace("\n", " ");
            let _ = log_tx.send(format!("i18n:{}", serde_json::json!({"key": "log.ai_reasoning", "args": {"preview": preview_clean}}))).await;

            let mut clean_ai_text = ai_text.clone();
            if let Some(cutoff) = clean_ai_text.find("<|tool_response>") {
                clean_ai_text.truncate(cutoff);
            }

            history.push(ChatMessage {
                role: "assistant".to_string(),
                content: clean_ai_text,
                images_base64: None,
            });

            let tool_calls = extract_json_blocks(&ai_text);
            if tool_calls.is_empty() {
                let _ = log_tx
                    .send(format!(
                        "i18n:{}",
                        serde_json::json!({"key": "log.final_round_success", "args": {}})
                    ))
                    .await;
                return Ok((Self::sanitize_output(&ai_text), history));
            }

            let _ = log_tx.send(format!("i18n:{}", serde_json::json!({"key": "log.planner_tools_requested", "args": {"count": tool_calls.len()}}))).await;
            let results = self
                .multi_agent
                .execute_tools(
                    tool_calls,
                    Some(self.get_llm_client_by_id(&active_worker_id)),
                    registry_prompt_opt.map(|s| s.to_string()),
                    Some(trace_id.to_string()),
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
            history.push(ChatMessage { role: "user".to_string(), content: format!("SYSTEM OBSERVATION RESULTS:\n{}\n\nINSTRUCTION: If these results fulfill the user's most recent request completely, provide your FINAL direct response to the user. Explain ONLY what was just accomplished. Do NOT artificially repeat conversational history. If you need more data or intermediate steps, respond ONLY with JSON tool blocks.", result_summary_md), images_base64: None });
        }

        if let Some(ChatMessage {
            role: r,
            content: c,
            ..
        }) = history.last()
        {
            if r == "assistant" {
                return Ok((Self::sanitize_output(c), history));
            }
        }
        Ok((
            format!(
                "i18n:{}",
                serde_json::json!({"key": "chat.agent_terminated", "args": {}})
            ),
            history,
        ))
    }
}
