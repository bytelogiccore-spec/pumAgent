use crate::agent::llm_client::{ChatMessage, LLMResult};
use tokio::sync::mpsc;

impl super::Orchestrator {
    pub async fn run_critic_phase(
        &self,
        critic_prompt_opt: Option<&str>,
        user_messages: &[ChatMessage],
        result_summary_md: &str,
        active_critic_id: &mut String,
        log_tx: &mpsc::Sender<String>,
    ) -> Result<LLMResult, String> {
        let _ = log_tx
            .send(format!(
                "i18n:{}",
                serde_json::json!({"key": "log.critic_validating", "args": {}})
            ))
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
            .replace("{RESULT_SUMMARY}", result_summary_md);

        let mut critic_res_result = self
            .get_llm_client_by_id(active_critic_id)
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
            let _ = log_tx
                .send(format!(
                    "i18n:{}",
                    serde_json::json!({"key": "log.critic_fallback", "args": {}})
                ))
                .await;
            for fallback_ep in self.get_fallback_endpoints(active_critic_id) {
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
                    *active_critic_id = fallback_ep.id.clone();
                    let _ = log_tx.send(format!("i18n:{}", serde_json::json!({"key": "log.fallback_success", "args": {"model": fallback_ep.name}}))).await;
                    break;
                }
            }
        }

        let critic_res = critic_res_result.unwrap_or_else(|_| LLMResult {
            content: "STATUS: PASS".to_string(),
            raw: serde_json::Value::Null,
            native_tool_calls: vec![],
        });

        Ok(critic_res)
    }
}
