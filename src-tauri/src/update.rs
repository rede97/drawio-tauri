//! Auto-update via `tauri-plugin-updater`.
//!
//! The updater fetches `latest.json` from the GitHub release marked "latest"
//! (endpoint configured in `tauri.conf.json`). Release assets and signatures
//! are produced by `scripts/release.ps1` (build + sign + `gh release` upload).
//!
//! Two entry points:
//! - [`spawn_background_check`] — silent startup check (matches the official
//!   drawio-desktop behavior when `disableUpdate=0`)
//! - [`check_manual`] — Help → Check for Updates menu action, always reports
//!   the outcome in a dialog

use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_updater::UpdaterExt;

/// Delay before the silent startup check, so the webview can finish loading.
const STARTUP_CHECK_DELAY_SECS: u64 = 5;

/// Kick off the silent background update check at app start.
pub fn spawn_background_check(app: &tauri::AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        std::thread::sleep(std::time::Duration::from_secs(STARTUP_CHECK_DELAY_SECS));
        run_check(handle, false).await;
    });
}

/// Manual check triggered by the webapp's Help → Check for Updates action.
pub async fn check_manual(app: tauri::AppHandle) {
    run_check(app, true).await;
}

async fn run_check(app: tauri::AppHandle, manual: bool) {
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            if manual {
                error_dialog(&app, &format!("Updater unavailable: {}", e));
            }
            return;
        }
    };

    match updater.check().await {
        Ok(Some(update)) => {
            let current = env!("CARGO_PKG_VERSION");
            let msg = format!(
                "A new version {} is available.\nCurrent version: {}\n\nDownload and install now?",
                update.version, current
            );
            let accepted = app.dialog()
                .message(msg)
                .title("Update Available")
                .kind(tauri_plugin_dialog::MessageDialogKind::Info)
                .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                    "Update".into(), "Later".into()))
                .blocking_show();
            if accepted {
                download_install_restart(app, update).await;
            }
        }
        Ok(None) => {
            if manual {
                app.dialog()
                    .message("You are running the latest version.")
                    .title("No Updates")
                    .kind(tauri_plugin_dialog::MessageDialogKind::Info)
                    .blocking_show();
            }
        }
        Err(e) => {
            if manual {
                error_dialog(&app, &format!("Failed to check for updates: {}", e));
            }
        }
    }
}

async fn download_install_restart(app: tauri::AppHandle, update: tauri_plugin_updater::Update) {
    let result = update
        .download_and_install(|_chunk, _total| {}, || {})
        .await;

    match result {
        Ok(()) => {
            let restart = app.dialog()
                .message("The update has been installed. Restart draw.io now?")
                .title("Update Installed")
                .kind(tauri_plugin_dialog::MessageDialogKind::Info)
                .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                    "Restart".into(), "Later".into()))
                .blocking_show();
            if restart {
                app.cleanup_before_exit();
                tauri::process::restart(&app.env());
            }
        }
        Err(e) => {
            error_dialog(&app, &format!("Failed to install the update: {}", e));
        }
    }
}

fn error_dialog(app: &tauri::AppHandle, msg: &str) {
    app.dialog()
        .message(msg)
        .title("Update Check Failed")
        .kind(tauri_plugin_dialog::MessageDialogKind::Error)
        .blocking_show();
}
