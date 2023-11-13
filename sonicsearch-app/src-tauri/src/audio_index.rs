use std::hash::Hasher;
use std::path::PathBuf;
use std::result;
use std::{
    fs::File,
    io::{self, BufReader, Read},
    path::Path,
};

use anyhow::{Context, Result};
use futures::future::join_all;
use sqlx::SqlitePool;
use tauri::State;
use twox_hash::XxHash64;
use walkdir::WalkDir;

use crate::state::AppState;

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

#[tauri::command]
pub async fn update_audio_index(app_state: State<'_, AppState>) -> result::Result<(), ()> {
    println!("Updating audio file index...");
    let user_audio_dir = dirs::audio_dir().expect("Failed to get user home directory");

    let upsert_futures: Vec<_> = WalkDir::new(user_audio_dir)
        .into_iter()
        .map(|dir| (dir, app_state.pool.clone()))
        .map(|(dir, pool)| async move {
            let dir = dir.as_ref().ok();
            if dir.is_some_and(|entry| entry.path().is_file() && is_audio_file(entry.path())) {
                Some(Box::pin(upsert_audio_file(
                    pool.to_owned(),
                    dir.expect("dir should exist").path().to_owned(),
                )))
            } else {
                None
            }
        })
        .collect();

    join_all(upsert_futures)
        .await
        .into_iter()
        .for_each(|result| {
            result.ok_or_else(|| println!("Failed to spawn upsert_audio_file task"));
        });

    println!("\nAudio file index updated.");
    Ok(())
}

#[derive(sqlx::FromRow)]
struct AudioFile {
    file_hash: String,
    file_path: String,
}

pub async fn upsert_audio_file(pool: SqlitePool, path: PathBuf) -> Result<()> {
    println!("Indexing {} ", path.display());
    let file_hash = compute_hash(&path).context("Failed to compute hash")?;
    let file_path = path.to_string_lossy().into_owned();
    let existing_row: Option<AudioFile> =
        sqlx::query_as(r#"SELECT * FROM audio_file WHERE file_hash = ?"#)
            .bind(file_hash)
            .fetch_optional(&pool)
            .await?;

    if existing_row.is_none() {
        println!("TODO: index_new_file for {}", path.display());
        // index_new_file(pool, file_hash, file_path).await?;
    } else if existing_row.is_some_and(|row| row.file_path != file_path) {
        println!("TODO: update path for {}", path.display());
        // update_path(pool, file_hash, file_path).await?;
    } else {
        println!("{} already indexed.", path.display());
    }

    Ok(())
}

pub fn get_search_results(_search_string: &str, pool: &SqlitePool) -> Result<Vec<String>> {
    // Stubbed search algorithm: pick 10 random audio files

    Ok(vec!["~/fake_path.wav".to_string()])
}
