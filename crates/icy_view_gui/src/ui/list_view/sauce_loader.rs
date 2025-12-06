//! SAUCE information loader for file list view
//!
//! This module provides async loading of SAUCE information for files in the list view.
//! Similar to the thumbnail loader, it uses background tasks to avoid blocking the UI.
//! Uses string interning for memory efficiency since author/group names are often repeated.

use std::collections::HashMap;
use std::sync::Arc;

use log::debug;
use parking_lot::RwLock;
use string_interner::DefaultSymbol;
use string_interner::backend::StringBackend;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::Item;

/// Type alias for our string interner
type SauceInterner = string_interner::StringInterner<StringBackend<DefaultSymbol>>;

/// Interned SAUCE information - uses symbols instead of strings for memory efficiency
#[derive(Clone, Copy, Debug, Default)]
pub struct InternedSauceInfo {
    /// Title symbol (None if empty)
    pub title: Option<DefaultSymbol>,
    /// Author symbol (None if empty)
    pub author: Option<DefaultSymbol>,
    /// Group symbol (None if empty)
    pub group: Option<DefaultSymbol>,
}

/// Extracted SAUCE information that is Send + Sync (for external use)
#[derive(Clone, Debug, Default)]
pub struct SauceInfo {
    /// Title from SAUCE record
    pub title: String,
    /// Author from SAUCE record
    pub author: String,
    /// Group from SAUCE record
    pub group: String,
}

impl SauceInfo {
    /// Check if this SAUCE info has any content
    pub fn is_empty(&self) -> bool {
        self.title.is_empty() && self.author.is_empty() && self.group.is_empty()
    }
}

/// Result of loading SAUCE info for a file
#[derive(Clone, Debug)]
pub struct SauceResult {
    /// Path to identify the file
    pub path: String,
    /// The extracted SAUCE info (None if no SAUCE or loading failed)
    pub sauce: Option<SauceInfo>,
}

/// Request to load SAUCE info
pub struct SauceRequest {
    /// The item to load SAUCE for
    pub item: Arc<dyn Item>,
}

/// Cache for SAUCE information with string interning
pub struct SauceCache {
    /// String interner for deduplicating strings
    interner: SauceInterner,
    /// Cached SAUCE info (path -> InternedSauceInfo or None if no SAUCE)
    cache: HashMap<String, Option<InternedSauceInfo>>,
    /// Paths that are currently being loaded
    pending: std::collections::HashSet<String>,
}

impl SauceCache {
    pub fn new() -> Self {
        Self {
            interner: SauceInterner::default(),
            cache: HashMap::new(),
            pending: std::collections::HashSet::new(),
        }
    }

    /// Intern a string, returning None if the string is empty
    fn intern_if_not_empty(&mut self, s: &str) -> Option<DefaultSymbol> {
        if s.is_empty() { None } else { Some(self.interner.get_or_intern(s)) }
    }

    /// Resolve a symbol to a string
    pub fn resolve(&self, symbol: DefaultSymbol) -> Option<&str> {
        self.interner.resolve(symbol)
    }

    /// Get cached SAUCE info for a path and resolve to SauceInfo
    pub fn get(&self, path: &String) -> Option<Option<SauceInfo>> {
        self.cache.get(path).map(|opt| {
            opt.map(|interned| SauceInfo {
                title: interned.title.and_then(|s| self.resolve(s)).unwrap_or("").to_string(),
                author: interned.author.and_then(|s| self.resolve(s)).unwrap_or("").to_string(),
                group: interned.group.and_then(|s| self.resolve(s)).unwrap_or("").to_string(),
            })
        })
    }

    /// Check if a path has cached SAUCE info (without resolving)
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
        let interned = sauce.map(|info| InternedSauceInfo {
            title: self.intern_if_not_empty(&info.title),
            author: self.intern_if_not_empty(&info.author),
            group: self.intern_if_not_empty(&info.group),
        });
        self.cache.insert(path, interned);
    }

    /// Clear all cached data
    pub fn clear(&mut self) {
        self.cache.clear();
        self.pending.clear();
        // Note: We don't clear the interner - symbols from previous sessions
        // will be reused if the same strings appear again
    }

    /// Get statistics about the cache
    pub fn stats(&self) -> (usize, usize) {
        (self.cache.len(), self.interner.len())
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

            // Extract relevant fields into SauceInfo
            let sauce_info = sauce_record.map(|record| SauceInfo {
                title: record.title().to_string(),
                author: record.author().to_string(),
                group: record.group().to_string(),
            });

            // Store in cache
            cache.write().store(path.clone(), sauce_info.clone());

            // Send result
            let _ = result_tx.send(SauceResult { path, sauce: sauce_info });
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

    /// Clear the cache
    pub fn clear_cache(&self) {
        self.cache.write().clear();
    }

    /// Get the shared cache
    pub fn cache(&self) -> &SharedSauceCache {
        &self.cache
    }
}
