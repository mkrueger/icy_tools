//! Autosave functionality for collaboration server.
//!
//! This module provides automatic backup functionality similar to Moebius:
//! - Periodic saves with timestamps (configurable interval, default: 1 hour)
//! - Save on server shutdown (always enabled)
//! - Duplicate prevention (only save if content changed)

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use icy_engine::{AttributedChar, SaveOptions, Size, TextAttribute, TextBuffer, formats::FileFormat};
use icy_sauce::MetaData as SauceMetaData;
use tokio::sync::RwLock;

use super::protocol::Block;
use super::server::ServerState;

/// Autosave configuration.
#[derive(Debug, Clone)]
pub struct AutosaveConfig {
    /// Directory to save backups to (default: current directory ".").
    pub backup_folder: PathBuf,

    /// Interval between automatic saves.
    /// If None, only saves on shutdown (no periodic saves).
    /// Default: Some(1 hour)
    pub interval: Option<Duration>,

    /// Base filename for saves (without extension).
    /// Default: "collaboration"
    pub base_filename: String,

    /// File format extension (default: "ans").
    /// Supported: "ans", "xb" (XBin)
    pub format: String,
}

impl Default for AutosaveConfig {
    fn default() -> Self {
        Self {
            backup_folder: PathBuf::from("."),
            interval: Some(Duration::from_secs(60 * 60)), // 1 hour
            base_filename: "collaboration".to_string(),
            format: "ans".to_string(),
        }
    }
}

impl AutosaveConfig {
    /// Create a new autosave config with the given backup folder.
    pub fn new(backup_folder: impl Into<PathBuf>) -> Self {
        Self {
            backup_folder: backup_folder.into(),
            ..Default::default()
        }
    }

    /// Set the autosave interval. Use None to disable periodic saves (only shutdown saves).
    pub fn with_interval(mut self, interval: Option<Duration>) -> Self {
        self.interval = interval;
        self
    }

    /// Set the base filename.
    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.base_filename = filename.into();
        self
    }

    /// Set the file format.
    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.format = format.into();
        self
    }

    /// Check if periodic autosave is enabled (interval is set).
    pub fn has_periodic_saves(&self) -> bool {
        self.interval.is_some()
    }
}

/// Autosave manager for the collaboration server.
pub struct AutosaveManager {
    config: AutosaveConfig,
    state: Arc<ServerState>,
    last_hash: RwLock<Option<u64>>,
    last_saved_file: RwLock<Option<PathBuf>>,
}

impl AutosaveManager {
    /// Create a new autosave manager.
    pub fn new(config: AutosaveConfig, state: Arc<ServerState>) -> Self {
        Self {
            config,
            state,
            last_hash: RwLock::new(None),
            last_saved_file: RwLock::new(None),
        }
    }

    /// Generate a timestamped filename for the backup.
    pub fn generate_filename(&self) -> PathBuf {
        // Generate timestamp using std::time (no chrono dependency)
        use std::time::SystemTime;
        let now = SystemTime::now();
        let duration = now.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
        let secs = duration.as_secs();

        // Convert to readable format (YYYY-MM-DDTHHMMSS)
        // Simple calculation without external crate
        let days_since_epoch = secs / 86400;
        let time_of_day = secs % 86400;
        let hours = time_of_day / 3600;
        let minutes = (time_of_day % 3600) / 60;
        let seconds = time_of_day % 60;

        // Approximate date calculation (good enough for filenames)
        let mut year = 1970i32;
        let mut remaining_days = days_since_epoch as i32;

        loop {
            let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 366 } else { 365 };
            if remaining_days < days_in_year {
                break;
            }
            remaining_days -= days_in_year;
            year += 1;
        }

        let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let days_in_months: [i32; 12] = if is_leap {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };

        let mut month = 1;
        for days in days_in_months {
            if remaining_days < days {
                break;
            }
            remaining_days -= days;
            month += 1;
        }
        let day = remaining_days + 1;

        let timestamp = format!("{:04}-{:02}-{:02}T{:02}{:02}{:02}", year, month, day, hours, minutes, seconds);

        let filename = format!("{} - {}.{}", self.config.base_filename, timestamp, self.config.format);

        self.config.backup_folder.join(&filename)
    }

    /// Convert the server document to a TextBuffer for saving.
    pub async fn document_to_buffer(&self) -> (TextBuffer, SauceMetaData) {
        let doc = self.state.document.read().await;
        let (columns, rows) = self.state.session.get_dimensions();
        let sauce_data = self.state.session.get_sauce();

        // Convert SauceData to SauceMetaData
        let sauce = SauceMetaData {
            title: sauce_data.title.into(),
            author: sauce_data.author.into(),
            group: sauce_data.group.into(),
            comments: if sauce_data.comments.is_empty() {
                vec![]
            } else {
                sauce_data.comments.lines().map(|l| l.into()).collect()
            },
        };

        let mut buffer = TextBuffer::new(Size::new(columns as i32, rows as i32));

        // Set font and ICE colors from session
        buffer.ice_mode = if self.state.session.get_ice_colors() {
            icy_engine::IceMode::Ice
        } else {
            icy_engine::IceMode::Blink
        };

        // Copy blocks to buffer (first editable layer is always index 0 for new buffers)
        let layer_idx = 0;
        for col in 0..columns as usize {
            for row in 0..rows as usize {
                if col < doc.len() && row < doc[col].len() {
                    let block = &doc[col][row];
                    let ch = block_to_attributed_char(block);
                    buffer.layers[layer_idx].set_char((col as i32, row as i32), ch);
                }
            }
        }

        (buffer, sauce)
    }

    /// Calculate a hash of the current document for change detection.
    pub async fn calculate_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let doc = self.state.document.read().await;
        let mut hasher = DefaultHasher::new();

        for column in doc.iter() {
            for block in column.iter() {
                block.code.hash(&mut hasher);
                block.fg.hash(&mut hasher);
                block.bg.hash(&mut hasher);
            }
        }

        hasher.finish()
    }

    /// Check if the document has changed since the last save.
    pub async fn has_changed(&self) -> bool {
        let current_hash = self.calculate_hash().await;
        let last_hash = self.last_hash.read().await;

        match *last_hash {
            Some(hash) => hash != current_hash,
            None => true, // First save, always save
        }
    }

    /// Save the document to a file.
    /// Returns the path if saved, or None if unchanged (and thus skipped).
    pub async fn save(&self) -> Option<PathBuf> {
        // Check if document changed
        if !self.has_changed().await {
            log::debug!("Autosave skipped: no changes detected");
            return None;
        }

        let path = self.generate_filename();

        // Ensure backup folder exists
        if let Err(e) = std::fs::create_dir_all(&self.config.backup_folder) {
            log::error!("Failed to create backup folder {:?}: {}", self.config.backup_folder, e);
            return None;
        }

        // Convert document to buffer
        let (buffer, sauce) = self.document_to_buffer().await;

        // Save to file
        if let Err(e) = self.save_buffer_to_file(&buffer, &sauce, &path) {
            log::error!("Autosave failed to {:?}: {}", path, e);
            return None;
        }

        // Update hash
        let current_hash = self.calculate_hash().await;
        *self.last_hash.write().await = Some(current_hash);

        // Check for duplicate (compare with last saved file)
        let should_keep = self.check_and_remove_duplicate(&path).await;

        if should_keep {
            *self.last_saved_file.write().await = Some(path.clone());
            log::info!("Autosave: saved backup to {:?}", path);
            Some(path)
        } else {
            log::debug!("Autosave: removed duplicate file {:?}", path);
            None
        }
    }

    /// Save the document to a specific path (for shutdown saves).
    pub async fn save_to(&self, path: &Path) -> std::io::Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let (buffer, sauce) = self.document_to_buffer().await;
        self.save_buffer_to_file(&buffer, &sauce, path)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    }

    /// Save a TextBuffer to a file.
    fn save_buffer_to_file(&self, buffer: &TextBuffer, sauce: &SauceMetaData, path: &Path) -> icy_engine::Result<()> {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("ans");

        let format = FileFormat::from_extension(extension).unwrap_or(FileFormat::Ansi);
        let mut options = SaveOptions::default();
        options.sauce = Some(sauce.clone());

        let bytes = format.to_bytes(buffer, &options)?;
        std::fs::write(path, bytes)?;

        Ok(())
    }

    /// Check if the new file is identical to the last saved file.
    /// If identical, delete the new file and return false.
    async fn check_and_remove_duplicate(&self, new_path: &Path) -> bool {
        let last_file = self.last_saved_file.read().await;

        if let Some(last_path) = last_file.as_ref() {
            if last_path != new_path && last_path.exists() && new_path.exists() {
                if files_are_identical(last_path, new_path) {
                    // Remove the duplicate
                    if let Err(e) = std::fs::remove_file(new_path) {
                        log::warn!("Failed to remove duplicate file {:?}: {}", new_path, e);
                    }
                    return false;
                }
            }
        }

        true
    }

    /// Start the periodic autosave task. Returns a handle that can be used to stop it.
    /// Only starts if interval is configured (has_periodic_saves() returns true).
    pub fn start(self: Arc<Self>) -> Option<tokio::task::JoinHandle<()>> {
        let interval = self.config.interval?;

        Some(tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);

            loop {
                interval_timer.tick().await;

                if let Some(path) = self.save().await {
                    use super::server::colors;
                    use anstream::println;

                    println!("{}[Autosave]{} Backup saved to {:?}", colors::GREEN, colors::RESET, path);
                }
            }
        }))
    }
}

/// Convert a protocol Block to an AttributedChar.
fn block_to_attributed_char(block: &Block) -> AttributedChar {
    let mut attr = TextAttribute::default();
    attr.set_foreground(block.fg as u32);
    attr.set_background(block.bg as u32);

    AttributedChar::new(char::from_u32(block.code as u32).unwrap_or(' '), attr)
}

/// Check if two files have identical content.
fn files_are_identical(path1: &Path, path2: &Path) -> bool {
    use std::fs::File;
    use std::io::{BufReader, Read};

    let file1 = match File::open(path1) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let file2 = match File::open(path2) {
        Ok(f) => f,
        Err(_) => return false,
    };

    // Check file sizes first
    let meta1 = match file1.metadata() {
        Ok(m) => m,
        Err(_) => return false,
    };
    let meta2 = match file2.metadata() {
        Ok(m) => m,
        Err(_) => return false,
    };

    if meta1.len() != meta2.len() {
        return false;
    }

    // Compare content
    let mut reader1 = BufReader::new(file1);
    let mut reader2 = BufReader::new(file2);

    let mut buf1 = [0u8; 8192];
    let mut buf2 = [0u8; 8192];

    loop {
        let n1 = match reader1.read(&mut buf1) {
            Ok(n) => n,
            Err(_) => return false,
        };
        let n2 = match reader2.read(&mut buf2) {
            Ok(n) => n,
            Err(_) => return false,
        };

        if n1 != n2 {
            return false;
        }

        if n1 == 0 {
            return true; // EOF reached, files are identical
        }

        if buf1[..n1] != buf2[..n2] {
            return false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autosave_config_default() {
        let config = AutosaveConfig::default();
        assert!(config.has_periodic_saves());
        assert_eq!(config.interval, Some(Duration::from_secs(3600)));
        assert_eq!(config.format, "ans");
        assert_eq!(config.backup_folder, PathBuf::from("."));
    }

    #[test]
    fn test_autosave_config_builder() {
        let config = AutosaveConfig::new("/tmp/backups")
            .with_interval(Some(Duration::from_secs(300)))
            .with_filename("my_art")
            .with_format("xb");

        assert!(config.has_periodic_saves());
        assert_eq!(config.interval, Some(Duration::from_secs(300)));
        assert_eq!(config.base_filename, "my_art");
        assert_eq!(config.format, "xb");
    }

    #[test]
    fn test_autosave_config_no_periodic() {
        let config = AutosaveConfig::default().with_interval(None);

        assert!(!config.has_periodic_saves());
        assert_eq!(config.interval, None);
    }
}
