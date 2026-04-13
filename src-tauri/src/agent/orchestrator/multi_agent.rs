use crate::agent::llm_client::ChatMessage;
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

        let lang_display = self.get_lang_display(language);

        let fallback_planner_prompt = crate::agent::prompts::build_planner_prompt(
            system_prompt,
            &skills_rules,
            &current_time,
            &schedule_files_md,
            &brain_files_md,
        );

        let base_planner = planner_prompt_opt.unwrap_or(&fallback_planner_prompt);
        let mut planner_prompt = base_planner
            .replace("{SYSTEM_PROMPT}", system_prompt)
            .replace("{CURRENT_TIME}", &current_time)
            .replace("{BRAIN_FILES}", &brain_files_md)
            .replace("{SCHEDULES}", &schedule_files_md)
            .replace("{SKILLS_RULES}", &skills_rules);

        if !planner_prompt.contains("[GLOBAL BEHAVIOR RULES]") {
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
        let mut active_writer_id = self.routing.writer_id.clone();
        let _active_worker_id = self.routing.worker_id.clone();

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
                let _ = log_tx.send(format!("i18n:{}", serde_json::json!({"key": "log.max_loops_reached", "args": {"loops": max_loops}}))).await;
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

            let planner_res = self
                .run_planner_phase(&history, &mut active_planner_id, &log_tx, loop_count)
                .await?;

            let mut json_blocks = planner_res.native_tool_calls.clone();

            // AI Hallucination Fallback: If native processing yields 0 tools but there's a JSON block
            if json_blocks.is_empty() {
                json_blocks = extract_json_blocks(&planner_res.content);
            }

            if json_blocks.is_empty() {
                let text_res = planner_res.content.trim();

                // If it doesn't say "DONE", it means planner replied directly
                if !text_res.to_uppercase().contains("DONE") {
                    let _ = log_tx
                        .send(format!(
                            "i18n:{}",
                            serde_json::json!({"key": "log.planner_direct_answer", "args": {}})
                        ))
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
                    .send(format!(
                        "i18n:{}",
                        serde_json::json!({"key": "log.planner_tools_done", "args": {}})
                    ))
                    .await;
                // Run Writer Agent
                let (writer_res, _) = self
                    .run_writer_phase(
                        writer_prompt_opt,
                        &lang_display,
                        history.clone(),
                        &mut active_writer_id,
                        &log_tx,
                        true,
                    )
                    .await
                    .unwrap();

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

            let _ = log_tx.send(format!("i18n:{}", serde_json::json!({"key": "log.planner_tools_requested", "args": {"count": json_blocks.len()}}))).await;
            let results = self
                .multi_agent
                .execute_tools(
                    json_blocks,
                    Some(self.get_llm_for_planner()),
                    registry_prompt_opt.map(|s| s.to_string()),
                )
                .await;

            let mut result_summary_md = String::from("### Tool Execution Results\n");
            for r in &results {
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

            // [Hard-Fail Fallback] Check if the agent called an entirely bogus tool
            let mut critical_fail = None;
            for r in &results {
                if !r.ok && r.output.contains("Unknown tool") {
                    critical_fail = Some(format!(
                        "🤖 에이전트가 존재하지 않는 도구(`{}.{}`)를 호출하려다 중단되었습니다.\n\n*에러 원인: 현재 환경에서 해당 기능을 직접 수행할 수 있는 API가 없습니다. 프롬프트를 좀 더 구체적으로 변경하거나 다른 방식으로 지시해 주세요.*",
                        r.tool_name, r.action
                    ));
                    break;
                }
            }
            if let Some(fail_msg) = critical_fail {
                let _ = log_tx
                    .send(format!(
                        "i18n:{}",
                        serde_json::json!({"key": "log.critical_tool_failure", "args": {}})
                    ))
                    .await;
                history.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: fail_msg.clone(),
                    images_base64: None,
                });
                self.save_transcript(session_id.clone(), &history);
                return Ok((fail_msg, history));
            }

            // Critic Agent Phase
            let critic_res = self
                .run_critic_phase(
                    critic_prompt_opt,
                    &user_messages,
                    &result_summary_md,
                    &mut active_critic_id,
                    &log_tx,
                )
                .await
                .unwrap();

            if critic_res.content.contains("STATUS: PASS") {
                let _ = log_tx
                    .send(format!(
                        "i18n:{}",
                        serde_json::json!({"key": "log.critic_pass", "args": {}})
                    ))
                    .await;
                history.push(ChatMessage {
                    role: "user".to_string(),
                    content: result_summary_md,
                    images_base64: None,
                });

                // Run Writer Phase
                let (writer_res, _) = self
                    .run_writer_phase(
                        writer_prompt_opt,
                        &lang_display,
                        history.clone(),
                        &mut active_writer_id,
                        &log_tx,
                        false,
                    )
                    .await
                    .unwrap();

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
                let _ = log_tx
                    .send(format!(
                        "i18n:{}",
                        serde_json::json!({"key": "log.critic_fail", "args": {"feedback": fb}})
                    ))
                    .await;
                history.push(ChatMessage {
                    role: "user".to_string(),
                    content: format!("SYSTEM OBSERVATION RESULTS:\n{}\n\n[Critic Agent Feedback]\nYour last tool executed, but the task is not yet complete. Critic feedback: {}\nPlease use tools again with different strategies to fulfill the task.", result_summary_md, fb),
                    images_base64: None,
                });
            }
        }

        self.save_transcript(session_id.clone(), &history);
        Ok((
            format!(
                "i18n:{}",
                serde_json::json!({"key": "chat.agent_terminated", "args": {}})
            ),
            history,
        ))
    }
}
