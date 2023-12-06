use std::ops::DerefMut;

use log::trace;
use tauri::AppHandle;

use tauri_specta::Event;
use tokio::sync::RwLock;

#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct PreIndexingProgress {
    pub(crate) total: u32,
    pub(crate) preindexed: u32,
}

#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct IndexingProgress {
    pub(crate) total: u32,
    pub(crate) indexed: u32,
}

#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub enum Status {
    Started,
    PreIndexing(PreIndexingProgress),
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

    pub async fn set_started(&self) -> tauri::Result<()> {
        let new_status = Status::Started;
        *self.status.write().await.deref_mut() = new_status.clone();
        IndexingStatusChanged(new_status.clone()).emit_all(&self.app_handle)
    }

    pub async fn set_preindexing_start(&self, total: u32) -> tauri::Result<()> {
        let new_status = Status::PreIndexing(PreIndexingProgress {
            total,
            preindexed: 0,
        });
        *self.status.write().await.deref_mut() = new_status.clone();
        IndexingStatusChanged(new_status.clone()).emit_all(&self.app_handle)
    }

    pub async fn increment_preindexed(&self) -> Result<(), String> {
        let mut status = self.status.write().await;
        if let Status::PreIndexing(progress) = status.deref_mut() {
            progress.preindexed += 1;
            // Only emit update on the percentiles
            trace!(
                "preindexed: {}, total: {}, percent: {}",
                progress.preindexed,
                progress.total,
                progress.preindexed % (progress.total / 100)
            );
            if progress.preindexed % (progress.total / 100) == 0 {
                IndexingStatusChanged(status.clone())
                    .emit_all(&self.app_handle)
                    .map_err(|err| err.to_string())
            } else {
                Ok(())
            }
        } else {
            // TODO: could put this guard rail at the type level
            Err("Cannot increment preindexed if not preindexing".to_string())
        }
    }

    pub async fn set_indexing_started(&self, total: u32) -> tauri::Result<()> {
        let new_status = Status::Indexing(IndexingProgress { total, indexed: 0 });
        *self.status.write().await.deref_mut() = new_status.clone();
        IndexingStatusChanged(new_status.clone()).emit_all(&self.app_handle)
    }

    pub async fn increment_indexed(&self) -> Result<(), String> {
        let mut status = self.status.write().await;
        if let Status::Indexing(progress) = status.deref_mut() {
            progress.indexed += 1;
            trace!(
                "indexed: {}, total: {}, percent: {}",
                progress.indexed,
                progress.total,
                progress.indexed % (progress.total / 100)
            );
            // Only emit update on the percentiles
            if progress.indexed % (progress.total / 100) == 0 {
                IndexingStatusChanged(status.clone())
                    .emit_all(&self.app_handle)
                    .map_err(|err| err.to_string())
            } else {
                Ok(())
            }
        } else {
            // TODO: could put this guard rail at the type level
            Err("Cannot increment indexed if not indexing".to_string())
        }
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
