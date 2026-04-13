use tauri::menu::{MenuBuilder, MenuItemBuilder, MenuEvent};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let heartbeat_item = MenuItemBuilder::with_id("heartbeat", "HeartBeat: -").enabled(false).build(app)?;
    let maximize_item = MenuItemBuilder::with_id("maximize", "최대화").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "종료").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&heartbeat_item)
        .item(&maximize_item)
        .item(&quit_item)
        .build()?;

    let tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "quit" => app.exit(0),
            "maximize" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|_tray, event| {
            if let tauri::tray::TrayIconEvent::Click { .. } = event {
                // testing the enum
            }
        })
        .build(app)?;

    // test updating text
    heartbeat_item.set_text("HeartBeat: 10초 남음")?;

    Ok(())
}
