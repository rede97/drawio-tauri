//! Editor window creation and app-level window bookkeeping.
//!
//! The first window is labeled `main`; additional windows (File → New
//! Window) get `win-N` labels. Every editor window gets the bridge init
//! script and the close-confirm handshake. Command-line file args are
//! delivered to the first window only (mirrors drawio-desktop).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::Manager;

use crate::{close_flow, prefs};

/// App-level window bookkeeping shared by all editor windows.
pub struct WindowsState {
    /// Counter for unique labels of additional windows (win-1, win-2, ...)
    counter: AtomicU64,
    /// Command-line file args go to the first editor window only
    args_consumed: AtomicBool,
    /// FIFO of file args for windows spawned by second-instance launches
    pending_args: Mutex<Vec<Vec<String>>>,
    /// Bridge init script (version placeholder already substituted)
    bridge_js: String,
    /// Editor URL with prefs-derived params
    editor_url: String,
}

impl WindowsState {
    pub fn new(bridge_js: String, editor_url: String) -> Self {
        Self {
            counter: AtomicU64::new(0),
            args_consumed: AtomicBool::new(false),
            pending_args: Mutex::new(Vec::new()),
            bridge_js,
            editor_url,
        }
    }

    pub fn bridge_js(&self) -> &str {
        &self.bridge_js
    }

    /// Returns true exactly once: the first editor window to ask receives
    /// the command-line file args via `args-obj`.
    pub fn take_first_window_args(&self) -> bool {
        !self.args_consumed.swap(true, Ordering::SeqCst)
    }

    /// Queue file args for a window spawned by a second-instance launch
    pub fn push_pending_args(&self, args: Vec<String>) {
        self.pending_args.lock().unwrap().push(args);
    }

    /// Take the oldest queued second-instance args, if any
    pub fn take_pending_args(&self) -> Option<Vec<String>> {
        let mut pending = self.pending_args.lock().unwrap();
        if pending.is_empty() {
            None
        } else {
            Some(pending.remove(0))
        }
    }
}

/// Editor URL with desktop-mode and prefs-derived params
pub fn build_editor_url(prefs_data: &prefs::Prefs) -> String {
    let gf = if prefs_data.is_google_fonts_enabled { "1" } else { "0" };
    let sc = if prefs_data.enable_spell_check { "1" } else { "0" };
    let sb = if prefs_data.store_bkp { "1" } else { "0" };
    format!("index.html?dev=0&test=0&gapi=0&db=0&od=0&gh=0&gl=0&tr=0&browser=0&picker=0&mode=device&export=https://convert.diagrams.net/node/export&disableUpdate=0&enableSpellCheck={sc}&enableStoreBkp={sb}&isGoogleFontsEnabled={gf}")
}

/// Create an editor window with bridge injection and the close-confirm
/// handshake wired. `label` is `Some("main")` for the first window and
/// `None` for additional windows (File → New Window).
pub fn create_editor_window(
    app: &tauri::AppHandle,
    label: Option<String>,
    width: Option<f64>,
) -> tauri::Result<tauri::WebviewWindow> {
    let (bridge_js, editor_url, label) = {
        let state = app.state::<WindowsState>();
        let label = label.unwrap_or_else(|| {
            format!("win-{}", state.counter.fetch_add(1, Ordering::SeqCst) + 1)
        });
        (state.bridge_js.clone(), state.editor_url.clone(), label)
    };

    let window = tauri::WebviewWindowBuilder::new(
        app,
        &label,
        tauri::WebviewUrl::App(editor_url.into()),
    )
    .title("draw.io")
    .inner_size(width.unwrap_or(1280.0), 800.0)
    .min_inner_size(800.0, 600.0)
    .resizable(true)
    .fullscreen(false)
    .initialization_script(bridge_js)
    .build()?;

    close_flow::register_close_handler(&window);
    Ok(window)
}
