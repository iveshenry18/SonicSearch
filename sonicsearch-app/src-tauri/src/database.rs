use std::fs;

use anyhow::{Context, Result};
use rusqlite::ffi::sqlite3_auto_extension;
use sqlite_vss::{sqlite3_vector_init, sqlite3_vss_init};
use sqlx::SqlitePool;
use tauri::AppHandle;

pub async fn initialize_database(app_handle: &AppHandle) -> Result<SqlitePool> {
    println!("Setting up database...");
    unsafe {
        sqlite3_auto_extension(Some(sqlite3_vector_init));
        sqlite3_auto_extension(Some(sqlite3_vss_init));
    }

    let app_dir = app_handle
        .path_resolver()
        .app_data_dir()
        .expect("The app data directory should exist.");
    fs::create_dir_all(&app_dir).expect("The app data directory should be created.");

    let sqlite_path = app_dir.join("SonicSearch.sqlite");
    let pool = SqlitePool::connect(sqlite_path.to_str().expect("sqlite_path should exist"))
        .await
        .context("Failed to open database")?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS audio_file (
            file_hash TEXT PRIMARY KEY,
            file_path TEXT NOT NULL
        )"#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS audio_file_segment (
            file_hash TEXT,
            starting_timestamp REAL NOT NULL,            
            embedding BLOB NOT NULL,
            FOREIGN KEY (file_hash) REFERENCES audio_file(file_hash),
            PRIMARY KEY (file_hash, starting_timestamp)
        )"#,
    )
    .execute(&pool)
    .await?;

    println!("SonicSearch.db created and initialized.");

    Ok(pool)
}
