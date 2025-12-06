use crate::items::Item;
use async_trait::async_trait;
use std::path::PathBuf;

/// An item (file) inside an archive
#[derive(Clone)]
pub struct ArchiveItem {
    path: PathBuf,
    data: Vec<u8>,
}

impl ArchiveItem {
    pub fn new(path: PathBuf, data: Vec<u8>) -> Self {
        Self { path, data }
    }
}

#[async_trait]
impl Item for ArchiveItem {
    fn get_label(&self) -> String {
        // Show only the filename, not the full path
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.path.to_string_lossy().to_string())
    }

    fn get_file_path(&self) -> String {
        // Return just the filename for navigation
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.path.to_string_lossy().to_string())
    }

    async fn read_data(&self) -> Option<Vec<u8>> {
        Some(self.data.clone())
    }

    fn clone_box(&self) -> Box<dyn Item> {
        Box::new(self.clone())
    }
}
