pub mod audio_embedder;
pub mod database;
use std::sync::Arc;

use futures::lock::Mutex;

use ort::Session;
use sqlx::SqlitePool;
use tauri::async_runtime::RwLock;

use audio_embedder::AudioEmbedder;


use crate::audio_index::indexing_status::IndexingStatus;

use self::database::vector_index::VectorIndex;

pub struct AppState {
    pub pool: SqlitePool,
    pub clap_model_text_embedder: Arc<Mutex<Session>>,
    pub clap_model_audio_embedder: AudioEmbedder,
    pub indexing_status: IndexingStatus,
    pub vector_index: RwLock<VectorIndex>,
}
