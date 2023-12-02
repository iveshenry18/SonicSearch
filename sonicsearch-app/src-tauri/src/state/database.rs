use std::fs;

use anyhow::{Context, Result};
use faiss::{FlatIndex, IdMap};
use sqlx::SqlitePool;
use tauri::AppHandle;

pub mod vector_index;

pub async fn initialize_database(
    app_handle: &AppHandle,
    vector_index: &mut IdMap<FlatIndex>,
) -> Result<SqlitePool> {
    println!("Setting up database...");

    let app_dir = app_handle
        .path_resolver()
        .app_data_dir()
        .expect("The app data directory should exist.");
    println!("App data directory: {:?}", app_dir);
    fs::create_dir_all(&app_dir).expect("The app data directory should be created.");

    let sqlite_path = app_dir.join("SonicSearch.sqlite");
    let pool = SqlitePool::connect(sqlite_path.to_str().expect("sqlite_path should exist"))
        .await
        .context("Failed to open database")?;

    sqlx::migrate!()
        .run(&pool)
        .await
        .context("Error during migration")?;

    vector_index::synchronize_index(&pool, vector_index)
        .await
        .context("Failed to synchronize virtual table")?;

    println!("SonicSearch.db created and initialized.");

    Ok(pool)
}

pub fn encode_embedding(embedding: &[f32]) -> Vec<u8> {
    embedding
        .iter()
        .flat_map(|coord| f32::to_ne_bytes(*coord))
        .collect::<Vec<u8>>()
}

pub fn decode_embedding(db_embedding: &[u8]) -> Result<Vec<f32>> {
    if db_embedding.len() % 4 != 0 {
        return Err(anyhow::anyhow!(
            "Could not decode: Embedding length {} is not a multiple of 4",
            db_embedding.len()
        ));
    }
    db_embedding
        .chunks_exact(4)
        .map(|chunk| {
            let mut bytes = [0; 4];
            bytes.copy_from_slice(chunk);
            Ok(f32::from_ne_bytes(bytes))
        })
        .collect::<Result<Vec<f32>>>()
}
