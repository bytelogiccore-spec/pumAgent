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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LlmEndpoint {
    pub id: String,
    pub name: String,
    pub api_url: String,
    pub model: String,
    pub api_key: String,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default = "default_is_first_run")]
    pub is_first_run: bool,

    #[serde(default)]
    pub endpoints: Vec<LlmEndpoint>,
    #[serde(default)]
    pub planner_endpoint_id: String,
    #[serde(default)]
    pub critic_endpoint_id: String,
    #[serde(default)]
    pub writer_endpoint_id: String,
    #[serde(default)]
    pub worker_endpoint_id: String,
    #[serde(default)]
    pub reflector_endpoint_id: String,
    #[serde(default)]
    pub registry_endpoint_id: String,

    // Legacy fields for seamless migration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_api_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_llm_api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_routing_planner: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_routing_critic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_routing_writer: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_routing_worker: Option<bool>,
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
    pub custom_languages: Vec<String>,
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
            if let Ok(mut config) = serde_json::from_str::<AppConfig>(&content) {
                // Migration logic from old single/cloud to endpoints array
                if config.endpoints.is_empty() {
                    let mut new_endpoints = Vec::new();

                    if let Some(url) = &config.api_url {
                        if !url.is_empty() {
                            new_endpoints.push(LlmEndpoint {
                                id: "local-primary".to_string(),
                                name: "Local Engine".to_string(),
                                api_url: url.clone(),
                                model: config.model.clone().unwrap_or_default(),
                                api_key: config.llm_api_key.clone().unwrap_or_default(),
                                is_enabled: true,
                            });
                        }
                    }

                    if let Some(curl) = &config.cloud_api_url {
                        if !curl.is_empty() {
                            new_endpoints.push(LlmEndpoint {
                                id: "cloud-secondary".to_string(),
                                name: "Cloud Engine".to_string(),
                                api_url: curl.clone(),
                                model: config.cloud_model.clone().unwrap_or_default(),
                                api_key: config.cloud_llm_api_key.clone().unwrap_or_default(),
                                is_enabled: true,
                            });
                        }
                    }

                    if new_endpoints.is_empty() {
                        new_endpoints.push(LlmEndpoint {
                            id: "default-local".to_string(),
                            name: "Default Local".to_string(),
                            api_url: "http://127.0.0.1:8000/v1/chat/completions".to_string(),
                            model: "gemma-4".to_string(),
                            api_key: "".to_string(),
                            is_enabled: true,
                        });
                    }

                    config.endpoints = new_endpoints;

                    // Set routing based on old flags
                    let route_cloud = |flag: Option<bool>| -> String {
                        if flag.unwrap_or(false)
                            && config.endpoints.iter().any(|e| e.id == "cloud-secondary")
                        {
                            "cloud-secondary".to_string()
                        } else if config.endpoints.iter().any(|e| e.id == "local-primary") {
                            "local-primary".to_string()
                        } else {
                            config.endpoints[0].id.clone()
                        }
                    };

                    config.planner_endpoint_id = route_cloud(config.cloud_routing_planner);
                    config.critic_endpoint_id =
                        route_cloud(config.cloud_routing_critic.or(Some(true)));
                    config.writer_endpoint_id =
                        route_cloud(config.cloud_routing_writer.or(Some(true)));
                    config.worker_endpoint_id = route_cloud(config.cloud_routing_worker);

                    // Clear legacy fields
                    config.api_url = None;
                    config.model = None;
                    config.llm_api_key = None;
                    config.cloud_api_url = None;
                    config.cloud_model = None;
                    config.cloud_llm_api_key = None;
                    config.cloud_routing_planner = None;
                    config.cloud_routing_critic = None;
                    config.cloud_routing_writer = None;
                    config.cloud_routing_worker = None;

                    // Save the migrated config immediately
                    let _ = config.save(base_dir);
                }
                return config;
            }
        }
        AppConfig {
            is_first_run: true,
            endpoints: vec![LlmEndpoint {
                id: "default-local".to_string(),
                name: "Default Local".to_string(),
                api_url: "http://127.0.0.1:8000/v1/chat/completions".to_string(),
                model: "gemma-4".to_string(),
                api_key: "".to_string(),
                is_enabled: true,
            }],
            planner_endpoint_id: "default-local".to_string(),
            critic_endpoint_id: "default-local".to_string(),
            writer_endpoint_id: "default-local".to_string(),
            worker_endpoint_id: "default-local".to_string(),
            reflector_endpoint_id: "default-local".to_string(),
            registry_endpoint_id: "default-local".to_string(),
            api_url: None,
            model: None,
            llm_api_key: None,
            cloud_api_url: None,
            cloud_model: None,
            cloud_llm_api_key: None,
            cloud_routing_planner: None,
            cloud_routing_critic: None,
            cloud_routing_writer: None,
            cloud_routing_worker: None,
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
            custom_languages: Vec::new(),
            registry_prompt: None,
            heartbeat_enabled: false,
            heartbeat_interval: 3600,
            telegram_enabled: false,
            telegram_bot_token: "".to_string(),
            telegram_chat_id: "".to_string(),
        }
    }
}
