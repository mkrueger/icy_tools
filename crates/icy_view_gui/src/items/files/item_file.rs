use async_trait::async_trait;
use std::path::PathBuf;

use super::get_file_name;
use crate::items::Item;

pub struct ItemFile {
    path: PathBuf,
    label: String,
}

impl ItemFile {
    pub fn new(path: PathBuf) -> Self {
        let label = get_file_name(&path).to_string();
        Self { path, label }
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

    async fn read_data(&self) -> Option<Vec<u8>> {
        let path = self.path.clone();
        tokio::fs::read(&path).await.ok()
    }

    fn clone_box(&self) -> Box<dyn Item> {
        Box::new(ItemFile {
            path: self.path.clone(),
            label: self.label.clone(),
        })
    }
}
