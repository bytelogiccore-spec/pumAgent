use crate::agent::llm_client::{ChatMessage, LLMResult};
use tokio::sync::mpsc;

impl super::Orchestrator {
    pub async fn run_writer_phase(
        &self,
        writer_prompt_opt: Option<&str>,
        lang_name: &str,
        mut history: Vec<ChatMessage>,
        active_writer_id: &mut String,
        log_tx: &mpsc::Sender<String>,
        is_tool_done: bool,
    ) -> Result<(LLMResult, Vec<ChatMessage>), String> {
        let writer_prompt = if is_tool_done {
            let fallback_writer_prompt = crate::agent::prompts::get_fallback_writer_prompt(lang_name);
            let writer_system = writer_prompt_opt.unwrap_or(&fallback_writer_prompt);
            let writer_prompt_msg = ChatMessage {
                role: "system".to_string(),
                content: writer_system.to_string(),
                images_base64: None,
            };
            let writer_directive = crate::agent::prompts::get_writer_final_directive(lang_name);
            history.push(ChatMessage {
                role: "user".to_string(),
                content: writer_directive,
                images_base64: None,
            });
            writer_prompt_msg
        } else {
            let fallback_writer_prompt = format!("You are a WRITER Agent. Synthesize the findings and user conversations into a highly professional response entirely in natural {}.", lang_name);
            let writer_system = writer_prompt_opt.unwrap_or(&fallback_writer_prompt);
            let writer_prompt_msg = ChatMessage {
                role: "system".to_string(),
                content: writer_system.to_string(),
                images_base64: None,
            };
            history.push(ChatMessage { role: "user".to_string(), content: format!("The user's request has been fulfilled or the data is ready. Please provide the final response to the user in {}. DO NOT unnecessarily repeat conversational history. Focus ONLY on what was just done or discovered.", lang_name), images_base64: None });
            writer_prompt_msg
        };

        history.insert(0, writer_prompt);

        let _ = log_tx
            .send(format!(
                "i18n:{}",
                if is_tool_done { serde_json::json!({"key": "log.writer_writing", "args": {}}) } else { serde_json::json!({"key": "log.writer_summarizing", "args": {}}) }
            ))
            .await;

        let mut writer_res_result = self
            .get_llm_client_by_id(active_writer_id)
            .chat(&history, 0.7)
            .await;
            
        if writer_res_result.is_err() {
            let _ = log_tx
                .send(format!(
                    "i18n:{}",
                    serde_json::json!({"key": "log.writer_fallback", "args": {}})
                ))
                .await;
            for fallback_ep in self.get_fallback_endpoints(active_writer_id) {
                writer_res_result = crate::agent::llm_client::LLMClient::new(
                    fallback_ep.api_url.clone(),
                    fallback_ep.model.clone(),
                    fallback_ep.api_key.clone(),
                )
                .chat(&history, 0.7)
                .await;
                if writer_res_result.is_ok() {
                    *active_writer_id = fallback_ep.id.clone();
                    let _ = log_tx.send(format!("i18n:{}", serde_json::json!({"key": "log.fallback_success", "args": {"model": fallback_ep.name}}))).await;
                    break;
                }
            }
        }

        let writer_res = writer_res_result.unwrap_or_else(|_| LLMResult {
            content: "Writer Error".to_string(),
            raw: serde_json::Value::Null,
            native_tool_calls: vec![],
        });

        Ok((writer_res, history))
    }
}
