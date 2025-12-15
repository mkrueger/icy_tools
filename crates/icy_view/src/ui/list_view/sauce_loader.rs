//! SAUCE information loader for file list view
//!
//! This module provides async loading of SAUCE information for files in the list view.
//! Similar to the thumbnail loader, it uses background tasks to avoid blocking the UI.
//! Uses Arc<str> for zero-cost cloning of SAUCE strings.

use std::collections::HashMap;
use std::sync::Arc;

use log::debug;
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::items::Item;

/// Cached SAUCE information using Arc<str> for zero-cost cloning
#[derive(Clone, Debug, Default)]
pub struct SauceInfo {
    /// Title from SAUCE record (empty Arc<str> if none)
    pub title: Arc<str>,
    /// Author from SAUCE record (empty Arc<str> if none)
    pub author: Arc<str>,
    /// Group from SAUCE record (empty Arc<str> if none)
    pub group: Arc<str>,
}

impl SauceInfo {
    /// Create from string slices
    pub fn new(title: &str, author: &str, group: &str) -> Self {
        Self {
            title: Arc::from(title),
            author: Arc::from(author),
            group: Arc::from(group),
        }
    }
}

/// Result of loading SAUCE info for a file
#[derive(Clone, Debug)]
pub struct SauceResult;

/// Request to load SAUCE info
pub struct SauceRequest {
    /// The item to load SAUCE for
    pub item: Arc<dyn Item>,
}

/// Cache for SAUCE information using Arc<str> for zero-cost cloning
pub struct SauceCache {
    /// Cached SAUCE info (path -> SauceInfo or None if no SAUCE)
    cache: HashMap<String, Option<SauceInfo>>,
    /// Paths that are currently being loaded
    pending: std::collections::HashSet<String>,
}

impl SauceCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            pending: std::collections::HashSet::new(),
        }
    }

    /// Get cached SAUCE info for a path - zero-cost clone via Arc<str>
    pub fn get(&self, path: &String) -> Option<Option<SauceInfo>> {
        self.cache.get(path).cloned()
    }

    /// Check if a path has cached SAUCE info
    pub fn contains(&self, path: &String) -> bool {
        self.cache.contains_key(path)
    }

    /// Check if a path is already being loaded
    pub fn is_pending(&self, path: &String) -> bool {
        self.pending.contains(path)
    }

    /// Mark a path as pending
    pub fn mark_pending(&mut self, path: String) {
        self.pending.insert(path);
    }

    /// Store SAUCE result and remove from pending
    pub fn store(&mut self, path: String, sauce: Option<SauceInfo>) {
        self.pending.remove(&path);
        self.cache.insert(path, sauce);
    }

    /// Clear all cached data
    pub fn clear(&mut self) {
        self.cache.clear();
        self.pending.clear();
    }
}

impl Default for SauceCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared SAUCE cache type
pub type SharedSauceCache = Arc<RwLock<SauceCache>>;

/// SAUCE loader that uses Tokio for async loading
pub struct SauceLoader {
    /// Sender for results
    result_tx: mpsc::UnboundedSender<SauceResult>,
    /// Current cancellation token
    cancel_token: CancellationToken,
    /// Tokio runtime handle
    runtime: Arc<tokio::runtime::Runtime>,
    /// Shared cache
    cache: SharedSauceCache,
}

impl SauceLoader {
    /// Spawn a new SAUCE loader
    /// Returns the loader and the result receiver
    pub fn spawn() -> (Self, mpsc::UnboundedReceiver<SauceResult>, SharedSauceCache) {
        let (result_tx, result_rx) = mpsc::unbounded_channel();
        let cache = Arc::new(RwLock::new(SauceCache::new()));

        // Create a multi-threaded Tokio runtime for SAUCE loading
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .thread_name("sauce-loader")
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime for SAUCE loader");

        (
            Self {
                result_tx,
                cancel_token: CancellationToken::new(),
                runtime: Arc::new(runtime),
                cache: cache.clone(),
            },
            result_rx,
            cache,
        )
    }

    /// Queue a SAUCE info load request
    pub fn load(&self, request: SauceRequest) {
        let result_tx = self.result_tx.clone();
        let cancel_token = self.cancel_token.child_token();
        let cache = self.cache.clone();

        let item = request.item;
        let path = item.get_full_path().unwrap_or_else(|| item.get_file_path());

        // Check cache first
        {
            let cache_read = cache.read();
            if cache_read.contains(&path) {
                // Already cached
                return;
            }
            if cache_read.is_pending(&path) {
                // Already loading
                return;
            }
        }

        // Mark as pending
        cache.write().mark_pending(path.clone());

        debug!("[SauceLoader] Spawning task for: {:?}", path);

        // Spawn async task
        self.runtime.spawn(async move {
            // Check cancellation
            if cancel_token.is_cancelled() {
                debug!("[SauceLoader] Task cancelled before start: {:?}", path);
                return;
            }

            // Load SAUCE info
            let sauce_record = item.get_sauce_info(&cancel_token).await;

            if cancel_token.is_cancelled() {
                debug!("[SauceLoader] Task cancelled after load: {:?}", path);
                return;
            }

            // Extract relevant fields into SauceInfo using Arc<str> for zero-cost cloning
            let sauce_info = sauce_record.map(|record| SauceInfo::new(&record.title().to_string(), &record.author().to_string(), &record.group().to_string()));

            // Store in cache
            cache.write().store(path.clone(), sauce_info.clone());

            // Send result
            let _ = result_tx.send(SauceResult);
        });
    }

    /// Cancel all pending loads
    pub fn cancel_all(&self) {
        debug!("[SauceLoader] Cancelling all pending loads");
        self.cancel_token.cancel();
    }

    /// Reset the loader with a new cancellation token
    pub fn reset(&mut self) {
        self.cancel_token = CancellationToken::new();
    }
}
