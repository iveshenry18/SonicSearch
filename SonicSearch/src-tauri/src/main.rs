// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod database;
mod state;
mod clap;

use database::{update_audio_file_index, get_search_results};
use state::{AppState, ServiceAccess};
use tauri::{State, Manager, AppHandle};

#[tauri::command]
fn search(app_handle: AppHandle, search_string: &str) -> Vec<String> {
    println!("Searching for: {}", search_string);
    // TODO: handle errors
    let items = app_handle.db(|db| get_search_results(search_string, db)).unwrap();

    items
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            db: Default::default(),
            clap_model: Default::default(),
        })
        .invoke_handler(tauri::generate_handler![search])
        .setup(|app| {
            let handle = app.handle();

            let app_state: State<AppState> = handle.state();

            let clap_model = clap::load_clap_model(&app.path_resolver()).expect("Failed to load clap model");
            let db = database::initialize_database(&handle)
                .expect("Database initialization should succeed");

            update_audio_file_index(&db).expect("Failed to update audio file index");
            *app_state.db.lock().unwrap() = Some(db);
            *app_state.clap_model.lock().unwrap() = Some(clap_model);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
