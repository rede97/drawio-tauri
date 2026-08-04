//! draw.io desktop (Tauri) — application assembly.
//!
//! The implementation is split into focused modules:
//! - [`prefs`]       — persisted user preferences (prefs.json)
//! - [`fonts`]       — system font enumeration
//! - [`drafts`]      — draft/backup file conventions (.$name.dtmp / .$name.bkp)
//! - [`watcher`]     — polling-based file watching
//! - [`close_flow`]  — close-confirm handshake with the webapp
//! - [`update`]      — auto-update checks (tauri-plugin-updater)
//! - [`ipc`]         — Electron bridge IPC commands

mod close_flow;
mod drafts;
mod fonts;
mod ipc;
mod prefs;
mod update;
mod watcher;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let bridge_js = include_str!("electron_bridge.js")
        .replace("__DRAWIO_VERSION__", env!("CARGO_PKG_VERSION"));
    let prefs_data = prefs::load_prefs();
    let gf = if prefs_data.is_google_fonts_enabled { "1" } else { "0" };
    let sc = if prefs_data.enable_spell_check { "1" } else { "0" };
    let sb = if prefs_data.store_bkp { "1" } else { "0" };
    let url = format!("index.html?dev=0&test=0&gapi=0&db=0&od=0&gh=0&gl=0&tr=0&browser=0&picker=0&mode=device&export=https://convert.diagrams.net/node/export&disableUpdate=0&enableSpellCheck={sc}&enableStoreBkp={sb}&isGoogleFontsEnabled={gf}");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            ipc::read_file_bytes,
            ipc::write_file_bytes,
            ipc::getapp_version,
            ipc::electron_request,
            ipc::electron_message
        ])
        .setup(move |app| {
            app.manage(close_flow::CloseState::new());
            app.manage(prefs::PrefsState::new(prefs_data.clone()));

            let window = tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App(url.clone().into()))
                .title("draw.io")
                .inner_size(1280.0, 800.0)
                .min_inner_size(800.0, 600.0)
                .resizable(true)
                .fullscreen(false)
                .initialization_script(bridge_js)
                .build()?;

            // File watching (polling thread) and the close-confirm handshake
            app.manage(watcher::FileWatchState::start(window.clone()));
            close_flow::register_close_handler(&window);

            // Silent update check shortly after startup (disableUpdate=0)
            update::spawn_background_check(app.handle());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running draw.io");
}
