//! Session management for icy_draw
//!
//! Implements VS Code-like "Hot Exit" functionality:
//! - Saves session state (open windows, positions, files) on exit
//! - Restores session on startup (unless CLI args specify a file)
//! - Autosaves unsaved changes to prevent data loss on crash
//!
//! Each editor type has its own session state that includes the undo stack
//! plus editor-specific data (caret position, selected glyph, zoom level, etc.)

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use icy_ui::window;
use serde::{Deserialize, Serialize};

use crate::ui::EditMode;

// Re-export the editor-specific session states
pub use icy_engine_edit::bitfont::BitFontSessionState;
pub use icy_engine_edit::AnsiEditorSessionState;

/// Session state for the CharFont (TDF) editor
/// Uses the same undo system as AnsiEditor since it's based on AnsiEditorCore
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CharFontSessionState {
    /// Version for future compatibility
    #[serde(default = "default_version")]
    pub version: u32,

    /// The underlying ansi editor session state
    pub ansi_state: AnsiEditorSessionState,

    /// Currently selected character slot
    #[serde(default)]
    pub selected_slot: usize,

    /// Preview text
    #[serde(default)]
    pub preview_text: String,
}

fn default_version() -> u32 {
    1
}

/// Session state for the Animation editor
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnimationSessionState {
    /// Version for future compatibility
    #[serde(default = "default_version")]
    pub version: u32,

    /// Undo stack (simple text snapshots)
    #[serde(default)]
    pub undo_stack: Vec<String>,

    /// Current frame index
    #[serde(default)]
    pub current_frame: usize,

    /// Playback position in seconds
    #[serde(default)]
    pub playback_position: f64,

    /// Whether currently playing
    #[serde(default)]
    pub is_playing: bool,

    /// Scroll position in script editor
    #[serde(default)]
    pub script_scroll_offset: f32,
}

/// Combined editor session state enum
/// Each variant contains the full state needed to restore that editor type
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EditorSessionData {
    Ansi(AnsiEditorSessionState),
    BitFont(BitFontSessionState),
    CharFont(CharFontSessionState),
    Animation(AnimationSessionState),
}

/// Legacy editor-session payload used before the explicit session-data envelope
/// existed.
///
/// Kept separate from [`EditorSessionData`] so future breaking changes can add
/// fields/variants to the current payload and still keep an intentional V1
/// migration path for old autosave/session files.
#[derive(Clone, Debug, Serialize, Deserialize)]
enum EditorSessionDataV1 {
    Ansi(AnsiEditorSessionState),
    BitFont(BitFontSessionState),
    CharFont(CharFontSessionState),
    Animation(AnimationSessionState),
}

impl From<EditorSessionDataV1> for EditorSessionData {
    fn from(v1: EditorSessionDataV1) -> Self {
        match v1 {
            EditorSessionDataV1::Ansi(state) => EditorSessionData::Ansi(state),
            EditorSessionDataV1::BitFont(state) => EditorSessionData::BitFont(state),
            EditorSessionDataV1::CharFont(state) => EditorSessionData::CharFont(state),
            EditorSessionDataV1::Animation(state) => EditorSessionData::Animation(state),
        }
    }
}

/// Versioned envelope for the per-editor session-data file (`*.session`).
///
/// `session.json` only points at this binary file; the detailed editor state
/// lives here. New writes always use the newest variant. Reads accept both the
/// versioned envelope and historical bare `EditorSessionData` blobs so users do
/// not lose autosave/tool/window state after format changes.
#[derive(Clone, Debug, Serialize, Deserialize)]
enum EditorSessionDataVersion {
    V1(EditorSessionDataV1),
    V2(EditorSessionData),
}

const EDITOR_SESSION_DATA_MAGIC: [u8; 4] = *b"ICYS";

/// Binary session-data envelope.
///
/// The magic prefix makes the new format unambiguous from historical bare
/// `EditorSessionData` enum blobs, which is important because both are bitcode
/// enums and can otherwise be difficult to distinguish safely.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct EditorSessionDataEnvelope {
    magic: [u8; 4],
    payload: EditorSessionDataVersion,
}

impl EditorSessionDataVersion {
    fn into_current(self) -> EditorSessionData {
        match self {
            EditorSessionDataVersion::V1(v1) => v1.into(),
            EditorSessionDataVersion::V2(v2) => v2,
        }
    }
}

impl Default for EditorSessionData {
    fn default() -> Self {
        Self::Ansi(AnsiEditorSessionState::default())
    }
}

impl EditorSessionData {
    /// Encode using an explicit versioned envelope.
    fn encode_versioned(&self) -> Result<Vec<u8>, bitcode::Error> {
        bitcode::serialize(&EditorSessionDataEnvelope {
            magic: EDITOR_SESSION_DATA_MAGIC,
            payload: EditorSessionDataVersion::V2(self.clone()),
        })
    }

    /// Decode current or legacy editor-session data.
    ///
    /// Accepted input shapes:
    /// - `EditorSessionDataVersion::V2(current)` (current writer)
    /// - `EditorSessionDataVersion::V1(legacy)` (explicit legacy envelope)
    /// - bare `EditorSessionData` (pre-#1 session files)
    /// - bare `EditorSessionDataV1` (pre-#1 legacy alias)
    fn decode_versioned(bytes: &[u8]) -> Result<Self, bitcode::Error> {
        if let Ok(envelope) = bitcode::deserialize::<EditorSessionDataEnvelope>(bytes) {
            if envelope.magic == EDITOR_SESSION_DATA_MAGIC {
                return Ok(envelope.payload.into_current());
            }
        }

        if let Ok(current) = bitcode::deserialize::<EditorSessionData>(bytes) {
            return Ok(current);
        }

        bitcode::deserialize::<EditorSessionDataV1>(bytes).map(Into::into)
    }
}

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
    /// Path to the editor session data file (bitcode serialized)
    #[serde(default)]
    pub session_data_path: Option<PathBuf>,
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
    /// Path to the serialized editor session data (bitcode)
    pub session_data_path: Option<PathBuf>,
}

impl WindowState {
    /// Convert to restore info - determines what to load and how
    pub fn to_restore_info(&self) -> WindowRestoreInfo {
        if self.has_unsaved_changes {
            // Has unsaved changes - load from autosave if available AND exists
            let autosave_exists = self.autosave_path.as_ref().map_or(false, |p| p.exists());
            let load_path = if autosave_exists {
                self.autosave_path.clone()
            } else {
                // Autosave file doesn't exist anymore, fall back to original
                self.file_path.clone()
            };
            WindowRestoreInfo {
                original_path: self.file_path.clone(),
                load_path,
                mark_dirty: autosave_exists, // Only mark dirty if we're actually loading autosave
                position: self.position,
                size: self.size,
                session_data_path: self.session_data_path.clone(),
            }
        } else {
            // Clean state - load from original file
            WindowRestoreInfo {
                original_path: self.file_path.clone(),
                load_path: self.file_path.clone(),
                mark_dirty: false,
                position: self.position,
                size: self.size,
                session_data_path: self.session_data_path.clone(),
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
        let status = self.autosave_status.entry(window_id).or_insert_with(|| AutosaveStatus::new(current_undo_len));
        status.should_autosave(current_undo_len, self.autosave_delay_secs)
    }

    /// Get session data file path for a given file
    pub fn get_session_data_path(&self, original_path: &PathBuf) -> PathBuf {
        let hash = crc32fast::hash(original_path.to_string_lossy().as_bytes());
        self.session_dir.join(format!("{:08x}.session", hash))
    }

    /// Get session data path for an untitled document
    pub fn get_untitled_session_data_path(&self, untitled_index: usize) -> PathBuf {
        self.session_dir.join(format!("untitled_{}.session", untitled_index))
    }

    /// Save editor session data using a versioned bitcode envelope.
    pub fn save_session_data(&self, path: &PathBuf, data: &EditorSessionData) -> Result<(), String> {
        let bytes = data.encode_versioned().map_err(|e| format!("Failed to serialize session data: {}", e))?;

        // Atomic write
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, &bytes).map_err(|e| format!("Failed to write session data: {}", e))?;
        fs::rename(&temp_path, path).map_err(|e| format!("Failed to rename session data: {}", e))?;

        log::debug!("Session data saved to {:?} ({} bytes)", path, bytes.len());
        Ok(())
    }

    /// Load editor session data using bitcode, accepting both current versioned
    /// envelopes and legacy bare payloads.
    pub fn load_session_data(&self, path: &PathBuf) -> Option<EditorSessionData> {
        if !path.exists() {
            return None;
        }

        match fs::read(path) {
            Ok(bytes) => match EditorSessionData::decode_versioned(&bytes) {
                Ok(data) => {
                    log::debug!("Session data loaded from {:?}", path);
                    Some(data)
                }
                Err(e) => {
                    log::warn!("Failed to deserialize session data from {:?}: {}", path, e);
                    None
                }
            },
            Err(e) => {
                log::warn!("Failed to read session data from {:?}: {}", path, e);
                None
            }
        }
    }

    /// Remove session data file
    #[allow(dead_code)]
    pub fn remove_session_data(&self, path: &PathBuf) {
        if path.exists() {
            let _ = fs::remove_file(path);
            log::debug!("Session data removed: {:?}", path);
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn sample_ansi_data() -> EditorSessionData {
        let mut state = AnsiEditorSessionState::default();
        state.selected_tool = "Pencil".to_string();
        state.zoom_level = 2.0;
        state.scroll_offset = (12.0, 34.0);
        EditorSessionData::Ansi(state)
    }

    fn sample_bitfont_data() -> EditorSessionData {
        let mut state = BitFontSessionState::default();
        state.selected_glyph = 42;
        state.edit_zoom = 3.0;
        state.selected_tool = "Pencil".to_string();
        EditorSessionData::BitFont(state)
    }

    fn sample_charfont_data() -> EditorSessionData {
        let mut state = CharFontSessionState::default();
        state.selected_slot = 65;
        state.preview_text = "ICE".to_string();
        state.ansi_state.selected_tool = "Font".to_string();
        EditorSessionData::CharFont(state)
    }

    fn sample_animation_data() -> EditorSessionData {
        let state = AnimationSessionState {
            current_frame: 7,
            playback_position: 1.25,
            is_playing: true,
            script_scroll_offset: 99.0,
            ..AnimationSessionState::default()
        };
        EditorSessionData::Animation(state)
    }

    fn assert_same_session_data(actual: &EditorSessionData, expected: &EditorSessionData) {
        match (actual, expected) {
            (EditorSessionData::Ansi(actual), EditorSessionData::Ansi(expected)) => {
                assert_eq!(actual.selected_tool, expected.selected_tool);
                assert_eq!(actual.zoom_level, expected.zoom_level);
                assert_eq!(actual.scroll_offset, expected.scroll_offset);
            }
            (EditorSessionData::BitFont(actual), EditorSessionData::BitFont(expected)) => {
                assert_eq!(actual.selected_glyph, expected.selected_glyph);
                assert_eq!(actual.edit_zoom, expected.edit_zoom);
                assert_eq!(actual.selected_tool, expected.selected_tool);
            }
            (EditorSessionData::CharFont(actual), EditorSessionData::CharFont(expected)) => {
                assert_eq!(actual.selected_slot, expected.selected_slot);
                assert_eq!(actual.preview_text, expected.preview_text);
                assert_eq!(actual.ansi_state.selected_tool, expected.ansi_state.selected_tool);
            }
            (EditorSessionData::Animation(actual), EditorSessionData::Animation(expected)) => {
                assert_eq!(actual.current_frame, expected.current_frame);
                assert_eq!(actual.playback_position, expected.playback_position);
                assert_eq!(actual.is_playing, expected.is_playing);
                assert_eq!(actual.script_scroll_offset, expected.script_scroll_offset);
            }
            _ => panic!("session data variants differ: actual={actual:?}, expected={expected:?}"),
        }
    }

    fn explicit_v1_from_current(data: EditorSessionData) -> EditorSessionDataV1 {
        match data {
            EditorSessionData::Ansi(state) => EditorSessionDataV1::Ansi(state),
            EditorSessionData::BitFont(state) => EditorSessionDataV1::BitFont(state),
            EditorSessionData::CharFont(state) => EditorSessionDataV1::CharFont(state),
            EditorSessionData::Animation(state) => EditorSessionDataV1::Animation(state),
        }
    }

    fn temp_session_manager() -> (SessionManager, PathBuf) {
        let dir = std::env::temp_dir().join(format!("icy_draw_session_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp session dir");
        let manager = SessionManager {
            session_dir: dir.clone(),
            autosave_delay_secs: 5,
            autosave_status: HashMap::new(),
        };
        (manager, dir)
    }

    #[test]
    fn encode_writes_magic_v2_envelope() {
        let data = sample_ansi_data();
        let bytes = data.encode_versioned().expect("encode versioned session data");

        let envelope = bitcode::deserialize::<EditorSessionDataEnvelope>(&bytes).expect("decode envelope");
        assert_eq!(envelope.magic, EDITOR_SESSION_DATA_MAGIC);
        match envelope.payload {
            EditorSessionDataVersion::V2(decoded) => assert_same_session_data(&decoded, &data),
            EditorSessionDataVersion::V1(_) => panic!("new encoder must not write V1"),
        }
    }

    #[test]
    fn decode_current_v2_round_trips_all_editor_variants() {
        for data in [sample_ansi_data(), sample_bitfont_data(), sample_charfont_data(), sample_animation_data()] {
            let bytes = data.encode_versioned().expect("encode current session data");
            let decoded = EditorSessionData::decode_versioned(&bytes).expect("decode current session data");
            assert_same_session_data(&decoded, &data);
        }
    }

    #[test]
    fn decode_explicit_v1_envelope_migrates_to_current() {
        let current = sample_charfont_data();
        let legacy = explicit_v1_from_current(current.clone());
        let bytes = bitcode::serialize(&EditorSessionDataEnvelope {
            magic: EDITOR_SESSION_DATA_MAGIC,
            payload: EditorSessionDataVersion::V1(legacy),
        })
        .expect("serialize explicit v1 envelope");

        let decoded = EditorSessionData::decode_versioned(&bytes).expect("decode migrated v1 envelope");
        assert_same_session_data(&decoded, &current);
    }

    #[test]
    fn decode_legacy_bare_editor_session_data() {
        let current = sample_bitfont_data();
        let bytes = bitcode::serialize(&current).expect("serialize legacy bare current data");

        let decoded = EditorSessionData::decode_versioned(&bytes).expect("decode bare current data");
        assert_same_session_data(&decoded, &current);
    }

    #[test]
    fn decode_legacy_bare_v1_data() {
        let current = sample_animation_data();
        let legacy = explicit_v1_from_current(current.clone());
        let bytes = bitcode::serialize(&legacy).expect("serialize legacy bare v1 data");

        let decoded = EditorSessionData::decode_versioned(&bytes).expect("decode bare v1 data");
        assert_same_session_data(&decoded, &current);
    }

    #[test]
    fn decode_rejects_invalid_envelope_magic() {
        let data = sample_ansi_data();
        let bytes = bitcode::serialize(&EditorSessionDataEnvelope {
            magic: *b"BAD!",
            payload: EditorSessionDataVersion::V2(data),
        })
        .expect("serialize invalid envelope");

        assert!(EditorSessionData::decode_versioned(&bytes).is_err());
    }

    #[test]
    fn session_manager_saves_and_loads_versioned_data() {
        let (manager, dir) = temp_session_manager();
        let path = dir.join("editor.session");
        let data = sample_ansi_data();

        manager.save_session_data(&path, &data).expect("save session data");

        let bytes = fs::read(&path).expect("read session data file");
        let envelope = bitcode::deserialize::<EditorSessionDataEnvelope>(&bytes).expect("saved file should be versioned envelope");
        assert_eq!(envelope.magic, EDITOR_SESSION_DATA_MAGIC);

        let loaded = manager.load_session_data(&path).expect("load session data");
        assert_same_session_data(&loaded, &data);

        let _ = fs::remove_dir_all(dir);
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
