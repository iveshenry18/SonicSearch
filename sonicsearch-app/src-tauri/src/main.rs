// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_index;
mod clap;
mod database;
mod state;

use std::sync::Arc;

use futures::lock::Mutex;

use audio_index::{get_search_results, update_audio_index};
use sqlx::SqlitePool;
use state::{audio_embedder::AudioEmbedder, AppState};
use tauri::{async_runtime::RwLock, Manager};

#[tauri::command]
fn search(app_state: tauri::State<AppState>, search_string: &str) -> Result<Vec<String>, String> {
    println!("Searching for: {}", search_string);
    get_search_results(search_string, &app_state.pool.clone()).or(Err("Failed to search".into()))
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![search, update_audio_index])
        .setup(|app| {
            let handle = app.handle();

            let (clap_model_text_embedder, clap_model_audio_embedder) =
                clap::load_clap_models(&app.path_resolver()).expect("Failed to load clap model");

            let pool: SqlitePool =
                tauri::async_runtime::block_on(database::initialize_database(&handle))?;

            app.manage(AppState {
                pool,
                clap_model_audio_embedder: AudioEmbedder::new(clap_model_audio_embedder),
                clap_model_text_embedder: Arc::new(Mutex::new(clap_model_text_embedder)),
                is_indexing: RwLock::new(false),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![search, update_audio_index])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
