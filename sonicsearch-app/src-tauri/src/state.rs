use std::sync::Mutex;

use ort::Session;
use sqlx::SqlitePool;

pub struct AppState {
    pub pool: SqlitePool,
    pub clap_model_text_embedder: Mutex<Session>,
    pub clap_model_audio_embedder: Mutex<Session>,
    pub is_indexing: Mutex<bool>,
}

// For reference... later

// pub trait ServiceAccess {
//     fn db<F, TResult>(&self, operation: F) -> TResult
//     where
//         F: FnOnce(&SqlitePool) -> TResult;

//     fn db_mut<F, TResult>(&self, operation: F) -> TResult
//     where
//         F: FnOnce(&mut SqlitePool) -> TResult;
// }

// impl ServiceAccess for AppHandle {
//     fn db<F, TResult>(&self, operation: F) -> TResult
//     where
//         F: FnOnce(&SqlitePool) -> TResult,
//     {
//         let app_state: State<AppState> = self.state();
//         let db_connection_guard = app_state.pool.lock().unwrap();
//         let db = db_connection_guard.as_ref().unwrap();

//         operation(db)
//     }

//     fn db_mut<F, TResult>(&self, operation: F) -> TResult
//     where
//         F: FnOnce(&mut SqlitePool) -> TResult,
//     {
//         let app_state: State<AppState> = self.state();
//         let mut db_connection_guard = app_state.pool.lock().unwrap();
//         let db = db_connection_guard.as_mut().unwrap();

//         operation(db)
//     }
// }
