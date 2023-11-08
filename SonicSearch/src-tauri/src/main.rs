// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clap;
mod database;
mod state;

use database::{get_search_results, update_audio_file_index};
use state::{AppState, ServiceAccess};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
fn search(app_handle: AppHandle, search_string: &str) -> Vec<String> {
    println!("Searching for: {}", search_string);
    // TODO: handle errors
    let items = app_handle
        .db(|db| get_search_results(search_string, db))
        .unwrap();

    items
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            db: Default::default(),
            clap_model_audio_embedder: Default::default(),
            clap_model_text_embedder: Default::default(),
        })
        .invoke_handler(tauri::generate_handler![search])
        .setup(|app| {
            let handle = app.handle();

            let app_state: State<AppState> = handle.state();

            // let (clap_model_text_embedder, clap_model_audio_embedder) =
            //     clap::load_clap_models(&app.path_resolver()).expect("Failed to load clap model");
            let db = database::initialize_database(&handle)
                .expect("Database initialization should succeed");

            *app_state.db.lock().unwrap() = Some(db);
            // *app_state.clap_model_text_embedder.lock().unwrap() = Some(clap_model_text_embedder);
            // *app_state.clap_model_audio_embedder.lock().unwrap() = Some(clap_model_audio_embedder);

            update_audio_file_index(
                app_state
                    .db
                    .lock()
                    .unwrap()
                    .as_ref()
                    .expect("Could not access db to update audio file index"),
            )
            .expect("Failed to update audio file index");

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
