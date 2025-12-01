use async_trait::async_trait;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

use super::{get_file_name, read_folder};
use crate::items::{FileIcon, Item};
use crate::ui::thumbnail_view::{FOLDER_PLACEHOLDER, RgbaData};

pub struct ItemFolder {
    pub path: PathBuf,
    label: String,
}

impl ItemFolder {
    pub fn new(path: PathBuf) -> Self {
        let label = get_file_name(&path).to_string();
        Self { path, label }
    }
}

#[async_trait]
impl Item for ItemFolder {
    fn get_label(&self) -> String {
        self.label.clone()
    }

    fn get_file_icon(&self) -> FileIcon {
        FileIcon::Folder
    }

    fn is_container(&self) -> bool {
        true
    }

    fn get_file_path(&self) -> PathBuf {
        // Return just the folder name for navigation
        PathBuf::from(&self.label)
    }

    fn get_full_path(&self) -> Option<PathBuf> {
        Some(self.path.clone())
    }

    fn get_sync_thumbnail(&self) -> Option<RgbaData> {
        // Folders can provide their thumbnail synchronously - no async loading needed
        Some(FOLDER_PLACEHOLDER.clone())
    }

    async fn get_thumbnail_preview(&self, _cancel_token: &CancellationToken) -> Option<RgbaData> {
        // Return None - use get_sync_thumbnail() instead
        None
    }

    async fn get_subitems(&self, _cancel_token: &CancellationToken) -> Option<Vec<Box<dyn Item>>> {
        let path = self.path.clone();
        Some(match read_folder(&path) {
            Ok(items) => items,
            Err(err) => {
                log::error!("Failed to read folder: {:?}", err);
                Vec::new()
            }
        })
    }

    fn clone_box(&self) -> Box<dyn Item> {
        Box::new(ItemFolder {
            path: self.path.clone(),
            label: self.label.clone(),
        })
    }
}
