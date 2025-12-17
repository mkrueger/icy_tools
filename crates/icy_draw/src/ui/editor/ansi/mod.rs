//! ANSI Editor Mode
//!
//! This module contains the main ANSI art editor with:
//! - Left sidebar: Color switcher, Palette, Tools
//! - Top toolbar: Tool-specific options
//! - Center: Terminal/Canvas view
//! - Right panel: Minimap, Layers, Channels
//!
//! # Important: Editing Buffer State
//!
//! **Use `icy_engine_edit::EditState` functions for all buffer modifications.**
//! These functions generate proper undo actions.
//!
//! **DO NOT ALTER BUFFER OR SCREEN STATE DIRECTLY!!!**
//!
//! ## Examples:
//! - Font changes: Use `state.set_font()` or `state.set_font_in_slot(slot, font)`
//! - Layer operations: Use `state.add_layer()`, `state.remove_layer()`, etc.
//! - Character changes: Use `state.set_char()`, etc.
//! - Selection operations: Use corresponding EditState methods
//!
//! Direct buffer modifications bypass the undo system and will cause
//! inconsistent state when users try to undo/redo.

pub mod constants;
pub mod dialog;
pub mod main_area;
pub mod selection_drag;
mod shape_points;
pub mod tools;
pub mod widget;

pub use selection_drag::SelectionDrag;

use dialog::tag::TagDialogMessage;
use dialog::tag_list::TagListDialogMessage;

pub use dialog::edit_layer::*;
pub use dialog::file_settings::*;
pub use dialog::font_selector::*;
pub use dialog::font_slot_manager::*;
pub use dialog::reference_image::*;
pub use dialog::tdf_font_selector::{TdfFontSelectorDialog, TdfFontSelectorMessage};

pub use main_area::AnsiEditorMainArea;

pub use widget::canvas::*;
pub use widget::char_selector::*;
pub use widget::color_switcher::gpu::*;
pub use widget::fkey_toolbar::gpu::*;
pub use widget::layer_view::*;
pub use widget::minimap::*;
pub use widget::palette_grid::*;
pub use widget::right_panel::*;
pub use widget::toolbar::tool_panel_wrapper::{ToolPanel, ToolPanelMessage};
pub use widget::toolbar::top::*;

use icy_engine_edit::EditState;
use icy_engine_edit::tools::Tool;

use std::path::PathBuf;
use std::sync::Arc;

use clipboard_rs::{Clipboard, ClipboardContent};
use iced::{Element, Length, Task};
use icy_engine::formats::FileFormat;
use icy_engine::{MouseButton, Screen, TextBuffer, TextPane};
use icy_engine_gui::{ICY_CLIPBOARD_TYPE, TerminalMessage};
use parking_lot::{Mutex, RwLock};

use crate::SharedFontLibrary;
use crate::ui::Options;

/// Target for the character selector popup
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharSelectorTarget {
    /// Editing an F-key slot (0-11)
    FKeySlot(usize),
    /// Editing the brush paint character
    BrushChar,
}

/// Messages for the ANSI editor
use widget::outline_selector::{OutlineSelector, OutlineSelectorMessage, outline_selector_width};

#[derive(Clone, Debug)]
pub enum AnsiEditorMessage {
    /// Tool panel messages
    ToolPanel(ToolPanelMessage),
    /// Canvas view messages  
    Canvas(TerminalMessage),
    /// Right panel messages (minimap, layers, etc.)
    RightPanel(RightPanelMessage),
    /// Top toolbar messages
    TopToolbar(TopToolbarMessage),
    /// Tool-owned toolbar/options/status messages
    ToolMessage(tools::ToolMessage),
    /// Char selector popup messages (F-key character selection)
    CharSelector(CharSelectorMessage),
    /// Outline selector popup messages (font tool outline style)
    OutlineSelector(OutlineSelectorMessage),
    /// Color switcher messages
    ColorSwitcher(ColorSwitcherMessage),
    /// Palette grid messages
    PaletteGrid(PaletteGridMessage),
    /// Periodic tick while minimap drag is active (drives drag-out autoscroll).
    MinimapAutoscrollTick(f32),
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
    /// Open layer properties dialog
    EditLayer(usize),
    /// Duplicate a layer
    DuplicateLayer(usize),
    /// Merge layer down
    MergeLayerDown(usize),
    /// Clear layer contents
    ClearLayer(usize),
    /// Scroll viewport
    ScrollViewport(f32, f32),

    // === Marker/Guide Messages ===
    /// Set guide position (in characters, e.g. 80x25)
    /// Use (0, 0) or negative values to clear
    SetGuide(i32, i32),
    /// Clear guide
    ClearGuide,
    /// Set raster/grid spacing (in characters, e.g. 8x8)
    /// Use (0, 0) or negative values to clear
    SetRaster(i32, i32),
    /// Clear raster/grid
    ClearRaster,
    /// Toggle guide visibility
    ToggleGuide,
    /// Toggle raster/grid visibility
    ToggleRaster,
    /// Toggle line numbers display
    ToggleLineNumbers,
    /// Toggle layer borders display
    ToggleLayerBorders,

    /// Tag config dialog messages
    TagDialog(TagDialogMessage),

    /// Open the tag list dialog
    OpenTagListDialog,
    /// Tag list dialog messages
    TagListDialog(TagListDialogMessage),
}

// CanvasMouseEvent removed - use TerminalMessage directly from icy_engine_gui

/// Core ANSI editor logic/state (tools, dispatching, canvas, etc.).
///
/// This is intentionally not exposed publicly; the public entrypoint is
/// `AnsiEditorMainArea`, which owns the UI chrome/panels and delegates into this.
struct AnsiEditor {
    /// The screen (contains EditState which wraps buffer, caret, undo stack, etc.)
    /// Use screen.lock().as_any_mut().downcast_mut::<EditState>() to access EditState methods
    pub screen: Arc<Mutex<Box<dyn Screen>>>,
    /// Current active tool
    pub current_tool: Tool,
    /// Top toolbar (tool-specific options)
    pub top_toolbar: TopToolbar,
    /// Color switcher (FG/BG display)
    pub color_switcher: ColorSwitcher,
    /// Canvas view state
    pub canvas: CanvasView,
    /// Shared options
    pub options: Arc<RwLock<Options>>,
    /// Whether the document is modified
    pub is_modified: bool,

    /// While Some, the minimap is being dragged. Stores last pointer position relative to minimap
    /// bounds (may be outside) to simulate egui-style continuous drag updates.
    minimap_drag_pointer: Option<(f32, f32)>,

    // === Selection/Drag State ===
    /// Whether mouse is currently dragging
    pub is_dragging: bool,
    /// Tool that currently has mouse capture during a drag (move/up are routed here)
    mouse_capture_tool: Option<MouseCaptureTarget>,
    /// Current selection drag mode
    pub selection_drag: SelectionDrag,
    /// Selection state at start of drag (for resize operations)
    pub start_selection: Option<icy_engine::Rectangle>,

    // === Marker/Guide State ===
    /// Guide position in characters (e.g. 80x25 for a smallscale boundary)
    /// None = guide disabled
    pub guide: Option<(f32, f32)>,
    /// Whether guide is currently visible
    pub show_guide: bool,
    /// Raster/grid spacing in characters (e.g. 8x8)
    /// None = raster disabled
    pub raster: Option<(f32, f32)>,
    /// Whether raster is currently visible
    pub show_raster: bool,
    /// Whether line numbers are shown at the edges
    pub show_line_numbers: bool,
    /// Whether layer borders are shown
    pub show_layer_borders: bool,

    // === Character Selector State ===
    /// If Some, show the character selector popup for the given target
    pub char_selector_target: Option<CharSelectorTarget>,

    // === Tag Tool State ===
    /// Consolidated tag tool state (dialogs, drag, selection, context menu)
    pub tag_state: tools::TagToolState,

    // === Paint Stroke State (Pencil/Brush/Erase) ===
    paint_button: MouseButton,

    // === Shape Tool State ===
    /// If true, shape tools clear/erase instead of drawing (Moebius-style shift behavior).
    shape_clear: bool,

    // === Tool Handler System ===
    /// Tool handler for Pipette
    pipette_handler: tools::PipetteTool,
    /// Tool handler for Select
    select_handler: tools::SelectTool,
    /// Tool handler for Pencil
    pencil_handler: tools::PencilTool,
    /// Tool handler for Shape tools (Line, Rectangle, Ellipse)
    shape_handler: tools::ShapeTool,
    /// Tool handler for Fill
    fill_handler: tools::FillTool,
    /// Tool handler for Click/Text
    click_handler: tools::ClickTool,
    /// Tool handler for Font
    font_handler: tools::FontTool,
    /// Tool handler for Tag
    tag_handler: tools::TagTool,
    /// Tool handler for Paste/Floating layer
    paste_handler: tools::PasteTool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MouseCaptureTarget {
    Tool(Tool),
    Paste,
}

impl AnsiEditor {
    /// Renders the center canvas area (canvas + overlays).
    ///
    /// This is intentionally kept in the core editor so the surrounding layout/chrome
    /// can stay in `AnsiEditorMainArea`.
    pub(super) fn view<'a>(&'a self) -> Element<'a, AnsiEditorMessage> {
        // Canvas is created FIRST so Terminal's shader renders and populates the shared cache.
        let canvas = self.canvas.view().map(AnsiEditorMessage::Canvas);

        // Get scroll position from viewport for overlay positioning.
        let (scroll_x, scroll_y) = {
            let vp = self.canvas.terminal.viewport.read();
            (vp.scroll_x, vp.scroll_y)
        };

        // Get font dimensions for overlays.
        let (font_width, font_height) = {
            let screen = self.screen.lock();
            let size = screen.font_dimensions();
            (size.width as f32, size.height as f32)
        };

        // Caret + buffer dimensions for line numbers.
        let (caret_row, caret_col, buffer_height, buffer_width) = {
            let mut screen_guard = self.screen.lock();
            let state = screen_guard
                .as_any_mut()
                .downcast_mut::<EditState>()
                .expect("AnsiEditor screen should always be EditState");
            let caret = state.get_caret();
            let buffer = state.get_buffer();
            (caret.y as usize, caret.x as usize, buffer.height(), buffer.width() as usize)
        };

        // Build the center area with optional line numbers overlay and tag context menu.
        let mut center_layers: Vec<Element<'_, AnsiEditorMessage>> = vec![iced::widget::container(canvas).width(Length::Fill).height(Length::Fill).into()];

        if self.show_line_numbers {
            let line_numbers_overlay = widget::line_numbers::line_numbers_overlay(
                self.canvas.terminal.render_info.clone(),
                buffer_width,
                buffer_height as usize,
                font_width,
                font_height,
                caret_row,
                caret_col,
                scroll_x,
                scroll_y,
            );
            center_layers.push(line_numbers_overlay);
        }

        // Add tag context menu overlay if active.
        if self.tag_state.has_context_menu() {
            let display_scale = self.canvas.terminal.render_info.read().display_scale;
            if let Some(context_menu) = self
                .tag_state
                .view_context_menu_overlay(font_width, font_height, scroll_x, scroll_y, display_scale)
            {
                center_layers.push(context_menu.map(AnsiEditorMessage::ToolMessage));
            }
        }

        iced::widget::stack(center_layers).width(Length::Fill).height(Length::Fill).into()
    }

    // NOTE (Layer-local coordinates)
    // ============================
    // The terminal/canvas events provide positions in *document* coordinates.
    // All *painting* operations (Brush/Pencil/Erase/Shapes) are ALWAYS executed in
    // *layer-local* coordinates, i.e. relative to the current layer's offset.
    // Do NOT pass document/global positions into brush algorithms.
    // Selection/mask operations are handled by EditState and keep using document coords.

    /// Compute half-block coordinates from widget-local pixel position.
    /// Returns layer-local half-block coordinates (Y has 2x resolution).
    /// `pixel_position` is widget-local (relative to terminal bounds).
    fn compute_half_block_pos(&self, pixel_position: (f32, f32)) -> icy_engine::Position {
        let render_info = self.canvas.terminal.render_info.read();
        let viewport = self.canvas.terminal.viewport.read();

        // Convert widget-local to screen coordinates (RenderInfo methods expect screen coords)
        let screen_x = render_info.bounds_x + pixel_position.0;
        let screen_y = render_info.bounds_y + pixel_position.1;

        // Get visible half-block coordinates (without scroll offset)
        let (cell_x, half_block_y) = render_info.screen_to_half_block_cell_unclamped(screen_x, screen_y);

        // scroll_x is in content coordinates - convert to columns
        let font_width = render_info.font_width.max(1.0);
        let scroll_offset_cols = (viewport.scroll_x / font_width).floor() as i32;

        // scroll_y is in content coordinates - convert to half-block lines (2x)
        let font_height = render_info.font_height.max(1.0);
        let scroll_offset_half_lines = (viewport.scroll_y / font_height * 2.0).floor() as i32;

        // Get absolute half-block coordinates (with scroll offset)
        let abs_half_block = icy_engine::Position::new(cell_x + scroll_offset_cols, half_block_y + scroll_offset_half_lines);

        // Convert to layer-local coordinates
        // In half-block space, layer Y offset is also doubled
        let layer_offset = self.with_edit_state_readonly(|state| {
            if let Some(layer) = state.get_cur_layer() {
                let offset = layer.offset();
                icy_engine::Position::new(offset.x, offset.y * 2)
            } else {
                icy_engine::Position::default()
            }
        });

        abs_half_block - layer_offset
    }

    /// Helper to access EditState without mutable borrow (uses shared lock internally)
    fn with_edit_state_readonly<R, F: FnOnce(&icy_engine_edit::EditState) -> R>(&self, f: F) -> R {
        let mut screen = self.screen.lock();
        let edit_state = screen
            .as_any_mut()
            .downcast_mut::<icy_engine_edit::EditState>()
            .expect("screen should be EditState");
        f(edit_state)
    }

    // =========================================================================
    // Tool Handler Dispatch (new trait-based system)
    // =========================================================================

    fn build_half_block_mapper(&self, state: &EditState) -> tools::HalfBlockMapper {
        let render_info = self.canvas.terminal.render_info.read();
        let viewport = self.canvas.terminal.viewport.read();

        let layer_offset = state
            .get_cur_layer()
            .map(|l| {
                let o = l.offset();
                icy_engine::Position::new(o.x, o.y * 2)
            })
            .unwrap_or_default();

        tools::HalfBlockMapper {
            bounds_x: render_info.bounds_x,
            bounds_y: render_info.bounds_y,
            viewport_x: render_info.viewport_x,
            viewport_y: render_info.viewport_y,
            display_scale: render_info.display_scale,
            scan_lines: render_info.scan_lines,
            font_width: render_info.font_width,
            font_height: render_info.font_height,
            scroll_x: viewport.scroll_x,
            scroll_y: viewport.scroll_y,
            layer_offset,
        }
    }

    fn dispatch_pipette_terminal_message(&mut self, msg: &TerminalMessage) -> tools::ToolResult {
        use tools::ToolHandler;

        let mut screen_guard = self.screen.lock();
        let state = screen_guard.as_any_mut().downcast_mut::<EditState>().unwrap();

        let mut undo_guard = None;
        let half_block_mapper = Some(self.build_half_block_mapper(state));
        let mut ctx = tools::ToolContext {
            state,
            options: Some(&self.options),
            undo_guard: &mut undo_guard,
            half_block_mapper,
            tag_state: None,
        };

        let result = self.pipette_handler.handle_terminal_message(&mut ctx, msg);
        drop(screen_guard);
        self.process_tool_result_from(MouseCaptureTarget::Tool(Tool::Pipette), result)
    }

    fn dispatch_select_event(&mut self, event: &iced::Event) -> tools::ToolResult {
        use tools::ToolHandler;

        let mut screen_guard = self.screen.lock();
        let state = screen_guard.as_any_mut().downcast_mut::<EditState>().unwrap();

        let mut undo_guard = None;
        let half_block_mapper = Some(self.build_half_block_mapper(state));
        let mut ctx = tools::ToolContext {
            state,
            options: Some(&self.options),
            undo_guard: &mut undo_guard,
            half_block_mapper,
            tag_state: None,
        };

        let result = self.select_handler.handle_event(&mut ctx, event);
        drop(screen_guard);
        self.process_tool_result_from(MouseCaptureTarget::Tool(Tool::Select), result)
    }

    fn dispatch_select_terminal_message(&mut self, msg: &TerminalMessage) -> tools::ToolResult {
        use tools::ToolHandler;

        let mut screen_guard = self.screen.lock();
        let state = screen_guard.as_any_mut().downcast_mut::<EditState>().unwrap();

        let mut undo_guard = None;
        let half_block_mapper = Some(self.build_half_block_mapper(state));
        let mut ctx = tools::ToolContext {
            state,
            options: Some(&self.options),
            undo_guard: &mut undo_guard,
            half_block_mapper,
            tag_state: None,
        };

        let result = self.select_handler.handle_terminal_message(&mut ctx, msg);
        drop(screen_guard);
        self.process_tool_result_from(MouseCaptureTarget::Tool(Tool::Select), result)
    }

    fn dispatch_pencil_terminal_message(&mut self, msg: &TerminalMessage) -> tools::ToolResult {
        use tools::ToolHandler;

        let mut screen_guard = self.screen.lock();
        let state = screen_guard.as_any_mut().downcast_mut::<EditState>().unwrap();

        let mut undo_guard = None;
        let half_block_mapper = Some(self.build_half_block_mapper(state));
        let mut ctx = tools::ToolContext {
            state,
            options: Some(&self.options),
            undo_guard: &mut undo_guard,
            half_block_mapper,
            tag_state: None,
        };

        let result = self.pencil_handler.handle_terminal_message(&mut ctx, msg);
        drop(screen_guard);
        self.process_tool_result_from(MouseCaptureTarget::Tool(Tool::Pencil), result)
    }

    fn dispatch_shape_terminal_message(&mut self, msg: &TerminalMessage) -> tools::ToolResult {
        use tools::ToolHandler;

        let mut screen_guard = self.screen.lock();
        let state = screen_guard.as_any_mut().downcast_mut::<EditState>().unwrap();

        let mut undo_guard = None;
        let half_block_mapper = Some(self.build_half_block_mapper(state));
        let mut ctx = tools::ToolContext {
            state,
            options: Some(&self.options),
            undo_guard: &mut undo_guard,
            half_block_mapper,
            tag_state: None,
        };

        self.shape_handler.set_tool(self.current_tool);

        let result = self.shape_handler.handle_terminal_message(&mut ctx, msg);
        drop(screen_guard);
        self.process_tool_result_from(MouseCaptureTarget::Tool(self.current_tool), result)
    }

    fn dispatch_fill_terminal_message(&mut self, msg: &TerminalMessage) -> tools::ToolResult {
        use tools::ToolHandler;

        let mut screen_guard = self.screen.lock();
        let state = screen_guard.as_any_mut().downcast_mut::<EditState>().unwrap();

        let mut undo_guard = None;
        let half_block_mapper = Some(self.build_half_block_mapper(state));
        let mut ctx = tools::ToolContext {
            state,
            options: Some(&self.options),
            undo_guard: &mut undo_guard,
            half_block_mapper,
            tag_state: None,
        };

        let result = self.fill_handler.handle_terminal_message(&mut ctx, msg);
        drop(screen_guard);
        self.process_tool_result_from(MouseCaptureTarget::Tool(Tool::Fill), result)
    }

    fn dispatch_click_terminal_message(&mut self, source_tool: Tool, msg: &TerminalMessage) -> tools::ToolResult {
        use tools::ToolHandler;

        let mut screen_guard = self.screen.lock();
        let state = screen_guard.as_any_mut().downcast_mut::<EditState>().unwrap();

        let mut undo_guard = None;
        let half_block_mapper = Some(self.build_half_block_mapper(state));
        let mut ctx = tools::ToolContext {
            state,
            options: Some(&self.options),
            undo_guard: &mut undo_guard,
            half_block_mapper,
            tag_state: None,
        };

        let result = self.click_handler.handle_terminal_message(&mut ctx, msg);
        drop(screen_guard);
        self.process_tool_result_from(MouseCaptureTarget::Tool(source_tool), result)
    }

    fn dispatch_click_event(&mut self, event: &iced::Event) -> tools::ToolResult {
        use tools::ToolHandler;

        let mut screen_guard = self.screen.lock();
        let state = screen_guard.as_any_mut().downcast_mut::<EditState>().unwrap();

        let mut undo_guard = None;
        let half_block_mapper = Some(self.build_half_block_mapper(state));
        let mut ctx = tools::ToolContext {
            state,
            options: Some(&self.options),
            undo_guard: &mut undo_guard,
            half_block_mapper,
            tag_state: None,
        };

        let result = self.click_handler.handle_event(&mut ctx, event);
        drop(screen_guard);
        self.process_tool_result_from(MouseCaptureTarget::Tool(Tool::Click), result)
    }

    fn dispatch_font_event(&mut self, event: &iced::Event) -> tools::ToolResult {
        use tools::ToolHandler;

        let mut screen_guard = self.screen.lock();
        let state = screen_guard.as_any_mut().downcast_mut::<EditState>().unwrap();

        let mut undo_guard = None;
        let half_block_mapper = Some(self.build_half_block_mapper(state));
        let mut ctx = tools::ToolContext {
            state,
            options: Some(&self.options),
            undo_guard: &mut undo_guard,
            half_block_mapper,
            tag_state: None,
        };

        let result = self.font_handler.handle_event(&mut ctx, event);
        drop(screen_guard);
        self.process_tool_result_from(MouseCaptureTarget::Tool(Tool::Font), result)
    }

    fn dispatch_tag_event(&mut self, event: &iced::Event) -> tools::ToolResult {
        use tools::ToolHandler;

        let mut screen_guard = self.screen.lock();
        let state = screen_guard.as_any_mut().downcast_mut::<EditState>().unwrap();

        let mut undo_guard = None;
        let half_block_mapper = Some(self.build_half_block_mapper(state));
        let mut ctx = tools::ToolContext {
            state,
            options: Some(&self.options),
            undo_guard: &mut undo_guard,
            half_block_mapper,
            tag_state: Some(&mut self.tag_state),
        };

        let result = self.tag_handler.handle_event(&mut ctx, event);
        drop(screen_guard);
        self.process_tool_result(result)
    }

    fn dispatch_tag_terminal_message(&mut self, msg: &TerminalMessage) -> tools::ToolResult {
        use tools::ToolHandler;

        let mut screen_guard = self.screen.lock();
        let state = screen_guard.as_any_mut().downcast_mut::<EditState>().unwrap();

        let mut undo_guard = None;
        let half_block_mapper = Some(self.build_half_block_mapper(state));
        let mut ctx = tools::ToolContext {
            state,
            options: Some(&self.options),
            undo_guard: &mut undo_guard,
            half_block_mapper,
            tag_state: Some(&mut self.tag_state),
        };

        let result = self.tag_handler.handle_terminal_message(&mut ctx, msg);
        drop(screen_guard);
        self.process_tool_result_from(MouseCaptureTarget::Tool(Tool::Tag), result)
    }

    fn dispatch_paste_terminal_message(&mut self, msg: &TerminalMessage) -> tools::ToolResult {
        use tools::ToolHandler;

        let mut screen_guard = self.screen.lock();
        let state = screen_guard.as_any_mut().downcast_mut::<EditState>().unwrap();

        let mut undo_guard = None;
        let half_block_mapper = Some(self.build_half_block_mapper(state));
        let mut ctx = tools::ToolContext {
            state,
            options: Some(&self.options),
            undo_guard: &mut undo_guard,
            half_block_mapper,
            tag_state: None,
        };

        let result = self.paste_handler.handle_terminal_message(&mut ctx, msg);
        drop(screen_guard);
        self.process_tool_result_from(MouseCaptureTarget::Paste, result)
    }

    fn dispatch_paste_event(&mut self, event: &iced::Event) -> tools::ToolResult {
        use tools::ToolHandler;

        let mut screen_guard = self.screen.lock();
        let state = screen_guard.as_any_mut().downcast_mut::<EditState>().unwrap();

        let mut undo_guard = None;
        let half_block_mapper = Some(self.build_half_block_mapper(state));
        let mut ctx = tools::ToolContext {
            state,
            options: Some(&self.options),
            undo_guard: &mut undo_guard,
            half_block_mapper,
            tag_state: None,
        };

        let result = self.paste_handler.handle_event(&mut ctx, event);
        drop(screen_guard);
        self.process_tool_result_from(MouseCaptureTarget::Paste, result)
    }

    fn dispatch_paste_action(&mut self, action: tools::PasteAction) -> tools::ToolResult {
        let mut screen_guard = self.screen.lock();
        let state = screen_guard.as_any_mut().downcast_mut::<EditState>().unwrap();

        let result = self.paste_handler.perform_action(state, action);
        drop(screen_guard);
        self.process_tool_result_from(MouseCaptureTarget::Paste, result)
    }

    /// Process a ToolResult and perform editor-side effects.
    ///
    /// `source` is used to attribute mouse capture correctly (e.g. paste mode has its own capture).
    fn process_tool_result_from(&mut self, source: MouseCaptureTarget, result: tools::ToolResult) -> tools::ToolResult {
        use tools::ToolResult;

        match result {
            ToolResult::None => ToolResult::None,
            ToolResult::Redraw => ToolResult::Redraw,
            ToolResult::Commit(msg) => {
                self.is_modified = true;
                ToolResult::Commit(msg)
            }
            ToolResult::Status(msg) => ToolResult::Status(msg),
            ToolResult::UpdateLayerBounds => {
                self.update_layer_bounds();
                ToolResult::None
            }
            ToolResult::SwitchTool(tool) => {
                self.change_tool(tool);
                ToolResult::Redraw
            }
            ToolResult::StartCapture => {
                self.mouse_capture_tool = Some(source);
                self.is_dragging = true;
                ToolResult::None
            }
            ToolResult::EndCapture => {
                self.mouse_capture_tool = None;
                self.is_dragging = false;
                ToolResult::None
            }
            ToolResult::SetCursorIcon(icon) => {
                *self.canvas.terminal.cursor_icon.write() = icon;
                ToolResult::None
            }
            ToolResult::Multi(results) => {
                let mut last_result = ToolResult::None;
                for r in results {
                    last_result = self.process_tool_result_from(source, r);
                }
                last_result
            }
        }
    }

    /// Backwards-compatible entrypoint when the source is the currently active editor tool.
    fn process_tool_result(&mut self, result: tools::ToolResult) -> tools::ToolResult {
        self.process_tool_result_from(MouseCaptureTarget::Tool(self.current_tool), result)
    }

    /// Construct the core editor from a buffer.
    ///
    /// Returns (editor, palette, format_mode) so the public wrapper can initialize
    /// palette-dependent UI widgets.
    fn from_buffer_inner(
        buffer: TextBuffer,
        options: Arc<RwLock<Options>>,
        font_library: SharedFontLibrary,
    ) -> (Self, icy_engine::Palette, icy_engine_edit::FormatMode) {
        // Clone the palette before moving buffer into EditState
        let palette = buffer.palette.clone();
        let format_mode = icy_engine_edit::FormatMode::from_buffer(&buffer);

        // Create EditState and wrap as Box<dyn Screen> for Terminal compatibility
        let edit_state = EditState::from_buffer(buffer);
        let screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(edit_state)));

        // Initialize outline style from shared settings
        let outline_style = { *options.read().font_outline_style.read() };
        {
            let mut guard = screen.lock();
            if let Some(state) = guard.as_any_mut().downcast_mut::<EditState>() {
                state.set_outline_style(outline_style);
            }
        }

        let mut color_switcher = ColorSwitcher::new();
        color_switcher.sync_palette(&palette);

        // Create canvas with cloned Arc to screen + shared monitor settings
        let shared_monitor_settings = { options.read().monitor_settings.clone() };
        let mut canvas = CanvasView::new(screen.clone(), shared_monitor_settings);
        // Enable caret blinking by default (Click tool is the default)
        canvas.set_has_focus(true);

        let initial_fkey_set = {
            let mut opts = options.write();
            opts.fkeys.clamp_current_set();
            opts.fkeys.current_set
        };

        let top_toolbar = TopToolbar::new();

        let mut click_handler = tools::ClickTool::new();
        // Keep the Click tool in sync with persisted FKey set selection.
        let _ = initial_fkey_set;
        click_handler.sync_fkey_set_from_options(&options);

        let editor = Self {
            screen,
            current_tool: Tool::Click,
            top_toolbar,
            color_switcher,
            canvas,
            options,
            is_modified: false,

            minimap_drag_pointer: None,
            // Selection/drag state
            is_dragging: false,
            mouse_capture_tool: None,
            selection_drag: SelectionDrag::None,
            start_selection: None,
            // Marker/guide state - disabled by default
            guide: None,
            show_guide: false,
            raster: None,
            show_raster: false,
            show_line_numbers: false,
            show_layer_borders: false,

            char_selector_target: None,

            tag_state: tools::TagToolState::new(),

            paint_button: MouseButton::Left,

            shape_clear: false,

            // Tool handler system
            pipette_handler: tools::PipetteTool::new(),
            select_handler: tools::SelectTool::new(),
            pencil_handler: tools::PencilTool::new(),
            shape_handler: tools::ShapeTool::new(),
            fill_handler: tools::FillTool::new(),
            click_handler,
            font_handler: tools::FontTool::new(font_library),
            tag_handler: tools::TagTool::new(),
            paste_handler: tools::PasteTool::new(),
        };

        (editor, palette, format_mode)
    }

    fn clear_tool_overlay(&mut self) {
        self.canvas.set_tool_overlay_mask(None, None);
    }

    fn task_none_with_markers_update(&mut self) -> Task<AnsiEditorMessage> {
        self.update_markers();
        Task::none()
    }

    fn task_none_with_layer_bounds_update(&mut self) -> Task<AnsiEditorMessage> {
        self.update_layer_bounds();
        Task::none()
    }

    fn apply_tag_tool_result(&mut self, result: tools::ToolResult) -> Task<AnsiEditorMessage> {
        let processed = self.process_tool_result_from(MouseCaptureTarget::Tool(Tool::Tag), result);

        if !matches!(processed, tools::ToolResult::None) {
            self.update_tag_overlays();
            if self.tag_state.add_new_index.is_none() && !self.tag_state.selection_drag_active {
                self.clear_tool_overlay();
            }
        }

        Task::none()
    }

    fn end_drag_capture(&mut self) {
        self.is_dragging = false;
        self.mouse_capture_tool = None;
        self.selection_drag = SelectionDrag::None;
        self.start_selection = None;
    }

    fn cancel_shape_drag(&mut self) -> bool {
        if self.is_dragging
            && matches!(
                self.current_tool,
                Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled
            )
        {
            self.end_drag_capture();
            self.paint_button = MouseButton::Left;
            self.shape_clear = false;
            self.clear_tool_overlay();
            return true;
        }
        false
    }

    fn set_current_fkey_set(&mut self, set_idx: usize) {
        // F-key set switching is owned by ClickTool.
        self.click_handler.set_current_fkey_set(&self.options, set_idx);
    }

    fn type_fkey_slot(&mut self, slot: usize) -> tools::ToolResult {
        let set_idx = self.click_handler.current_fkey_set();

        let mut screen_guard = self.screen.lock();
        if let Some(state) = screen_guard.as_any_mut().downcast_mut::<EditState>() {
            let mut undo_guard = None;
            let mut ctx = tools::ToolContext {
                state,
                options: Some(&self.options),
                undo_guard: &mut undo_guard,
                half_block_mapper: None,
                tag_state: None,
            };
            let result = self.click_handler.type_fkey_slot(&mut ctx, set_idx, slot);
            drop(screen_guard);
            return self.process_tool_result_from(MouseCaptureTarget::Tool(Tool::Click), result);
        }

        tools::ToolResult::None
    }

    /// Access the EditState via downcast from the Screen trait object
    /// Panics if the screen is not an EditState (should never happen in AnsiEditor)
    pub(crate) fn with_edit_state<T, F: FnOnce(&mut EditState) -> T>(&mut self, f: F) -> T {
        let mut screen = self.screen.lock();
        let edit_state = screen
            .as_any_mut()
            .downcast_mut::<EditState>()
            .expect("AnsiEditor screen should always be EditState");
        f(edit_state)
    }

    fn with_edit_state_and_tag_state<T, F: FnOnce(&mut EditState, &mut tools::TagToolState) -> T>(&mut self, f: F) -> T {
        let mut screen = self.screen.lock();
        let edit_state = screen
            .as_any_mut()
            .downcast_mut::<EditState>()
            .expect("AnsiEditor screen should always be EditState");
        f(edit_state, &mut self.tag_state)
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

    // ========================================================================
    // Clipboard operations
    // ========================================================================

    /// Check if cut operation is available (selection exists)
    #[allow(dead_code)]
    pub fn can_cut(&self) -> bool {
        self.with_edit_state_readonly(|state| state.selection().is_some())
    }

    /// Cut selection to clipboard
    pub fn cut(&mut self) -> Result<(), String> {
        self.copy_without_deselect()?;
        let mut screen = self.screen.lock();
        if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
            edit_state.erase_selection().map_err(|e| e.to_string())?;
            let _ = edit_state.clear_selection();
        }
        Ok(())
    }

    /// Check if copy operation is available (selection exists)
    #[allow(dead_code)]
    pub fn can_copy(&self) -> bool {
        self.with_edit_state_readonly(|state| state.selection().is_some())
    }

    /// Copy selection to clipboard in multiple formats (ICY, RTF, Text)
    pub fn copy(&mut self) -> Result<(), String> {
        self.copy_without_deselect()?;
        // Clear selection after copy
        let mut screen = self.screen.lock();
        if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
            let _ = edit_state.clear_selection();
        }
        Ok(())
    }

    /// Copy selection to clipboard without clearing the selection
    /// Used internally by cut() which handles its own selection clearing
    fn copy_without_deselect(&mut self) -> Result<(), String> {
        let mut screen = self.screen.lock();
        let edit_state = screen
            .as_any_mut()
            .downcast_mut::<EditState>()
            .ok_or_else(|| "Could not access edit state".to_string())?;

        let mut contents = Vec::new();

        // Debug: log selection state
        log::debug!("copy_without_deselect: selection={:?}", edit_state.selection());

        // Plain text (required - if no text, nothing to copy)
        let text = match edit_state.copy_text() {
            Some(t) => t,
            None => return Err("No selection to copy".to_string()),
        };

        // ICY binary format (for paste between ICY applications)
        if let Some(data) = edit_state.clipboard_data() {
            log::debug!("copy_without_deselect: ICY data size={}", data.len());
            contents.push(ClipboardContent::Other(ICY_CLIPBOARD_TYPE.into(), data));
        } else {
            log::warn!("copy_without_deselect: No ICY clipboard data generated");
        }

        // RTF (rich text with colors)
        if let Some(rich_text) = edit_state.copy_rich_text() {
            contents.push(ClipboardContent::Rtf(rich_text));
        }

        // Plain text - MUST be last on Windows
        contents.push(ClipboardContent::Text(text));

        // Set clipboard contents
        crate::CLIPBOARD_CONTEXT.set(contents).map_err(|e| format!("Failed to set clipboard: {e}"))?;

        Ok(())
    }

    /// Check if paste operation is available (clipboard has compatible content)
    #[allow(dead_code)]
    pub fn can_paste(&self) -> bool {
        self.paste_handler.can_paste()
    }

    /// Paste from clipboard (ICY format, image, or text)
    /// Creates a floating layer that can be positioned before anchoring
    pub fn paste(&mut self) -> Result<(), String> {
        // Don't paste if already in paste mode
        if self.is_paste_mode() {
            return Ok(());
        }

        let previous_tool = self.current_tool;

        let mut screen_guard = self.screen.lock();
        let state = screen_guard
            .as_any_mut()
            .downcast_mut::<EditState>()
            .ok_or_else(|| "Could not access edit state".to_string())?;

        let mut undo_guard = None;
        let half_block_mapper = Some(self.build_half_block_mapper(state));
        let mut ctx = tools::ToolContext {
            state,
            options: Some(&self.options),
            undo_guard: &mut undo_guard,
            half_block_mapper,
            tag_state: None,
        };

        let result = self.paste_handler.paste_from_clipboard(&mut ctx, previous_tool)?;
        drop(screen_guard);
        let _ = self.process_tool_result(result);

        Ok(())
    }

    pub fn font_tool_library(&self) -> SharedFontLibrary {
        self.font_handler.font_tool.font_library()
    }

    pub fn font_tool_select_font(&mut self, font_idx: i32) {
        self.font_handler.select_font(font_idx);
    }

    /// Check if we are in paste mode (floating layer active for positioning)
    /// This is the primary check for paste mode UI and input handling
    pub fn is_paste_mode(&self) -> bool {
        self.paste_handler.is_active()
    }

    /// Save the document to the given path
    pub fn save(&mut self, path: &std::path::Path) -> Result<(), String> {
        let mut screen = self.screen.lock();
        if let Some(edit_state) = screen.as_any_mut().downcast_ref::<EditState>() {
            // Determine format from extension
            let format = FileFormat::from_path(path).ok_or_else(|| "Unknown file format".to_string())?;

            // Get buffer and save with default options
            let buffer = edit_state.get_buffer();
            let options = icy_engine::AnsiSaveOptionsV2::default();
            let bytes = format.to_bytes(buffer, &options).map_err(|e| e.to_string())?;

            std::fs::write(path, bytes).map_err(|e| e.to_string())?;

            self.is_modified = false;
            Ok(())
        } else {
            Err("Could not access edit state".to_string())
        }
    }

    /// Check if this editor needs animation updates (for smooth animations)
    pub fn needs_animation(&self) -> bool {
        self.current_tool == Tool::Click || self.minimap_drag_pointer.is_some()
    }

    /// Get the current marker state for menu display
    pub fn get_marker_menu_state(&self) -> widget::toolbar::menu_bar::MarkerMenuState {
        widget::toolbar::menu_bar::MarkerMenuState {
            guide: self.guide.map(|(x, y)| (x as u32, y as u32)),
            guide_visible: self.show_guide,
            raster: self.raster.map(|(x, y)| (x as u32, y as u32)),
            raster_visible: self.show_raster,
            line_numbers_visible: self.show_line_numbers,
            layer_borders_visible: self.show_layer_borders,
        }
    }

    /// Get the current mirror mode state
    pub fn get_mirror_mode(&self) -> bool {
        let mut screen = self.screen.lock();
        if let Some(state) = screen.as_any_mut().downcast_ref::<EditState>() {
            state.get_mirror_mode()
        } else {
            false
        }
    }

    /// Toggle mirror mode
    pub fn toggle_mirror_mode(&mut self) {
        let mut screen = self.screen.lock();
        if let Some(state) = screen.as_any_mut().downcast_mut::<EditState>() {
            let current = state.get_mirror_mode();
            state.set_mirror_mode(!current);
        }
    }

    /// Compute viewport info for the minimap overlay
    /// Returns normalized coordinates (0.0-1.0) representing the visible area in the terminal
    fn compute_viewport_info(&self) -> ViewportInfo {
        // IMPORTANT: The terminal shader may clamp/fit the visible region (resolution/letterbox).
        // For a pixel-exact minimap overlay, use the effective values written by the shader.
        let cache = self.canvas.terminal.render_cache.read();
        widget::minimap::viewport_info_from_effective_view(
            cache.content_width as f32,
            cache.content_height,
            cache.visible_width,
            cache.visible_height,
            cache.scroll_offset_x,
            cache.scroll_offset_y,
        )
    }

    /// Scroll the canvas to a normalized position (0.0-1.0)
    /// The viewport will be centered on this position
    fn scroll_canvas_to_normalized(&mut self, norm_x: f32, norm_y: f32) {
        let cache = self.canvas.terminal.render_cache.read();
        let content_width = (cache.content_width as f32).max(1.0);
        let content_height = cache.content_height.max(1.0);
        let visible_width = cache.visible_width.max(1.0);
        let visible_height = cache.visible_height.max(1.0);
        drop(cache);

        // Keep current X when horizontal scrolling isn't possible.
        let current_scroll_x = self.canvas.terminal.viewport.read().scroll_x;

        let target_x = if content_width > visible_width {
            norm_x * content_width - visible_width / 2.0
        } else {
            current_scroll_x
        };

        // Convert normalized position to content coordinates
        // Center the viewport on the clicked position
        let target_y = norm_y * content_height - visible_height / 2.0;

        // Scroll to the target position (clamping is done internally)
        self.canvas.scroll_to(target_x, target_y);
    }

    /// Update the editor state
    pub fn update(&mut self, message: AnsiEditorMessage) -> Task<AnsiEditorMessage> {
        match message {
            AnsiEditorMessage::OpenTagListDialog => {
                self.with_edit_state_and_tag_state(|state, tag_state| {
                    tag_state.open_list_dialog(state);
                });
                Task::none()
            }
            AnsiEditorMessage::TagListDialog(msg) => {
                let result = self.with_edit_state_and_tag_state(|state, tag_state| tag_state.handle_list_dialog_message(state, msg));
                self.apply_tag_tool_result(result)
            }
            AnsiEditorMessage::TagDialog(msg) => {
                let result = self.with_edit_state_and_tag_state(|state, tag_state| tag_state.handle_dialog_message(state, msg));
                self.apply_tag_tool_result(result)
            }
            AnsiEditorMessage::ToolPanel(msg) => {
                // Handled by AnsiEditorMainArea.
                let _ = msg;
                Task::none()
            }
            AnsiEditorMessage::Canvas(msg) => {
                // Forward terminal mouse events directly to tool handling
                self.handle_terminal_mouse_event(&msg);
                self.canvas.update(msg).map(AnsiEditorMessage::Canvas)
            }
            AnsiEditorMessage::RightPanel(msg) => {
                // Handled by AnsiEditorMainArea.
                let _ = msg;
                Task::none()
            }
            AnsiEditorMessage::TopToolbar(msg) => {
                // Intercept brush char-table requests here (keeps the dialog local to the editor)
                match msg {
                    TopToolbarMessage::OpenBrushCharTable => {
                        self.char_selector_target = Some(CharSelectorTarget::BrushChar);
                        Task::none()
                    }
                    TopToolbarMessage::SetBrushChar(_) => {
                        // Selecting a character implicitly closes the overlay.
                        self.char_selector_target = None;
                        let task = self.top_toolbar.update(msg).map(AnsiEditorMessage::TopToolbar);
                        self.update_mouse_tracking_mode();
                        task
                    }
                    TopToolbarMessage::TypeFKey(slot) => {
                        let _ = self.type_fkey_slot(slot);
                        Task::none()
                    }
                    TopToolbarMessage::NextFKeyPage => {
                        let next = self.click_handler.current_fkey_set().saturating_add(1);
                        self.set_current_fkey_set(next);
                        Task::none()
                    }
                    TopToolbarMessage::PrevFKeyPage => {
                        let cur = self.click_handler.current_fkey_set();
                        let prev = {
                            let opts = self.options.read();
                            let count = opts.fkeys.set_count();
                            if count == 0 { 0 } else { (cur + count - 1) % count }
                        };
                        self.set_current_fkey_set(prev);
                        Task::none()
                    }
                    TopToolbarMessage::OpenFontDirectory => {
                        // Open the font directory in the system file manager
                        if let Some(font_dir) = Options::font_dir() {
                            // Create directory if it doesn't exist
                            if !font_dir.exists() {
                                let _ = std::fs::create_dir_all(&font_dir);
                            }
                            if let Err(e) = open::that(&font_dir) {
                                log::warn!("Failed to open font directory: {}", e);
                            }
                        }
                        Task::none()
                    }
                    TopToolbarMessage::SelectFont(index) => {
                        self.font_handler.select_font(index);
                        Task::none()
                    }
                    TopToolbarMessage::SelectOutline(index) => {
                        // Update outline style in options
                        *self.options.read().font_outline_style.write() = index;
                        Task::none()
                    }
                    TopToolbarMessage::OpenOutlineSelector => {
                        // Open the outline selector popup
                        self.font_handler.open_outline_selector();
                        Task::none()
                    }
                    TopToolbarMessage::OpenFontSelector => {
                        // This will be handled by main_window to open the dialog
                        // Return a task that signals this (handled via Message routing)
                        Task::none()
                    }
                    TopToolbarMessage::OpenTagList => {
                        // Route to tag tool
                        return self.update(AnsiEditorMessage::ToolMessage(tools::ToolMessage::TagOpenList));
                    }
                    TopToolbarMessage::StartAddTag => {
                        // Toggle add-tag mode
                        return self.update(AnsiEditorMessage::ToolMessage(tools::ToolMessage::TagStartAdd));
                    }
                    TopToolbarMessage::EditSelectedTag => {
                        // Edit the first selected tag
                        if let Some(&idx) = self.tag_state.selection.first() {
                            return self.update(AnsiEditorMessage::ToolMessage(tools::ToolMessage::TagEdit(idx)));
                        }
                        Task::none()
                    }
                    TopToolbarMessage::DeleteSelectedTags => {
                        // Delete all selected tags
                        if !self.tag_state.selection.is_empty() {
                            return self.update(AnsiEditorMessage::ToolMessage(tools::ToolMessage::TagDeleteSelected));
                        }
                        Task::none()
                    }
                    TopToolbarMessage::ToggleFilled(v) => {
                        // Keep the tool variant in sync with the filled toggle.
                        let new_tool = match self.current_tool {
                            Tool::RectangleOutline | Tool::RectangleFilled => {
                                if v {
                                    Tool::RectangleFilled
                                } else {
                                    Tool::RectangleOutline
                                }
                            }
                            Tool::EllipseOutline | Tool::EllipseFilled => {
                                if v {
                                    Tool::EllipseFilled
                                } else {
                                    Tool::EllipseOutline
                                }
                            }
                            other => other,
                        };

                        if new_tool != self.current_tool {
                            self.change_tool(new_tool);
                        }

                        let task = self.top_toolbar.update(TopToolbarMessage::ToggleFilled(v)).map(AnsiEditorMessage::TopToolbar);
                        self.update_mouse_tracking_mode();
                        task
                    }

                    // === Paste Mode Actions ===
                    TopToolbarMessage::PasteStamp => {
                        let _ = self.dispatch_paste_action(tools::PasteAction::Stamp);
                        Task::none()
                    }
                    TopToolbarMessage::PasteRotate => {
                        let _ = self.dispatch_paste_action(tools::PasteAction::Rotate);
                        Task::none()
                    }
                    TopToolbarMessage::PasteFlipX => {
                        let _ = self.dispatch_paste_action(tools::PasteAction::FlipX);
                        Task::none()
                    }
                    TopToolbarMessage::PasteFlipY => {
                        let _ = self.dispatch_paste_action(tools::PasteAction::FlipY);
                        Task::none()
                    }
                    TopToolbarMessage::PasteToggleTransparent => {
                        let _ = self.dispatch_paste_action(tools::PasteAction::ToggleTransparent);
                        Task::none()
                    }
                    TopToolbarMessage::PasteAnchor => {
                        let _ = self.dispatch_paste_action(tools::PasteAction::Anchor);
                        Task::none()
                    }
                    TopToolbarMessage::PasteCancel => {
                        let _ = self.dispatch_paste_action(tools::PasteAction::Discard);
                        Task::none()
                    }

                    _ => {
                        let task: Task<AnsiEditorMessage> = self.top_toolbar.update(msg).map(AnsiEditorMessage::TopToolbar);
                        self.update_mouse_tracking_mode();
                        task
                    }
                }
            }
            AnsiEditorMessage::ToolMessage(msg) => {
                use tools::ToolHandler;

                let paste_mode = self.is_paste_mode();
                let current_tool = self.current_tool;

                let mut screen_guard = self.screen.lock();
                if let Some(state) = screen_guard.as_any_mut().downcast_mut::<EditState>() {
                    let mut undo_guard = None;
                    let mut ctx = tools::ToolContext {
                        state,
                        options: Some(&self.options),
                        undo_guard: &mut undo_guard,
                        half_block_mapper: None,
                        tag_state: if matches!(current_tool, Tool::Tag) { Some(&mut self.tag_state) } else { None },
                    };

                    let (source, result) = if paste_mode {
                        (MouseCaptureTarget::Paste, self.paste_handler.handle_message(&mut ctx, &msg))
                    } else {
                        let r = match current_tool {
                            Tool::Click => self.click_handler.handle_message(&mut ctx, &msg),
                            Tool::Font => self.font_handler.handle_message(&mut ctx, &msg),
                            Tool::Pencil => self.pencil_handler.handle_message(&mut ctx, &msg),
                            Tool::Pipette => self.pipette_handler.handle_message(&mut ctx, &msg),
                            Tool::Select => self.select_handler.handle_message(&mut ctx, &msg),
                            Tool::Fill => self.fill_handler.handle_message(&mut ctx, &msg),
                            Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled => {
                                self.shape_handler.set_tool(self.current_tool);
                                self.shape_handler.handle_message(&mut ctx, &msg)
                            }
                            Tool::Tag => self.tag_handler.handle_message(&mut ctx, &msg),
                        };
                        (MouseCaptureTarget::Tool(current_tool), r)
                    };

                    drop(screen_guard);

                    // Tool may request editor-owned UI (e.g. open popups)
                    if paste_mode {
                        // Paste tool currently doesn't request editor-owned popups.
                    } else {
                        match current_tool {
                            Tool::Click => {
                                if let Some(tools::ClickToolUiAction::OpenCharSelectorForFKey(slot)) = self.click_handler.take_ui_action() {
                                    self.char_selector_target = Some(CharSelectorTarget::FKeySlot(slot));
                                }
                            }
                            Tool::Font => {
                                if let Some(action) = self.font_handler.take_ui_action() {
                                    let _ = self.process_tool_result_from(source, result);

                                    return match action {
                                        tools::FontToolUiAction::OpenTdfFontSelector => {
                                            Task::done(AnsiEditorMessage::TopToolbar(TopToolbarMessage::OpenFontSelector))
                                        }
                                        tools::FontToolUiAction::OpenFontDirectory => {
                                            Task::done(AnsiEditorMessage::TopToolbar(TopToolbarMessage::OpenFontDirectory))
                                        }
                                    };
                                }
                            }
                            Tool::Pencil => {
                                if self.pencil_handler.take_ui_action().is_some() {
                                    self.char_selector_target = Some(CharSelectorTarget::BrushChar);
                                }
                            }
                            Tool::Fill => {
                                if self.fill_handler.take_ui_action().is_some() {
                                    self.char_selector_target = Some(CharSelectorTarget::BrushChar);
                                }
                            }
                            Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled => {
                                if self.shape_handler.take_ui_action().is_some() {
                                    self.char_selector_target = Some(CharSelectorTarget::BrushChar);
                                }
                            }
                            _ => {}
                        }
                    }

                    let processed = self.process_tool_result_from(source, result);

                    if matches!(current_tool, Tool::Tag) && !matches!(processed, tools::ToolResult::None) {
                        self.update_tag_overlays();
                        if self.tag_state.add_new_index.is_none() && !self.tag_state.selection_drag_active {
                            self.clear_tool_overlay();
                        }
                    }

                    self.update_mouse_tracking_mode();
                }
                Task::none()
            }
            AnsiEditorMessage::CharSelector(msg) => {
                match msg {
                    CharSelectorMessage::SelectChar(code) => {
                        match self.char_selector_target {
                            Some(CharSelectorTarget::FKeySlot(slot)) => {
                                // Update the F-key slot with the selected character
                                let set_idx = self.click_handler.current_fkey_set();
                                let fkeys_to_save = {
                                    let mut opts = self.options.write();
                                    opts.fkeys.set_code_at(set_idx, slot, code);
                                    opts.fkeys.clone()
                                };
                                // Trigger async save
                                std::thread::spawn(move || {
                                    let _ = fkeys_to_save.save();
                                });
                                self.click_handler.clear_fkey_cache();
                            }
                            Some(CharSelectorTarget::BrushChar) => {
                                let ch = char::from_u32(code as u32).unwrap_or(' ');
                                // Route brush char selection back to the active tool.
                                let msg = tools::ToolMessage::SetBrushChar(ch);
                                let _ = self.update(AnsiEditorMessage::ToolMessage(msg));
                            }
                            None => {}
                        }
                        self.char_selector_target = None;
                    }
                    CharSelectorMessage::Cancel => {
                        self.char_selector_target = None;
                    }
                }
                Task::none()
            }
            AnsiEditorMessage::OutlineSelector(msg) => {
                self.font_handler.handle_outline_selector_message(&self.options, msg);
                Task::none()
            }
            AnsiEditorMessage::ColorSwitcher(_) => {
                // handled by wrapper (AnsiEditorMainArea)
                Task::none()
            }
            AnsiEditorMessage::PaletteGrid(_) => {
                // handled by wrapper (AnsiEditorMainArea)
                Task::none()
            }
            AnsiEditorMessage::SelectTool(_) => {
                // handled by wrapper (AnsiEditorMainArea)
                Task::none()
            }
            AnsiEditorMessage::SelectLayer(idx) => {
                self.with_edit_state(|state| state.set_current_layer(idx));
                self.task_none_with_layer_bounds_update()
            }
            AnsiEditorMessage::ToggleLayerVisibility(idx) => {
                let result = self.with_edit_state(move |state| state.toggle_layer_visibility(idx));
                if result.is_ok() {
                    self.is_modified = true;
                }
                Task::none()
            }
            AnsiEditorMessage::AddLayer => {
                let current_layer = self.with_edit_state(|state| state.get_current_layer().unwrap_or(0));
                let result = self.with_edit_state(|state| state.add_new_layer(current_layer));
                if result.is_ok() {
                    self.is_modified = true;
                    self.update_layer_bounds();
                }
                Task::none()
            }
            AnsiEditorMessage::RemoveLayer(idx) => {
                // Don't allow removing the last layer
                let layer_count = self.with_edit_state(|state| state.get_buffer().layers.len());
                if layer_count > 1 {
                    let result = self.with_edit_state(|state| state.remove_layer(idx));
                    if result.is_ok() {
                        self.is_modified = true;
                        self.update_layer_bounds();
                    }
                }
                Task::none()
            }
            AnsiEditorMessage::MoveLayerUp(idx) => {
                let result = self.with_edit_state(|state| state.raise_layer(idx));
                if result.is_ok() {
                    self.is_modified = true;
                    self.update_layer_bounds();
                }
                Task::none()
            }
            AnsiEditorMessage::MoveLayerDown(idx) => {
                let result = self.with_edit_state(|state| state.lower_layer(idx));
                if result.is_ok() {
                    self.is_modified = true;
                    self.update_layer_bounds();
                }
                Task::none()
            }
            AnsiEditorMessage::DuplicateLayer(idx) => {
                let result = self.with_edit_state(|state| state.duplicate_layer(idx));
                if result.is_ok() {
                    self.is_modified = true;
                    self.update_layer_bounds();
                }
                Task::none()
            }
            AnsiEditorMessage::MergeLayerDown(idx) => {
                let result = self.with_edit_state(|state| state.merge_layer_down(idx));
                if result.is_ok() {
                    self.is_modified = true;
                    self.update_layer_bounds();
                }
                Task::none()
            }
            AnsiEditorMessage::ClearLayer(idx) => {
                let result = self.with_edit_state(|state| state.clear_layer(idx));
                if result.is_ok() {
                    self.is_modified = true;
                }
                Task::none()
            }
            AnsiEditorMessage::ScrollViewport(dx, dy) => {
                self.canvas.scroll_by(dx, dy);
                Task::none()
            }
            AnsiEditorMessage::MinimapAutoscrollTick(_) => {
                // handled by wrapper (AnsiEditorMainArea)
                Task::none()
            }
            AnsiEditorMessage::EditLayer(_layer_index) => {
                // This message is handled by main_window to show the dialog
                // It's emitted from here and intercepted at a higher level
                Task::none()
            }

            // === Marker/Guide Messages ===
            AnsiEditorMessage::SetGuide(x, y) => {
                if x <= 0 && y <= 0 {
                    self.guide = None;
                } else {
                    self.guide = Some((x as f32, y as f32));
                    self.show_guide = true;
                }
                self.task_none_with_markers_update()
            }
            AnsiEditorMessage::ClearGuide => {
                self.guide = None;
                self.task_none_with_markers_update()
            }
            AnsiEditorMessage::SetRaster(x, y) => {
                if x <= 0 && y <= 0 {
                    self.raster = None;
                } else {
                    self.raster = Some((x as f32, y as f32));
                    self.show_raster = true;
                }
                self.task_none_with_markers_update()
            }
            AnsiEditorMessage::ClearRaster => {
                self.raster = None;
                self.task_none_with_markers_update()
            }
            AnsiEditorMessage::ToggleGuide => {
                self.show_guide = !self.show_guide;
                self.task_none_with_markers_update()
            }
            AnsiEditorMessage::ToggleRaster => {
                self.show_raster = !self.show_raster;
                self.task_none_with_markers_update()
            }
            AnsiEditorMessage::ToggleLineNumbers => {
                self.show_line_numbers = !self.show_line_numbers;
                Task::none()
            }
            AnsiEditorMessage::ToggleLayerBorders => {
                self.show_layer_borders = !self.show_layer_borders;
                self.task_none_with_layer_bounds_update()
            }
        }
    }

    /// Update the canvas markers based on current guide/raster settings
    fn update_markers(&mut self) {
        // Get font dimensions from screen for pixel conversion
        let (font_width, font_height) = {
            let screen = self.screen.lock();
            let font = screen.font(0);
            if let Some(f) = font {
                let size = f.size();
                (size.width as f32, size.height as f32)
            } else {
                (8.0, 16.0) // Default fallback
            }
        };

        // Update raster grid in pixel coordinates
        if self.show_raster {
            if let Some((cols, rows)) = self.raster {
                // Convert character spacing to pixel spacing
                let pixel_width = cols * font_width;
                let pixel_height = rows * font_height;
                self.canvas.set_raster(Some((pixel_width, pixel_height)));
            } else {
                self.canvas.set_raster(None);
            }
        } else {
            self.canvas.set_raster(None);
        }

        // Update guide crosshair in pixel coordinates
        if self.show_guide {
            if let Some((col, row)) = self.guide {
                // Convert character position to pixel position
                let pixel_x = col * font_width;
                let pixel_y = row * font_height;
                self.canvas.set_guide(Some((pixel_x, pixel_y)));
            } else {
                self.canvas.set_guide(None);
            }
        } else {
            self.canvas.set_guide(None);
        }
    }

    /// Update the layer bounds display based on current layer selection
    fn update_layer_bounds(&mut self) {
        // Always set layer bounds (needed for selection marching ants drawing)
        self.canvas.set_show_layer_borders(self.show_layer_borders);
        // Get current layer info from EditState
        let layer_bounds = {
            let mut screen = self.screen.lock();

            // Get font dimensions for pixel conversion
            let font = screen.font(0);
            let (font_width, font_height) = if let Some(f) = font {
                let size = f.size();
                (size.width as f32, size.height as f32)
            } else {
                (8.0, 16.0) // Default fallback
            };

            // Access the EditState to get buffer and current layer
            if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
                let buffer = edit_state.get_buffer();

                // In paste mode, find the floating layer instead of current layer
                let target_layer = if edit_state.has_floating_layer() {
                    buffer.layers.iter().enumerate().find(|(_, l)| l.role.is_paste()).map(|(i, _)| i)
                } else {
                    edit_state.get_current_layer().ok()
                };

                if let Some(layer_idx) = target_layer {
                    if let Some(layer) = buffer.layers.get(layer_idx) {
                        // Use offset() which respects preview_offset during drag
                        let offset = layer.offset();
                        let size = layer.size();
                        let width = size.width;
                        let height = size.height;

                        // Convert to pixels
                        let x = offset.x as f32 * font_width;
                        let y = offset.y as f32 * font_height;
                        let w = width as f32 * font_width;
                        let h = height as f32 * font_height;

                        Some((x, y, w, h))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        self.canvas.set_layer_bounds(layer_bounds, true);
    }

    /// Update tag rectangle overlays when Tag tool is active
    fn update_tag_overlays(&mut self) {
        // First, get all the data we need (position, length, is_selected)
        let (font_width, font_height, tag_data): (f32, f32, Vec<(icy_engine::Position, usize, bool)>) = {
            let mut screen = self.screen.lock();

            // Get font dimensions for pixel conversion
            let font = screen.font(0);
            let (fw, fh) = if let Some(f) = font {
                let size = f.size();
                (size.width as f32, size.height as f32)
            } else {
                (8.0, 16.0) // Default fallback
            };

            // Access EditState to get tags and update overlay mask
            let tags = if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
                let tag_info = self.tag_state.collect_overlay_data(edit_state);
                tools::TagTool::update_overlay_mask_in_state(edit_state);
                tag_info
            } else {
                vec![]
            };

            (fw, fh, tags)
        };

        // Now render overlay to canvas (no longer holding screen lock)
        let (mask, rect) = tools::TagTool::overlay_mask_for_tags(font_width, font_height, &tag_data);
        self.canvas.set_tool_overlay_mask(mask, rect);
    }

    /// Update the selection display in the shader
    fn update_selection_display(&mut self) {
        use icy_engine::AddType;
        use icy_engine_gui::selection_colors;

        // Get selection from EditState and convert to pixel coordinates
        let (selection_rect, selection_color, selection_mask_data, font_dimensions) = {
            let mut screen = self.screen.lock();

            // Get font dimensions for pixel conversion
            let size = screen.font_dimensions();
            let font_width = size.width as f32;
            let font_height = size.height as f32;

            // Access the EditState to get selection
            if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
                // Get the selection mask
                let selection_mask = edit_state.selection_mask();
                let selection = edit_state.selection();

                // Determine selection color based on add_type
                let selection_color = match selection.map(|s| s.add_type) {
                    Some(AddType::Add) => selection_colors::ADD,
                    Some(AddType::Subtract) => selection_colors::SUBTRACT,
                    _ => selection_colors::DEFAULT,
                };

                // Check if selection mask has content
                if !selection_mask.is_empty() {
                    // Generate texture data from selection mask.
                    // IMPORTANT: the shader samples this mask in *document cell coordinates* (0..buffer_w/0..buffer_h),
                    // so the texture must cover the full document size (no cropping/bounding-rect).
                    let buffer = edit_state.get_buffer();
                    let width = buffer.width().max(1) as u32;
                    let height = buffer.height().max(1) as u32;

                    // Create RGBA texture data (4 bytes per pixel)
                    let mut rgba_data = vec![0u8; (width * height * 4) as usize];

                    for y in 0..height {
                        for x in 0..width {
                            let doc_x = x as i32;
                            let doc_y = y as i32;
                            let is_selected = selection_mask.is_selected(icy_engine::Position::new(doc_x, doc_y));

                            let pixel_idx = ((y * width + x) * 4) as usize;
                            if is_selected {
                                // White = selected
                                rgba_data[pixel_idx] = 255;
                                rgba_data[pixel_idx + 1] = 255;
                                rgba_data[pixel_idx + 2] = 255;
                                rgba_data[pixel_idx + 3] = 255;
                            } else {
                                // Black = not selected
                                rgba_data[pixel_idx] = 0;
                                rgba_data[pixel_idx + 1] = 0;
                                rgba_data[pixel_idx + 2] = 0;
                                rgba_data[pixel_idx + 3] = 255;
                            }
                        }
                    }

                    // Selection rect is the *active* rectangular selection only (if present), not the mask bounds.
                    let selection_rect = selection.map(|sel| {
                        let rect = sel.as_rectangle();
                        let x = rect.left() as f32 * font_width;
                        let y = rect.top() as f32 * font_height;
                        let w = (rect.width() + 1) as f32 * font_width;
                        let h = (rect.height() + 1) as f32 * font_height;
                        (x, y, w, h)
                    });

                    (
                        selection_rect,
                        selection_color,
                        Some((rgba_data, width, height)),
                        Some((font_width, font_height)),
                    )
                } else if let Some(sel) = selection {
                    // No mask, but have selection rectangle
                    let rect = sel.as_rectangle();
                    let x = rect.left() as f32 * font_width;
                    let y = rect.top() as f32 * font_height;
                    let w = (rect.width() + 1) as f32 * font_width;
                    let h = (rect.height() + 1) as f32 * font_height;

                    (Some((x, y, w, h)), selection_color, None, Some((font_width, font_height)))
                } else {
                    (None, selection_colors::DEFAULT, None, None)
                }
            } else {
                (None, selection_colors::DEFAULT, None, None)
            }
        };

        self.canvas.set_selection(selection_rect);
        self.canvas.set_selection_color(selection_color);
        self.canvas.set_selection_mask(selection_mask_data, font_dimensions);
    }

    /// Set or update the reference image
    pub fn set_reference_image(&mut self, path: Option<PathBuf>, alpha: f32) {
        self.canvas.set_reference_image(path, alpha);
    }

    /// Toggle reference image visibility
    pub fn toggle_reference_image(&mut self) {
        self.canvas.toggle_reference_image();
    }

    /// Handle top-level window/input events that must reach the editor even when
    /// the inner widgets don't receive them (focus loss, cursor leaving window,
    /// global key presses).
    ///
    /// Returns `true` if the event was handled.
    pub fn handle_event(&mut self, event: &iced::Event) -> bool {
        match event {
            // Cancel transient shape drag/overlay on focus loss or when the cursor leaves the window.
            iced::Event::Window(iced::window::Event::Unfocused) | iced::Event::Mouse(iced::mouse::Event::CursorLeft) => {
                let _ = self.cancel_shape_drag();
                self.minimap_drag_pointer = None;
                true
            }
            // Ensure minimap drag/autoscroll stops even if the release happens outside the minimap widget.
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                self.minimap_drag_pointer = None;
                true
            }
            // Forward keyboard events directly into the editor.
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers: _, .. }) => {
                // Character selector overlay has priority and is closed with Escape.
                if self.char_selector_target.is_some() {
                    if matches!(key, iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)) {
                        self.char_selector_target = None;
                        return true;
                    }
                }

                // Editor-owned Escape handling for transient shape drag overlays.
                if matches!(key, iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)) {
                    let _ = self.cancel_shape_drag();
                }

                // Paste mode has priority for special keys (handled by PasteTool)
                if self.is_paste_mode() {
                    log::debug!("handle_key_press: key={:?}, paste_mode=true", key);
                    let result = self.dispatch_paste_event(event);
                    if !matches!(result, tools::ToolResult::None) {
                        return true;
                    }
                }

                // Tool-specific key events are owned by the active tool.
                let result = match self.current_tool {
                    Tool::Click => self.dispatch_click_event(event),
                    Tool::Font => self.dispatch_font_event(event),
                    Tool::Select => self.dispatch_select_event(event),
                    Tool::Tag => self.dispatch_tag_event(event),
                    _ => tools::ToolResult::None,
                };

                // UI-only updates owned by the editor.
                match self.current_tool {
                    Tool::Click | Tool::Font | Tool::Select => {
                        if !matches!(result, tools::ToolResult::None) {
                            self.update_selection_display();
                        }
                    }
                    Tool::Tag => {
                        if !matches!(result, tools::ToolResult::None) {
                            self.update_tag_overlays();
                            if self.tag_state.add_new_index.is_none() && !self.tag_state.selection_drag_active {
                                self.clear_tool_overlay();
                            }
                        }
                    }
                    _ => {}
                }

                true
            }
            _ => false,
        }
    }

    /// Handle terminal mouse events directly from icy_engine_gui
    fn handle_terminal_mouse_event(&mut self, msg: &TerminalMessage) {
        match msg {
            TerminalMessage::Press(evt) => {
                if evt.text_position.is_none() {
                    return;
                }

                // Paste mode always has priority.
                if self.is_paste_mode() {
                    let _ = self.dispatch_paste_terminal_message(msg);
                    return;
                }

                let _result = match self.current_tool {
                    Tool::Pipette => self.dispatch_pipette_terminal_message(msg),
                    Tool::Pencil => self.dispatch_pencil_terminal_message(msg),
                    Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled => {
                        self.dispatch_shape_terminal_message(msg)
                    }
                    Tool::Fill => self.dispatch_fill_terminal_message(msg),
                    Tool::Select => {
                        let ev = self.dispatch_select_terminal_message(msg);
                        self.update_selection_display();
                        ev
                    }
                    Tool::Click | Tool::Font => {
                        let ev = self.dispatch_click_terminal_message(self.current_tool, msg);
                        self.update_selection_display();
                        ev
                    }
                    Tool::Tag => {
                        let ev = self.dispatch_tag_terminal_message(msg);
                        self.update_tag_overlays();
                        ev
                    }
                };
            }
            TerminalMessage::Release(evt) => {
                if evt.text_position.is_none() {
                    return;
                }

                let target = self.mouse_capture_tool.unwrap_or_else(|| {
                    if self.is_paste_mode() {
                        MouseCaptureTarget::Paste
                    } else {
                        MouseCaptureTarget::Tool(self.current_tool)
                    }
                });

                let _result = match target {
                    MouseCaptureTarget::Paste => self.dispatch_paste_terminal_message(msg),
                    MouseCaptureTarget::Tool(tool) => match tool {
                        Tool::Pipette => self.dispatch_pipette_terminal_message(msg),
                        Tool::Pencil => self.dispatch_pencil_terminal_message(msg),
                        Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled => {
                            let prev = self.current_tool;
                            self.current_tool = tool;
                            let ev = self.dispatch_shape_terminal_message(msg);
                            self.current_tool = prev;
                            ev
                        }
                        Tool::Fill => self.dispatch_fill_terminal_message(msg),
                        Tool::Select => {
                            let ev = self.dispatch_select_terminal_message(msg);
                            self.update_selection_display();
                            ev
                        }
                        Tool::Click | Tool::Font => {
                            let ev = self.dispatch_click_terminal_message(tool, msg);
                            self.update_selection_display();
                            ev
                        }
                        Tool::Tag => {
                            let ev = self.dispatch_tag_terminal_message(msg);
                            self.update_tag_overlays();
                            if !self.tag_state.selection_drag_active {
                                self.clear_tool_overlay();
                            }
                            ev
                        }
                    },
                };
            }
            TerminalMessage::Move(evt) | TerminalMessage::Drag(evt) => {
                use tools::ToolHandler;

                let Some(pos) = evt.text_position else {
                    return;
                };

                // Brush hover preview is editor-owned UI.
                self.update_brush_preview(pos, evt.pixel_position);

                let target = self.mouse_capture_tool.unwrap_or_else(|| {
                    if self.is_paste_mode() {
                        MouseCaptureTarget::Paste
                    } else {
                        MouseCaptureTarget::Tool(self.current_tool)
                    }
                });

                let _result = match target {
                    MouseCaptureTarget::Paste => self.dispatch_paste_terminal_message(msg),
                    MouseCaptureTarget::Tool(tool) => match tool {
                        Tool::Pipette => self.dispatch_pipette_terminal_message(msg),
                        Tool::Pencil => self.dispatch_pencil_terminal_message(msg),
                        Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled => {
                            let prev = self.current_tool;
                            self.current_tool = tool;
                            let ev = self.dispatch_shape_terminal_message(msg);
                            self.current_tool = prev;
                            ev
                        }
                        Tool::Fill => self.dispatch_fill_terminal_message(msg),
                        Tool::Select => {
                            let ev = self.dispatch_select_terminal_message(msg);
                            self.update_selection_display();
                            *self.canvas.terminal.cursor_icon.write() = Some(self.select_handler.cursor());
                            ev
                        }
                        Tool::Click | Tool::Font => {
                            let ev = self.dispatch_click_terminal_message(tool, msg);
                            self.update_selection_display();
                            *self.canvas.terminal.cursor_icon.write() = Some(self.click_handler.cursor());
                            ev
                        }
                        Tool::Tag => {
                            let ev = self.dispatch_tag_terminal_message(msg);
                            self.update_tag_overlays();
                            if self.tag_state.selection_drag_active {
                                let (font_width, font_height) = {
                                    let screen = self.screen.lock();
                                    let font = screen.font(0);
                                    if let Some(f) = font {
                                        let size = f.size();
                                        (size.width as f32, size.height as f32)
                                    } else {
                                        (8.0, 16.0)
                                    }
                                };

                                let (mask, rect) = tools::TagTool::overlay_mask_for_selection_drag(
                                    font_width,
                                    font_height,
                                    self.tag_state.drag_start,
                                    self.tag_state.drag_cur,
                                );
                                self.canvas.set_tool_overlay_mask(mask, rect);
                            }
                            ev
                        }
                    },
                };
            }
            TerminalMessage::Scroll(delta) => match delta {
                iced::mouse::ScrollDelta::Lines { x, y } => {
                    self.canvas.scroll_by(*x * 20.0, *y * 20.0);
                }
                iced::mouse::ScrollDelta::Pixels { x, y } => {
                    self.canvas.scroll_by(*x, *y);
                }
            },
            TerminalMessage::Zoom(_) => {
                // Zoom is handled elsewhere
            }
        }
    }

    fn update_brush_preview(&mut self, pos: icy_engine::Position, pixel_position: (f32, f32)) {
        let show_preview = matches!(self.current_tool, Tool::Pencil);
        if !show_preview {
            self.canvas.set_brush_preview(None);
            return;
        }

        let brush_size = self.pencil_handler.brush_size().max(1) as i32;
        let half = brush_size / 2;

        // Get font dimensions for pixel conversion
        let (font_w, font_h) = {
            let screen = self.screen.lock();
            let size = screen.font_dimensions();
            (size.width as f32, size.height as f32)
        };

        let is_half_block_mode = matches!(self.pencil_handler.brush_primary(), BrushPrimaryMode::HalfBlock);

        let rect = if is_half_block_mode {
            // Compute doc-space half-block coordinate (Y doubled)
            let layer_offset = self.with_edit_state_readonly(|state| state.get_cur_layer().map(|l| l.offset()).unwrap_or_default());

            let hb_layer = self.compute_half_block_pos(pixel_position);
            let hb_doc = icy_engine::Position::new(hb_layer.x + layer_offset.x, hb_layer.y + layer_offset.y * 2);

            let left_hb = hb_doc.x - half;
            let top_hb = hb_doc.y - half;

            let x = left_hb as f32 * font_w;
            let y = top_hb as f32 * (font_h * 0.5);
            let w = brush_size as f32 * font_w;
            let h = brush_size as f32 * (font_h * 0.5);
            Some((x, y, w, h))
        } else {
            // Normal (cell) mode in doc coordinates
            let left = pos.x - half;
            let top = pos.y - half;

            let x = left as f32 * font_w;
            let y = top as f32 * font_h;
            let w = brush_size as f32 * font_w;
            let h = brush_size as f32 * font_h;
            Some((x, y, w, h))
        };

        self.canvas.set_brush_preview(rect);
    }

    // view() moved to `AnsiEditorMainArea` (see `main_area.rs`).

    /// Sync UI components with the current edit state
    /// Call this after operations that may change the palette or tags
    pub fn sync_ui(&mut self) {
        let (palette, format_mode, tag_count) =
            self.with_edit_state(|state| (state.get_buffer().palette.clone(), state.get_format_mode(), state.get_buffer().tags.len()));
        let _palette_limit = (format_mode == icy_engine_edit::FormatMode::XBinExtended).then_some(8);
        self.color_switcher.sync_palette(&palette);
        // Clear invalid tag selections (tags may have been removed by undo)
        self.tag_state.selection.retain(|&idx| idx < tag_count);
        // Update tag overlays (tag positions may have changed due to undo/redo)
        self.update_tag_overlays();
    }

    /// Refresh selection + selection-mask overlay data sent to the shader.
    /// Useful for menu/command actions executed outside the editor's own input handling.
    pub fn refresh_selection_display(&mut self) {
        self.update_selection_display();
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
        let format_mode = state.get_format_mode();

        // Get font info based on format mode
        let (font_name, current_font_slot, slot_fonts) = if format_mode == icy_engine_edit::FormatMode::XBinExtended {
            let slot0 = buffer.font(0).map(|f| f.name().to_string());
            let slot1 = buffer.font(1).map(|f| f.name().to_string());
            let current_slot = caret.font_page().min(1);
            (
                slot0.clone().or(slot1.clone()).unwrap_or_else(|| "Unknown".to_string()),
                current_slot,
                Some([slot0, slot1]),
            )
        } else {
            // Get font for current slot, falling back to slot 0 if not found
            let font_page = caret.font_page();
            let font_name = buffer
                .font(font_page)
                .or_else(|| buffer.font(0))
                .map(|f| f.name().to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            (font_name, font_page, None)
        };

        AnsiStatusInfo {
            cursor_position: (caret.x, caret.y),
            buffer_size: (buffer.width(), buffer.height()),
            current_layer,
            total_layers: buffer.layers.len(),
            current_tool: self.current_tool.name().to_string(),
            insert_mode: caret.insert_mode,
            font_name,
            format_mode,
            current_font_slot,
            slot_fonts,
        }
    }

    fn change_tool(&mut self, tool: Tool) {
        // Block tool changes during paste mode - must anchor or cancel first
        if self.is_paste_mode() {
            return;
        }

        // If capture is still set but we're not dragging anymore (e.g. drag got cancelled
        // via keyboard without any subsequent mouse move), clear it so tool switching
        // cannot get stuck.
        if !self.is_dragging && self.mouse_capture_tool.is_some() {
            self.mouse_capture_tool = None;
        }

        // Block tool changes while a drag is in progress.
        if self.is_dragging {
            return;
        }

        if self.current_tool == tool {
            return;
        }

        // Cancels any in-progress shape preview/drag when switching tools.
        let _ = self.cancel_shape_drag();

        let mut is_visble = matches!(tool, Tool::Click | Tool::Font);
        is_visble &= self.with_edit_state(|state: &mut EditState| {
            state.set_caret_visible(is_visble && state.selection().is_none());
            state.selection().is_none()
        });

        // Enable terminal focus for caret blinking in Click/Font tools
        self.canvas.set_has_focus(is_visble);

        self.current_tool = tool;

        self.update_mouse_tracking_mode();

        // Clear tool hover preview when switching tools.
        if !matches!(tool, Tool::Pencil) {
            self.canvas.set_brush_preview(None);
        }

        // Update tag overlays when switching to/from Tag tool
        if tool == Tool::Tag {
            self.update_tag_overlays();
        } else {
            // Clear tag overlays when leaving Tag tool
            self.canvas.set_tool_overlay_mask(None, None);
        }
        // Fonts are loaded centrally via FontLibrary - no per-editor loading needed
    }

    fn tool_supports_half_block_mode(tool: Tool) -> bool {
        matches!(
            tool,
            Tool::Pencil | Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled | Tool::Fill
        )
    }

    fn update_mouse_tracking_mode(&mut self) {
        // HalfBlock tracking is tied to the tool *options* (brush primary mode).
        let wants_half_block = match self.current_tool {
            Tool::Pencil => self.pencil_handler.brush_primary() == BrushPrimaryMode::HalfBlock,
            Tool::Fill => self.fill_handler.brush_primary() == BrushPrimaryMode::HalfBlock,
            Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled => {
                self.shape_handler.brush_primary() == BrushPrimaryMode::HalfBlock
            }
            _ => false,
        };
        let tool_allows = Self::tool_supports_half_block_mode(self.current_tool);

        let tracking = if wants_half_block && tool_allows {
            icy_engine_gui::MouseTracking::HalfBlock
        } else {
            icy_engine_gui::MouseTracking::Chars
        };

        self.canvas.terminal.set_mouse_tracking(tracking);
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
    pub font_name: String,
    /// Current format mode
    pub format_mode: icy_engine_edit::FormatMode,
    /// Currently active font slot (0 or 1 for XBinExtended)
    pub current_font_slot: usize,
    /// Font names for slots (only set for XBinExtended)
    pub slot_fonts: Option<[Option<String>; 2]>,
}
