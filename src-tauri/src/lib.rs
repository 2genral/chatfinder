mod indexer;
mod models;
mod search;
mod state;

use models::{AppStatus, IndexSummary, SearchResult};
use state::AppState;
use std::{fs, io};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
async fn get_status(state: State<'_, AppState>) -> Result<AppStatus, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || indexer::load_status(&state))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn sync_index(app: AppHandle, state: State<'_, AppState>) -> Result<IndexSummary, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || indexer::sync_index(&app, &state))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn search_sessions(
    query: String,
    provider: Option<String>,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let state = state.inner().clone();
    let provider = provider.unwrap_or_else(|| "all".to_string());
    tauri::async_runtime::spawn_blocking(move || {
        search::search_sessions(&state, &query, &provider, limit.unwrap_or(100))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;
            let state =
                AppState::new(app_data_dir.join("chatfinder.sqlite3")).map_err(io::Error::other)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            sync_index,
            search_sessions
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
