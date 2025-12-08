//! Most Recently Used (MRU) files management
//!
//! Tracks and persists recently opened files for quick access.

use std::fs::{File, create_dir_all};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

/// Maximum number of recent files to keep
const MAX_RECENT_FILES: usize = 10;

/// Manages the list of most recently used files
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MostRecentlyUsedFiles {
    files: Vec<PathBuf>,
}

impl MostRecentlyUsedFiles {
    /// Create a new empty MRU list
    pub fn new() -> Self {
        Self { files: Vec::new() }
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
        ProjectDirs::from("com", "GitHub", "icy_draw")
            .map(|dirs| dirs.config_dir().join("recent_files.json"))
    }

    /// Get recent files, filtering out non-existent ones
    pub fn get_recent_files(&mut self) -> &[PathBuf] {
        self.files.retain(|p| p.exists());
        &self.files
    }

    /// Get recent files without modifying (for menu display)
    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Check if there are any recent files
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Add a file to the recent files list
    pub fn add_recent_file(&mut self, file: &Path) {
        let file = file.to_path_buf();
        
        // Remove if already exists (to move to end)
        self.files.retain(|f| f != &file);
        
        // Add to end
        self.files.push(file);
        
        // Trim to max size
        while self.files.len() > MAX_RECENT_FILES {
            self.files.remove(0);
        }
        
        if let Err(e) = self.save() {
            log::error!("Error saving recent files: {}", e);
        }
    }

    /// Clear all recent files
    pub fn clear_recent_files(&mut self) {
        self.files.clear();
        if let Err(e) = self.save() {
            log::error!("Error saving recent files: {}", e);
        }
    }

    /// Save MRU list to config file
    fn save(&self) -> std::io::Result<()> {
        let Some(path) = Self::get_mru_file_path() else {
            return Ok(());
        };

        // Ensure config directory exists
        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        }

        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &self)?;
        Ok(())
    }
}
