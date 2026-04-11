mod commands;
mod db;
mod scheduler;

use commands::AppState;
use db::Database;
use scheduler::{start_scheduler_loop, WorkflowScheduler};
use std::sync::{Arc, Mutex};
use tauri::{
    tray::{TrayIcon, TrayIconBuilder},
    Manager, WebviewUrl, WebviewWindowBuilder,
};


/// Held in managed state so the tray icon is never dropped while the app runs.
pub struct TrayState {
    pub _icon: TrayIcon,
}

fn detect_chaos_labs_root() -> String {
    if let Ok(home) = std::env::var("HOME") {
        let candidate = format!("{}/chaos-labs", home);
        if std::path::Path::new(&candidate).exists() {
            return candidate;
        }
    }
    eprintln!("WARNING: ~/chaos-labs not found; defaulting to $HOME/chaos-labs");
    std::env::var("HOME")
        .map(|h| format!("{}/chaos-labs", h))
        .unwrap_or_else(|_| String::from("/tmp/chaos-labs"))
}

fn detect_python_path(chaos_labs_root: &str) -> String {
    let venv_python = format!("{}/.venv/bin/python3", chaos_labs_root);
    if std::path::Path::new(&venv_python).exists() {
        return venv_python;
    }
    String::from("python3")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .build(),
            )?;

            let app_data_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            std::fs::create_dir_all(&app_data_dir).ok();

            let chaos_labs_root = detect_chaos_labs_root();
            let python_path = detect_python_path(&chaos_labs_root);

            let db = Arc::new(Database::new(&app_data_dir));
            let scheduler = Arc::new(Mutex::new(WorkflowScheduler::new(
                db.clone(),
            )));

            app.manage(AppState {
                db: db.clone(),
                scheduler: scheduler.clone(),
                chaos_labs_root: chaos_labs_root.clone(),
                python_path: python_path.clone(),
            });

            start_scheduler_loop(
                scheduler.clone(),
                db.clone(),
                chaos_labs_root,
                python_path,
                app.handle().clone(),
            );

            let handle = app.handle().clone();
            let _ = WebviewWindowBuilder::new(&handle, "popup", WebviewUrl::App("index.html?view=popup".into()))
                .title("Chaos Labs")
                .inner_size(340.0, 440.0)
                .resizable(false)
                .visible(false)
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .build()?;

            let tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().expect("No icon"))
                .icon_as_template(true)
                .tooltip("Chaos Labs Scheduler")
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button_state: tauri::tray::MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(popup) = app.get_webview_window("popup") {
                            if popup.is_visible().unwrap_or(false) {
                                let _ = popup.hide();
                            } else {
                                let phys_pos = rect.position.to_physical::<f64>(1.0);
                                let phys_size = rect.size.to_physical::<f64>(1.0);

                                // Derive the clicked screen's scale factor from the tray icon's
                                // physical height. macOS menu bar buttons are ~22-24 logical points;
                                // on 2x Retina they report ~44-48 physical pixels.
                                let scale = if phys_size.height > 30.0 { 2.0 } else { 1.0 };

                                let icon_x = phys_pos.x / scale;
                                let icon_y = phys_pos.y / scale;
                                let icon_w = phys_size.width / scale;
                                let icon_h = phys_size.height / scale;

                                let popup_width = 340.0_f64;
                                let x = icon_x + (icon_w / 2.0) - (popup_width / 2.0);
                                let y = icon_y + icon_h;

                                log::info!(
                                    "Tray click: phys=({:.0},{:.0}) size=({:.0},{:.0}) scale={} -> logical=({:.0},{:.0})",
                                    phys_pos.x, phys_pos.y, phys_size.width, phys_size.height,
                                    scale, x, y
                                );

                                let _ = popup.set_position(tauri::LogicalPosition::new(x, y));
                                let _ = popup.show();
                                let _ = popup.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            app.manage(TrayState { _icon: tray });

            log::info!("Chaos Labs Scheduler started, tray icon created");

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_config,
            commands::list_workflows,
            commands::get_workflow,
            commands::create_workflow,
            commands::update_workflow,
            commands::delete_workflow,
            commands::trigger_workflow,
            commands::get_run_history,
            commands::get_run_log,
            commands::get_scheduler_status,
            commands::list_available_scripts,
            commands::open_dashboard,
            commands::open_run_detail,
            commands::hide_popup,
            commands::open_url,
            commands::quit_app,
            commands::get_launch_at_login,
            commands::set_launch_at_login,
            commands::set_notification_prefs,
            commands::analyze_run_error,
            commands::generate_workflow_description,
            commands::get_email_config,
            commands::set_email_config,
            commands::test_email_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
