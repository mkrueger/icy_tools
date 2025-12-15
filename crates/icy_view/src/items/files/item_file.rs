use async_trait::async_trait;
use icy_sauce::SauceRecord;
use std::path::PathBuf;
use std::time::SystemTime;
use tokio_util::sync::CancellationToken;

use super::get_file_name;
use crate::items::{Item, ItemError};

pub struct ItemFile {
    path: PathBuf,
    label: String,
    size: Option<u64>,
    modified: Option<SystemTime>,
}

impl ItemFile {
    pub fn new(path: String) -> Self {
        let path_buf = PathBuf::from(&path);
        let label = get_file_name(&path_buf).to_string();
        let metadata = std::fs::metadata(&path_buf).ok();
        let size = metadata.as_ref().and_then(|m| Some(m.len()));
        let modified = metadata.as_ref().and_then(|m| m.modified().ok());
        Self {
            path: path_buf,
            label,
            size,
            modified,
        }
    }
}

#[async_trait]
impl Item for ItemFile {
    fn get_label(&self) -> String {
        self.label.clone()
    }

    fn get_file_path(&self) -> String {
        // Return just the filename for navigation
        self.label.clone()
    }

    fn get_full_path(&self) -> Option<String> {
        Some(self.path.to_string_lossy().replace('\\', "/"))
    }

    fn size(&self) -> Option<u64> {
        self.size
    }

    fn get_modified_time(&self) -> Option<SystemTime> {
        self.modified
    }

    async fn read_data(&self) -> Result<Vec<u8>, ItemError> {
        let path = self.path.clone();
        tokio::fs::read(&path)
            .await
            .map_err(|e| ItemError::Io(format!("Failed to read file {:?}: {}", path, e)))
    }

    fn clone_box(&self) -> Box<dyn Item> {
        Box::new(ItemFile {
            path: self.path.clone(),
            label: self.label.clone(),
            size: self.size,
            modified: self.modified,
        })
    }

    /// Get SAUCE info for this item (async for file I/O)
    /// Returns the SAUCE record if the file has one
    async fn get_sauce_info(&self, _cancel_token: &CancellationToken) -> Option<SauceRecord> {
        // Default implementation reads data and extracts SAUCE
        if let Ok(data) = self.read_data().await {
            return SauceRecord::from_bytes(&data).ok().flatten();
        }
        None
    }
}
