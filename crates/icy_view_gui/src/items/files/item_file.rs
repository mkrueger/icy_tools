use async_trait::async_trait;
use std::path::PathBuf;
use std::time::SystemTime;

use super::get_file_name;
use crate::items::Item;

pub struct ItemFile {
    path: PathBuf,
    label: String,
    size: Option<u64>,
    modified: Option<SystemTime>,
}

impl ItemFile {
    pub fn new(path: PathBuf) -> Self {
        let label = get_file_name(&path).to_string();
        let metadata = std::fs::metadata(&path).ok();
        let size = metadata.as_ref().and_then(|m| Some(m.len()));
        let modified = metadata.as_ref().and_then(|m| m.modified().ok());
        Self { path, label, size, modified }
    }
}

#[async_trait]
impl Item for ItemFile {
    fn get_label(&self) -> String {
        self.label.clone()
    }

    fn get_file_path(&self) -> PathBuf {
        // Return just the filename for navigation
        PathBuf::from(&self.label)
    }

    fn get_full_path(&self) -> Option<PathBuf> {
        Some(self.path.clone())
    }

    fn get_size(&self) -> Option<u64> {
        self.size
    }

    fn get_modified_time(&self) -> Option<SystemTime> {
        self.modified
    }

    async fn read_data(&self) -> Option<Vec<u8>> {
        let path = self.path.clone();
        match tokio::fs::read(&path).await {
            Ok(data) => Some(data),
            Err(e) => {
                // Log as debug since this can happen for special files, broken symlinks, etc.
                log::debug!("Failed to read file {:?}: {}", path, e);
                None
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Item> {
        Box::new(ItemFile {
            path: self.path.clone(),
            label: self.label.clone(),
            size: self.size,
            modified: self.modified,
        })
    }
}
