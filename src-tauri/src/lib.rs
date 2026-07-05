mod actions;
mod api;
mod branding;
mod commands;
mod db;
mod email;
mod operators;
mod scheduler;
mod service;
mod steps;
mod workflow_spec;

use commands::AppState;
use db::Database;
use scheduler::{start_scheduler_loop, WorkflowScheduler};
use service::{Notifier, SchedulerService};

/// Bridges the GUI-agnostic [`Notifier`] trait to Tauri's notification plugin.
pub struct DesktopNotifier {
    app: tauri::AppHandle,
}

impl Notifier for DesktopNotifier {
    fn notify(&self, title: &str, body: &str) {
        use tauri_plugin_notification::NotificationExt;
        if let Err(e) = self
            .app
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
        {
            log::warn!("Failed to send desktop notification: {e}");
        }
    }
}
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

use branding::{SINGLE_INSTANCE_ADDR, TRAY_ID};

fn acquire_single_instance_lock() -> std::io::Result<TcpListener> {
    acquire_single_instance_lock_at(SINGLE_INSTANCE_ADDR)
}

fn acquire_single_instance_lock_at(addr: &str) -> std::io::Result<TcpListener> {
    TcpListener::bind(addr)
}

/// Resolve the workspace root the scheduler runs workflows against. Honors
/// `CHAOS_SCHEDULER_WORKSPACE_ROOT` (and legacy `CHAOS_LABS_ROOT`) and otherwise
/// defaults to the app data dir — the standalone default, no longer coupled to
/// the chaos-labs repo.
fn detect_workspace_root(app_data_dir: &Path) -> String {
    branding::detect_workspace_root(&app_data_dir.to_string_lossy())
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
        .join(branding::LEGACY_BUNDLE_ID)
        .join("scheduler.db");

    // Never relocate a legacy DB into itself (guards the case where the bundle
    // id has not actually changed yet in a dev build).
    if new_db == legacy_db {
        return;
    }

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
        // Also clear any stale WAL/-shm sidecars so the imported copy is used.
        let _ = std::fs::remove_file(new_db.with_extension("db-wal"));
        let _ = std::fs::remove_file(new_db.with_extension("db-shm"));

        // Use a consistent SQLite snapshot (VACUUM INTO) so any committed-but-
        // uncheckpointed WAL contents in the legacy DB are preserved, rather
        // than a raw file copy that could miss WAL data.
        let migrated = rusqlite::Connection::open(&legacy_db)
            .and_then(|conn| {
                conn.execute(
                    "VACUUM INTO ?1",
                    rusqlite::params![new_db.to_string_lossy().to_string()],
                )
            })
            .is_ok();

        if migrated {
            log::info!("Migrated legacy scheduler database into new app data dir");
        } else if std::fs::copy(&legacy_db, &new_db).is_ok() {
            log::info!("Migrated legacy scheduler database (raw copy fallback)");
        } else {
            log::warn!("Failed to migrate legacy scheduler database");
        }
    }
}

fn start_metrics_endpoint(db: Arc<Database>) {
    std::thread::spawn(move || {
        let Ok(listener) = TcpListener::bind(branding::METRICS_ADDR) else {
            log::warn!(
                "Failed to bind Scheduler metrics endpoint on {}",
                branding::METRICS_ADDR
            );
            return;
        };
        log::info!(
            "Scheduler metrics endpoint listening on {}",
            branding::METRICS_ADDR
        );
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
        .plugin(tauri_plugin_updater::Builder::new().build())
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
                        "Another Chaos Scheduler instance is already active; exiting before startup ({err})"
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

            let chaos_labs_root = detect_workspace_root(&app_data_dir);
            let python_path = detect_python_path(&chaos_labs_root);

            let db = Arc::new(Database::new(&app_data_dir));
            let scheduler = Arc::new(Mutex::new(WorkflowScheduler::new(db.clone())));
            start_metrics_endpoint(db.clone());

            let notifier: Arc<dyn Notifier> = Arc::new(DesktopNotifier {
                app: app.handle().clone(),
            });
            let service = SchedulerService::new(db.clone(), notifier);

            // Embedded HTTP API (loopback by default; configurable bind).
            let api_addr = std::env::var("CHAOS_SCHEDULER_API_ADDR")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| branding::DEFAULT_API_ADDR.to_string());
            let api_state = api::ApiState {
                service: service.clone(),
                db: db.clone(),
                workspace_root: chaos_labs_root.clone(),
                python_path: python_path.clone(),
                rate: Arc::new(Mutex::new(api::RateLimiter::new(
                    120,
                    std::time::Duration::from_secs(60),
                ))),
                preauth_rate: Arc::new(Mutex::new(api::RateLimiter::new(
                    120,
                    std::time::Duration::from_secs(60),
                ))),
                host_allowlist: db
                    .get_scheduler_config("api_host_allowlist")
                    .ok()
                    .flatten()
                    .map(|s| {
                        s.split(',')
                            .map(|h| h.trim().to_string())
                            .filter(|h| !h.is_empty())
                            .collect()
                    })
                    .unwrap_or_default(),
                cors_allowlist: db
                    .get_scheduler_config("api_cors_allowlist")
                    .ok()
                    .flatten()
                    .map(|s| {
                        s.split(',')
                            .map(|o| o.trim().to_string())
                            .filter(|o| !o.is_empty())
                            .collect()
                    })
                    .unwrap_or_default(),
            };
            api::start_api_server(api_state, api_addr);

            app.manage(AppState {
                db: db.clone(),
                scheduler: scheduler.clone(),
                service,
                workspace_root: chaos_labs_root.clone(),
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
            .title(branding::POPUP_TITLE)
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
                .tooltip(branding::TRAY_TOOLTIP)
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

            log::info!("Chaos Scheduler started, tray icon created");

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
            commands::list_environments,
            commands::create_environment,
            commands::update_environment,
            commands::delete_environment,
            commands::set_workflow_spec,
            commands::create_api_key,
            commands::list_api_keys,
            commands::revoke_api_key,
            commands::check_for_update,
            commands::apply_update,
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
                    let _ = tray.set_tooltip(Some(branding::TRAY_TOOLTIP));
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
