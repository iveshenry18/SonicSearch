use anyhow::{Context, Result};

use faiss::{FlatIndex, IdMap};
use log::debug;
use sqlx::SqlitePool;

use crate::{state::database::encode_embedding, clap::EMBEDDING_SIZE};

pub fn initialize_index() -> Result<IdMap<FlatIndex>> {
    debug!("Initializing index");
    let factory_index = FlatIndex::new_l2(EMBEDDING_SIZE.into()).context("Failed to create FlatIndex")?;
    IdMap::new(factory_index).context("Failed to convert the IndexImpl to an Index")
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
    const _LIMIT: u32 = 10;
    let encoded_search_string_embedding: String = encode_embedding(search_string_embedding)
        .context("Failed to encode search string embedding")?;
    debug!(
        "Encoded search string embedding has length {} and values {}...",
        &encoded_search_string_embedding.len(),
        &encoded_search_string_embedding[0..50]
    );
    todo!("Implement search with faiss");
}
