use chrono::{DateTime, Local, Utc};
use dbx_core::Database;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ScheduleConfig {
    pub name: String,
    #[serde(default)]
    pub interval_seconds: Option<u64>,
    #[serde(default)]
    pub cron_expression: Option<String>,
    pub description: String,
    pub task_prompt: String,
    #[serde(default)]
    pub last_run: Option<String>, // ISO 8601
    #[serde(default)]
    pub end_date: Option<String>,
}

pub struct Scheduler {
    db: Arc<Database>,
}

impl Scheduler {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Reads all JSON schedules and checks which ones need to run based on current time.
    /// Returns a tuple: (List of pending task_prompts, A string summarizing all registered schedules for LLM context, A string summarizing time left for all schedules)
    pub fn evaluate_schedules(&self) -> (Vec<(String, ScheduleConfig)>, String, String) {
        let mut pending_tasks = Vec::new();
        let mut schedules_summary = String::new();
        let mut status_summary = String::new();
        let now: DateTime<Utc> = Utc::now();
        let prefix = "schedules:";

        // Use DBX-Core's native range scan for O(logN + K) performance instead of Full-Scan O(N)
        let start_key = b"schedules:";
        let end_key = b"schedules;"; // Next ASCII char after ':' is ';'

        if let Ok(entries) = self.db.range("knowledge_base", start_key, end_key) {
            for (key, val) in entries {
                if let Ok(key_str) = String::from_utf8(key) {
                    let ext = if key_str.ends_with(".json") {
                        "json"
                    } else if key_str.ends_with(".md") {
                        "md"
                    } else {
                        ""
                    };

                    if let Ok(content) = String::from_utf8(val) {
                        if ext == "json" {
                            if let Ok(sched) = serde_json::from_str::<ScheduleConfig>(&content) {
                                let mut should_run = false;
                                let mut next_exec = "Unknown".to_string();

                                let last_run_dt = sched
                                    .last_run
                                    .as_ref()
                                    .and_then(|s| s.parse::<DateTime<Utc>>().ok());

                                let mut is_expired = false;
                                if let Some(end_str) = &sched.end_date {
                                    if let Ok(end_dt) = end_str.parse::<DateTime<Utc>>() {
                                        if now > end_dt {
                                            is_expired = true;
                                            next_exec = "Expired".to_string();
                                        }
                                    }
                                }

                                // Check Cron & Interval
                                if !is_expired {
                                    if let Some(cron_expr) = &sched.cron_expression {
                                        if let Ok(cron) = cron::Schedule::from_str(cron_expr) {
                                            if let Some(last) = last_run_dt {
                                                if let Some(target) = cron.after(&last).next() {
                                                    if now >= target {
                                                        should_run = true;
                                                    }
                                                }
                                            } else {
                                                should_run = true; // Run immediately to set baseline
                                            }
                                            if let Some(next_upcoming) = cron.upcoming(Local).next() {
                                                next_exec = format!("{} (Cron)", next_upcoming.format("%Y-%m-%d %H:%M:%S"));
                                            }
                                        } else {
                                            next_exec = "Cron Syntax Error".to_string();
                                        }
                                    } else if let Some(interval) = sched.interval_seconds {
                                        if let Some(last) = last_run_dt {
                                            let diff = (now - last).num_seconds();
                                            if diff >= interval as i64 {
                                                should_run = true;
                                            }
                                            let next = last + chrono::Duration::seconds(interval as i64);
                                            next_exec = next.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string();
                                        } else {
                                            should_run = true;
                                            let next = now + chrono::Duration::seconds(interval as i64);
                                            next_exec = next.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string();
                                        }
                                    }
                                }

                                schedules_summary.push_str(&format!(
                                    "---\n[{}]\n{}\n",
                                    sched.name, sched.description
                                ));
                                status_summary.push_str(&format!(
                                    "- **{}**: Next execution scheduled ({})\n",
                                    sched.name, next_exec
                                ));

                                if should_run {
                                    pending_tasks.push((key_str, sched));
                                }
                            }
                        } else if ext == "md" {
                            let file_name = key_str.replace(prefix, "");
                            schedules_summary.push_str(&format!(
                                "---\n[{}] (Legacy Markdown)\n{}\n",
                                file_name, content
                            ));
                            status_summary.push_str(&format!(
                                "- **{}**: Legacy Markdown rule (Execution time unparseable)\n",
                                file_name
                            ));
                        }
                    }
                }
            }
        }

        if schedules_summary.is_empty() {
            schedules_summary = "No schedules registered.".to_string();
            status_summary = "No schedules registered.".to_string();
        }

        (pending_tasks, schedules_summary, status_summary)
    }

    pub fn update_last_run(&self, key: &str, mut sched: ScheduleConfig) {
        sched.last_run = Some(Utc::now().to_rfc3339());
        if let Ok(content) = serde_json::to_string_pretty(&sched) {
            let _ = self
                .db
                .insert("knowledge_base", key.as_bytes(), content.as_bytes());
            let _ = self.db.flush();
        }
    }
}
