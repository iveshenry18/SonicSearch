// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_index;
mod clap;
mod search;
mod state;

use std::sync::Arc;

use anyhow::Context;
use futures::lock::Mutex;

use audio_index::update_audio_index;
use search::search_index;
use simple_logger::SimpleLogger;
use sqlx::SqlitePool;
use state::{
    audio_embedder::AudioEmbedder,
    database::{self, vector_index::initialize_index},
    AppState,
};
use tauri::{async_runtime::RwLock, Manager};

fn main() {
    log::set_max_level(log::LevelFilter::Info);
    SimpleLogger::new().init().unwrap();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![search_index, update_audio_index])
        .setup(|app| {
            let handle = app.handle();

            let (clap_model_text_embedder, clap_model_audio_embedder) =
                clap::load_clap_models(&app.path_resolver()).expect("Failed to load clap model");

            let mut vector_index = initialize_index().context("Failed to initialize index")?;

            let pool: SqlitePool =
                tauri::async_runtime::block_on(database::initialize_database(&handle, &mut vector_index))
                    .context("Failed to initialize database")?;

            app.manage(AppState {
                pool,
                clap_model_audio_embedder: AudioEmbedder::new(clap_model_audio_embedder),
                clap_model_text_embedder: Arc::new(Mutex::new(clap_model_text_embedder)),
                is_indexing: RwLock::new(false),
                vector_index: RwLock::new(vector_index),
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
