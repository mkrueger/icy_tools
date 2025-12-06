use async_trait::async_trait;
use icy_engine::formats::FileFormat;
use icy_engine_gui::ui::FileIcon;
use tokio_util::sync::CancellationToken;

use crate::items::{ArchiveContainer, Item, load_image_to_rgba, sort_folder};
use crate::ui::thumbnail_view::{DIZ_NOT_FOUND_PLACEHOLDER, RgbaData};

use super::{API_PATH, SixteenColorsFile, cache::fetch_json_async, get_cache};

/// A release pack folder containing individual files
pub struct SixteenColorsPack {
    pub filename: String,
    pub month: u64,
    pub year: u64,
    pub name: String,
}

impl SixteenColorsPack {
    pub fn new(filename: String, month: u64, year: u64, name: String) -> Self {
        Self { filename, month, year, name }
    }
}

#[async_trait]
impl Item for SixteenColorsPack {
    fn get_label(&self) -> String {
        self.name.clone()
    }

    fn get_file_path(&self) -> String {
        self.name.clone()
    }

    fn is_container(&self) -> bool {
        true
    }

    fn get_file_icon(&self) -> FileIcon {
        FileIcon::FolderData
    }

    async fn get_thumbnail_preview(&self, _cancel_token: &CancellationToken) -> Option<RgbaData> {
        // Try both FILE_ID.DIZ.png and FILE_ID.ANS.png
        let urls = [
            format!("https://16colo.rs/pack/{}/tn/FILE_ID.DIZ.png", self.name),
            format!("https://16colo.rs/pack/{}/tn/FILE_ID.ANS.png", self.name),
        ];

        let cache = get_cache();

        // Check cache first for any of the URLs
        {
            let cache_read = cache.read();
            for url in &urls {
                if let Some(rgba) = cache_read.get_thumbnail(url) {
                    return Some(rgba.clone());
                }
            }
            // If all URLs have failed before, return "no file_id.diz" placeholder
            if urls.iter().all(|url| cache_read.has_failed(url)) {
                return Some(DIZ_NOT_FOUND_PLACEHOLDER.clone());
            }
        }

        // Try each URL in order
        for url in urls {
            // Skip if already failed
            if cache.read().has_failed(&url) {
                continue;
            }

            match reqwest::get(&url).await {
                Ok(response) => {
                    if !response.status().is_success() {
                        cache.write().mark_failed(url);
                        continue;
                    }
                    match response.bytes().await {
                        Ok(bytes) => {
                            // Check for minimum valid PNG size
                            if bytes.len() < 200 {
                                cache.write().mark_failed(url);
                                continue;
                            }
                            match load_image_to_rgba(&bytes) {
                                Some(rgba) => {
                                    cache.write().set_thumbnail(url, rgba.clone());
                                    return Some(rgba);
                                }
                                None => {
                                    cache.write().mark_failed(url);
                                    continue;
                                }
                            }
                        }
                        Err(_) => {
                            cache.write().mark_failed(url);
                            continue;
                        }
                    }
                }
                Err(_) => {
                    cache.write().mark_failed(url);
                    continue;
                }
            }
        }

        // All URLs failed - show "no file_id.diz" placeholder
        Some(DIZ_NOT_FOUND_PLACEHOLDER.clone())
    }

    async fn get_subitems(&self, _cancel_token: &CancellationToken) -> Option<Vec<Box<dyn Item>>> {
        let url = format!("{}/pack/{}?rows=0", API_PATH, self.name);
        let cache = get_cache();
        let json = fetch_json_async(&cache, &url).await?;

        let mut result: Vec<Box<dyn Item>> = Vec::new();
        if let Some(packs) = json["files"].as_array() {
            for pack in packs {
                let filename = pack["filename"].as_str().unwrap_or_default().to_string();
                let location = pack["file_location"].as_str().unwrap_or_default().to_string();
                let uri = pack["uri"].as_str().unwrap_or_default().to_string();
                let thumbnail = pack["thumbnail"].as_str().unwrap_or_default().to_string();

                // Check if it's an archive file
                if let Some(FileFormat::Archive(format)) = FileFormat::from_extension(&filename) {
                    result.push(Box::new(ArchiveContainer::new(
                        Box::new(SixteenColorsFile::new(filename, location, uri, thumbnail)),
                        format,
                    )));
                } else {
                    result.push(Box::new(SixteenColorsFile::new(filename, location, uri, thumbnail)));
                }
            }
            sort_folder(&mut result);
        }
        Some(result)
    }

    fn clone_box(&self) -> Box<dyn Item> {
        Box::new(SixteenColorsPack::new(self.filename.clone(), self.month, self.year, self.name.clone()))
    }
}
