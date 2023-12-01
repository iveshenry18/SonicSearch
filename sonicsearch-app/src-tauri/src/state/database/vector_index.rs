use anyhow::{Context, Result};

use faiss::Index;
use log::debug;
use sqlx::SqlitePool;

use crate::state::database::encode_embedding;

pub fn initialize_index() -> Result<Index> {
    debug!("Initializing index");
    todo!("Install and implement index with faiss");
}

/// Synchronize embeddings from the audio_file_segment table
/// to the faiss index.
pub async fn synchronize_index(pool: &SqlitePool) -> Result<()> {
    let _transaction = pool.begin().await?;

    todo!("Install and implement index with faiss");

    // transaction.commit().await?;
    // Ok(())
}

#[derive(sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct PathAndTimestamp {
    path: String,
    starting_timestamp: f64,
}

pub async fn get_knn(
    search_string_embedding: &[f32],
    _pool: &SqlitePool,
) -> Result<Vec<PathAndTimestamp>> {
    const LIMIT: u32 = 10;
    let encoded_search_string_embedding: String = encode_embedding(search_string_embedding)
        .context("Failed to encode search string embedding")?;
    debug!(
        "Encoded search string embedding has length {} and values {}...",
        &encoded_search_string_embedding.len(),
        &encoded_search_string_embedding[0..50]
    );
    todo!("Implement search with faiss");
}
