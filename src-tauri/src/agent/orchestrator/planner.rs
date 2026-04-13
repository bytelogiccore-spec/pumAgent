use crate::agent::llm_client::{ChatMessage, LLMResult};
use tokio::sync::mpsc;

impl super::Orchestrator {
    pub async fn run_planner_phase(
        &self,
        history: &[ChatMessage],
        active_planner_id: &mut String,
        log_tx: &mpsc::Sender<String>,
        loop_count: u32,
    ) -> Result<LLMResult, String> {
        let _ = log_tx
            .send(format!(
                "i18n:{}",
                serde_json::json!({"key": "log.planner_planning", "args": {"step": loop_count}})
            ))
            .await;

        let tools_schema = self.multi_agent.get_tool_schemas();

        let mut planner_res_result = self
            .get_llm_client_by_id(active_planner_id)
            .with_tools(tools_schema.clone())
            .chat(history, 0.7)
            .await;

        if planner_res_result.is_err() {
            let err = planner_res_result.as_ref().unwrap_err();
            let _ = log_tx.send(format!("i18n:{}", serde_json::json!({"key": "log.planner_fallback", "args": {"err": err.to_string()}}))).await;

            for fallback_ep in self.get_fallback_endpoints(active_planner_id) {
                planner_res_result = crate::agent::llm_client::LLMClient::new(
                    fallback_ep.api_url.clone(),
                    fallback_ep.model.clone(),
                    fallback_ep.api_key.clone(),
                )
                .with_tools(tools_schema.clone())
                .chat(history, 0.7)
                .await;
                if planner_res_result.is_ok() {
                    *active_planner_id = fallback_ep.id.clone();
                    let _ = log_tx.send(format!("i18n:{}", serde_json::json!({"key": "log.fallback_success", "args": {"model": fallback_ep.name}}))).await;
                    break;
                }
            }
        }

        match planner_res_result {
            Ok(r) => Ok(r),
            Err(e) => {
                let err_msg = e.to_string();
                let _ = log_tx
                    .send(format!(
                        "i18n:{}",
                        serde_json::json!({"key": "log.llm_fatal", "args": {"err": err_msg}})
                    ))
                    .await;
                Err(format!(
                    "i18n:{}",
                    serde_json::json!({"key": "chat.agent_llm_fatal", "args": {"err": err_msg}})
                ))
            }
        }
    }
}
