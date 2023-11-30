use std::fs;

use anyhow::{Context, Result};
use rusqlite::ffi::sqlite3_auto_extension;
use sqlite_vss::{sqlite3_vector_init, sqlite3_vss_init};
use sqlx::SqlitePool;
use tauri::AppHandle;

/// This is a manual trigger function to update the vss_audio_file_segment
/// virtual table with values from the audio_file_segment table.
pub async fn synchronize_audio_file_segment_vss(pool: &SqlitePool) -> Result<()> {
    // Cannot use macro here because virtual table is not part of
    // the sqlx-managed migration
    sqlx::query(
        r#"INSERT INTO vss_audio_file_segment (rowid, embedding)
            SELECT rowid, embedding FROM audio_file_segment"#,
    )
    .execute(pool)
    .await
    .context("Failed to update vss_audio_file_segment: {:?}")?;
    Ok(())
}

const EMBEDDING_SIZE: u16 = 512;

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
    println!("App data directory: {:?}", app_dir);
    fs::create_dir_all(&app_dir).expect("The app data directory should be created.");

    let sqlite_path = app_dir.join("SonicSearch.sqlite");
    let pool = SqlitePool::connect(sqlite_path.to_str().expect("sqlite_path should exist"))
        .await
        .context("Failed to open database")?;

    sqlx::migrate!().run(&pool).await?;
    sqlx::query(
        r#"CREATE VIRTUAL TABLE IF NOT EXISTS vss_audio_file_segment USING vss0(
            embedding(?)
        )"#,
    )
    .bind(EMBEDDING_SIZE)
    .execute(&pool)
    .await?;

    synchronize_audio_file_segment_vss(&pool).await?;

    println!("SonicSearch.db created and initialized.");

    Ok(pool)
}
