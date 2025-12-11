//! ANSI Editor Mode
//!
//! This module contains the main ANSI art editor with:
//! - Left sidebar: Color switcher, Palette, Tools
//! - Top toolbar: Tool-specific options
//! - Center: Terminal/Canvas view
//! - Right panel: Minimap, Layers, Channels

mod canvas_view;
mod channels_view;
mod color_switcher_gpu;
pub mod constants;
mod layer_view;
pub mod menu_bar;
mod minimap_view;
mod palette_grid;
mod right_panel;
mod tool_panel;
mod tool_panel_wrapper;
mod top_toolbar;

pub use canvas_view::*;
pub use channels_view::*;
pub use color_switcher_gpu::*;
use icy_engine_edit::EditState;
use icy_engine_edit::tools::{self, Tool, ToolEvent};
pub use layer_view::*;
pub use minimap_view::*;
pub use palette_grid::*;
pub use right_panel::*;
// Use shared GPU-accelerated tool panel via wrapper
pub use tool_panel_wrapper::{ToolPanel, ToolPanelMessage};
pub use top_toolbar::*;

use std::path::PathBuf;
use std::sync::Arc;

use iced::{
    Element, Length, Task, Theme,
    widget::{column, container, row},
};
use icy_engine::formats::{FileFormat, LoadData};
use icy_engine::{Screen, TextBuffer, TextPane};
use icy_engine_gui::theme::main_area_background;
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
    /// The screen (contains EditState which wraps buffer, caret, undo stack, etc.)
    /// Use screen.lock().as_any_mut().downcast_mut::<EditState>() to access EditState methods
    pub screen: Arc<Mutex<Box<dyn Screen>>>,
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
        let buffer = format.from_bytes(&path, &data, Some(load_data))?.buffer;

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

        // Create EditState and wrap as Box<dyn Screen> for Terminal compatibility
        let edit_state = EditState::from_buffer(buffer);
        let screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(edit_state)));

        // Create palette components with synced palette
        let mut palette_grid = PaletteGrid::new();
        palette_grid.sync_palette(&palette);

        let mut color_switcher = ColorSwitcher::new();
        color_switcher.sync_palette(&palette);

        // Create canvas with cloned Arc to screen
        let canvas = CanvasView::new(screen.clone());

        Self {
            id,
            file_path,
            screen,
            tool_panel: ToolPanel::new(),
            current_tool: Tool::Click,
            top_toolbar: TopToolbar::new(),
            color_switcher,
            palette_grid,
            canvas,
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

    /// Set the file path (for session restore)
    pub fn set_file_path(&mut self, path: PathBuf) {
        self.file_path = Some(path);
    }

    /// Load from an autosave file, using the original path for format detection
    ///
    /// The autosave file is always saved in ICY format (to preserve layers, fonts, etc.),
    /// but we set the original path so future saves use the correct format.
    pub fn load_from_autosave(autosave_path: &std::path::Path, original_path: PathBuf, options: Arc<Mutex<SharedOptions>>) -> anyhow::Result<Self> {
        // Autosaves are always saved in ICY format
        let format = FileFormat::IcyDraw;

        // Read autosave data
        let data = std::fs::read(autosave_path)?;

        // Load buffer using ICY format
        let load_data = LoadData::default();
        let buffer = format.load_buffer(autosave_path, &data, Some(load_data))?;

        let mut editor = Self::with_buffer(buffer, Some(original_path), options);
        editor.is_modified = true; // Autosave means we have unsaved changes
        Ok(editor)
    }

    /// Access the EditState via downcast from the Screen trait object
    /// Panics if the screen is not an EditState (should never happen in AnsiEditor)
    fn with_edit_state<T, F: FnOnce(&mut EditState) -> T>(&mut self, f: F) -> T {
        let mut screen = self.screen.lock();
        let edit_state = screen
            .as_any_mut()
            .downcast_mut::<EditState>()
            .expect("AnsiEditor screen should always be EditState");
        f(edit_state)
    }

    /// Get undo stack length for dirty tracking
    pub fn undo_stack_len(&self) -> usize {
        let mut screen = self.screen.lock();
        if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
            edit_state.undo_stack_len()
        } else {
            0
        }
    }

    /// Save the document to the given path
    pub fn save(&mut self, path: &std::path::Path) -> Result<(), String> {
        let mut screen = self.screen.lock();
        if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
            // Determine format from extension
            let format = FileFormat::from_path(path).ok_or_else(|| "Unknown file format".to_string())?;

            // Get buffer and save with default options
            let buffer = edit_state.get_buffer_mut();
            let options = icy_engine::formats::SaveOptions::default();
            let bytes = format.to_bytes(buffer, &options).map_err(|e| e.to_string())?;

            std::fs::write(path, bytes).map_err(|e| e.to_string())?;

            self.is_modified = false;
            Ok(())
        } else {
            Err("Could not access edit state".to_string())
        }
    }

    /// Get bytes for autosave (saves in ICY format with thumbnail skipped for performance)
    pub fn get_autosave_bytes(&self) -> Result<Vec<u8>, String> {
        let mut screen = self.screen.lock();
        if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
            // Use ICY format for autosave to preserve all data (layers, fonts, etc.)
            let format = FileFormat::IcyDraw;
            let buffer = edit_state.get_buffer_mut();
            // Skip thumbnail generation for faster autosave
            let mut options = icy_engine::formats::SaveOptions::default();
            options.skip_thumbnail = true;
            format.to_bytes(buffer, &options).map_err(|e| e.to_string())
        } else {
            Err("Could not access edit state".to_string())
        }
    }

    /// Check if this editor needs animation updates (for smooth animations)
    pub fn needs_animation(&self) -> bool {
        self.color_switcher.needs_animation() || self.tool_panel.needs_animation() || self.canvas.needs_animation()
    }

    /// Update the editor state
    pub fn update(&mut self, message: AnsiEditorMessage) -> Task<AnsiEditorMessage> {
        match message {
            AnsiEditorMessage::ToolPanel(msg) => {
                // Handle tool panel messages
                match &msg {
                    ToolPanelMessage::ClickSlot(_) => {
                        // After the tool panel updates, sync our current_tool
                        let _ = self.tool_panel.update(msg.clone());
                        self.current_tool = self.tool_panel.current_tool();
                    }
                    ToolPanelMessage::Tick(delta) => {
                        self.tool_panel.tick(*delta);
                    }
                }
                Task::none()
            }
            AnsiEditorMessage::Canvas(msg) => self.canvas.update(msg).map(AnsiEditorMessage::Canvas),
            AnsiEditorMessage::RightPanel(msg) => self.right_panel.update(msg).map(AnsiEditorMessage::RightPanel),
            AnsiEditorMessage::TopToolbar(msg) => self.top_toolbar.update(msg).map(AnsiEditorMessage::TopToolbar),
            AnsiEditorMessage::ColorSwitcher(msg) => {
                match msg {
                    ColorSwitcherMessage::SwapColors => {
                        // Just start the animation, don't swap yet
                        self.color_switcher.start_swap_animation();
                    }
                    ColorSwitcherMessage::AnimationComplete => {
                        // Animation finished - now actually swap the colors
                        let (fg, bg) = self.with_edit_state(|state| {
                            let caret: &mut icy_engine::Caret = state.get_caret_mut();
                            let fg = caret.attribute.get_foreground();
                            let bg = caret.attribute.get_background();
                            caret.attribute.set_foreground(bg);
                            caret.attribute.set_background(fg);
                            (bg, fg)
                        });
                        self.palette_grid.set_foreground(fg);
                        self.palette_grid.set_background(bg);
                        // Confirm the swap so the shader resets to normal display
                        self.color_switcher.confirm_swap();
                    }
                    ColorSwitcherMessage::ResetToDefault => {
                        self.with_edit_state(|state| {
                            let caret = state.get_caret_mut();
                            caret.attribute.set_foreground(7);
                            caret.attribute.set_background(0);
                        });
                        self.palette_grid.set_foreground(7);
                        self.palette_grid.set_background(0);
                    }
                    ColorSwitcherMessage::Tick(delta) => {
                        if self.color_switcher.tick(delta) {
                            // Animation completed - trigger the actual color swap
                            return Task::done(AnsiEditorMessage::ColorSwitcher(ColorSwitcherMessage::AnimationComplete));
                        }
                    }
                }
                Task::none()
            }
            AnsiEditorMessage::PaletteGrid(msg) => {
                match msg {
                    PaletteGridMessage::SetForeground(color) => {
                        self.with_edit_state(|state| {
                            state.get_caret_mut().attribute.set_foreground(color);
                        });
                        self.palette_grid.set_foreground(color);
                    }
                    PaletteGridMessage::SetBackground(color) => {
                        self.with_edit_state(|state| {
                            state.get_caret_mut().attribute.set_background(color);
                        });
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
                self.with_edit_state(|state| state.set_current_layer(idx));
                Task::none()
            }
            AnsiEditorMessage::ToggleLayerVisibility(idx) => {
                let modified = self.with_edit_state(|state| {
                    if let Some(layer) = state.get_buffer_mut().layers.get_mut(idx) {
                        layer.set_is_visible(!layer.get_is_visible());
                        true
                    } else {
                        false
                    }
                });
                if modified {
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
                let modified = self.with_edit_state(|state| {
                    let layer_count = state.get_buffer().layers.len();
                    if layer_count > 1 && idx < layer_count {
                        state.get_buffer_mut().layers.remove(idx);
                        let new_layer_count = state.get_buffer().layers.len();
                        let current = state.get_current_layer().unwrap_or(0);
                        if current >= new_layer_count {
                            state.set_current_layer(new_layer_count.saturating_sub(1));
                        }
                        true
                    } else {
                        false
                    }
                });
                if modified {
                    self.is_modified = true;
                }
                Task::none()
            }
            AnsiEditorMessage::MoveLayerUp(idx) => {
                let modified = self.with_edit_state(|state| {
                    let layer_count = state.get_buffer().layers.len();
                    if idx + 1 < layer_count {
                        state.get_buffer_mut().layers.swap(idx, idx + 1);
                        state.set_current_layer(idx + 1);
                        true
                    } else {
                        false
                    }
                });
                if modified {
                    self.is_modified = true;
                }
                Task::none()
            }
            AnsiEditorMessage::MoveLayerDown(idx) => {
                let modified = self.with_edit_state(|state| {
                    if idx > 0 {
                        state.get_buffer_mut().layers.swap(idx, idx - 1);
                        state.set_current_layer(idx - 1);
                        true
                    } else {
                        false
                    }
                });
                if modified {
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
                        self.with_edit_state(|state| {
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
                        });
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
                self.with_edit_state(|state| {
                    let caret = state.get_caret_mut();
                    caret.x = pos.x;
                    caret.y = pos.y;
                });
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
            Tool::Select => ToolEvent::Redraw,
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
            Tool::Select
            | Tool::Pencil
            | Tool::Brush
            | Tool::Erase
            | Tool::Line
            | Tool::RectangleOutline
            | Tool::RectangleFilled
            | Tool::EllipseOutline
            | Tool::EllipseFilled => {
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
        // Fixed sidebar width - palette and tool panel adapt to this
        let sidebar_width = 64.0; // Fixed width for sidebar

        // Palette grid - adapts to sidebar width
        let palette_view = self.palette_grid.view_with_width(sidebar_width).map(AnsiEditorMessage::PaletteGrid);

        // Tool panel - calculate columns based on sidebar width
        // Use theme's main area background color
        let bg_weakest = main_area_background(&Theme::Dark);
        let tool_panel = self.tool_panel.view_with_config(sidebar_width, bg_weakest).map(AnsiEditorMessage::ToolPanel);

        let left_sidebar: iced::widget::Column<'_, AnsiEditorMessage> = column![palette_view, tool_panel,].spacing(4);

        // === TOP TOOLBAR (with color switcher on the left) ===
        // Get caret colors from the edit state
        let (caret_fg, caret_bg) = {
            let mut screen_guard = self.screen.lock();
            let state = screen_guard
                .as_any_mut()
                .downcast_mut::<EditState>()
                .expect("AnsiEditor screen should always be EditState");
            let caret = state.get_caret();
            (caret.attribute.get_foreground(), caret.attribute.get_background())
        };

        // Color switcher (classic icy_draw style) - shows caret's foreground/background colors
        let color_switcher = self.color_switcher.view(caret_fg, caret_bg).map(AnsiEditorMessage::ColorSwitcher);

        let top_toolbar_content = self.top_toolbar.view(self.current_tool).map(AnsiEditorMessage::TopToolbar);

        let toolbar_height = SWITCHER_SIZE;

        let top_toolbar = row![color_switcher, top_toolbar_content,].spacing(4);

        // === CENTER: Canvas ===
        let canvas = self.canvas.view().map(AnsiEditorMessage::Canvas);

        // === RIGHT PANEL ===
        let right_panel = self.right_panel.view(&self.screen, self.current_tool).map(AnsiEditorMessage::RightPanel);

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
        let palette = self.with_edit_state(|state| state.get_buffer().palette.clone());
        self.palette_grid.sync_palette(&palette);
        self.color_switcher.sync_palette(&palette);
    }

    /// Get status bar information for this editor
    pub fn status_info(&self) -> AnsiStatusInfo {
        let mut screen = self.screen.lock();
        let state = screen
            .as_any_mut()
            .downcast_mut::<EditState>()
            .expect("AnsiEditor screen should always be EditState");
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
