//! IPC commands bridging the draw.io webapp's Electron API surface to Tauri.
//!
//! The webapp (ElectronApp.js) talks to `window.electron` (injected by
//! `electron_bridge.js`), which forwards to the commands below:
//! - `electron.request()`  → [`electron_request`]
//! - `electron.sendMessage()` → [`electron_message`]

use crate::{close_flow, drafts, fonts, prefs, update, watcher};
use base64::Engine;
use prefs::{PrefField, PrefsState};
use serde_json::Value as JsonValue;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use watcher::FileWatchState;

/// Read raw bytes from a file path
#[tauri::command]
pub async fn read_file_bytes(path: String) -> Result<Vec<u8>, String> {
    fs::read(&path).map_err(|e| e.to_string())
}

/// Write raw bytes to a file path
#[tauri::command]
pub async fn write_file_bytes(path: String, contents: Vec<u8>) -> Result<(), String> {
    fs::write(&path, &contents).map_err(|e| e.to_string())
}

/// Return the app version
#[tauri::command]
pub fn getapp_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ── electron.request() ─────────────────────────────────────────────────────

/// Handle electron.request() calls from the webapp
#[tauri::command]
pub async fn electron_request(
    msg: JsonValue,
    app: tauri::AppHandle,
    state: tauri::State<'_, FileWatchState>,
    prefs_state: tauri::State<'_, PrefsState>,
) -> Result<JsonValue, String> {
    let action = msg["action"].as_str().unwrap_or("");

    match action {
        "getDocumentsFolder" => {
            let docs = dirs::document_dir()
                .unwrap_or_else(|| Path::new(".").to_path_buf());
            Ok(JsonValue::String(docs.to_string_lossy().to_string()))
        }

        "dirname" => {
            let p = msg["path"].as_str().unwrap_or("");
            let parent = Path::new(&p)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            Ok(JsonValue::String(parent))
        }

        "openExternal" => {
            let url = msg["url"].as_str().unwrap_or("");
            tauri_plugin_opener::open_url(url.to_string(), None::<&str>)
                .map_err(|e| e.to_string())?;
            Ok(JsonValue::Null)
        }

        "showSaveDialog" => show_save_dialog(&app, &msg),

        "showOpenDialog" => show_open_dialog(&app, &msg),

        "readFile" => {
            let filename = msg["filename"].as_str().ok_or("missing filename")?;
            let encoding = msg["encoding"].as_str().unwrap_or("utf-8");
            let bytes = fs::read(&filename).map_err(|e| e.to_string())?;
            if encoding == "base64" {
                Ok(JsonValue::String(base64::engine::general_purpose::STANDARD.encode(&bytes)))
            } else {
                Ok(JsonValue::String(String::from_utf8_lossy(&bytes).to_string()))
            }
        }

        "writeFile" => {
            let path = msg["path"].as_str().ok_or("missing path")?;
            let data = msg["data"].as_str().ok_or("missing data")?;
            let enc = msg["enc"].as_str().unwrap_or("utf-8");
            if enc == "base64" {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .map_err(|e| e.to_string())?;
                fs::write(&path, &bytes).map_err(|e| e.to_string())?;
            } else {
                fs::write(&path, data).map_err(|e| e.to_string())?;
            }
            Ok(JsonValue::Null)
        }

        "saveFile" => {
            let file_object = &msg["fileObject"];
            let path = file_object["path"].as_str().ok_or("missing fileObject.path")?;
            let data = msg["data"].as_str().ok_or("missing data")?;
            let def_enc = msg["defEnc"].as_str().unwrap_or("utf-8");

            // Write a .$name.bkp backup of the last good save before
            // overwriting (mirrors drawio-desktop, gated by the storeBkp pref)
            let store_bkp = prefs_state.prefs.lock().unwrap().store_bkp;
            if store_bkp && Path::new(&path).exists() {
                drafts::create_backup(&path);
            }

            if def_enc == "base64" {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .map_err(|e| e.to_string())?;
                fs::write(&path, &bytes).map_err(|e| e.to_string())?;
            } else {
                fs::write(&path, data).map_err(|e| e.to_string())?;
            }

            // Clean up legacy-prefix backup once the new save succeeded
            drafts::cleanup_legacy_backup(&path);

            file_stat_json(&path)
        }

        // Best-effort recovery source: the on-disk .bkp backup written before
        // the last overwrite. Returns {data, created, modified, path} or null.
        "getBkpFile" => {
            let file_object = &msg["fileObject"];
            let path = file_object["path"].as_str().ok_or("missing fileObject.path")?;
            match drafts::read_backup(&path) {
                Some((data, created, modified, bkp)) => Ok(serde_json::json!({
                    "data": data,
                    "created": created,
                    "modified": modified,
                    "path": bkp.to_string_lossy()
                })),
                None => Ok(JsonValue::Null),
            }
        }

        // File → Exit: run the window through the normal close-confirm flow
        "exit" => {
            if let Some(win) = app.get_webview_window("main") {
                win.close().map_err(|e| e.to_string())?;
            }
            Ok(JsonValue::Null)
        }

        "fileStat" => {
            let file = msg["file"].as_str().ok_or("missing file")?;
            file_stat_json(&file)
        }

        "isFileWritable" => {
            let file = msg["file"].as_str().ok_or("missing file")?;
            let meta = fs::metadata(&file).map_err(|e| e.to_string())?;
            Ok(JsonValue::Bool(!meta.permissions().readonly()))
        }

        // Lists sibling draft files (.$name.dtmp, .$name_N.dtmp) for a file,
        // porting legacy ~$-prefixed drafts to the new prefix. Each entry
        // includes the draft content (the webapp shows it in the DraftDialog).
        "getFileDrafts" => {
            let file_object = &msg["fileObject"];
            let file_path = file_object["path"].as_str().unwrap_or("");
            if file_path.is_empty() {
                return Ok(JsonValue::Array(vec![]));
            }

            let mut drafts_list = Vec::new();
            for path in drafts::collect_drafts(&file_path) {
                if let Ok(meta) = fs::metadata(&path) {
                    if meta.is_file() {
                        drafts_list.push(serde_json::json!({
                            "name": path.file_name().unwrap_or_default().to_string_lossy(),
                            "path": path.to_string_lossy(),
                            "data": fs::read_to_string(&path).unwrap_or_default(),
                            "size": meta.len(),
                            "mtime": meta.modified().ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_millis() as u64)
                                .unwrap_or(0)
                        }));
                    }
                }
            }
            Ok(JsonValue::Array(drafts_list))
        }

        // Writes a draft next to the original file as .$name[.dtmp|_N.dtmp]
        // and returns the draft path (stored in fileObject.draftFileName).
        "saveDraft" => {
            let file_object = &msg["fileObject"];
            let file_path = file_object["path"].as_str()
                .filter(|p| !p.is_empty())
                .ok_or("missing fileObject.path")?;
            let data = msg["data"].as_str().ok_or("missing data")?;
            let preferred = file_object["draftFileName"].as_str();

            let draft_path = drafts::write_draft(&file_path, preferred, data)
                .map_err(|e| e.to_string())?;
            Ok(JsonValue::String(draft_path.to_string_lossy().to_string()))
        }

        "deleteFile" => {
            let path = msg["path"].as_str().or(msg["file"].as_str()).ok_or("missing path")?;
            if Path::new(&path).exists() {
                fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
            Ok(JsonValue::Null)
        }

        "isPluginsEnabled" => Ok(JsonValue::Bool(true)),

        "watchFile" => {
            let path = msg["path"].as_str().ok_or("missing path")?.to_string();
            let path_buf = std::path::PathBuf::from(&path);
            if path_buf.exists() {
                if let Ok(meta) = std::fs::metadata(&path_buf) {
                    let mtime = meta.modified().ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as u64);
                    state.watch_path(path, mtime);
                }
            }
            Ok(JsonValue::Null)
        }

        "unwatchFile" => {
            let path = msg["path"].as_str().unwrap_or("");
            state.unwatch_path(path);
            Ok(JsonValue::Null)
        }

        "getPluginFile" | "installPlugin" | "uninstallPlugin" => {
            Err("Plugin management not yet implemented in Tauri".to_string())
        }

        "getLocalFonts" => {
            let font_list: Vec<JsonValue> = fonts::get_local_fonts()
                .into_iter()
                .map(JsonValue::String)
                .collect();
            Ok(JsonValue::Array(font_list))
        }

        // Joins path parts and reports existence (mirrors drawio-desktop)
        "checkFileExists" => {
            let parts: Vec<String> = msg["pathParts"].as_array()
                .map(|a| a.iter().filter_map(|p| p.as_str().map(String::from)).collect())
                .ok_or("missing pathParts")?;
            let path: PathBuf = parts.iter().collect();
            Ok(serde_json::json!({
                "exists": path.exists(),
                "path": path.to_string_lossy()
            }))
        }

        // Window controls (mirrors drawio-desktop windowAction). 'close'
        // goes through the normal close-confirm handshake.
        "windowAction" => {
            let method = msg["method"].as_str().unwrap_or("");
            if let Some(win) = app.get_webview_window("main") {
                match method {
                    "minimize" => win.minimize().map_err(|e| e.to_string())?,
                    "maximize" => win.maximize().map_err(|e| e.to_string())?,
                    "unmaximize" => win.unmaximize().map_err(|e| e.to_string())?,
                    "close" => win.close().map_err(|e| e.to_string())?,
                    "isMaximized" => {
                        return Ok(JsonValue::Bool(win.is_maximized().unwrap_or(false)));
                    }
                    // removeAllListeners and unknown methods: no-op
                    _ => {}
                }
            }
            Ok(JsonValue::Null)
        }

        "isFullscreen" => {
            let fs = app.get_webview_window("main")
                .map(|w| w.is_fullscreen().unwrap_or(false))
                .unwrap_or(false);
            Ok(JsonValue::Bool(fs))
        }

        _ => Err(format!("Unknown electron action: {}", action)),
    }
}

// ── electron.sendMessage() ─────────────────────────────────────────────────

/// Handle electron.sendMessage() calls from the webapp
#[tauri::command]
pub async fn electron_message(
    channel: String,
    data: JsonValue,
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    // Close-confirm handshake channels are handled by the close_flow module
    if close_flow::handle_message(&channel, &data, &app, &window) {
        return Ok(());
    }

    match channel.as_str() {
        "toggleFullscreen" => {
            let is_fs = window.is_fullscreen().unwrap_or(false);
            window.set_fullscreen(!is_fs).map_err(|e| e.to_string())?;
        }

        "openDevTools" => {
            #[cfg(debug_assertions)]
            window.open_devtools();
        }

        "zoomIn" => {
            window.eval("document.body.style.zoom = (parseFloat(document.body.style.zoom || 1) + 0.1).toFixed(1)").ok();
        }

        "zoomOut" => {
            window.eval("document.body.style.zoom = Math.max(0.3, (parseFloat(document.body.style.zoom || 1) - 0.1)).toFixed(1)").ok();
        }

        "resetZoom" => {
            window.eval("document.body.style.zoom = 1").ok();
        }

        "app-load-finished" => {
            let args: Vec<String> = std::env::args().skip(1).collect();
            let cwd = std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
            let payload = serde_json::json!({ "args": args, "cwd": cwd });
            let js = format!("window.__tauriArgsObj({})", payload);
            window.eval(&js).ok();
            // Override Help → Website link to fork homepage
            window.eval("try{var a=editorUi&&editorUi.actions;if(a){a.addAction('website...',function(){editorUi.openLink('https://github.com/rede97/drawio-tauri')})}}catch(e){}").ok();
        }

        "export" => {
            let payload = serde_json::json!({
                "message": "Export via Tauri bridge not yet implemented. Use browser export."
            });
            let js = format!("window.__tauriExportError({})", payload);
            window.eval(&js).ok();
        }

        "checkForUpdates" => {
            update::check_manual(app.clone()).await;
        }

        "toggleGoogleFonts" => app.state::<PrefsState>().toggle(PrefField::GoogleFonts),

        "toggleSpellCheck" => app.state::<PrefsState>().toggle(PrefField::SpellCheck),

        "toggleStoreBkp" => app.state::<PrefsState>().toggle(PrefField::StoreBkp),

        "newfile" => {
            window.eval("location.reload()").ok();
        }

        _ => {}
    }

    Ok(())
}

// ── Dialog helpers ─────────────────────────────────────────────────────────

fn apply_filters(
    builder: tauri_plugin_dialog::FileDialogBuilder<tauri::Wry>,
    msg: &JsonValue,
) -> tauri_plugin_dialog::FileDialogBuilder<tauri::Wry> {
    let mut builder = builder;
    if let Some(filters) = msg["filters"].as_array() {
        for filter in filters {
            let name = filter["name"].as_str().unwrap_or("");
            let exts: Vec<String> = filter["extensions"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.replace("*.", ""))).collect())
                .unwrap_or_default();
            let ext_refs: Vec<&str> = exts.iter().map(|s| s.as_str()).collect();
            builder = builder.add_filter(name, &ext_refs);
        }
    }
    builder
}

fn show_save_dialog(app: &tauri::AppHandle, msg: &JsonValue) -> Result<JsonValue, String> {
    let mut builder = app.dialog().file();
    builder = builder.set_parent(&app.get_webview_window("main").unwrap());
    if let Some(title) = msg["title"].as_str() {
        builder = builder.set_title(title);
    }
    builder = apply_filters(builder, msg);
    if let Some(default_path) = msg["defaultPath"].as_str() {
        let p = PathBuf::from(default_path);
        if let Some(parent) = p.parent() {
            builder = builder.set_directory(parent);
        }
        builder = builder.set_file_name(
            p.file_name().unwrap_or_default().to_string_lossy().to_string()
        );
    }
    match builder.blocking_save_file() {
        Some(file_path) => {
            let path = file_path.into_path().map_err(|e| e.to_string())?;
            Ok(JsonValue::String(path.to_string_lossy().to_string()))
        }
        None => Ok(JsonValue::Null),
    }
}

fn show_open_dialog(app: &tauri::AppHandle, msg: &JsonValue) -> Result<JsonValue, String> {
    let mut builder = app.dialog().file();
    builder = builder.set_parent(&app.get_webview_window("main").unwrap());
    if let Some(title) = msg["title"].as_str() {
        builder = builder.set_title(title);
    }
    builder = apply_filters(builder, msg);
    if let Some(default_path) = msg["defaultPath"].as_str() {
        let p = PathBuf::from(default_path);
        let dir = if p.is_dir() { p } else {
            p.parent().map(|d| d.to_path_buf()).unwrap_or_default()
        };
        builder = builder.set_directory(dir);
    }
    let multiple = msg["multiple"].as_bool().unwrap_or(false);
    if multiple {
        match builder.blocking_pick_files() {
            Some(files) => {
                let paths: Vec<JsonValue> = files.into_iter().filter_map(|f| {
                    f.into_path().ok().map(|p| JsonValue::String(p.to_string_lossy().to_string()))
                }).collect();
                Ok(JsonValue::Array(paths))
            }
            None => Ok(JsonValue::Null),
        }
    } else {
        match builder.blocking_pick_file() {
            Some(file_path) => {
                let path = file_path.into_path().map_err(|e| e.to_string())?;
                // Electron returns filePaths array → ElectronApp.js expects array
                Ok(JsonValue::Array(vec![JsonValue::String(path.to_string_lossy().to_string())]))
            }
            None => Ok(JsonValue::Null),
        }
    }
}

/// File stat JSON matching Electron's shape
fn file_stat_json(path: &str) -> Result<JsonValue, String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let to_ms = |t: std::io::Result<std::time::SystemTime>| {
        t.ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    };
    Ok(serde_json::json!({
        "size": meta.len(),
        "mtime": to_ms(meta.modified()),
        "birthtime": to_ms(meta.created()),
        "isFile": meta.is_file(),
        "isDirectory": meta.is_dir()
    }))
}
