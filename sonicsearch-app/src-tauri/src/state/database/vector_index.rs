use anyhow::{Context, Result};

use futures::future::join_all;
use hnsw_rs::{dist::DistCosine, hnsw::Hnsw};
use log::debug;
use sqlx::SqlitePool;

use crate::state::database::decode_embedding;

const DEFAULT_NB_ELEM: usize = 5_000;
const MAX_NB_CONNECTION: usize = 16;
const EF_C: usize = 200;
const K_LIMIT: usize = 10;
const EF_ARG: usize = 12;
const DEFAULT_NB_LAYER: usize = 8;

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
    let nb_layer = DEFAULT_NB_LAYER.min((nb_elem as f32).ln().trunc() as usize);

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
    vector_index.index.set_searching_mode(false);
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
        "{} total embeddings",
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
    debug!(
        "{} new embeddings to add to the index",
        new_id_embeddings.len()
    );

    debug!("Adding embeddings to index");
    vector_index.index.parallel_insert(&new_id_embeddings);

    debug!("Marking embeddings as indexed");
    let newly_indexed_ids = new_id_embeddings
        .iter()
        .map(|(_, rowid)| rowid)
        .collect::<Vec<_>>();
    vector_index.indexed_ids.extend(newly_indexed_ids);

    vector_index.index.set_searching_mode(true);

    debug!("Index synchronized");

    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SearchResult {
    file_path: String,
    starting_timestamp: f64,
    distance: f32,
}

#[derive(sqlx::FromRow)]
struct SearchRow {
    file_path: String,
    starting_timestamp: f64,
}

/// Returns K_LIMIT nearest neighbours of the given embedding
/// in order of increasing distance.
pub async fn get_knn(
    search_string_embedding: &[f32],
    pool: &SqlitePool,
    vector_index: &VectorIndex,
) -> Result<Vec<SearchResult>> {
    debug!(
        "Getting knn for embedding of size {}",
        search_string_embedding.len()
    );

    debug!("Searching vector index...");
    let mut neighbors = vector_index
        .index
        .search(search_string_embedding, K_LIMIT, EF_ARG);
    neighbors.sort_by(|a, b| {
        a.distance
            .partial_cmp(&b.distance)
            .expect("Distance should be comparable")
    });

    let search_result_futures = neighbors
        .iter()
        .map(|neighbor| (neighbor, pool.clone()))
        .map(|(neighbor, pool)| async move {
            let rowid = neighbor.d_id as i64;
            let search_rows = sqlx::query_as!(
                SearchRow,
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
            ))?;
            Ok(SearchResult {
                file_path: search_rows.file_path,
                starting_timestamp: search_rows.starting_timestamp,
                distance: neighbor.distance,
            })
        })
        .collect::<Vec<_>>();

    let search_results = join_all(search_result_futures)
        .await
        .into_iter()
        .collect::<Result<Vec<_>>>()
        .context("Failed to get path and timestamps")?;

    Ok(search_results)
}
