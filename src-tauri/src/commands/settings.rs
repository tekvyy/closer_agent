use crate::db::{self, DbState, Settings};
use crate::jobs::JobQueue;
use tauri::State;

#[tauri::command]
pub fn get_settings(state: State<'_, DbState>) -> Result<Settings, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let settings = db::get_settings(&conn).map_err(|e| e.to_string())?;
    eprintln!(
        "[settings] Retrieved settings: model='{}', use_chrome={}",
        settings.model, settings.use_chrome
    );
    Ok(settings)
}

#[tauri::command]
pub fn update_settings(
    state: State<'_, DbState>,
    model: String,
    use_chrome: bool,
) -> Result<(), String> {
    if !crate::model_config::is_supported_model(&model) {
        return Err(format!("Unsupported model: {model}"));
    }

    eprintln!(
        "[settings] Updating settings: model='{}', use_chrome={}",
        model, use_chrome
    );
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    db::update_settings(&conn, &model, use_chrome).map_err(|e| {
        let err_msg = e.to_string();
        eprintln!("[settings] ERROR: Failed to update settings: {}", err_msg);
        err_msg
    })?;
    eprintln!("[settings] Successfully updated settings");
    Ok(())
}

/// Turn autopilot on or off. Turning it off also cancels the jobs it queued that haven't
/// started yet, so the backlog stops too.
#[tauri::command]
pub async fn set_auto_research(
    state: State<'_, DbState>,
    queue: State<'_, JobQueue>,
    enabled: bool,
) -> Result<(), String> {
    {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        db::set_auto_research(&conn, enabled).map_err(|e| e.to_string())?;
    }
    if !enabled {
        let cancelled = queue.cancel_queued(true).await;
        eprintln!("[settings] Autopilot off, cancelled {cancelled} queued jobs");
    }
    Ok(())
}

/// Cancel every job waiting for a slot. Returns how many were cancelled.
#[tauri::command]
pub async fn cancel_queued_jobs(queue: State<'_, JobQueue>) -> Result<usize, String> {
    Ok(queue.cancel_queued(false).await)
}
