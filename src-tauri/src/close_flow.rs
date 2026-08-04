//! Close-confirm handshake, mirroring drawio-desktop's close flow:
//!
//! ```text
//! window close / File → Exit
//!   → Rust: prevent_close, generate unique id, eval __tauriCloseCheck(id)
//!   → webapp 'isModified' listener replies sendMessage('isModified-result', {uniqueId, isModified, draftPath})
//!   → Rust validates uniqueId against the pending close id:
//!       not modified → destroy window
//!       modified → 3-button dialog (Save / Discard Changes / Cancel):
//!         Save    → eval __tauriSaveAndClose(id) → webapp saves
//!                   → sendMessage('saveAndClose-result', {uniqueId, success})
//!                   → success: destroy; failure/cancel: stay open
//!         Discard → draftPath != null ? delete draft file directly → destroy
//!                                     : eval __tauriRemoveDraft() → webapp removes draft
//!                                     → sendMessage('draftRemoved') → destroy
//!         Cancel  → abort close, clear pending id
//! ```

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

pub struct CloseState {
    /// window label → unique id of the in-flight close handshake
    /// (absent = no close pending for that window)
    pending: Mutex<HashMap<String, String>>,
}

impl CloseState {
    pub fn new() -> Self {
        Self { pending: Mutex::new(HashMap::new()) }
    }

    /// Take the pending id of `label` only if it matches `id`
    fn take_if_matches(&self, label: &str, id: &str) -> bool {
        let mut pending = self.pending.lock().unwrap();
        if pending.get(label).map(String::as_str) == Some(id) {
            pending.remove(label);
            true
        } else {
            false
        }
    }

    fn clear(&self, label: &str) {
        self.pending.lock().unwrap().remove(label);
    }

    fn is_pending(&self, label: &str, id: &str) -> bool {
        self.pending.lock().unwrap().get(label).map(String::as_str) == Some(id)
    }
}

fn new_unique_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}{:x}", nanos, std::process::id())
}

/// Wire the CloseRequested handler on an editor window: every close request
/// is intercepted and routed through the isModified handshake with the webapp.
pub fn register_close_handler(window: &tauri::WebviewWindow) {
    let win_close = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let unique_id = new_unique_id();
            let label = win_close.label().to_string();
            {
                let state = win_close.app_handle().state::<CloseState>();
                let mut pending = state.pending.lock().unwrap();
                if pending.contains_key(&label) {
                    return; // close flow already in progress for this window
                }
                pending.insert(label, unique_id.clone());
            }
            win_close.eval(&format!("window.__tauriCloseCheck('{}')", unique_id)).ok();
        }
    });
}

/// Try to handle a close-handshake channel. Returns `true` when the channel
/// belonged to the handshake (caller should not process it further).
pub fn handle_message(
    channel: &str,
    data: &JsonValue,
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> bool {
    match channel {
        // Step 2: webapp answered "isModified"
        "isModified-result" => {
            let unique_id = data["uniqueId"].as_str().unwrap_or("");
            let label = window.label().to_string();
            {
                let state = app.state::<CloseState>();
                if !state.is_pending(&label, unique_id) {
                    return true; // stale or unsolicited reply
                }
            }

            if data["isModified"].as_bool().unwrap_or(false) {
                // 3-button dialog matching drawio-desktop: Save / Discard / Cancel
                let result = app.dialog()
                    .message("The document has unsaved changes. Do you want to save them?")
                    .title("Confirm")
                    .kind(tauri_plugin_dialog::MessageDialogKind::Warning)
                    .buttons(tauri_plugin_dialog::MessageDialogButtons::YesNoCancelCustom(
                        "Save".into(), "Discard Changes".into(), "Cancel".into()))
                    .blocking_show_with_result();

                match result {
                    tauri_plugin_dialog::MessageDialogResult::Yes => {
                        // Keep pending id; webapp saves then replies "saveAndClose-result"
                        window.eval(&format!("window.__tauriSaveAndClose('{}')", unique_id)).ok();
                    }
                    tauri_plugin_dialog::MessageDialogResult::No => {
                        app.state::<CloseState>().clear(&label);
                        // Discard: remove the draft, then destroy the window
                        if let Some(draft_path) = data["draftPath"].as_str() {
                            if Path::new(draft_path).exists() {
                                fs::remove_file(draft_path).ok();
                            }
                            window.destroy().ok();
                        } else {
                            window.eval("window.__tauriRemoveDraft()").ok();
                            // "draftRemoved" message finishes the close
                        }
                    }
                    _ => {
                        // Cancel: abort close
                        app.state::<CloseState>().clear(&label);
                    }
                }
            } else {
                app.state::<CloseState>().clear(&label);
                window.destroy().ok();
            }
            true
        }

        // Final step (Save chosen): webapp finished saving
        "saveAndClose-result" => {
            let unique_id = data["uniqueId"].as_str().unwrap_or("");
            let success = data["success"].as_bool().unwrap_or(false);
            let state = app.state::<CloseState>();
            if success && state.take_if_matches(window.label(), unique_id) {
                window.destroy().ok();
            } else {
                // Save failed or was cancelled (Save As dialog aborted): stay open
                state.clear(window.label());
            }
            true
        }

        // Final step (Discard chosen): draft removed
        "draftRemoved" => {
            app.state::<CloseState>().clear(window.label());
            window.destroy().ok();
            true
        }

        _ => false,
    }
}
