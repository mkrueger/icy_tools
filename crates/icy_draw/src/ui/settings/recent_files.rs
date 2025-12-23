//! Most Recently Used (MRU) files management
//!
//! Tracks and persists recently opened files for quick access.
//! Features:
//! - Automatic canonicalization of paths (resolves symlinks, relative paths)
//! - Lazy existence checking with time-based cache invalidation
//! - Atomic file saving to prevent corruption on crash

use std::cell::RefCell;
use std::fs::{create_dir_all, rename, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::Settings;

/// Maximum number of recent files to keep
const MAX_RECENT_FILES: usize = 10;

/// How long to cache file existence checks (in seconds)
const EXISTENCE_CACHE_TTL_SECS: u64 = 30;

/// Internal cache state for existing files
#[derive(Debug, Clone, Default)]
struct ExistenceCache {
    files: Vec<PathBuf>,
    timestamp: Option<Instant>,
}

/// Manages the list of most recently used files
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MostRecentlyUsedFiles {
    /// The list of recent files (stored as canonical paths)
    files: Vec<PathBuf>,

    /// Cache of existing files (not serialized, interior mutability for &self access)
    #[serde(skip)]
    cache: RefCell<ExistenceCache>,
}

impl MostRecentlyUsedFiles {
    /// Create a new empty MRU list
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            cache: RefCell::new(ExistenceCache::default()),
        }
    }

    /// Load MRU list from config file
    pub fn load() -> Self {
        let Some(path) = Self::get_mru_file_path() else {
            return Self::new();
        };

        if !path.exists() {
            return Self::new();
        }

        match File::open(&path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                serde_json::from_reader(reader).unwrap_or_default()
            }
            Err(e) => {
                log::warn!("Failed to load recent files: {}", e);
                Self::new()
            }
        }
    }

    /// Get the path to the MRU config file
    fn get_mru_file_path() -> Option<PathBuf> {
        Settings::config_dir().map(|d| d.join("recent_files.json"))
    }

    /// Get recent files, filtering out non-existent ones.
    /// Uses a time-based cache to avoid repeated filesystem checks.
    /// Returns a clone of the cached files to avoid borrow issues.
    pub fn files(&self) -> Vec<PathBuf> {
        let cache_valid = {
            let cache = self.cache.borrow();
            cache.timestamp.map_or(false, |ts| ts.elapsed() < Duration::from_secs(EXISTENCE_CACHE_TTL_SECS))
        };

        if cache_valid {
            return self.cache.borrow().files.clone();
        }

        // Rebuild cache - filter to only existing files
        let existing: Vec<PathBuf> = self.files.iter().filter(|p| p.exists()).cloned().collect();

        {
            let mut cache = self.cache.borrow_mut();
            cache.files = existing.clone();
            cache.timestamp = Some(Instant::now());
        }

        existing
    }

    /// Get all files without existence filtering (for hashing/internal use)
    pub fn files_unfiltered(&self) -> &[PathBuf] {
        &self.files
    }

    /// Invalidate the existence cache (call after filesystem operations)
    fn invalidate_cache(&self) {
        let mut cache = self.cache.borrow_mut();
        cache.files.clear();
        cache.timestamp = None;
    }

    /// Add a file to the recent files list.
    /// Paths are canonicalized to absolute paths to avoid duplicates.
    pub fn add_recent_file(&mut self, file: &Path) {
        // Canonicalize the path to get absolute path and resolve symlinks
        let file = match file.canonicalize() {
            Ok(canonical) => canonical,
            Err(_) => {
                // If canonicalize fails (file doesn't exist yet), use absolute path
                if file.is_absolute() {
                    file.to_path_buf()
                } else {
                    match std::env::current_dir() {
                        Ok(cwd) => cwd.join(file),
                        Err(_) => file.to_path_buf(),
                    }
                }
            }
        };

        // Remove if already exists (to move to end)
        self.files.retain(|f| f != &file);

        // Add to end (most recent)
        self.files.push(file);

        // Trim to max size (remove oldest first)
        while self.files.len() > MAX_RECENT_FILES {
            self.files.remove(0);
        }

        // Invalidate cache since we modified the list
        self.invalidate_cache();

        if let Err(e) = self.save() {
            log::error!("Error saving recent files: {}", e);
        }
    }

    /// Clear all recent files
    pub fn clear_recent_files(&mut self) {
        self.files.clear();
        self.invalidate_cache();

        if let Err(e) = self.save() {
            log::error!("Error saving recent files: {}", e);
        }
    }

    /// Save MRU list to config file atomically.
    /// Writes to a temp file first, then renames to prevent corruption.
    fn save(&self) -> std::io::Result<()> {
        let Some(path) = Self::get_mru_file_path() else {
            return Ok(());
        };

        // Ensure config directory exists
        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        }

        // Write to temp file first for atomic save
        let temp_path = path.with_extension("json.tmp");

        {
            let file = File::create(&temp_path)?;
            let mut writer = BufWriter::new(file);
            serde_json::to_writer_pretty(&mut writer, &self)?;
            writer.flush()?;
        }

        // Atomic rename (on most filesystems)
        rename(&temp_path, &path)?;

        Ok(())
    }
}
