use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use directories::BaseDirs;
use parking_lot::RwLock;
use serde_json::Value;

use crate::ui::thumbnail_view::RgbaData;

/// Get the cache directory for icy_view
fn get_cache_dir() -> Option<PathBuf> {
    BaseDirs::new().map(|dirs| dirs.cache_dir().join("icy_view").join("16colors"))
}

/// Convert a URL to a safe filename for caching
fn url_to_cache_filename(url: &str) -> String {
    // Create a hash-based filename to avoid path issues
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let hash = hasher.finish();

    // Extract file extension from URL if present
    let ext = url
        .rsplit('/')
        .next()
        .and_then(|f| f.rsplit('.').next())
        .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("bin");

    format!("{:016x}.{}", hash, ext)
}

/// Shared cache for 16colors.rs API responses and thumbnails
/// This is wrapped in Arc<RwLock<>> for thread-safe sharing between items
pub struct SixteenColorsCache {
    /// Cached API responses (JSON) - None means request failed
    pub api_responses: std::collections::HashMap<String, Option<Value>>,
    /// Cached thumbnail images (already decoded to RGBA)
    thumbnails: std::collections::HashMap<String, RgbaData>,
    /// Cached file data (raw bytes) - in-memory cache
    file_data: std::collections::HashMap<String, Vec<u8>>,
    /// URLs that have had connection errors (to avoid repeated error logs)
    failed_urls: HashSet<String>,
    /// Whether we've logged a connection error (log once)
    connection_error_logged: bool,
    /// Local disk cache directory
    cache_dir: Option<PathBuf>,
}

impl Default for SixteenColorsCache {
    fn default() -> Self {
        let cache_dir = get_cache_dir();

        // Create cache directory if it doesn't exist
        if let Some(ref dir) = cache_dir {
            if let Err(e) = std::fs::create_dir_all(dir) {
                log::warn!("Failed to create cache directory {:?}: {}", dir, e);
            }
        }

        Self {
            api_responses: std::collections::HashMap::new(),
            thumbnails: std::collections::HashMap::new(),
            file_data: std::collections::HashMap::new(),
            failed_urls: HashSet::new(),
            connection_error_logged: false,
            cache_dir,
        }
    }
}

impl SixteenColorsCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the disk cache path for a URL
    fn disk_cache_path(&self, url: &str) -> Option<PathBuf> {
        self.cache_dir.as_ref().map(|dir| dir.join(url_to_cache_filename(url)))
    }

    /// Check if URL is cached
    pub fn get_cached_json(&self, url: &str) -> Option<Option<Value>> {
        self.api_responses.get(url).cloned()
    }

    /// Check if URL has failed
    pub fn is_failed(&self, url: &str) -> bool {
        self.failed_urls.contains(url)
    }

    /// Store successful JSON response
    pub fn store_json(&mut self, url: String, json: Value) {
        self.api_responses.insert(url, Some(json));
    }

    /// Store failed JSON request
    pub fn store_json_failed(&mut self, url: String) {
        self.api_responses.insert(url.clone(), None);
        self.failed_urls.insert(url);
    }

    /// Log connection error (only once)
    pub fn log_connection_error(&mut self, err: &reqwest::Error) {
        if !self.connection_error_logged {
            log::warn!("16colors.rs connection error: {} (further errors suppressed)", err);
            self.connection_error_logged = true;
        }
        // Don't mark as failed - might be temporary
    }

    /// Get the disk cache path for a thumbnail URL
    fn thumbnail_disk_cache_path(&self, url: &str) -> Option<PathBuf> {
        self.cache_dir.as_ref().map(|dir| dir.join("thumbs").join(url_to_cache_filename(url)))
    }

    /// Get a cached thumbnail - checks memory cache first, then disk cache
    pub fn get_thumbnail(&self, url: &str) -> Option<RgbaData> {
        // Check in-memory cache first
        if let Some(data) = self.thumbnails.get(url) {
            return Some(data.clone());
        }

        // Check disk cache
        if let Some(path) = self.thumbnail_disk_cache_path(url) {
            if path.exists() {
                match std::fs::read(&path) {
                    Ok(data) => {
                        // Deserialize: first 8 bytes are width/height as u32, rest is RGBA data
                        if data.len() >= 8 {
                            let width = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                            let height = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                            let rgba_data: Arc<Vec<u8>> = Arc::new(data[8..].to_vec());
                            log::debug!("Loaded thumbnail from disk cache: {}", url);
                            return Some(RgbaData {
                                width,
                                height,
                                data: rgba_data,
                            });
                        }
                    }
                    Err(e) => {
                        log::debug!("Failed to read thumbnail disk cache {:?}: {}", path, e);
                    }
                }
            }
        }

        None
    }

    /// Store a thumbnail in the cache (both memory and disk)
    pub fn set_thumbnail(&mut self, url: String, rgba: RgbaData) {
        // Store to disk cache
        if let Some(path) = self.thumbnail_disk_cache_path(&url) {
            // Ensure thumbs directory exists
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // Serialize: first 8 bytes are width/height as u32, rest is RGBA data
            let mut serialized = Vec::with_capacity(8 + rgba.data.len());
            serialized.extend_from_slice(&rgba.width.to_le_bytes());
            serialized.extend_from_slice(&rgba.height.to_le_bytes());
            serialized.extend_from_slice(&rgba.data);
            if let Err(e) = std::fs::write(&path, &serialized) {
                log::debug!("Failed to write thumbnail disk cache {:?}: {}", path, e);
            } else {
                log::debug!("Saved thumbnail to disk cache: {}", url);
            }
        }

        // Store in memory cache
        self.thumbnails.insert(url, rgba);
    }

    /// Get cached file data - checks memory cache first, then disk cache
    pub fn get_file_data(&self, url: &str) -> Option<Vec<u8>> {
        // Check in-memory cache first
        if let Some(data) = self.file_data.get(url) {
            return Some(data.clone());
        }

        // Check disk cache
        if let Some(path) = self.disk_cache_path(url) {
            if path.exists() {
                match std::fs::read(&path) {
                    Ok(data) => {
                        log::debug!("Loaded from disk cache: {}", url);
                        return Some(data);
                    }
                    Err(e) => {
                        log::debug!("Failed to read disk cache {:?}: {}", path, e);
                    }
                }
            }
        }

        None
    }

    /// Store file data in the cache (both memory and disk)
    pub fn set_file_data(&mut self, url: String, data: Vec<u8>) {
        // Store to disk cache
        if let Some(path) = self.disk_cache_path(&url) {
            if let Err(e) = std::fs::write(&path, &data) {
                log::debug!("Failed to write disk cache {:?}: {}", path, e);
            } else {
                log::debug!("Saved to disk cache: {}", url);
            }
        }

        // Store in memory cache
        self.file_data.insert(url, data);
    }

    /// Check if a URL has failed before
    pub fn has_failed(&self, url: &str) -> bool {
        self.failed_urls.contains(url)
    }

    /// Mark a URL as failed (for non-JSON requests like thumbnails)
    pub fn mark_failed(&mut self, url: String) {
        self.failed_urls.insert(url);
    }

    /// Clear all cached data (memory only, preserves disk cache)
    pub fn clear(&mut self) {
        self.api_responses.clear();
        self.thumbnails.clear();
        self.file_data.clear();
        self.failed_urls.clear();
        self.connection_error_logged = false;
    }

    /// Clear disk cache as well
    pub fn clear_disk_cache(&mut self) {
        if let Some(ref dir) = self.cache_dir {
            if let Err(e) = std::fs::remove_dir_all(dir) {
                log::warn!("Failed to clear disk cache {:?}: {}", dir, e);
            } else {
                // Recreate the directory
                let _ = std::fs::create_dir_all(dir);
            }
        }
    }

    /// Get cache statistics (for debugging)
    pub fn stats(&self) -> (usize, usize, usize) {
        (self.api_responses.len(), self.thumbnails.len(), self.file_data.len())
    }

    /// Get disk cache size in bytes
    pub fn disk_cache_size(&self) -> u64 {
        self.cache_dir
            .as_ref()
            .map(|dir| {
                std::fs::read_dir(dir)
                    .ok()
                    .map(|entries| entries.filter_map(|e| e.ok()).filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum())
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    }
}

/// Thread-safe shared cache handle
pub type SharedSixteenColorsCache = Arc<RwLock<SixteenColorsCache>>;

/// Async function to fetch JSON, using cache
pub async fn fetch_json_async(cache: &SharedSixteenColorsCache, url: &str) -> Option<Value> {
    // Check cache first (read lock)
    {
        let cache_read = cache.read();
        if let Some(cached) = cache_read.get_cached_json(url) {
            return cached;
        }
        if cache_read.is_failed(url) {
            return None;
        }
    }

    // Fetch from network (no lock held)
    match reqwest::get(url).await {
        Ok(response) => match response.json::<Value>().await {
            Ok(json) => {
                cache.write().store_json(url.to_string(), json.clone());
                Some(json)
            }
            Err(err) => {
                log::debug!("Error parsing json from {}: {}", url, err);
                cache.write().store_json_failed(url.to_string());
                None
            }
        },
        Err(err) => {
            cache.write().log_connection_error(&err);
            None
        }
    }
}
