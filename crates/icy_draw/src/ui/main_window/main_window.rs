//! MainWindow for icy_draw
//!
//! Each MainWindow represents one editing window with its own state and mode.
//! The mode determines what kind of editor is shown (ANSI, BitFont, CharFont, Animation).

use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    sync::Arc,
};

use parking_lot::RwLock;

use crate::{SharedFontLibrary, fl};
use iced::{
    Alignment, Element, Event, Length, Subscription, Task, Theme,
    widget::{Space, column, container, mouse_area, row, rule, text},
};
use icy_engine::TextPane;
use icy_engine::formats::FileFormat;
use icy_engine_edit::UndoState;
use icy_engine_gui::commands::cmd;
use icy_engine_gui::ui::{DialogResult, DialogStack, ExportDialogMessage, confirm_yes_no_cancel, error_dialog};
use icy_engine_gui::{Toast, ToastManager, command_handler, command_handlers};

use super::commands::create_draw_commands;
use super::menu::{MenuBarState, UndoInfo};
use crate::Plugin;
use crate::Settings;
use crate::ui::collaboration::CollaborationState;
use crate::ui::editor::animation::{AnimationEditor, AnimationEditorMessage};
use crate::ui::editor::ansi::{AnsiEditorCoreMessage, AnsiEditorMainArea, AnsiEditorMessage, AnsiStatusInfo};
use crate::ui::editor::bitfont::{BitFontEditor, BitFontEditorMessage};

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

use super::commands::{area_cmd, selection_cmd};

// Command handler for MainWindow keyboard shortcuts
command_handler!(MainWindowCommands, create_draw_commands(), => Message {
    // View
    cmd::VIEW_FULLSCREEN => Message::ToggleFullscreen,
    cmd::HELP_ABOUT => Message::ShowAbout,
    // Selection
    selection_cmd::SELECT_NONE => Message::Deselect,
    selection_cmd::SELECT_INVERSE => Message::AnsiEditor(AnsiEditorMessage::InverseSelection),
    selection_cmd::SELECT_ERASE => Message::DeleteSelection,
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
    Paste,
    SelectAll,
    Deselect,
    DeleteSelection,

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
    ReportBug,
    ShowAbout,
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
    /// Show collaboration dialog (connect or host)
    ShowCollaborationDialog,
    /// Toggle chat panel visibility
    ToggleChatPanel,
    /// Collaboration dialog messages
    CollaborationDialog(crate::ui::dialog::CollaborationDialogMessage),
    /// Connect to server with result from dialog
    ConnectToServer(crate::ui::dialog::ConnectDialogResult),
    /// Host a collaboration session
    HostSession(crate::ui::dialog::HostSessionResult),
    /// Chat panel messages
    ChatPanel(crate::ui::collaboration::ChatPanelMessage),
    /// Collaboration subscription message
    Collaboration(crate::ui::collaboration::CollaborationMessage),
}

/// Status bar information that can be provided by any editor mode
#[derive(Clone, Debug, Default)]
pub struct StatusBarInfo {
    pub left: String,
    pub center: String,
    pub right: String,
    /// Optional clickable font name (for ANSI editor)
    pub font_name: Option<String>,
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

impl From<AnsiStatusInfo> for StatusBarInfo {
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

        // Show paste layer info in center when in paste mode
        let center = if let (Some(pos), Some(size)) = (info.paste_layer_position, info.paste_layer_size) {
            format!("Paste: {}×{} @ ({}, {})", size.0, size.1, pos.0, pos.1)
        } else {
            format!("Layer {}/{}", info.current_layer + 1, info.total_layers)
        };

        Self {
            left: format!("{}×{}", info.buffer_size.0, info.buffer_size.1,),
            center,
            right: format!(
                "{}  {}  ({}, {})",
                info.current_tool,
                if info.insert_mode { "INS" } else { "OVR" },
                info.cursor_position.0,
                info.cursor_position.1,
            ),
            font_name: if slot_fonts.is_some() { None } else { Some(info.font_name) },
            slot_fonts,
        }
    }
}

// Command handler for MainWindow - maps hotkeys to messages
command_handlers! {
    fn handle_main_window_command() -> Option<Message> {
        cmd::EDIT_UNDO => Message::Undo,
        cmd::EDIT_REDO => Message::Redo,
        cmd::EDIT_DELETE => Message::DeleteSelection,
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

    /// Menu bar state (tracks expanded menus)
    menu_state: MenuBarState,

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

    /// Cached status bar info for the current `ui_revision`
    status_info_cache: RefCell<Option<(u64, StatusBarInfo)>>,

    /// Loaded plugins from the plugin directory
    plugins: Arc<Vec<Plugin>>,

    /// Toast notifications for user feedback
    toasts: Vec<Toast>,

    /// Collaboration state for real-time editing
    pub(super) collaboration_state: CollaborationState,
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
            menu_state: MenuBarState::new(),
            is_fullscreen: false,
            dialogs,
            commands: MainWindowCommands::new(),
            last_save,
            close_after_save: false,
            pending_open_path: None,
            title: String::new(),
            ui_revision: Cell::new(0),
            status_info_cache: RefCell::new(None),
            plugins: Arc::new(Plugin::read_plugin_directory()),
            toasts: Vec::new(),
            collaboration_state: CollaborationState::new(),
        };
        window.update_title();
        window
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
            menu_state: MenuBarState::new(),
            is_fullscreen: false,
            dialogs,
            commands: MainWindowCommands::new(),
            last_save,
            close_after_save: false,
            pending_open_path: None,
            title: String::new(),
            ui_revision: Cell::new(0),
            status_info_cache: RefCell::new(None),
            plugins: Arc::new(Plugin::read_plugin_directory()),
            toasts: Vec::new(),
            collaboration_state: CollaborationState::new(),
        };
        window.update_title();
        window
    }

    /// Get the current file path
    pub fn file_path(&self) -> Option<&PathBuf> {
        self.mode_state.file_path()
    }

    /// Check if the document is modified (dirty)
    /// Compares current undo stack length with the length at last save
    pub fn is_modified(&self) -> bool {
        self.mode_state.undo_stack_len() != self.last_save
    }

    /// Mark document as saved - updates last_save to current undo stack length
    pub fn mark_saved(&mut self) {
        self.last_save = self.mode_state.undo_stack_len();
        self.update_title();
    }

    /// Update the cached title based on current file path and dirty state
    fn update_title(&mut self) {
        let file_name = self
            .file_path()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| crate::fl!("unsaved-title"));

        let modified = if self.is_modified() { "*" } else { "" };

        self.title = format!("{}{}", file_name, modified);
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
        self.options.read().monitor_settings.read().get_theme()
    }

    /// Get current edit mode
    pub fn mode(&self) -> EditMode {
        self.mode_state.mode()
    }

    /// Get current undo stack length (for autosave tracking)
    pub fn undo_stack_len(&self) -> usize {
        self.mode_state.undo_stack_len()
    }

    /// Get a clone of the undo stack for session serialization
    pub fn get_undo_stack(&self) -> Option<icy_engine_edit::EditorUndoStack> {
        self.mode_state.get_undo_stack()
    }

    /// Restore undo stack from session
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
                        .unwrap_or_else(|| "Untitled".to_string());

                    self.dialogs.push(confirm_yes_no_cancel(
                        format!("Save changes to \"{}\"?", filename),
                        "Your changes will be lost if you don't save them.",
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
                use crate::ui::dialog::new_file::{FileTemplate, create_buffer_for_template};
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
                        Task::none()
                    }
                    ModeState::BitFont(editor) => {
                        editor.undo();
                        Task::none()
                    }
                    ModeState::CharFont(_) => Task::none(),
                    ModeState::Animation(_) => Task::none(), // Animation uses text_editor's built-in undo
                }
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
                        Task::none()
                    }
                    ModeState::BitFont(editor) => {
                        editor.redo();
                        Task::none()
                    }
                    ModeState::CharFont(_) => Task::none(),
                    ModeState::Animation(_) => Task::none(), // Animation uses text_editor's built-in redo
                }
            }
            Message::Cut => {
                match &mut self.mode_state {
                    ModeState::BitFont(editor) => {
                        if let Err(e) = editor.state.cut() {
                            log::error!("Cut failed: {}", e);
                        }
                        editor.invalidate_caches();
                    }
                    ModeState::Ansi(editor) => {
                        if let Err(e) = editor.cut() {
                            log::error!("Cut failed: {}", e);
                        }
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
                        if let Err(e) = editor.state.copy() {
                            log::error!("Copy failed: {}", e);
                        }
                        editor.invalidate_caches();
                    }
                    ModeState::Ansi(editor) => {
                        if let Err(e) = editor.copy() {
                            log::error!("Copy failed: {}", e);
                        }
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
            Message::Paste => {
                match &mut self.mode_state {
                    ModeState::BitFont(editor) => {
                        if let Err(e) = editor.state.paste() {
                            log::error!("Paste failed: {}", e);
                        }
                        editor.invalidate_caches();
                    }
                    ModeState::Ansi(editor) => {
                        if let Err(e) = editor.paste() {
                            log::error!("Paste failed: {}", e);
                        }
                    }
                    ModeState::CharFont(_) => {
                        // TODO: Implement paste for CharFont
                    }
                    ModeState::Animation(_) => {
                        // TODO: Implement paste for Animation
                    }
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
                    let mut collab_tasks: Vec<iced::Task<Message>> = Vec::new();
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
            // SettingsDialog messages are routed through DialogStack::update above
            Message::SettingsDialog(_) => Task::none(),
            Message::SettingsSaved(_) => {
                // Apply outline style to current ANSI editor (if any)
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    let outline_style = { *self.options.read().font_outline_style.read() };
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
            Message::DeleteSelection => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    return editor
                        .update(
                            AnsiEditorMessage::Core(AnsiEditorCoreMessage::DeleteSelection),
                            &mut self.dialogs,
                            &self.plugins,
                        )
                        .map(Message::AnsiEditor);
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
                    iced::window::Mode::Fullscreen
                } else {
                    iced::window::Mode::Windowed
                };
                iced::window::latest().and_then(move |window| iced::window::set_mode(window, mode))
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
            Message::ShowCollaborationDialog => {
                // Show collaboration dialog with settings pre-filled
                use crate::ui::dialog::CollaborationDialog;
                let opts = self.options.read();
                self.dialogs.push(CollaborationDialog::with_settings(&opts));
                Task::none()
            }
            Message::CollaborationDialog(_) => {
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
                    let opts = self.options.read();
                    opts.add_collaboration_server(&result.url);
                    opts.set_collaboration_nick(&result.nick);
                    opts.set_collaboration_group(&result.group);
                }

                // Store connection info and start connecting
                // The subscription will pick this up and establish the connection
                self.collaboration_state
                    .start_connecting(result.url.clone(), result.nick.clone(), result.group.clone(), result.password.clone());

                let toast = Toast::info(format!("Connecting to {}...", result.url));
                Task::done(Message::ShowToast(toast))
            }
            Message::HostSession(ref result) => {
                // Start hosting a collaboration session with the current document
                log::info!("Starting collaboration server on port {} as {}", result.port, result.nick);

                // Save nick and group to settings
                {
                    let opts = self.options.read();
                    opts.set_collaboration_nick(&result.nick);
                    opts.set_collaboration_group(&result.group);
                }

                // Get document dimensions from current editor
                let (columns, rows) = if let ModeState::Ansi(editor) = &self.mode_state {
                    editor.get_buffer_dimensions()
                } else {
                    (80, 25)
                };

                // Start the embedded server
                let port = result.port;
                let password = result.password.clone();
                let nick = result.nick.clone();

                // Spawn server in background
                std::thread::spawn(move || {
                    use icy_engine_edit::collaboration::{ServerConfig, run_server as run_collab_server};

                    let bind_addr = format!("0.0.0.0:{}", port);
                    let bind_addr: std::net::SocketAddr = match bind_addr.parse() {
                        Ok(addr) => addr,
                        Err(e) => {
                            log::error!("Invalid bind address: {}", e);
                            return;
                        }
                    };

                    let config = ServerConfig {
                        bind_addr,
                        password,
                        max_users: 0,
                        columns,
                        rows,
                        enable_extended_protocol: true,
                        status_message: String::new(),
                    };

                    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
                    rt.block_on(async {
                        if let Err(e) = run_collab_server(config).await {
                            log::error!("Server error: {}", e);
                        }
                    });
                });

                // Give server a moment to start, then connect to it
                let local_url = format!("ws://127.0.0.1:{}", port);
                let group = result.group.clone();
                self.collaboration_state
                    .start_connecting(local_url.clone(), nick, group, result.password.clone());

                let toast = Toast::success(format!("Hosting session on port {}", port));
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
                                    editor.scroll_to_position(col, row);
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

                                // Apply initial document snapshot from server
                                if let ModeState::Ansi(editor) = &mut self.mode_state {
                                    editor.apply_remote_document(&doc);
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
                            CollaborationEvent::CanvasResized { columns, rows } => {
                                self.collaboration_state.update_canvas_size(*columns, *rows);

                                if let ModeState::Ansi(editor) = &mut self.mode_state {
                                    editor.apply_remote_canvas_resize(*columns, *rows);
                                    editor.sync_ui();
                                }
                            }
                            CollaborationEvent::PasteAsSelection { user_id, blocks: _ } => {
                                // Moebius sends blocks for a floating selection preview.
                                // Our UI currently only visualizes the cursor mode; keep the last known position.
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
                                let toast = Toast::info("Disconnected from collaboration server".to_string());
                                return Task::done(Message::ShowToast(toast));
                            }
                            CollaborationEvent::Error(e) => {
                                log::error!("Collaboration error: {}", e);
                                self.collaboration_state.end_session();
                                let toast = Toast::error(format!("Connection error: {}", e));
                                return Task::done(Message::ShowToast(toast));
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
        use iced::Subscription;

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
        // Build the UI based on current mode
        let options = self.options.clone();

        // Get undo/redo descriptions for menu
        let undo_info = self.get_undo_info();

        // Get marker state for menu display
        let marker_state = match &self.mode_state {
            ModeState::Ansi(editor) => editor.get_marker_menu_state(),
            _ => crate::ui::main_window::menu::MarkerMenuState::default(),
        };

        // Get mirror mode state from editor
        let mirror_mode = match &self.mode_state {
            ModeState::Ansi(editor) => editor.get_mirror_mode(),
            _ => false,
        };

        let menu_bar = self
            .menu_state
            .view(&self.mode_state.mode(), options, &undo_info, &marker_state, self.plugins.clone(), mirror_mode);

        // Pass collaboration state to the ANSI editor; it builds the chat pane and splitter itself.
        let content: Element<'_, Message> = match &self.mode_state {
            ModeState::Ansi(editor) => {
                let collab = self.collaboration_state.active.then_some(&self.collaboration_state);
                editor.view(collab).map(Message::AnsiEditor)
            }
            ModeState::BitFont(editor) => editor.view(None).map(Message::BitFontEditor),
            ModeState::CharFont(editor) => editor.view(None).map(Message::CharFontEditor),
            ModeState::Animation(editor) => editor.view(None).map(Message::AnimationEditor),
        };

        // Status bar
        let status_bar = self.view_status_bar();

        let main_content: Element<'_, Message> = column![menu_bar, content, rule::horizontal(1), status_bar,].into();

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

        // Default status bar for other modes
        let info = self.status_info_cached();

        // Build right section - with slot buttons for XBinExtended or clickable font name
        let right_section: Element<'_, Message> = if let Some(slots) = &info.slot_fonts {
            // XBinExtended mode: show two slot buttons
            let slot0_name = slots.slot0_name.as_deref().unwrap_or("Slot 0");
            let slot1_name = slots.slot1_name.as_deref().unwrap_or("Slot 1");

            let slot0_style = if slots.current_slot == 0 { active_slot_style } else { inactive_slot_style };
            let slot1_style = if slots.current_slot == 1 { active_slot_style } else { inactive_slot_style };

            let slot0_btn = mouse_area(container(text(format!("0: {}", slot0_name)).size(11)).style(slot0_style).padding([2, 6]))
                .on_press(Message::AnsiEditor(AnsiEditorMessage::SwitchFontSlot(0)));

            let slot1_btn = mouse_area(container(text(format!("1: {}", slot1_name)).size(11)).style(slot1_style).padding([2, 6]))
                .on_press(Message::AnsiEditor(AnsiEditorMessage::SwitchFontSlot(1)));

            row![text(format!("{}  ", info.right)).size(12), slot0_btn, Space::new().width(4.0), slot1_btn,]
                .align_y(Alignment::Center)
                .into()
        } else if let Some(font_name) = &info.font_name {
            let font_display = mouse_area(
                container(text(format!("🔤 {}", font_name)).size(12))
                    .style(|theme: &Theme| {
                        let palette = theme.extended_palette();
                        container::Style {
                            background: Some(iced::Background::Color(palette.background.weak.color)),
                            border: iced::Border {
                                radius: 3.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    })
                    .padding([2, 6]),
            )
            .on_press(Message::AnsiEditor(AnsiEditorMessage::OpenFontSelector));

            row![text(format!("{}  ", info.right)).size(12), font_display,]
                .align_y(Alignment::Center)
                .into()
        } else {
            text(info.right).size(12).into()
        };

        container(
            row![
                // Left section
                container(text(info.left).size(12)).width(Length::FillPortion(1)),
                // Center section
                container(text(info.center).size(12)).width(Length::FillPortion(1)).center_x(Length::Fill),
                // Right section - with clickable font name
                container(right_section).width(Length::FillPortion(1)).align_x(Alignment::End),
            ]
            .align_y(Alignment::Center)
            .padding([2, 8]),
        )
        .height(Length::Fixed(24.0))
        .into()
    }

    fn get_status_info(&self) -> StatusBarInfo {
        match &self.mode_state {
            ModeState::Ansi(editor) => editor.status_info().into(),
            ModeState::BitFont(editor) => {
                let (left, center, right) = editor.status_info();
                StatusBarInfo {
                    left,
                    center,
                    right,
                    font_name: None,
                    slot_fonts: None,
                }
            }
            ModeState::CharFont(editor) => {
                let (left, center, right) = editor.status_info();
                StatusBarInfo {
                    left,
                    center,
                    right,
                    font_name: None,
                    slot_fonts: None,
                }
            }
            ModeState::Animation(editor) => {
                let (line, col) = editor.cursor_position();
                StatusBarInfo {
                    left: format!("Ln {}, Col {}", line + 1, col + 1),
                    center: String::new(),
                    right: if editor.is_dirty() { "Modified".into() } else { String::new() },
                    font_name: None,
                    slot_fonts: None,
                }
            }
        }
    }

    fn status_info_cached(&self) -> StatusBarInfo {
        let rev = self.ui_revision.get();
        if let Some((cached_rev, cached)) = self.status_info_cache.borrow().as_ref() {
            if *cached_rev == rev {
                return cached.clone();
            }
        }

        let info = self.get_status_info();
        *self.status_info_cache.borrow_mut() = Some((rev, info.clone()));
        info
    }

    /// Get undo/redo descriptions for menu display
    fn get_undo_info(&self) -> UndoInfo {
        match &self.mode_state {
            ModeState::Ansi(editor) => editor.with_edit_state_readonly(|state| UndoInfo::new(state.undo_description(), state.redo_description())),
            ModeState::BitFont(editor) => UndoInfo::new(editor.undo_description(), editor.redo_description()),
            ModeState::CharFont(editor) => UndoInfo::new(editor.undo_description(), editor.redo_description()),
            ModeState::Animation(editor) => UndoInfo::new(editor.undo_description(), editor.redo_description()),
        }
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
                    iced::keyboard::Event::KeyPressed { modifiers, .. } if !modifiers.control() && !modifiers.alt() && !modifiers.logo() => {
                        return (None, Task::none());
                    }
                    iced::keyboard::Event::KeyReleased { modifiers, .. } if !modifiers.control() && !modifiers.alt() && !modifiers.logo() => {
                        return (None, Task::none());
                    }
                    _ => {}
                }
            }
        }

        // Try the command handler first for both keyboard and mouse events
        if let Some(msg) = self.commands.handle(event) {
            if self.collaboration_state.chat_input_focused {
                if let Event::Mouse(iced::mouse::Event::ButtonPressed(_)) = event {
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

        // Handle mode-specific menu commands
        match &self.mode_state {
            ModeState::BitFont(editor) => {
                // Check BitFont menu commands
                let undo_desc = editor.undo_description();
                let redo_desc = editor.redo_description();
                if let Some(msg) = crate::ui::editor::bitfont::menu_bar::handle_command_event(event, undo_desc.as_deref(), redo_desc.as_deref()) {
                    return (Some(msg), Task::none());
                }
            }
            ModeState::Animation(editor) => {
                // Check Animation menu commands
                let undo_desc = editor.undo_description();
                let redo_desc = editor.redo_description();
                if let Some(msg) = crate::ui::editor::animation::menu_bar::handle_command_event(event, undo_desc.as_deref(), redo_desc.as_deref()) {
                    return (Some(msg), Task::none());
                }
            }
            _ => {}
        }

        // Handle editor-specific events (tools, navigation, etc.)
        match &mut self.mode_state {
            ModeState::Ansi(editor) => {
                if editor.handle_event(event) {
                    if self.collaboration_state.chat_input_focused {
                        if let Event::Mouse(iced::mouse::Event::ButtonPressed(_)) = event {
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
        }
    }

    /// Build editor status for MCP get_status command
    fn build_editor_status(&self) -> crate::mcp::types::EditorStatus {
        use crate::mcp::types::{AnimationStatus, BitFontStatus, EditorStatus};

        let editor = match &self.mode_state {
            ModeState::Ansi(_) => "ansi",
            ModeState::BitFont(_) => "bitfont",
            ModeState::CharFont(_) => "charfont",
            ModeState::Animation(_) => "animation",
        };

        let file = self.file_path().map(|p| p.display().to_string());
        let dirty = self.is_modified();

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

    fn handle_event(&mut self, event: &iced::Event) -> (Option<Self::Message>, Task<Self::Message>) {
        self.handle_event(event)
    }
}

// ============================================================================
// Style functions for status bar slot buttons
// ============================================================================

fn active_slot_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(iced::Background::Color(palette.primary.base.color)),
        text_color: Some(palette.primary.base.text),
        border: iced::Border {
            radius: 3.0.into(),
            width: 1.0,
            color: palette.primary.strong.color,
        },
        ..Default::default()
    }
}

fn inactive_slot_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(iced::Background::Color(palette.background.weak.color)),
        text_color: Some(palette.background.base.text),
        border: iced::Border {
            radius: 3.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
