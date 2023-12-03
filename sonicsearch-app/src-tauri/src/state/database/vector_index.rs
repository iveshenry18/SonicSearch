use anyhow::{Context, Result};

use futures::future::join_all;
use hnsw_rs::{dist::DistCosine, hnsw::Hnsw};
use log::debug;
use sqlx::SqlitePool;

use crate::state::database::decode_embedding;

const DEFAULT_NB_ELEM: usize = 5_000;
const MAX_NB_CONNECTION: usize = 24;
const EF_C: usize = 400;
const K_LIMIT: usize = 10;
const EF_ARG: usize = 20;

// "The parameter ef controls the width of the search in the lowest level, it must be greater than number of neighbours asked.
// A rule of thumb could be between knbn and max_nb_connection."
// https://docs.rs/hnsw_rs/latest/hnsw_rs/hnsw/struct.Hnsw.html#method.parallel_insert
#[allow(clippy::assertions_on_constants)]
const _: () = debug_assert!(EF_ARG > K_LIMIT && EF_ARG < MAX_NB_CONNECTION);

pub struct VectorIndex {
    /// The hnsw index
    index: Hnsw<'static, f32, DistCosine>,
    /// The ids of the values currently in the index
    indexed_ids: Vec<usize>,
}

pub fn initialize_index(nb_elem: Option<usize>) -> VectorIndex {
    debug!("Initializing index");

    let nb_elem = nb_elem.unwrap_or(DEFAULT_NB_ELEM);
    let nb_layer = 16.min((nb_elem as f32).ln().trunc() as usize);

    let hnsw =
        Hnsw::<f32, DistCosine>::new(MAX_NB_CONNECTION, nb_elem, nb_layer, EF_C, DistCosine {});
    debug!("Index initialized");

    VectorIndex {
        index: hnsw,
        indexed_ids: Vec::new(),
    }
}

struct IndexRow {
    rowid: Option<i64>,
    embedding: Vec<u8>,
}

/// Synchronize embeddings from the audio_file_segment table
/// to the vector index.
/// This function only indexes embeddings that have not been indexed yet.
pub async fn synchronize_index(pool: &SqlitePool, vector_index: &mut VectorIndex) -> Result<()> {
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

    let new_id_embeddings: Vec<(Vec<f32>, usize)> = id_embeddings
        .iter()
        .map(|row| {
            let embedding: Vec<f32> =
                decode_embedding(&row.embedding).context("Could not decode embeddings")?;
            let rowid = row.rowid.expect("rowid should exist") as usize;
            anyhow::Ok((embedding, rowid))
        })
        .filter(|res| {
            res.as_ref()
                .is_ok_and(|(_, rowid)| !vector_index.indexed_ids.contains(rowid))
        })
        .collect::<Result<Vec<_>>>()
        .context("Error while parsing embeddings from db")?;

    let new_id_embeddings: Vec<(&Vec<f32>, usize)> = new_id_embeddings
        .iter()
        .map(|(embedding, rowid)| (embedding, *rowid))
        .collect::<Vec<_>>();

    debug!("Adding embeddings to index");
    vector_index.index.parallel_insert(&new_id_embeddings);

    debug!("Marking embeddings as indexed");
    let newly_indexed_ids = new_id_embeddings
        .iter()
        .map(|(_, rowid)| rowid)
        .collect::<Vec<_>>();
    vector_index.indexed_ids.extend(newly_indexed_ids);

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
    vector_index: &VectorIndex,
) -> Result<Vec<PathAndTimestamp>> {
    debug!(
        "Getting knn for embedding of size {}",
        search_string_embedding.len()
    );

    debug!("Searching vector index...");
    let search_result = vector_index
        .index
        .search(search_string_embedding, K_LIMIT, EF_ARG);

    let rowids = search_result
        .iter()
        .map(|neighbour| neighbour.d_id)
        .collect::<Vec<_>>();

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
