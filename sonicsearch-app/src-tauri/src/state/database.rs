use std::fs;

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use tauri::AppHandle;

pub mod vector_index;

const EMBEDDING_SIZE: u16 = 512;

pub async fn initialize_database(app_handle: &AppHandle) -> Result<SqlitePool> {
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

    vector_index::synchronize_index(&pool)
        .await
        .context("Failed to synchronize virtual table")?;

    println!("SonicSearch.db created and initialized.");

    Ok(pool)
}

pub fn encode_embedding(embedding: &[f32]) -> Result<String> {
    Ok(serde_json::to_string(&(embedding.to_owned()))?)
}
