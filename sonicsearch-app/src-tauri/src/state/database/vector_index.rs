use anyhow::{anyhow, Context, Result};

use faiss::{FlatIndex, IdMap, Idx, Index};
use futures::future::join_all;
use log::debug;
use sqlx::SqlitePool;

use crate::{clap::EMBEDDING_SIZE, state::database::decode_embedding};

pub fn initialize_index() -> Result<IdMap<FlatIndex>> {
    debug!("Initializing index");
    let flat_index =
        FlatIndex::new_l2(EMBEDDING_SIZE.into()).context("Failed to create FlatIndex")?;
    IdMap::new(flat_index).context("Failed to add IdMap to FlatIndex")
}

struct IndexRow {
    rowid: Option<i64>,
    embedding: Vec<u8>,
}

/// Synchronize embeddings from the audio_file_segment table
/// to the faiss index.
pub async fn synchronize_index(pool: &SqlitePool, index: &mut IdMap<FlatIndex>) -> Result<()> {
    debug!("Synchronizing index");
    let id_embeddings: Vec<IndexRow> = sqlx::query_as!(
        IndexRow,
        r#"
        SELECT
            audio_file_segment.rowid,
            audio_file_segment.embedding
        FROM audio_file_segment
        WHERE audio_file_segment.embedding IS NOT NULL
        AND audio_file_segment.rowid IS NOT NULL
        "#
    )
    .fetch_all(pool)
    .await?;
    debug!(
        "Found {} embeddings to add to the index",
        id_embeddings.len()
    );

    let id_embeddings: Vec<(i64, Vec<f32>)> = id_embeddings
        .iter()
        .map(|row| {
            let embedding: Vec<f32> =
                decode_embedding(&row.embedding).context("Could not decode embeddings")?;
            let rowid = row.rowid.expect("rowid should exist");
            anyhow::Ok((rowid, embedding))
        })
        .collect::<Result<Vec<_>>>()
        .context("Error while parsing embeddings from db")?;

    let (ids, embeddings) = id_embeddings.into_iter().unzip::<_, _, Vec<_>, Vec<_>>();

    let ids: Vec<Idx> = ids
        .iter()
        .map(|id| {
            let id: u64 = (*id).try_into().context("Failed to convert i64 to i32")?;
            let id: Idx = Idx::new(id);
            Ok(id)
        })
        .collect::<Result<Vec<_>>>()?;

    let flattened_embeddings: Vec<f32> = embeddings
        .iter()
        .flat_map(|embedding| embedding.to_owned())
        .collect();

    // This may be expensive but I'm not sure how add_with_ids behaves on conflict
    index.reset().context("Failed to reset index")?;
    index
        .add_with_ids(flattened_embeddings.as_slice(), ids.as_slice())
        .context("Failed to add embeddings to index")?;

    debug!("Index synchronized");

    Ok(())
}

#[derive(sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct PathAndTimestamp {
    file_path: String,
    starting_timestamp: f64,
}

pub async fn get_knn(
    search_string_embedding: &[f32],
    pool: &SqlitePool,
    vector_index: &mut IdMap<FlatIndex>,
) -> Result<Vec<PathAndTimestamp>> {
    debug!("Getting knn for embedding of size {}", search_string_embedding.len());
    const K_LIMIT: usize = 10;

    debug!("Searching vector index...");
    // TODO: This hard crashes with no message.
    let search_result = vector_index
        .assign(search_string_embedding, K_LIMIT)
        .map_err(|e| anyhow!("Failed to get knn: ".to_owned() + &e.to_string()))?;
    debug!("Got search results.");

    let rowids = search_result
        .labels
        .iter()
        .map(|label| label.get().context("Failed to get rowid from label"))
        .collect::<Result<Vec<_>>>()?;

    let path_and_timestamp_futures = rowids
        .iter()
        .map(|rowid| (rowid, pool.clone()))
        .map(|(rowid, pool)| async move {
            let rowid = *rowid as i64;
            sqlx::query_as!(
                PathAndTimestamp,
                r#"
            SELECT
                af.file_path,
                afs.starting_timestamp
            FROM audio_file_segment afs 
                JOIN audio_file af ON afs.file_hash = af.file_hash
            WHERE afs.rowid == ?
            "#,
                rowid
            )
            .fetch_one(&pool)
            .await
            .context(format!(
                "Failed to fetch path and timestamp from database for rowid {}",
                rowid
            ))
        })
        .collect::<Vec<_>>();

    let path_and_timestamps = join_all(path_and_timestamp_futures)
        .await
        .into_iter()
        .collect::<Result<Vec<_>>>()
        .context("Failed to get path and timestamps")?;

    Ok(path_and_timestamps)
}
