use std::ops::DerefMut;

use tauri::AppHandle;

use tauri_specta::Event;
use tokio::sync::RwLock;

#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct IndexingProgress {
    pub(crate) total_duration: Option<f64>,
    pub(crate) duration_completed: Option<f64>,
}

#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub enum Status {
    Indexing(IndexingProgress),
    Idle,
}

pub struct IndexingStatus {
    pub(crate) status: RwLock<Status>,
    pub(crate) app_handle: AppHandle,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type, tauri_specta::Event)]
pub struct IndexingStatusChanged(Status);

impl IndexingStatus {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            status: RwLock::new(Status::Idle),
            app_handle,
        }
    }

    pub async fn set_indexing(&self, progress: IndexingProgress) -> tauri::Result<()> {
        let new_status = Status::Indexing(progress);
        *self.status.write().await.deref_mut() = new_status.clone();
        IndexingStatusChanged(new_status.clone()).emit_all(&self.app_handle)
    }

    pub async fn set_idle(&self) -> tauri::Result<()> {
        let new_status = Status::Idle;
        *self.status.write().await.deref_mut() = new_status.clone();
        IndexingStatusChanged(new_status.clone()).emit_all(&self.app_handle)
    }

    pub async fn get_status(&self) -> Status {
        self.status.read().await.clone()
    }
}
