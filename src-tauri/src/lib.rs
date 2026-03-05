use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![read_file_bytes, write_file_bytes, get_app_version])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.set_title("draw.io").unwrap();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running draw.io");
}

/// Read raw bytes from a file path
#[tauri::command]
async fn read_file_bytes(path: String) -> Result<Vec<u8>, String> {
    std::fs::read(&path).map_err(|e| e.to_string())
}

/// Write raw bytes to a file path
#[tauri::command]
async fn write_file_bytes(path: String, contents: Vec<u8>) -> Result<(), String> {
    std::fs::write(&path, &contents).map_err(|e| e.to_string())
}

/// Return the app version
#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
