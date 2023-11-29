pub mod audio_embedder;
use std::sync::Arc;

use futures::lock::Mutex;

use ort::Session;
use sqlx::SqlitePool;
use tauri::async_runtime::RwLock;

use audio_embedder::AudioEmbedder;

pub struct AppState {
    pub pool: SqlitePool,
    pub clap_model_text_embedder: Arc<Mutex<Session>>,
    pub clap_model_audio_embedder: AudioEmbedder,
    pub is_indexing: RwLock<bool>,
}
