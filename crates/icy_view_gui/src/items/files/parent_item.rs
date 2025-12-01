use async_trait::async_trait;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

use crate::items::{FileIcon, Item};
use crate::ui::thumbnail_view::{FOLDER_PLACEHOLDER, RgbaData};

/// Special item representing the parent directory ("..")
pub struct ParentItem;

#[async_trait]
impl Item for ParentItem {
    fn get_label(&self) -> String {
        "..".to_string()
    }

    fn get_file_icon(&self) -> FileIcon {
        FileIcon::FolderOpen
    }

    fn is_parent(&self) -> bool {
        true
    }

    fn get_file_path(&self) -> PathBuf {
        "..".into()
    }

    fn get_sync_thumbnail(&self) -> Option<RgbaData> {
        // Parent folder shows folder icon synchronously
        Some(FOLDER_PLACEHOLDER.clone())
    }

    async fn get_thumbnail_preview(&self, _cancel_token: &CancellationToken) -> Option<RgbaData> {
        // Use get_sync_thumbnail() instead
        None
    }

    fn clone_box(&self) -> Box<dyn Item> {
        Box::new(ParentItem)
    }
}
