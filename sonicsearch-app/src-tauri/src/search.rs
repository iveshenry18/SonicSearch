use std::result;

use anyhow::{anyhow, Context, Result};
use faiss::{FlatIndex, IdMap};
use log::{debug, info, warn};
use ndarray::{arr1, Axis, CowArray};
use ort::Session;
use sqlx::SqlitePool;
use tauri::{AppHandle, PathResolver};
use tokenizers::{tokenizer::Tokenizer, Encoding};

use crate::state::{
    database::vector_index::{self, PathAndTimestamp},
    AppState,
};

#[tauri::command]
pub async fn search_index(
    app_state: tauri::State<'_, AppState>,
    app_handle: AppHandle,
    search_string: &str,
) -> result::Result<Vec<PathAndTimestamp>, String> {
    info!("Searching for: {}", search_string);
    let text_embedder = app_state.clap_model_text_embedder.lock().await;
    let mut locked_vector_index = app_state.vector_index.write().await;
    debug!("Got text embedder lock");
    get_search_results(
        search_string,
        &app_state.pool.clone(),
        &mut locked_vector_index,
        &text_embedder,
        &app_handle,
    )
    .await
    .map_err(|e| {
        warn!("Error during search: {:?}", e);
        format!("ERROR during search: {:?}", e.to_string())
    })
}

async fn get_search_results(
    search_string: &str,
    pool: &SqlitePool,
    vector_index: &mut IdMap<FlatIndex>,
    text_embedder: &Session,
    app_handle: &AppHandle,
) -> Result<Vec<vector_index::PathAndTimestamp>> {
    debug!("Preprocessing search string: {}", search_string);
    let preprocessed_search_string = preprocess_search_string(search_string);
    debug!("Tokenizing search string: {}", preprocessed_search_string);
    let search_string_encoding = tokenize(preprocessed_search_string, &app_handle.path_resolver())
        .map_err(|e| anyhow!(e.to_string()))?;
    debug!("Embedding encoding {:?}", search_string_encoding);
    let embedded_search_string = embed(search_string_encoding, text_embedder).await?;
    debug!(
        "Searching with embedding of size {}",
        embedded_search_string.len()
    );
    vector_index::get_knn(&embedded_search_string, pool, vector_index).await
}

async fn embed(
    search_string_encoding: Encoding,
    text_embedder_session: &Session,
) -> Result<Vec<f32>> {
    let input_ids = CowArray::from(
        arr1(search_string_encoding.get_ids())
            .mapv(|x| x as i64)
            .insert_axis(Axis(0)) // Fake batch
            .into_dyn(),
    );
    let attention_mask = CowArray::from(
        arr1(search_string_encoding.get_attention_mask())
            .mapv(|x| x as i64)
            .insert_axis(Axis(0)) // Fake batch
            .into_dyn(),
    );
    let outputs = text_embedder_session
        .run(vec![
            ort::Value::from_array(text_embedder_session.allocator(), &input_ids)
                .context("Failed to create ort::Value from array of input_ids")?,
            ort::Value::from_array(text_embedder_session.allocator(), &attention_mask)
                .context("Failed to create ort::Value from array of attention_mask")?,
        ])
        .context("Failed to run session")?;

    let embedding = outputs
        .get(0)
        .context("Output 0 should contain embeddings")?
        .try_extract::<f32>()
        .context("Failed to extract embeddings")?
        .view()
        .axis_iter(Axis(0))
        .collect::<Vec<_>>()
        .get(0)
        .context("Failed to get embedding of first in \"batch\"")?
        .to_shape((512,))
        .context("Failed to reshape output")?
        .to_vec();

    Ok(embedding)
}

fn tokenize(
    preprocessed_search_string: String,
    path_resolver: &PathResolver,
) -> tokenizers::Result<Encoding> {
    // TODO: Move tokenizer to state
    let tokenizer_json_filename = "onnx_models/tokenizer/tokenizer.json";
    let tokenizer_json_path = path_resolver
        .resolve_resource(tokenizer_json_filename)
        .unwrap_or_else(|| panic!("Model path {} should resolve.", tokenizer_json_filename));
    let tokenizer = Tokenizer::from_file(tokenizer_json_path.to_str().context(format!(
        "Failed to convert path {} to str",
        tokenizer_json_path.display()
    ))?)?;

    tokenizer.encode(preprocessed_search_string, false)
}

/// If search string is short, add "The sound of {}" to the beginning of the string
fn preprocess_search_string(search_string: &str) -> String {
    const MIN_SEARCH_STRING_LENGTH: usize = 10;
    match search_string.len() {
        0..=MIN_SEARCH_STRING_LENGTH => format!("The sound of {}", search_string),
        _ => search_string.to_string(),
    }
}
