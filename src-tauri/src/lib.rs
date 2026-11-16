// FlowLog — local computer-activity log. Polls the frontmost app every 5 s,
// records (app, title) changes to a JSONL file in the app data dir.
// No screenshots, no keylogging, no network, no account.

mod store;
mod tracker;

use store::{safe_export_path, Event, Shared, TickResult, Tracker};
use std::sync::Mutex;
use tauri::{Manager, State};

#[tauri::command]
fn tracker_status(state: State<Shared>) -> serde_json::Value {
    let t = state.0.lock().unwrap();
    serde_json::json!({
        "paused": t.paused(),
        "excluded": t.excluded(),
        "tracked_events": t.event_count(),
    })
}

#[tauri::command]
fn tick(state: State<Shared>) -> TickResult {
    state.0.lock().unwrap().tick()
}

#[tauri::command]
fn set_paused(state: State<Shared>, paused: bool) -> Result<(), String> {
    state.0.lock().unwrap().set_paused(paused)
}

#[tauri::command]
fn set_excluded(state: State<Shared>, apps: Vec<String>) -> Result<Vec<String>, String> {
    let mut t = state.0.lock().unwrap();
    t.set_excluded(apps)?;
    Ok(t.excluded())
}

#[tauri::command]
fn events_between(state: State<Shared>, start: u64, end: u64) -> Vec<Event> {
    state.0.lock().unwrap().events_between(start, end)
}

#[tauri::command]
fn clear_events(state: State<Shared>) -> Result<(), String> {
    state.0.lock().unwrap().clear_events()
}

/// Export a report the frontend already rendered. Written into the user's
/// Downloads folder with a strict filename whitelist.
#[tauri::command]
fn write_export(app: tauri::AppHandle, filename: String, content: String) -> Result<String, String> {
    let download_dir = app
        .path()
        .download_dir()
        .map_err(|e| format!("could not locate your Downloads folder: {e}"))?;
    let path = safe_export_path(&download_dir, &filename)?;
    std::fs::write(&path, content).map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let tracker = Tracker::load(data_dir);
            app.manage(Shared(Mutex::new(tracker)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            tracker_status, tick, set_paused, set_excluded, events_between, clear_events, write_export,
        ])
        .run(tauri::generate_context!())
        .expect("error while running FlowLog");
}
