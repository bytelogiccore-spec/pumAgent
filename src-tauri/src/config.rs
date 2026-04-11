use serde::{Deserialize, Serialize};
use std::fs;

fn default_search_provider() -> String {
    "duckduckgo".to_string()
}

fn default_language() -> String {
    "en".to_string()
}

fn default_is_first_run() -> bool {
    true
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default = "default_is_first_run")]
    pub is_first_run: bool,
    pub api_url: String,
    pub model: String,
    #[serde(default)]
    pub llm_api_key: String,
    #[serde(default)]
    pub cloud_api_url: String,
    #[serde(default)]
    pub cloud_model: String,
    #[serde(default)]
    pub cloud_llm_api_key: String,
    #[serde(default)]
    pub cloud_routing_planner: bool,
    #[serde(default = "default_true")]
    pub cloud_routing_critic: bool,
    #[serde(default = "default_true")]
    pub cloud_routing_writer: bool,
    #[serde(default)]
    pub cloud_routing_worker: bool,
    pub max_loops: u32,
    pub system_prompt: String,
    #[serde(default = "default_search_provider")]
    pub search_provider: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub tavily_api_key: String,
    #[serde(default)]
    pub google_api_key: String,
    #[serde(default)]
    pub google_cx: String,
    #[serde(default)]
    pub use_multi_agent_workflow: bool,
    #[serde(default)]
    pub use_think_mode: bool,

    #[serde(default)]
    pub planner_prompt: Option<String>,
    #[serde(default)]
    pub critic_prompt: Option<String>,
    #[serde(default)]
    pub writer_prompt: Option<String>,
    #[serde(default)]
    pub reflector_prompt: Option<String>,
    #[serde(default)]
    pub heartbeat_prompt: Option<String>,
    #[serde(default)]
    pub worker_prompt: Option<String>,
    #[serde(default)]
    pub registry_prompt: Option<String>,

    #[serde(default)]
    pub heartbeat_enabled: bool,
    #[serde(default)]
    pub heartbeat_interval: u64,

    #[serde(default)]
    pub telegram_enabled: bool,
    #[serde(default)]
    pub telegram_bot_token: String,
    #[serde(default)]
    pub telegram_chat_id: String,
}

impl AppConfig {
    pub fn save(&self, base_dir: &std::path::Path) -> Result<(), String> {
        let config_path = base_dir.join("agent_config.json");
        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&config_path, content).map_err(|e| e.to_string())
    }

    pub fn load(base_dir: &std::path::Path) -> Self {
        let config_path = base_dir.join("agent_config.json");
        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(config) = serde_json::from_str(&content) {
                return config;
            }
        }
        AppConfig {
            is_first_run: true,
            api_url: "http://127.0.0.1:8000/v1/chat/completions".to_string(),
            model: "gemma-4".to_string(),
            llm_api_key: "".to_string(),
            cloud_api_url: "".to_string(),
            cloud_model: "anthropic/claude-3-opus-20240229".to_string(),
            cloud_llm_api_key: "".to_string(),
            cloud_routing_planner: false,
            cloud_routing_critic: true,
            cloud_routing_writer: true,
            cloud_routing_worker: false,
            max_loops: 20,
            system_prompt: "You are a highly capable autonomous AI assistant. Think clearly, step-by-step, and strictly follow the formatting rules to achieve the user's goal.".to_string(),
            search_provider: "duckduckgo".to_string(),
            language: "en".to_string(),
            tavily_api_key: "".to_string(),
            google_api_key: "".to_string(),
            google_cx: "".to_string(),
            use_multi_agent_workflow: false,
            use_think_mode: false,
            planner_prompt: None,
            critic_prompt: None,
            writer_prompt: None,
            reflector_prompt: None,
            heartbeat_prompt: None,
            worker_prompt: None,
            registry_prompt: None,
            heartbeat_enabled: false,
            heartbeat_interval: 3600,
            telegram_enabled: false,
            telegram_bot_token: "".to_string(),
            telegram_chat_id: "".to_string(),
        }
    }
}
