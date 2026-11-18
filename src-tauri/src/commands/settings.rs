use crate::db::{self, DbState, Settings};
use crate::jobs::JobQueue;
use tauri::State;

#[tauri::command]
pub fn get_settings(state: State<'_, DbState>) -> Result<Settings, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let settings = db::get_settings(&conn).map_err(|e| e.to_string())?;
    Ok(settings)
}

/// Choose which account research runs on, by saving that account's research model
#[tauri::command]
pub fn update_settings(state: State<'_, DbState>, model: String) -> Result<(), String> {
    if !crate::model_config::is_supported_model(&model) {
        return Err(format!("Unsupported model: {model}"));
    }

    eprintln!("[settings] Updating settings: model='{}'", model);
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    db::update_settings(&conn, &model).map_err(|e| {
        let err_msg = e.to_string();
        eprintln!("[settings] ERROR: Failed to update settings: {}", err_msg);
        err_msg
    })?;
    Ok(())
}

/// Cancel every job waiting for a slot. Returns how many were cancelled.
#[tauri::command]
pub async fn cancel_queued_jobs(queue: State<'_, JobQueue>) -> Result<usize, String> {
    Ok(queue.cancel_queued().await)
}
