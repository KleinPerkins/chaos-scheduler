mod commands;
mod db;
mod scheduler;

use commands::AppState;
use db::Database;
use scheduler::{start_scheduler_loop, WorkflowScheduler};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{
    tray::{TrayIcon, TrayIconBuilder},
    Manager, WebviewUrl, WebviewWindowBuilder,
};

/// Held in managed state so the tray icon is never dropped while the app runs.
pub struct TrayState {
    pub _icon: TrayIcon,
}

/// Holds the singleton listener for the lifetime of the process.
pub struct SingleInstanceState {
    pub _listener: TcpListener,
}

const TRAY_ID: &str = "chaos-labs-scheduler-tray";
const SINGLE_INSTANCE_ADDR: &str = "127.0.0.1:9616";

fn acquire_single_instance_lock() -> std::io::Result<TcpListener> {
    acquire_single_instance_lock_at(SINGLE_INSTANCE_ADDR)
}

fn acquire_single_instance_lock_at(addr: &str) -> std::io::Result<TcpListener> {
    TcpListener::bind(addr)
}

fn detect_chaos_labs_root() -> String {
    // The data root is configuration, not a hardcode: honor CHAOS_LABS_ROOT
    // (set by the launchd plist) when present, then fall back to the canonical
    // repo location where the data, Python scripts, and venv actually live.
    if let Ok(root) = std::env::var("CHAOS_LABS_ROOT") {
        let root = root.trim();
        if !root.is_empty() {
            return root.to_string();
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return format!("{}/dev/personal/chaos-labs", home);
    }
    eprintln!("WARNING: HOME not set; defaulting to /tmp/chaos-labs");
    String::from("/tmp/chaos-labs")
}

fn detect_python_path(chaos_labs_root: &str) -> String {
    let venv_python = format!("{}/.venv/bin/python3", chaos_labs_root);
    if std::path::Path::new(&venv_python).exists() {
        return venv_python;
    }
    String::from("python3")
}

fn migrate_legacy_scheduler_db(app_data_dir: &Path) {
    let new_db = app_data_dir.join("scheduler.db");
    let Ok(home) = std::env::var("HOME") else {
        return;
    };
    let legacy_db = PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("com.chaoslabs.scheduler")
        .join("scheduler.db");

    if legacy_db.exists() {
        let should_migrate = if new_db.exists() {
            match rusqlite::Connection::open(&new_db) {
                Ok(conn) => {
                    let workflow_count =
                        conn.query_row("SELECT COUNT(*) FROM workflows", [], |row| {
                            row.get::<_, i64>(0)
                        });
                    matches!(workflow_count, Ok(0))
                }
                Err(_) => false,
            }
        } else {
            true
        };

        if !should_migrate {
            return;
        }

        if new_db.exists() {
            let _ = std::fs::remove_file(&new_db);
        }

        match std::fs::copy(&legacy_db, &new_db) {
            Ok(_) => log::info!("Migrated legacy scheduler database into new app data dir"),
            Err(err) => log::warn!("Failed to migrate legacy scheduler database: {err}"),
        }
    }
}

fn start_metrics_endpoint(db: Arc<Database>) {
    std::thread::spawn(move || {
        let Ok(listener) = TcpListener::bind("127.0.0.1:9617") else {
            log::warn!("Failed to bind Scheduler metrics endpoint on 127.0.0.1:9617");
            return;
        };
        log::info!("Scheduler metrics endpoint listening on 127.0.0.1:9617");
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let mut buffer = [0_u8; 512];
            let _ = stream.read(&mut buffer);
            let request = String::from_utf8_lossy(&buffer);
            if !request.starts_with("GET /metrics ") {
                let body = "not found\n";
                let response = format!(
                    "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n{}",
                    body.len(), body
                );
                let _ = stream.write_all(response.as_bytes());
                continue;
            }
            let body = db
                .prometheus_metrics()
                .unwrap_or_else(|e| format!("# scheduler_metrics_error {}\n", e));
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain; version=0.0.4\r\n\r\n{}",
                body.len(), body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
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

            let single_instance_listener = match acquire_single_instance_lock() {
                Ok(listener) => listener,
                Err(err) => {
                    log::warn!(
                        "Another Chaos Labs Scheduler instance is already active; exiting before startup ({err})"
                    );
                    std::process::exit(0);
                }
            };
            app.manage(SingleInstanceState {
                _listener: single_instance_listener,
            });

            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");
            std::fs::create_dir_all(&app_data_dir).ok();
            migrate_legacy_scheduler_db(&app_data_dir);

            let chaos_labs_root = detect_chaos_labs_root();
            let python_path = detect_python_path(&chaos_labs_root);

            let db = Arc::new(Database::new(&app_data_dir));
            let scheduler = Arc::new(Mutex::new(WorkflowScheduler::new(db.clone())));
            start_metrics_endpoint(db.clone());

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
            let _ = WebviewWindowBuilder::new(
                &handle,
                "popup",
                WebviewUrl::App("index.html?view=popup".into()),
            )
            .title("Chaos Labs")
            .inner_size(340.0, 440.0)
            .resizable(false)
            .visible(false)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .build()?;

            let tray = TrayIconBuilder::with_id(TRAY_ID)
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
            commands::rerun_workflow,
            commands::plan_backfill,
            commands::dispatch_backfill,
            commands::list_dead_letters,
            commands::get_dead_letter,
            commands::acknowledge_dead_letter,
            commands::recover_dead_letter,
            commands::get_run_history,
            commands::get_run_log,
            commands::get_run_tasks,
            commands::get_run_attempts,
            commands::get_run_metrics,
            commands::get_run_relationships,
            commands::get_global_run_history,
            commands::cleanup_retention,
            commands::get_workflow_history_buckets,
            commands::get_sla_violations,
            commands::query_resource_samples,
            commands::query_token_usage_rollup,
            commands::query_stale_assets,
            commands::get_mission_control_preferences,
            commands::set_mission_control_preferences,
            commands::get_mission_control_snapshot,
            commands::get_scheduler_status,
            commands::list_queues,
            commands::update_queue,
            commands::list_queued_runs,
            commands::cancel_queued_run,
            commands::list_available_scripts,
            commands::open_dashboard,
            commands::open_run_detail,
            commands::hide_popup,
            commands::open_url,
            commands::quit_app,
            commands::get_launch_at_login,
            commands::set_launch_at_login,
            commands::set_notification_prefs,
            commands::get_notification_prefs,
            commands::analyze_run_error,
            commands::generate_workflow_description,
            commands::get_email_config,
            commands::set_email_config,
            commands::test_email_config,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Ready = event {
                if let Some(tray) = app_handle.tray_by_id(TRAY_ID) {
                    // Reassert tray visibility on Ready for macOS 26+ resilience.
                    let _ = tray.set_visible(false);
                    let _ = tray.set_visible(true);
                    let _ = tray.set_icon_as_template(true);
                    let _ = tray.set_tooltip(Some("Chaos Labs Scheduler"));
                }
            }

            // Dock-click reopen: ensures the popup is reachable even if the
            // menu bar tray icon is hidden by macOS (NSStatusItem registration
            // can be silently dropped by ControlCenter on macOS 26+; the Dock
            // icon is the always-available fallback access path).
            if let tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } = event
            {
                if !has_visible_windows {
                    if let Some(main) = app_handle.get_webview_window("main") {
                        let _ = main.show();
                        let _ = main.set_focus();
                        log::info!("Dock reopen: main dashboard shown");
                    }
                    if let Some(popup) = app_handle.get_webview_window("popup") {
                        let _ = popup.hide();
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_instance_lock_blocks_second_listener_until_first_drops() {
        let first = acquire_single_instance_lock_at("127.0.0.1:0").unwrap();
        let addr = first.local_addr().unwrap();

        assert!(acquire_single_instance_lock_at(&addr.to_string()).is_err());

        drop(first);
        assert!(acquire_single_instance_lock_at(&addr.to_string()).is_ok());
    }
}
