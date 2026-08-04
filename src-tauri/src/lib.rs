//! draw.io desktop (Tauri) — application assembly.
//!
//! The implementation is split into focused modules:
//! - [`prefs`]       — persisted user preferences (prefs.json)
//! - [`fonts`]       — system font enumeration
//! - [`drafts`]      — draft/backup file conventions (.$name.dtmp / .$name.bkp)
//! - [`watcher`]     — polling-based file watching
//! - [`close_flow`]  — close-confirm handshake with the webapp
//! - [`update`]      — auto-update checks (tauri-plugin-updater)
//! - [`windows`]     — editor window creation and bookkeeping (multi-window)
//! - [`export`]      — local export pipeline (hidden export3.html renderer)
//! - [`ipc`]         — Electron bridge IPC commands

mod close_flow;
mod drafts;
mod export;
mod fonts;
mod ipc;
mod prefs;
mod update;
mod watcher;
mod windows;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let bridge_js = include_str!("electron_bridge.js")
        .replace("__DRAWIO_VERSION__", env!("CARGO_PKG_VERSION"));
    let prefs_data = prefs::load_prefs();
    let editor_url = windows::build_editor_url(&prefs_data);

    tauri::Builder::default()
        // Single instance: a second launch (CLI / file association) forwards
        // its file arg to a new window in the running instance, mirroring
        // drawio-desktop's second-instance handling
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            let file = argv.iter().skip(1).rev().find(|a| !a.starts_with('-')).and_then(|a| {
                let p = std::path::PathBuf::from(a);
                let p = if p.is_absolute() { p } else { std::path::PathBuf::from(&cwd).join(p) };
                p.is_file().then(|| p.to_string_lossy().to_string())
            });
            if let Some(file) = file {
                app.state::<windows::WindowsState>().push_pending_args(vec![file]);
            }
            windows::create_editor_window(app, None, None).ok();
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .manage(windows::WindowsState::new(bridge_js, editor_url))
        .manage(export::ExportState::new())
        .manage(close_flow::CloseState::new())
        .manage(prefs::PrefsState::new(prefs_data))
        .manage(watcher::FileWatchState::start())
        .invoke_handler(tauri::generate_handler![
            ipc::read_file_bytes,
            ipc::write_file_bytes,
            ipc::getapp_version,
            ipc::electron_request,
            ipc::electron_message
        ])
        .setup(|app| {
            // First editor window (label "main"); receives command-line args
            // when no second-instance args were queued for it
            windows::create_editor_window(app.handle(), Some("main".to_string()), None)?;

            // Silent update check shortly after startup (disableUpdate=0)
            update::spawn_background_check(app.handle());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running draw.io");
}
