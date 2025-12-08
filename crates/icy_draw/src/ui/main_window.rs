//! MainWindow for icy_draw
//!
//! Each MainWindow represents one editing window with its own state and mode.
//! The mode determines what kind of editor is shown (ANSI, BitFont, CharFont).

use std::{path::PathBuf, sync::Arc};

use parking_lot::Mutex;

use iced::{
    Element, Event, Length, Task, Theme,
    widget::{column, container, row, rule, text},
};
use icy_engine::formats::{BitFontFormat, FileFormat};
use icy_engine_edit::{EditState, UndoState};
use icy_engine_gui::commands::{CommandSet, IntoHotkey, cmd};
use icy_engine_gui::command_handlers;
use icy_engine_gui::ui::{ButtonSet, ConfirmationDialog, DialogType};

use super::ansi_editor::{AnsiEditor, AnsiEditorMessage, AnsiStatusInfo};
use super::bitfont_editor::{BitFontEditor, BitFontEditorMessage};
use super::commands::create_draw_commands;
use super::{SharedOptions, menu::{MenuBarState, UndoInfo}};

/// The editing mode of a window
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditMode {
    /// ANSI/ASCII art editor - the main mode
    Ansi,
    /// BitFont editor for editing bitmap fonts
    BitFont,
    /// CharFont editor for editing TDF character fonts
    CharFont,
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
}

impl ModeState {
    pub fn mode(&self) -> EditMode {
        match self {
            Self::Ansi(_) => EditMode::Ansi,
            Self::BitFont(_) => EditMode::BitFont,
            Self::CharFont(_) => EditMode::CharFont,
        }
    }

    /// Check if the current document is modified
    pub fn is_modified(&self) -> bool {
        match self {
            Self::Ansi(editor) => editor.is_modified,
            Self::BitFont(editor) => editor.is_modified,
            Self::CharFont(_) => false,
        }
    }

    /// Get the file path if any
    pub fn file_path(&self) -> Option<&PathBuf> {
        match self {
            Self::Ansi(editor) => editor.file_path.as_ref(),
            Self::BitFont(editor) => editor.file_path.as_ref(),
            Self::CharFont(_) => None,
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
    ExportFile,
    CloseFile,
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

    // BitFont Editor menu actions (wrappers for convenience)
    BitFontClearGlyph,
    BitFontInverseGlyph,
    BitFontFlipX,
    BitFontFlipY,
    BitFontSelectAll,
    BitFontClearSelection,
    BitFontFillSelection,
    BitFontNextChar,
    BitFontPrevChar,
    // Tool selection
    BitFontSelectToolClick,
    BitFontSelectToolSelect,
    BitFontSelectToolRect,
    BitFontSelectToolFill,

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

    /// Error dialog state (title, message) - None if no dialog
    error_dialog: Option<(String, String)>,

    /// Command set for hotkey handling
    commands: CommandSet,
}

impl MainWindow {
    pub fn new(id: usize, path: Option<PathBuf>, options: Arc<Mutex<SharedOptions>>) -> Self {
        let (mode_state, error_dialog) = if let Some(ref p) = path {
            // Determine mode based on file extension
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");

            if BitFontFormat::is_bitfont_extension(ext) {
                // BitFont format detected (yaff, psf, fXX)
                match BitFontEditor::from_file(p.clone()) {
                    Ok(editor) => (ModeState::BitFont(editor), None),
                    Err(e) => {
                        let error = Some(("Error Loading Font".to_string(), e));
                        (ModeState::BitFont(BitFontEditor::new()), error)
                    }
                }
            } else if ext.eq_ignore_ascii_case("tdf") {
                // TDF CharFont format
                (ModeState::CharFont(CharFontEditorState::new()), None)
            } else {
                // Try as ANSI/ASCII art file
                match AnsiEditor::with_file(p.clone(), options.clone()) {
                    Ok(editor) => (ModeState::Ansi(editor), None),
                    Err(e) => {
                        let error = Some(("Error Loading File".to_string(), format!("Failed to load '{}': {}", p.display(), e)));
                        (ModeState::Ansi(AnsiEditor::new(options.clone())), error)
                    }
                }
            }
        } else {
            (ModeState::Ansi(AnsiEditor::new(options.clone())), None)
        };

        Self {
            id,
            mode_state,
            options,
            menu_state: MenuBarState::new(),
            show_left_panel: true,
            show_right_panel: true,
            error_dialog,
            commands: create_draw_commands(),
        }
    }

    /// Get the current file path
    pub fn file_path(&self) -> Option<&PathBuf> {
        self.mode_state.file_path()
    }

    /// Check if the document is modified
    pub fn is_modified(&self) -> bool {
        self.mode_state.is_modified()
    }

    pub fn title(&self) -> String {
        let mode = self.mode_state.mode();
        let file_name = self.file_path().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("Untitled");

        let modified = if self.is_modified() { " •" } else { "" };

        format!("{}{} - iCY DRAW [{}]", file_name, modified, mode)
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NewFile => {
                // Create new ANSI document
                self.mode_state = ModeState::Ansi(AnsiEditor::new(self.options.clone()));
                Task::none()
            }
            Message::OpenFile => {
                // Build filter from supported file formats
                let extensions: Vec<&str> = FileFormat::ALL
                    .iter()
                    .filter(|f| f.is_supported())
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
            Message::FileOpened(path) => {
                // Determine file type based on extension and open appropriate editor
                let ext = path.extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                
                // Check if it's a bitmap font file
                let is_font_file = ext == "psf" || ext == "yaff" || 
                    (ext.starts_with('f') && ext.len() == 3 && ext[1..].chars().all(|c| c.is_ascii_digit()));
                
                if is_font_file {
                    // Open in BitFont editor
                    match BitFontEditor::from_file(path.clone()) {
                        Ok(editor) => {
                            self.mode_state = ModeState::BitFont(editor);
                            // Add to recent files
                            self.options.lock().recent_files.add_recent_file(&path);
                        }
                        Err(e) => {
                            self.error_dialog = Some(("Error Loading File".to_string(), format!("Failed to load '{}': {}", path.display(), e)));
                        }
                    }
                } else {
                    // Open in ANSI editor
                    match AnsiEditor::with_file(path.clone(), self.options.clone()) {
                        Ok(editor) => {
                            self.mode_state = ModeState::Ansi(editor);
                            // Add to recent files
                            self.options.lock().recent_files.add_recent_file(&path);
                        }
                        Err(e) => {
                            self.error_dialog = Some(("Error Loading File".to_string(), format!("Failed to load '{}': {}", path.display(), e)));
                        }
                    }
                }
                Task::none()
            }
            Message::FileLoadError(title, error) => {
                self.error_dialog = Some((title, error));
                Task::none()
            }
            Message::CloseDialog => {
                self.error_dialog = None;
                Task::none()
            }
            Message::SaveFile => {
                // TODO: Implement save
                Task::none()
            }
            Message::SaveFileAs => {
                // TODO: Implement save as
                Task::none()
            }
            Message::CloseFile => {
                // TODO: Implement close (with save prompt if modified)
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
                }
            }
            Message::Cut => {
                // TODO: Implement cut
                Task::none()
            }
            Message::Copy => {
                // TODO: Implement copy
                Task::none()
            }
            Message::Paste => {
                // TODO: Implement paste
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
                };
                Task::none()
            }
            Message::BitFontEditor(msg) => {
                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    editor.update(msg).map(Message::BitFontEditor)
                } else {
                    Task::none()
                }
            }
            // BitFont menu actions (wrappers)
            Message::BitFontClearGlyph => {
                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    editor.update(BitFontEditorMessage::Clear).map(Message::BitFontEditor)
                } else {
                    Task::none()
                }
            }
            Message::BitFontInverseGlyph => {
                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    editor.update(BitFontEditorMessage::Inverse).map(Message::BitFontEditor)
                } else {
                    Task::none()
                }
            }
            Message::BitFontFlipX => {
                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    editor.update(BitFontEditorMessage::FlipX).map(Message::BitFontEditor)
                } else {
                    Task::none()
                }
            }
            Message::BitFontFlipY => {
                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    editor.update(BitFontEditorMessage::FlipY).map(Message::BitFontEditor)
                } else {
                    Task::none()
                }
            }
            Message::BitFontSelectAll => {
                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    editor.update(BitFontEditorMessage::SelectAll).map(Message::BitFontEditor)
                } else {
                    Task::none()
                }
            }
            Message::BitFontClearSelection => {
                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    editor.update(BitFontEditorMessage::ClearSelection).map(Message::BitFontEditor)
                } else {
                    Task::none()
                }
            }
            Message::BitFontFillSelection => {
                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    editor.update(BitFontEditorMessage::FillSelection).map(Message::BitFontEditor)
                } else {
                    Task::none()
                }
            }
            Message::BitFontNextChar => {
                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    editor.update(BitFontEditorMessage::NextChar).map(Message::BitFontEditor)
                } else {
                    Task::none()
                }
            }
            Message::BitFontPrevChar => {
                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    editor.update(BitFontEditorMessage::PrevChar).map(Message::BitFontEditor)
                } else {
                    Task::none()
                }
            }
            Message::BitFontSelectToolClick => {
                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    editor.update(BitFontEditorMessage::SelectTool(crate::ui::bitfont_editor::BitFontTool::Click)).map(Message::BitFontEditor)
                } else {
                    Task::none()
                }
            }
            Message::BitFontSelectToolSelect => {
                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    editor.update(BitFontEditorMessage::SelectTool(crate::ui::bitfont_editor::BitFontTool::Select)).map(Message::BitFontEditor)
                } else {
                    Task::none()
                }
            }
            Message::BitFontSelectToolRect => {
                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    editor.update(BitFontEditorMessage::SelectTool(crate::ui::bitfont_editor::BitFontTool::RectangleOutline)).map(Message::BitFontEditor)
                } else {
                    Task::none()
                }
            }
            Message::BitFontSelectToolFill => {
                if let ModeState::BitFont(editor) = &mut self.mode_state {
                    editor.update(BitFontEditorMessage::SelectTool(crate::ui::bitfont_editor::BitFontTool::Fill)).map(Message::BitFontEditor)
                } else {
                    Task::none()
                }
            }
            Message::AnsiEditor(msg) => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    editor.update(msg).map(Message::AnsiEditor)
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
            Message::AnimationTick => {
                if let ModeState::Ansi(editor) = &mut self.mode_state {
                    // Send tick with delta time (16ms at 60fps)
                    let delta = 0.016;

                    // Update color switcher
                    let color_task = editor
                        .update(AnsiEditorMessage::ColorSwitcher(crate::ui::ansi_editor::ColorSwitcherMessage::Tick(delta)))
                        .map(Message::AnsiEditor);

                    // Update tool panel
                    let tool_task = editor
                        .update(AnsiEditorMessage::ToolPanel(crate::ui::ansi_editor::ToolPanelMessage::Tick(delta)))
                        .map(Message::AnsiEditor);

                    Task::batch([color_task, tool_task])
                } else {
                    Task::none()
                }
            }

            // ═══════════════════════════════════════════════════════════════════
            // File operations (TODO: implement)
            // ═══════════════════════════════════════════════════════════════════
            Message::OpenRecentFile(path) => {
                // Re-use FileOpened logic
                return self.update(Message::FileOpened(path));
            }
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
            _ => false,
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
        };

        // Status bar
        let status_bar = self.view_status_bar();

        let main_content: Element<'_, Message> = column![menu_bar, content, rule::horizontal(1), status_bar,].into();

        // Show error dialog if present
        if let Some((title, message)) = &self.error_dialog {
            let dialog = ConfirmationDialog::new(title, message).dialog_type(DialogType::Error).buttons(ButtonSet::Close);

            dialog.view(main_content, |_result| Message::CloseDialog)
        } else {
            main_content
        }
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

    fn view_status_bar(&self) -> Element<'_, Message> {
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
            ModeState::BitFont(editor) => {
                UndoInfo::new(editor.undo_description(), editor.redo_description())
            }
            ModeState::CharFont(_) => UndoInfo::default(),
        }
    }

    /// Handle events passed from the window manager
    pub fn handle_event(&mut self, event: &Event) -> Option<Message> {
        // Try to match hotkeys via command system
        if let Some(hotkey) = event.into_hotkey() {
            if let Some(cmd_id) = self.commands.match_hotkey(&hotkey) {
                if let Some(msg) = handle_main_window_command(cmd_id) {
                    return Some(msg);
                }
            }
        }
        match &mut self.mode_state {
            ModeState::Ansi(_editor) => {
                // TODO: Handle ANSI editor events (keyboard, etc.)
            }
            ModeState::BitFont(_state) => {
                // TODO: Handle BitFont editor events
            }
            ModeState::CharFont(_state) => {
            }
        }

        None
    }
}
