//! MainWindow for icy_draw
//!
//! Each MainWindow represents one editing window with its own state and mode.
//! The mode determines what kind of editor is shown (ANSI, BitFont, CharFont, Animation).

use std::{path::PathBuf, sync::Arc};

use parking_lot::Mutex;

use iced::{
    Alignment, Element, Event, Length, Task, Theme,
    widget::{column, container, mouse_area, row, rule, text},
};
use icy_engine::formats::FileFormat;
use icy_engine_edit::{EditState, UndoState};
use icy_engine_gui::command_handlers;
use icy_engine_gui::commands::{CommandSet, IntoHotkey, cmd};
use icy_engine_gui::ui::{DialogResult, DialogStack, confirm_yes_no_cancel, error_dialog};

use super::animation_editor::{AnimationEditor, AnimationEditorMessage};
use super::ansi_editor::{AnsiEditor, AnsiEditorMessage, AnsiStatusInfo};
use super::bitfont_editor::{BitFontEditor, BitFontEditorMessage, BitFontTopToolbarMessage};
use super::commands::create_draw_commands;
use super::{
    SharedOptions,
    menu::{MenuBarState, UndoInfo},
};

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

/// State for the CharFont (TDF) editor mode
pub struct CharFontEditorState {
    // TODO: Add CharFont editor state
    // - TDF font data
    // - selected character
}

impl CharFontEditorState {
    pub fn new() -> Self {
        Self {}
    }
}

/// Mode-specific state
pub enum ModeState {
    Ansi(AnsiEditor),
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
            Self::CharFont(_) => 0,
            Self::Animation(editor) => editor.undo_stack_len(),
        }
    }

    /// Get the file path if any
    pub fn file_path(&self) -> Option<&PathBuf> {
        match self {
            Self::Ansi(editor) => editor.file_path.as_ref(),
            Self::BitFont(editor) => editor.file_path(),
            Self::CharFont(_) => None,
            Self::Animation(editor) => editor.file_path(),
        }
    }

    /// Set the file path
    pub fn set_file_path(&mut self, path: PathBuf) {
        match self {
            Self::Ansi(editor) => editor.file_path = Some(path),
            Self::BitFont(editor) => editor.set_file_path(path),
            Self::CharFont(_) => {}
            Self::Animation(editor) => editor.set_file_path(path),
        }
    }

    /// Save the document to the given path
    pub fn save(&mut self, path: &std::path::Path) -> Result<(), String> {
        match self {
            Self::Ansi(editor) => editor.save(path),
            Self::BitFont(editor) => editor.save(path),
            Self::CharFont(_) => Err("CharFont save not implemented".to_string()),
            Self::Animation(editor) => editor.save(path),
        }
    }

    /// Get bytes for autosave (without modifying the file path)
    pub fn get_autosave_bytes(&self) -> Result<Vec<u8>, String> {
        match self {
            Self::Ansi(editor) => editor.get_autosave_bytes(),
            Self::BitFont(editor) => editor.get_autosave_bytes(),
            Self::CharFont(_) => Err("CharFont autosave not implemented".to_string()),
            Self::Animation(editor) => editor.get_autosave_bytes(),
        }
    }

    /// Get the default file extension for this mode
    pub fn default_extension(&self) -> &'static str {
        match self {
            Self::Ansi(_) => "ans",
            Self::BitFont(_) => "psf",
            Self::CharFont(_) => "tdf",
            Self::Animation(_) => "icyanim",
        }
    }
}

/// Message type for MainWindow
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum Message {
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
    PasteAsNewImage,
    PasteAsBrush,
    SelectAll,
    Deselect,
    InverseSelection,
    DeleteSelection,

    // Edit - Area operations
    JustifyLineLeft,
    JustifyLineRight,
    JustifyLineCenter,
    InsertRow,
    DeleteRow,
    InsertColumn,
    DeleteColumn,
    EraseRow,
    EraseRowToStart,
    EraseRowToEnd,
    EraseColumn,
    EraseColumnToStart,
    EraseColumnToEnd,
    ScrollAreaUp,
    ScrollAreaDown,
    ScrollAreaLeft,
    ScrollAreaRight,

    // Edit - Transform
    FlipX,
    FlipY,
    Crop,
    JustifyCenter,
    JustifyLeft,
    JustifyRight,

    // Edit - Document
    ToggleMirrorMode,
    EditSauce,
    ToggleLGAFont,
    ToggleAspectRatio,
    SetCanvasSize,

    // ═══════════════════════════════════════════════════════════════════════════
    // Selection
    // ═══════════════════════════════════════════════════════════════════════════
    // (handled by SelectAll, Deselect, InverseSelection above)

    // ═══════════════════════════════════════════════════════════════════════════
    // Colors
    // ═══════════════════════════════════════════════════════════════════════════
    SwitchIceMode(icy_engine::IceMode),
    SwitchPaletteMode(icy_engine::PaletteMode),
    SelectPalette,
    OpenPalettesDirectory,
    NextFgColor,
    PrevFgColor,
    NextBgColor,
    PrevBgColor,
    PickAttributeUnderCaret,
    ToggleColor,
    SwitchToDefaultColor,

    // ═══════════════════════════════════════════════════════════════════════════
    // Fonts
    // ═══════════════════════════════════════════════════════════════════════════
    SwitchFontMode(icy_engine::FontMode),
    OpenFontSelector,
    AddFonts,
    OpenFontManager,
    OpenFontDirectory,

    // ═══════════════════════════════════════════════════════════════════════════
    // View operations
    // ═══════════════════════════════════════════════════════════════════════════
    ZoomIn,
    ZoomOut,
    ZoomReset,
    SetZoom(f32),
    ToggleFitWidth,
    SetGuide(i32, i32),
    ToggleGuides,
    SetRaster(i32, i32),
    ToggleRaster,
    ToggleLayerBorders,
    ToggleLineNumbers,
    ToggleLeftPanel,
    ToggleRightPanel,
    ToggleFullscreen,
    SetReferenceImage,
    ToggleReferenceImage,
    ClearReferenceImage,

    // ═══════════════════════════════════════════════════════════════════════════
    // Plugins
    // ═══════════════════════════════════════════════════════════════════════════
    RunPlugin(usize),
    OpenPluginDirectory,

    // ═══════════════════════════════════════════════════════════════════════════
    // Help
    // ═══════════════════════════════════════════════════════════════════════════
    OpenDiscussions,
    ReportBug,
    OpenLogFile,
    ShowAbout,

    // ═══════════════════════════════════════════════════════════════════════════
    // Mode switching
    // ═══════════════════════════════════════════════════════════════════════════
    SwitchMode(EditMode),

    // ANSI Editor messages
    AnsiEditor(AnsiEditorMessage),

    // BitFont Editor messages
    BitFontEditor(BitFontEditorMessage),

    // Animation Editor messages
    AnimationEditor(AnimationEditorMessage),

    // Font Size Dialog (used by BitFont Editor)
    FontSizeDialog(super::bitfont_editor::FontSizeDialogMessage),
    FontSizeApply(i32, i32),

    // Font Import Dialog
    ShowImportFontDialog,
    FontImport(super::font_import::FontImportMessage),
    FontImported(icy_engine::BitFont),

    // Font Export Dialog
    ShowExportFontDialog,
    FontExport(super::font_export::FontExportMessage),
    FontExported,

    // Animation Export Dialog
    ShowAnimationExportDialog,
    AnimationExport(super::animation_editor::AnimationExportMessage),

    // Internal
    Tick,
    ViewportTick,
    AnimationTick,
}

/// Status bar information that can be provided by any editor mode
#[derive(Clone, Debug, Default)]
pub struct StatusBarInfo {
    pub left: String,
    pub center: String,
    pub right: String,
}

impl From<AnsiStatusInfo> for StatusBarInfo {
    fn from(info: AnsiStatusInfo) -> Self {
        Self {
            left: format!(
                "({}, {})  {}×{}",
                info.cursor_position.0, info.cursor_position.1, info.buffer_size.0, info.buffer_size.1,
            ),
            center: format!("Layer {}/{}", info.current_layer + 1, info.total_layers,),
            right: format!("{}  {}", info.current_tool, if info.insert_mode { "INS" } else { "OVR" },),
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
        cmd::VIEW_ZOOM_IN => Message::ZoomIn,
        cmd::VIEW_ZOOM_OUT => Message::ZoomOut,
        cmd::VIEW_ZOOM_RESET => Message::ZoomReset,
    }
}

/// A single editing window
#[allow(dead_code)]
pub struct MainWindow {
    /// Window ID (1-based, for Alt+N switching)
    pub id: usize,

    /// Current editing mode and state
    mode_state: ModeState,

    /// Shared options
    options: Arc<Mutex<SharedOptions>>,

    /// Menu bar state (tracks expanded menus)
    menu_state: MenuBarState,

    /// Show left panel (tools, colors)
    show_left_panel: bool,

    /// Show right panel (layers, minimap)
    show_right_panel: bool,

    /// Dialog stack for modal dialogs
    dialogs: DialogStack<Message>,

    /// Command set for hotkey handling
    commands: CommandSet,

    /// Undo stack length at last save - for dirty tracking
    last_save: usize,

    /// Close the window after a successful save (for SaveAndClose flow)
    close_after_save: bool,

    /// Pending file to open after save (None inside = new file, Some(path) = open path)
    pending_open_path: Option<Option<PathBuf>>,
}

impl MainWindow {
    pub fn new(id: usize, path: Option<PathBuf>, options: Arc<Mutex<SharedOptions>>) -> Self {
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
                            (ModeState::Animation(AnimationEditor::new()), error)
                        }
                    }
                }
                _ => {
                    // Try as ANSI/ASCII art file (includes TDF and unknown formats)
                    match AnsiEditor::with_file(p.clone(), options.clone()) {
                        Ok(editor) => (ModeState::Ansi(editor), None),
                        Err(e) => {
                            let error = Some(("Error Loading File".to_string(), format!("Failed to load '{}': {}", p.display(), e)));
                            (ModeState::Ansi(AnsiEditor::new(options.clone())), error)
                        }
                    }
                }
            }
        } else {
            (ModeState::Ansi(AnsiEditor::new(options.clone())), None)
        };

        let last_save = mode_state.undo_stack_len();

        let mut dialogs = DialogStack::new();
        if let Some((title, message)) = initial_error {
            dialogs.push(error_dialog(title, message, |_| Message::CloseDialog));
        }

        Self {
            id,
            mode_state,
            options,
            menu_state: MenuBarState::new(),
            show_left_panel: true,
            show_right_panel: true,
            dialogs,
            commands: create_draw_commands(),
            last_save,
            close_after_save: false,
            pending_open_path: None,
        }
    }

    /// Create a MainWindow restored from a session
    ///
    /// This loads content from `load_path` but sets `original_path` as the file path.
    /// If `mark_dirty` is true, the window will be marked as modified.
    ///
    /// When `load_path` differs from `original_path`, it's an autosave file and we use
    /// `load_from_autosave` to load it (since autosave files have .autosave extension
    /// and can't be identified by extension).
    pub fn new_restored(id: usize, original_path: Option<PathBuf>, load_path: Option<PathBuf>, mark_dirty: bool, options: Arc<Mutex<SharedOptions>>) -> Self {
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
                            let error = Some(("Error Loading Animation Autosave".to_string(), e));
                            (ModeState::Animation(AnimationEditor::new()), error)
                        }
                    },
                    _ => {
                        // ANSI/other formats
                        match AnsiEditor::load_from_autosave(autosave, orig.clone(), options.clone()) {
                            Ok(editor) => (ModeState::Ansi(editor), None),
                            Err(e) => {
                                let error = Some(("Error Loading Autosave".to_string(), format!("{}", e)));
                                (ModeState::Ansi(AnsiEditor::new(options.clone())), error)
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
                    _ => match AnsiEditor::with_file(p.clone(), options.clone()) {
                        Ok(mut editor) => {
                            if let Some(ref orig) = original_path {
                                editor.set_file_path(orig.clone());
                            }
                            (ModeState::Ansi(editor), None)
                        }
                        Err(e) => {
                            let error = Some(("Error Loading File".to_string(), format!("Failed to load '{}': {}", p.display(), e)));
                            (ModeState::Ansi(AnsiEditor::new(options.clone())), error)
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
                    _ => match AnsiEditor::with_file(orig.clone(), options.clone()) {
                        Ok(editor) => (ModeState::Ansi(editor), None),
                        Err(e) => {
                            let error = Some(("Error Loading File".to_string(), format!("Failed to load '{}': {}", orig.display(), e)));
                            (ModeState::Ansi(AnsiEditor::new(options.clone())), error)
                        }
                    },
                }
            }

            // Case 4: No paths - create empty
            (None, None) => (ModeState::Ansi(AnsiEditor::new(options.clone())), None),
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

        Self {
            id,
            mode_state,
            options,
            menu_state: MenuBarState::new(),
            show_left_panel: true,
            show_right_panel: true,
            dialogs,
            commands: create_draw_commands(),
            last_save,
            close_after_save: false,
            pending_open_path: None,
        }
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
    }

    pub fn title(&self) -> String {
        let mode = self.mode_state.mode();
        let file_name = self
            .file_path()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| crate::fl!("unsaved-title"));

        let modified = if self.is_modified() { " •" } else { "" };

        format!("{}{} - iCY DRAW [{}]", file_name, modified, mode)
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    /// Get current edit mode
    pub fn mode(&self) -> EditMode {
        self.mode_state.mode()
    }

    /// Get current undo stack length (for autosave tracking)
    pub fn undo_stack_len(&self) -> usize {
        self.mode_state.undo_stack_len()
    }

    /// Get bytes for autosave
    pub fn get_autosave_bytes(&self) -> Result<Vec<u8>, String> {
        self.mode_state.get_autosave_bytes()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        // Route messages to dialogs first
        if let Some(task) = self.dialogs.update(&message) {
            return task;
        }

        match message {
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

                // Create new ANSI document
                self.mode_state = ModeState::Ansi(AnsiEditor::new(self.options.clone()));
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
            Message::OpenFile => {
                // Check for unsaved changes first
                if self.is_modified() {
                    let filename = self
                        .file_path()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Untitled".to_string());

                    // Store that we want to open a file (path TBD from dialog)
                    self.pending_open_path = Some(Some(PathBuf::new())); // Placeholder

                    self.dialogs.push(confirm_yes_no_cancel(
                        format!("Save changes to \"{}\"?", filename),
                        "Your changes will be lost if you don't save them.",
                        |result| match result {
                            DialogResult::Yes => Message::SaveFile,           // Will trigger file dialog after save
                            DialogResult::No => Message::ForceShowOpenDialog, // Show open dialog without dirty check
                            _ => Message::CloseDialog,
                        },
                    ));
                    Task::none()
                } else {
                    self.update(Message::ForceShowOpenDialog)
                }
            }
            Message::ForceShowOpenDialog => {
                // Close the confirmation dialog first (if any)
                self.dialogs.pop();

                // Show file picker without dirty check
                self.pending_open_path = None;
                let extensions: Vec<&str> = FileFormat::ALL
                    .iter()
                    .filter(|f| f.is_supported() || f.is_bitfont())
                    .flat_map(|f| f.all_extensions())
                    .copied()
                    .collect();

                Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .add_filter("Supported Files", &extensions)
                            .add_filter("All Files", &["*"])
                            .set_title("Open File")
                            .pick_file()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    |result| {
                        if let Some(path) = result {
                            Message::FileOpened(path)
                        } else {
                            Message::Tick // No file selected, do nothing
                        }
                    },
                )
            }
            Message::OpenRecentFile(path) => {
                // Check for unsaved changes first
                if self.is_modified() {
                    let filename = self
                        .file_path()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Untitled".to_string());

                    let open_path = path.clone();
                    self.dialogs.push(confirm_yes_no_cancel(
                        format!("Save changes to \"{}\"?", filename),
                        "Your changes will be lost if you don't save them.",
                        move |result| match result {
                            DialogResult::Yes => Message::SaveAndOpenFile(open_path.clone()),
                            DialogResult::No => Message::ForceOpenFile(open_path.clone()),
                            _ => Message::CloseDialog,
                        },
                    ));
                    Task::none()
                } else {
                    // No unsaved changes, open directly
                    self.update(Message::FileOpened(path))
                }
            }
            Message::SaveAndOpenFile(path) => {
                // Close the confirmation dialog first
                self.dialogs.pop();

                // Save first, then open the file
                if let Some(current_path) = self.file_path().cloned() {
                    match self.mode_state.save(&current_path) {
                        Ok(()) => {
                            self.mark_saved();
                            self.update(Message::FileOpened(path))
                        }
                        Err(e) => {
                            self.dialogs.push(error_dialog("Error Saving File", e, |_| Message::CloseDialog));
                            Task::none()
                        }
                    }
                } else {
                    // No path - need SaveAs, store pending open path
                    self.pending_open_path = Some(Some(path));
                    self.update(Message::SaveFileAs)
                }
            }
            Message::ForceOpenFile(path) => {
                // Close the confirmation dialog first (if any)
                self.dialogs.pop();

                // Open file without saving
                self.update(Message::FileOpened(path))
            }
            Message::FileOpened(path) => {
                // Determine file type using FileFormat
                let format = FileFormat::from_path(&path);

                match format {
                    Some(FileFormat::BitFont(_)) => {
                        // Open in BitFont editor
                        match BitFontEditor::from_file(path.clone()) {
                            Ok(editor) => {
                                self.mode_state = ModeState::BitFont(editor);
                                self.mark_saved();
                                self.options.lock().recent_files.add_recent_file(&path);
                            }
                            Err(e) => {
                                self.dialogs.push(error_dialog(
                                    "Error Loading Font",
                                    format!("Failed to load '{}': {}", path.display(), e),
                                    |_| Message::CloseDialog,
                                ));
                            }
                        }
                    }
                    Some(FileFormat::IcyAnim) => {
                        // Open in Animation editor
                        match AnimationEditor::load_file(path.clone()) {
                            Ok(editor) => {
                                self.mode_state = ModeState::Animation(editor);
                                self.mark_saved();
                                self.options.lock().recent_files.add_recent_file(&path);
                            }
                            Err(e) => {
                                self.dialogs.push(error_dialog(
                                    "Error Loading Animation",
                                    format!("Failed to load '{}': {}", path.display(), e),
                                    |_| Message::CloseDialog,
                                ));
                            }
                        }
                    }
                    _ => {
                        // Open in ANSI editor (default for all other formats)
                        match AnsiEditor::with_file(path.clone(), self.options.clone()) {
                            Ok(editor) => {
                                self.mode_state = ModeState::Ansi(editor);
                                self.mark_saved();
                                self.options.lock().recent_files.add_recent_file(&path);
                            }
                            Err(e) => {
                                self.dialogs.push(error_dialog(
                                    "Error Loading File",
                                    format!("Failed to load '{}': {}", path.display(), e),
                                    |_| Message::CloseDialog,
                                ));
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::FileLoadError(title, error) => {
                self.dialogs.push(error_dialog(title, error, |_| Message::CloseDialog));
                Task::none()
            }
            Message::CloseDialog => {
                // Close the topmost dialog
                self.dialogs.pop();
                Task::none()
            }
            Message::SaveFile => {
                // If we have a file path, save directly; otherwise show SaveAs dialog
                if let Some(path) = self.mode_state.file_path().cloned() {
                    match self.mode_state.save(&path) {
                        Ok(()) => {
                            self.mark_saved();

                            // Check if we should close after save
                            if self.close_after_save {
                                self.close_after_save = false;
                                self.pending_open_path = None;
                                return Task::done(Message::ForceCloseFile);
                            }

                            // Check if we have a pending file to open after save
                            if let Some(pending) = self.pending_open_path.take() {
                                return match pending {
                                    None => self.update(Message::ForceNewFile), // New file
                                    Some(open_path) if open_path.as_os_str().is_empty() => {
                                        // Empty path means show file picker
                                        self.update(Message::ForceShowOpenDialog)
                                    }
                                    Some(open_path) => self.update(Message::FileOpened(open_path)), // Open specific file
                                };
                            }
                        }
                        Err(e) => {
                            self.close_after_save = false;
                            self.pending_open_path = None;
                            self.dialogs.push(error_dialog("Error Saving File", e, |_| Message::CloseDialog));
                        }
                    }
                    Task::none()
                } else {
                    // No file path - trigger SaveAs
                    self.update(Message::SaveFileAs)
                }
            }
            Message::SaveFileAs => {
                // Show save dialog
                let default_ext = self.mode_state.default_extension();
                let mode = self.mode_state.mode();

                Task::perform(
                    async move {
                        let filter_name = match mode {
                            EditMode::Ansi => "ANSI Files",
                            EditMode::BitFont => "Font Files",
                            EditMode::CharFont => "TDF Files",
                            EditMode::Animation => "Animation Files",
                        };

                        rfd::AsyncFileDialog::new()
                            .add_filter(filter_name, &[default_ext])
                            .add_filter("All Files", &["*"])
                            .set_title("Save File As")
                            .save_file()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    |result| {
                        if let Some(path) = result {
                            Message::FileSaved(path)
                        } else {
                            Message::Tick // Cancelled
                        }
                    },
                )
            }
            Message::FileSaved(path) => {
                // Save to the selected path
                match self.mode_state.save(&path) {
                    Ok(()) => {
                        // Update file path and mark as saved
                        self.mode_state.set_file_path(path.clone());
                        self.mark_saved();
                        // Add to recent files
                        self.options.lock().recent_files.add_recent_file(&path);

                        // Check if we should close after save
                        if self.close_after_save {
                            self.close_after_save = false;
                            self.pending_open_path = None;
                            return Task::done(Message::ForceCloseFile);
                        }

                        // Check if we have a pending file to open after save
                        if let Some(pending) = self.pending_open_path.take() {
                            return match pending {
                                None => self.update(Message::ForceNewFile), // New file
                                Some(open_path) if open_path.as_os_str().is_empty() => {
                                    // Empty path means show file picker
                                    self.update(Message::OpenFile)
                                }
                                Some(open_path) => self.update(Message::FileOpened(open_path)), // Open specific file
                            };
                        }
                    }
                    Err(e) => {
                        self.close_after_save = false; // Reset flags on error
                        self.pending_open_path = None;
                        self.dialogs.push(error_dialog("Error Saving File", e, |_| Message::CloseDialog));
                    }
                }
                Task::none()
            }
            Message::CloseFile => {
                // Check if document has unsaved changes
                if self.is_modified() {
                    // Show save confirmation dialog
                    let filename = self
                        .file_path()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Untitled".to_string());

                    self.dialogs.push(confirm_yes_no_cancel(
                        format!("Save changes to \"{}\"?", filename),
                        "Your changes will be lost if you don't save them.",
                        |result| match result {
                            DialogResult::Yes => Message::SaveAndCloseFile,
                            DialogResult::No => Message::ForceCloseFile,
                            _ => Message::CloseDialog, // Cancel - just close dialog
                        },
                    ));
                    Task::none()
                } else {
                    // No unsaved changes, close directly
                    Task::done(Message::ForceCloseFile)
                }
            }
            Message::SaveAndCloseFile => {
                // Close the confirmation dialog first
                self.dialogs.pop();

                // Save first, then close
                if let Some(path) = self.file_path().cloned() {
                    // Has a path - save directly then close
                    match self.mode_state.save(&path) {
                        Ok(()) => {
                            self.mark_saved();
                            Task::done(Message::ForceCloseFile)
                        }
                        Err(e) => {
                            self.dialogs.push(error_dialog("Error Saving File", e, |_| Message::CloseDialog));
                            Task::none()
                        }
                    }
                } else {
                    // No path - need SaveAs dialog, then close after
                    // We'll set a flag to close after save
                    self.close_after_save = true;
                    self.update(Message::SaveFileAs)
                }
            }
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
                        // Access EditState through the screen
                        let mut screen = editor.screen.lock();
                        if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
                            if let Err(e) = edit_state.undo() {
                                log::error!("Undo failed: {}", e);
                            }
                        }
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
                        // Access EditState through the screen
                        let mut screen = editor.screen.lock();
                        if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
                            if let Err(e) = edit_state.redo() {
                                log::error!("Redo failed: {}", e);
                            }
                        }
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
                    ModeState::Ansi(_) => {
                        // TODO: Implement cut for ANSI
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
                    ModeState::Ansi(_) => {
                        // TODO: Implement copy for ANSI
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
                    ModeState::Ansi(_) => {
                        // TODO: Implement paste for ANSI
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
                // TODO: Implement select all
                Task::none()
            }
            Message::ZoomIn => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    editor.canvas.set_zoom(editor.canvas.zoom + 0.25);
                }
                Task::none()
            }
            Message::ZoomOut => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    editor.canvas.set_zoom(editor.canvas.zoom - 0.25);
                }
                Task::none()
            }
            Message::ZoomReset => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    editor.canvas.set_zoom(1.0);
                }
                Task::none()
            }
            Message::ToggleRightPanel => {
                self.show_right_panel = !self.show_right_panel;
                Task::none()
            }
            Message::SwitchMode(mode) => {
                self.mode_state = match mode {
                    EditMode::Ansi => ModeState::Ansi(AnsiEditor::new(self.options.clone())),
                    EditMode::BitFont => ModeState::BitFont(BitFontEditor::new()),
                    EditMode::CharFont => ModeState::CharFont(CharFontEditorState::new()),
                    EditMode::Animation => ModeState::Animation(AnimationEditor::new()),
                };
                Task::none()
            }
            Message::BitFontEditor(msg) => {
                // Intercept ShowFontSizeDialog to push onto dialog stack
                if matches!(msg, BitFontEditorMessage::ShowFontSizeDialog) {
                    if let ModeState::BitFont(editor) = &self.mode_state {
                        let (width, height) = editor.font_size();
                        self.dialogs.push(super::bitfont_editor::FontSizeDialog::new(width, height));
                    }
                    return Task::none();
                }

                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    editor.update(msg).map(Message::BitFontEditor)
                } else {
                    Task::none()
                }
            }
            // Font Size Dialog messages are routed through DialogStack::update above
            Message::FontSizeDialog(_) => Task::none(),
            Message::FontSizeApply(width, height) => {
                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    let _ = editor.resize_font(width, height);
                    editor.invalidate_caches();
                }
                Task::none()
            }
            Message::AnsiEditor(msg) => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    editor.update(msg).map(Message::AnsiEditor)
                } else {
                    Task::none()
                }
            }
            Message::AnimationEditor(msg) => {
                if let ModeState::Animation(editor) = &mut self.mode_state {
                    editor.update(msg).map(Message::AnimationEditor)
                } else {
                    Task::none()
                }
            }
            Message::Tick => Task::none(),
            Message::ViewportTick => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    editor.update(AnsiEditorMessage::ViewportTick).map(Message::AnsiEditor)
                } else {
                    Task::none()
                }
            }
            Message::AnimationTick => match &mut self.mode_state {
                ModeState::Ansi(editor) => {
                    let delta = 0.016;

                    let color_task = editor
                        .update(AnsiEditorMessage::ColorSwitcher(crate::ui::ansi_editor::ColorSwitcherMessage::Tick(delta)))
                        .map(Message::AnsiEditor);

                    let tool_task = editor
                        .update(AnsiEditorMessage::ToolPanel(crate::ui::ansi_editor::ToolPanelMessage::Tick(delta)))
                        .map(Message::AnsiEditor);

                    Task::batch([color_task, tool_task])
                }
                ModeState::BitFont(editor) => {
                    let delta = 0.016;
                    editor
                        .update(BitFontEditorMessage::TopToolbar(BitFontTopToolbarMessage::ColorSwitcher(
                            crate::ui::ansi_editor::ColorSwitcherMessage::Tick(delta),
                        )))
                        .map(Message::BitFontEditor)
                }
                ModeState::CharFont(_) => Task::none(),
                ModeState::Animation(editor) => editor.update(AnimationEditorMessage::Tick).map(Message::AnimationEditor),
            },

            // ═══════════════════════════════════════════════════════════════════
            // Font Import Dialog
            // ═══════════════════════════════════════════════════════════════════
            Message::ShowImportFontDialog => {
                self.dialogs.push(super::font_import::FontImportDialog::new());
                Task::none()
            }
            // FontImport messages are routed through DialogStack::update above
            Message::FontImport(_) => Task::none(),
            Message::FontImported(font) => {
                // Switch to BitFont editor with the imported font
                let mut editor = BitFontEditor::new();
                editor.state = icy_engine_edit::bitfont::BitFontEditState::from_font(font);
                editor.invalidate_caches();
                self.mode_state = ModeState::BitFont(editor);
                self.mark_saved();
                Task::none()
            }

            // ═══════════════════════════════════════════════════════════════════
            // Font Export Dialog
            // ═══════════════════════════════════════════════════════════════════
            Message::ShowExportFontDialog => {
                if let ModeState::BitFont(editor) = &self.mode_state {
                    let font = editor.state.build_font();
                    self.dialogs.push(super::font_export::FontExportDialog::new(font));
                }
                Task::none()
            }
            // FontExport messages are routed through DialogStack::update above
            Message::FontExport(_) => Task::none(),
            Message::FontExported => {
                // Font was successfully exported - nothing special to do
                Task::none()
            }

            // ═══════════════════════════════════════════════════════════════════
            // Animation Export Dialog
            // ═══════════════════════════════════════════════════════════════════
            Message::ShowAnimationExportDialog => {
                if let ModeState::Animation(editor) = &self.mode_state {
                    let animator = editor.animator.clone();
                    let source_path = editor.file_path().cloned();
                    self.dialogs
                        .push(super::animation_editor::AnimationExportDialog::new(animator, source_path.as_ref()));
                }
                Task::none()
            }
            // AnimationExport messages are routed through DialogStack::update above
            Message::AnimationExport(_) => Task::none(),

            // ═══════════════════════════════════════════════════════════════════
            // File operations (TODO: implement)
            // ═══════════════════════════════════════════════════════════════════
            Message::ClearRecentFiles => {
                self.options.lock().recent_files.clear_recent_files();
                Task::none()
            }
            Message::ExportFile => Task::none(),
            Message::ShowSettings => Task::none(),

            // ═══════════════════════════════════════════════════════════════════
            // Edit operations (TODO: implement)
            // ═══════════════════════════════════════════════════════════════════
            Message::PasteAsNewImage => Task::none(),
            Message::PasteAsBrush => Task::none(),

            // ═══════════════════════════════════════════════════════════════════
            // Selection operations (TODO: implement)
            // ═══════════════════════════════════════════════════════════════════
            Message::Deselect => Task::none(),
            Message::InverseSelection => Task::none(),
            Message::DeleteSelection => Task::none(),

            // ═══════════════════════════════════════════════════════════════════
            // Area operations (TODO: implement)
            // ═══════════════════════════════════════════════════════════════════
            Message::JustifyLineCenter => Task::none(),
            Message::JustifyLineLeft => Task::none(),
            Message::JustifyLineRight => Task::none(),
            Message::InsertRow => Task::none(),
            Message::DeleteRow => Task::none(),
            Message::InsertColumn => Task::none(),
            Message::DeleteColumn => Task::none(),
            Message::EraseRow => Task::none(),
            Message::EraseRowToStart => Task::none(),
            Message::EraseRowToEnd => Task::none(),
            Message::EraseColumn => Task::none(),
            Message::EraseColumnToStart => Task::none(),
            Message::EraseColumnToEnd => Task::none(),
            Message::ScrollAreaUp => Task::none(),
            Message::ScrollAreaDown => Task::none(),
            Message::ScrollAreaLeft => Task::none(),
            Message::ScrollAreaRight => Task::none(),

            // ═══════════════════════════════════════════════════════════════════
            // Transform operations (TODO: implement)
            // ═══════════════════════════════════════════════════════════════════
            Message::FlipX => Task::none(),
            Message::FlipY => Task::none(),
            Message::Crop => Task::none(),
            Message::JustifyCenter => Task::none(),
            Message::JustifyLeft => Task::none(),
            Message::JustifyRight => Task::none(),

            // ═══════════════════════════════════════════════════════════════════
            // Document settings (TODO: implement)
            // ═══════════════════════════════════════════════════════════════════
            Message::ToggleMirrorMode => Task::none(),
            Message::EditSauce => Task::none(),
            Message::ToggleLGAFont => Task::none(),
            Message::ToggleAspectRatio => Task::none(),
            Message::SetCanvasSize => Task::none(),

            // ═══════════════════════════════════════════════════════════════════
            // Color operations (TODO: implement)
            // ═══════════════════════════════════════════════════════════════════
            Message::SwitchIceMode(_mode) => Task::none(),
            Message::SwitchPaletteMode(_mode) => Task::none(),
            Message::SelectPalette => Task::none(),
            Message::OpenPalettesDirectory => Task::none(),
            Message::NextFgColor => Task::none(),
            Message::PrevFgColor => Task::none(),
            Message::NextBgColor => Task::none(),
            Message::PrevBgColor => Task::none(),
            Message::PickAttributeUnderCaret => Task::none(),
            Message::ToggleColor => Task::none(),
            Message::SwitchToDefaultColor => Task::none(),

            // ═══════════════════════════════════════════════════════════════════
            // Font operations (TODO: implement)
            // ═══════════════════════════════════════════════════════════════════
            Message::SwitchFontMode(_mode) => Task::none(),
            Message::OpenFontSelector => Task::none(),
            Message::AddFonts => Task::none(),
            Message::OpenFontManager => Task::none(),
            Message::OpenFontDirectory => Task::none(),

            // ═══════════════════════════════════════════════════════════════════
            // View operations (TODO: implement for some)
            // ═══════════════════════════════════════════════════════════════════
            Message::SetZoom(zoom) => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    editor.canvas.set_zoom(zoom);
                }
                Task::none()
            }
            Message::SetGuide(_x, _y) => Task::none(),
            Message::SetRaster(_x, _y) => Task::none(),
            Message::ToggleGuides => Task::none(),
            Message::ToggleRaster => Task::none(),
            Message::ToggleLayerBorders => Task::none(),
            Message::ToggleLineNumbers => Task::none(),
            Message::ToggleLeftPanel => {
                self.show_left_panel = !self.show_left_panel;
                Task::none()
            }
            Message::ToggleFullscreen => Task::none(),
            Message::SetReferenceImage => Task::none(),
            Message::ToggleReferenceImage => Task::none(),
            Message::ClearReferenceImage => Task::none(),

            // ═══════════════════════════════════════════════════════════════════
            // Plugin operations (TODO: implement)
            // ═══════════════════════════════════════════════════════════════════
            Message::RunPlugin(_id) => Task::none(),
            Message::OpenPluginDirectory => Task::none(),

            // ═══════════════════════════════════════════════════════════════════
            // Help operations (TODO: implement)
            // ═══════════════════════════════════════════════════════════════════
            Message::OpenDiscussions => Task::none(),
            Message::ReportBug => Task::none(),
            Message::OpenLogFile => Task::none(),
            Message::ShowAbout => Task::none(),

            // ═══════════════════════════════════════════════════════════════════
            // Other view operations
            // ═══════════════════════════════════════════════════════════════════
            Message::ToggleFitWidth => Task::none(),
        }
    }

    /// Check if this window needs animation updates
    pub fn needs_animation(&self) -> bool {
        match &self.mode_state {
            ModeState::Ansi(editor) => editor.needs_animation(),
            ModeState::BitFont(editor) => editor.needs_animation(),
            ModeState::CharFont(_) => false,
            ModeState::Animation(editor) => editor.needs_animation(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        // Build the UI based on current mode
        let recent_files = &self.options.lock().recent_files;

        // Get undo/redo descriptions for menu
        let undo_info = self.get_undo_info();
        let menu_bar = self.menu_state.view(&self.mode_state.mode(), recent_files, &undo_info);

        let content: Element<'_, Message> = match &self.mode_state {
            ModeState::Ansi(editor) => self.view_ansi_editor(editor),
            ModeState::BitFont(editor) => self.view_bitfont_editor(editor),
            ModeState::CharFont(_state) => self.view_charfont_editor(),
            ModeState::Animation(editor) => self.view_animation_editor(editor),
        };

        // Status bar
        let status_bar = self.view_status_bar();

        let main_content: Element<'_, Message> = column![menu_bar, content, rule::horizontal(1), status_bar,].into();

        // Show dialogs from dialog stack
        self.dialogs.view(main_content)
    }

    fn view_ansi_editor<'a>(&'a self, editor: &'a AnsiEditor) -> Element<'a, Message> {
        editor.view().map(Message::AnsiEditor)
    }

    fn view_bitfont_editor<'a>(&'a self, editor: &'a BitFontEditor) -> Element<'a, Message> {
        editor.view().map(Message::BitFontEditor)
    }

    fn view_charfont_editor(&self) -> Element<'_, Message> {
        container(text("CharFont (TDF) Editor - Coming Soon").size(24))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    fn view_animation_editor<'a>(&'a self, editor: &'a AnimationEditor) -> Element<'a, Message> {
        editor.view().map(Message::AnimationEditor)
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
        let info = self.get_status_info();

        container(
            row![
                // Left section
                container(text(info.left).size(12)).width(Length::FillPortion(1)),
                // Center section
                container(text(info.center).size(12)).width(Length::FillPortion(1)).center_x(Length::Fill),
                // Right section
                container(text(info.right).size(12)).width(Length::FillPortion(1)),
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
                StatusBarInfo { left, center, right }
            }
            ModeState::CharFont(_) => StatusBarInfo {
                left: "CharFont Editor".into(),
                center: String::new(),
                right: String::new(),
            },
            ModeState::Animation(editor) => {
                let (line, col) = editor.cursor_position();
                StatusBarInfo {
                    left: format!("Ln {}, Col {}", line + 1, col + 1),
                    center: String::new(),
                    right: if editor.is_dirty() { "Modified".into() } else { String::new() },
                }
            }
        }
    }

    /// Get undo/redo descriptions for menu display
    fn get_undo_info(&self) -> UndoInfo {
        match &self.mode_state {
            ModeState::Ansi(editor) => {
                let mut screen = editor.screen.lock();
                if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
                    UndoInfo::new(edit_state.undo_description(), edit_state.redo_description())
                } else {
                    UndoInfo::default()
                }
            }
            ModeState::BitFont(editor) => UndoInfo::new(editor.undo_description(), editor.redo_description()),
            ModeState::CharFont(_) => UndoInfo::default(),
            ModeState::Animation(editor) => UndoInfo::new(editor.undo_description(), editor.redo_description()),
        }
    }

    /// Handle events passed from the window manager
    pub fn handle_event(&mut self, event: &Event) -> (Option<Message>, Task<Message>) {
        // If dialogs are open, route events there first
        if !self.dialogs.is_empty() {
            let task = self.dialogs.handle_event(event);
            // Dialogs consume all events when open
            return (None, task);
        }

        // Try to match hotkeys via command system (global commands)
        if let Some(hotkey) = event.into_hotkey() {
            if let Some(cmd_id) = self.commands.match_hotkey(&hotkey) {
                if let Some(msg) = handle_main_window_command(cmd_id) {
                    return (Some(msg), Task::none());
                }
            }
        }

        // Handle mode-specific menu commands
        match &self.mode_state {
            ModeState::BitFont(editor) => {
                // Check BitFont menu commands
                let undo_desc = editor.undo_description();
                let redo_desc = editor.redo_description();
                if let Some(msg) = super::bitfont_editor::menu_bar::handle_command_event(event, undo_desc.as_deref(), redo_desc.as_deref()) {
                    return (Some(msg), Task::none());
                }
            }
            ModeState::Animation(editor) => {
                // Check Animation menu commands
                let undo_desc = editor.undo_description();
                let redo_desc = editor.redo_description();
                if let Some(msg) = super::animation_editor::menu_bar::handle_command_event(event, undo_desc.as_deref(), redo_desc.as_deref()) {
                    return (Some(msg), Task::none());
                }
            }
            _ => {}
        }

        // Handle editor-specific events (tools, navigation, etc.)
        match &mut self.mode_state {
            ModeState::Ansi(_editor) => {
                // TODO: Handle ANSI editor events (keyboard, etc.)
            }
            ModeState::BitFont(state) => {
                if let Some(msg) = state.handle_event(event) {
                    return (Some(Message::BitFontEditor(msg)), Task::none());
                }
            }
            ModeState::CharFont(_state) => {}
            ModeState::Animation(_state) => {}
        }

        (None, Task::none())
    }
}

// Implement the Window trait for use with shared WindowManager helpers
impl icy_engine_gui::Window for MainWindow {
    type Message = Message;

    fn id(&self) -> usize {
        self.id
    }

    fn title(&self) -> &str {
        // Return a static reference - we need to store the title
        // For now, just return a placeholder since title() generates dynamically
        // The WindowManager will use format_window_title which calls title() separately
        ""
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

    fn needs_animation(&self) -> bool {
        self.needs_animation()
    }
}
