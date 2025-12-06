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
