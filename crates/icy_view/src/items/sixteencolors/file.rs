use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::items::{Item, ItemError, load_image_to_rgba};
use crate::thumbnail::RgbaData;

use super::{MAIN_PATH, get_cache};

/// A single file from 16colors.rs
pub struct SixteenColorsFile {
    pub filename: String,
    pub location: String,
    pub uri: String,
    pub thumbnail: String,
}

impl SixteenColorsFile {
    pub fn new(filename: String, location: String, uri: String, thumbnail: String) -> Self {
        Self {
            filename,
            location,
            uri,
            thumbnail,
        }
    }
}

#[async_trait]
impl Item for SixteenColorsFile {
    fn get_label(&self) -> String {
        self.filename.clone()
    }

    fn get_file_path(&self) -> String {
        // Use location + filename to make path unique across packs
        format!("{}/{}", self.location, self.filename)
    }

    fn is_virtual_file(&self) -> bool {
        true
    }

    async fn get_thumbnail_preview(&self, _cancel_token: &CancellationToken) -> Option<RgbaData> {
        if self.thumbnail.is_empty() {
            return None;
        }

        let url = format!("{}{}", MAIN_PATH, self.thumbnail);
        let cache = get_cache();

        // Check cache first (memory and disk)
        {
            let cache_read = cache.read();
            if let Some(rgba) = cache_read.get_thumbnail(&url) {
                return Some(rgba);
            }
            if cache_read.has_failed(&url) {
                return None;
            }
        }

        // Fetch from network
        match reqwest::get(&url).await {
            Ok(response) => {
                if !response.status().is_success() {
                    cache.write().mark_failed(url);
                    return None;
                }
                match response.bytes().await {
                    Ok(bytes) => {
                        if bytes.len() < 200 {
                            cache.write().mark_failed(url);
                            return None;
                        }
                        match load_image_to_rgba(&bytes) {
                            Some(rgba) => {
                                cache.write().set_thumbnail(url, rgba.clone());
                                Some(rgba)
                            }
                            None => {
                                cache.write().mark_failed(url);
                                None
                            }
                        }
                    }
                    Err(_) => {
                        cache.write().mark_failed(url);
                        None
                    }
                }
            }
            Err(_) => {
                cache.write().mark_failed(url);
                None
            }
        }
    }

    async fn read_data(&self) -> Result<Vec<u8>, ItemError> {
        let url = format!("{}{}", MAIN_PATH, self.uri);
        let cache = get_cache();

        // Check cache first (memory and disk)
        {
            let cache_read = cache.read();
            if let Some(data) = cache_read.get_file_data(&url) {
                return Ok(data);
            }
            if cache_read.has_failed(&url) {
                return Err(ItemError::NotFound(format!("Previously failed: {}", url)));
            }
        }

        // Fetch from network
        match reqwest::get(&url).await {
            Ok(response) => {
                if !response.status().is_success() {
                    let status = response.status();
                    cache.write().mark_failed(url.clone());
                    return Err(ItemError::Network(format!("HTTP {} for {}", status, url)));
                }
                match response.bytes().await {
                    Ok(bytes) => {
                        let data = bytes.to_vec();
                        cache.write().set_file_data(url, data.clone());
                        Ok(data)
                    }
                    Err(err) => {
                        log::error!("Failed to read 16colors data: {}", err);
                        cache.write().mark_failed(url.clone());
                        Err(ItemError::Network(format!("Failed to read response: {}", err)))
                    }
                }
            }
            Err(err) => {
                log::error!("Failed to fetch 16colors data: {}", err);
                cache.write().mark_failed(url.clone());
                Err(ItemError::Network(format!("Connection error: {}", err)))
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Item> {
        Box::new(SixteenColorsFile {
            filename: self.filename.clone(),
            location: self.location.clone(),
            uri: self.uri.clone(),
            thumbnail: self.thumbnail.clone(),
        })
    }
}
