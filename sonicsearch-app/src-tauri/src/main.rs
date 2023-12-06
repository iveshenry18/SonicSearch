// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_index;
mod clap;
pub mod index_paths;
mod search;
mod state;

use std::sync::Arc;

use anyhow::Context;
use futures::lock::Mutex;

use audio_index::{indexing_status::{IndexingStatus, IndexingStatusChanged}, update_audio_index, UpdateAudioIndex};
use search::search_index;
use sqlx::SqlitePool;
use state::{
    audio_embedder::AudioEmbedder,
    database::{self, vector_index::initialize_index},
    AppState,
};
use tauri::{
    async_runtime::{self, RwLock},
    Manager,
};
use tauri_specta::Event;

use crate::index_paths::{
    add_path_to_index, add_paths_to_index, delete_path_from_index, get_paths_from_index,
};

fn main() {
    env_logger::init();

    let specta_builder = {
        let specta_builder = tauri_specta::ts::builder()
            .commands(tauri_specta::collect_commands![
                add_path_to_index,
                add_paths_to_index,
                get_paths_from_index,
                delete_path_from_index
            ])
            .events(tauri_specta::collect_events![IndexingStatusChanged, UpdateAudioIndex]);

        #[cfg(debug_assertions)]
        let specta_builder = specta_builder.path("../src/lib/specta-bindings.ts");

        specta_builder.into_plugin()
    };

    tauri::Builder::default()
        .plugin(specta_builder)
        .invoke_handler(tauri::generate_handler![
            search_index,
            add_path_to_index,
            add_paths_to_index,
            get_paths_from_index,
            delete_path_from_index
        ])
        .setup(|app| {
            let handle = app.handle();

            let (clap_model_text_embedder, clap_model_audio_embedder) =
                clap::load_clap_models(&app.path_resolver()).expect("Failed to load clap model");

            let mut vector_index = initialize_index(None);

            let pool: SqlitePool = tauri::async_runtime::block_on(database::initialize_database(
                &handle,
                &mut vector_index,
            ))
            .context("Failed to initialize database")?;

            app.manage(AppState {
                pool,
                clap_model_audio_embedder: AudioEmbedder::new(clap_model_audio_embedder),
                clap_model_text_embedder: Arc::new(Mutex::new(clap_model_text_embedder)),
                indexing_status: IndexingStatus::new(handle.clone()),
                vector_index: RwLock::new(vector_index),
            });

            UpdateAudioIndex::listen_global(&handle.clone(), move |_| {
                let handle = handle.clone();
                async_runtime::spawn(async move {
                    match update_audio_index(handle.state::<AppState>()).await {
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("Error while updating audio index: {:?}", e);
                        }
                    }
                });
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
