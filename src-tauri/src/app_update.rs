//! App self-update: check GitHub Releases for a newer version, download it in the
//! background, and install it when the user chooses to restart.

use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, State};
use tauri_plugin_updater::{Update, Updater, UpdaterExt};

/// Split across the host's addresses, so an unreachable server costs a few seconds
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// A downloaded update waiting for the user to restart
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<(Update, Vec<u8>)>>);

fn updater(app: &AppHandle) -> Result<Updater, String> {
    app.updater_builder()
        // Some networks silently drop connections to one of GitHub's download servers;
        // without a connect timeout the request hangs on it instead of trying the next
        .configure_client(|client| client.connect_timeout(CONNECT_TIMEOUT))
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())
}

/// Check for a newer version and download it. Resolves with the version that is ready
/// to install, or None when the app is up to date.
#[tauri::command]
pub async fn download_app_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<Option<String>, String> {
    if let Some((update, _)) = pending.0.lock().unwrap().as_ref() {
        return Ok(Some(update.version.clone()));
    }

    let Some(update) = updater(&app)?.check().await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    eprintln!("[app_update] Downloading {}", update.version);
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;

    let version = update.version.clone();
    *pending.0.lock().unwrap() = Some((update, bytes));
    Ok(Some(version))
}

/// Install the downloaded update and restart into it. On Windows the installer closes
/// the app itself and reopens it when done.
#[tauri::command]
pub fn install_app_update(app: AppHandle, pending: State<'_, PendingUpdate>) -> Result<(), String> {
    let Some((update, bytes)) = pending.0.lock().unwrap().take() else {
        return Err("No downloaded update to install".into());
    };
    update.install(bytes).map_err(|e| e.to_string())?;
    app.restart()
}
