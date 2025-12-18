//! Session management for icy_draw
//!
//! Implements VS Code-like "Hot Exit" functionality:
//! - Saves session state (open windows, positions, files) on exit
//! - Restores session on startup (unless CLI args specify a file)
//! - Autosaves unsaved changes to prevent data loss on crash

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use iced::window;
use serde::{Deserialize, Serialize};

use crate::ui::EditMode;

/// Session state that gets saved/restored
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Version of the session format (for future compatibility)
    pub version: u32,
    /// List of window states
    pub windows: Vec<WindowState>,
    /// App version that created this session
    pub app_version: String,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            version: 1,
            windows: Vec::new(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// State of a single window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    /// Window position (x, y)
    pub position: Option<(f32, f32)>,
    /// Window size (width, height)
    pub size: (f32, f32),
    /// Original file path (if any) - the actual file on disk
    pub file_path: Option<PathBuf>,
    /// Edit mode (Ansi, BitFont, Animation)
    pub edit_mode: String,
    /// Whether this window has unsaved changes
    pub has_unsaved_changes: bool,
    /// Autosave file path (if unsaved changes exist)
    pub autosave_path: Option<PathBuf>,
}

/// Information needed to restore a window
#[derive(Debug, Clone)]
pub struct WindowRestoreInfo {
    /// The original file path (shown in title, used for save)
    pub original_path: Option<PathBuf>,
    /// The path to actually load content from (autosave or original)
    pub load_path: Option<PathBuf>,
    /// Whether to mark as dirty after loading
    pub mark_dirty: bool,
    /// Window position
    pub position: Option<(f32, f32)>,
    /// Window size
    pub size: (f32, f32),
}

impl WindowState {
    /// Convert to restore info - determines what to load and how
    pub fn to_restore_info(&self) -> WindowRestoreInfo {
        if self.has_unsaved_changes {
            // Has unsaved changes - load from autosave if available
            WindowRestoreInfo {
                original_path: self.file_path.clone(),
                load_path: self.autosave_path.clone().or(self.file_path.clone()),
                mark_dirty: true,
                position: self.position,
                size: self.size,
            }
        } else {
            // Clean state - load from original file
            WindowRestoreInfo {
                original_path: self.file_path.clone(),
                load_path: self.file_path.clone(),
                mark_dirty: false,
                position: self.position,
                size: self.size,
            }
        }
    }
}

/// Autosave status for a window
pub struct AutosaveStatus {
    /// Undo stack length at last autosave
    pub last_saved_undo_len: usize,
    /// Timer for debouncing
    pub last_change_time: Instant,
    /// Undo stack length at last change detection
    pub last_change_undo_len: usize,
}

impl AutosaveStatus {
    pub fn new(initial_undo_len: usize) -> Self {
        Self {
            last_saved_undo_len: initial_undo_len,
            last_change_time: Instant::now(),
            last_change_undo_len: initial_undo_len,
        }
    }

    /// Check if autosave should be triggered
    /// Returns true if:
    /// - Undo stack has changed since last autosave
    /// - At least `delay_secs` seconds have passed since last change
    pub fn should_autosave(&mut self, current_undo_len: usize, delay_secs: u64) -> bool {
        // No changes since last autosave
        if current_undo_len == self.last_saved_undo_len {
            return false;
        }

        // New change detected - reset timer
        if current_undo_len != self.last_change_undo_len {
            self.last_change_time = Instant::now();
            self.last_change_undo_len = current_undo_len;
            return false;
        }

        // Check if enough time has passed
        if self.last_change_time.elapsed().as_secs() >= delay_secs {
            self.last_saved_undo_len = current_undo_len;
            return true;
        }

        false
    }

    /// Mark autosave as completed
    pub fn mark_saved(&mut self, undo_len: usize) {
        self.last_saved_undo_len = undo_len;
    }
}

/// Session manager handles saving/loading session state
pub struct SessionManager {
    /// Directory for session and autosave files
    session_dir: PathBuf,
    /// Autosave delay in seconds
    autosave_delay_secs: u64,
    /// Autosave status per window (keyed by window::Id)
    autosave_status: HashMap<window::Id, AutosaveStatus>,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        let session_dir = get_session_dir();

        // Ensure directory exists
        if !session_dir.exists() {
            let _ = fs::create_dir_all(&session_dir);
        }

        Self {
            session_dir,
            autosave_delay_secs: 5,
            autosave_status: HashMap::new(),
        }
    }

    /// Get the session file path
    fn session_file_path(&self) -> PathBuf {
        self.session_dir.join("session.json")
    }

    /// Load session state from disk
    pub fn load_session(&self) -> Option<SessionState> {
        let path = self.session_file_path();
        if !path.exists() {
            return None;
        }

        match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(state) => Some(state),
                Err(e) => {
                    log::warn!("Failed to parse session file: {}", e);
                    None
                }
            },
            Err(e) => {
                log::warn!("Failed to read session file: {}", e);
                None
            }
        }
    }

    /// Save session state to disk
    pub fn save_session(&self, state: &SessionState) -> Result<(), String> {
        let path = self.session_file_path();
        log::info!("Saving session to {:?} with {} windows", path, state.windows.len());

        // Use atomic write: write to temp file then rename
        let temp_path = path.with_extension("tmp");

        let json = serde_json::to_string_pretty(state).map_err(|e| format!("Failed to serialize session: {}", e))?;

        fs::write(&temp_path, &json).map_err(|e| format!("Failed to write session file: {}", e))?;

        fs::rename(&temp_path, &path).map_err(|e| format!("Failed to rename session file: {}", e))?;

        log::info!("Session saved to {:?}", path);
        Ok(())
    }

    /// Clear the session (called after successful restore or when starting fresh)
    pub fn clear_session(&self) {
        let path = self.session_file_path();
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
    }

    /// Get autosave file path for a given file
    pub fn get_autosave_path(&self, original_path: &PathBuf) -> PathBuf {
        // Use CRC32 hash of the path for the filename
        let hash = crc32fast::hash(original_path.to_string_lossy().as_bytes());
        self.session_dir.join(format!("{:08x}.autosave", hash))
    }

    /// Get autosave path for an untitled document
    /// Uses a counter that increments for each untitled document
    pub fn get_untitled_autosave_path(&self, untitled_index: usize) -> PathBuf {
        self.session_dir.join(format!("untitled_{}.autosave", untitled_index))
    }

    /// Save autosave data
    pub fn save_autosave(&self, autosave_path: &PathBuf, data: &[u8]) -> Result<(), String> {
        // Atomic write
        let temp_path = autosave_path.with_extension("tmp");

        fs::write(&temp_path, data).map_err(|e| format!("Failed to write autosave: {}", e))?;

        fs::rename(&temp_path, autosave_path).map_err(|e| format!("Failed to rename autosave: {}", e))?;

        log::debug!("Autosave written to {:?}", autosave_path);
        Ok(())
    }

    /// Remove autosave file
    pub fn remove_autosave(&self, autosave_path: &PathBuf) {
        if autosave_path.exists() {
            let _ = fs::remove_file(autosave_path);
            log::debug!("Autosave removed: {:?}", autosave_path);
        }
    }

    /// Get or create autosave status for a window
    pub fn get_autosave_status(&mut self, window_id: window::Id, initial_undo_len: usize) -> &mut AutosaveStatus {
        self.autosave_status.entry(window_id).or_insert_with(|| AutosaveStatus::new(initial_undo_len))
    }

    /// Remove autosave status for a window
    pub fn remove_autosave_status(&mut self, window_id: window::Id) {
        self.autosave_status.remove(&window_id);
    }

    /// Check if autosave should be triggered for a window
    pub fn should_autosave(&mut self, window_id: window::Id, current_undo_len: usize) -> bool {
        if let Some(status) = self.autosave_status.get_mut(&window_id) {
            status.should_autosave(current_undo_len, self.autosave_delay_secs)
        } else {
            false
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the session directory path
fn get_session_dir() -> PathBuf {
    if let Some(proj_dirs) = crate::PROJECT_DIRS.as_ref() {
        let dir = proj_dirs.data_local_dir().join("session");
        return dir;
    }
    // Fallback to config dir
    PathBuf::from(".icy_draw_session")
}

/// Convert EditMode to string for serialization
pub fn edit_mode_to_string(mode: &EditMode) -> String {
    match mode {
        EditMode::Ansi => "ansi".to_string(),
        EditMode::BitFont => "bitfont".to_string(),
        EditMode::CharFont => "charfont".to_string(),
        EditMode::Animation => "animation".to_string(),
    }
}
