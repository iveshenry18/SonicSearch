use anyhow::{Context, Result};
use dirs;
use rusqlite::{ffi::sqlite3_auto_extension, params, Connection};
use sqlite_vss::{sqlite3_vector_init, sqlite3_vss_init};
use std::hash::Hasher;
use std::{
    fs::{self, File},
    io::{self, BufReader, Read},
    path::Path,
};
use tauri::AppHandle;
use twox_hash::XxHash64;
use walkdir::WalkDir;

pub fn initialize_database(app_handle: &AppHandle) -> Result<Connection> {
    println!("Setting up database...");
    unsafe {
        sqlite3_auto_extension(Some(sqlite3_vector_init));
        sqlite3_auto_extension(Some(sqlite3_vss_init));
    }
    let app_dir = app_handle.path_resolver().app_data_dir().expect("The app data directory should exist.");
    fs::create_dir_all(&app_dir).expect("The app data directory should be created.");
    let sqlite_path = app_dir.join("SonicSearch.sqlite");

    let conn = Connection::open(sqlite_path).context("Failed to open database")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS audio_file (
            file_hash TEXT PRIMARY KEY,
            file_path TEXT NOT NULL,            
            embedding BLOB NOT NULL
        )",
        [],
    )?;

    println!("SonicSearch.db created.");
    Ok(conn)
}

fn compute_hash(file_path: &Path) -> io::Result<String> {
    let hash_seed = 1023489u64;
    let mut hasher = XxHash64::with_seed(hash_seed);
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);

    let mut buffer = [0; 1024]; // Read in chunks of 1024 bytes
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.write(&buffer[..n]);
    }

    Ok(hasher.finish().to_string())
}

fn is_audio_file(path: &Path) -> bool {
    // Implement logic to determine if a file is an audio file
    // This could be by extension or by inspecting the file content
    path.extension()
        .map(|ext| {
            let ext = ext.to_str().unwrap_or("").to_lowercase();
            // Define your supported audio file extensions here
            ["mp3", "wav", "flac", "m4a", "aiff"].contains(&ext.as_str())
        })
        .unwrap_or(false)
}

pub fn update_audio_file_index(conn: &Connection) -> Result<()> {
    println!("Updating audio file index...");
    let user_audio_dir = dirs::audio_dir().expect("Failed to get user home directory");

    for entry in WalkDir::new(user_audio_dir) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                eprintln!("Warning: {}", e); // Optionally log the permissions error
                continue; // Skip the current entry and move to the next one
            }
        };

        let path = entry.path();

        if path.is_file() && is_audio_file(path) {
            println!("Indexing {} ", path.display());
            let file_hash = compute_hash(&path).context("Failed to compute hash")?;
            let file_path = path.to_string_lossy().into_owned();
            let embedding_stub = vec![12u8; 32];

            // Insert if new. If hash exists, update path but not embedding.
            conn.execute(
                "INSERT INTO audio_file (file_hash, file_path, embedding)
                     VALUES (?1, ?2, ?3)
                     ON CONFLICT(file_hash) DO UPDATE SET file_path = ?2",
                params![file_hash, file_path, embedding_stub],
            )?;
        }
    }

    println!("\nAudio file index updated.");
    Ok(())
}

pub fn get_search_results(_search_string: &str, db: &Connection) -> Result<Vec<String>, rusqlite::Error> {
    // Stubbed search algorithm: pick 10 random audio files
    let mut stmt = db.prepare("SELECT file_path FROM audio_file ORDER BY RANDOM() LIMIT 10")?;
    let mut rows = stmt.query([])?;
    let mut file_paths = Vec::new();
    while let Some(row) = rows.next()? {
        let file_path: String = row.get(0)?;
        file_paths.push(file_path);
    }
    
    Ok(file_paths)
}