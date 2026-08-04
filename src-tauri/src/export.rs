//! Local export pipeline via a hidden export3.html renderer window,
//! mirroring drawio-desktop's exportDiagram():
//!
//! ```text
//! editor window 'export' message
//!   → Rust spawns hidden window (export-N) loading export3.html
//!   → renderer signals 'export-renderer-ready' → Rust evals __tauriRender(args)
//!   → renderer draws the pages, replies 'render-finished' (or 'export-error')
//!   → print: show window, inject @page CSS + body zoom, window.print();
//!            'print-after' (afterprint hook) ends the job
//!     svg:   Rust evals __tauriGetSvgData() → renderer replies 'svg-data'
//!     xml:   renderer replies 'xml-data' on its own
//!   → Rust forwards 'export-success' / 'export-error' to the requesting
//!     window (the webapp's mxElectronRequest waits on those events, then
//!     sends 'export-finalize')
//! ```
//!
//! png/jpg/pdf file output needs webview capture / printToPDF equivalents
//! that Tauri does not expose; those formats still return an error.
//! Print goes through the system print dialog of the renderer window:
//! `@page size` approximates Electron's preferCSSPageSize and `body zoom`
//! approximates scaleFactor = 100 / pageScale [jgraph/drawio#5540].

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::Manager;

use crate::windows::WindowsState;

pub struct ExportState {
    /// export window label → running job
    jobs: Mutex<HashMap<String, ExportJob>>,
    counter: AtomicU64,
}

struct ExportJob {
    /// Label of the editor window that requested the export
    requester: String,
    /// Args from the webapp's 'export' message
    args: JsonValue,
}

impl ExportState {
    pub fn new() -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(0),
        }
    }

    fn next_label(&self) -> String {
        format!("export-{}", self.counter.fetch_add(1, Ordering::SeqCst))
    }

    fn take_job(&self, label: &str) -> Option<ExportJob> {
        self.jobs.lock().unwrap().remove(label)
    }

    fn job_args(&self, label: &str) -> Option<(String, JsonValue)> {
        self.jobs
            .lock()
            .unwrap()
            .get(label)
            .map(|job| (job.requester.clone(), job.args.clone()))
    }
}

/// Push an export reply into the requesting editor window.
fn reply(app: &tauri::AppHandle, requester: &str, channel: &str, data: JsonValue) {
    if let Some(win) = app.get_webview_window(requester) {
        let js = format!("window.__tauriExportReply('{}', {})", channel, data);
        win.eval(&js).ok();
    }
}

/// Reply to the requester and tear down the renderer window + job.
fn reply_and_destroy(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    channel: &str,
    data: JsonValue,
) {
    if let Some(job) = app.state::<ExportState>().take_job(window.label()) {
        reply(app, &job.requester, channel, data);
    }
    window.destroy().ok();
}

/// Route export-related messages. Returns true when handled.
///
/// Messages from `export-*` windows are renderer-side events; `export` /
/// `export-finalize` come from editor windows.
pub fn handle_message(
    channel: &str,
    data: &JsonValue,
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> bool {
    if window.label().starts_with("export-") {
        return handle_renderer_message(channel, data, app, window);
    }

    match channel {
        "export" => {
            start_export(app, window, data.clone());
            true
        }

        // mxElectronRequest cleanup after success/error: destroy any
        // lingering renderer windows of this requester (normally gone already)
        "export-finalize" => {
            let labels: Vec<String> = {
                let state = app.state::<ExportState>();
                let jobs = state.jobs.lock().unwrap();
                jobs.iter()
                    .filter(|(_, job)| job.requester == window.label())
                    .map(|(label, _)| label.clone())
                    .collect()
            };
            for label in labels {
                app.state::<ExportState>().take_job(&label);
                if let Some(win) = app.get_webview_window(&label) {
                    win.destroy().ok();
                }
            }
            true
        }

        _ => false,
    }
}

/// 'export' message from an editor window: spawn the hidden renderer window.
fn start_export(app: &tauri::AppHandle, requester: &tauri::WebviewWindow, args: JsonValue) {
    let state = app.state::<ExportState>();
    let label = state.next_label();
    let bridge_js = app.state::<WindowsState>().bridge_js().to_string();

    let result = tauri::WebviewWindowBuilder::new(
        app,
        &label,
        tauri::WebviewUrl::App("export3.html".into()),
    )
    .title("draw.io Export")
    .inner_size(1024.0, 768.0)
    .visible(false)
    .skip_taskbar(true)
    .initialization_script(bridge_js)
    .build();

    match result {
        Ok(_win) => {
            state.jobs.lock().unwrap().insert(
                label,
                ExportJob {
                    requester: requester.label().to_string(),
                    args,
                },
            );
        }
        Err(e) => {
            reply(
                app,
                requester.label(),
                "export-error",
                JsonValue::String(e.to_string()),
            );
        }
    }
}

/// Messages coming from an export renderer window.
fn handle_renderer_message(
    channel: &str,
    data: &JsonValue,
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> bool {
    match channel {
        // export.js loaded and registered its 'render' listener
        "export-renderer-ready" => {
            match app.state::<ExportState>().job_args(window.label()) {
                Some((_, args)) => {
                    window.eval(&format!("window.__tauriRender({})", args)).ok();
                }
                None => {
                    window.destroy().ok();
                }
            }
            true
        }

        "render-finished" => on_render_finished(data, app, window),

        // svg flow: renderer answered __tauriGetSvgData()
        "svg-data" => {
            reply_and_destroy(app, window, "export-success", data.clone());
            true
        }

        // xml flow: renderer pushes the resolved diagram XML on its own
        "xml-data" => {
            reply_and_destroy(app, window, "export-success", data.clone());
            true
        }

        "xml-data-error" => {
            reply_and_destroy(app, window, "export-error", JsonValue::Null);
            true
        }

        // print flow: the print dialog was closed (printed or canceled)
        "print-after" => {
            reply_and_destroy(app, window, "export-success", JsonValue::Null);
            true
        }

        // renderer-side failure (bad input, layout error, ...)
        "export-error" => {
            reply_and_destroy(app, window, "export-error", data.clone());
            true
        }

        _ => false,
    }
}

/// Renderer finished drawing the pages: dispatch per export format.
fn on_render_finished(
    data: &JsonValue,
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> bool {
    let (requester, args) = match app.state::<ExportState>().job_args(window.label()) {
        Some(job) => job,
        None => {
            window.destroy().ok();
            return true;
        }
    };

    let format = args["format"].as_str().unwrap_or("");
    let is_print = args["print"].as_bool().unwrap_or(false);

    // Mirror the official validity check: null/tiny bounds means the input
    // failed to render (printToPDF would never return below 5px)
    let bounds_ok = data["bounds"]
        .as_str()
        .and_then(|b| serde_json::from_str::<JsonValue>(b).ok())
        .map(|b| {
            b["width"].as_f64().unwrap_or(0.0) >= 5.0
                && b["height"].as_f64().unwrap_or(0.0) >= 5.0
        })
        .unwrap_or(false);

    // The xml flow reports through xml-data; a null render-finished can
    // follow it, so only treat it as an error when no reply was sent yet
    if format != "xml" && !bounds_ok {
        reply_and_destroy(
            app,
            window,
            "export-error",
            JsonValue::String("Error: empty or invalid diagram".to_string()),
        );
        return true;
    }

    if is_print {
        // Show the window so the system print dialog has a visible owner;
        // it is destroyed on 'print-after'
        let page_w = args["pageWidth"].as_f64().unwrap_or(827.0);
        // 1.025 height adjustment mirrors drawio-desktop ("fixes the output")
        let page_h = args["pageHeight"].as_f64().unwrap_or(1169.0) * 1.025;
        let page_scale = args["pageScale"].as_f64().unwrap_or(1.0);
        let zoom = if page_scale > 0.0 { 1.0 / page_scale } else { 1.0 };
        let js = format!(
            "(function(){{var st=document.createElement('style');\
             st.textContent='@page{{size:{w}px {h}px;margin:0;}}\
             html,body{{margin:0!important;padding:0!important;}}body{{zoom:{zoom};}}';\
             document.head.appendChild(st);\
             window.addEventListener('afterprint',function(){{\
             window.electron.sendMessage('print-after',{{}});}},{{once:true}});\
             setTimeout(function(){{window.print();}},50);}})();",
            w = page_w,
            h = page_h,
            zoom = zoom
        );
        window.show().ok();
        window.set_focus().ok();
        window.eval(&js).ok();
        true
    } else if format == "svg" {
        window.eval("window.__tauriGetSvgData()").ok();
        true
    } else if format == "xml" {
        // Renderer sends xml-data / xml-data-error on its own; keep waiting
        true
    } else {
        reply(
            app,
            &requester,
            "export-error",
            JsonValue::String(format!(
                "Export format '{}' is not yet supported by the local Tauri pipeline",
                format
            )),
        );
        window.destroy().ok();
        app.state::<ExportState>().take_job(window.label());
        true
    }
}
