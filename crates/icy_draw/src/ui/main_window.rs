//! MainWindow for icy_draw
//!
//! Each MainWindow represents one editing window with its own state and mode.
//! The mode determines what kind of editor is shown (ANSI, BitFont, CharFont).

use std::{path::PathBuf, sync::Arc};

use parking_lot::Mutex;

use iced::{
    Element, Event, Task, Theme,
    widget::{column, container, text},
};

use super::{SharedOptions, menu::MenuBuilder};

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

/// State for the ANSI editor mode
pub struct AnsiEditorState {
    // TODO: Add AnsiEditor state
    // - buffer
    // - layers
    // - undo stack
    // - selection
    // - cursor position
}

impl AnsiEditorState {
    pub fn new() -> Self {
        Self {}
    }

    pub fn with_file(_path: PathBuf) -> Self {
        // TODO: Load file
        Self {}
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
    Ansi(AnsiEditorState),
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
}

/// Message type for MainWindow
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum Message {
    // File operations
    NewFile,
    OpenFile,
    SaveFile,
    SaveFileAs,
    CloseFile,

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

    // Mode switching
    SwitchMode(EditMode),

    // Menu
    MenuAction(String),

    // Internal
    Tick,
}

/// A single editing window
#[allow(dead_code)]
pub struct MainWindow {
    /// Window ID (1-based, for Alt+N switching)
    pub id: usize,

    /// Current file path (if saved)
    pub file_path: Option<PathBuf>,

    /// Current editing mode and state
    mode_state: ModeState,

    /// Shared options
    options: Arc<Mutex<SharedOptions>>,

    /// Menu builder for this window
    menu_builder: MenuBuilder,

    /// Is the document modified?
    is_modified: bool,
}

impl MainWindow {
    pub fn new(id: usize, path: Option<PathBuf>, options: Arc<Mutex<SharedOptions>>) -> Self {
        let mode_state = if let Some(ref p) = path {
            // Determine mode based on file extension
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            match ext.to_lowercase().as_str() {
                "psf" | "f16" | "f14" | "f08" => ModeState::BitFont(BitFontEditorState::new()),
                "tdf" => ModeState::CharFont(CharFontEditorState::new()),
                _ => ModeState::Ansi(AnsiEditorState::with_file(p.clone())),
            }
        } else {
            ModeState::Ansi(AnsiEditorState::new())
        };

        Self {
            id,
            file_path: path,
            mode_state,
            options,
            menu_builder: MenuBuilder::new(),
            is_modified: false,
        }
    }

    pub fn title(&self) -> String {
        let mode = self.mode_state.mode();
        let file_name = self
            .file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled");

        let modified = if self.is_modified { " •" } else { "" };

        format!("{}{} - iCY DRAW [{}]", file_name, modified, mode)
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NewFile => {
                // TODO: Implement new file
                Task::none()
            }
            Message::OpenFile => {
                // TODO: Implement open file dialog
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
                // TODO: Implement zoom in
                Task::none()
            }
            Message::ZoomOut => {
                // TODO: Implement zoom out
                Task::none()
            }
            Message::ZoomReset => {
                // TODO: Implement zoom reset
                Task::none()
            }
            Message::SwitchMode(mode) => {
                // TODO: Handle mode switching properly (save current state, etc.)
                self.mode_state = match mode {
                    EditMode::Ansi => ModeState::Ansi(AnsiEditorState::new()),
                    EditMode::BitFont => ModeState::BitFont(BitFontEditorState::new()),
                    EditMode::CharFont => ModeState::CharFont(CharFontEditorState::new()),
                };
                Task::none()
            }
            Message::MenuAction(action) => {
                log::info!("Menu action: {}", action);
                // TODO: Route menu actions to appropriate handlers
                Task::none()
            }
            Message::Tick => Task::none(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        // Build the UI based on current mode
        let menu_bar = self.menu_builder.build();

        let content: Element<'_, Message> = match &self.mode_state {
            ModeState::Ansi(_state) => self.view_ansi_editor(),
            ModeState::BitFont(_state) => self.view_bitfont_editor(),
            ModeState::CharFont(_state) => self.view_charfont_editor(),
        };

        column![menu_bar, content,].into()
    }

    fn view_ansi_editor(&self) -> Element<'_, Message> {
        // TODO: Implement ANSI editor view
        container(text("ANSI Editor - Coming Soon").size(24))
            .center_x(iced::Length::Fill)
            .center_y(iced::Length::Fill)
            .into()
    }

    fn view_bitfont_editor(&self) -> Element<'_, Message> {
        // TODO: Implement BitFont editor view
        container(text("BitFont Editor - Coming Soon").size(24))
            .center_x(iced::Length::Fill)
            .center_y(iced::Length::Fill)
            .into()
    }

    fn view_charfont_editor(&self) -> Element<'_, Message> {
        // TODO: Implement CharFont editor view
        container(text("CharFont (TDF) Editor - Coming Soon").size(24))
            .center_x(iced::Length::Fill)
            .center_y(iced::Length::Fill)
            .into()
    }

    /// Handle events passed from the window manager
    pub fn handle_event(&mut self, _event: &Event) -> Option<Message> {
        // Handle mode-specific events
        match &mut self.mode_state {
            ModeState::Ansi(_state) => {
                // TODO: Handle ANSI editor events
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
