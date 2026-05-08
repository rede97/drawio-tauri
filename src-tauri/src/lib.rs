use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use std::path::PathBuf;
use serde_json::Value as JsonValue;
use std::fs;
use std::path::Path;
use base64::Engine;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct Prefs {
    #[serde(rename = "googleFonts")]
    is_google_fonts_enabled: bool,
    #[serde(rename = "spellCheck")]
    enable_spell_check: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            is_google_fonts_enabled: false,
            enable_spell_check: false,
        }
    }
}

struct PrefsState {
    prefs: Mutex<Prefs>,
}

impl PrefsState {
    fn new(prefs: Prefs) -> Self {
        Self { prefs: Mutex::new(prefs) }
    }
}

fn prefs_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| Path::new(".").to_path_buf())
        .join("drawio")
        .join("prefs.json")
}

fn load_prefs() -> Prefs {
    let path = prefs_path();
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_prefs(prefs: &Prefs) {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string_pretty(prefs) {
        fs::write(&path, json).ok();
    }
}

fn get_local_fonts() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(out) = std::process::Command::new("reg")
            .args(["query", r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts"])
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut fonts: Vec<String> = text.lines()
                .filter_map(|line| {
                    if !line.contains("REG_SZ") { return None; }
                    let name = line.split("REG_SZ").next()?.trim().to_string();
                    if name.is_empty() || name.starts_with("HKEY_") { return None; }
                    Some(name
                        .replace(" (TrueType)", "")
                        .replace(" (OpenType)", "")
                        .trim()
                        .to_string())
                })
                .filter(|s| !s.is_empty())
                .collect();
            fonts.sort();
            fonts.dedup();
            return fonts;
        }
        Vec::new()
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(out) = std::process::Command::new("fc-list")
            .args([":", "family"])
            .output()
        {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                let mut fonts: Vec<String> = text.lines()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                fonts.sort();
                fonts.dedup();
                return fonts;
            }
        }

        #[cfg(target_os = "macos")]
        {
            let mut fonts = Vec::new();
            for dir in &["/System/Library/Fonts", "/Library/Fonts"] {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if let Some(dot) = name.rfind('.') {
                            fonts.push(name[..dot].to_string());
                        }
                    }
                }
            }
            fonts.sort();
            fonts.dedup();
            fonts
        }

        #[cfg(not(target_os = "macos"))]
        {
            Vec::new()
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let bridge_js = include_str!("electron_bridge.js");
    let prefs = load_prefs();
    let gf = if prefs.is_google_fonts_enabled { "1" } else { "0" };
    let sc = if prefs.enable_spell_check { "1" } else { "0" };
    let url = format!("index.html?dev=0&test=0&gapi=0&db=0&od=0&gh=0&gl=0&tr=0&browser=0&picker=0&mode=device&export=https://convert.diagrams.net/node/export&disableUpdate=0&enableSpellCheck={sc}&enableStoreBkp=1&isGoogleFontsEnabled={gf}");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            read_file_bytes,
            write_file_bytes,
            getapp_version,
            electron_request,
            electron_message
        ])
        .setup(move |app| {
            app.manage(CloseState::new());
            app.manage(PrefsState::new(prefs.clone()));

            let window = tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App(url.clone().into()))
                .title("draw.io")
                .inner_size(1280.0, 800.0)
                .min_inner_size(800.0, 600.0)
                .resizable(true)
                .fullscreen(false)
                .initialization_script(bridge_js)
                .build()?;

            // File watching via polling (lightweight, no notify/mio deps)
            let watched: Arc<Mutex<HashMap<String, Option<u64>>>> = Arc::new(Mutex::new(HashMap::new()));
            let watched_poll = watched.clone();
            let win_poll = window.clone();

            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let paths: Vec<String> = {
                        watched_poll.lock().unwrap().keys().cloned().collect()
                    };
                    for path_str in paths {
                        let path = std::path::PathBuf::from(&path_str);
                        if let Ok(meta) = std::fs::metadata(&path) {
                            let mtime = meta.modified().ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_millis() as u64);
                            let prev = {
                                watched_poll.lock().unwrap().get(&path_str).copied().flatten()
                            };
                            if mtime != prev {
                                if let Some(entry) = watched_poll.lock().unwrap().get_mut(&path_str) {
                                    *entry = mtime;
                                }
                                let curr_obj = serde_json::json!({
                                    "size": meta.len(),
                                    "mtime": mtime.unwrap_or(0),
                                });
                                let prev_obj = prev.map(|p| serde_json::json!({"size": 0, "mtime": p}))
                                    .unwrap_or(serde_json::json!(null));
                                let payload = serde_json::json!({
                                    "path": path_str,
                                    "curr": curr_obj,
                                    "prev": prev_obj
                                });
                                let js = format!("window.__tauriFileChanged({})", payload);
                                win_poll.eval(&js).ok();
                            }
                        }
                    }
                }
            });

            let watch_state = FileWatchState::new(watched);
            app.manage(watch_state);

            // Close-requested handler: check for unsaved changes
            let win_close = window.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    let state = win_close.app_handle().state::<CloseState>();
                    {
                        let mut confirmed = state.confirmed.lock().unwrap();
                        if *confirmed {
                            *confirmed = false;
                            return; // allow close
                        }
                    }
                    api.prevent_close();
                    win_close.eval("window.__tauriCloseCheck()").ok();
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running draw.io");
}

/// Read raw bytes from a file path
#[tauri::command]
async fn read_file_bytes(path: String) -> Result<Vec<u8>, String> {
    fs::read(&path).map_err(|e| e.to_string())
}

/// Write raw bytes to a file path
#[tauri::command]
async fn write_file_bytes(path: String, contents: Vec<u8>) -> Result<(), String> {
    fs::write(&path, &contents).map_err(|e| e.to_string())
}

/// Return the app version
#[tauri::command]
fn getapp_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ── Electron bridge IPC commands ──────────────────────────────────────────

/// Handle electron.request() calls from the webapp
#[tauri::command]
async fn electron_request(
    msg: JsonValue,
    app: tauri::AppHandle,
    state: tauri::State<'_, FileWatchState>,
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

        "showSaveDialog" => {
            let mut builder = app.dialog().file();
            builder = builder.set_parent(&app.get_webview_window("main").unwrap());
            if let Some(title) = msg["title"].as_str() {
                builder = builder.set_title(title);
            }
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
            if let Some(default_path) = msg["defaultPath"].as_str() {
                let p = PathBuf::from(default_path);
                if let Some(parent) = p.parent() {
                    builder = builder.set_directory(parent);
                }
                builder = builder.set_file_name(
                    p.file_name().unwrap_or_default().to_string_lossy().to_string()
                );
            }
            let result = builder.blocking_save_file();
	match result {
                Some(file_path) => {
                    let path = file_path.into_path().map_err(|e| e.to_string())?;
                    Ok(JsonValue::String(path.to_string_lossy().to_string()))
                }
                None => Ok(JsonValue::Null)
            }
        }

        "showOpenDialog" => {
            let mut builder = app.dialog().file();
            builder = builder.set_parent(&app.get_webview_window("main").unwrap());
            if let Some(title) = msg["title"].as_str() {
                builder = builder.set_title(title);
            }
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
            if let Some(default_path) = msg["defaultPath"].as_str() {
                let p = PathBuf::from(default_path);
                let dir = if p.is_dir() { p } else {
                    p.parent().map(|d| d.to_path_buf()).unwrap_or_default()
                };
                builder = builder.set_directory(dir);
            }
            let multiple = msg["multiple"].as_bool().unwrap_or(false);
            if multiple {
                let result = builder.blocking_pick_files();
	match result {
                    Some(files) => {
                        let paths: Vec<JsonValue> = files.into_iter().filter_map(|f| {
                            f.into_path().ok().map(|p| JsonValue::String(p.to_string_lossy().to_string()))
                        }).collect();
                        Ok(JsonValue::Array(paths))
                    }
                    None => Ok(JsonValue::Null)
                }
            } else {
                let result = builder.blocking_pick_file();
	match result {
                    Some(file_path) => {
                        let path = file_path.into_path().map_err(|e| e.to_string())?;
                        // Electron returns filePaths array → ElectronApp.js expects array
                        Ok(JsonValue::Array(vec![JsonValue::String(path.to_string_lossy().to_string())]))
                    }
                    None => Ok(JsonValue::Null)
                }
            }
        }

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
	if def_enc == "base64" {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .map_err(|e| e.to_string())?;
                fs::write(&path, &bytes).map_err(|e| e.to_string())?;
            } else {
                fs::write(&path, data).map_err(|e| e.to_string())?;
            }
            // Return file stat (matching Electron's behavior)
            let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
            Ok(serde_json::json!({
                "size": meta.len(),
                "mtime": meta.modified().ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                "birthtime": meta.created().ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                "isFile": meta.is_file(),
                "isDirectory": meta.is_dir()
            }))
        }

        "fileStat" => {
            let file = msg["file"].as_str().ok_or("missing file")?;
            let meta = fs::metadata(&file).map_err(|e| e.to_string())?;
            Ok(serde_json::json!({
                "size": meta.len(),
                "mtime": meta.modified().ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                "birthtime": meta.created().ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                "isFile": meta.is_file(),
                "isDirectory": meta.is_dir()
            }))
        }

        "isFileWritable" => {
            let file = msg["file"].as_str().ok_or("missing file")?;
            let meta = fs::metadata(&file).map_err(|e| e.to_string())?;
            Ok(JsonValue::Bool(!meta.permissions().readonly()))
        }

        "getFileDrafts" => {
            let file_path = msg["filePath"].as_str().unwrap_or("");
            let drafts_dir = draft_dir(&file_path);
            let mut drafts = Vec::new();
            if let Ok(entries) = fs::read_dir(&drafts_dir) {
                for entry in entries.flatten() {
                    if let Ok(meta) = entry.metadata() {
                        if meta.is_file() {
                            drafts.push(serde_json::json!({
                                "name": entry.file_name().to_string_lossy(),
                                "path": entry.path().to_string_lossy(),
                                "size": meta.len(),
                                "mtime": meta.modified().ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_millis() as u64)
                                    .unwrap_or(0)
                            }));
                        }
                    }
                }
            }
            Ok(JsonValue::Array(drafts))
        }

        "saveDraft" => {
            let file_path = msg["filePath"].as_str().unwrap_or("untitled");
            let data = msg["data"].as_str().unwrap_or("");
            let drafts_dir = draft_dir(&file_path);
            fs::create_dir_all(&drafts_dir).map_err(|e| e.to_string())?;
            let draft_name = msg.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("draft.drawio");
            let draft_path = drafts_dir.join(draft_name);
            fs::write(&draft_path, data).map_err(|e| e.to_string())?;
            Ok(JsonValue::Null)
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
            let fonts: Vec<JsonValue> = get_local_fonts()
                .into_iter()
                .map(JsonValue::String)
                .collect();
            Ok(JsonValue::Array(fonts))
        }

        _ => Err(format!("Unknown electron action: {}", action)),
    }
}

/// Handle electron.sendMessage() calls from the webapp
#[tauri::command]
async fn electron_message(
    channel: String,
    _data: JsonValue,
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
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
            let current = env!("CARGO_PKG_VERSION");
            let resp = ureq::get("https://api.github.com/repos/rede97/drawio-tauri/releases/latest")
                .set("User-Agent", "drawio-tauri")
                .set("Accept", "application/vnd.github+json")
                .call();
            match resp {
                Ok(r) => {
                    if let Ok(json) = r.into_json::<serde_json::Value>() {
                        let tag = json["tag_name"].as_str().unwrap_or("");
                        let latest = tag.trim_start_matches('v');
                        let html_url = json["html_url"].as_str().unwrap_or(
                            "https://github.com/rede97/drawio-tauri/releases"
                        );
                        if is_newer_version(latest, current) {
                            let msg = format!(
                                "A new version {} is available.\nCurrent version: {}\n\n{}",
                                latest, current, html_url
                            );
                            let clicked = app.dialog()
                                .message(msg)
                                .title("Update Available")
                                .kind(tauri_plugin_dialog::MessageDialogKind::Info)
                                .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                                    "Download".into(), "Later".into()))
                                .blocking_show();
                            if clicked {
                                tauri_plugin_opener::open_url(html_url.to_string(), None::<&str>).ok();
                            }
                        } else {
                            app.dialog()
                                .message("You are running the latest version.")
                                .title("No Updates")
                                .kind(tauri_plugin_dialog::MessageDialogKind::Info)
                                .blocking_show();
                        }
                    }
                }
                Err(_) => {
                    app.dialog()
                        .message("Failed to check for updates. Please try again later.")
                        .title("Update Check Failed")
                        .kind(tauri_plugin_dialog::MessageDialogKind::Error)
                        .blocking_show();
                }
            }
        }

        "toggleGoogleFonts" => {
            let state = app.state::<PrefsState>();
            let mut prefs = state.prefs.lock().unwrap();
            prefs.is_google_fonts_enabled = !prefs.is_google_fonts_enabled;
            save_prefs(&prefs);
        }

        "toggleSpellCheck" => {
            let state = app.state::<PrefsState>();
            let mut prefs = state.prefs.lock().unwrap();
            prefs.enable_spell_check = !prefs.enable_spell_check;
            save_prefs(&prefs);
        }

        "toggleStoreBkp" => {}

        "newfile" => {
            window.eval("location.reload()").ok();
        }

        "isModified-result" => {
            let data = &_data;
            if data["isModified"].as_bool().unwrap_or(false) {
                let result = app.dialog()
                    .message("You have unsaved changes. Discard them?")
                    .title("draw.io")
                    .kind(tauri_plugin_dialog::MessageDialogKind::Warning)
                    .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                        "Discard Changes".into(), "Cancel".into()))
                    .blocking_show();
                if result {
                    // User clicked "Discard Changes" → close
                    {
                        let state = app.state::<CloseState>();
                        *state.confirmed.lock().unwrap() = true;
                    }
                    window.close().ok();
                }
                // Cancel → do nothing, let user continue editing
            } else {
                {
                    let state = app.state::<CloseState>();
                    *state.confirmed.lock().unwrap() = true;
                }
                window.close().ok();
            }
        }

        "save-complete" => {
            {
                let state = app.state::<CloseState>();
                *state.confirmed.lock().unwrap() = true;
            }
            window.close().ok();
        }

        "draftRemoved" => {}

        _ => {}
    }

    Ok(())
}

fn draft_dir(file_path: &str) -> std::path::PathBuf {
    let base = dirs::data_local_dir()
        .unwrap_or_else(|| Path::new(".").to_path_buf())
        .join("drawio")
        .join("drafts");
    let hash = format!("{:x}", md5_like_hash(file_path));
    base.join(hash)
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.').filter_map(|s| s.parse::<u32>().ok()).collect()
    };
    let a = parse(latest);
    let b = parse(current);
    if a.is_empty() || b.is_empty() {
        return latest != current;
    }
    a > b
}

fn md5_like_hash(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

// ── Close-confirmation state ───────────────────────────────────────────────

struct CloseState {
    confirmed: Mutex<bool>,
}

impl CloseState {
    fn new() -> Self {
        Self { confirmed: Mutex::new(false) }
    }
}

// ── File watching state ────────────────────────────────────────────────────

struct FileWatchState {
    watched: Arc<Mutex<HashMap<String, Option<u64>>>>,
}

impl FileWatchState {
    fn new(watched: Arc<Mutex<HashMap<String, Option<u64>>>>) -> Self {
        Self { watched }
    }

    fn watch_path(&self, path: String, mtime: Option<u64>) {
        self.watched.lock().unwrap().insert(path, mtime);
    }

    fn unwatch_path(&self, path: &str) {
        self.watched.lock().unwrap().remove(path);
    }
}
