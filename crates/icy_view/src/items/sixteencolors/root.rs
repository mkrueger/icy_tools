use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::items::{FileIcon, Item, ItemError};
use crate::ui::thumbnail_view::{FOLDER_PLACEHOLDER, RgbaData};

use super::{API_PATH, SixteenColorsYear, cache::fetch_json_async, get_cache};

/// Root folder for 16colors.rs browsing
pub struct SixteenColorsRoot {}

impl SixteenColorsRoot {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Item for SixteenColorsRoot {
    fn get_label(&self) -> String {
        "16colo.rs".to_string()
    }

    fn get_file_path(&self) -> String {
        String::new() // Empty path for root
    }

    fn is_container(&self) -> bool {
        true
    }

    fn get_file_icon(&self) -> FileIcon {
        FileIcon::FolderData
    }

    fn get_sync_thumbnail(&self) -> Option<RgbaData> {
        Some(FOLDER_PLACEHOLDER.clone())
    }

    async fn get_thumbnail_preview(&self, _cancel_token: &CancellationToken) -> Option<RgbaData> {
        // Use get_sync_thumbnail() instead
        None
    }

    async fn get_subitems(&self, _cancel_token: &CancellationToken) -> Result<Vec<Box<dyn Item>>, ItemError> {
        let url = format!("{}/year?rows=0", API_PATH);
        let cache = get_cache();
        let json = fetch_json_async(&cache, &url).await?;

        let mut result: Vec<Box<dyn Item>> = Vec::new();
        if let Some(packs) = json.as_array() {
            for pack in packs {
                let year = pack["year"].as_u64().unwrap_or(0);
                let packs_count = pack["packs"].as_u64().unwrap_or(0);
                result.push(Box::new(SixteenColorsYear::new(year, packs_count)));
            }
            result.reverse();
        }
        Ok(result)
    }

    fn clone_box(&self) -> Box<dyn Item> {
        Box::new(SixteenColorsRoot::new())
    }
}
