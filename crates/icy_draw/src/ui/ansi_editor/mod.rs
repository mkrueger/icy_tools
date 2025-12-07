//! ANSI Editor Mode
//!
//! This module contains the main ANSI art editor with:
//! - Left sidebar: Color switcher, Palette, Tools
//! - Top toolbar: Tool-specific options
//! - Center: Terminal/Canvas view
//! - Right panel: Minimap, Layers, Channels

mod canvas_view;
mod channels_view;
mod color_switcher;
mod layer_view;
mod minimap_view;
mod palette_grid;
mod right_panel;
mod tool_panel;
mod top_toolbar;

pub use canvas_view::*;
pub use channels_view::*;
pub use color_switcher::*;
use icy_engine_edit::tools::{self, Tool, ToolEvent};
use icy_engine_edit::EditState;
pub use layer_view::*;
pub use minimap_view::*;
pub use palette_grid::*;
pub use right_panel::*;
pub use tool_panel::*;
pub use top_toolbar::*;

use std::path::PathBuf;
use std::sync::Arc;

use iced::{
    Element, Length, Task,
    widget::{column, container, row, rule, scrollable},
};
use icy_engine::formats::{FileFormat, LoadData};
use icy_engine::{TextBuffer, TextPane};
use parking_lot::Mutex;

use crate::ui::SharedOptions;

/// Messages for the ANSI editor
#[derive(Clone, Debug)]
pub enum AnsiEditorMessage {
    /// Tool panel messages
    ToolPanel(ToolPanelMessage),
    /// Canvas view messages  
    Canvas(CanvasMessage),
    /// Right panel messages (minimap, layers, etc.)
    RightPanel(RightPanelMessage),
    /// Top toolbar messages
    TopToolbar(TopToolbarMessage),
    /// Color switcher messages
    ColorSwitcher(ColorSwitcherMessage),
    /// Palette grid messages
    PaletteGrid(PaletteGridMessage),
    /// Tool selection changed
    SelectTool(usize),
    /// Layer selection changed
    SelectLayer(usize),
    /// Toggle layer visibility
    ToggleLayerVisibility(usize),
    /// Add new layer
    AddLayer,
    /// Remove layer
    RemoveLayer(usize),
    /// Move layer up
    MoveLayerUp(usize),
    /// Move layer down
    MoveLayerDown(usize),
    /// Viewport tick for animations
    ViewportTick,
    /// Scroll viewport
    ScrollViewport(f32, f32),
    /// Key pressed
    KeyPressed(iced::keyboard::Key, iced::keyboard::Modifiers),
    /// Mouse event on canvas
    CanvasMouseEvent(CanvasMouseEvent),
}

/// Mouse events on the canvas
#[derive(Clone, Debug)]
pub enum CanvasMouseEvent {
    Press { position: iced::Point, button: iced::mouse::Button },
    Release { position: iced::Point, button: iced::mouse::Button },
    Move { position: iced::Point },
    Scroll { delta: iced::mouse::ScrollDelta },
}

/// The main ANSI editor component
pub struct AnsiEditor {
    /// Unique ID for this editor
    pub id: u64,
    /// File path (if saved)
    pub file_path: Option<PathBuf>,
    /// The edit state (wraps buffer, caret, undo stack, etc.)
    pub edit_state: Arc<Mutex<EditState>>,
    /// Tool panel state (left sidebar icons)
    pub tool_panel: ToolPanel,
    /// Current active tool
    pub current_tool: Tool,
    /// Top toolbar (tool-specific options)
    pub top_toolbar: TopToolbar,
    /// Color switcher (FG/BG display)
    pub color_switcher: ColorSwitcher,
    /// Palette grid
    pub palette_grid: PaletteGrid,
    /// Canvas view state
    pub canvas: CanvasView,
    /// Right panel state (minimap, layers)
    pub right_panel: RightPanel,
    /// Shared options
    pub options: Arc<Mutex<SharedOptions>>,
    /// Whether the document is modified
    pub is_modified: bool,
}

static mut NEXT_ID: u64 = 0;

impl AnsiEditor {
    /// Create a new empty ANSI editor
    pub fn new(options: Arc<Mutex<SharedOptions>>) -> Self {
        let buffer = TextBuffer::create((80, 25));
        Self::with_buffer(buffer, None, options)
    }

    /// Create an ANSI editor with a file
    ///
    /// Returns the editor with the loaded buffer, or an error if loading failed.
    pub fn with_file(path: PathBuf, options: Arc<Mutex<SharedOptions>>) -> anyhow::Result<Self> {
        // Detect file format
        let format = FileFormat::from_path(&path).ok_or_else(|| anyhow::anyhow!("Unknown file format"))?;

        if !format.is_supported() {
            anyhow::bail!("Format '{}' is not supported for editing", format.name());
        }

        // Read file data
        let data = std::fs::read(&path)?;

        // Load buffer using the format
        let load_data = LoadData::default();
        let buffer = format.load_buffer(&path, &data, Some(load_data))?;

        Ok(Self::with_buffer(buffer, Some(path), options))
    }

    /// Create an ANSI editor with an existing buffer
    pub fn with_buffer(buffer: TextBuffer, file_path: Option<PathBuf>, options: Arc<Mutex<SharedOptions>>) -> Self {
        let id = unsafe {
            NEXT_ID = NEXT_ID.wrapping_add(1);
            NEXT_ID
        };

        // Clone the palette before moving buffer into EditState
        let palette = buffer.palette.clone();
        let edit_state = Arc::new(Mutex::new(EditState::from_buffer(buffer)));

        // Create palette components with synced palette
        let mut palette_grid = PaletteGrid::new();
        palette_grid.sync_palette(&palette);
        
        let mut color_switcher = ColorSwitcher::new();
        color_switcher.sync_palette(&palette);

        Self {
            id,
            file_path,
            edit_state,
            tool_panel: ToolPanel::new(),
            current_tool: Tool::Click,
            top_toolbar: TopToolbar::new(),
            color_switcher,
            palette_grid,
            canvas: CanvasView::new(),
            right_panel: RightPanel::new(),
            options,
            is_modified: false,
        }
    }

    /// Get the document title for display
    pub fn title(&self) -> String {
        let file_name = self
            .file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled");

        let modified = if self.is_modified { " •" } else { "" };
        format!("{}{}", file_name, modified)
    }

    /// Update the editor state
    pub fn update(&mut self, message: AnsiEditorMessage) -> Task<AnsiEditorMessage> {
        match message {
            AnsiEditorMessage::ToolPanel(msg) => {
                // Handle tool panel messages - this updates current_tool via toggle
                let ToolPanelMessage::ClickSlot(_) = &msg;
                // After the tool panel updates, sync our current_tool
                let _ = self.tool_panel.update(msg.clone());
                self.current_tool = self.tool_panel.current_tool();
                Task::none()
            }
            AnsiEditorMessage::Canvas(msg) => self.canvas.update(msg, &self.edit_state).map(AnsiEditorMessage::Canvas),
            AnsiEditorMessage::RightPanel(msg) => self.right_panel.update(msg).map(AnsiEditorMessage::RightPanel),
            AnsiEditorMessage::TopToolbar(msg) => self.top_toolbar.update(msg).map(AnsiEditorMessage::TopToolbar),
            AnsiEditorMessage::ColorSwitcher(msg) => {
                match msg {
                    ColorSwitcherMessage::SwapColors => {
                        let mut state = self.edit_state.lock();
                        let caret = state.get_caret_mut();
                        let fg = caret.attribute.get_foreground();
                        let bg = caret.attribute.get_background();
                        caret.attribute.set_foreground(bg);
                        caret.attribute.set_background(fg);
                        self.palette_grid.set_foreground(bg);
                        self.palette_grid.set_background(fg);
                    }
                    ColorSwitcherMessage::ResetToDefault => {
                        let mut state = self.edit_state.lock();
                        let caret = state.get_caret_mut();
                        caret.attribute.set_foreground(7);
                        caret.attribute.set_background(0);
                        self.palette_grid.set_foreground(7);
                        self.palette_grid.set_background(0);
                    }
                }
                Task::none()
            }
            AnsiEditorMessage::PaletteGrid(msg) => {
                match msg {
                    PaletteGridMessage::SetForeground(color) => {
                        let mut state = self.edit_state.lock();
                        state.get_caret_mut().attribute.set_foreground(color);
                        self.palette_grid.set_foreground(color);
                    }
                    PaletteGridMessage::SetBackground(color) => {
                        let mut state = self.edit_state.lock();
                        state.get_caret_mut().attribute.set_background(color);
                        self.palette_grid.set_background(color);
                    }
                }
                Task::none()
            }
            AnsiEditorMessage::SelectTool(idx) => {
                // Select tool by slot index
                self.current_tool = tools::click_tool_slot(idx, self.current_tool);
                self.tool_panel.set_tool(self.current_tool);
                Task::none()
            }
            AnsiEditorMessage::SelectLayer(idx) => {
                let mut state = self.edit_state.lock();
                state.set_current_layer(idx);
                Task::none()
            }
            AnsiEditorMessage::ToggleLayerVisibility(idx) => {
                let mut state = self.edit_state.lock();
                if let Some(layer) = state.get_buffer_mut().layers.get_mut(idx) {
                    layer.set_is_visible(!layer.get_is_visible());
                    self.is_modified = true;
                }
                Task::none()
            }
            AnsiEditorMessage::AddLayer => {
                // TODO: Add new layer
                self.is_modified = true;
                Task::none()
            }
            AnsiEditorMessage::RemoveLayer(idx) => {
                let mut state = self.edit_state.lock();
                let layer_count = state.get_buffer().layers.len();
                if layer_count > 1 && idx < layer_count {
                    state.get_buffer_mut().layers.remove(idx);
                    let new_layer_count = state.get_buffer().layers.len();
                    let current = state.get_current_layer().unwrap_or(0);
                    if current >= new_layer_count {
                        state.set_current_layer(new_layer_count.saturating_sub(1));
                    }
                    self.is_modified = true;
                }
                Task::none()
            }
            AnsiEditorMessage::MoveLayerUp(idx) => {
                let mut state = self.edit_state.lock();
                let layer_count = state.get_buffer().layers.len();
                if idx + 1 < layer_count {
                    state.get_buffer_mut().layers.swap(idx, idx + 1);
                    state.set_current_layer(idx + 1);
                    self.is_modified = true;
                }
                Task::none()
            }
            AnsiEditorMessage::MoveLayerDown(idx) => {
                let mut state = self.edit_state.lock();
                if idx > 0 {
                    state.get_buffer_mut().layers.swap(idx, idx - 1);
                    state.set_current_layer(idx - 1);
                    self.is_modified = true;
                }
                Task::none()
            }
            AnsiEditorMessage::ViewportTick => {
                self.canvas.update_animations();
                Task::none()
            }
            AnsiEditorMessage::ScrollViewport(dx, dy) => {
                self.canvas.scroll_by(dx, dy);
                Task::none()
            }
            AnsiEditorMessage::KeyPressed(key, modifiers) => {
                self.handle_key_press(key, modifiers);
                Task::none()
            }
            AnsiEditorMessage::CanvasMouseEvent(event) => {
                self.handle_canvas_mouse_event(event);
                Task::none()
            }
        }
    }

    /// Handle key press events
    fn handle_key_press(&mut self, key: iced::keyboard::Key, modifiers: iced::keyboard::Modifiers) {
        // Check for tool shortcuts (single character keys)
        if !modifiers.control() && !modifiers.alt() {
            if let iced::keyboard::Key::Character(c) = &key {
                if let Some(ch) = c.chars().next() {
                    // Find tool with this shortcut
                    for (slot_idx, pair) in tools::TOOL_SLOTS.iter().enumerate() {
                        if pair.primary.shortcut() == Some(ch) {
                            self.current_tool = tools::click_tool_slot(slot_idx, self.current_tool);
                            self.tool_panel.set_tool(self.current_tool);
                            return;
                        }
                        if pair.secondary.shortcut() == Some(ch) {
                            self.current_tool = tools::click_tool_slot(slot_idx, self.current_tool);
                            self.tool_panel.set_tool(self.current_tool);
                            return;
                        }
                    }
                }
            }
        }

        // Handle tool-specific key events
        let event = self.handle_tool_key(&key, &modifiers);
        self.handle_tool_event(event);
    }

    /// Handle tool-specific key events based on current tool
    fn handle_tool_key(&mut self, key: &iced::keyboard::Key, modifiers: &iced::keyboard::Modifiers) -> ToolEvent {
        use iced::keyboard::key::Named;

        match self.current_tool {
            Tool::Click | Tool::Font => {
                // Handle typing and cursor movement
                match key {
                    iced::keyboard::Key::Character(c) => {
                        if !modifiers.control() && !modifiers.alt() {
                            if let Some(ch) = c.chars().next() {
                                // Type character at cursor
                                // TODO: Actually insert into buffer
                                let _ = ch;
                                return ToolEvent::Commit("Type character".to_string());
                            }
                        }
                    }
                    iced::keyboard::Key::Named(named) => {
                        let mut state = self.edit_state.lock();
                        let buffer_width = state.get_buffer().get_width();
                        let caret = state.get_caret_mut();
                        match named {
                            Named::ArrowUp => caret.y = (caret.y - 1).max(0),
                            Named::ArrowDown => caret.y += 1,
                            Named::ArrowLeft => caret.x = (caret.x - 1).max(0),
                            Named::ArrowRight => caret.x += 1,
                            Named::Home => caret.x = 0,
                            Named::End => caret.x = buffer_width - 1,
                            Named::PageUp => caret.y = (caret.y - 24).max(0),
                            Named::PageDown => caret.y += 24,
                            _ => {}
                        }
                        return ToolEvent::Redraw;
                    }
                    _ => {}
                }
            }
            _ => {
                // Other tools don't handle keyboard in the same way
            }
        }
        ToolEvent::None
    }

    /// Handle canvas mouse events by forwarding to current tool
    fn handle_canvas_mouse_event(&mut self, event: CanvasMouseEvent) {
        use icy_engine::Position;

        // Convert screen position to buffer position
        // TODO: Use actual terminal rendering info for accurate conversion
        let to_buffer_pos = |point: iced::Point| -> Position {
            // Approximate conversion - should use render_info from terminal
            Position::new(point.x as i32 / 8, point.y as i32 / 16)
        };

        match event {
            CanvasMouseEvent::Press { position, button: _ } => {
                let pos = to_buffer_pos(position);
                let tool_event = self.handle_tool_mouse_down(pos);
                self.handle_tool_event(tool_event);
            }
            CanvasMouseEvent::Release { position, button: _ } => {
                let pos = to_buffer_pos(position);
                let tool_event = self.handle_tool_mouse_up(pos);
                self.handle_tool_event(tool_event);
            }
            CanvasMouseEvent::Move { position } => {
                let pos = to_buffer_pos(position);
                let tool_event = self.handle_tool_mouse_move(pos);
                self.handle_tool_event(tool_event);
            }
            CanvasMouseEvent::Scroll { delta } => match delta {
                iced::mouse::ScrollDelta::Lines { x, y } => {
                    self.canvas.scroll_by(x * 20.0, y * 20.0);
                }
                iced::mouse::ScrollDelta::Pixels { x, y } => {
                    self.canvas.scroll_by(x, y);
                }
            },
        }
    }

    /// Handle mouse down based on current tool
    fn handle_tool_mouse_down(&mut self, pos: icy_engine::Position) -> ToolEvent {
        match self.current_tool {
            Tool::Click => {
                // Move cursor to clicked position
                let mut state = self.edit_state.lock();
                let caret = state.get_caret_mut();
                caret.x = pos.x;
                caret.y = pos.y;
                ToolEvent::Redraw
            }
            Tool::Select => {
                // Start selection - TODO: implement selection in EditState
                ToolEvent::Redraw
            }
            Tool::Pencil | Tool::Brush => {
                // Start drawing - TODO: implement drawing state
                ToolEvent::Redraw
            }
            Tool::Pipette => {
                // Pick character/color at position
                // TODO: Actually pick from buffer
                ToolEvent::Status(format!("Picked at ({}, {})", pos.x, pos.y))
            }
            Tool::Fill => {
                // Flood fill at position
                // TODO: Implement flood fill
                ToolEvent::Commit("Flood fill".to_string())
            }
            _ => ToolEvent::None,
        }
    }

    /// Handle mouse up based on current tool
    fn handle_tool_mouse_up(&mut self, _pos: icy_engine::Position) -> ToolEvent {
        match self.current_tool {
            Tool::Select => {
                ToolEvent::Redraw
            }
            Tool::Pencil | Tool::Brush | Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled => {
                // TODO: Commit the drawn shape
                ToolEvent::Commit(format!("{} drawn", self.current_tool.name()))
            }
            _ => ToolEvent::None,
        }
    }

    /// Handle mouse move based on current tool
    fn handle_tool_mouse_move(&mut self, _pos: icy_engine::Position) -> ToolEvent {
        match self.current_tool {
            Tool::Select | Tool::Pencil | Tool::Brush | Tool::Erase | Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled => {
                // TODO: Update preview/drawing
                ToolEvent::Redraw
            }
            _ => ToolEvent::None,
        }
    }

    /// Handle tool events (redraw, commit, status)
    fn handle_tool_event(&mut self, event: ToolEvent) {
        match event {
            ToolEvent::None => {}
            ToolEvent::Redraw => {
                // Trigger redraw - handled automatically by Iced
            }
            ToolEvent::Commit(description) => {
                self.is_modified = true;
                // TODO: Add to undo stack with description
                let _ = description;
            }
            ToolEvent::Status(message) => {
                // TODO: Update status bar
                let _ = message;
            }
        }
    }

    /// Render the editor view with Moebius-style layout:
    /// - Left sidebar: Palette (vertical) + Tool icons
    /// - Top toolbar: Color switcher + Tool-specific options
    /// - Center: Canvas
    /// - Right panel: Minimap, Layers, Channels
    pub fn view(&self) -> Element<'_, AnsiEditorMessage> {
        // === LEFT SIDEBAR ===
        // Palette grid (vertical for small palettes)
        let palette_width = self.palette_grid.cached_palette_width();
        let palette_view = self.palette_grid.view().map(AnsiEditorMessage::PaletteGrid);

        // Tool panel (9 icons)
        let tool_panel = self.tool_panel.view().map(AnsiEditorMessage::ToolPanel);

        // Calculate sidebar width based on palette
        let sidebar_width = palette_width.max(38.0) + 8.0;

        let left_sidebar = column![scrollable(palette_view).height(Length::Fill), tool_panel,].spacing(0);

        // === TOP TOOLBAR (with color switcher on the left) ===
        // Color switcher (classic icy_draw style) - shows caret's foreground/background colors
        let color_switcher = self
            .color_switcher
            .view()
            .map(AnsiEditorMessage::ColorSwitcher);

        let top_toolbar_content = self.top_toolbar.view(self.current_tool).map(AnsiEditorMessage::TopToolbar);

        // Toolbar height matches color switcher size + padding
        let toolbar_height = 32.0 + 8.0;

        let top_toolbar = row![container(color_switcher).padding(4), rule::vertical(1), top_toolbar_content,].spacing(4);

        // === CENTER: Canvas ===
        let canvas = self.canvas.view(&self.edit_state).map(AnsiEditorMessage::Canvas);

        // === RIGHT PANEL ===
        let right_panel = self
            .right_panel
            .view(&self.edit_state, self.current_tool)
            .map(AnsiEditorMessage::RightPanel);

        // Main layout:
        // Top row: Full-width toolbar (with color switcher)
        // Bottom row: Left sidebar | Canvas | Right panel

        let bottom_row = row![
            // Left sidebar - dynamic width based on palette size
            container(left_sidebar).width(Length::Fixed(sidebar_width)),
            // Center - canvas
            container(canvas).width(Length::Fill).height(Length::Fill),
            // Right panel - fixed width
            container(right_panel).width(Length::Fixed(200.0)),
        ];

        column![
            // Top toolbar - full width
            container(top_toolbar)
                .width(Length::Fill)
                .height(Length::Fixed(toolbar_height))
                .style(container::rounded_box),
            // Bottom content
            bottom_row,
        ]
        .spacing(0)
        .into()
    }

    /// Sync UI components with the current edit state
    /// Call this after operations that may change the palette
    pub fn sync_ui(&mut self) {
        let state = self.edit_state.lock();
        let buffer = state.get_buffer();
        self.palette_grid.sync_palette(&buffer.palette);
        self.color_switcher.sync_palette(&buffer.palette);
    }

    /// Get status bar information for this editor
    pub fn status_info(&self) -> AnsiStatusInfo {
        let state = self.edit_state.lock();
        let buffer = state.get_buffer();
        let caret = state.get_caret();
        let current_layer = state.get_current_layer().unwrap_or(0);
        
        AnsiStatusInfo {
            cursor_position: (caret.x, caret.y),
            buffer_size: (buffer.get_width(), buffer.get_height()),
            current_layer,
            total_layers: buffer.layers.len(),
            current_tool: self.current_tool.name().to_string(),
            insert_mode: caret.insert_mode,
        }
    }
}

/// Status bar information for the ANSI editor
#[derive(Clone, Debug)]
pub struct AnsiStatusInfo {
    pub cursor_position: (i32, i32),
    pub buffer_size: (i32, i32),
    pub current_layer: usize,
    pub total_layers: usize,
    pub current_tool: String,
    pub insert_mode: bool,
}
