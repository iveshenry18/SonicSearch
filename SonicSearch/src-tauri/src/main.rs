// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rusqlite::{ffi::sqlite3_auto_extension, Connection, Result};
use sqlite_vss::{sqlite3_vector_init, sqlite3_vss_init};

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn search(name: &str) -> String {
    format!(
        "Thanks for searching for {}! I'm not gonna find it, but I'm glad you searched.",
        name
    )
}

fn setup_db() -> Result<()> {
    unsafe {
        sqlite3_auto_extension(Some(sqlite3_vector_init));
        sqlite3_auto_extension(Some(sqlite3_vss_init));
    }

    let conn = Connection::open("SonicSearch.db")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS audio_file (
            file_hash TEXT PRIMARY KEY,
            file_path TEXT NOT NULL,            
            embedding BLOB NOT NULL
        )",
        [],
    )?;

    Ok(())
}

fn main() {
    setup_db().expect("Failed to setup database");

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet])
        .invoke_handler(tauri::generate_handler![search])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
