use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::items::{load_image_to_rgba, Item, ItemError};
use crate::thumbnail::RgbaData;

use super::{get_cache, MAIN_PATH};

async fn fetch_file_data(url: &str, cache: &super::SharedSixteenColorsCache) -> Result<Vec<u8>, ItemError> {
    {
        let cache_read = cache.read();
        if let Some(data) = cache_read.get_file_data(url) {
            return Ok(data);
        }
    }

    match reqwest::get(url).await {
        Ok(response) => {
            if !response.status().is_success() {
                return Err(ItemError::Network(format!("HTTP {} for {}", response.status(), url)));
            }
            match response.bytes().await {
                Ok(bytes) => {
                    let data = bytes.to_vec();
                    cache.write().set_file_data(url.to_string(), data.clone());
                    Ok(data)
                }
                Err(err) => {
                    log::error!("Failed to read 16colors data: {}", err);
                    Err(ItemError::Network(format!("Failed to read response: {}", err)))
                }
            }
        }
        Err(err) => {
            log::error!("Failed to fetch 16colors data: {}", err);
            Err(ItemError::Network(format!("Connection error: {}", err)))
        }
    }
}

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
        fetch_file_data(&url, &cache).await
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

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::super::SixteenColorsCache;
    use super::*;

    #[test]
    fn file_download_retries_after_http_failure() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/image.jpg", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for (status, body) in [("500 Internal Server Error", &b""[..]), ("200 OK", &b"jpeg data"[..])] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 1024];
                stream.read(&mut request).unwrap();
                write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
                stream.write_all(body).unwrap();
            }
        });

        let cache = std::sync::Arc::new(parking_lot::RwLock::new(SixteenColorsCache::new_in_memory()));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let first = runtime.block_on(fetch_file_data(&url, &cache));
        assert!(matches!(first, Err(ItemError::Network(message)) if message.contains("HTTP 500")));
        assert_eq!(runtime.block_on(fetch_file_data(&url, &cache)).unwrap(), b"jpeg data");
        server.join().unwrap();
    }
}
