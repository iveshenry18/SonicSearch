use std::ops::DerefMut;

use chrono::{DateTime, Utc};
use log::trace;
use tauri::AppHandle;

use tauri_specta::Event;
use tokio::sync::RwLock;

#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct PreIndexingProgress {
    pub(crate) started_preindexing: DateTime<Utc>,
    pub(crate) preindexed: u32,
}

#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct IndexingProgress {
    pub(crate) started_indexing: DateTime<Utc>,
    pub(crate) newly_indexed: u32,
    pub(crate) total_to_index: u32,
}

#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct Progress {
    preindexing: PreIndexingProgress,
    indexing: Option<IndexingProgress>,
    total: u32,
}

#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub enum Status {
    Started,
    InProgress(Progress),
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

    pub async fn set_preindexing_started(&self, total: u32) -> tauri::Result<()> {
        let new_status = Status::InProgress(Progress {
            total,
            indexing: None,
            preindexing: PreIndexingProgress {
                started_preindexing: Utc::now(),
                preindexed: 0,
            },
        });
        *self.status.write().await.deref_mut() = new_status.clone();
        IndexingStatusChanged(new_status.clone()).emit_all(&self.app_handle)
    }

    pub async fn increment_preindexed(&self) -> Result<(), String> {
        let mut status = self.status.write().await;
        if let Status::InProgress(progress) = status.deref_mut() {
            progress.preindexing.preindexed += 1;
            trace!(
                "indexed: {}, total: {}, percent: {}",
                progress.preindexing.preindexed,
                progress.total,
                progress.preindexing.preindexed % (progress.total / 100)
            );
            // Only emit update on the percentiles
            if progress.preindexing.preindexed % (progress.total / 100) == 0 {
                IndexingStatusChanged(status.clone())
                    .emit_all(&self.app_handle)
                    .map_err(|err| err.to_string())
            } else {
                Ok(())
            }
        } else {
            // TODO: could put this guard rail at the type level
            Err("Cannot increment preindex if not in progress".to_string())
        }
    }

    pub async fn set_indexing_started(&self, total_to_index: u32) -> Result<(), String> {
        let mut status = self.status.write().await;
        if let Status::InProgress(progress) = status.deref_mut() {
            progress.indexing = Some(IndexingProgress {
                started_indexing: Utc::now(),
                newly_indexed: 0,
                total_to_index,
            });
            IndexingStatusChanged(status.clone())
                .emit_all(&self.app_handle)
                .map_err(|err| err.to_string())
        } else {
            Err("Cannot set indexing started if not indexing".to_string())
        }
    }

    pub async fn increment_indexed(&self) -> Result<(), String> {
        let mut status = self.status.write().await;
        // If we're indexing, increment the number of indexed files
        if let Status::InProgress(progress) = status.deref_mut() {
            if let Some(indexing_progress) = &mut progress.indexing {
                indexing_progress.newly_indexed += 1;
                trace!(
                    "indexed: {}, total: {}, percent: {}",
                    indexing_progress.newly_indexed,
                    indexing_progress.total_to_index,
                    indexing_progress.newly_indexed % (indexing_progress.total_to_index / 100)
                );
                // Only emit update on the percentiles
                if indexing_progress.newly_indexed % (indexing_progress.total_to_index / 100) == 0 {
                    IndexingStatusChanged(status.clone())
                        .emit_all(&self.app_handle)
                        .map_err(|err| err.to_string())
                } else {
                    Ok(())
                }
            } else {
                Err("Cannot increment indexed if not indexing".to_string())
            }
        } else {
            Err("Cannot increment indexed if not in progress".to_string())
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
