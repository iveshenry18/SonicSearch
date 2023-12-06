use std::path::PathBuf;

use anyhow::Result;
use sqlx::SqlitePool;
use tauri::State;

use crate::{audio_index::update_audio_index, state::AppState};

/// Add a path to the index
#[tauri::command]
#[specta::specta]
pub async fn add_path_to_index(
    app_state: State<'_, AppState>,
    path: String,
) -> Result<Vec<PathBuf>, String> {
    let parsed_path = parse_path(&path).map_err(|e| e.to_string())?;
    add_path_to_db(&app_state.pool, parsed_path).await?;
    // TODO: Dangerous clone!
    update_audio_index(app_state.clone())
        .await
        .map_err(|e| e.to_string())?;
    get_paths_from_index(app_state)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn add_paths_to_index(
    app_state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<Vec<PathBuf>, String> {
    for path in &paths {
        let parsed_path = parse_path(path)?;
        let pool = app_state.pool.clone();
        add_path_to_db(&pool, parsed_path).await?;
    }
    // TODO: Dangerous clone!
    update_audio_index(app_state.clone())
        .await
        .map_err(|e| e.to_string())?;
    get_paths_from_index(app_state)
        .await
        .map_err(|e| e.to_string())
}

async fn add_path_to_db(pool: &SqlitePool, path: PathBuf) -> Result<(), String> {
    let path = path.to_str().ok_or("Path is not valid UTF-8")?;
    sqlx::query!("INSERT INTO dir_paths (path) VALUES (?)", path)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn parse_path(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::new().join(path);
    match path.try_exists().map_err(|err| {
        format!(
            "Error while checking existence of path {:?} does not exist: {:?}",
            path, err
        )
    })? {
        true => Ok(path),
        false => Err(format!(
            "Path does not exist: {}",
            path.to_str().ok_or("[unparseable path]")?
        )),
    }
}

/// Get all paths from the index
#[tauri::command]
#[specta::specta]
pub async fn get_paths_from_index(app_state: State<'_, AppState>) -> Result<Vec<PathBuf>, String> {
    get_paths_from_db(&app_state.pool).await
}

async fn get_paths_from_db(pool: &SqlitePool) -> Result<Vec<PathBuf>, String> {
    let paths = sqlx::query!("SELECT path FROM dir_paths")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
    let paths = paths
        .into_iter()
        .map(|path| PathBuf::new().join(path.path))
        .collect();
    Ok(paths)
}

/// Delete a path from the index
#[tauri::command]
#[specta::specta]
pub async fn delete_path_from_index(
    app_state: State<'_, AppState>,
    path: String,
) -> Result<Vec<PathBuf>, String> {
    let parsed_path = parse_path(&path).map_err(|e| e.to_string())?;
    delete_path_from_db(&app_state.pool, parsed_path).await?;
    get_paths_from_db(&app_state.pool).await
}

async fn delete_path_from_db(pool: &SqlitePool, path: PathBuf) -> Result<(), String> {
    let path = path.to_str();
    sqlx::query!("DELETE FROM dir_paths WHERE path = ?", path)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
