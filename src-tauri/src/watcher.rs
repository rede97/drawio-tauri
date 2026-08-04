//! Polling-based file watching (lightweight, no notify/mio deps).
//!
//! Each editor window registers paths via `watchFile`; a single background
//! thread checks mtimes every 2 seconds and pushes
//! `window.__tauriFileChanged(payload)` into the owning webview on change.
//! Entries whose window was destroyed are dropped automatically.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

struct WatchedFile {
    mtime: Option<u64>,
    window: tauri::WebviewWindow,
}

pub struct FileWatchState {
    watched: Arc<Mutex<HashMap<String, WatchedFile>>>,
}

impl FileWatchState {
    /// Start the polling thread and return the state to be managed by Tauri.
    pub fn start() -> Self {
        let watched: Arc<Mutex<HashMap<String, WatchedFile>>> = Arc::new(Mutex::new(HashMap::new()));
        let watched_poll = watched.clone();

        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(2));
                let paths: Vec<String> = {
                    watched_poll.lock().unwrap().keys().cloned().collect()
                };
                let mut stale = Vec::new();
                for path_str in paths {
                    let path = std::path::PathBuf::from(&path_str);
                    if let Ok(meta) = std::fs::metadata(&path) {
                        let mtime = meta.modified().ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_millis() as u64);
                        let (prev, window) = {
                            match watched_poll.lock().unwrap().get(&path_str) {
                                Some(entry) => (entry.mtime, entry.window.clone()),
                                None => continue,
                            }
                        };
                        if mtime != prev {
                            if let Some(entry) = watched_poll.lock().unwrap().get_mut(&path_str) {
                                entry.mtime = mtime;
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
                            if window.eval(&js).is_err() {
                                // Owning window is gone: drop the watch
                                stale.push(path_str.clone());
                            }
                        }
                    }
                }
                if !stale.is_empty() {
                    let mut watched = watched_poll.lock().unwrap();
                    for path in stale {
                        watched.remove(&path);
                    }
                }
            }
        });

        Self { watched }
    }

    pub fn watch_path(&self, path: String, mtime: Option<u64>, window: tauri::WebviewWindow) {
        self.watched
            .lock()
            .unwrap()
            .insert(path, WatchedFile { mtime, window });
    }

    pub fn unwatch_path(&self, path: &str) {
        self.watched.lock().unwrap().remove(path);
    }
}
