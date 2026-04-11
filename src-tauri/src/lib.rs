pub mod agent;
pub mod commands;
pub mod config;
pub mod state;
pub mod tools;

use config::AppConfig;
use state::AgentState;

use agent::multi_agent::MultiAgent;
use dbx_core::Database;
use std::fs;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tools::brain::BrainTool;
use tools::crawler::Crawler;
use tools::knowledge::KnowledgeTool;
use tools::search::SearchTool;
use tools::telegram_tool::TelegramTool;
use tools::terminal::TerminalTool;

// Import our isolated commands

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
                if let Some(state) = window.try_state::<AgentState>() {
                    let _ = state.db.flush();
                    println!("[PumAgent] Window minimized to tray, performed DB flush for safety.");
                }
            }
        })
        .setup(|app| {
            let app_handle = app.handle().clone();
            std::panic::set_hook(Box::new(move |info| {
                let msg = match info.payload().downcast_ref::<&'static str>() {
                    Some(s) => *s,
                    None => match info.payload().downcast_ref::<String>() {
                        Some(s) => &s[..],
                        None => "Unknown panic",
                    },
                };
                let location = info.location().unwrap();
                let error_message = format!(
                    "Rust Panic! File: {}, Line: {}\n\n{}",
                    location.file(),
                    location.line(),
                    msg
                );
                eprintln!("[PumAgent Panic] {}", error_message);
                let _ = app_handle.emit("system_panic", error_message);
            }));

            #[cfg(debug_assertions)]
            let base_dir = std::env::current_dir()
                .unwrap_or_default()
                .join("..")
                .join("..")
                .join("PumAgentData");

            #[cfg(not(debug_assertions))]
            let base_dir = std::env::current_dir()
                .unwrap_or_default()
                .join("..")
                .join("PumAgentData");

            if !base_dir.exists() {
                let _ = fs::create_dir_all(&base_dir);
            }

            // Initialize DBX Core
            let db_path = base_dir.join("pumagent_store.dbx");
            let db = Database::open(&db_path).expect("Failed to initialize DBX engine.");

            // --- One Time Migration Script ---
            // Let's migrate legacy `knowledge/schedules` if they exist
            let sched_dir = base_dir.join("knowledge").join("schedules");
            if sched_dir.exists() {
                if let Ok(entries) = fs::read_dir(&sched_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() {
                            let fname = p.file_name().unwrap().to_string_lossy().to_string();
                            if let Ok(content) = fs::read_to_string(&p) {
                                let key = format!("schedules:{}", fname);
                                let _ =
                                    db.insert("knowledge_base", key.as_bytes(), content.as_bytes());
                            }
                        }
                    }
                }
                let _ = fs::rename(
                    &sched_dir,
                    base_dir.join("knowledge").join("schedules_migrated"),
                );
            }

            // Legacy Brain migration
            let brain_dir = base_dir.join("brain");
            if brain_dir.exists() {
                if let Ok(entries) = fs::read_dir(&brain_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() {
                            let fname = p.file_name().unwrap().to_string_lossy().to_string();
                            if let Ok(content) = fs::read_to_string(&p) {
                                let _ = db.insert(
                                    "brain_artifacts",
                                    fname.as_bytes(),
                                    content.as_bytes(),
                                );
                            }
                        }
                    }
                }
                let _ = fs::rename(&brain_dir, base_dir.join("brain_migrated"));
            }
            // ---------------------------------

            let crawler = Crawler::new();
            let search_tool =
                SearchTool::new("API_KEY".to_string(), "CX".to_string(), base_dir.clone());
            let brain_tool = BrainTool::new(db.clone());
            let terminal_tool = TerminalTool::new(base_dir.clone());
            let knowledge_tool = KnowledgeTool::new(db.clone());
            let telegram_tool = TelegramTool::new(base_dir.clone());

            let state = AgentState {
                multi_agent: Arc::new(MultiAgent::new(
                    crawler,
                    search_tool,
                    brain_tool,
                    terminal_tool,
                    knowledge_tool,
                    telegram_tool,
                )),
                base_dir: base_dir.clone(),
                cancel_flag: Arc::new(AtomicBool::new(false)),
                db: db.clone(),
            };
            let multi_agent_ref = state.multi_agent.clone();
            app.manage(state);

            let app_handle = app.handle().clone();

            // Setup Tray Icon & Menu
            let heartbeat_item =
                tauri::menu::MenuItemBuilder::with_id("heartbeat", "❤️ -").build(app)?;
            let maximize_item =
                tauri::menu::MenuItemBuilder::with_id("maximize", "Maximize").build(app)?;
            let quit_item = tauri::menu::MenuItemBuilder::with_id("quit", "Quit").build(app)?;

            let menu = tauri::menu::MenuBuilder::new(app)
                .item(&heartbeat_item)
                .item(&maximize_item)
                .item(&quit_item)
                .build()?;

            let _tray = tauri::tray::TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "quit" => {
                        std::process::exit(0);
                    }
                    "maximize" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            let heartbeat_item_clone = heartbeat_item.clone();

            let initial_config = AppConfig::load(&base_dir);
            if initial_config.telegram_enabled && !initial_config.telegram_bot_token.is_empty() {
                let ma_clone = multi_agent_ref.clone();
                let bd_clone = base_dir.clone();
                let ah_clone = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    crate::agent::telegram::start_telegram_bot(
                        initial_config,
                        bd_clone,
                        ma_clone,
                        ah_clone,
                        db.clone(),
                    )
                    .await;
                });
            }

            tauri::async_runtime::spawn(async move {
                let mut last_tick = tokio::time::Instant::now();
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    let config = AppConfig::load(&base_dir);
                    if config.heartbeat_enabled && config.heartbeat_interval > 0 {
                        let elapsed = last_tick.elapsed().as_secs();
                        let remaining = config.heartbeat_interval.saturating_sub(elapsed);
                        let _ = app_handle.emit("heartbeat_progress", remaining);
                        let format_time = |secs: u64| {
                            let h = secs / 3600;
                            let m = (secs % 3600) / 60;
                            let s = secs % 60;
                            if h > 0 {
                                format!("{:02}:{:02}:{:02}", h, m, s)
                            } else {
                                format!("{:02}:{:02}", m, s)
                            }
                        };
                        let _ =
                            heartbeat_item_clone.set_text(format!("❤️ {}", format_time(remaining)));

                        if elapsed >= config.heartbeat_interval {
                            last_tick = tokio::time::Instant::now();
                            let _ = app_handle.emit("heartbeat_tick", ());
                        }
                    } else {
                        last_tick = tokio::time::Instant::now();
                        let _ = app_handle.emit("heartbeat_progress", 0u64);
                        let _ = heartbeat_item_clone.set_text("❤️ Disabled");
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::agent::execute_agent_tools,
            commands::agent::execute_background_scheduler,
            commands::agent::compress_memory,
            commands::agent::load_config,
            commands::agent::save_config,
            commands::agent::test_llm_connection,
            commands::fs::list_brain_artifacts,
            commands::fs::read_brain_artifact,
            commands::fs::write_brain_artifact,
            commands::fs::delete_brain_artifact,
            commands::fs::list_logs,
            commands::fs::read_log,
            commands::fs::delete_logs,
            commands::fs::list_knowledge,
            commands::fs::read_knowledge,
            commands::fs::write_knowledge,
            commands::fs::delete_knowledge,
            commands::agent::flush_db,
            commands::agent::translate_i18n,
            commands::agent::stop_agent
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_dbx_contents() {
        let db_path = std::env::current_dir()
            .unwrap()
            .join("..")
            .join("PumAgentData")
            .join("pumagent_store.dbx");

        let db = dbx_core::Database::open(&db_path).unwrap();
        println!("--- BRAIN ARTIFACTS ---");
        if let Ok(entries) = db.scan("brain_artifacts") {
            for (k, v) in entries {
                println!("KEY: {}", String::from_utf8_lossy(&k));
                println!("VAL: {}\n", String::from_utf8_lossy(&v));
            }
        }
        println!("--- SCHEDULES (knowledge_base) ---");
        if let Ok(entries) = db.scan("knowledge_base") {
            for (k, v) in entries {
                if k.starts_with(b"schedules:") {
                    println!("KEY: {}", String::from_utf8_lossy(&k));
                    println!("VAL: {}\n", String::from_utf8_lossy(&v));
                }
            }
        }
    }
}
