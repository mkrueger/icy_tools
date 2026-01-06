//! MainWindow for icy_draw
//!
//! Each MainWindow represents one editing window with its own state and mode.
//! The mode determines what kind of editor is shown (ANSI, BitFont, CharFont, Animation).

use std::collections::hash_map::DefaultHasher;
use std::{
    cell::Cell,
    collections::HashMap,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::Arc,
};

use parking_lot::RwLock;

use crate::{fl, SharedFontLibrary};
use icy_engine::formats::FileFormat;
use icy_engine::TextPane;
use icy_engine_edit::UndoState;
use icy_engine_gui::commands::cmd;
use icy_engine_gui::ui::{confirm_yes_no_cancel, error_dialog, DialogResult, DialogStack, ExportDialogMessage};
use icy_engine_gui::{command_handler, command_handlers, Toast, ToastManager};
use icy_ui::{
    widget::{button, column, container, mouse_area, row, rule, text, Space},
    Alignment, Border, Color, Element, Event, Length, Subscription, Task, Theme,
};

use super::commands::create_draw_commands;
use crate::ui::collaboration::CollaborationState;
use crate::ui::editor::animation::{AnimationEditor, AnimationEditorMessage};
use crate::ui::editor::ansi::{AnsiEditorCoreMessage, AnsiEditorMainArea, AnsiEditorMessage, AnsiStatusInfo};
use crate::ui::editor::bitfont::{BitFontEditor, BitFontEditorMessage};
use crate::Plugin;
use crate::Settings;

/// Undo/Redo information for menu display
#[derive(Default, Clone)]
pub struct UndoInfo {
    /// Description of the next undo operation (None if nothing to undo)
    pub undo_description: Option<String>,
    /// Description of the next redo operation (None if nothing to redo)
    pub redo_description: Option<String>,
}

impl UndoInfo {
    pub fn new(undo_description: Option<String>, redo_description: Option<String>) -> Self {
        Self {
            undo_description,
            redo_description,
        }
    }
}

/// The editing mode of a window
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditMode {
    /// ANSI/ASCII art editor - the main mode
    Ansi,
    /// BitFont editor for editing bitmap fonts
    BitFont,
    /// CharFont editor for editing TDF character fonts
    CharFont,
    /// Animation editor for Lua-scripted ANSI animations
    Animation,
}

use super::commands::{area_cmd, color_cmd, selection_cmd, view_cmd};

// Command handler for MainWindow keyboard shortcuts
command_handler!(MainWindowCommands, create_draw_commands(), => Message {
    // View
    cmd::VIEW_FULLSCREEN => Message::ToggleFullscreen,
    view_cmd::REFERENCE_IMAGE => Message::AnsiEditor(AnsiEditorMessage::ShowReferenceImageDialog),
    view_cmd::TOGGLE_REFERENCE_IMAGE => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleReferenceImage)),
    cmd::HELP_ABOUT => Message::ShowAbout,

    // Colors
    color_cmd::NEXT_FG => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::NextFgColor)),
    color_cmd::PREV_FG => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PrevFgColor)),
    color_cmd::NEXT_BG => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::NextBgColor)),
    color_cmd::PREV_BG => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PrevBgColor)),
    color_cmd::PICK_ATTRIBUTE_UNDER_CARET => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PickAttributeUnderCaret)),
    color_cmd::SWAP => Message::AnsiEditor(AnsiEditorMessage::ColorSwitcher(crate::ui::ColorSwitcherMessage::SwapColors)),
    // Selection
    selection_cmd::SELECT_NONE => Message::Deselect,
    selection_cmd::SELECT_INVERSE => Message::AnsiEditor(AnsiEditorMessage::InverseSelection),
    selection_cmd::SELECT_FLIP_X => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::FlipX)),
    selection_cmd::SELECT_FLIP_Y => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::FlipY)),
    selection_cmd::SELECT_CROP => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::Crop)),
    selection_cmd::SELECT_JUSTIFY_LEFT => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLeft)),
    selection_cmd::SELECT_JUSTIFY_CENTER => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyCenter)),
    selection_cmd::SELECT_JUSTIFY_RIGHT => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyRight)),
    // Area operations (forwarded to ANSI editor)
    area_cmd::JUSTIFY_LINE_LEFT => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLineLeft)),
    area_cmd::JUSTIFY_LINE_CENTER => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLineCenter)),
    area_cmd::JUSTIFY_LINE_RIGHT => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLineRight)),
    area_cmd::INSERT_ROW => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::InsertRow)),
    area_cmd::DELETE_ROW => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::DeleteRow)),
    area_cmd::INSERT_COLUMN => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::InsertColumn)),
    area_cmd::DELETE_COLUMN => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::DeleteColumn)),
    area_cmd::ERASE_ROW => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseRow)),
    area_cmd::ERASE_ROW_TO_START => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseRowToStart)),
    area_cmd::ERASE_ROW_TO_END => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseRowToEnd)),
    area_cmd::ERASE_COLUMN => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseColumn)),
    area_cmd::ERASE_COLUMN_TO_START => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseColumnToStart)),
    area_cmd::ERASE_COLUMN_TO_END => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseColumnToEnd)),
    area_cmd::SCROLL_UP => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ScrollAreaUp)),
    area_cmd::SCROLL_DOWN => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ScrollAreaDown)),
    area_cmd::SCROLL_LEFT => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ScrollAreaLeft)),
    area_cmd::SCROLL_RIGHT => Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ScrollAreaRight)),
});

impl Default for EditMode {
    fn default() -> Self {
        Self::Ansi
    }
}

impl std::fmt::Display for EditMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ansi => write!(f, "ANSI"),
            Self::BitFont => write!(f, "BitFont"),
            Self::CharFont => write!(f, "CharFont"),
            Self::Animation => write!(f, "Animation"),
        }
    }
}

/// State for the BitFont editor mode - now uses the full BitFontEditor
pub type BitFontEditorState = BitFontEditor;

/// State for the CharFont (TDF) editor mode - now uses the full CharFontEditor
pub type CharFontEditorState = crate::ui::editor::charfont::CharFontEditor;

/// Mode-specific state
pub enum ModeState {
    Ansi(AnsiEditorMainArea),
    BitFont(BitFontEditorState),
    CharFont(CharFontEditorState),
    Animation(AnimationEditor),
}

impl ModeState {
    pub fn mode(&self) -> EditMode {
        match self {
            Self::Ansi(_) => EditMode::Ansi,
            Self::BitFont(_) => EditMode::BitFont,
            Self::CharFont(_) => EditMode::CharFont,
            Self::Animation(_) => EditMode::Animation,
        }
    }

    /// Get the current undo stack length for dirty tracking
    pub fn undo_stack_len(&self) -> usize {
        match self {
            Self::Ansi(editor) => editor.undo_stack_len(),
            Self::BitFont(editor) => editor.undo_stack_len(),
            Self::CharFont(editor) => editor.undo_stack_len(),
            Self::Animation(editor) => editor.undo_stack_len(),
        }
    }

    /// Get a clone of the undo stack for serialization (Ansi editor only for now)
    #[allow(dead_code)]
    pub fn get_undo_stack(&self) -> Option<icy_engine_edit::EditorUndoStack> {
        match self {
            Self::Ansi(editor) => editor.get_undo_stack(),
            // BitFont and CharFont use different undo stack types - future work
            Self::BitFont(_) => None,
            Self::CharFont(_) => None,
            Self::Animation(_) => None,
        }
    }

    /// Restore undo stack from serialization (Ansi editor only for now)
    #[allow(dead_code)]
    pub fn set_undo_stack(&mut self, stack: icy_engine_edit::EditorUndoStack) {
        match self {
            Self::Ansi(editor) => editor.set_undo_stack(stack),
            // BitFont and CharFont use different undo stack types - future work
            Self::BitFont(_) => {}
            Self::CharFont(_) => {}
            Self::Animation(_) => {}
        }
    }

    /// Get session data for serialization
    pub fn get_session_data(&self) -> Option<crate::session::EditorSessionData> {
        match self {
            Self::Ansi(editor) => editor.get_session_data().map(crate::session::EditorSessionData::Ansi),
            Self::BitFont(editor) => editor.get_session_data().map(crate::session::EditorSessionData::BitFont),
            Self::CharFont(editor) => editor.get_session_data().map(crate::session::EditorSessionData::CharFont),
            Self::Animation(editor) => editor.get_session_data().map(crate::session::EditorSessionData::Animation),
        }
    }

    /// Restore session data from serialization
    pub fn set_session_data(&mut self, data: crate::session::EditorSessionData) {
        match (self, data) {
            (Self::Ansi(editor), crate::session::EditorSessionData::Ansi(state)) => editor.set_session_data(state),
            (Self::BitFont(editor), crate::session::EditorSessionData::BitFont(state)) => editor.set_session_data(state),
            (Self::CharFont(editor), crate::session::EditorSessionData::CharFont(state)) => editor.set_session_data(state),
            (Self::Animation(editor), crate::session::EditorSessionData::Animation(state)) => editor.set_session_data(state),
            _ => log::warn!("Session data type mismatch"),
        }
    }

    /// Get the file path if any
    pub fn file_path(&self) -> Option<&PathBuf> {
        match self {
            Self::Ansi(editor) => editor.file_path(),
            Self::BitFont(editor) => editor.file_path(),
            Self::CharFont(editor) => editor.file_path(),
            Self::Animation(editor) => editor.file_path(),
        }
    }

    /// Set the file path
    pub fn set_file_path(&mut self, path: PathBuf) {
        match self {
            Self::Ansi(editor) => editor.set_file_path(path),
            Self::BitFont(editor) => editor.set_file_path(path),
            Self::CharFont(editor) => editor.set_file_path(path),
            Self::Animation(editor) => editor.set_file_path(path),
        }
    }

    /// Save the document to the given path
    pub fn save(&mut self, path: &std::path::Path) -> Result<(), String> {
        match self {
            Self::Ansi(editor) => editor.save(path),
            Self::BitFont(editor) => editor.save(path),
            Self::CharFont(editor) => editor.save(path),
            Self::Animation(editor) => editor.save(path),
        }
    }

    /// Get bytes for autosave (without modifying the file path)
    pub fn get_autosave_bytes(&self) -> Result<Vec<u8>, String> {
        match self {
            Self::Ansi(editor) => editor.get_autosave_bytes(),
            Self::BitFont(editor) => editor.get_autosave_bytes(),
            Self::CharFont(editor) => editor.get_autosave_bytes(),
            Self::Animation(editor) => editor.get_autosave_bytes(),
        }
    }

    /// Get the standard/native file format for this mode.
    ///
    /// Returns `(extension, localized filter label)`.
    pub fn file_format(&self) -> (&'static str, String) {
        match self {
            Self::Ansi(_) => ("icy", fl!("file-dialog-filter-icydraw-files")),
            Self::BitFont(_) => ("psf", fl!("file-dialog-filter-font-files")),
            Self::CharFont(_) => ("tdf", fl!("file-dialog-filter-tdf-files")),
            Self::Animation(_) => ("icyanim", fl!("file-dialog-filter-animation-files")),
        }
    }
}

pub(super) fn enforce_extension(mut path: PathBuf, required_ext: &str) -> PathBuf {
    if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case(required_ext)) != Some(true) {
        path.set_extension(required_ext);
    }
    path
}

/// Result from reading clipboard data
/// Holds all available clipboard formats read in one operation
#[derive(Clone, Debug)]
pub struct ClipboardReadResult {
    /// ICY binary format data (if available)
    pub icy_data: Option<Vec<u8>>,
    /// Image data as RGBA with dimensions (if available)
    pub image: Option<image::RgbaImage>,
    /// Plain text content (if available)
    pub text: Option<String>,
}

/// Message type for MainWindow
#[derive(Clone, Debug)]
pub enum Message {
    /// No-op message used by UI widgets that need an `on_press` but should not trigger any updates.
    Noop,
    // ═══════════════════════════════════════════════════════════════════════════
    // File operations
    // ═══════════════════════════════════════════════════════════════════════════
    NewFile,
    OpenFile,
    OpenRecentFile(PathBuf),
    ClearRecentFiles,
    FileOpened(PathBuf),
    FileLoadError(String, String), // (title, error_message)
    SaveFile,
    SaveFileAs,
    /// Emitted after a successful save (Save or Save As)
    SaveSucceeded(PathBuf),
    FileSaved(PathBuf), // Path where file was saved (from SaveAs dialog)
    ExportFile,
    CloseFile,
    /// Close the current editor window (Ctrl+W semantics)
    CloseEditor,
    /// Quit the application (close all windows)
    QuitApp,
    /// Save the file and then close the window
    SaveAndCloseFile,
    /// Close without saving (user confirmed "Don't Save")
    ForceCloseFile,
    /// Save and then open a new file (after dirty check)
    SaveAndNewFile,
    /// Open new file without saving (user confirmed "Don't Save")
    ForceNewFile,
    /// Save and then open a file (after dirty check)
    SaveAndOpenFile(PathBuf),
    /// Open file without saving (user confirmed "Don't Save")
    ForceOpenFile(PathBuf),
    /// Show open file dialog without dirty check (user confirmed "Don't Save")
    ForceShowOpenDialog,
    ShowSettings,
    ShowImportFontDialog,

    SettingsDialog(crate::ui::dialog::settings::SettingsDialogMessage),
    SettingsSaved(crate::ui::dialog::settings::SettingsResult),

    ExportDialog(ExportDialogMessage),
    ExportComplete(PathBuf),

    // ═══════════════════════════════════════════════════════════════════════════
    // Dialog
    // ═══════════════════════════════════════════════════════════════════════════
    CloseDialog,

    // ═══════════════════════════════════════════════════════════════════════════
    // Edit operations
    // ═══════════════════════════════════════════════════════════════════════════
    Undo,
    Redo,
    Cut,
    Copy,
    /// Copy completed (clipboard write finished)
    CopyCompleted(Result<(), String>),
    Paste,
    /// Clipboard data received for paste operation
    ClipboardDataForPaste(Option<ClipboardReadResult>),
    /// Paste clipboard image (or ICY buffer) as a new ANSI document with a Sixel layer
    PasteAsNewImage,
    /// Clipboard data received for PasteAsNewImage operation
    ClipboardDataForNewImage(Option<ClipboardReadResult>),
    /// Open file dialog to insert a Sixel from an image file
    InsertSixelFromFile,
    /// Insert a Sixel from the selected image file path
    InsertSixelFromPath(std::path::PathBuf),
    /// Request WindowManager to open a new window with pending buffer
    OpenNewWindowWithBuffer,
    SelectAll,
    Deselect,

    // ═══════════════════════════════════════════════════════════════════════════
    // View operations
    // ═══════════════════════════════════════════════════════════════════════════
    ZoomIn,
    ZoomOut,
    ZoomReset,
    SetZoom(f32),
    ToggleFullscreen,

    // New File Dialog
    NewFileDialog(crate::ui::dialog::new_file::NewFileMessage),
    NewFileCreated(crate::ui::dialog::new_file::FileTemplate, i32, i32),

    // ═══════════════════════════════════════════════════════════════════════════
    // Help
    // ═══════════════════════════════════════════════════════════════════════════
    OpenDiscussions,
    OpenLogFile,
    ReportBug,
    ShowAbout,
    /// Open the GitHub releases page for the latest version
    OpenReleasesPage,
    AboutDialog(icy_engine_gui::ui::AboutDialogMessage),

    // ═══════════════════════════════════════════════════════════════════════════
    // Mode switching
    // ═══════════════════════════════════════════════════════════════════════════
    SwitchMode(EditMode),

    // ANSI Editor messages
    AnsiEditor(AnsiEditorMessage),

    // BitFont Editor messages
    BitFontEditor(BitFontEditorMessage),

    // CharFont (TDF) Editor messages
    CharFontEditor(crate::ui::editor::charfont::CharFontEditorMessage),

    // Animation Editor messages
    AnimationEditor(AnimationEditorMessage),

    // File Settings Dialog
    ShowFileSettingsDialog,
    FileSettingsDialog(crate::ui::editor::ansi::FileSettingsDialogMessage),
    ApplyFileSettings(crate::ui::editor::ansi::FileSettingsResult),

    // Toast notifications
    ShowToast(Toast),
    CloseToast(usize),

    // ═══════════════════════════════════════════════════════════════════════════
    // Collaboration
    // ═══════════════════════════════════════════════════════════════════════════
    /// Show connect to server dialog
    ShowConnectDialog,
    /// Toggle chat panel visibility
    ToggleChatPanel,
    /// Connect dialog messages
    ConnectDialog(crate::ui::dialog::ConnectDialogMessage),
    /// Connect to server with result from dialog
    ConnectToServer(crate::ui::dialog::ConnectDialogResult),
    /// Chat panel messages
    ChatPanel(crate::ui::collaboration::ChatPanelMessage),
    /// Collaboration subscription message
    Collaboration(crate::ui::collaboration::CollaborationMessage),
}

/// Status bar information for ANSI editor
#[derive(Clone, Debug, Default)]
pub struct AnsiStatusBarInfo {
    /// Caret position (x, y) - None if tool doesn't show caret
    pub cursor_position: Option<(i32, i32)>,
    /// Selection range (min, max) - None if no selection
    pub selection_range: Option<((i32, i32), (i32, i32))>,
    /// Buffer size (width, height)
    pub buffer_size: (i32, i32),
    /// Font name for display
    pub font_name: String,
    /// Ice colors mode enabled
    pub ice_colors: bool,
    /// Letter spacing (9px mode) enabled
    pub letter_spacing: bool,
    /// Use aspect ratio correction
    pub use_aspect_ratio: bool,
    /// For XBinExtended: slot font info
    pub slot_fonts: Option<SlotFontsInfo>,
}

/// Font slot information for XBinExtended mode
#[derive(Clone, Debug)]
pub struct SlotFontsInfo {
    pub current_slot: usize,
    pub slot0_name: Option<String>,
    pub slot1_name: Option<String>,
}

impl From<AnsiStatusInfo> for AnsiStatusBarInfo {
    fn from(info: AnsiStatusInfo) -> Self {
        let slot_fonts = if info.format_mode == icy_engine_edit::FormatMode::XBinExtended {
            info.slot_fonts.map(|fonts| SlotFontsInfo {
                current_slot: info.current_font_slot,
                slot0_name: fonts[0].clone(),
                slot1_name: fonts[1].clone(),
            })
        } else {
            None
        };

        let selection_range = info.selection_range.map(|(min_x, min_y, max_x, max_y)| ((min_x, min_y), (max_x, max_y)));

        Self {
            cursor_position: info.cursor_position,
            selection_range,
            buffer_size: info.buffer_size,
            font_name: info.font_name,
            ice_colors: info.ice_colors,
            letter_spacing: info.letter_spacing,
            use_aspect_ratio: info.use_aspect_ratio,
            slot_fonts,
        }
    }
}

// Command handler for MainWindow - maps hotkeys to messages
command_handlers! {
    fn handle_main_window_command() -> Option<Message> {
        cmd::EDIT_UNDO => Message::Undo,
        cmd::EDIT_REDO => Message::Redo,
        cmd::FILE_NEW => Message::NewFile,
        cmd::FILE_OPEN => Message::OpenFile,
        cmd::FILE_SAVE => Message::SaveFile,
        cmd::FILE_SAVE_AS => Message::SaveFileAs,
        cmd::SETTINGS_OPEN => Message::ShowSettings,
        cmd::VIEW_ZOOM_IN => Message::ZoomIn,
        cmd::VIEW_ZOOM_OUT => Message::ZoomOut,
        cmd::VIEW_ZOOM_RESET => Message::ZoomReset,
    }
}

/// A single editing window
pub struct MainWindow {
    /// Window ID (1-based, for Alt+N switching)
    pub id: usize,

    /// Current editing mode and state
    pub(super) mode_state: ModeState,

    /// Shared options
    pub(super) options: Arc<RwLock<Settings>>,

    /// Shared font library for TDF/Figlet fonts
    pub(super) font_library: SharedFontLibrary,

    /// Fullscreen mode toggle
    is_fullscreen: bool,

    /// Dialog stack for modal dialogs
    pub(super) dialogs: DialogStack<Message>,

    /// Command set for hotkey handling
    commands: MainWindowCommands,

    /// Undo stack length at last save - for dirty tracking
    pub(super) last_save: usize,

    /// Close the window after a successful save (for SaveAndClose flow)
    pub(super) close_after_save: bool,

    /// Pending file to open after save (None inside = new file, Some(path) = open path)
    pub(super) pending_open_path: Option<Option<PathBuf>>,

    /// Cached title string for Window trait (updated when file changes)
    pub title: String,

    /// Bumps on every UI update (used to cache expensive view-derived data)
    ui_revision: Cell<u64>,

    /// Loaded plugins from the plugin directory
    plugins: Arc<Vec<Plugin>>,

    /// Toast notifications for user feedback
    toasts: Vec<Toast>,

    /// Collaboration state for real-time editing
    pub(super) collaboration_state: CollaborationState,

    /// Cached rendered previews for remote paste-as-selection blocks
    remote_paste_preview_cache: HashMap<u32, CachedRemotePastePreview>,
}

#[derive(Clone)]
struct CachedRemotePastePreview {
    blocks_hash: u64,
    handle: icy_ui::widget::image::Handle,
    width_px: u32,
    height_px: u32,
    columns: u32,
    rows: u32,
}

impl MainWindow {
    pub fn new(id: usize, path: Option<PathBuf>, options: Arc<RwLock<Settings>>, font_library: SharedFontLibrary) -> Self {
        let (mode_state, initial_error) = if let Some(ref p) = path {
            // Determine mode based on file format
            let format = FileFormat::from_path(p);

            match format {
                Some(FileFormat::BitFont(_)) => {
                    // BitFont format detected (yaff, psf, fXX)
                    match BitFontEditor::from_file(p.clone()) {
                        Ok(editor) => (ModeState::BitFont(editor), None),
                        Err(e) => {
                            let error = Some(("Error Loading Font".to_string(), e));
                            log::error!("Error loading BitFont file '{}': {}", p.display(), error.as_ref().unwrap().1);
                            (ModeState::BitFont(BitFontEditor::new()), error)
                        }
                    }
                }
                Some(FileFormat::IcyAnim) => {
                    // Animation script format
                    match AnimationEditor::load_file(p.clone()) {
                        Ok(editor) => (ModeState::Animation(editor), None),
                        Err(e) => {
                            let error = Some(("Error Loading Animation".to_string(), e));
                            log::error!("Error loading Animation file '{}': {}", p.display(), error.as_ref().unwrap().1);
                            (ModeState::Animation(AnimationEditor::new()), error)
                        }
                    }
                }
                Some(FileFormat::CharacterFont(_)) => {
                    // TDF character font format
                    match crate::ui::editor::charfont::CharFontEditor::with_file(p.clone(), options.clone(), font_library.clone()) {
                        Ok(editor) => (ModeState::CharFont(editor), None),
                        Err(e) => {
                            let error = Some(("Error Loading TDF Font".to_string(), format!("{}", e)));
                            log::error!("Error loading TDF Font file '{}': {}", p.display(), error.as_ref().unwrap().1);
                            (
                                ModeState::CharFont(crate::ui::editor::charfont::CharFontEditor::new(options.clone(), font_library.clone())),
                                error,
                            )
                        }
                    }
                }
                _ => {
                    // Try as ANSI/ASCII art file
                    match AnsiEditorMainArea::with_file(p.clone(), options.clone(), font_library.clone()) {
                        Ok(editor) => (ModeState::Ansi(editor), None),
                        Err(e) => {
                            let error = Some(("Error Loading File".to_string(), format!("Failed to load '{}': {}", p.display(), e)));
                            log::error!("Error loading file '{}': {}", p.display(), error.as_ref().unwrap().1);
                            (ModeState::Ansi(AnsiEditorMainArea::new(options.clone(), font_library.clone())), error)
                        }
                    }
                }
            }
        } else {
            (ModeState::Ansi(AnsiEditorMainArea::new(options.clone(), font_library.clone())), None)
        };

        let last_save = mode_state.undo_stack_len();

        let mut dialogs = DialogStack::new();
        if let Some((title, message)) = initial_error {
            dialogs.push(error_dialog(title, message, |_| Message::CloseDialog));
        }

        let mut window = Self {
            id,
            mode_state,
            options,
            font_library,
            is_fullscreen: false,
            dialogs,
            commands: MainWindowCommands::new(),
            last_save,
            close_after_save: false,
            pending_open_path: None,
            title: String::new(),
            ui_revision: Cell::new(0),
            plugins: Arc::new(Plugin::read_plugin_directory()),
            toasts: Vec::new(),
            collaboration_state: CollaborationState::new(),
            remote_paste_preview_cache: HashMap::new(),
        };
        window.update_title();
        window
    }

    /// Create a MainWindow with a pre-existing TextBuffer (e.g., from paste as new image)
    pub fn with_buffer(id: usize, buffer: icy_engine::TextBuffer, options: Arc<RwLock<Settings>>, font_library: SharedFontLibrary) -> Self {
        let mode_state = ModeState::Ansi(AnsiEditorMainArea::with_buffer(buffer, None, options.clone(), font_library.clone()));
        let last_save = mode_state.undo_stack_len();

        let mut window = Self {
            id,
            mode_state,
            options,
            font_library,
            is_fullscreen: false,
            dialogs: DialogStack::new(),
            commands: MainWindowCommands::new(),
            last_save,
            close_after_save: false,
            pending_open_path: None,
            title: String::new(),
            ui_revision: Cell::new(0),
            plugins: Arc::new(Plugin::read_plugin_directory()),
            toasts: Vec::new(),
            collaboration_state: CollaborationState::new(),
            remote_paste_preview_cache: HashMap::new(),
        };
        window.update_title();
        window
    }

    fn hash_blocks(blocks: &icy_engine_edit::collaboration::Blocks) -> u64 {
        let mut hasher = DefaultHasher::new();
        blocks.columns.hash(&mut hasher);
        blocks.rows.hash(&mut hasher);
        for b in &blocks.data {
            b.code.hash(&mut hasher);
            b.fg.hash(&mut hasher);
            b.bg.hash(&mut hasher);
        }
        hasher.finish()
    }

    fn sync_remote_paste_previews_to_editor(&mut self) {
        use crate::ui::collaboration::state::CursorMode;
        use crate::ui::editor::ansi::widget::remote_paste_preview::RemotePastePreview;
        use icy_ui::Color;

        let ModeState::Ansi(editor) = &mut self.mode_state else {
            return;
        };

        let mut previews: Vec<RemotePastePreview> = Vec::new();

        for user in self.collaboration_state.remote_users().values() {
            if user.cursor_mode != CursorMode::Operation {
                continue;
            }

            let blocks = match self.collaboration_state.remote_paste_blocks.get(&user.user.id) {
                Some(b) => b,
                None => continue,
            };

            let (r, g, b) = self.collaboration_state.user_color(user.user.id);
            let color = Color::from_rgb8(r, g, b);
            let label = if user.user.group.is_empty() {
                user.user.nick.clone()
            } else {
                format!("{} <{}>", user.user.nick, user.user.group)
            };

            let (col, row) = user.operation.as_ref().map(|o| (o.col, o.row)).or(user.cursor).unwrap_or((0, 0));

            let blocks_hash = Self::hash_blocks(blocks);
            let cached = self.remote_paste_preview_cache.get(&user.user.id);

            let cache_entry = if cached.map(|c| c.blocks_hash) == Some(blocks_hash) {
                cached.cloned()
            } else {
                editor
                    .render_collab_blocks_preview(blocks)
                    .map(|(handle, width_px, height_px)| CachedRemotePastePreview {
                        blocks_hash,
                        handle,
                        width_px,
                        height_px,
                        columns: blocks.columns,
                        rows: blocks.rows,
                    })
            };

            let Some(entry) = cache_entry else {
                continue;
            };

            self.remote_paste_preview_cache.insert(user.user.id, entry.clone());

            previews.push(RemotePastePreview {
                _user_id: user.user.id,
                _nick: user.user.nick.clone(),
                label,
                color,
                col,
                row,
                handle: entry.handle.clone(),
                _width_px: entry.width_px,
                _height_px: entry.height_px,
                columns: entry.columns,
                rows: entry.rows,
            });
        }

        editor.set_remote_paste_previews(previews);
    }

    /// Create a MainWindow restored from a session
    ///
    /// This loads content from `load_path` but sets `original_path` as the file path.
    /// If `mark_dirty` is true, the window will be marked as modified.
    ///
    /// When `load_path` differs from `original_path`, it's an autosave file and we use
    /// `load_from_autosave` to load it (since autosave files have .autosave extension
    /// and can't be identified by extension).
    pub fn new_restored(
        id: usize,
        original_path: Option<PathBuf>,
        load_path: Option<PathBuf>,
        mark_dirty: bool,
        options: Arc<RwLock<Settings>>,
        font_library: SharedFontLibrary,
    ) -> Self {
        let (mode_state, initial_error) = match (&load_path, &original_path) {
            // Case 1: We have an autosave file to load (load_path differs from original_path)
            (Some(autosave), Some(orig)) if autosave != orig => {
                // Determine format from ORIGINAL path, not autosave path
                let format = FileFormat::from_path(orig);

                match format {
                    Some(FileFormat::BitFont(_)) => match BitFontEditor::load_from_autosave(autosave, orig.clone()) {
                        Ok(editor) => (ModeState::BitFont(editor), None),
                        Err(e) => {
                            let error = Some(("Error Loading Font Autosave".to_string(), e));
                            (ModeState::BitFont(BitFontEditor::new()), error)
                        }
                    },
                    Some(FileFormat::IcyAnim) => match AnimationEditor::load_from_autosave(autosave, orig.clone()) {
                        Ok(editor) => (ModeState::Animation(editor), None),
                        Err(e) => {
                            log::error!("Error loading animation autosave: {}", e);
                            let error = Some(("Error Loading Animation Autosave".to_string(), e));
                            (ModeState::Animation(AnimationEditor::new()), error)
                        }
                    },
                    Some(FileFormat::CharacterFont(_)) => {
                        match crate::ui::editor::charfont::CharFontEditor::load_from_autosave(autosave, orig.clone(), options.clone(), font_library.clone()) {
                            Ok(editor) => (ModeState::CharFont(editor), None),
                            Err(e) => {
                                log::error!("Error loading TDF font autosave: {}", e);
                                let error = Some(("Error Loading TDF Font Autosave".to_string(), format!("{}", e)));
                                (
                                    ModeState::CharFont(crate::ui::editor::charfont::CharFontEditor::new(options.clone(), font_library.clone())),
                                    error,
                                )
                            }
                        }
                    }
                    _ => {
                        // ANSI/other formats
                        match AnsiEditorMainArea::load_from_autosave(autosave, orig.clone(), options.clone(), font_library.clone()) {
                            Ok(editor) => (ModeState::Ansi(editor), None),
                            Err(e) => {
                                log::error!("Error loading autosave: {}", e);
                                let error = Some(("Error Loading Autosave".to_string(), format!("{}", e)));
                                (ModeState::Ansi(AnsiEditorMainArea::new(options.clone(), font_library.clone())), error)
                            }
                        }
                    }
                }
            }

            // Case 2: load_path same as original_path, or only load_path given - load normally
            (Some(p), _) => {
                let format = FileFormat::from_path(p);

                match format {
                    Some(FileFormat::BitFont(_)) => match BitFontEditor::from_file(p.clone()) {
                        Ok(mut editor) => {
                            if let Some(ref orig) = original_path {
                                editor.set_file_path(orig.clone());
                            }
                            (ModeState::BitFont(editor), None)
                        }
                        Err(e) => {
                            let error = Some(("Error Loading Font".to_string(), e));
                            (ModeState::BitFont(BitFontEditor::new()), error)
                        }
                    },
                    Some(FileFormat::IcyAnim) => match AnimationEditor::load_file(p.clone()) {
                        Ok(mut editor) => {
                            if let Some(ref orig) = original_path {
                                editor.set_file_path(orig.clone());
                            }
                            (ModeState::Animation(editor), None)
                        }
                        Err(e) => {
                            let error = Some(("Error Loading Animation".to_string(), e));
                            (ModeState::Animation(AnimationEditor::new()), error)
                        }
                    },
                    Some(FileFormat::CharacterFont(_)) => {
                        match crate::ui::editor::charfont::CharFontEditor::with_file(p.clone(), options.clone(), font_library.clone()) {
                            Ok(mut editor) => {
                                if let Some(ref orig) = original_path {
                                    editor.set_file_path(orig.clone());
                                }
                                (ModeState::CharFont(editor), None)
                            }
                            Err(e) => {
                                let error = Some(("Error Loading TDF Font".to_string(), format!("{}", e)));
                                (
                                    ModeState::CharFont(crate::ui::editor::charfont::CharFontEditor::new(options.clone(), font_library.clone())),
                                    error,
                                )
                            }
                        }
                    }
                    _ => match AnsiEditorMainArea::with_file(p.clone(), options.clone(), font_library.clone()) {
                        Ok(mut editor) => {
                            if let Some(ref orig) = original_path {
                                editor.set_file_path(orig.clone());
                            }
                            (ModeState::Ansi(editor), None)
                        }
                        Err(e) => {
                            let error = Some(("Error Loading File".to_string(), format!("Failed to load '{}': {}", p.display(), e)));
                            (ModeState::Ansi(AnsiEditorMainArea::new(options.clone(), font_library.clone())), error)
                        }
                    },
                }
            }

            // Case 3: No load_path but have original_path - load original directly
            (None, Some(orig)) => {
                let format = FileFormat::from_path(orig);

                match format {
                    Some(FileFormat::BitFont(_)) => match BitFontEditor::from_file(orig.clone()) {
                        Ok(editor) => (ModeState::BitFont(editor), None),
                        Err(e) => {
                            let error = Some(("Error Loading Font".to_string(), e));
                            (ModeState::BitFont(BitFontEditor::new()), error)
                        }
                    },
                    Some(FileFormat::IcyAnim) => match AnimationEditor::load_file(orig.clone()) {
                        Ok(editor) => (ModeState::Animation(editor), None),
                        Err(e) => {
                            let error = Some(("Error Loading Animation".to_string(), e));
                            (ModeState::Animation(AnimationEditor::new()), error)
                        }
                    },
                    Some(FileFormat::CharacterFont(_)) => {
                        match crate::ui::editor::charfont::CharFontEditor::with_file(orig.clone(), options.clone(), font_library.clone()) {
                            Ok(editor) => (ModeState::CharFont(editor), None),
                            Err(e) => {
                                let error = Some(("Error Loading TDF Font".to_string(), format!("{}", e)));
                                (
                                    ModeState::CharFont(crate::ui::editor::charfont::CharFontEditor::new(options.clone(), font_library.clone())),
                                    error,
                                )
                            }
                        }
                    }
                    _ => match AnsiEditorMainArea::with_file(orig.clone(), options.clone(), font_library.clone()) {
                        Ok(editor) => (ModeState::Ansi(editor), None),
                        Err(e) => {
                            let error = Some(("Error Loading File".to_string(), format!("Failed to load '{}': {}", orig.display(), e)));
                            (ModeState::Ansi(AnsiEditorMainArea::new(options.clone(), font_library.clone())), error)
                        }
                    },
                }
            }

            // Case 4: No paths - create empty
            (None, None) => (ModeState::Ansi(AnsiEditorMainArea::new(options.clone(), font_library.clone())), None),
        };

        // Determine last_save based on dirty state
        let last_save = if mark_dirty {
            // Mark as dirty by setting last_save to something different
            mode_state.undo_stack_len().wrapping_add(1)
        } else {
            mode_state.undo_stack_len()
        };

        let mut dialogs = DialogStack::new();
        if let Some((title, message)) = initial_error {
            dialogs.push(error_dialog(title, message, |_| Message::CloseDialog));
        }

        let mut window = Self {
            id,
            mode_state,
            options,
            font_library,
            is_fullscreen: false,
            dialogs,
            commands: MainWindowCommands::new(),
            last_save,
            close_after_save: false,
            pending_open_path: None,
            title: String::new(),
            ui_revision: Cell::new(0),
            plugins: Arc::new(Plugin::read_plugin_directory()),
            toasts: Vec::new(),
            collaboration_state: CollaborationState::new(),
            remote_paste_preview_cache: HashMap::new(),
        };
        window.update_title();
        window
    }

    /// Get the current file path
    pub fn file_path(&self) -> Option<&PathBuf> {
        self.mode_state.file_path()
    }

    /// Check if the document is modified (dirty)
    /// Compares current undo stack length with the length at last save.
    /// Always returns false in collaboration mode (server handles persistence).
    pub fn is_modified(&self) -> bool {
        // In collaboration mode, the server handles document persistence
        // so we don't show dirty state to the user
        if self.collaboration_state.is_connected() {
            return false;
        }
        self.mode_state.undo_stack_len() != self.last_save
    }

    /// Mark document as saved - updates last_save to current undo stack length
    pub fn mark_saved(&mut self) {
        self.last_save = self.mode_state.undo_stack_len();
        self.update_title();
    }

    /// Update the cached title based on current file path and dirty state
    fn update_title(&mut self) {
        self.title = self.compute_title();
    }

    /// Compute the title dynamically based on current file path and dirty state
    pub fn compute_title(&self) -> String {
        let file_name = self
            .file_path()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| crate::fl!("unsaved-title"));

        let modified = if self.is_modified() { "*" } else { "" };

        format!("{}{}", file_name, modified)
    }

    /// Get zoom info string for display in title bar (e.g., "[AUTO]" or "[150%]")
    pub fn get_zoom_info_string(&self) -> String {
        if let ModeState::Ansi(editor) = &self.mode_state {
            editor.zoom_info_string()
        } else {
            String::new()
        }
    }

    pub fn theme(&self) -> Theme {
        // Check if any dialog wants to override the theme (e.g., settings preview)
        if let Some(theme) = self.dialogs.theme() {
            return theme;
        }
        self.options.read().monitor_settings.get_theme()
    }

    /// Get current edit mode
    pub fn mode(&self) -> EditMode {
        self.mode_state.mode()
    }

    pub fn plugins(&self) -> &Arc<Vec<Plugin>> {
        &self.plugins
    }

    pub fn ansi_view_menu_state(&self) -> Option<crate::ui::editor::ansi::AnsiViewMenuState> {
        match &self.mode_state {
            ModeState::Ansi(editor) => Some(editor.view_menu_state()),
            _ => None,
        }
    }

    pub fn ansi_mirror_mode(&self) -> Option<bool> {
        match &self.mode_state {
            ModeState::Ansi(editor) => Some(editor.mirror_mode()),
            _ => None,
        }
    }

    /// Get current undo stack length (for autosave tracking)
    pub fn undo_stack_len(&self) -> usize {
        self.mode_state.undo_stack_len()
    }

    /// Get a clone of the undo stack for session serialization
    #[allow(dead_code)]
    pub fn get_undo_stack(&self) -> Option<icy_engine_edit::EditorUndoStack> {
        self.mode_state.get_undo_stack()
    }

    /// Restore undo stack from session
    #[allow(dead_code)]
    pub fn set_undo_stack(&mut self, stack: icy_engine_edit::EditorUndoStack) {
        self.mode_state.set_undo_stack(stack);
    }

    /// Get session data for serialization (includes undo stack + editor state)
    pub fn get_session_data(&self) -> Option<crate::session::EditorSessionData> {
        self.mode_state.get_session_data()
    }

    /// Restore session data
    pub fn set_session_data(&mut self, data: crate::session::EditorSessionData) {
        self.mode_state.set_session_data(data);
    }

    /// Get bytes for autosave
    pub fn get_autosave_bytes(&self) -> Result<Vec<u8>, String> {
        self.mode_state.get_autosave_bytes()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        // Invalidate cached view-derived data on any real update.
        if !matches!(message, Message::Noop) {
            self.ui_revision.set(self.ui_revision.get().wrapping_add(1));
        }

        // Route messages to dialogs first
        if let Some(task) = self.dialogs.update(&message) {
            return task;
        }

        match message {
            Message::Noop => Task::none(),
            Message::NewFile => {
                // Check for unsaved changes first
                if self.is_modified() {
                    let filename = self
                        .file_path()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| fl!("unsaved-title"));

                    self.dialogs.push(confirm_yes_no_cancel(
                        fl!("save-changes-title", filename = filename),
                        fl!("save-changes-description"),
                        |result| match result {
                            DialogResult::Yes => Message::SaveAndNewFile,
                            DialogResult::No => Message::ForceNewFile,
                            _ => Message::CloseDialog,
                        },
                    ));
                    Task::none()
                } else {
                    self.update(Message::ForceNewFile)
                }
            }
            Message::ForceNewFile => {
                // Close the confirmation dialog first (if any)
                self.dialogs.pop();

                // Show New File dialog
                self.dialogs.push(crate::ui::dialog::new_file::NewFileDialog::new());
                Task::none()
            }
            Message::NewFileDialog(_) => {
                // Handled by dialog
                Task::none()
            }
            Message::NewFileCreated(template, width, height) => {
                use crate::ui::dialog::new_file::{create_buffer_for_template, FileTemplate};
                use retrofont::tdf::TdfFontType;

                match template {
                    FileTemplate::Animation => {
                        // Create Animation editor
                        self.mode_state = ModeState::Animation(AnimationEditor::new());
                    }
                    FileTemplate::BitFont => {
                        self.mode_state = ModeState::BitFont(BitFontEditor::new());
                    }
                    FileTemplate::ColorFont => {
                        self.mode_state = ModeState::CharFont(crate::ui::editor::charfont::CharFontEditor::new_with_font_type(
                            TdfFontType::Color,
                            self.options.clone(),
                            self.font_library.clone(),
                        ));
                    }
                    FileTemplate::BlockFont => {
                        self.mode_state = ModeState::CharFont(crate::ui::editor::charfont::CharFontEditor::new_with_font_type(
                            TdfFontType::Block,
                            self.options.clone(),
                            self.font_library.clone(),
                        ));
                    }
                    FileTemplate::OutlineFont => {
                        self.mode_state = ModeState::CharFont(crate::ui::editor::charfont::CharFontEditor::new_with_font_type(
                            TdfFontType::Outline,
                            self.options.clone(),
                            self.font_library.clone(),
                        ));
                    }
                    _ => {
                        // Create ANSI editor with buffer from template
                        let buf = create_buffer_for_template(template, width, height);
                        self.mode_state = ModeState::Ansi(AnsiEditorMainArea::with_buffer(buf, None, self.options.clone(), self.font_library.clone()));
                    }
                }
                self.mark_saved();
                Task::none()
            }
            Message::SaveAndNewFile => {
                // Close the confirmation dialog first
                self.dialogs.pop();

                // Save first, then create new
                if let Some(path) = self.file_path().cloned() {
                    match self.mode_state.save(&path) {
                        Ok(()) => {
                            self.mark_saved();
                            self.update(Message::ForceNewFile)
                        }
                        Err(e) => {
                            self.dialogs.push(error_dialog("Error Saving File", e, |_| Message::CloseDialog));
                            Task::none()
                        }
                    }
                } else {
                    // No path - need SaveAs, store pending action
                    self.pending_open_path = Some(None); // None = new file
                    self.update(Message::SaveFileAs)
                }
            }
            Message::OpenFile => self.open_file(),
            Message::ForceShowOpenDialog => self.show_open_dialog(),
            Message::OpenRecentFile(path) => self.open_recent_file(path),
            Message::SaveAndOpenFile(path) => self.save_and_open_file(path),
            Message::ForceOpenFile(path) => self.force_open_file(path),
            Message::FileOpened(path) => self.file_opened(path),
            Message::FileLoadError(title, error) => {
                self.dialogs.push(error_dialog(title, error, |_| Message::CloseDialog));
                Task::none()
            }
            Message::CloseDialog => {
                // Close the topmost dialog
                self.dialogs.pop();
                Task::none()
            }
            Message::SaveFile => self.save_file(),
            Message::SaveFileAs => self.save_file_as(),
            Message::FileSaved(path) => self.file_saved(path),
            Message::SaveSucceeded(_) => Task::none(),
            Message::CloseFile => self.close_file(),
            Message::CloseEditor | Message::QuitApp => {
                // Handled at WindowManager level
                Task::none()
            }
            Message::SaveAndCloseFile => self.save_and_close_file(),
            Message::ForceCloseFile => {
                // Close the confirmation dialog first (if any)
                self.dialogs.pop();

                // This message is handled by WindowManager to actually close the window
                // It gets passed up and WindowManager handles it
                Task::none()
            }
            Message::Undo => {
                // Dispatch undo to the current editor mode
                match &mut self.mode_state {
                    ModeState::Ansi(editor) => {
                        editor.with_edit_state(|state| {
                            if let Err(e) = state.undo() {
                                log::error!("Undo failed: {}", e);
                            }
                        });
                        // Sync UI after undo (palette may have changed)
                        editor.sync_ui();
                    }
                    ModeState::BitFont(editor) => {
                        editor.undo();
                    }
                    ModeState::CharFont(_) => {}
                    ModeState::Animation(editor) => {
                        let task = editor.update(AnimationEditorMessage::Undo, &mut self.dialogs).map(Message::AnimationEditor);
                        self.update_title();
                        return task;
                    }
                }
                self.update_title();
                Task::none()
            }
            Message::Redo => {
                // Dispatch redo to the current editor mode
                match &mut self.mode_state {
                    ModeState::Ansi(editor) => {
                        editor.with_edit_state(|state| {
                            if let Err(e) = state.redo() {
                                log::error!("Redo failed: {}", e);
                            }
                        });
                        // Sync UI after redo (palette may have changed)
                        editor.sync_ui();
                    }
                    ModeState::BitFont(editor) => {
                        editor.redo();
                    }
                    ModeState::CharFont(_) => {}
                    ModeState::Animation(editor) => {
                        let task = editor.update(AnimationEditorMessage::Redo, &mut self.dialogs).map(Message::AnimationEditor);
                        self.update_title();
                        return task;
                    }
                }
                self.update_title();
                Task::none()
            }
            Message::Cut => {
                match &mut self.mode_state {
                    ModeState::BitFont(editor) => {
                        // BitFont cut uses its own clipboard format
                        let task = editor.state.cut(|res| Message::CopyCompleted(res.map_err(|e| e.to_string())));
                        editor.invalidate_caches();
                        return task;
                    }
                    ModeState::Ansi(editor) => {
                        return editor.cut(|res| Message::CopyCompleted(res.map_err(|e| e.to_string())));
                    }
                    ModeState::CharFont(_) => {
                        // TODO: Implement cut for CharFont
                    }
                    ModeState::Animation(_) => {
                        // TODO: Implement cut for Animation
                    }
                }
                Task::none()
            }
            Message::Copy => {
                match &mut self.mode_state {
                    ModeState::BitFont(editor) => {
                        // BitFont uses its own clipboard format
                        let task = editor.state.copy(|res| Message::CopyCompleted(res.map_err(|e| e.to_string())));
                        editor.invalidate_caches();
                        return task;
                    }
                    ModeState::Ansi(editor) => {
                        return editor.copy(|res| Message::CopyCompleted(res.map_err(|e| e.to_string())));
                    }
                    ModeState::CharFont(_) => {
                        // TODO: Implement copy for CharFont
                    }
                    ModeState::Animation(_) => {
                        // TODO: Implement copy for Animation
                    }
                }
                Task::none()
            }
            Message::CopyCompleted(result) => {
                if let Err(e) = result {
                    log::error!("Copy failed: {}", e);
                }
                Task::none()
            }
            Message::Paste => {
                use icy_engine_gui::ICY_CLIPBOARD_TYPE;
                use icy_ui::clipboard::STANDARD;

                match &self.mode_state {
                    ModeState::BitFont(_) => {
                        // BitFont paste is handled separately via its own clipboard format
                        // Read BitFont format first
                        return icy_engine_edit::bitfont::get_from_clipboard(|result| match result {
                            Ok(data) => Message::ClipboardDataForPaste(Some(ClipboardReadResult {
                                icy_data: Some(data.to_bytes()),
                                image: None,
                                text: None,
                            })),
                            Err(_) => Message::ClipboardDataForPaste(None),
                        });
                    }
                    _ => {
                        // For ANSI and other editors, read ICY format first
                        return STANDARD.read_format(&[ICY_CLIPBOARD_TYPE]).map(|icy_result| {
                            Message::ClipboardDataForPaste(Some(ClipboardReadResult {
                                icy_data: icy_result.map(|d| d.data),
                                image: None,
                                text: None,
                            }))
                        });
                    }
                }
            }
            Message::ClipboardDataForPaste(data) => {
                use icy_ui::clipboard::STANDARD;

                if let Some(clipboard_data) = data {
                    match &mut self.mode_state {
                        ModeState::BitFont(editor) => {
                            // BitFont paste from our custom format
                            if let Some(data) = clipboard_data.icy_data {
                                if let Ok(parsed) = icy_engine_edit::bitfont::BitFontClipboardData::from_bytes(&data) {
                                    if let Err(e) = editor.state.paste_data(parsed) {
                                        log::error!("BitFont paste failed: {}", e);
                                    }
                                }
                            }
                            editor.invalidate_caches();
                        }
                        ModeState::Ansi(editor) => {
                            // Try ICY format first
                            if let Some(icy_data) = clipboard_data.icy_data {
                                if let Err(e) = editor.paste_icy_data(&icy_data) {
                                    log::error!("Paste ICY data failed: {}", e);
                                }
                            } else if let Some(img) = clipboard_data.image {
                                if let Err(e) = editor.paste_image(img) {
                                    log::error!("Paste image failed: {}", e);
                                }
                            } else if let Some(text) = clipboard_data.text {
                                if let Err(e) = editor.paste_text(&text) {
                                    log::error!("Paste text failed: {}", e);
                                }
                            } else {
                                // No ICY data, try reading image next
                                return STANDARD.read_image().map(|clipboard_data| {
                                    let img = clipboard_data.and_then(|cd| image::load_from_memory(&cd.data).ok().map(|i| i.to_rgba8()));
                                    Message::ClipboardDataForPaste(Some(ClipboardReadResult {
                                        icy_data: None,
                                        image: img,
                                        text: None,
                                    }))
                                });
                            }
                        }
                        ModeState::CharFont(_) => {
                            // TODO: Implement paste for CharFont
                        }
                        ModeState::Animation(_) => {
                            // TODO: Implement paste for Animation
                        }
                    }
                }
                Task::none()
            }
            Message::PasteAsNewImage => {
                use icy_ui::clipboard::STANDARD;

                // Read image format for paste as new image
                STANDARD.read_image().map(|clipboard_data| {
                    let img = clipboard_data.and_then(|cd| image::load_from_memory(&cd.data).ok().map(|i| i.to_rgba8()));
                    Message::ClipboardDataForNewImage(Some(ClipboardReadResult {
                        icy_data: None,
                        image: img,
                        text: None,
                    }))
                })
            }
            Message::ClipboardDataForNewImage(data) => {
                use icy_engine::{Layer, Position, Role, Sixel, Size, TextBuffer, TextPane};

                if let Some(clipboard_data) = data {
                    // Try image first since that's what PasteAsNewImage is mainly for
                    if let Some(img) = clipboard_data.image {
                        let w = img.width();
                        let h = img.height();

                        let mut sixel = Sixel::new(Position::default());
                        sixel.picture_data = img.into_raw();
                        sixel.set_width(w as i32);
                        sixel.set_height(h as i32);

                        // Calculate buffer size based on default font dimensions (8x16)
                        let font_width = 8;
                        let font_height = 16;
                        let buf_width = ((w as i32 + font_width - 1) / font_width).max(1);
                        let buf_height = ((h as i32 + font_height - 1) / font_height).max(1);

                        let mut buf = TextBuffer::new(Size::new(buf_width, buf_height));

                        // Create a dedicated image layer for the sixel
                        let mut layer = Layer::new("Image".to_string(), (buf_width, buf_height));
                        layer.role = Role::Image;
                        layer.properties.has_alpha_channel = true;
                        layer.sixels.push(sixel);
                        buf.layers.push(layer);

                        // Store buffer in global static for WindowManager to pick up
                        if let Ok(mut pending) = crate::PENDING_NEW_WINDOW_BUFFERS.lock() {
                            pending.push(buf);
                        }
                        return Task::done(Message::OpenNewWindowWithBuffer);
                    }
                }
                Task::none()
            }
            Message::OpenNewWindowWithBuffer => {
                // This message is handled by WindowManager, not here
                Task::none()
            }
            Message::InsertSixelFromFile => {
                // Open file dialog to select an image file
                Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "bmp", "webp", "tiff", "ico"])
                            .add_filter("All files", &["*"])
                            .pick_file()
                            .await
                            .map(|h| h.path().to_path_buf())
                    },
                    |result| {
                        if let Some(path) = result {
                            Message::InsertSixelFromPath(path)
                        } else {
                            Message::Noop
                        }
                    },
                )
            }
            Message::InsertSixelFromPath(path) => {
                use icy_engine::{Position, Sixel};

                // Load the image file
                if let Ok(img) = image::open(&path) {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();

                    let mut sixel = Sixel::new(Position::default());
                    sixel.picture_data = rgba.into_raw();
                    sixel.set_width(w as i32);
                    sixel.set_height(h as i32);

                    if let ModeState::Ansi(editor) = &mut self.mode_state {
                        editor.with_edit_state(|state| {
                            if let Err(e) = state.paste_sixel(sixel) {
                                log::error!("Failed to insert sixel from file: {}", e);
                            }
                        });
                    }
                } else {
                    log::error!("Failed to load image from {:?}", path);
                }
                Task::none()
            }
            Message::SelectAll => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    editor.with_edit_state(|state| {
                        let w = state.get_buffer().width();
                        let h = state.get_buffer().height();
                        let _ = state.set_selection(icy_engine::Rectangle::from(0, 0, w, h));
                    });
                }
                Task::none()
            }
            Message::ZoomIn => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    editor.zoom_in();
                }
                Task::none()
            }
            Message::ZoomOut => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    editor.zoom_out();
                }
                Task::none()
            }
            Message::ZoomReset => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    editor.zoom_reset();
                }
                Task::none()
            }
            Message::SwitchMode(mode) => {
                self.mode_state = match mode {
                    EditMode::Ansi => ModeState::Ansi(AnsiEditorMainArea::new(self.options.clone(), self.font_library.clone())),
                    EditMode::BitFont => ModeState::BitFont(BitFontEditor::new()),
                    EditMode::CharFont => ModeState::CharFont(crate::ui::editor::charfont::CharFontEditor::new(
                        self.options.clone(),
                        self.font_library.clone(),
                    )),
                    EditMode::Animation => ModeState::Animation(AnimationEditor::new()),
                };
                Task::none()
            }
            Message::BitFontEditor(msg) => {
                // FontImported is handled specially to switch to BitFont editor
                if let BitFontEditorMessage::FontImported(font) = msg {
                    let mut editor = BitFontEditor::new();
                    editor.state = icy_engine_edit::bitfont::BitFontEditState::from_font(font);
                    editor.invalidate_caches();
                    self.mode_state = ModeState::BitFont(editor);
                    self.mark_saved();
                    return Task::none();
                }

                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    editor.update(msg, &mut self.dialogs).map(Message::BitFontEditor)
                } else {
                    Task::none()
                }
            }
            Message::CharFontEditor(msg) => {
                if let ModeState::CharFont(editor) = &mut self.mode_state {
                    editor.update(msg, &mut self.dialogs).map(Message::CharFontEditor)
                } else {
                    Task::none()
                }
            }
            Message::AnsiEditor(msg) => {
                // Intercept ChatPanel messages and handle them at MainWindow level
                if let AnsiEditorMessage::ChatPanel(chat_msg) = msg {
                    return self.update(Message::ChatPanel(chat_msg));
                }

                // Handle AnsiEditorMessage::Core for CharFont editor by forwarding to CharFontEditor
                if let ModeState::CharFont(editor) = &mut self.mode_state {
                    if let AnsiEditorMessage::Core(core_msg) = msg {
                        // Forward all Core messages to CharFontEditor through its AnsiEditor variant
                        return editor
                            .update(crate::ui::editor::charfont::CharFontEditorMessage::AnsiEditor(core_msg), &mut self.dialogs)
                            .map(Message::CharFontEditor);
                    }
                    // Non-Core AnsiEditorMessage variants are not applicable to CharFont mode
                    return Task::none();
                }
                // Forward all messages to the Ansi editor
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    let editor_task = editor.update(msg, &mut self.dialogs, &self.plugins).map(Message::AnsiEditor);

                    // Process pending collaboration events from tool results
                    let mut collab_tasks: Vec<icy_ui::Task<Message>> = Vec::new();
                    let pending_events = editor.take_pending_collab_events();
                    let is_connected = self.collaboration_state.is_connected();
                    if !pending_events.is_empty() {
                        log::debug!("[Collab] Retrieved {} pending events, is_connected: {}", pending_events.len(), is_connected);
                    }
                    if is_connected {
                        use crate::ui::editor::ansi::CollabToolEvent;

                        for event in pending_events {
                            match event {
                                CollabToolEvent::PasteAsSelection => {
                                    if let Some(blocks) = editor.get_floating_layer_blocks() {
                                        if let Some(task) = self.collaboration_state.send_paste_as_selection(blocks) {
                                            collab_tasks.push(task.map(|_| Message::Noop));
                                        }
                                    }
                                    // Also send initial operation position
                                    if let Some((x, y)) = editor.get_floating_layer_position() {
                                        if let Some(task) = self.collaboration_state.send_operation(x, y) {
                                            collab_tasks.push(task.map(|_| Message::Noop));
                                        }
                                    }
                                }
                                CollabToolEvent::Operation(x, y) => {
                                    log::debug!("[Collab] Sending Operation({}, {}) to server", x, y);
                                    if let Some(task) = self.collaboration_state.send_operation(x, y) {
                                        collab_tasks.push(task.map(|_| Message::Noop));
                                    }
                                }
                            }
                        }
                    }

                    // Sync collaboration state from undo stack after editor updates
                    if self.collaboration_state.is_connected() {
                        if let Some((undo_stack_arc, caret_pos, selecting)) = editor.get_collab_sync_info() {
                            // Use try_lock to avoid potential deadlocks
                            if let Ok(undo_stack) = undo_stack_arc.try_lock() {
                                if let Some(collab_task) = self.collaboration_state.sync_from_undo_stack(&undo_stack, caret_pos, selecting) {
                                    collab_tasks.push(collab_task.map(|_| Message::Noop));
                                }
                            }
                        }
                    }

                    if collab_tasks.is_empty() {
                        editor_task
                    } else {
                        collab_tasks.insert(0, editor_task);
                        Task::batch(collab_tasks)
                    }
                } else {
                    Task::none()
                }
            }
            Message::AnimationEditor(msg) => {
                if let ModeState::Animation(editor) = &mut self.mode_state {
                    editor.update(msg, &mut self.dialogs).map(Message::AnimationEditor)
                } else {
                    Task::none()
                }
            }

            /*
            Message::Tick => {
                self.update_title();
                Task::none()
            }
            Message::AnimationTick => {
                // Update dialog animations first
                self.dialogs.update_animation();

                match &mut self.mode_state {
                    ModeState::Ansi(editor) => {
                        let delta = 0.016;
                        let tool_task = editor
                            .update(AnsiEditorMessage::ToolPanel(crate::ui::editor::ansi::ToolPanelMessage::Tick(delta)))
                            .map(Message::AnsiEditor);

                        let minimap_task = editor.update(AnsiEditorMessage::MinimapAutoscrollTick(delta)).map(Message::AnsiEditor);

                        Task::batch([tool_task, minimap_task])
                    }
                    ModeState::BitFont(_editor) => {
                        // ColorSwitcher tickt sich selbst (RedrawRequested) – kein globaler Tick.
                        Task::none()
                    }
                    ModeState::CharFont(_editor) => {
                        // ColorSwitcher tickt sich selbst (RedrawRequested) – kein globaler Tick.
                        Task::none()
                    }
                    ModeState::Animation(editor) => editor.update(AnimationEditorMessage::Tick).map(Message::AnimationEditor),
                }
            }*/
            // Toast notifications
            Message::ShowToast(toast) => {
                self.toasts.push(toast);
                Task::none()
            }
            Message::CloseToast(index) => {
                if index < self.toasts.len() {
                    self.toasts.remove(index);
                }
                Task::none()
            }

            // ═══════════════════════════════════════════════════════════════════
            // File Settings Dialog
            // ═══════════════════════════════════════════════════════════════════
            Message::ShowFileSettingsDialog => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    let dialog = editor.with_edit_state(|state| crate::ui::editor::ansi::FileSettingsDialog::from_edit_state(state));
                    self.dialogs.push(dialog);
                }
                Task::none()
            }
            // FileSettingsDialog messages are routed through DialogStack::update above
            Message::FileSettingsDialog(_) => Task::none(),
            Message::ApplyFileSettings(result) => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    editor.with_edit_state(|state| {
                        // Apply canvas size with undo support
                        let current_size = state.get_buffer().size();
                        if result.width != current_size.width || result.height != current_size.height {
                            let _ = state.resize_buffer(false, icy_engine::Size::new(result.width, result.height));
                        }

                        // Apply font cell size with undo support
                        let _ = state.set_font_dimensions(icy_engine::Size::new(result.font_width, result.font_height));

                        // Apply SAUCE metadata with undo support
                        let mut sauce_meta = icy_engine_edit::SauceMetaData::default();
                        sauce_meta.title = result.title.as_str().into();
                        sauce_meta.author = result.author.as_str().into();
                        sauce_meta.group = result.group.as_str().into();
                        for line in result.comments.lines() {
                            sauce_meta.comments.push(line.into());
                        }
                        let _ = state.update_sauce_data(sauce_meta);

                        // Apply format mode (sets palette_mode and font_mode)
                        state.set_format_mode(result.format_mode);

                        // Apply ice mode with undo support
                        let ice_mode = if result.ice_colors {
                            icy_engine::IceMode::Ice
                        } else {
                            icy_engine::IceMode::Blink
                        };
                        let _ = state.set_ice_mode(ice_mode);

                        // Apply display options with undo support
                        let _ = state.set_use_letter_spacing(result.use_9px_font);
                        let _ = state.set_use_aspect_ratio(result.legacy_aspect);
                    });
                }
                Task::none()
            }

            // ═══════════════════════════════════════════════════════════════════
            // File operations (TODO: implement)
            // ═══════════════════════════════════════════════════════════════════
            Message::ClearRecentFiles => {
                self.options.write().recent_files.clear_recent_files();
                Task::none()
            }
            Message::ExportFile => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    return editor
                        .update(AnsiEditorMessage::ExportFile, &mut self.dialogs, &self.plugins)
                        .map(Message::AnsiEditor);
                }
                Task::none()
            }
            Message::ExportDialog(_) => {
                // ExportDialog messages are routed through DialogStack::update above
                if let Some(task) = self.dialogs.update(&message) {
                    return task;
                }
                Task::none()
            }
            Message::ExportComplete(path) => {
                log::info!("Export complete: {:?}", path);
                // TODO: Could show a toast notification here
                Task::none()
            }
            Message::ShowSettings => {
                let preview_font = match &mut self.mode_state {
                    ModeState::Ansi(editor) => editor.with_edit_state(|state| state.get_buffer().font(0).cloned()),
                    _ => None,
                };

                self.dialogs
                    .push(crate::ui::dialog::settings::SettingsDialog::new(self.options.clone(), preview_font));
                Task::none()
            }
            Message::ShowImportFontDialog => {
                self.dialogs.push(crate::ui::dialog::font_import::FontImportDialog::new());
                Task::none()
            }
            // SettingsDialog messages are routed through DialogStack::update above
            Message::SettingsDialog(_) => Task::none(),
            Message::SettingsSaved(_) => {
                // Apply outline style to current ANSI editor (if any)
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    let outline_style = { self.options.read().font_outline_style };
                    editor.with_edit_state(|state| state.set_outline_style(outline_style));
                }
                Task::none()
            }

            // ═══════════════════════════════════════════════════════════════════
            // Selection operations (delegated to AnsiEditor)
            // ═══════════════════════════════════════════════════════════════════
            Message::Deselect => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    return editor
                        .update(AnsiEditorMessage::Core(AnsiEditorCoreMessage::Deselect), &mut self.dialogs, &self.plugins)
                        .map(Message::AnsiEditor);
                }
                Task::none()
            }

            // ═══════════════════════════════════════════════════════════════════
            // Help
            // ═══════════════════════════════════════════════════════════════════
            Message::OpenLogFile => {
                if let Some(log_file) = Settings::log_file() {
                    if log_file.exists() {
                        #[cfg(windows)]
                        {
                            let _ = std::process::Command::new("notepad").arg(&log_file).spawn();
                        }
                        #[cfg(not(windows))]
                        {
                            if let Err(err) = open::that(&log_file) {
                                log::error!("Failed to open log file: {}", err);
                            }
                        }
                    } else if let Some(parent) = log_file.parent() {
                        if let Err(err) = open::that(parent) {
                            log::error!("Failed to open log directory: {}", err);
                        }
                    }
                }
                Task::none()
            }

            // ═══════════════════════════════════════════════════════════════════
            // View operations
            // ═══════════════════════════════════════════════════════════════════
            Message::SetZoom(zoom) => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    editor.set_zoom(zoom);
                }
                Task::none()
            }
            Message::ToggleFullscreen => {
                self.is_fullscreen = !self.is_fullscreen;
                let mode = if self.is_fullscreen {
                    icy_ui::window::Mode::Fullscreen
                } else {
                    icy_ui::window::Mode::Windowed
                };
                icy_ui::window::latest().and_then(move |window| icy_ui::window::set_mode(window, mode))
            }

            // ═══════════════════════════════════════════════════════════════════
            // Help operations
            // ═══════════════════════════════════════════════════════════════════
            Message::OpenDiscussions => {
                if let Err(e) = open::that("https://github.com/mkrueger/icy_tools/discussions") {
                    log::error!("Failed to open discussions URL: {}", e);
                }
                Task::none()
            }
            Message::OpenReleasesPage => {
                let url = format!(
                    "https://github.com/mkrueger/icy_tools/releases/tag/IcyDraw{}",
                    crate::LATEST_VERSION.to_string()
                );
                if let Err(e) = open::that(&url) {
                    log::error!("Failed to open releases URL: {}", e);
                }
                Task::none()
            }
            Message::ReportBug => {
                if let Err(e) = open::that("https://github.com/mkrueger/icy_tools/issues") {
                    log::error!("Failed to open issues URL: {}", e);
                }
                Task::none()
            }
            Message::ShowAbout => {
                self.dialogs.push(crate::ui::dialog::about::about_dialog(Message::AboutDialog, |msg| match msg {
                    Message::AboutDialog(m) => Some(m),
                    _ => None,
                }));
                Task::none()
            }
            Message::AboutDialog(ref msg) => {
                // Handle OpenLink messages from the about dialog
                if let icy_engine_gui::ui::AboutDialogMessage::OpenLink(url) = msg {
                    if let Err(e) = open::that(url) {
                        log::error!("Failed to open URL {}: {}", url, e);
                    }
                }
                // Route to dialog stack for other messages
                if let Some(task) = self.dialogs.update(&message) {
                    return task;
                }
                Task::none()
            }

            // ═══════════════════════════════════════════════════════════════════════════
            // Collaboration
            // ═══════════════════════════════════════════════════════════════════════════
            Message::ShowConnectDialog => {
                // Show connect to server dialog with settings pre-filled
                use crate::ui::dialog::ConnectDialog;
                let opts = self.options.read();
                self.dialogs.push(ConnectDialog::with_settings(&opts));
                Task::none()
            }
            Message::ConnectDialog(_) => {
                // Route to dialog stack
                if let Some(task) = self.dialogs.update(&message) {
                    return task;
                }
                Task::none()
            }
            Message::ConnectToServer(ref result) => {
                // Start collaboration connection
                log::info!("Connecting to server: {} as {}", result.url, result.nick);

                // Save server, nick and group to settings for next time
                {
                    let mut opts = self.options.write();
                    opts.add_collaboration_server(&result.url);
                    opts.collaboration.nick = result.nick.clone();
                    opts.collaboration.group = result.group.clone();
                    opts.store_persistent();
                }

                // Store connection info and start connecting
                // The subscription will pick this up and establish the connection
                self.collaboration_state
                    .start_connecting(result.url.clone(), result.nick.clone(), result.group.clone(), result.password.clone());

                let toast = Toast::info(format!("Connecting to {}...", result.url));
                Task::done(Message::ShowToast(toast))
            }
            Message::ToggleChatPanel => {
                // Toggle chat panel visibility
                self.collaboration_state.toggle_chat();
                Task::none()
            }
            Message::ChatPanel(ref msg) => {
                use crate::ui::collaboration::ChatPanelMessage;
                match msg {
                    ChatPanelMessage::InputChanged(text) => {
                        self.collaboration_state.chat_input = text.clone();
                        // User is typing in the chat input, so it's focused
                        self.collaboration_state.chat_input_focused = true;
                    }
                    ChatPanelMessage::SendMessage => {
                        if !self.collaboration_state.chat_input.trim().is_empty() {
                            let text = std::mem::take(&mut self.collaboration_state.chat_input);
                            if let Some(task) = self.collaboration_state.send_chat(text) {
                                return task.discard();
                            }
                        }
                        // After sending, the input loses focus
                        self.collaboration_state.chat_input_focused = false;
                    }
                    ChatPanelMessage::GotoUser(user_id) => {
                        // Move camera to user's cursor position
                        if let Some(user) = self.collaboration_state.get_user(*user_id) {
                            if let Some((col, row)) = user.cursor {
                                log::info!("Goto user {} at ({}, {})", user.user.nick, col, row);
                                if let ModeState::Ansi(editor) = &mut self.mode_state {
                                    // Clicking elsewhere removes focus from chat input
                                    self.collaboration_state.chat_input_focused = false;
                                    return editor.scroll_to_position(col, row).map(Message::AnsiEditor);
                                }
                            }
                        }
                        // Clicking elsewhere removes focus from chat input
                        self.collaboration_state.chat_input_focused = false;
                    }
                }
                Task::none()
            }
            Message::Collaboration(ref collab_msg) => {
                use crate::ui::collaboration::CollaborationMessage;
                use icy_engine_edit::collaboration::CollaborationEvent;

                match collab_msg {
                    CollaborationMessage::Ready(client) => {
                        log::info!("Collaboration connected!");
                        self.collaboration_state.on_connected(client.clone());
                        let toast = Toast::success("Connected to collaboration server".to_string());
                        return Task::done(Message::ShowToast(toast));
                    }
                    CollaborationMessage::Event(event) => {
                        match event {
                            CollaborationEvent::Connected(doc) => {
                                self.collaboration_state.start_session(&doc);

                                // Create a new editor with the document from the server
                                let buffer = AnsiEditorMainArea::create_buffer_from_remote_document(&doc);
                                self.mode_state =
                                    ModeState::Ansi(AnsiEditorMainArea::with_buffer(buffer, None, self.options.clone(), self.font_library.clone()));

                                // Apply SAUCE metadata to the new editor
                                if let ModeState::Ansi(editor) = &mut self.mode_state {
                                    editor.with_edit_state(|state| {
                                        let mut sauce = icy_engine_edit::SauceMetaData::default();
                                        sauce.title = doc.title.clone().into();
                                        sauce.author = doc.author.clone().into();
                                        sauce.group = doc.group.clone().into();
                                        sauce.comments = doc.comments.lines().map(|line| line.to_string().into()).collect();
                                        state.set_sauce_meta(sauce);
                                    });
                                    editor.sync_ui();
                                }

                                // Seed initial user list (Moebius sends existing users only)
                                // show_join=false: these users were already connected, no "joined" message
                                for user in doc.users.iter() {
                                    self.collaboration_state.add_user(user.clone(), false);
                                }
                                self.sync_remote_cursors_to_editor();
                            }
                            CollaborationEvent::UserJoined(user) => {
                                log::info!("User joined: {}", user.nick);
                                // show_join=true: this user just joined, show "joined" message
                                self.collaboration_state.add_user(user.clone(), true);
                                self.sync_remote_cursors_to_editor();
                            }
                            CollaborationEvent::UserLeft { user_id, nick } => {
                                log::info!("User {} left", nick);
                                // show_leave=true: this user just left, show "left" message
                                self.collaboration_state.remove_user(*user_id, true);
                                self.sync_remote_cursors_to_editor();
                            }
                            CollaborationEvent::CursorMoved { user_id, col, row } => {
                                self.collaboration_state.update_cursor(*user_id, *col, *row);
                                self.sync_remote_cursors_to_editor();
                            }
                            CollaborationEvent::SelectionChanged { user_id, selecting, col, row } => {
                                self.collaboration_state.update_selection(*user_id, *selecting, *col, *row);
                                self.sync_remote_cursors_to_editor();
                            }
                            CollaborationEvent::OperationStarted { user_id, col, row } => {
                                self.collaboration_state.update_operation(*user_id, *col, *row);
                                self.sync_remote_cursors_to_editor();
                            }
                            CollaborationEvent::CursorHidden { user_id } => {
                                self.collaboration_state.hide_user_cursor(*user_id);
                                self.sync_remote_cursors_to_editor();
                            }
                            CollaborationEvent::StatusChanged(status) => {
                                self.collaboration_state.update_user_status(status.id, status.status);
                            }
                            CollaborationEvent::SauceChanged(sauce) => {
                                // Apply remote SAUCE metadata update
                                if let ModeState::Ansi(editor) = &mut self.mode_state {
                                    editor.apply_remote_sauce(sauce.title.clone(), sauce.author.clone(), sauce.group.clone(), sauce.comments.clone());
                                }
                                // Show chat notification
                                if let Some(user) = self.collaboration_state.remote_users().get(&sauce.id) {
                                    let nick = user.user.nick.clone();
                                    self.collaboration_state.add_system_message(&format!("{} changed the SAUCE record", nick));
                                }
                            }
                            CollaborationEvent::Draw { col, row, block } => {
                                // Apply remote draw to our buffer
                                if let ModeState::Ansi(editor) = &mut self.mode_state {
                                    editor.apply_remote_draw(*col, *row, block.code, block.fg, block.bg);
                                }
                            }
                            CollaborationEvent::CanvasResized { user_id, columns, rows } => {
                                self.collaboration_state.update_canvas_size(*columns, *rows);

                                if let ModeState::Ansi(editor) = &mut self.mode_state {
                                    editor.apply_remote_canvas_resize(*columns, *rows);
                                    editor.sync_ui();
                                }
                                // Show system message
                                if let Some(user) = self.collaboration_state.get_user(*user_id) {
                                    self.collaboration_state
                                        .add_system_message(&format!("{} changed the canvas size to {} × {}", user.user.nick, columns, rows));
                                }
                            }
                            CollaborationEvent::IceColorsChanged { user_id, value } => {
                                // Apply remote ICE colors change
                                if let ModeState::Ansi(editor) = &mut self.mode_state {
                                    editor.apply_remote_ice_colors(*value);
                                }
                                // Show system message
                                if let Some(user) = self.collaboration_state.get_user(*user_id) {
                                    let state = if *value { "on" } else { "off" };
                                    self.collaboration_state
                                        .add_system_message(&format!("{} turned iCE colors {}", user.user.nick, state));
                                }
                            }
                            CollaborationEvent::Use9pxChanged { user_id, value } => {
                                // Apply remote 9px font (letter spacing) change
                                if let ModeState::Ansi(editor) = &mut self.mode_state {
                                    editor.apply_remote_9px_font(*value);
                                }
                                // Show system message
                                if let Some(user) = self.collaboration_state.get_user(*user_id) {
                                    let state = if *value { "on" } else { "off" };
                                    self.collaboration_state
                                        .add_system_message(&format!("{} turned letter spacing {}", user.user.nick, state));
                                }
                            }
                            CollaborationEvent::FontChanged { user_id, font_name } => {
                                // Apply remote font change
                                if let ModeState::Ansi(editor) = &mut self.mode_state {
                                    editor.apply_remote_font_change(font_name);
                                }
                                // Show system message
                                if let Some(user) = self.collaboration_state.get_user(*user_id) {
                                    self.collaboration_state
                                        .add_system_message(&format!("{} changed the font to {}", user.user.nick, font_name));
                                }
                            }
                            CollaborationEvent::PasteAsSelection { user_id, blocks } => {
                                // Moebius sends blocks for a floating selection preview.
                                self.collaboration_state.update_paste_as_selection(*user_id, blocks.clone());

                                // Keep the last known position (cursor/operation will be used by overlays).
                                let (col, row) = self.collaboration_state.get_user(*user_id).and_then(|u| u.cursor).unwrap_or((0, 0));
                                self.collaboration_state.update_operation(*user_id, col, row);
                                self.sync_remote_cursors_to_editor();
                            }
                            CollaborationEvent::Rotate { user_id } | CollaborationEvent::FlipX { user_id } | CollaborationEvent::FlipY { user_id } => {
                                // These operations apply to the sender's floating selection in Moebius.
                                // We treat them as "operation mode" activity for cursor visualization.
                                let (col, row) = self.collaboration_state.get_user(*user_id).and_then(|u| u.cursor).unwrap_or((0, 0));

                                self.collaboration_state.update_operation(*user_id, col, row);
                                self.sync_remote_cursors_to_editor();
                            }
                            CollaborationEvent::BackgroundChanged { user_id, value: _ } => {
                                // Moebius uses this as a canvas background setting.
                                // icy_draw currently has no equivalent canvas background field, so we just notify.
                                if let Some(user) = self.collaboration_state.get_user(*user_id) {
                                    self.collaboration_state
                                        .add_system_message(&format!("{} changed the background", user.user.nick));
                                }
                            }
                            CollaborationEvent::Chat(msg) => {
                                self.collaboration_state.add_chat_message(msg.clone());
                            }
                            CollaborationEvent::Disconnected => {
                                log::info!("Disconnected from collaboration server");
                                self.collaboration_state.end_session();
                                self.sync_remote_cursors_to_editor();
                                self.dialogs
                                    .push(error_dialog(fl!("collab-connection-lost-title"), fl!("collab-connection-lost-message"), |_| {
                                        Message::CloseDialog
                                    }));
                            }
                            CollaborationEvent::Error(e) => {
                                log::error!("Collaboration error: {}", e);
                                self.collaboration_state.end_session();
                                self.sync_remote_cursors_to_editor();
                                self.dialogs.push(error_dialog(
                                    fl!("collab-connection-error-title"),
                                    fl!("collab-connection-error-message", error = e.to_string()),
                                    |_| Message::CloseDialog,
                                ));
                            }
                            _ => {
                                // Handle other events as needed
                                log::debug!("Unhandled collaboration event: {:?}", event);
                            }
                        }
                    }
                }
                Task::none()
            }
        }
    }

    /// Get the collaboration subscription if connecting or connected
    pub fn subscription(&self) -> Subscription<Message> {
        use icy_ui::Subscription;

        if self.collaboration_state.connecting || self.collaboration_state.active {
            if let (Some(url), Some(nick)) = (&self.collaboration_state.server_url, &self.collaboration_state.nick) {
                let password = self.collaboration_state.password.clone().unwrap_or_default();
                let group = self.collaboration_state.group.clone().unwrap_or_default();
                let config = icy_engine_edit::collaboration::ClientConfig {
                    url: url.clone(),
                    nick: nick.clone(),
                    group,
                    password,
                    ping_interval_secs: 30,
                };
                return crate::ui::collaboration::connect(config).map(Message::Collaboration);
            }
        }

        Subscription::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        // Cache current theme for this view pass
        let theme = self.theme();

        // Pass collaboration state to the ANSI editor; it builds the chat pane and splitter itself.
        let content: Element<'_, Message> = match &self.mode_state {
            ModeState::Ansi(editor) => {
                let collab = self.collaboration_state.active.then_some(&self.collaboration_state);
                editor.view(collab, &theme).map(Message::AnsiEditor)
            }
            ModeState::BitFont(editor) => editor.view(None).map(Message::BitFontEditor),
            ModeState::CharFont(editor) => editor.view(None).map(Message::CharFontEditor),
            ModeState::Animation(editor) => editor.view(None).map(Message::AnimationEditor),
        };

        // Status bar
        let status_bar = self.view_status_bar();

        // Note: The menu bar is now handled by the application_menu in WindowManager.
        // On non-macOS, icy_ui automatically prepends the menu bar widget to the view.
        // On macOS, the native menu bar is used instead.
        let main_content: Element<'_, Message> = column![content, rule::horizontal(1), status_bar,].into();

        // Show dialogs from dialog stack
        let with_dialogs = self.dialogs.view(main_content);

        // Wrap with toast manager
        ToastManager::new(with_dialogs, &self.toasts, Message::CloseToast).into()
    }

    /// Sync remote cursor positions from collaboration state to the ANSI editor
    fn sync_remote_cursors_to_editor(&mut self) {
        use crate::ui::collaboration::state::CursorMode;
        use crate::ui::editor::ansi::widget::remote_cursors::{RemoteCursor, RemoteCursorMode};

        let cursors: Vec<RemoteCursor> = self
            .collaboration_state
            .remote_users()
            .values()
            .filter_map(|user| {
                // If the user is in operation mode *and* we have a paste preview for them,
                // the dedicated paste preview overlay already renders frame + label.
                // Hide the small operation cursor box to avoid a duplicate label.
                if user.cursor_mode == CursorMode::Operation && self.collaboration_state.remote_paste_blocks.contains_key(&user.user.id) {
                    return None;
                }

                // Skip hidden cursors
                if user.cursor_mode == CursorMode::Hidden {
                    return None;
                }

                // Convert mode
                let mode = match user.cursor_mode {
                    CursorMode::Hidden => RemoteCursorMode::Hidden,
                    CursorMode::Editing => RemoteCursorMode::Editing,
                    CursorMode::Selection => {
                        // Use cursor position as start, selection position as current
                        if let Some((start_col, start_row)) = user.cursor {
                            RemoteCursorMode::Selection { start_col, start_row }
                        } else {
                            RemoteCursorMode::Editing
                        }
                    }
                    CursorMode::Operation => RemoteCursorMode::Operation,
                };

                // Determine which position to use
                let (col, row) = match user.cursor_mode {
                    CursorMode::Selection => user.selection.as_ref().map(|s| (s.col, s.row)).or(user.cursor)?,
                    CursorMode::Operation => user.operation.as_ref().map(|o| (o.col, o.row)).or(user.cursor)?,
                    _ => user.cursor?,
                };

                Some(RemoteCursor {
                    nick: user.user.nick.clone(),
                    col,
                    row,
                    user_id: user.user.id,
                    mode,
                })
            })
            .collect();

        if let ModeState::Ansi(editor) = &mut self.mode_state {
            editor.set_remote_cursors(cursors);
            self.sync_remote_paste_previews_to_editor();
        }
    }

    fn view_status_bar(&self) -> Element<'_, Message> {
        // Special handling for Animation mode - clickable log message in center
        if let ModeState::Animation(editor) = &self.mode_state {
            let (line, col) = editor.cursor_position();
            let left_text = format!("Ln {}, Col {}", line + 1, col + 1);
            let right_text = if editor.is_dirty() { "Modified" } else { "" };

            // Center shows last log message, clickable to toggle log panel
            let log_msg = editor.last_log_message().unwrap_or_else(|| "Log".to_string());
            let log_icon = if editor.is_log_visible() { "▼" } else { "▶" };
            let center_content = mouse_area(container(text(format!("{} {}", log_icon, log_msg)).size(12)).center_x(Length::Fill))
                .on_press(Message::AnimationEditor(AnimationEditorMessage::ToggleLogPanel));

            return container(
                row![
                    // Left section
                    container(text(left_text).size(12)).width(Length::FillPortion(1)),
                    // Center section - clickable log message
                    container(center_content).width(Length::FillPortion(2)).center_x(Length::Fill),
                    // Right section
                    container(text(right_text).size(12)).width(Length::FillPortion(1)),
                ]
                .align_y(Alignment::Center)
                .padding([2, 8]),
            )
            .height(Length::Fixed(24.0))
            .into();
        }

        // ANSI editor status bar - moebius style
        if let ModeState::Ansi(editor) = &self.mode_state {
            let info: AnsiStatusBarInfo = editor.status_info().into();
            return self.view_ansi_status_bar(info);
        }

        // BitFont/CharFont - simple status bar
        let (left, center, right) = match &self.mode_state {
            ModeState::BitFont(editor) => editor.status_info(),
            ModeState::CharFont(editor) => editor.status_info(),
            _ => (String::new(), String::new(), String::new()),
        };

        container(
            row![
                container(text(left).size(12)).width(Length::FillPortion(1)),
                container(text(center).size(12)).width(Length::FillPortion(1)).center_x(Length::Fill),
                container(text(right).size(12)).width(Length::FillPortion(1)).align_x(Alignment::End),
            ]
            .align_y(Alignment::Center)
            .padding([2, 8]),
        )
        .height(Length::Fixed(24.0))
        .into()
    }

    /// Render ANSI editor status bar in moebius style:
    /// Left: Letter Spacing toggle (9px/8px), iCE/BLINK toggle, SQUARE toggle, ASPECT RATIO toggle
    /// Center: Buffer dimensions
    /// Right: Position/Selection, Font
    fn view_ansi_status_bar(&self, info: AnsiStatusBarInfo) -> Element<'_, Message> {
        // Copy values needed for closures
        let use_aspect_ratio = info.use_aspect_ratio;

        // Left section: Toggles with separators
        // Letter spacing: "9 px" when on, "8 px" when off
        let letter_spacing_text = if info.letter_spacing { "9 px" } else { "8 px" };
        let letter_spacing_toggle = button(text(letter_spacing_text).size(14))
            .style(statusbar_toggle_style)
            .padding([0, 4])
            .on_press(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleLetterSpacing)));

        // Ice colors: "ICE" when on, "BLINK" when off
        let ice_colors_text = if info.ice_colors { "ICE" } else { "BLINK" };
        let ice_colors_toggle = button(text(ice_colors_text).size(14))
            .style(statusbar_toggle_style)
            .padding([0, 4])
            .on_press(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleIceColors)));

        // Aspect ratio: "ASPECT RATIO" when on, "SQUARE" when off
        let aspect_text = if use_aspect_ratio { "ASPECT RATIO" } else { "SQUARE" };
        let aspect_toggle = button(text(aspect_text).size(14))
            .style(statusbar_toggle_style)
            .padding([0, 4])
            .on_press(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleAspectRatio)));

        // Vertical separator
        let separator = || rule::vertical(1).style(statusbar_separator_style);

        let left_section = row![
            ice_colors_toggle,
            Space::new().width(8.0),
            separator(),
            Space::new().width(8.0),
            letter_spacing_toggle,
            Space::new().width(8.0),
            separator(),
            Space::new().width(8.0),
            aspect_toggle,
        ]
        .align_y(Alignment::Center);

        // Center section: Buffer dimensions (secondary color)
        let center_text = format!("{}×{}", info.buffer_size.0, info.buffer_size.1);

        // Right section: Position/Selection + Font
        let position_text: Element<'_, Message> = if let Some(((min_x, min_y), (max_x, max_y))) = info.selection_range {
            // Show selection range and size
            let width = (max_x - min_x).abs() + 1;
            let height = (max_y - min_y).abs() + 1;
            text(format!("({},{})–({},{}) {}×{}", min_x, min_y, max_x, max_y, width, height))
                .size(14)
                .style(statusbar_secondary_text_style)
                .into()
        } else if let Some((x, y)) = info.cursor_position {
            text(format!("({},{})", x, y)).size(14).style(statusbar_secondary_text_style).into()
        } else {
            Space::new().width(0.0).into()
        };

        // Font display - XBinExtended has slot buttons, otherwise clickable font name
        let font_section: Element<'_, Message> = if let Some(slots) = &info.slot_fonts {
            let slot0_name = slots.slot0_name.as_deref().unwrap_or("Slot 0");
            let slot1_name = slots.slot1_name.as_deref().unwrap_or("Slot 1");

            let slot0_style = if slots.current_slot == 0 { active_slot_style } else { inactive_slot_style };
            let slot1_style = if slots.current_slot == 1 { active_slot_style } else { inactive_slot_style };

            let slot0_btn = mouse_area(container(text(format!("0: {}", slot0_name)).size(14)).style(slot0_style).padding([2, 6]))
                .on_press(Message::AnsiEditor(AnsiEditorMessage::SwitchFontSlot(0)));

            let slot1_btn = mouse_area(container(text(format!("1: {}", slot1_name)).size(14)).style(slot1_style).padding([2, 6]))
                .on_press(Message::AnsiEditor(AnsiEditorMessage::SwitchFontSlot(1)));

            row![slot0_btn, Space::new().width(4.0), slot1_btn].align_y(Alignment::Center).into()
        } else {
            // Font name as button with hover effect
            let font_display = button(text(info.font_name.clone()).size(14))
                .style(statusbar_font_button_style)
                .padding([0, 4])
                .on_press(Message::AnsiEditor(AnsiEditorMessage::OpenFontSelector));
            font_display.into()
        };

        let right_section = row![position_text, Space::new().width(16.0), font_section,].align_y(Alignment::Center);

        container(
            row![
                container(left_section).width(Length::FillPortion(1)),
                container(text(center_text).size(14).style(statusbar_secondary_text_style))
                    .width(Length::FillPortion(1))
                    .center_x(Length::Fill),
                container(right_section).width(Length::FillPortion(1)).align_x(Alignment::End),
            ]
            .align_y(Alignment::Center)
            .padding([2, 8]),
        )
        .height(Length::Fixed(24.0))
        .into()
    }

    /// Check if animation editor needs periodic ticks (for playback or recompilation)
    pub fn needs_animation_tick(&self) -> bool {
        if let ModeState::Animation(editor) = &self.mode_state {
            editor.is_playing() || editor.needs_recompile_check()
        } else {
            false
        }
    }

    /// Get undo/redo descriptions for menu display
    pub fn get_undo_info(&self) -> UndoInfo {
        match &self.mode_state {
            ModeState::Ansi(editor) => editor.with_edit_state_readonly(|state| UndoInfo::new(state.undo_description(), state.redo_description())),
            ModeState::BitFont(editor) => UndoInfo::new(editor.undo_description(), editor.redo_description()),
            ModeState::CharFont(editor) => UndoInfo::new(editor.undo_description(), editor.redo_description()),
            ModeState::Animation(editor) => UndoInfo::new(editor.undo_description(), editor.redo_description()),
        }
    }

    /// Check if the window is connected to a collaboration server
    pub fn is_connected(&self) -> bool {
        self.collaboration_state.active
    }

    /// Handle events passed from the window manager
    pub fn handle_event(&mut self, event: &Event) -> (Option<Message>, Task<Message>) {
        // When chat input is focused, do NOT forward normal typing events to the editor.
        // Iced widgets will still receive them, but the editor (and command handler)
        // would otherwise also interpret them, causing text to be drawn while typing chat.
        if self.collaboration_state.chat_input_focused {
            if let Event::Keyboard(key_event) = event {
                // Allow shortcuts (Ctrl/Alt/Logo) to still work.
                // Block plain typing (and navigation keys) from reaching editor/commands.
                match key_event {
                    icy_ui::keyboard::Event::KeyPressed { modifiers, .. } if !modifiers.control() && !modifiers.alt() && !modifiers.logo() => {
                        return (None, Task::none());
                    }
                    icy_ui::keyboard::Event::KeyReleased { modifiers, .. } if !modifiers.control() && !modifiers.alt() && !modifiers.logo() => {
                        return (None, Task::none());
                    }
                    _ => {}
                }
            }
        }

        // Try the command handler first for both keyboard and mouse events
        if let Some(msg) = self.commands.handle(event) {
            if self.collaboration_state.chat_input_focused {
                if let Event::Mouse(icy_ui::mouse::Event::ButtonPressed { .. }) = event {
                    self.collaboration_state.chat_input_focused = false;
                }
            }
            return (Some(msg), Task::none());
        }

        // If dialogs are open, route events there first
        if !self.dialogs.is_empty() {
            let task = self.dialogs.handle_event(event);
            // Dialogs consume all events when open
            return (None, task);
        }

        // Note: Menu hotkeys are now handled by icy_ui's AppMenu system in WindowManager.
        // The application_menu() function defines shortcuts via MenuShortcut, which are
        // automatically processed by the shell before events reach here.

        // Handle editor-specific events (tools, navigation, etc.)
        match &mut self.mode_state {
            ModeState::Ansi(editor) => {
                if editor.handle_event(event) {
                    if self.collaboration_state.chat_input_focused {
                        if let Event::Mouse(icy_ui::mouse::Event::ButtonPressed { .. }) = event {
                            self.collaboration_state.chat_input_focused = false;
                        }
                    }
                    return (None, Task::none());
                }
            }
            ModeState::BitFont(state) => {
                if let Some(msg) = state.handle_event(event) {
                    return (Some(Message::BitFontEditor(msg)), Task::none());
                }
            }
            ModeState::CharFont(state) => {
                if state.handle_event(event) {
                    return (None, Task::none());
                }
            }
            ModeState::Animation(_state) => {}
        }

        (None, Task::none())
    }

    /// Handle MCP commands from the automation server
    pub fn handle_mcp_command(&mut self, cmd: &crate::mcp::McpCommand) {
        use crate::mcp::McpCommand;

        match cmd {
            McpCommand::GetHelp { editor_type, response } => {
                let doc = match editor_type.as_deref() {
                    Some("animation") => include_str!("../../../doc/ANIMATION.md"),
                    Some("bitfont") => include_str!("../../../doc/BITFONT.md"),
                    _ => include_str!("../../../doc/HELP.md"),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(doc.to_string());
                }
            }

            McpCommand::GetStatus(response) => {
                let status = self.build_editor_status();
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(status);
                }
            }

            McpCommand::NewDocument { doc_type, response } => {
                let result = match doc_type.as_str() {
                    "ansi" => {
                        self.mode_state = ModeState::Ansi(AnsiEditorMainArea::new(self.options.clone(), self.font_library.clone()));
                        self.last_save = 0;
                        self.update_title();
                        Ok(())
                    }
                    "animation" => {
                        self.mode_state = ModeState::Animation(AnimationEditor::new());
                        self.last_save = 0;
                        self.update_title();
                        Ok(())
                    }
                    "bitfont" => {
                        self.mode_state = ModeState::BitFont(BitFontEditor::new());
                        self.last_save = 0;
                        self.update_title();
                        Ok(())
                    }
                    "charfont" => {
                        self.mode_state = ModeState::CharFont(crate::ui::editor::charfont::CharFontEditor::new(
                            self.options.clone(),
                            self.font_library.clone(),
                        ));
                        self.last_save = 0;
                        self.update_title();
                        Ok(())
                    }
                    _ => Err(format!("Unknown document type: {}", doc_type)),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::LoadDocument { path, response } => {
                let path_buf = PathBuf::from(path);
                let result = self.load_file_internal(&path_buf);
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::Save(response) => {
                let result = if let Some(path) = self.file_path().cloned() {
                    match self.mode_state.save(&path) {
                        Ok(()) => {
                            self.mark_saved();
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                } else {
                    Err("No file path set. Use save_as or provide a path.".to_string())
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::Undo(response) => {
                let result = match &mut self.mode_state {
                    ModeState::Ansi(editor) => {
                        editor.with_edit_state(|state| {
                            if let Err(e) = state.undo() {
                                log::error!("MCP Undo failed: {}", e);
                            }
                        });
                        editor.sync_ui();
                        Ok(())
                    }
                    ModeState::BitFont(editor) => {
                        editor.undo();
                        Ok(())
                    }
                    ModeState::CharFont(_editor) => {
                        // CharFont doesn't have direct undo support yet
                        Ok(())
                    }
                    ModeState::Animation(_editor) => {
                        // Animation uses text_editor's built-in undo
                        Ok(())
                    }
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::Redo(response) => {
                let result = match &mut self.mode_state {
                    ModeState::Ansi(editor) => {
                        editor.with_edit_state(|state| {
                            if let Err(e) = state.redo() {
                                log::error!("MCP Redo failed: {}", e);
                            }
                        });
                        editor.sync_ui();
                        Ok(())
                    }
                    ModeState::BitFont(editor) => {
                        editor.redo();
                        Ok(())
                    }
                    ModeState::CharFont(_editor) => {
                        // CharFont doesn't have direct redo support yet
                        Ok(())
                    }
                    ModeState::Animation(_editor) => {
                        // Animation uses text_editor's built-in redo
                        Ok(())
                    }
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            // Animation-specific commands
            McpCommand::AnimationGetText { offset, length, response } => {
                let result = match &self.mode_state {
                    ModeState::Animation(editor) => {
                        let text = editor.get_script_text();
                        let start = offset.unwrap_or(0);
                        let len = length.unwrap_or(text.len().saturating_sub(start));
                        let end = (start + len).min(text.len());
                        Ok(text[start..end].to_string())
                    }
                    _ => Err("Not in animation editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnimationReplaceText {
                offset,
                length,
                text,
                response,
            } => {
                let result = match &mut self.mode_state {
                    ModeState::Animation(editor) => {
                        editor.replace_script_text(*offset, *length, text);
                        Ok(())
                    }
                    _ => Err("Not in animation editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnimationGetScreen { frame, format, response } => {
                let result = match &self.mode_state {
                    ModeState::Animation(editor) => editor.get_frame_as_text(*frame, format),
                    _ => Err("Not in animation editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            // BitFont-specific commands
            McpCommand::BitFontListChars(response) => {
                let result = match &self.mode_state {
                    ModeState::BitFont(editor) => Ok(editor.list_char_codes()),
                    _ => Err("Not in bitfont editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::BitFontGetChar { code, response } => {
                let result = match &self.mode_state {
                    ModeState::BitFont(editor) => editor.get_glyph_data(*code),
                    _ => Err("Not in bitfont editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::BitFontSetChar { data, response, .. } => {
                let result = match &mut self.mode_state {
                    ModeState::BitFont(editor) => editor.set_glyph_data(data),
                    _ => Err("Not in bitfont editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiRunScript {
                script,
                undo_description,
                response,
            } => {
                let result = match &mut self.mode_state {
                    ModeState::Ansi(editor) => editor.run_lua_script(&script, undo_description.as_deref()),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiGetLayer { layer, response } => {
                let result = match &self.mode_state {
                    ModeState::Ansi(editor) => editor.get_layer_data(*layer),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiSetChar {
                layer,
                x,
                y,
                ch,
                attribute,
                response,
            } => {
                let result = match &mut self.mode_state {
                    ModeState::Ansi(editor) => editor.set_char_at(*layer, *x, *y, ch, attribute),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiSetColor { index, r, g, b, response } => {
                let result = match &mut self.mode_state {
                    ModeState::Ansi(editor) => editor.set_palette_color(*index, *r, *g, *b),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiGetScreen { format, response } => {
                let result = match &self.mode_state {
                    ModeState::Ansi(editor) => editor.get_screen(format),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiGetCaret { response } => {
                let result = match &self.mode_state {
                    ModeState::Ansi(editor) => editor.get_caret_info(),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiSetCaret { x, y, attribute, response } => {
                let result = match &mut self.mode_state {
                    ModeState::Ansi(editor) => editor.set_caret(*x, *y, attribute),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiListLayers { response } => {
                let result = match &self.mode_state {
                    ModeState::Ansi(editor) => editor.list_layers(),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiAddLayer { after_layer, response } => {
                let result = match &mut self.mode_state {
                    ModeState::Ansi(editor) => editor.add_layer(*after_layer),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiDeleteLayer { layer, response } => {
                let result = match &mut self.mode_state {
                    ModeState::Ansi(editor) => editor.delete_layer(*layer),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiSetLayerProps {
                layer,
                title,
                is_visible,
                is_locked,
                is_position_locked,
                offset_x,
                offset_y,
                transparency,
                response,
            } => {
                let result = match &mut self.mode_state {
                    ModeState::Ansi(editor) => editor.set_layer_props(&crate::mcp::types::AnsiSetLayerPropsRequest {
                        layer: *layer,
                        title: title.clone(),
                        is_visible: *is_visible,
                        is_locked: *is_locked,
                        is_position_locked: *is_position_locked,
                        offset_x: *offset_x,
                        offset_y: *offset_y,
                        transparency: *transparency,
                    }),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiMergeDownLayer { layer, response } => {
                let result = match &mut self.mode_state {
                    ModeState::Ansi(editor) => editor.merge_down_layer(*layer),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiMoveLayer { layer, direction, response } => {
                let result = match &mut self.mode_state {
                    ModeState::Ansi(editor) => editor.move_layer(*layer, direction.clone()),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiResize { width, height, response } => {
                let result = match &mut self.mode_state {
                    ModeState::Ansi(editor) => editor.resize_buffer(*width, *height),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiGetRegion {
                layer,
                x,
                y,
                width,
                height,
                response,
            } => {
                let result = match &self.mode_state {
                    ModeState::Ansi(editor) => editor.get_region(*layer, *x, *y, *width, *height),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiSetRegion {
                layer,
                x,
                y,
                width,
                height,
                chars,
                response,
            } => {
                let result = match &mut self.mode_state {
                    ModeState::Ansi(editor) => editor.set_region(*layer, *x, *y, *width, *height, chars),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiGetSelection { response } => {
                let result = match &self.mode_state {
                    ModeState::Ansi(editor) => editor.get_selection(),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiSetSelection { x, y, width, height, response } => {
                let result = match &mut self.mode_state {
                    ModeState::Ansi(editor) => editor.set_selection(*x, *y, *width, *height),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiClearSelection { response } => {
                let result = match &mut self.mode_state {
                    ModeState::Ansi(editor) => editor.clear_selection(),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }

            McpCommand::AnsiSelectionAction { action, response } => {
                let result = match &mut self.mode_state {
                    ModeState::Ansi(editor) => editor.selection_action(action),
                    _ => Err("Not in ANSI editor mode".to_string()),
                };
                if let Some(tx) = response.lock().take() {
                    let _ = tx.send(result);
                }
            }
        }
    }

    /// Build editor status for MCP get_status command
    fn build_editor_status(&self) -> crate::mcp::types::EditorStatus {
        use crate::mcp::types::{
            AnimationStatus, AnsiStatus, BitFontStatus, BufferInfo, CaretInfo, ColorInfo, EditorStatus, LayerInfo, RectangleInfo, SelectionInfo,
            TextAttributeInfo,
        };

        let editor = match &self.mode_state {
            ModeState::Ansi(_) => "ansi",
            ModeState::BitFont(_) => "bitfont",
            ModeState::CharFont(_) => "charfont",
            ModeState::Animation(_) => "animation",
        };

        let file = self.file_path().map(|p| p.display().to_string());
        let dirty = self.is_modified();

        // Build ANSI status if in ANSI mode
        let ansi = if let ModeState::Ansi(ansi_editor) = &self.mode_state {
            Some(ansi_editor.with_edit_state_readonly(|state| {
                let buffer = state.get_buffer();
                let caret = state.get_caret();

                // Convert AttributeColor to ColorInfo
                let fg_color = match caret.attribute.foreground_color() {
                    icy_engine::AttributeColor::Palette(n) => ColorInfo::Palette(n),
                    icy_engine::AttributeColor::ExtendedPalette(n) => ColorInfo::ExtendedPalette(n),
                    icy_engine::AttributeColor::Rgb(r, g, b) => ColorInfo::Rgb { r, g, b },
                    icy_engine::AttributeColor::Transparent => ColorInfo::Transparent,
                };
                let bg_color = match caret.attribute.background_color() {
                    icy_engine::AttributeColor::Palette(n) => ColorInfo::Palette(n),
                    icy_engine::AttributeColor::ExtendedPalette(n) => ColorInfo::ExtendedPalette(n),
                    icy_engine::AttributeColor::Rgb(r, g, b) => ColorInfo::Rgb { r, g, b },
                    icy_engine::AttributeColor::Transparent => ColorInfo::Transparent,
                };

                // Build layer info
                let layers: Vec<LayerInfo> = buffer
                    .layers
                    .iter()
                    .enumerate()
                    .map(|(index, layer)| {
                        let mode = match layer.properties.mode {
                            icy_engine::Mode::Normal => "normal",
                            icy_engine::Mode::Chars => "chars",
                            icy_engine::Mode::Attributes => "attributes",
                        };
                        let role = match layer.role {
                            icy_engine::Role::Normal => "normal",
                            icy_engine::Role::Image => "image",
                        };
                        LayerInfo {
                            index,
                            title: layer.properties.title.clone(),
                            is_visible: layer.properties.is_visible,
                            is_locked: layer.properties.is_locked,
                            is_position_locked: layer.properties.is_position_locked,
                            offset_x: layer.offset().x,
                            offset_y: layer.offset().y,
                            width: layer.size().width,
                            height: layer.size().height,
                            mode: mode.to_string(),
                            role: role.to_string(),
                        }
                    })
                    .collect();

                // Build selection info
                let selection = state.selection().map(|sel| {
                    let rect = sel.as_rectangle();
                    SelectionInfo {
                        anchor_x: sel.anchor.x,
                        anchor_y: sel.anchor.y,
                        lead_x: sel.lead.x,
                        lead_y: sel.lead.y,
                        shape: match sel.shape {
                            icy_engine::Shape::Rectangle => "rectangle".to_string(),
                            icy_engine::Shape::Lines => "lines".to_string(),
                        },
                        locked: sel.locked,
                        bounds: RectangleInfo {
                            x: rect.left(),
                            y: rect.top(),
                            width: rect.width(),
                            height: rect.height(),
                        },
                    }
                });

                // Font mode string
                let font_mode = match buffer.font_mode {
                    icy_engine::FontMode::Sauce => "sauce",
                    icy_engine::FontMode::Single => "single",
                    icy_engine::FontMode::FixedSize => "fixed_size",
                    icy_engine::FontMode::Unlimited => "unlimited",
                };

                // Ice mode string
                let ice_mode = match buffer.ice_mode {
                    icy_engine::IceMode::Unlimited => "unlimited",
                    icy_engine::IceMode::Blink => "blink",
                    icy_engine::IceMode::Ice => "ice",
                };

                // Get document position from layer position
                let doc_pos = state.layer_to_document_position(caret.position());

                AnsiStatus {
                    buffer: BufferInfo {
                        width: buffer.width(),
                        height: buffer.height(),
                        layer_count: buffer.layers.len(),
                        font_count: buffer.font_count(),
                        font_mode: font_mode.to_string(),
                        ice_mode: ice_mode.to_string(),
                        palette: buffer
                            .palette
                            .export_palette(&FileFormat::Palette(icy_engine::PaletteFormat::Hex))
                            .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
                            .unwrap_or_default(),
                    },
                    caret: CaretInfo {
                        x: caret.x,
                        y: caret.y,
                        doc_x: doc_pos.x,
                        doc_y: doc_pos.y,
                        attribute: TextAttributeInfo {
                            foreground: fg_color,
                            background: bg_color,
                            bold: caret.attribute.is_bold(),
                            blink: caret.attribute.is_blinking(),
                        },
                        insert_mode: caret.insert_mode,
                        font_page: caret.font_page(),
                    },
                    layers,
                    current_layer: state.get_current_layer().unwrap_or(0),
                    selection,
                    format_mode: state.get_format_mode().to_string(),
                    outline_style: state.get_outline_style(),
                    mirror_mode: state.get_mirror_mode(),
                }
            }))
        } else {
            None
        };

        let animation = if let ModeState::Animation(editor) = &self.mode_state {
            Some(AnimationStatus {
                text_length: editor.get_script_text().len(),
                frame_count: editor.frame_count(),
                errors: editor.get_errors(),
                is_playing: editor.is_playing(),
                current_frame: editor.current_frame(),
            })
        } else {
            None
        };

        let bitfont = if let ModeState::BitFont(editor) = &self.mode_state {
            let (width, height) = editor.font_size();
            Some(BitFontStatus {
                glyph_width: width,
                glyph_height: height,
                glyph_count: editor.glyph_count(),
                first_char: editor.first_char(),
                last_char: editor.last_char(),
                selected_char: editor.selected_char_code(),
            })
        } else {
            None
        };

        EditorStatus {
            editor: editor.to_string(),
            file,
            dirty,
            ansi,
            animation,
            bitfont,
        }
    }

    /// Internal method to load a file (used by MCP and regular file loading)
    fn load_file_internal(&mut self, path: &PathBuf) -> Result<(), String> {
        let format = icy_engine::formats::FileFormat::from_path(path);

        match format {
            Some(icy_engine::formats::FileFormat::BitFont(_)) => match BitFontEditor::from_file(path.clone()) {
                Ok(editor) => {
                    self.mode_state = ModeState::BitFont(editor);
                    self.last_save = self.mode_state.undo_stack_len();
                    self.update_title();
                    Ok(())
                }
                Err(e) => Err(e),
            },
            Some(icy_engine::formats::FileFormat::IcyAnim) => match AnimationEditor::load_file(path.clone()) {
                Ok(editor) => {
                    self.mode_state = ModeState::Animation(editor);
                    self.last_save = self.mode_state.undo_stack_len();
                    self.update_title();
                    Ok(())
                }
                Err(e) => Err(e),
            },
            Some(icy_engine::formats::FileFormat::CharacterFont(_)) => {
                match crate::ui::editor::charfont::CharFontEditor::with_file(path.clone(), self.options.clone(), self.font_library.clone()) {
                    Ok(editor) => {
                        self.mode_state = ModeState::CharFont(editor);
                        self.last_save = self.mode_state.undo_stack_len();
                        self.update_title();
                        Ok(())
                    }
                    Err(e) => Err(format!("{}", e)),
                }
            }
            _ => match AnsiEditorMainArea::with_file(path.clone(), self.options.clone(), self.font_library.clone()) {
                Ok(editor) => {
                    self.mode_state = ModeState::Ansi(editor);
                    self.last_save = self.mode_state.undo_stack_len();
                    self.update_title();
                    Ok(())
                }
                Err(e) => Err(format!("{}", e)),
            },
        }
    }
}

// Implement the Window trait for use with shared WindowManager helpers
impl icy_engine_gui::Window for MainWindow {
    type Message = Message;

    fn id(&self) -> usize {
        self.id
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn get_zoom_info_string(&self) -> String {
        MainWindow::get_zoom_info_string(self)
    }

    fn update(&mut self, msg: Self::Message) -> Task<Self::Message> {
        self.update(msg)
    }

    fn view(&self) -> Element<'_, Self::Message> {
        self.view()
    }

    fn theme(&self) -> Theme {
        self.theme()
    }

    fn handle_event(&mut self, event: &icy_ui::Event) -> (Option<Self::Message>, Task<Self::Message>) {
        self.handle_event(event)
    }
}

// ============================================================================
// Style functions for status bar slot buttons
// ============================================================================

fn active_slot_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(icy_ui::Background::Color(theme.accent.base)),
        text_color: Some(theme.background.on),
        border: icy_ui::Border {
            radius: 3.0.into(),
            width: 1.0,
            color: theme.accent.hover,
        },
        ..Default::default()
    }
}

fn inactive_slot_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(icy_ui::Background::Color(theme.secondary.base)),
        text_color: Some(theme.background.on),
        border: icy_ui::Border {
            radius: 3.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

// ============================================================================
// Style functions for status bar toggles and text
// ============================================================================

/// Style for status bar toggle buttons (left section)
/// Default: secondary color, Hover: base.text color
fn statusbar_toggle_style(theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(icy_ui::Background::Color(Color::TRANSPARENT)),
        text_color: theme.button.on,
        border: Border::default(),
        ..Default::default()
    };

    match status {
        button::Status::Hovered | button::Status::Pressed => button::Style {
            text_color: theme.background.on,
            ..base
        },
        _ => base,
    }
}

/// Style for the font button in status bar (right section)
/// Default: secondary color, Hover: base.text color
fn statusbar_font_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(icy_ui::Background::Color(Color::TRANSPARENT)),
        text_color: theme.button.on,
        border: Border::default(),
        ..Default::default()
    };

    match status {
        button::Status::Hovered | button::Status::Pressed => button::Style {
            text_color: theme.background.on,
            ..base
        },
        _ => base,
    }
}

/// Style for secondary text in status bar (position, dimensions)
fn statusbar_secondary_text_style(theme: &Theme) -> text::Style {
    text::Style { color: Some(theme.button.on) }
}

/// Style for vertical separator in status bar
fn statusbar_separator_style(theme: &Theme) -> rule::Style {
    rule::Style {
        color: theme.button.base,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: false,
    }
}
