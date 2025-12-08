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
use icy_engine::formats::FileFormat;
use icy_engine_gui::ui::{ButtonSet, ConfirmationDialog, DialogType};

use super::ansi_editor::{AnsiEditor, AnsiEditorMessage, AnsiStatusInfo};
use super::{SharedOptions, menu::MenuBarState};

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

/// State for the BitFont editor mode
pub struct BitFontEditorState {
    // TODO: Add BitFont editor state
    // - font data
    // - selected glyph
    // - edit grid
}

impl BitFontEditorState {
    pub fn new() -> Self {
        Self {}
    }
}

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
            Self::BitFont(_) => false,
            Self::CharFont(_) => false,
        }
    }

    /// Get the file path if any
    pub fn file_path(&self) -> Option<&PathBuf> {
        match self {
            Self::Ansi(editor) => editor.file_path.as_ref(),
            Self::BitFont(_) => None,
            Self::CharFont(_) => None,
        }
    }
}

/// Message type for MainWindow
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum Message {
    // File operations
    NewFile,
    OpenFile,
    FileOpened(PathBuf),
    FileLoadError(String, String), // (title, error_message)
    SaveFile,
    SaveFileAs,
    CloseFile,

    // Dialog
    CloseDialog,

    // Edit operations
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,

    // View operations
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ToggleRightPanel,

    // Mode switching
    SwitchMode(EditMode),

    // ANSI Editor messages
    AnsiEditor(AnsiEditorMessage),

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

    /// Show right panel (layers, minimap)
    show_right_panel: bool,

    /// Error dialog state (title, message) - None if no dialog
    error_dialog: Option<(String, String)>,
}

impl MainWindow {
    pub fn new(id: usize, path: Option<PathBuf>, options: Arc<Mutex<SharedOptions>>) -> Self {
        let (mode_state, error_dialog) = if let Some(ref p) = path {
            // Determine mode based on file extension
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            match ext.to_lowercase().as_str() {
                "psf" | "f16" | "f14" | "f08" => (ModeState::BitFont(BitFontEditorState::new()), None),
                "tdf" => (ModeState::CharFont(CharFontEditorState::new()), None),
                _ => match AnsiEditor::with_file(p.clone(), options.clone()) {
                    Ok(editor) => (ModeState::Ansi(editor), None),
                    Err(e) => {
                        let error = Some(("Error Loading File".to_string(), format!("Failed to load '{}': {}", p.display(), e)));
                        (ModeState::Ansi(AnsiEditor::new(options.clone())), error)
                    }
                },
            }
        } else {
            (ModeState::Ansi(AnsiEditor::new(options.clone())), None)
        };

        Self {
            id,
            mode_state,
            options,
            menu_state: MenuBarState::new(),
            show_right_panel: true,
            error_dialog,
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
                // Try to load the file
                match AnsiEditor::with_file(path.clone(), self.options.clone()) {
                    Ok(editor) => {
                        self.mode_state = ModeState::Ansi(editor);
                    }
                    Err(e) => {
                        self.error_dialog = Some(("Error Loading File".to_string(), format!("Failed to load '{}': {}", path.display(), e)));
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
                // TODO: Implement undo
                Task::none()
            }
            Message::Redo => {
                // TODO: Implement redo
                Task::none()
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
                    EditMode::BitFont => ModeState::BitFont(BitFontEditorState::new()),
                    EditMode::CharFont => ModeState::CharFont(CharFontEditorState::new()),
                };
                Task::none()
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
        let menu_bar = self.menu_state.view();

        let content: Element<'_, Message> = match &self.mode_state {
            ModeState::Ansi(editor) => self.view_ansi_editor(editor),
            ModeState::BitFont(_state) => self.view_bitfont_editor(),
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

    fn view_bitfont_editor(&self) -> Element<'_, Message> {
        container(text("BitFont Editor - Coming Soon").size(24))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
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
            ModeState::BitFont(_) => StatusBarInfo {
                left: "BitFont Editor".into(),
                center: String::new(),
                right: String::new(),
            },
            ModeState::CharFont(_) => StatusBarInfo {
                left: "CharFont Editor".into(),
                center: String::new(),
                right: String::new(),
            },
        }
    }

    /// Handle events passed from the window manager
    pub fn handle_event(&mut self, _event: &Event) -> Option<Message> {
        // Handle mode-specific events
        match &mut self.mode_state {
            ModeState::Ansi(_editor) => {
                // TODO: Handle ANSI editor events (keyboard, etc.)
            }
            ModeState::BitFont(_state) => {
                // TODO: Handle BitFont editor events
            }
            ModeState::CharFont(_state) => {
                // TODO: Handle CharFont editor events
            }
        }
        None
    }
}
