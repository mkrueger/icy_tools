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
pub mod widget;

use dialog::tag::{TagDialog, TagDialogMessage};
use dialog::tag_list::{TagListDialog, TagListDialogMessage, TagListItem};

pub use dialog::edit_layer::*;
pub use dialog::file_settings::*;
pub use dialog::font_selector::*;
pub use dialog::font_slot_manager::*;
pub use dialog::reference_image::*;
pub use dialog::tdf_font_selector::{TdfFontSelectorDialog, TdfFontSelectorMessage};

pub use widget::canvas::*;
pub use widget::char_selector::*;
pub use widget::color_switcher::gpu::*;
pub use widget::fkey_toolbar::gpu::*;
pub use widget::font_tool::FontToolState;
pub use widget::layer_view::*;
pub use widget::minimap::*;
pub use widget::palette_grid::*;
pub use widget::right_panel::*;
pub use widget::toolbar::tool_panel_wrapper::{ToolPanel, ToolPanelMessage};
pub use widget::toolbar::top::*;

use icy_engine_edit::EditState;
use icy_engine_edit::OperationType;
use icy_engine_edit::UndoState;
use icy_engine_edit::tools::{self, Tool, ToolEvent};

use std::path::PathBuf;
use std::sync::Arc;

use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContent, ContentFormat};
use iced::{
    Alignment, Element, Length, Task, Theme,
    widget::{column, container, row},
};
use icy_engine::formats::{FileFormat, LoadData};
use icy_engine::{MouseButton, Position, Screen, Sixel, Tag, TagRole, TextBuffer, TextPane};
use icy_engine_gui::ICY_CLIPBOARD_TYPE;
use icy_engine_gui::terminal::crt_state::{is_command_pressed, is_ctrl_pressed, is_shift_pressed};
use icy_engine_gui::theme::main_area_background;
use parking_lot::{Mutex, RwLock};

use crate::SharedFontLibrary;
use crate::ui::Options;
use icy_engine::BufferType;

/// Target for the character selector popup
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharSelectorTarget {
    /// Editing an F-key slot (0-11)
    FKeySlot(usize),
    /// Editing the brush paint character
    BrushChar,
}

/// Convert icy_engine MouseButton to iced mouse button
fn convert_mouse_button(button: MouseButton) -> iced::mouse::Button {
    match button {
        MouseButton::Left => iced::mouse::Button::Left,
        MouseButton::Right => iced::mouse::Button::Right,
        MouseButton::Middle => iced::mouse::Button::Middle,
        _ => iced::mouse::Button::Left, // Default to left for other buttons
    }
}

/// Messages for the ANSI editor
use widget::outline_selector::{OutlineSelector, OutlineSelectorMessage, outline_selector_width};

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
    /// F-key toolbar messages (Click tool)
    FKeyToolbar(FKeyToolbarMessage),
    /// Char selector popup messages (F-key character selection)
    CharSelector(CharSelectorMessage),
    /// Outline selector popup messages (font tool outline style)
    OutlineSelector(OutlineSelectorMessage),
    /// Color switcher messages
    ColorSwitcher(ColorSwitcherMessage),
    /// Palette grid messages
    PaletteGrid(PaletteGridMessage),

    /// Cancel an in-progress shape drag (clears preview overlay).
    CancelShapeDrag,
    /// Cancel an in-progress minimap drag/autoscroll.
    CancelMinimapDrag,
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
    /// Key pressed
    KeyPressed(iced::keyboard::Key, iced::keyboard::Modifiers),
    /// Mouse event on canvas
    CanvasMouseEvent(CanvasMouseEvent),

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
    /// Open TDF font selector
    OpenTdfFontSelector,
    /// TDF font selector messages
    TdfFontSelector(TdfFontSelectorMessage),

    /// Tag config dialog messages
    TagDialog(TagDialogMessage),

    /// Open the tag list dialog
    OpenTagListDialog,
    /// Tag list dialog messages
    TagListDialog(TagListDialogMessage),

    // === Tag Context Menu Messages ===
    /// Edit a tag (opens dialog in edit mode)
    TagEdit(usize),
    /// Delete a tag
    TagDelete(usize),
    /// Clone a tag
    TagClone(usize),
    /// Close tag context menu
    TagContextMenuClose,
    /// Delete all selected tags
    TagDeleteSelected,
    /// Start "Add Tag" mode - cursor shows TAG preview until user clicks
    TagStartAddMode,
    /// Cancel "Add Tag" mode
    TagCancelAddMode,
}

/// Mouse events on the canvas (using text/buffer coordinates)
#[derive(Clone, Debug)]
pub enum CanvasMouseEvent {
    Press {
        position: icy_engine::Position,
        pixel_position: (f32, f32),
        button: iced::mouse::Button,
        modifiers: icy_engine::KeyModifiers,
    },
    Release {
        position: icy_engine::Position,
        pixel_position: (f32, f32),
        button: iced::mouse::Button,
    },
    Move {
        position: icy_engine::Position,
        pixel_position: (f32, f32),
    },
    Scroll {
        delta: iced::mouse::ScrollDelta,
    },
}

/// Selection drag mode - determines what part of selection is being dragged
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub enum SelectionDrag {
    #[default]
    None,
    /// Create new selection
    Create,
    /// Move existing selection
    Move,
    /// Resize from edges/corners
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl SelectionDrag {
    /// Convert to mouse cursor interaction for resize handles
    pub fn to_cursor_interaction(self) -> Option<iced::mouse::Interaction> {
        use iced::mouse::Interaction;
        match self {
            SelectionDrag::None | SelectionDrag::Create => None,
            SelectionDrag::Move => Some(Interaction::Grab),
            SelectionDrag::Left | SelectionDrag::Right => Some(Interaction::ResizingHorizontally),
            SelectionDrag::Top | SelectionDrag::Bottom => Some(Interaction::ResizingVertically),
            SelectionDrag::TopLeft | SelectionDrag::BottomRight => Some(Interaction::ResizingDiagonallyDown),
            SelectionDrag::TopRight | SelectionDrag::BottomLeft => Some(Interaction::ResizingDiagonallyUp),
        }
    }
}

/// Pipette tool state - stores the currently hovered character and modifiers
#[derive(Default, Clone, Debug)]
pub struct PipetteState {
    /// Currently hovered character (if any)
    pub cur_char: Option<icy_engine::AttributedChar>,
    /// Current hover position
    pub cur_pos: Option<icy_engine::Position>,
    /// Take foreground color (Shift=only FG, Ctrl=only BG, neither=both)
    pub take_fg: bool,
    /// Take background color
    pub take_bg: bool,
}

impl PipetteState {
    /// Update the modifier flags based on current keyboard state
    pub fn update_modifiers(&mut self) {
        let shift = is_shift_pressed();
        let ctrl = is_ctrl_pressed() || is_command_pressed();

        // Moebius-style:
        // - No modifier: both FG and BG
        // - Shift only: FG only
        // - Ctrl only: BG only
        // - Both: both (fallback)
        self.take_fg = !ctrl || shift;
        self.take_bg = !shift || ctrl;
    }
}

/// Drag position tracking for mouse operations
#[derive(Default, Clone, Copy, Debug)]
pub struct DragPos {
    /// Start position in buffer coordinates
    pub start: icy_engine::Position,
    /// Current position in buffer coordinates
    pub cur: icy_engine::Position,
    /// Start position absolute (including scroll offset)
    pub start_abs: icy_engine::Position,
    /// Current position absolute (including scroll offset)
    pub cur_abs: icy_engine::Position,
    /// Start position in half-block coordinates (2x Y resolution)
    /// Used for line/shape tools in half-block mode
    pub start_half_block: icy_engine::Position,
    /// Current position in half-block coordinates (2x Y resolution)
    pub cur_half_block: icy_engine::Position,
}

/// The main ANSI editor component
pub struct AnsiEditor {
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
    /// F-key toolbar (GPU shader version, Click tool only)
    pub fkey_toolbar: ShaderFKeyToolbar,
    /// Color switcher (FG/BG display)
    pub color_switcher: ColorSwitcher,
    /// Palette grid
    pub palette_grid: PaletteGrid,
    /// Canvas view state
    pub canvas: CanvasView,
    /// Right panel state (minimap, layers)
    pub right_panel: RightPanel,
    /// Shared options
    pub options: Arc<RwLock<Options>>,
    /// Whether the document is modified
    pub is_modified: bool,

    /// While Some, the minimap is being dragged. Stores last pointer position relative to minimap
    /// bounds (may be outside) to simulate egui-style continuous drag updates.
    minimap_drag_pointer: Option<(f32, f32)>,

    // === Selection/Drag State ===
    /// Current drag positions for mouse operations
    pub drag_pos: DragPos,
    /// Whether mouse is currently dragging
    pub is_dragging: bool,
    /// Tool that currently has mouse capture during a drag (move/up are routed here)
    mouse_capture_tool: Option<Tool>,
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

    // === Font Tool State ===
    /// Font tool state (loaded fonts, selected font, etc.)
    pub font_tool: FontToolState,
    /// If true, show the outline selector popup for font tool
    pub outline_selector_open: bool,
    /// If true, show the TDF font selector dialog
    pub tdf_font_selector_open: bool,
    /// TDF font selector dialog state
    pub tdf_font_selector: TdfFontSelectorDialog,

    // === Tag Tool State ===
    pub tag_dialog: Option<TagDialog>,
    pub tag_list_dialog: Option<TagListDialog>,
    /// If true, we are dragging a tag
    tag_drag_active: bool,
    /// Indices of tags being dragged (supports multi-selection)
    tag_drag_indices: Vec<usize>,
    /// Tag positions at start of drag (parallel to tag_drag_indices)
    tag_drag_start_positions: Vec<icy_engine::Position>,
    /// Selected tag indices for multi-selection
    pub tag_selection: Vec<usize>,
    /// Context menu state: Some((tag_index, screen_position)) when open
    pub tag_context_menu: Option<(usize, icy_engine::Position)>,
    /// If Some(index), we are adding a new tag and dragging it - stores the index of the new tag
    pub tag_add_new_index: Option<usize>,
    /// If true, we are doing a selection rectangle drag to select multiple tags
    tag_selection_drag_active: bool,
    /// Atomic undo guard for tag drag operations
    tag_drag_undo: Option<icy_engine_edit::AtomicUndoGuard>,

    // === Paint Stroke State (Pencil/Brush/Erase) ===
    paint_undo: Option<icy_engine_edit::AtomicUndoGuard>,
    paint_last_pos: Option<icy_engine::Position>,
    paint_button: iced::mouse::Button,

    // === Half-Block Mode State ===
    /// Current mouse position in half-block coordinates (2x Y resolution).
    /// Used for pencil/brush drawing and line interpolation in half-block mode.
    /// Updated on every mouse move during drag operations.
    pub half_block_click_pos: icy_engine::Position,

    // === Shape Tool State ===
    /// If true, shape tools clear/erase instead of drawing (Moebius-style shift behavior).
    shape_clear: bool,

    // === Pipette Tool State ===
    /// Pipette tool state (current character, modifiers)
    pub pipette: PipetteState,

    // === Layer Drag State (Ctrl+Click+Drag in Click tool) ===
    /// If true, we are dragging a layer (Ctrl+Click+Drag)
    layer_drag_active: bool,
    /// Layer offset at start of drag
    layer_drag_start_offset: icy_engine::Position,
    /// Atomic undo guard for layer drag operations
    layer_drag_undo: Option<icy_engine_edit::AtomicUndoGuard>,

    // === Paste Mode State ===
    /// If true, we are dragging the floating paste layer
    paste_drag_active: bool,
    /// Floating layer offset at start of drag
    paste_drag_start_offset: icy_engine::Position,
    /// Atomic undo guard for paste drag operations
    paste_drag_undo: Option<icy_engine_edit::AtomicUndoGuard>,
    /// Tool that was active before paste mode started (to restore after anchor/cancel)
    paste_previous_tool: Option<Tool>,
}

impl AnsiEditor {
    // NOTE (Layer-local coordinates)
    // ============================
    // The terminal/canvas events provide positions in *document* coordinates.
    // All *painting* operations (Brush/Pencil/Erase/Shapes) are ALWAYS executed in
    // *layer-local* coordinates, i.e. relative to the current layer's offset.
    // Do NOT pass document/global positions into brush algorithms.
    // Selection/mask operations are handled by EditState and keep using document coords.

    fn doc_to_layer_pos(&mut self, pos: icy_engine::Position) -> icy_engine::Position {
        self.with_edit_state(|state| if let Some(layer) = state.get_cur_layer() { pos - layer.offset() } else { pos })
    }

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

    fn half_block_is_top_from_pixel(&self, pixel_position: (f32, f32)) -> bool {
        let render_info = self.canvas.terminal.render_info.read();
        let font_h = render_info.font_height.max(1.0);
        let scale = render_info.display_scale.max(0.001);

        // pixel_position is widget-local.
        let mut y = (pixel_position.1 - render_info.viewport_y) / scale;
        if render_info.scan_lines {
            y /= 2.0;
        }

        let cell_y = (y / font_h).floor();
        let within = y - cell_y * font_h;
        within < (font_h * 0.5)
    }

    /// Paint a half-block point with brush_size expansion in half-block coordinates.
    /// This is similar to Moebius's half_block_line pattern where brush_size
    /// expands around the half-block coordinate.
    fn apply_half_block_with_brush_size(&mut self, half_block_pos: icy_engine::Position, button: iced::mouse::Button) {
        let brush_size = self.top_toolbar.brush_options.brush_size.max(1) as i32;
        let half = brush_size / 2;

        for dy in 0..brush_size {
            for dx in 0..brush_size {
                let hb_x = half_block_pos.x + dx - half;
                let hb_y = half_block_pos.y + dy - half;

                // Skip negative coordinates (would be outside document)
                if hb_y < 0 {
                    continue;
                }

                // Convert half-block to cell coordinates
                let cell_pos = icy_engine::Position::new(hb_x, hb_y / 2);
                let is_top = (hb_y % 2) == 0;

                self.apply_paint_stamp_with_half_block_info(cell_pos, is_top, button);
            }
        }
    }

    /// Internal: Paint stamp at cell position with explicit half-block top/bottom info
    fn apply_paint_stamp_with_half_block_info(&mut self, doc_pos: icy_engine::Position, half_block_is_top: bool, button: iced::mouse::Button) {
        let tool = self.current_tool;

        let (primary, paint_char, brush_size, colorize_fg, colorize_bg) = {
            let opts = &self.top_toolbar.brush_options;
            (opts.primary, opts.paint_char, opts.brush_size.max(1), opts.colorize_fg, opts.colorize_bg)
        };

        let swap_colors = button == iced::mouse::Button::Right;
        // Shift key also swaps colors (like in many graphics programs)
        let shift_swap = is_shift_pressed();
        // half_block_is_top is passed in directly for HalfBlock mode with brush_size

        self.with_edit_state(|state| {
            let (offset, layer_w, layer_h) = if let Some(layer) = state.get_cur_layer() {
                (layer.offset(), layer.width(), layer.height())
            } else {
                return;
            };
            let use_selection = state.is_something_selected();

            let caret_attr = state.get_caret().attribute;
            // Don't swap colors for Shading (has its own up/down behavior) or Char mode (right-click = erase)
            // Shift key swaps colors for all modes except those with special right-click behavior
            let swap_for_colors = (swap_colors || shift_swap) && !matches!(primary, BrushPrimaryMode::Shading | BrushPrimaryMode::Char);
            let (fg, bg) = if swap_for_colors {
                (caret_attr.background(), caret_attr.foreground())
            } else {
                (caret_attr.foreground(), caret_attr.background())
            };

            // Note: Pencil and Brush now both use brush_size from slider.
            // For HalfBlock mode, brush_size expansion happens in half-block coordinates
            // at the call site, so here we use size 1 to paint single points.
            let effective_brush_size = if matches!(primary, BrushPrimaryMode::HalfBlock) { 1 } else { brush_size };
            let brush_size_i: i32 = effective_brush_size as i32;

            let center = doc_pos - offset;
            let half = brush_size_i / 2;

            for dy in 0..brush_size_i {
                for dx in 0..brush_size_i {
                    let layer_pos = icy_engine::Position::new(center.x + dx - half, center.y + dy - half);

                    // Bounds check against layer.
                    if layer_pos.x < 0 || layer_pos.y < 0 || layer_pos.x >= layer_w || layer_pos.y >= layer_h {
                        continue;
                    }

                    // Selection is in document coords.
                    if use_selection {
                        let doc_cell = layer_pos + offset;
                        if !state.is_selected(doc_cell) {
                            continue;
                        }
                    }

                    match tool {
                        Tool::Pencil | Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled => {
                            use icy_engine_edit::brushes::{BrushMode as EngineBrushMode, ColorMode as EngineColorMode, DrawContext, PointRole};

                            let brush_mode = match primary {
                                BrushPrimaryMode::Char => {
                                    // Right-click in Char mode = erase (set to space)
                                    if swap_colors {
                                        EngineBrushMode::Char(' ')
                                    } else {
                                        EngineBrushMode::Char(paint_char)
                                    }
                                }
                                BrushPrimaryMode::HalfBlock => EngineBrushMode::HalfBlock,
                                BrushPrimaryMode::Shading => {
                                    if swap_colors {
                                        EngineBrushMode::ShadeDown
                                    } else {
                                        EngineBrushMode::Shade
                                    }
                                }
                                BrushPrimaryMode::Replace => EngineBrushMode::Replace(paint_char),
                                BrushPrimaryMode::Blink => EngineBrushMode::Blink(!swap_colors),
                                BrushPrimaryMode::Colorize => EngineBrushMode::Colorize,
                            };

                            let color_mode = if matches!(primary, BrushPrimaryMode::Colorize) {
                                match (colorize_fg, colorize_bg) {
                                    (true, true) => EngineColorMode::Both,
                                    (true, false) => EngineColorMode::Foreground,
                                    (false, true) => EngineColorMode::Background,
                                    (false, false) => EngineColorMode::None,
                                }
                            } else {
                                EngineColorMode::Both
                            };

                            let mut template = caret_attr;
                            template.set_foreground(fg);
                            template.set_background(bg);

                            // Use the brush library on a small adapter around the current layer.
                            struct LayerTarget<'a> {
                                state: &'a mut icy_engine_edit::EditState,
                                width: i32,
                                height: i32,
                            }
                            impl<'a> icy_engine_edit::brushes::DrawTarget for LayerTarget<'a> {
                                fn width(&self) -> i32 {
                                    self.width
                                }
                                fn height(&self) -> i32 {
                                    self.height
                                }
                                fn char_at(&self, pos: icy_engine_edit::Position) -> Option<icy_engine_edit::AttributedChar> {
                                    self.state.get_cur_layer().map(|l| l.char_at(pos))
                                }
                                fn set_char(&mut self, pos: icy_engine_edit::Position, ch: icy_engine_edit::AttributedChar) {
                                    let _ = self.state.set_char_in_atomic(pos, ch);
                                }
                            }

                            let ctx = DrawContext::default()
                                .with_brush_mode(brush_mode)
                                .with_color_mode(color_mode)
                                .with_foreground(fg)
                                .with_background(bg)
                                .with_template_attribute(template)
                                .with_half_block_is_top(half_block_is_top);

                            let mut target = LayerTarget {
                                state,
                                width: layer_w,
                                height: layer_h,
                            };

                            ctx.plot_point(&mut target, layer_pos, PointRole::Fill);
                        }
                        _ => {}
                    }
                }
            }
        });
    }

    /// Paint stamp at cell position, determining half-block top/bottom from pixel position
    fn apply_paint_stamp(&mut self, doc_pos: icy_engine::Position, pixel_position: (f32, f32), button: iced::mouse::Button) {
        let half_block_is_top = self.half_block_is_top_from_pixel(pixel_position);
        self.apply_paint_stamp_with_half_block_info(doc_pos, half_block_is_top, button);
    }

    #[allow(dead_code)]
    fn layer_to_doc_pos(&mut self, pos: icy_engine::Position) -> icy_engine::Position {
        self.with_edit_state(|state| if let Some(layer) = state.get_cur_layer() { pos + layer.offset() } else { pos })
    }

    fn current_select_add_type(&self) -> icy_engine::AddType {
        if self.current_tool != Tool::Select {
            return icy_engine::AddType::Default;
        }

        // Modifiers are read from global state because event modifiers may be unreliable.
        if is_ctrl_pressed() || is_command_pressed() {
            icy_engine::AddType::Subtract
        } else if is_shift_pressed() {
            icy_engine::AddType::Add
        } else {
            icy_engine::AddType::Default
        }
    }

    /// Create a new empty ANSI editor
    pub fn new(options: Arc<RwLock<Options>>, font_library: SharedFontLibrary) -> Self {
        let buffer = TextBuffer::create((80, 25));
        Self::with_buffer(buffer, None, options, font_library)
    }

    /// Create an ANSI editor with a file
    ///
    /// Returns the editor with the loaded buffer, or an error if loading failed.
    pub fn with_file(path: PathBuf, options: Arc<RwLock<Options>>, font_library: SharedFontLibrary) -> anyhow::Result<Self> {
        // Detect file format
        let format = FileFormat::from_path(&path).ok_or_else(|| anyhow::anyhow!("Unknown file format"))?;

        if !format.is_supported() {
            anyhow::bail!("Format '{}' is not supported for editing", format.name());
        }

        // Read file data
        let data = std::fs::read(&path)?;

        // Load buffer using the format
        let load_data = LoadData::default();
        let buffer = format.from_bytes(&data, Some(load_data))?.buffer;

        Ok(Self::with_buffer(buffer, Some(path), options, font_library))
    }

    /// Create an ANSI editor with an existing buffer
    pub fn with_buffer(buffer: TextBuffer, file_path: Option<PathBuf>, options: Arc<RwLock<Options>>, font_library: SharedFontLibrary) -> Self {
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

        // Create palette components with synced palette
        let mut palette_grid = PaletteGrid::new();
        let palette_limit = (format_mode == icy_engine_edit::FormatMode::XBinExtended).then_some(8);
        palette_grid.sync_palette(&palette, palette_limit);

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

        let mut top_toolbar = TopToolbar::new();
        top_toolbar.select_options.current_fkey_page = initial_fkey_set;

        Self {
            file_path,
            screen,
            tool_panel: ToolPanel::new(),
            current_tool: Tool::Click,
            top_toolbar,
            fkey_toolbar: ShaderFKeyToolbar::new(),
            color_switcher,
            palette_grid,
            canvas,
            right_panel: RightPanel::new(),
            options,
            is_modified: false,

            minimap_drag_pointer: None,
            // Selection/drag state
            drag_pos: DragPos::default(),
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

            font_tool: FontToolState::new(font_library.clone()),
            outline_selector_open: false,
            tdf_font_selector_open: false,
            tdf_font_selector: TdfFontSelectorDialog::new(font_library),

            tag_dialog: None,
            tag_list_dialog: None,
            tag_drag_active: false,
            tag_drag_indices: Vec::new(),
            tag_drag_start_positions: Vec::new(),
            tag_selection: Vec::new(),
            tag_context_menu: None,
            tag_add_new_index: None,
            tag_selection_drag_active: false,
            tag_drag_undo: None,

            paint_undo: None,
            paint_last_pos: None,
            paint_button: iced::mouse::Button::Left,

            half_block_click_pos: icy_engine::Position::default(),

            shape_clear: false,

            pipette: PipetteState::default(),

            layer_drag_active: false,
            layer_drag_start_offset: icy_engine::Position::default(),
            layer_drag_undo: None,

            paste_drag_active: false,
            paste_drag_start_offset: icy_engine::Position::default(),
            paste_drag_undo: None,
            paste_previous_tool: None,
        }
    }

    fn clear_tool_overlay(&mut self) {
        self.canvas.set_tool_overlay_mask(None, None);
    }

    fn redraw_with_tag_overlays(&mut self) -> ToolEvent {
        self.update_tag_overlays();
        ToolEvent::Redraw
    }

    fn redraw_with_selection_display(&mut self) -> ToolEvent {
        self.update_selection_display();
        ToolEvent::Redraw
    }

    fn redraw_with_layer_bounds(&mut self) -> ToolEvent {
        self.update_layer_bounds();
        ToolEvent::Redraw
    }

    fn task_none_with_markers_update(&mut self) -> Task<AnsiEditorMessage> {
        self.update_markers();
        Task::none()
    }

    fn task_none_with_layer_bounds_update(&mut self) -> Task<AnsiEditorMessage> {
        self.update_layer_bounds();
        Task::none()
    }

    fn task_none_with_tag_overlays_update(&mut self) -> Task<AnsiEditorMessage> {
        self.update_tag_overlays();
        Task::none()
    }

    fn close_tag_context_menu(&mut self) {
        self.tag_context_menu = None;
    }

    fn commit_and_update_tag_overlays(&mut self, description: impl Into<String>) -> Task<AnsiEditorMessage> {
        self.handle_tool_event(ToolEvent::Commit(description.into()));
        self.task_none_with_tag_overlays_update()
    }

    fn end_drag_capture(&mut self) {
        self.is_dragging = false;
        self.mouse_capture_tool = None;
        self.selection_drag = SelectionDrag::None;
        self.start_selection = None;
    }

    fn cancel_layer_drag(&mut self) {
        if !self.layer_drag_active {
            return;
        }

        self.layer_drag_active = false;
        self.layer_drag_undo = None;

        self.clear_current_layer_preview_offset();
        self.update_layer_bounds();

        self.end_drag_capture();
    }

    fn clear_current_layer_preview_offset(&mut self) {
        self.with_edit_state(|state| {
            if let Some(layer) = state.get_cur_layer_mut() {
                layer.set_preview_offset(None);
            }
            state.mark_dirty();
        });
    }

    fn finish_layer_drag(&mut self) -> ToolEvent {
        if !self.layer_drag_active {
            return ToolEvent::None;
        }

        self.layer_drag_active = false;
        self.end_drag_capture();

        // Calculate final offset and apply
        let delta = self.drag_pos.cur_abs - self.drag_pos.start_abs;
        let new_offset = self.layer_drag_start_offset + delta;

        self.with_edit_state(|state| {
            if let Some(layer) = state.get_cur_layer_mut() {
                layer.set_preview_offset(None);
            }
            let _ = state.move_layer(new_offset);
        });

        // Finalize atomic undo
        self.layer_drag_undo = None;

        // Update layer border display after move
        self.update_layer_bounds();

        ToolEvent::Commit("Move layer".to_string())
    }

    fn cancel_tag_selection_drag(&mut self) {
        if !self.tag_selection_drag_active {
            return;
        }
        self.tag_selection_drag_active = false;
        self.clear_tool_overlay();
        self.end_drag_capture();
    }

    fn finish_tag_selection_drag(&mut self) -> ToolEvent {
        if !self.tag_selection_drag_active {
            return ToolEvent::None;
        }

        self.tag_selection_drag_active = false;
        self.end_drag_capture();

        // Calculate selection rectangle
        let min_x = self.drag_pos.start.x.min(self.drag_pos.cur.x);
        let max_x = self.drag_pos.start.x.max(self.drag_pos.cur.x);
        let min_y = self.drag_pos.start.y.min(self.drag_pos.cur.y);
        let max_y = self.drag_pos.start.y.max(self.drag_pos.cur.y);

        // Find all tags that intersect with the selection rectangle
        let selected_indices: Vec<usize> = self.with_edit_state(|state| {
            state
                .get_buffer()
                .tags
                .iter()
                .enumerate()
                .filter(|(_, tag)| {
                    let tag_min_x = tag.position.x;
                    let tag_max_x = tag.position.x + tag.len() as i32 - 1;
                    let tag_y = tag.position.y;

                    // Check if tag intersects with selection rectangle
                    tag_y >= min_y && tag_y <= max_y && tag_max_x >= min_x && tag_min_x <= max_x
                })
                .map(|(i, _)| i)
                .collect()
        });

        self.tag_selection = selected_indices;

        // Clear selection overlay
        self.clear_tool_overlay();
        self.update_tag_overlays();

        ToolEvent::Redraw
    }

    fn finish_selection_drag(&mut self) -> ToolEvent {
        if self.selection_drag == SelectionDrag::None {
            return ToolEvent::None;
        }

        // Finalize selection
        if self.selection_drag == SelectionDrag::Create {
            // If start == cur, treat as click
            if self.drag_pos.start == self.drag_pos.cur {
                if self.current_tool == Tool::Select {
                    // Keep selection mask intact.
                    self.with_edit_state(|state| {
                        let _ = state.deselect();
                    });
                } else {
                    self.with_edit_state(|state| {
                        let _ = state.clear_selection();
                    });
                }
            } else if self.current_tool == Tool::Select {
                // Get current add_type before committing
                let add_type = self.with_edit_state(|state| state.selection().map(|s| s.add_type));

                #[cfg(debug_assertions)]
                eprintln!("[DEBUG] Mouse up - Tool::Select, add_type: {:?}", add_type);

                // Only commit to mask for Add/Subtract modes.
                // Default mode keeps the selection active (not in mask) so move/resize doesn't leave stale mask artifacts.
                match add_type {
                    Some(icy_engine::AddType::Add) | Some(icy_engine::AddType::Subtract) => {
                        #[cfg(debug_assertions)]
                        eprintln!("[DEBUG] Mouse up - Committing selection to mask and deselecting");
                        self.with_edit_state(|state| {
                            let _ = state.add_selection_to_mask();
                            let _ = state.deselect();
                        });
                    }
                    _ => {
                        #[cfg(debug_assertions)]
                        eprintln!("[DEBUG] Mouse up - Default mode: keeping selection active");
                        // Default mode: keep selection active, don't commit to mask
                    }
                }
            }
        }
        // Move/Resize: selection is already updated during drag, just keep it active

        // Always update selection display after any selection drag ends
        self.update_selection_display();

        self.end_drag_capture();
        ToolEvent::Redraw
    }

    fn end_tag_drag(&mut self) {
        self.tag_drag_active = false;
        self.tag_drag_indices.clear();
        self.tag_drag_start_positions.clear();
        // Drop the guard to finalize/commit the atomic undo.
        self.tag_drag_undo = None;
        self.end_drag_capture();
    }

    fn cancel_shape_drag(&mut self) -> bool {
        if self.is_dragging
            && matches!(
                self.current_tool,
                Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled
            )
        {
            self.end_drag_capture();
            self.paint_button = iced::mouse::Button::Left;
            self.shape_clear = false;
            self.clear_tool_overlay();
            return true;
        }
        false
    }

    fn shape_points(tool: Tool, p0: icy_engine::Position, p1: icy_engine::Position) -> Vec<icy_engine::Position> {
        use icy_engine_edit::brushes;

        match tool {
            Tool::Line => brushes::get_line_points(p0, p1),
            Tool::RectangleOutline => brushes::get_rectangle_points(p0, p1).into_iter().map(|(p, _)| p).collect(),
            Tool::RectangleFilled => brushes::get_filled_rectangle_points(p0, p1).into_iter().map(|(p, _)| p).collect(),
            Tool::EllipseOutline => {
                use std::collections::HashSet;
                let points = brushes::get_ellipse_points_from_rect(p0, p1);
                let mut set: HashSet<(i32, i32)> = HashSet::new();
                for (p, _) in points {
                    set.insert((p.x, p.y));
                }
                set.into_iter().map(|(x, y)| icy_engine::Position::new(x, y)).collect()
            }
            Tool::EllipseFilled => brushes::get_filled_ellipse_points_from_rect(p0, p1).into_iter().map(|(p, _)| p).collect(),
            _ => Vec::new(),
        }
    }

    fn update_shape_tool_overlay_preview(&mut self) {
        let is_shape_tool = matches!(
            self.current_tool,
            Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled
        );
        if !is_shape_tool || !self.is_dragging {
            return;
        }

        let is_half_block_mode = matches!(self.top_toolbar.brush_options.primary, BrushPrimaryMode::HalfBlock);

        // Use the same coordinate system as the terminal shader:
        // - x uses `render_info.font_width`
        // - y uses `render_info.font_height`, doubled when scan_lines is enabled.
        let (shader_font_w, shader_font_h, shader_scan_lines) = {
            let render_info = self.canvas.terminal.render_info.read();
            (render_info.font_width.max(1.0), render_info.font_height.max(1.0), render_info.scan_lines)
        };
        let shader_font_h_effective = shader_font_h * if shader_scan_lines { 2.0 } else { 1.0 };

        let debug_overlay = cfg!(debug_assertions) && std::env::var("ICY_DEBUG_TOOL_OVERLAY").is_ok_and(|v| v != "0");

        let alpha: u8 = 153; // ~0.6 like Moebius overlay

        let (overlay_rect_px, overlay_rgba) = (|| {
            use icy_engine_edit::brushes::{BrushMode as EngineBrushMode, ColorMode as EngineColorMode, DrawContext, DrawTarget, PointRole};

            let mut screen = self.screen.lock();
            let edit_state = screen
                .as_any_mut()
                .downcast_mut::<icy_engine_edit::EditState>()
                .expect("screen should be EditState");

            let base_buffer = edit_state.get_buffer();

            let caret_attr = edit_state.get_caret().attribute;
            let swap_colors = self.paint_button == iced::mouse::Button::Right;
            // Shift key also swaps colors
            let shift_swap = is_shift_pressed();

            let (primary, paint_char, brush_size, colorize_fg, colorize_bg) = {
                let opts = &self.top_toolbar.brush_options;
                (opts.primary, opts.paint_char, opts.brush_size.max(1), opts.colorize_fg, opts.colorize_bg)
            };

            let (doc_p0, doc_p1, points): (icy_engine_edit::Position, icy_engine_edit::Position, Vec<(icy_engine_edit::Position, bool)>) = if is_half_block_mode
            {
                // Convert layer-local half-block drag positions to document half-block coordinates.
                let offset = edit_state.get_cur_layer().map(|l| l.offset()).unwrap_or_default();
                let hb_off = icy_engine_edit::Position::new(offset.x, offset.y * 2);
                let start_hb = self.drag_pos.start_half_block + hb_off;
                let cur_hb = self.drag_pos.cur_half_block + hb_off;

                let pts_hb = Self::shape_points(self.current_tool, start_hb, cur_hb);
                let pts = pts_hb
                    .into_iter()
                    .filter(|p| p.y >= 0)
                    .map(|p| {
                        let cell = icy_engine_edit::Position::new(p.x, p.y / 2);
                        let is_top = (p.y % 2) == 0;
                        (cell, is_top)
                    })
                    .collect::<Vec<_>>();
                (start_hb, cur_hb, pts)
            } else {
                let p0 = self.drag_pos.start;
                let p1 = self.drag_pos.cur;
                let pts = Self::shape_points(self.current_tool, p0, p1)
                    .into_iter()
                    .filter(|p| p.x >= 0 && p.y >= 0)
                    .map(|p| (p, true))
                    .collect::<Vec<_>>();
                (p0, p1, pts)
            };

            let _ = (doc_p0, doc_p1); // keep for debugging symmetry

            if points.is_empty() {
                return (None, Vec::new());
            }

            // IMPORTANT: Temp-layer must match the current target layer size and position.
            // Most operations are defined in layer-local coordinates.
            let (layer_offset, layer_w, layer_h, layer_vis, layer_locked, layer_title) = if let Some(layer) = edit_state.get_cur_layer() {
                (
                    layer.offset(),
                    layer.width().max(1),
                    layer.height().max(1),
                    layer.properties.is_visible,
                    layer.properties.is_locked,
                    layer.title().to_string(),
                )
            } else {
                (
                    icy_engine_edit::Position::default(),
                    base_buffer.width().max(1),
                    base_buffer.height().max(1),
                    true,
                    false,
                    "<none>".to_string(),
                )
            };

            if debug_overlay {
                eprintln!(
                    "[tool_overlay] begin tool={:?} half_block={} clear={} button={:?} scan_lines={} font_w={} font_h={} eff_h={}",
                    self.current_tool,
                    is_half_block_mode,
                    self.shape_clear,
                    self.paint_button,
                    shader_scan_lines,
                    shader_font_w,
                    shader_font_h,
                    shader_font_h_effective
                );
                eprintln!(
                    "[tool_overlay] target_layer title='{}' off=({}, {}) size=({}, {}) visible={} locked={}",
                    layer_title, layer_offset.x, layer_offset.y, layer_w, layer_h, layer_vis, layer_locked
                );
            }

            // Prepare a temporary buffer sized like the target layer.
            let mut tmp = icy_engine_edit::TextBuffer::create((layer_w, layer_h));
            tmp.palette = base_buffer.palette.clone();
            tmp.palette_mode = base_buffer.palette_mode;
            tmp.ice_mode = base_buffer.ice_mode;
            tmp.font_mode = base_buffer.font_mode;
            tmp.set_font_table(base_buffer.font_table());
            tmp.set_font_dimensions(icy_engine_edit::Size::new(shader_font_w as i32, shader_font_h as i32));
            tmp.set_use_letter_spacing(base_buffer.use_letter_spacing());
            tmp.set_use_aspect_ratio(base_buffer.use_aspect_ratio());

            // TextBuffer::create uses default layer properties where `is_visible` is false.
            // We must explicitly enable it, otherwise `Layer::set_char` becomes a no-op
            // and the overlay diff ends up empty (invisible).
            if let Some(layer0) = tmp.layers.get_mut(0) {
                layer0.properties.is_visible = true;
                layer0.properties.is_locked = false;
            }

            if debug_overlay {
                let l0 = &tmp.layers[0];
                eprintln!(
                    "[tool_overlay] tmp_layer size=({}, {}) visible={} locked={} off=({}, {})",
                    l0.width(),
                    l0.height(),
                    l0.properties.is_visible,
                    l0.properties.is_locked,
                    l0.properties.offset.x,
                    l0.properties.offset.y
                );
            }

            // Fill from the current target layer.
            if let Some(layer) = edit_state.get_cur_layer() {
                for y in 0..layer_h {
                    for x in 0..layer_w {
                        let ch = layer.char_at(icy_engine_edit::Position::new(x, y));
                        tmp.layers[0].set_char((x, y), ch);
                    }
                }
            } else {
                // Fallback: no layer available, fill from composited screen.
                for y in 0..layer_h {
                    for x in 0..layer_w {
                        let doc_pos = icy_engine_edit::Position::new(layer_offset.x + x, layer_offset.y + y);
                        let ch = edit_state.char_at(doc_pos);
                        tmp.layers[0].set_char((x, y), ch);
                    }
                }
            }

            let options = icy_engine_edit::RenderOptions::from(icy_engine_edit::Rectangle::from(0, 0, layer_w, layer_h));

            if debug_overlay {
                let expected_px_w = (layer_w as f32 * shader_font_w).round() as i32;
                let expected_px_h = (layer_h as f32 * shader_font_h_effective).round() as i32;
                eprintln!(
                    "[tool_overlay] render_rect chars=({}, {}) expected_px=({}, {})",
                    layer_w, layer_h, expected_px_w, expected_px_h
                );
            }
            let (size_before, rgba_before) = tmp.render_to_rgba(&options, shader_scan_lines);

            if debug_overlay {
                let all_black = rgba_before.iter().all(|b| *b == 0);
                eprintln!(
                    "[tool_overlay] before_render size=({}, {}) bytes={} all black={}",
                    size_before.width,
                    size_before.height,
                    rgba_before.len(),
                    all_black
                );
            }

            // Apply the shape operation onto tmp.
            let use_selection = edit_state.is_something_selected();

            if debug_overlay {
                eprintln!("[tool_overlay] points_total={} use_selection={}", points.len(), use_selection);
            }

            if self.shape_clear {
                let mut in_bounds = 0usize;
                let mut sel_kept = 0usize;
                let mut changed_cells = 0usize;
                for (p, _) in &points {
                    // p is in document coordinates; map to layer-local.
                    let layer_pos = *p - layer_offset;
                    if layer_pos.x < 0 || layer_pos.y < 0 || layer_pos.x >= layer_w || layer_pos.y >= layer_h {
                        continue;
                    }
                    in_bounds += 1;
                    if use_selection && !edit_state.is_selected(*p) {
                        continue;
                    }
                    sel_kept += 1;
                    let before = tmp.layers[0].char_at(layer_pos);
                    let after = icy_engine_edit::AttributedChar::invisible();
                    if before != after {
                        changed_cells += 1;
                    }
                    tmp.layers[0].set_char(layer_pos, after);
                }

                if debug_overlay {
                    eprintln!(
                        "[tool_overlay] clear_op in_bounds={} after_selection={} changed_cells={} ",
                        in_bounds, sel_kept, changed_cells
                    );
                }
            } else {
                let brush_mode = match primary {
                    BrushPrimaryMode::Char => {
                        // Right-click in Char mode = erase (set to space)
                        if swap_colors {
                            EngineBrushMode::Char(' ')
                        } else {
                            EngineBrushMode::Char(paint_char)
                        }
                    }
                    BrushPrimaryMode::HalfBlock => EngineBrushMode::HalfBlock,
                    BrushPrimaryMode::Shading => {
                        if swap_colors {
                            EngineBrushMode::ShadeDown
                        } else {
                            EngineBrushMode::Shade
                        }
                    }
                    BrushPrimaryMode::Replace => EngineBrushMode::Replace(paint_char),
                    BrushPrimaryMode::Blink => EngineBrushMode::Blink(!swap_colors),
                    BrushPrimaryMode::Colorize => EngineBrushMode::Colorize,
                };

                let color_mode = if matches!(primary, BrushPrimaryMode::Colorize) {
                    match (colorize_fg, colorize_bg) {
                        (true, true) => EngineColorMode::Both,
                        (true, false) => EngineColorMode::Foreground,
                        (false, true) => EngineColorMode::Background,
                        (false, false) => EngineColorMode::None,
                    }
                } else {
                    EngineColorMode::Both
                };

                // Don't swap colors for Shading (has its own up/down behavior) or Char mode (right-click = erase)
                // Shift key swaps colors for all modes except those with special right-click behavior
                let swap_for_colors = (swap_colors || shift_swap) && !matches!(primary, BrushPrimaryMode::Shading | BrushPrimaryMode::Char);
                let (fg, bg) = if swap_for_colors {
                    (caret_attr.background(), caret_attr.foreground())
                } else {
                    (caret_attr.foreground(), caret_attr.background())
                };

                let mut template = caret_attr;
                template.set_foreground(fg);
                template.set_background(bg);

                struct BufferTarget<'a> {
                    buffer: &'a mut icy_engine_edit::TextBuffer,
                    width: i32,
                    height: i32,
                    changed_cells: &'a mut usize,
                }
                impl<'a> DrawTarget for BufferTarget<'a> {
                    fn width(&self) -> i32 {
                        self.width
                    }
                    fn height(&self) -> i32 {
                        self.height
                    }
                    fn char_at(&self, pos: icy_engine_edit::Position) -> Option<icy_engine_edit::AttributedChar> {
                        if pos.x < 0 || pos.y < 0 || pos.x >= self.width || pos.y >= self.height {
                            return None;
                        }
                        Some(self.buffer.char_at(pos))
                    }
                    fn set_char(&mut self, pos: icy_engine_edit::Position, ch: icy_engine_edit::AttributedChar) {
                        let before = self.buffer.char_at(pos);
                        if before != ch {
                            *self.changed_cells += 1;
                        }
                        self.buffer.layers[0].set_char(pos, ch);
                    }
                }

                let mut changed_cells = 0usize;
                let mut target = BufferTarget {
                    buffer: &mut tmp,
                    width: layer_w,
                    height: layer_h,
                    changed_cells: &mut changed_cells,
                };

                // For HalfBlock mode, brush_size expansion happens in half-block coordinates,
                // so here we use size 1. For other modes, use the actual brush_size.
                let effective_brush_size = if is_half_block_mode { 1 } else { brush_size as i32 };
                let half = effective_brush_size / 2;

                let mut in_bounds = 0usize;
                let mut sel_kept = 0usize;
                for (p, is_top) in &points {
                    // Expand each point by brush_size (like in apply_paint_stamp_with_half_block_info)
                    for dy in 0..effective_brush_size {
                        for dx in 0..effective_brush_size {
                            let expanded_pos = *p + icy_engine_edit::Position::new(dx - half, dy - half);
                            let layer_pos = expanded_pos - layer_offset;
                            if layer_pos.x < 0 || layer_pos.y < 0 || layer_pos.x >= layer_w || layer_pos.y >= layer_h {
                                continue;
                            }
                            in_bounds += 1;
                            if use_selection && !edit_state.is_selected(expanded_pos) {
                                continue;
                            }
                            sel_kept += 1;

                            let ctx = DrawContext::default()
                                .with_brush_mode(brush_mode.clone())
                                .with_color_mode(color_mode.clone())
                                .with_foreground(fg)
                                .with_background(bg)
                                .with_template_attribute(template)
                                .with_half_block_is_top(*is_top);
                            ctx.plot_point(&mut target, layer_pos, PointRole::Fill);
                        }
                    }
                }

                if debug_overlay {
                    eprintln!(
                        "[tool_overlay] draw_op in_bounds={} after_selection={} changed_cells={} ",
                        in_bounds, sel_kept, changed_cells
                    );
                }
            }

            let (size_after, rgba_after) = tmp.render_to_rgba(&options, shader_scan_lines);
            if size_before != size_after || rgba_before.len() != rgba_after.len() {
                if debug_overlay {
                    eprintln!(
                        "[tool_overlay] ERROR size_mismatch before=({}, {}) after=({}, {}) bytes_before={} bytes_after={}",
                        size_before.width,
                        size_before.height,
                        size_after.width,
                        size_after.height,
                        rgba_before.len(),
                        rgba_after.len()
                    );
                }
                return (None, Vec::new());
            }

            let mut overlay = Vec::with_capacity(rgba_after.len());
            let mut changed_pixels = 0usize;
            for i in (0..rgba_after.len()).step_by(4) {
                let pixel_changed = rgba_after[i..i + 4] != rgba_before[i..i + 4];
                if pixel_changed {
                    changed_pixels += 1;
                    overlay.push(rgba_after[i]);
                    overlay.push(rgba_after[i + 1]);
                    overlay.push(rgba_after[i + 2]);
                    overlay.push(alpha);
                } else {
                    overlay.extend_from_slice(&[0u8, 0u8, 0u8, 0u8]);
                }
            }

            let rect_x = layer_offset.x as f32 * shader_font_w;
            let rect_y = layer_offset.y as f32 * shader_font_h_effective;
            let rect_w = size_after.width as f32;
            let rect_h = size_after.height as f32;

            if debug_overlay {
                eprintln!(
                    "[tool_overlay] after_render size=({}, {}) changed_pixels={} rect=({}, {}, {}, {})",
                    size_after.width, size_after.height, changed_pixels, rect_x, rect_y, rect_w, rect_h
                );
            }

            (Some((rect_x, rect_y, rect_w, rect_h)), overlay)
        })();

        if let (Some((x, y, w, h)), rgba) = (overlay_rect_px, overlay_rgba) {
            if w > 0.0 && h > 0.0 {
                let mask_w = w as u32;
                let mask_h = h as u32;
                self.canvas.set_tool_overlay_mask(Some((rgba, mask_w, mask_h)), Some((x, y, w, h)));
                return;
            }
        }

        self.clear_tool_overlay();
    }

    fn set_current_fkey_set(&mut self, set_idx: usize) {
        let fkeys_to_save = {
            let mut opts = self.options.write();
            opts.fkeys.clamp_current_set();

            let count = opts.fkeys.set_count();
            let clamped = if count == 0 { 0 } else { set_idx % count };

            self.top_toolbar.select_options.current_fkey_page = clamped;
            opts.fkeys.current_set = clamped;
            opts.fkeys.clone()
        };

        // Save off-thread to avoid blocking the UI/event loop.
        std::thread::spawn(move || {
            let _ = fkeys_to_save.save();
        });
    }

    fn snapshot_tags(&mut self) -> Vec<TagListItem> {
        self.with_edit_state(|state| {
            state
                .get_buffer()
                .tags
                .iter()
                .enumerate()
                .map(|(index, tag)| TagListItem {
                    index,
                    is_enabled: tag.is_enabled,
                    preview: tag.preview.clone(),
                    replacement_value: tag.replacement_value.clone(),
                    position: tag.position,
                    placement: tag.tag_placement,
                })
                .collect()
        })
    }

    fn type_fkey_slot(&mut self, slot: usize) {
        let set_idx = self.top_toolbar.select_options.current_fkey_page;
        let code = {
            let opts = self.options.read();
            opts.fkeys.code_at(set_idx, slot)
        };

        let buffer_type = self.with_edit_state(|state| state.get_buffer().buffer_type);

        let raw = char::from_u32(code as u32).unwrap_or(' ');
        let unicode_cp437 = BufferType::CP437.convert_to_unicode(raw);
        let target = buffer_type.convert_from_unicode(unicode_cp437);

        let result: Result<(), icy_engine::EngineError> = self.with_edit_state(|state| state.type_key(target));
        if let Err(e) = result {
            log::warn!("Failed to type fkey (set {}, slot {}): {}", set_idx, slot, e);
        }
    }

    /// Set the file path (for session restore)
    pub fn set_file_path(&mut self, path: PathBuf) {
        self.file_path = Some(path);
    }

    /// Load from an autosave file, using the original path for format detection
    ///
    /// The autosave file is always saved in ICY format (to preserve layers, fonts, etc.),
    /// but we set the original path so future saves use the correct format.
    pub fn load_from_autosave(
        autosave_path: &std::path::Path,
        original_path: PathBuf,
        options: Arc<RwLock<Options>>,
        font_library: SharedFontLibrary,
    ) -> anyhow::Result<Self> {
        // Autosaves are always saved in ICY format
        let format = FileFormat::IcyDraw;

        // Read autosave data
        let data = std::fs::read(autosave_path)?;

        // Load buffer using ICY format
        let load_data = LoadData::default();
        let buffer = format.from_bytes(&data, Some(load_data))?.buffer;

        let mut editor = Self::with_buffer(buffer, Some(original_path), options, font_library);
        editor.is_modified = true; // Autosave means we have unsaved changes
        Ok(editor)
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

    /// Get the character at the given position from the current layer
    fn get_char_at(&self, pos: icy_engine::Position) -> icy_engine::AttributedChar {
        let mut screen = self.screen.lock();
        if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
            if let Some(cur_layer) = edit_state.get_cur_layer() {
                cur_layer.char_at(pos - cur_layer.offset())
            } else {
                icy_engine::AttributedChar::invisible()
            }
        } else {
            icy_engine::AttributedChar::invisible()
        }
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
        crate::CLIPBOARD_CONTEXT.has(ContentFormat::Other(ICY_CLIPBOARD_TYPE.into()))
            || crate::CLIPBOARD_CONTEXT.has(ContentFormat::Image)
            || crate::CLIPBOARD_CONTEXT.has(ContentFormat::Text)
    }

    /// Paste from clipboard (ICY format, image, or text)
    /// Creates a floating layer that can be positioned before anchoring
    pub fn paste(&mut self) -> Result<(), String> {
        // Don't paste if already in paste mode
        if self.is_paste_mode() {
            return Ok(());
        }

        // Save current tool to restore after paste mode ends
        self.paste_previous_tool = Some(self.current_tool);

        let mut screen = self.screen.lock();
        let edit_state = screen
            .as_any_mut()
            .downcast_mut::<EditState>()
            .ok_or_else(|| "Could not access edit state".to_string())?;

        // Priority: ICY binary > Image > Text
        if let Ok(data) = crate::CLIPBOARD_CONTEXT.get_buffer(ICY_CLIPBOARD_TYPE) {
            log::debug!("paste: Using ICY binary data, size={}", data.len());
            edit_state.paste_clipboard_data(&data).map_err(|e| e.to_string())?;
        } else if let Ok(img) = crate::CLIPBOARD_CONTEXT.get_image() {
            log::debug!("paste: Using image data");
            let mut sixel = Sixel::new(Position::default());
            sixel.picture_data = img.to_rgba8().map_err(|e| e.to_string())?.as_raw().clone();
            let (w, h) = img.get_size();
            sixel.set_width(w as i32);
            sixel.set_height(h as i32);
            edit_state.paste_sixel(sixel).map_err(|e| e.to_string())?;
        } else if let Ok(text) = crate::CLIPBOARD_CONTEXT.get_text() {
            log::debug!("paste: Using text data: {:?}", text);
            edit_state.paste_text(&text).map_err(|e| e.to_string())?;
        } else {
            // Paste failed, restore tool state
            self.paste_previous_tool = None;
            return Err("No compatible clipboard content".to_string());
        }

        // Drop the screen lock before calling methods that need it
        drop(screen);

        // Update layer bounds to show the floating layer
        self.update_layer_bounds();

        Ok(())
    }

    /// Check if we are in paste mode (floating layer active for positioning)
    /// This is the primary check for paste mode UI and input handling
    pub fn is_paste_mode(&self) -> bool {
        self.paste_previous_tool.is_some()
    }

    /// Anchor the floating layer (finalize paste)
    #[allow(dead_code)]
    pub fn anchor_layer(&mut self) -> Result<(), String> {
        // If user anchors while a paste drag is active, first commit the drag position.
        let pending_paste_move = if self.paste_drag_active {
            let delta = self.drag_pos.cur - self.drag_pos.start;
            Some(Position::new(
                self.paste_drag_start_offset.x + delta.x,
                self.paste_drag_start_offset.y + delta.y,
            ))
        } else {
            None
        };

        let result = {
            let mut screen = self.screen.lock();
            if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
                if let Some(new_offset) = pending_paste_move {
                    edit_state.set_layer_preview_offset(None);
                    let _ = edit_state.move_layer(new_offset);
                }
                edit_state.anchor_layer().map_err(|e| e.to_string())
            } else {
                Err("Could not access edit state".to_string())
            }
        };

        // Restore previous tool after paste mode ends
        self.restore_previous_tool();

        result
    }

    /// Discard the floating layer (cancel paste)
    #[allow(dead_code)]
    pub fn discard_floating_layer(&mut self) -> Result<(), String> {
        // Only discard if in paste mode
        if !self.is_paste_mode() {
            return Ok(());
        }

        // Cancel any active paste drag + atomic undo guard.
        self.paste_drag_active = false;
        self.paste_drag_undo = None;
        self.end_drag_capture();

        let result = {
            let mut screen = self.screen.lock();
            if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
                // Remove the floating layer by undoing the paste
                edit_state.undo().map_err(|e| e.to_string())?;
                Ok(())
            } else {
                Err("Could not access edit state".to_string())
            }
        };

        // Restore previous tool after paste mode ends
        self.restore_previous_tool();

        result
    }

    /// Restore the tool that was active before paste mode started
    fn restore_previous_tool(&mut self) {
        // Leaving paste mode: ensure no drag/capture state leaks out.
        self.paste_drag_active = false;
        self.paste_drag_undo = None;
        self.end_drag_capture();

        if let Some(tool) = self.paste_previous_tool.take() {
            // Temporarily clear floating layer check to allow tool change
            self.current_tool = tool;
            self.tool_panel.set_tool(tool);
            self.update_mouse_tracking_mode();
        }
    }

    /// Stamp the floating layer down without anchoring (copies the layer content to the layer below)
    pub fn stamp_floating_layer(&mut self) {
        let mut screen = self.screen.lock();
        if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
            if let Err(e) = edit_state.stamp_layer_down() {
                log::warn!("Failed to stamp layer: {}", e);
            }
        }
    }

    /// Rotate the floating layer 90° clockwise
    pub fn rotate_floating_layer(&mut self) {
        let mut screen = self.screen.lock();
        if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
            if let Err(e) = edit_state.rotate_layer() {
                log::warn!("Failed to rotate layer: {}", e);
            }
        }
    }

    /// Flip the floating layer horizontally
    pub fn flip_floating_layer_x(&mut self) {
        let mut screen = self.screen.lock();
        if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
            if let Err(e) = edit_state.flip_x() {
                log::warn!("Failed to flip layer X: {}", e);
            }
        }
    }

    /// Flip the floating layer vertically
    pub fn flip_floating_layer_y(&mut self) {
        let mut screen = self.screen.lock();
        if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
            if let Err(e) = edit_state.flip_y() {
                log::warn!("Failed to flip layer Y: {}", e);
            }
        }
    }

    /// Toggle transparent mode for the floating layer
    pub fn toggle_floating_layer_transparent(&mut self) {
        let mut screen = self.screen.lock();
        if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
            if let Err(e) = edit_state.make_layer_transparent() {
                log::warn!("Failed to make layer transparent: {}", e);
            }
        }
    }

    /// Create sidebar controls for paste mode (replaces tool panel)
    fn view_paste_sidebar_controls(&self) -> Element<'_, AnsiEditorMessage> {
        use iced::widget::{Space, button, column, container, text};
        use icy_engine_gui::ui::TEXT_SIZE_SMALL;

        // Helper to create a paste action button
        fn paste_btn<'a>(label: &'a str, msg: TopToolbarMessage) -> Element<'a, AnsiEditorMessage> {
            button(text(label).size(TEXT_SIZE_SMALL).center())
                .width(Length::Fill)
                .padding([6, 8])
                .on_press(AnsiEditorMessage::TopToolbar(msg))
                .into()
        }

        let content = column![
            text("Paste Mode").size(14),
            Space::new().height(Length::Fixed(8.0)),
            paste_btn("✓ Anchor (Enter)", TopToolbarMessage::PasteAnchor),
            paste_btn("✕ Cancel (Esc)", TopToolbarMessage::PasteCancel),
            Space::new().height(Length::Fixed(12.0)),
            text("Transform").size(12),
            paste_btn("Stamp (S)", TopToolbarMessage::PasteStamp),
            paste_btn("Rotate (R)", TopToolbarMessage::PasteRotate),
            paste_btn("Flip X", TopToolbarMessage::PasteFlipX),
            paste_btn("Flip Y", TopToolbarMessage::PasteFlipY),
            paste_btn("Transparent (T)", TopToolbarMessage::PasteToggleTransparent),
            Space::new().height(Length::Fill),
        ]
        .spacing(4)
        .width(Length::Fill);

        container(content).width(Length::Fill).padding(8).into()
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

    /// Get bytes for autosave (saves in ICY format with thumbnail skipped for performance)
    pub fn get_autosave_bytes(&self) -> Result<Vec<u8>, String> {
        let mut screen = self.screen.lock();
        if let Some(edit_state) = screen.as_any_mut().downcast_ref::<EditState>() {
            // Use ICY format for autosave to preserve all data (layers, fonts, etc.)
            let format = FileFormat::IcyDraw;
            let buffer = edit_state.get_buffer();
            // Skip thumbnail generation for faster autosave
            let mut options = icy_engine::AnsiSaveOptionsV2::default();
            options.skip_thumbnail = true;
            format.to_bytes(buffer, &options).map_err(|e| e.to_string())
        } else {
            Err("Could not access edit state".to_string())
        }
    }

    /// Check if this editor needs animation updates (for smooth animations)
    pub fn needs_animation(&self) -> bool {
        self.current_tool == Tool::Click || self.tool_panel.needs_animation() || self.minimap_drag_pointer.is_some()
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
                // Avoid stacked modals.
                self.tag_dialog = None;
                self.tag_list_dialog = Some(TagListDialog::new(self.snapshot_tags()));
                Task::none()
            }
            AnsiEditorMessage::TagListDialog(msg) => match msg {
                TagListDialogMessage::Close => {
                    self.tag_list_dialog = None;
                    Task::none()
                }
                TagListDialogMessage::Delete(index) => {
                    if let Err(err) = self.with_edit_state(|state| state.remove_tag(index)) {
                        log::warn!("Failed to remove tag: {}", err);
                        return Task::none();
                    }
                    self.handle_tool_event(ToolEvent::Commit("Remove tag".to_string()));
                    let items = self.snapshot_tags();
                    if let Some(dialog) = self.tag_list_dialog.as_mut() {
                        dialog.items = items;
                    }
                    Task::none()
                }
            },
            AnsiEditorMessage::TagDialog(msg) => {
                let Some(dialog) = &mut self.tag_dialog else {
                    return Task::none();
                };

                match msg {
                    TagDialogMessage::SetPreview(s) => {
                        dialog.preview = s.clone();
                        Task::none()
                    }
                    TagDialogMessage::SetReplacement(s) => {
                        dialog.replacement_value = s.clone();
                        Task::none()
                    }
                    TagDialogMessage::SetPosX(s) => {
                        dialog.pos_x = s;
                        Task::none()
                    }
                    TagDialogMessage::SetPosY(s) => {
                        dialog.pos_y = s;
                        Task::none()
                    }
                    TagDialogMessage::SetPlacement(p) => {
                        dialog.placement = p;
                        Task::none()
                    }
                    TagDialogMessage::Cancel => {
                        self.tag_dialog = None;
                        Task::none()
                    }
                    TagDialogMessage::Ok => {
                        let mut position = dialog.position;
                        let preview = dialog.preview.trim().to_string();
                        let replacement_value = dialog.replacement_value.clone();
                        let placement = dialog.placement;
                        let pos_x = dialog.pos_x.trim().to_string();
                        let pos_y = dialog.pos_y.trim().to_string();
                        let edit_index = dialog.edit_index;
                        let tag_length = dialog.length.unwrap_or(0);
                        let from_selection = dialog.from_selection;
                        self.tag_dialog = None;

                        if preview.is_empty() {
                            return Task::none();
                        }

                        if let Ok(x) = pos_x.parse::<i32>() {
                            position.x = x;
                        }
                        if let Ok(y) = pos_y.parse::<i32>() {
                            position.y = y;
                        }

                        let attribute = self.with_edit_state(|state| state.get_caret().attribute);
                        let new_tag = Tag {
                            is_enabled: true,
                            preview,
                            replacement_value,
                            position,
                            length: tag_length,
                            alignment: std::fmt::Alignment::Left,
                            tag_placement: placement.to_engine(),
                            tag_role: TagRole::Displaycode,
                            attribute,
                        };

                        let commit_message = if edit_index.is_some() { "Edit tag" } else { "Add tag" };

                        if let Some(index) = edit_index {
                            // Edit existing tag
                            if let Err(err) = self.with_edit_state(|state| {
                                let size = state.get_buffer().size();
                                let max_x = (size.width - 1).max(0);
                                let max_y = (size.height - 1).max(0);

                                let mut new_tag = new_tag;
                                new_tag.position.x = new_tag.position.x.clamp(0, max_x);
                                new_tag.position.y = new_tag.position.y.clamp(0, max_y);

                                state.update_tag(new_tag, index)
                            }) {
                                log::warn!("Failed to update tag: {}", err);
                                return Task::none();
                            }
                        } else {
                            // Add new tag
                            if let Err(err) = self.with_edit_state(|state| {
                                let size = state.get_buffer().size();
                                let max_x = (size.width - 1).max(0);
                                let max_y = (size.height - 1).max(0);

                                let mut new_tag = new_tag;
                                new_tag.position.x = new_tag.position.x.clamp(0, max_x);
                                new_tag.position.y = new_tag.position.y.clamp(0, max_y);

                                if !state.get_buffer().show_tags {
                                    state.show_tags(true)?;
                                }
                                state.add_new_tag(new_tag)?;

                                // Clear selection if tag was created from selection
                                if from_selection {
                                    let _ = state.clear_selection();
                                }
                                Ok::<(), icy_engine::EngineError>(())
                            }) {
                                log::warn!("Failed to add tag: {}", err);
                                return Task::none();
                            }
                        }
                        self.commit_and_update_tag_overlays(commit_message.to_string())
                    }
                }
            }
            AnsiEditorMessage::TagEdit(index) => {
                self.close_tag_context_menu();
                let tag = self.with_edit_state(|state| state.get_buffer().tags.get(index).cloned());
                if let Some(tag) = tag {
                    self.tag_list_dialog = None;
                    self.tag_dialog = Some(TagDialog::edit(&tag, index));
                }
                Task::none()
            }
            AnsiEditorMessage::TagDelete(index) => {
                self.close_tag_context_menu();
                if let Err(err) = self.with_edit_state(|state| state.remove_tag(index)) {
                    log::warn!("Failed to remove tag: {}", err);
                    return Task::none();
                }
                self.tag_selection.retain(|&i| i != index);
                // Adjust indices for tags after the deleted one
                for i in &mut self.tag_selection {
                    if *i > index {
                        *i -= 1;
                    }
                }
                self.commit_and_update_tag_overlays("Remove tag")
            }
            AnsiEditorMessage::TagClone(index) => {
                self.close_tag_context_menu();
                if let Err(err) = self.with_edit_state(|state| state.clone_tag(index)) {
                    log::warn!("Failed to clone tag: {}", err);
                    return Task::none();
                }
                self.commit_and_update_tag_overlays("Clone tag")
            }
            AnsiEditorMessage::TagContextMenuClose => {
                self.close_tag_context_menu();
                Task::none()
            }
            AnsiEditorMessage::TagDeleteSelected => {
                self.close_tag_context_menu();
                // Delete tags in reverse order to keep indices valid
                let mut indices: Vec<usize> = self.tag_selection.clone();
                indices.sort_by(|a, b| b.cmp(a)); // Sort descending
                for index in indices {
                    if let Err(err) = self.with_edit_state(|state| state.remove_tag(index)) {
                        log::warn!("Failed to remove tag {}: {}", index, err);
                    }
                }
                let count = self.tag_selection.len();
                self.tag_selection.clear();
                self.commit_and_update_tag_overlays(format!("Remove {} tags", count))
            }
            AnsiEditorMessage::TagStartAddMode => {
                // Create a new tag immediately and start dragging it
                self.close_tag_context_menu();

                // Generate unique tag name
                let next_tag_name = self.generate_next_tag_name();
                let attribute = self.with_edit_state(|state| state.get_caret().attribute);

                // Create new tag at origin (will be moved by drag)
                let new_tag = icy_engine::Tag {
                    position: icy_engine::Position::default(),
                    length: next_tag_name.len(),
                    preview: next_tag_name,
                    is_enabled: true,
                    alignment: std::fmt::Alignment::Left,
                    replacement_value: String::new(),
                    tag_placement: icy_engine::TagPlacement::InText,
                    tag_role: icy_engine::TagRole::Displaycode,
                    attribute,
                };

                // Start atomic undo for the add operation
                self.tag_drag_undo = Some(self.with_edit_state(|state| state.begin_atomic_undo("Add tag")));

                // Add the tag
                if let Err(err) = self.with_edit_state(|state| {
                    if !state.get_buffer().show_tags {
                        let _ = state.show_tags(true);
                    }
                    state.add_new_tag(new_tag)
                }) {
                    log::warn!("Failed to add tag: {}", err);
                    self.tag_drag_undo = None;
                    return Task::none();
                }

                // Get the index of the new tag and start dragging it
                let new_index = self.with_edit_state(|state| state.get_buffer().tags.len() - 1);
                self.tag_add_new_index = Some(new_index);
                self.tag_selection.clear();
                self.tag_selection.push(new_index);
                self.tag_drag_active = true;
                self.tag_drag_indices = vec![new_index];
                self.tag_drag_start_positions = vec![icy_engine::Position::default()];
                self.is_dragging = true;

                self.task_none_with_tag_overlays_update()
            }
            AnsiEditorMessage::TagCancelAddMode => {
                // Cancel adding tag - undo the add operation
                if self.tag_add_new_index.is_some() {
                    self.tag_add_new_index = None;
                    self.end_tag_drag();
                    // Undo the add operation
                    let _ = self.with_edit_state(|state| state.undo());
                    return self.task_none_with_tag_overlays_update();
                }
                Task::none()
            }
            AnsiEditorMessage::ToolPanel(msg) => {
                // Block tool panel changes during paste mode
                if self.is_paste_mode() {
                    return Task::none();
                }
                // Handle tool panel messages
                match &msg {
                    ToolPanelMessage::ClickSlot(_) => {
                        // After the tool panel updates, sync our current_tool
                        let _ = self.tool_panel.update(msg.clone());
                        self.change_tool(self.tool_panel.current_tool());
                    }
                    ToolPanelMessage::Tick(delta) => {
                        self.tool_panel.tick(*delta);
                    }
                }
                Task::none()
            }
            AnsiEditorMessage::Canvas(msg) => {
                // Handle clipboard operations from context menu
                match &msg {
                    CanvasMessage::Cut => {
                        if let Err(e) = self.cut() {
                            log::error!("Cut failed: {}", e);
                        }
                        return Task::none();
                    }
                    CanvasMessage::Copy => {
                        if let Err(e) = self.copy() {
                            log::error!("Copy failed: {}", e);
                        }
                        return Task::none();
                    }
                    CanvasMessage::Paste => {
                        if let Err(e) = self.paste() {
                            log::error!("Paste failed: {}", e);
                        }
                        return Task::none();
                    }
                    _ => {}
                }

                // Intercept terminal mouse events and forward to tool handling
                match &msg {
                    CanvasMessage::TerminalMessage(terminal_msg) => match terminal_msg {
                        icy_engine_gui::Message::Press(evt) => {
                            if let Some(text_pos) = evt.text_position {
                                let event = CanvasMouseEvent::Press {
                                    position: text_pos,
                                    pixel_position: evt.pixel_position,
                                    button: convert_mouse_button(evt.button),
                                    modifiers: evt.modifiers.clone(),
                                };
                                self.handle_canvas_mouse_event(event);
                            }
                        }
                        icy_engine_gui::Message::Release(evt) => {
                            if let Some(text_pos) = evt.text_position {
                                let event = CanvasMouseEvent::Release {
                                    position: text_pos,
                                    pixel_position: evt.pixel_position,
                                    button: convert_mouse_button(evt.button),
                                };
                                self.handle_canvas_mouse_event(event);
                            }
                        }
                        icy_engine_gui::Message::Move(evt) | icy_engine_gui::Message::Drag(evt) => {
                            if let Some(text_pos) = evt.text_position {
                                let event = CanvasMouseEvent::Move {
                                    position: text_pos,
                                    pixel_position: evt.pixel_position,
                                };
                                self.handle_canvas_mouse_event(event);
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
                self.canvas.update(msg).map(AnsiEditorMessage::Canvas)
            }
            AnsiEditorMessage::RightPanel(msg) => {
                // Handle minimap click-to-navigate before passing to right_panel
                if let RightPanelMessage::Minimap(ref minimap_msg) = msg {
                    match minimap_msg {
                        MinimapMessage::Click {
                            norm_x,
                            norm_y,
                            pointer_x,
                            pointer_y,
                        }
                        | MinimapMessage::Drag {
                            norm_x,
                            norm_y,
                            pointer_x,
                            pointer_y,
                        } => {
                            self.minimap_drag_pointer = Some((*pointer_x, *pointer_y));
                            // Convert normalized position to content coordinates and scroll
                            self.scroll_canvas_to_normalized(*norm_x, *norm_y);
                        }
                        MinimapMessage::DragEnd => {
                            self.minimap_drag_pointer = None;
                        }
                        _ => {}
                    }
                }
                // Handle layer messages - translate to AnsiEditorMessage
                if let RightPanelMessage::Layers(ref layer_msg) = msg {
                    match layer_msg {
                        LayerMessage::Select(idx) => {
                            // Check for double-click
                            if self.right_panel.layers.check_double_click(*idx) {
                                return Task::done(AnsiEditorMessage::EditLayer(*idx));
                            }
                            return Task::done(AnsiEditorMessage::SelectLayer(*idx));
                        }
                        LayerMessage::ToggleVisibility(idx) => {
                            return Task::done(AnsiEditorMessage::ToggleLayerVisibility(*idx));
                        }
                        LayerMessage::Add => {
                            return Task::done(AnsiEditorMessage::AddLayer);
                        }
                        LayerMessage::Remove(idx) => {
                            return Task::done(AnsiEditorMessage::RemoveLayer(*idx));
                        }
                        LayerMessage::MoveUp(idx) => {
                            return Task::done(AnsiEditorMessage::MoveLayerUp(*idx));
                        }
                        LayerMessage::MoveDown(idx) => {
                            return Task::done(AnsiEditorMessage::MoveLayerDown(*idx));
                        }
                        LayerMessage::Rename(_, _) => {
                            // TODO: Implement layer rename
                        }
                        LayerMessage::EditLayer(idx) => {
                            return Task::done(AnsiEditorMessage::EditLayer(*idx));
                        }
                        LayerMessage::Duplicate(idx) => {
                            return Task::done(AnsiEditorMessage::DuplicateLayer(*idx));
                        }
                        LayerMessage::MergeDown(idx) => {
                            return Task::done(AnsiEditorMessage::MergeLayerDown(*idx));
                        }
                        LayerMessage::Clear(idx) => {
                            return Task::done(AnsiEditorMessage::ClearLayer(*idx));
                        }
                    }
                }
                self.right_panel.update(msg).map(AnsiEditorMessage::RightPanel)
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
                        self.type_fkey_slot(slot);
                        self.handle_tool_event(ToolEvent::Commit("Type fkey".to_string()));
                        Task::none()
                    }
                    TopToolbarMessage::NextFKeyPage => {
                        let next = self.top_toolbar.select_options.current_fkey_page.saturating_add(1);
                        self.set_current_fkey_set(next);
                        Task::none()
                    }
                    TopToolbarMessage::PrevFKeyPage => {
                        let cur = self.top_toolbar.select_options.current_fkey_page;
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
                        self.font_tool.select_font(index);
                        self.font_tool.prev_char = '\0'; // Reset kerning
                        Task::none()
                    }
                    TopToolbarMessage::SelectOutline(index) => {
                        // Update outline style in options
                        *self.options.read().font_outline_style.write() = index;
                        Task::none()
                    }
                    TopToolbarMessage::OpenOutlineSelector => {
                        // Open the outline selector popup
                        self.outline_selector_open = true;
                        Task::none()
                    }
                    TopToolbarMessage::OpenFontSelector => {
                        // This will be handled by main_window to open the dialog
                        // Return a task that signals this (handled via Message routing)
                        Task::none()
                    }
                    TopToolbarMessage::OpenTagList => {
                        // Delegate to the existing OpenTagListDialog handler
                        return self.update(AnsiEditorMessage::OpenTagListDialog);
                    }
                    TopToolbarMessage::StartAddTag => {
                        // Toggle add tag mode or start it
                        if self.tag_add_new_index.is_some() {
                            // Already in add mode - toggle off (cancel)
                            return self.update(AnsiEditorMessage::TagCancelAddMode);
                        } else {
                            return self.update(AnsiEditorMessage::TagStartAddMode);
                        }
                    }
                    TopToolbarMessage::EditSelectedTag => {
                        // Edit the first selected tag
                        if let Some(&idx) = self.tag_selection.first() {
                            return self.update(AnsiEditorMessage::TagEdit(idx));
                        }
                        Task::none()
                    }
                    TopToolbarMessage::DeleteSelectedTags => {
                        // Delete all selected tags
                        if !self.tag_selection.is_empty() {
                            return self.update(AnsiEditorMessage::TagDeleteSelected);
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
                            self.tool_panel.set_tool(self.current_tool);
                        }

                        let task = self.top_toolbar.update(TopToolbarMessage::ToggleFilled(v)).map(AnsiEditorMessage::TopToolbar);
                        self.update_mouse_tracking_mode();
                        task
                    }

                    // === Paste Mode Actions ===
                    TopToolbarMessage::PasteStamp => {
                        self.stamp_floating_layer();
                        Task::none()
                    }
                    TopToolbarMessage::PasteRotate => {
                        self.rotate_floating_layer();
                        Task::none()
                    }
                    TopToolbarMessage::PasteFlipX => {
                        self.flip_floating_layer_x();
                        Task::none()
                    }
                    TopToolbarMessage::PasteFlipY => {
                        self.flip_floating_layer_y();
                        Task::none()
                    }
                    TopToolbarMessage::PasteToggleTransparent => {
                        self.toggle_floating_layer_transparent();
                        Task::none()
                    }
                    TopToolbarMessage::PasteAnchor => {
                        if let Err(e) = self.anchor_layer() {
                            log::error!("Failed to anchor layer: {}", e);
                        }
                        Task::none()
                    }
                    TopToolbarMessage::PasteCancel => {
                        if let Err(e) = self.discard_floating_layer() {
                            log::error!("Failed to discard floating layer: {}", e);
                        }
                        Task::none()
                    }

                    _ => {
                        let task = self.top_toolbar.update(msg).map(AnsiEditorMessage::TopToolbar);
                        self.update_mouse_tracking_mode();
                        task
                    }
                }
            }
            AnsiEditorMessage::FKeyToolbar(msg) => {
                match msg {
                    FKeyToolbarMessage::TypeFKey(slot) => {
                        self.type_fkey_slot(slot);
                        self.handle_tool_event(ToolEvent::Commit("Type fkey".to_string()));
                        self.fkey_toolbar.clear_cache();
                    }
                    FKeyToolbarMessage::OpenCharSelector(slot) => {
                        // Open the character selector popup for this F-key slot
                        self.char_selector_target = Some(CharSelectorTarget::FKeySlot(slot));
                    }
                    FKeyToolbarMessage::NextSet => {
                        let next = self.top_toolbar.select_options.current_fkey_page.saturating_add(1);
                        self.set_current_fkey_set(next);
                        self.fkey_toolbar.clear_cache();
                    }
                    FKeyToolbarMessage::PrevSet => {
                        let cur = self.top_toolbar.select_options.current_fkey_page;
                        let prev = {
                            let opts = self.options.read();
                            let count = opts.fkeys.set_count();
                            if count == 0 { 0 } else { (cur + count - 1) % count }
                        };
                        self.set_current_fkey_set(prev);
                        self.fkey_toolbar.clear_cache();
                    }
                }
                Task::none()
            }
            AnsiEditorMessage::CharSelector(msg) => {
                match msg {
                    CharSelectorMessage::SelectChar(code) => {
                        match self.char_selector_target {
                            Some(CharSelectorTarget::FKeySlot(slot)) => {
                                // Update the F-key slot with the selected character
                                let set_idx = self.top_toolbar.select_options.current_fkey_page;
                                let fkeys_to_save = {
                                    let mut opts = self.options.write();
                                    opts.fkeys.set_code_at(set_idx, slot, code);
                                    opts.fkeys.clone()
                                };
                                // Trigger async save
                                std::thread::spawn(move || {
                                    let _ = fkeys_to_save.save();
                                });
                                self.fkey_toolbar.clear_cache();
                            }
                            Some(CharSelectorTarget::BrushChar) => {
                                // Update the brush paint character
                                let ch = char::from_u32(code as u32).unwrap_or(' ');
                                self.top_toolbar.brush_options.paint_char = ch;
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
                match msg {
                    OutlineSelectorMessage::SelectOutline(style) => {
                        // Update outline style in options
                        *self.options.read().font_outline_style.write() = style;
                        self.outline_selector_open = false;
                    }
                    OutlineSelectorMessage::Cancel => {
                        self.outline_selector_open = false;
                    }
                }
                Task::none()
            }
            AnsiEditorMessage::ColorSwitcher(msg) => {
                match msg {
                    ColorSwitcherMessage::SwapColors => {
                        // Just start the animation, don't swap yet
                        self.color_switcher.start_swap_animation();
                    }
                    ColorSwitcherMessage::AnimationComplete => {
                        // Animation finished - now actually swap the colors
                        let (fg, bg) = self.with_edit_state(|state| state.swap_caret_colors());
                        self.palette_grid.set_foreground(fg);
                        self.palette_grid.set_background(bg);
                        // Confirm the swap so the shader resets to normal display
                        self.color_switcher.confirm_swap();
                    }
                    ColorSwitcherMessage::ResetToDefault => {
                        self.with_edit_state(|state| state.reset_caret_colors());
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
                        self.with_edit_state(|state| state.set_caret_foreground(color));
                        self.palette_grid.set_foreground(color);
                    }
                    PaletteGridMessage::SetBackground(color) => {
                        self.with_edit_state(|state| state.set_caret_background(color));
                        self.palette_grid.set_background(color);
                    }
                }
                Task::none()
            }
            AnsiEditorMessage::SelectTool(idx) => {
                // Select tool by slot index
                self.change_tool(tools::click_tool_slot(idx, self.current_tool));
                self.tool_panel.set_tool(self.current_tool);
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
            AnsiEditorMessage::KeyPressed(key, modifiers) => {
                self.handle_key_press(key, modifiers);
                Task::none()
            }
            AnsiEditorMessage::CancelShapeDrag => {
                let _ = self.cancel_shape_drag();
                self.minimap_drag_pointer = None;
                Task::none()
            }
            AnsiEditorMessage::CancelMinimapDrag => {
                self.minimap_drag_pointer = None;
                Task::none()
            }
            AnsiEditorMessage::MinimapAutoscrollTick(_delta) => {
                let Some((pointer_x, pointer_y)) = self.minimap_drag_pointer else {
                    return Task::none();
                };

                // Recompute normalized position from the last known pointer position. This is
                // essential when no further cursor events arrive (drag-out), but the minimap and
                // viewport keep moving.
                let render_cache = &self.canvas.terminal.render_cache;
                if let Some((norm_x, norm_y)) =
                    self.right_panel
                        .minimap
                        .handle_click(iced::Size::new(0.0, 0.0), iced::Point::new(pointer_x, pointer_y), Some(render_cache))
                {
                    self.scroll_canvas_to_normalized(norm_x, norm_y);
                }

                Task::none()
            }
            AnsiEditorMessage::CanvasMouseEvent(event) => {
                self.handle_canvas_mouse_event(event);
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
            AnsiEditorMessage::OpenTdfFontSelector => {
                self.tdf_font_selector_open = true;
                Task::none()
            }
            AnsiEditorMessage::TdfFontSelector(msg) => self.handle_tdf_font_selector_message(msg),
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

    /// Generate a unique tag name based on existing tags (TAG1, TAG2, etc.)
    fn generate_next_tag_name(&mut self) -> String {
        let tag_count = self.with_edit_state(|state| state.get_buffer().tags.len());
        format!("TAG{}", tag_count + 1)
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
                // First collect tag info with selection state
                let tag_info: Vec<_> = edit_state
                    .get_buffer()
                    .tags
                    .iter()
                    .enumerate()
                    .map(|(idx, tag)| (tag.position, tag.len(), self.tag_selection.contains(&idx)))
                    .collect();

                // Then update overlays
                let overlays = edit_state.get_tool_overlay_mask_mut();
                overlays.clear();

                for (pos, len, _) in &tag_info {
                    let rect = icy_engine::Rectangle::new(*pos, (*len as i32, 1).into());
                    overlays.add_rectangle(rect);
                }

                edit_state.mark_dirty();
                tag_info
            } else {
                vec![]
            };

            (fw, fh, tags)
        };

        // Now render overlay to canvas (no longer holding screen lock)
        self.render_tag_overlay_to_canvas(font_width, font_height, &tag_data);
    }

    /// Render tag overlay rectangles to the canvas
    fn render_tag_overlay_to_canvas(&mut self, font_width: f32, font_height: f32, tag_data: &[(icy_engine::Position, usize, bool)]) {
        let overlay_rects: Vec<(f32, f32, f32, f32, bool)> = tag_data
            .iter()
            .map(|(pos, len, is_selected)| {
                let x = pos.x as f32 * font_width;
                let y = pos.y as f32 * font_height;
                let w = *len as f32 * font_width;
                let h = font_height;
                (x, y, w, h, *is_selected)
            })
            .collect();

        // Create overlay mask with all tag rectangles
        if overlay_rects.is_empty() {
            self.canvas.set_tool_overlay_mask(None, None);
            return;
        }

        // Find bounding box of all tags
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for (x, y, w, h, _) in &overlay_rects {
            min_x = min_x.min(*x);
            min_y = min_y.min(*y);
            max_x = max_x.max(x + w);
            max_y = max_y.max(y + h);
        }

        let total_w = (max_x - min_x).ceil() as u32;
        let total_h = (max_y - min_y).ceil() as u32;

        if total_w == 0 || total_h == 0 {
            self.canvas.set_tool_overlay_mask(None, None);
            return;
        }

        // Create RGBA buffer for overlay
        let mut rgba = vec![0u8; (total_w * total_h * 4) as usize];

        for (x, y, w, h, is_selected) in &overlay_rects {
            let local_x = (x - min_x) as u32;
            let local_y = (y - min_y) as u32;
            let rect_w = *w as u32;
            let rect_h = *h as u32;

            // Different colors for selected vs non-selected tags
            let (r, g, b, a) = if *is_selected {
                (255, 200, 50, 255) // Yellow/orange for selected
            } else {
                (100, 150, 255, 200) // Translucent blue for normal
            };

            // Draw border
            for py in local_y..(local_y + rect_h).min(total_h) {
                for px in local_x..(local_x + rect_w).min(total_w) {
                    // Border pixels only
                    let is_border =
                        px == local_x || px == (local_x + rect_w - 1).min(total_w - 1) || py == local_y || py == (local_y + rect_h - 1).min(total_h - 1);

                    if is_border {
                        let idx = ((py * total_w + px) * 4) as usize;
                        if idx + 3 < rgba.len() {
                            rgba[idx] = r;
                            rgba[idx + 1] = g;
                            rgba[idx + 2] = b;
                            rgba[idx + 3] = a;
                        }
                    }
                }
            }
        }

        self.canvas
            .set_tool_overlay_mask(Some((rgba, total_w, total_h)), Some((min_x, min_y, total_w as f32, total_h as f32)));
    }

    /// Update the selection rectangle overlay during tag selection drag
    fn update_tag_selection_drag_overlay(&mut self) {
        // Get font dimensions
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

        // Calculate selection rectangle in pixel coordinates
        let min_x = self.drag_pos.start.x.min(self.drag_pos.cur.x) as f32 * font_width;
        let max_x = (self.drag_pos.start.x.max(self.drag_pos.cur.x) + 1) as f32 * font_width;
        let min_y = self.drag_pos.start.y.min(self.drag_pos.cur.y) as f32 * font_height;
        let max_y = (self.drag_pos.start.y.max(self.drag_pos.cur.y) + 1) as f32 * font_height;

        let w = (max_x - min_x).ceil() as u32;
        let h = (max_y - min_y).ceil() as u32;

        if w == 0 || h == 0 {
            return;
        }

        // Create RGBA buffer for selection rectangle overlay (dashed border, translucent fill)
        let mut rgba = vec![0u8; (w * h * 4) as usize];

        // Draw translucent fill and dashed border
        for py in 0..h {
            for px in 0..w {
                let idx = ((py * w + px) * 4) as usize;
                if idx + 3 >= rgba.len() {
                    continue;
                }

                let is_border = px == 0 || px == w - 1 || py == 0 || py == h - 1;

                if is_border {
                    // Dashed border pattern (every 4 pixels)
                    let is_dash = ((px + py) / 4) % 2 == 0;
                    if is_dash {
                        rgba[idx] = 255; // R - white
                        rgba[idx + 1] = 255; // G
                        rgba[idx + 2] = 255; // B
                        rgba[idx + 3] = 200; // A
                    }
                } else {
                    // Translucent fill
                    rgba[idx] = 100; // R
                    rgba[idx + 1] = 150; // G
                    rgba[idx + 2] = 255; // B - blue tint
                    rgba[idx + 3] = 30; // A - very translucent
                }
            }
        }

        self.canvas.set_tool_overlay_mask(Some((rgba, w, h)), Some((min_x, min_y, w as f32, h as f32)));
    }

    /// Handle TDF font selector messages
    fn handle_tdf_font_selector_message(&mut self, msg: TdfFontSelectorMessage) -> Task<AnsiEditorMessage> {
        match msg {
            TdfFontSelectorMessage::Cancel => {
                self.tdf_font_selector_open = false;
            }
            TdfFontSelectorMessage::Confirm(font_idx) => {
                // Apply selected font and close dialog
                if font_idx >= 0 {
                    self.font_tool.select_font(font_idx);
                }
                self.tdf_font_selector_open = false;
            }
            TdfFontSelectorMessage::SelectFont(idx) => {
                self.tdf_font_selector.select_font(idx);
            }
            TdfFontSelectorMessage::FilterChanged(filter) => {
                self.tdf_font_selector.set_filter(filter);
            }
            TdfFontSelectorMessage::ToggleOutline => {
                self.tdf_font_selector.toggle_outline();
            }
            TdfFontSelectorMessage::ToggleBlock => {
                self.tdf_font_selector.toggle_block();
            }
            TdfFontSelectorMessage::ToggleColor => {
                self.tdf_font_selector.toggle_color();
            }
            TdfFontSelectorMessage::ToggleFiglet => {
                self.tdf_font_selector.toggle_figlet();
            }
        }
        Task::none()
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

    /// Handle key press events
    fn handle_key_press(&mut self, key: iced::keyboard::Key, modifiers: iced::keyboard::Modifiers) {
        // Character selector overlay has priority and is closed with Escape.
        if self.char_selector_target.is_some() {
            if matches!(key, iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)) {
                self.char_selector_target = None;
                return;
            }
        }

        // Paste mode has priority for special keys
        if self.is_paste_mode() {
            log::debug!("handle_key_press: key={:?}, paste_mode=true", key);
            if self.handle_paste_mode_key(&key, &modifiers) {
                return;
            }
        }

        // Handle tool-specific key events
        let event = self.handle_tool_key(&key, &modifiers);
        self.handle_tool_event(event);
    }

    /// Handle keyboard input in paste mode
    /// Returns true if the key was handled
    fn handle_paste_mode_key(&mut self, key: &iced::keyboard::Key, _modifiers: &iced::keyboard::Modifiers) -> bool {
        use iced::keyboard::Key;
        use iced::keyboard::key::Named;

        log::debug!("handle_paste_mode_key: received key {:?}", key);

        match key {
            // Escape - cancel paste
            Key::Named(Named::Escape) => {
                log::debug!("handle_paste_mode_key: Escape pressed, discarding floating layer");
                let _ = self.discard_floating_layer();
                true
            }
            // Enter - anchor layer
            Key::Named(Named::Enter) => {
                log::debug!("handle_paste_mode_key: Enter pressed, anchoring layer");
                let _ = self.anchor_layer();
                true
            }
            // Arrow keys - move floating layer
            Key::Named(Named::ArrowUp) => {
                self.move_floating_layer(0, -1);
                true
            }
            Key::Named(Named::ArrowDown) => {
                self.move_floating_layer(0, 1);
                true
            }
            Key::Named(Named::ArrowLeft) => {
                self.move_floating_layer(-1, 0);
                true
            }
            Key::Named(Named::ArrowRight) => {
                self.move_floating_layer(1, 0);
                true
            }
            // S - stamp
            Key::Character(c) if c.eq_ignore_ascii_case("s") => {
                self.stamp_floating_layer();
                true
            }
            // R - rotate
            Key::Character(c) if c.eq_ignore_ascii_case("r") => {
                self.rotate_floating_layer();
                true
            }
            // X - flip horizontal
            Key::Character(c) if c.eq_ignore_ascii_case("x") => {
                self.flip_floating_layer_x();
                true
            }
            // Y - flip vertical
            Key::Character(c) if c.eq_ignore_ascii_case("y") => {
                self.flip_floating_layer_y();
                true
            }
            // T - toggle transparent
            Key::Character(c) if c.eq_ignore_ascii_case("t") => {
                self.toggle_floating_layer_transparent();
                true
            }
            _ => false,
        }
    }

    /// Move the floating layer by the given delta
    fn move_floating_layer(&mut self, dx: i32, dy: i32) {
        {
            let mut screen = self.screen.lock();
            if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
                // Get current offset of the current layer and add delta
                let current_offset = edit_state.get_cur_layer().map(|l| l.offset()).unwrap_or_default();
                let new_pos = Position::new(current_offset.x + dx, current_offset.y + dy);
                if let Err(e) = edit_state.move_layer(new_pos) {
                    log::warn!("Failed to move layer: {}", e);
                }
            }
        }
        // Update layer bounds to show new position
        self.update_layer_bounds();
    }

    /// Handle named key events for Click and Font tools (shared navigation/editing)
    fn handle_click_font_named_key(&mut self, named: &iced::keyboard::key::Named, modifiers: &iced::keyboard::Modifiers) -> ToolEvent {
        use iced::keyboard::key::Named;
        match named {
            Named::F1
            | Named::F2
            | Named::F3
            | Named::F4
            | Named::F5
            | Named::F6
            | Named::F7
            | Named::F8
            | Named::F9
            | Named::F10
            | Named::F11
            | Named::F12 => {
                let slot = match named {
                    Named::F1 => 0,
                    Named::F2 => 1,
                    Named::F3 => 2,
                    Named::F4 => 3,
                    Named::F5 => 4,
                    Named::F6 => 5,
                    Named::F7 => 6,
                    Named::F8 => 7,
                    Named::F9 => 8,
                    Named::F10 => 9,
                    Named::F11 => 10,
                    Named::F12 => 11,
                    _ => 0,
                };

                // Moebius: Alt+F1..F10 selects set 0..9, Shift+Alt selects 10..19.
                if modifiers.alt() && slot < 10 {
                    let base = if modifiers.shift() { 10 } else { 0 };
                    self.set_current_fkey_set(base + slot);
                    return ToolEvent::Redraw;
                }

                self.type_fkey_slot(slot);
                ToolEvent::Commit("Type fkey".to_string())
            }
            // Cursor movement
            Named::ArrowUp => {
                self.with_edit_state(|state| state.move_caret_up(1));
                ToolEvent::Redraw
            }
            Named::ArrowDown => {
                self.with_edit_state(|state| state.move_caret_down(1));
                ToolEvent::Redraw
            }
            Named::ArrowLeft => {
                self.with_edit_state(|state| state.move_caret_left(1));
                ToolEvent::Redraw
            }
            Named::ArrowRight => {
                self.with_edit_state(|state| state.move_caret_right(1));
                ToolEvent::Redraw
            }
            Named::Home => {
                self.with_edit_state(|state| state.set_caret_x(0));
                ToolEvent::Redraw
            }
            Named::End => {
                let width = self.with_edit_state(|state| state.get_buffer().width());
                self.with_edit_state(|state| state.set_caret_x(width - 1));
                ToolEvent::Redraw
            }
            Named::PageUp => {
                self.with_edit_state(|state| state.move_caret_up(24));
                ToolEvent::Redraw
            }
            Named::PageDown => {
                self.with_edit_state(|state| state.move_caret_down(24));
                ToolEvent::Redraw
            }
            // Text editing
            Named::Backspace => {
                if self.current_tool == Tool::Font {
                    return self.handle_font_tool_backspace();
                }
                let result = self.with_edit_state(|state| {
                    if state.is_something_selected() {
                        state.erase_selection()
                    } else {
                        state.backspace()
                    }
                });
                if let Err(e) = result {
                    log::warn!("Failed to backspace: {}", e);
                }
                self.update_selection_display();
                ToolEvent::Commit("Backspace".to_string())
            }
            Named::Delete => {
                let result = self.with_edit_state(|state| {
                    if state.is_something_selected() {
                        state.erase_selection()
                    } else {
                        state.delete_key()
                    }
                });
                if let Err(e) = result {
                    log::warn!("Failed to delete: {}", e);
                }
                self.update_selection_display();
                ToolEvent::Commit("Delete".to_string())
            }
            Named::Enter => {
                if self.current_tool == Tool::Font {
                    // Font tool: move to next line using font height
                    let font_height = self.font_tool.max_height();
                    self.with_edit_state(|state| {
                        let pos = state.get_caret().position();
                        state.set_caret_position(Position::new(0, pos.y + font_height as i32));
                    });
                    self.font_tool.prev_char = '\0';
                    ToolEvent::Redraw
                } else {
                    let result = self.with_edit_state(|state| state.new_line());
                    if let Err(e) = result {
                        log::warn!("Failed to new line: {}", e);
                    }
                    ToolEvent::Commit("New line".to_string())
                }
            }
            Named::Tab => {
                if modifiers.shift() {
                    self.with_edit_state(|state| state.handle_reverse_tab());
                } else {
                    self.with_edit_state(|state| state.handle_tab());
                }
                ToolEvent::Redraw
            }
            Named::Insert => {
                self.with_edit_state(|state| state.toggle_insert_mode());
                ToolEvent::Redraw
            }
            Named::Space => {
                if self.current_tool == Tool::Font {
                    return self.handle_font_tool_char(' ');
                }
                // Space is a named key in iced, treat as character input
                let result = self.with_edit_state(|state| state.type_key(' '));
                if let Err(e) = result {
                    log::warn!("Failed to type space: {}", e);
                }
                ToolEvent::Commit("Type character".to_string())
            }
            Named::Escape => {
                // Cancel tag add mode if active
                if self.tag_add_new_index.is_some() {
                    self.tag_add_new_index = None;
                    self.tag_selection_drag_active = false;
                    // Drop the undo guard and then undo to remove the tag
                    self.end_tag_drag();
                    let _ = self.with_edit_state(|state| state.undo());
                    return self.redraw_with_tag_overlays();
                }

                // Clear selection
                self.with_edit_state(|state| {
                    let _ = state.clear_selection();
                });
                self.redraw_with_selection_display()
            }
            _ => ToolEvent::None,
        }
    }

    /// Handle character input for Font tool (TDF/Figlet rendering)
    fn handle_font_tool_char(&mut self, ch: char) -> ToolEvent {
        use icy_engine_edit::TdfEditStateRenderer;

        // Check if we have a selected font
        let font_idx = self.font_tool.selected_font;
        if font_idx < 0 || (font_idx as usize) >= self.font_tool.font_count() {
            log::warn!("No font selected for Font tool");
            return ToolEvent::None;
        }

        // Check if character is supported (access font through library)
        let has_char = self.font_tool.with_font_at(font_idx as usize, |font| font.has_char(ch));
        if !has_char.unwrap_or(false) {
            log::debug!("Character '{}' not supported by current font", ch);
            return ToolEvent::None;
        }

        // Get outline style from options
        let outline_style = { *self.options.read().font_outline_style.read() };

        // Render the glyph - access screen and font library
        // Returns (new_x, start_y) - Y stays at original row, X advances
        let result: Result<Position, icy_engine::EngineError> = {
            let mut screen = self.screen.lock();
            let edit_state = screen
                .as_any_mut()
                .downcast_mut::<EditState>()
                .expect("AnsiEditor screen should always be EditState");

            // Begin atomic undo with RenderCharacter operation type for backspace support
            let _undo_guard = edit_state.begin_typed_atomic_undo("Render font character", OperationType::RenderCharacter);

            // Save caret position for undo - this allows backspace to restore position
            let _ = edit_state.undo_caret_position();

            let caret_pos = edit_state.get_caret().position();
            let start_y = caret_pos.y;

            match TdfEditStateRenderer::new(edit_state, caret_pos.x, start_y) {
                Ok(mut renderer) => {
                    let render_options = retrofont::RenderOptions {
                        outline_style,
                        ..Default::default()
                    };

                    // Access font through library and render
                    let lib = self.font_tool.font_library.read();
                    if let Some(font) = lib.get_font(font_idx as usize) {
                        match font.render_glyph(&mut renderer, ch, &render_options) {
                            Ok(_) => {
                                // Return new X position but keep original Y (don't move caret down)
                                Ok(Position::new(renderer.max_x(), start_y))
                            }
                            Err(e) => Err(icy_engine::EngineError::Generic(format!("Font render error: {}", e))),
                        }
                    } else {
                        Err(icy_engine::EngineError::Generic("Font not found".to_string()))
                    }
                }
                Err(e) => Err(e),
            }
        };

        match result {
            Ok(new_pos) => {
                // Update prev_char for kerning
                self.font_tool.prev_char = ch;

                // Move caret to new position
                self.with_edit_state(|state| {
                    state.set_caret_position(new_pos);
                });

                ToolEvent::Commit("Render font character".to_string())
            }
            Err(e) => {
                log::warn!("Failed to render font character: {}", e);
                ToolEvent::None
            }
        }
    }

    /// Handle backspace for Font tool (undo last rendered character)
    fn handle_font_tool_backspace(&mut self) -> ToolEvent {
        // Try to find and reverse the last RenderCharacter operation in the undo stack
        let mut use_backspace = true;

        let _reverse_result: Option<Result<(), icy_engine::EngineError>> = {
            let mut screen = self.screen.lock();
            let edit_state = screen
                .as_any_mut()
                .downcast_mut::<EditState>()
                .expect("AnsiEditor screen should always be EditState");

            // Find the last RenderCharacter operation that hasn't been reversed
            let undo_stack = edit_state.get_undo_stack();
            let Ok(stack) = undo_stack.lock() else {
                return ToolEvent::None;
            };

            let mut reverse_count = 0;
            let mut found_index = None;

            for i in (0..stack.len()).rev() {
                match stack[i].get_operation_type() {
                    OperationType::RenderCharacter => {
                        if reverse_count == 0 {
                            found_index = Some(i);
                            break;
                        }
                        reverse_count -= 1;
                    }
                    OperationType::ReversedRenderCharacter => {
                        reverse_count += 1;
                    }
                    OperationType::Unknown => {
                        // Stop at unknown operations
                        break;
                    }
                }
            }

            if let Some(idx) = found_index {
                if let Some(op) = stack[idx].try_clone() {
                    drop(stack); // Release the lock before push_reverse_undo

                    // Push a reverse undo operation
                    match edit_state.push_reverse_undo("Undo font character", op, OperationType::ReversedRenderCharacter) {
                        Ok(_) => {
                            use_backspace = false;
                            Some(Ok(()))
                        }
                        Err(e) => Some(Err(e)),
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        // Reset prev_char since we're going backwards
        self.font_tool.prev_char = '\0';

        if use_backspace {
            // Fall back to normal backspace if no RenderCharacter found
            let result = self.with_edit_state(|state| {
                if state.is_something_selected() {
                    state.erase_selection()
                } else {
                    state.backspace()
                }
            });
            if let Err(e) = result {
                log::warn!("Failed to backspace: {}", e);
            }
        }

        self.update_selection_display();
        ToolEvent::Commit("Font backspace".to_string())
    }

    /// Handle tool-specific key events based on current tool
    fn handle_tool_key(&mut self, key: &iced::keyboard::Key, modifiers: &iced::keyboard::Modifiers) -> ToolEvent {
        use iced::keyboard::key::Named;

        // Escape cancels active drags (shape/layer/tag-selection) and clears previews.
        if let iced::keyboard::Key::Named(Named::Escape) = key {
            if self.cancel_shape_drag() {
                return ToolEvent::Redraw;
            }

            if self.layer_drag_active {
                self.cancel_layer_drag();
                return ToolEvent::Redraw;
            }

            if self.tag_selection_drag_active {
                self.cancel_tag_selection_drag();
                return ToolEvent::Redraw;
            }
        }
        match self.current_tool {
            Tool::Click => {
                // Handle typing and cursor movement for Click tool (normal text input)
                match key {
                    iced::keyboard::Key::Character(c) => {
                        if !modifiers.control() && !modifiers.alt() {
                            if let Some(ch) = c.chars().next() {
                                // Convert Unicode to CP437 for ANSI art
                                let cp437_char = self.with_edit_state(|state| state.get_buffer().buffer_type.convert_from_unicode(ch));
                                // Type character at cursor using terminal_input
                                let result: Result<(), icy_engine::EngineError> = self.with_edit_state(|state| state.type_key(cp437_char));
                                if let Err(e) = result {
                                    log::warn!("Failed to type character: {}", e);
                                }
                                return ToolEvent::Commit("Type character".to_string());
                            }
                        }
                    }
                    iced::keyboard::Key::Named(named) => {
                        return self.handle_click_font_named_key(named, modifiers);
                    }
                    _ => {}
                }
            }
            Tool::Font => {
                // Handle TDF/Figlet font rendering
                match key {
                    iced::keyboard::Key::Character(c) => {
                        if !modifiers.control() && !modifiers.alt() {
                            if let Some(ch) = c.chars().next() {
                                return self.handle_font_tool_char(ch);
                            }
                        }
                    }
                    iced::keyboard::Key::Named(named) => {
                        return self.handle_click_font_named_key(named, modifiers);
                    }
                    _ => {}
                }
            }
            Tool::Select => {
                if let iced::keyboard::Key::Named(named) = key {
                    match named {
                        Named::Delete | Named::Backspace => {
                            let result: Result<(), icy_engine::EngineError> = self.with_edit_state(|state| {
                                if state.is_something_selected() {
                                    state.erase_selection()
                                } else {
                                    // No selection: do nothing in Select tool.
                                    Ok(())
                                }
                            });
                            if let Err(e) = result {
                                log::warn!("Failed to delete selection: {}", e);
                            }
                            self.update_selection_display();
                            return ToolEvent::Commit("Delete".to_string());
                        }
                        Named::Escape => {
                            self.with_edit_state(|state| {
                                let _ = state.clear_selection();
                            });
                            return self.redraw_with_selection_display();
                        }
                        _ => {}
                    }
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
        // Position is already in text/buffer coordinates from TerminalMouseEvent
        match event {
            CanvasMouseEvent::Press {
                position,
                pixel_position,
                button,
                modifiers,
            } => {
                let start_tool = self.current_tool;
                let tool_event = self.handle_tool_mouse_down(position, pixel_position, button, modifiers);
                self.handle_tool_event(tool_event);

                // Capture tool for the duration of a drag so move/up keep going to the same tool.
                // Paste mode already takes exclusive routing inside the handlers.
                if !self.is_paste_mode() && self.is_dragging {
                    self.mouse_capture_tool = Some(start_tool);
                }
            }
            CanvasMouseEvent::Release {
                position,
                pixel_position,
                button,
            } => {
                let prev_tool = self.current_tool;
                if !self.is_paste_mode() {
                    if let Some(captured) = self.mouse_capture_tool {
                        self.current_tool = captured;
                    }
                }

                let tool_event = self.handle_tool_mouse_up(position, pixel_position, button);
                self.handle_tool_event(tool_event);

                self.current_tool = prev_tool;
                if !self.is_dragging {
                    self.mouse_capture_tool = None;
                }
            }
            CanvasMouseEvent::Move { position, pixel_position } => {
                let prev_tool = self.current_tool;
                if !self.is_paste_mode() {
                    if let Some(captured) = self.mouse_capture_tool {
                        self.current_tool = captured;
                    }
                }

                let tool_event = self.handle_tool_mouse_move(position, pixel_position);
                self.handle_tool_event(tool_event);

                self.current_tool = prev_tool;
                if !self.is_dragging {
                    self.mouse_capture_tool = None;
                }
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
    fn handle_tool_mouse_down(
        &mut self,
        pos: icy_engine::Position,
        pixel_position: (f32, f32),
        button: iced::mouse::Button,
        modifiers: icy_engine::KeyModifiers,
    ) -> ToolEvent {
        // === Paste Mode: exclusive handling ===
        // When in paste mode, ALL mouse actions are for moving the floating layer
        if self.is_paste_mode() {
            log::debug!("handle_tool_mouse_down: pos={:?}, button={:?}, paste_mode=true", pos, button);
            if button == iced::mouse::Button::Left {
                // Start dragging the current layer (which is the paste layer)
                let start_offset = self.with_edit_state(|state| state.get_cur_layer().map(|l| l.offset()).unwrap_or_default());
                self.paste_drag_active = true;
                self.paste_drag_start_offset = start_offset;
                self.paste_drag_undo = Some(self.with_edit_state(|state| state.begin_atomic_undo("Move pasted layer".to_string())));
                self.drag_pos.start = pos;
                self.drag_pos.cur = pos;
                self.is_dragging = true;
                log::debug!("Paste drag started at {:?}, layer offset {:?}", pos, start_offset);
            }
            return ToolEvent::None;
        }

        match self.current_tool {
            Tool::Tag => {
                // Close context menu on any click
                self.close_tag_context_menu();

                if button == iced::mouse::Button::Left {
                    // If in add mode (dragging new tag), do nothing on click - already dragging
                    if self.tag_add_new_index.is_some() {
                        return ToolEvent::None;
                    }

                    // Check if clicking on an existing tag
                    let tag_at_pos = self.with_edit_state(|state| {
                        state
                            .get_buffer()
                            .tags
                            .iter()
                            .enumerate()
                            .find(|(_, t)| t.contains(pos))
                            .map(|(i, t)| (i, t.position))
                    });

                    if let Some((index, tag_pos)) = tag_at_pos {
                        // Handle Ctrl+Click for multi-selection toggle (meta = Cmd on macOS)
                        if modifiers.ctrl || modifiers.meta {
                            if self.tag_selection.contains(&index) {
                                self.tag_selection.retain(|&i| i != index);
                            } else {
                                self.tag_selection.push(index);
                            }
                            return self.redraw_with_tag_overlays();
                        }

                        // Check if this tag is part of multi-selection
                        if self.tag_selection.contains(&index) {
                            // Drag all selected tags - clone selection to avoid borrow conflict
                            let selection = self.tag_selection.clone();
                            let selected_positions: Vec<(usize, icy_engine::Position)> = self.with_edit_state(|state| {
                                selection
                                    .iter()
                                    .filter_map(|&i| state.get_buffer().tags.get(i).map(|t| (i, t.position)))
                                    .collect()
                            });
                            self.tag_drag_indices = selected_positions.iter().map(|(i, _)| *i).collect();
                            self.tag_drag_start_positions = selected_positions.iter().map(|(_, p)| *p).collect();
                        } else {
                            // Clear selection and drag single tag
                            self.tag_selection.clear();
                            self.tag_selection.push(index);
                            self.tag_drag_indices = vec![index];
                            self.tag_drag_start_positions = vec![tag_pos];
                        }

                        self.tag_drag_active = true;
                        self.is_dragging = true;
                        self.drag_pos.start = pos;
                        self.drag_pos.cur = pos;
                        self.drag_pos.start_abs = pos;
                        self.drag_pos.cur_abs = pos;
                        // Start atomic undo for drag operation
                        let desc = if self.tag_selection.len() > 1 {
                            format!("Move {} tags", self.tag_selection.len())
                        } else {
                            "Move tag".to_string()
                        };
                        self.tag_drag_undo = Some(self.with_edit_state(|state| state.begin_atomic_undo(desc)));
                        return self.redraw_with_tag_overlays();
                    }

                    // No tag at position - start a selection drag to select multiple tags
                    self.tag_selection.clear();
                    self.tag_list_dialog = None;
                    self.update_tag_overlays();

                    // Start selection rectangle drag
                    self.tag_selection_drag_active = true;
                    self.is_dragging = true;
                    self.drag_pos.start = pos;
                    self.drag_pos.cur = pos;
                    self.drag_pos.start_abs = pos;
                    self.drag_pos.cur_abs = pos;

                    ToolEvent::Redraw
                } else if button == iced::mouse::Button::Right {
                    // Right-click: open context menu if clicking on a tag
                    let tag_at_pos = self.with_edit_state(|state| state.get_buffer().tags.iter().enumerate().find(|(_, t)| t.contains(pos)).map(|(i, _)| i));

                    if let Some(index) = tag_at_pos {
                        // If the tag is not in selection, select only this tag
                        if !self.tag_selection.contains(&index) {
                            self.tag_selection.clear();
                            self.tag_selection.push(index);
                        }
                        self.tag_context_menu = Some((index, pos));
                        return ToolEvent::Redraw;
                    }
                    ToolEvent::None
                } else {
                    ToolEvent::None
                }
            }
            Tool::Click | Tool::Font => {
                // Ctrl+Click = Start layer drag
                if button == iced::mouse::Button::Left && (is_ctrl_pressed() || is_command_pressed()) {
                    // Start layer drag
                    let layer_offset = self.with_edit_state(|state| state.get_cur_layer().map(|l| l.offset()).unwrap_or_default());
                    self.layer_drag_active = true;
                    self.layer_drag_start_offset = layer_offset;
                    self.layer_drag_undo = Some(self.with_edit_state(|state| state.begin_atomic_undo("Move layer".to_string())));
                    self.is_dragging = true;
                    self.drag_pos.start = pos;
                    self.drag_pos.cur = pos;
                    self.drag_pos.start_abs = pos;
                    self.drag_pos.cur_abs = pos;
                    return ToolEvent::Redraw;
                }

                // Check if clicking inside existing selection for drag/resize
                let selection_drag = self.get_selection_drag_at(pos);

                if selection_drag != SelectionDrag::None {
                    // Start dragging existing selection
                    self.selection_drag = selection_drag;
                    self.is_dragging = true;
                    self.drag_pos.start = pos;
                    self.drag_pos.cur = pos;
                    self.drag_pos.start_abs = pos;
                    self.drag_pos.cur_abs = pos;

                    // Save selection state for resize operations
                    self.start_selection = self.with_edit_state(|state| state.selection().map(|s| s.as_rectangle()));
                } else {
                    // Clear selection and move cursor, or start new selection
                    self.with_edit_state(|state| {
                        let _ = state.clear_selection();
                        state.set_caret_position(pos);
                    });
                    self.update_selection_display();

                    // Start new selection drag
                    self.selection_drag = SelectionDrag::Create;
                    self.is_dragging = true;
                    self.drag_pos.start = pos;
                    self.drag_pos.cur = pos;
                    self.drag_pos.start_abs = pos;
                    self.drag_pos.cur_abs = pos;
                    self.start_selection = None;
                }
                ToolEvent::Redraw
            }
            Tool::Select => {
                use widget::toolbar::top::{SelectionMode, SelectionModifier};
                let selection_mode = self.top_toolbar.select_options.selection_mode;

                // Determine modifier from *global* keyboard state (event modifiers can be stale).
                let selection_modifier = if is_shift_pressed() {
                    SelectionModifier::Add
                } else if is_ctrl_pressed() || is_command_pressed() {
                    SelectionModifier::Remove
                } else {
                    SelectionModifier::Replace
                };

                match selection_mode {
                    SelectionMode::Normal => {
                        // In Add/Remove mode we always start a *new* rectangle selection
                        // (new anchor at mouse-down), even if the click is inside the
                        // existing selection. Otherwise we would move/resize and reuse
                        // the old anchor, which looks like the selection is being expanded.
                        if selection_modifier != SelectionModifier::Replace {
                            #[cfg(debug_assertions)]
                            eprintln!("[DEBUG] Mouse down - Add/Remove mode: force new selection (commit old to mask)");
                            self.with_edit_state(|state| {
                                let _ = state.add_selection_to_mask();
                                let _ = state.deselect();
                            });

                            self.selection_drag = SelectionDrag::Create;
                            self.is_dragging = true;
                            self.drag_pos.start = pos;
                            self.drag_pos.cur = pos;
                            self.drag_pos.start_abs = pos;
                            self.drag_pos.cur_abs = pos;
                            self.start_selection = None;
                        } else {
                            // Replace: starting a new selection interaction should always start from a clean mask.
                            // We intentionally keep the active selection so move/resize still works.
                            self.with_edit_state(|state| {
                                let _ = state.clear_selection_mask();
                            });

                            let selection_drag = self.get_selection_drag_at(pos);

                            if selection_drag != SelectionDrag::None {
                                // Start dragging existing selection
                                self.selection_drag = selection_drag;
                                self.is_dragging = true;
                                self.drag_pos.start = pos;
                                self.drag_pos.cur = pos;
                                self.drag_pos.start_abs = pos;
                                self.drag_pos.cur_abs = pos;
                                self.start_selection = self.with_edit_state(|state| state.selection().map(|s| s.as_rectangle()));
                            } else {
                                // Starting a new selection (replace).
                                #[cfg(debug_assertions)]
                                eprintln!("[DEBUG] Mouse down - Replace mode: clearing selection and mask");
                                self.with_edit_state(|state| {
                                    let _ = state.clear_selection();
                                });

                                self.selection_drag = SelectionDrag::Create;
                                self.is_dragging = true;
                                self.drag_pos.start = pos;
                                self.drag_pos.cur = pos;
                                self.drag_pos.start_abs = pos;
                                self.drag_pos.cur_abs = pos;
                                self.start_selection = None;
                            }
                        }

                        self.update_selection_display();
                    }
                    SelectionMode::Character => {
                        // Get character at clicked position
                        let cur_ch = self.get_char_at(pos);
                        self.with_edit_state(|state| {
                            state.enumerate_selections(|_, ch, _| selection_modifier.get_response(ch.ch == cur_ch.ch));
                        });
                        self.update_selection_display();
                    }
                    SelectionMode::Attribute => {
                        let cur_ch = self.get_char_at(pos);
                        self.with_edit_state(|state| {
                            state.enumerate_selections(|_, ch, _| selection_modifier.get_response(ch.attribute == cur_ch.attribute));
                        });
                        self.update_selection_display();
                    }
                    SelectionMode::Foreground => {
                        let cur_ch = self.get_char_at(pos);
                        self.with_edit_state(|state| {
                            state.enumerate_selections(|_, ch, _| selection_modifier.get_response(ch.attribute.foreground() == cur_ch.attribute.foreground()));
                        });
                        self.update_selection_display();
                    }
                    SelectionMode::Background => {
                        let cur_ch = self.get_char_at(pos);
                        self.with_edit_state(|state| {
                            state.enumerate_selections(|_, ch, _| selection_modifier.get_response(ch.attribute.background() == cur_ch.attribute.background()));
                        });
                        self.update_selection_display();
                    }
                }
                ToolEvent::Redraw
            }
            Tool::Pencil => {
                // Start paint stroke (layer-local painting; selection stays doc-based)
                self.selection_drag = SelectionDrag::None;
                self.is_dragging = true;
                self.drag_pos.start = pos;
                self.drag_pos.cur = pos;

                // Compute and store half-block coordinates for interpolation
                let half_block_pos = self.compute_half_block_pos(pixel_position);
                self.half_block_click_pos = half_block_pos;
                self.drag_pos.start_half_block = half_block_pos;
                self.drag_pos.cur_half_block = half_block_pos;

                if self.paint_undo.is_none() {
                    let desc = "Pencil";
                    self.paint_undo = Some(self.with_edit_state(|state| state.begin_atomic_undo(desc)));
                }

                self.paint_last_pos = Some(pos);
                self.paint_button = button;

                // Check if we're in half-block mode
                let is_half_block_mode = matches!(self.top_toolbar.brush_options.primary, BrushPrimaryMode::HalfBlock);

                if is_half_block_mode {
                    // Apply brush_size in half-block coordinates
                    self.apply_half_block_with_brush_size(half_block_pos, button);
                } else {
                    // Normal mode: apply at cell position
                    self.apply_paint_stamp(pos, pixel_position, button);
                }
                ToolEvent::Redraw
            }
            Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled => {
                // Start shape drag (preview is rendered as translucent overlay mask like Moebius)
                self.selection_drag = SelectionDrag::None;
                self.is_dragging = true;
                self.drag_pos.start = pos;
                self.drag_pos.cur = pos;
                self.drag_pos.start_abs = pos;
                self.drag_pos.cur_abs = pos;

                self.paint_button = button;
                self.shape_clear = is_shift_pressed();

                // Track half-block drag positions as well (used when HalfBlock primary mode is active)
                let half_block_pos = self.compute_half_block_pos(pixel_position);
                self.drag_pos.start_half_block = half_block_pos;
                self.drag_pos.cur_half_block = half_block_pos;

                self.update_shape_tool_overlay_preview();
                ToolEvent::Redraw
            }
            Tool::Pipette => {
                // Pipette: Pick character/color at position (Moebius-style)
                // Update modifier state based on current keys
                self.pipette.update_modifiers();

                // Get character at position
                let ch = self.with_edit_state(|state| {
                    use icy_engine::TextPane;
                    state.char_at(pos)
                });

                // Apply to caret attribute based on modifiers
                let (take_fg, take_bg) = (self.pipette.take_fg, self.pipette.take_bg);
                self.with_edit_state(|state| {
                    if take_fg {
                        state.set_caret_foreground(ch.attribute.foreground());
                    }
                    if take_bg {
                        state.set_caret_background(ch.attribute.background());
                    }
                });

                // Update palette grid to reflect new colors
                let (fg, bg) = self.with_edit_state(|state| {
                    let attr = state.get_caret().attribute;
                    (attr.foreground(), attr.background())
                });
                self.palette_grid.set_foreground(fg);
                self.palette_grid.set_background(bg);

                // TODO: Go back to previous tool (like Moebius)
                ToolEvent::Commit(format!("Picked color at ({}, {})", pos.x, pos.y))
            }
            Tool::Fill => {
                use std::collections::HashSet;

                // Store half-block click position for HalfBlock fill mode.
                let half_block_pos = self.compute_half_block_pos(pixel_position);
                self.half_block_click_pos = half_block_pos;

                let (primary, paint_char, colorize_fg, colorize_bg, exact) = {
                    let opts = &self.top_toolbar.brush_options;
                    (
                        opts.primary,
                        opts.paint_char,
                        opts.colorize_fg,
                        opts.colorize_bg,
                        self.top_toolbar.fill_exact_matching,
                    )
                };

                // Fill only supports HalfBlock / Char / Colorize (matches src_egui Fill UI).
                let primary = match primary {
                    BrushPrimaryMode::HalfBlock | BrushPrimaryMode::Char | BrushPrimaryMode::Colorize => primary,
                    _ => BrushPrimaryMode::Char,
                };

                // If Colorize mode is selected but no channels are enabled, do nothing.
                if matches!(primary, BrushPrimaryMode::Colorize) && !colorize_fg && !colorize_bg {
                    return ToolEvent::None;
                }

                let swap_colors = button == iced::mouse::Button::Right;
                // Shift key also swaps colors
                let shift_swap = is_shift_pressed();

                // Begin atomic undo for the entire fill.
                let _undo = self.with_edit_state(|state| state.begin_atomic_undo("Bucket fill"));

                match primary {
                    BrushPrimaryMode::HalfBlock => {
                        let start_hb = half_block_pos;
                        self.with_edit_state(|state| {
                            let (offset, width, height) = if let Some(layer) = state.get_cur_layer() {
                                (layer.offset(), layer.width(), layer.height())
                            } else {
                                return;
                            };
                            let use_selection = state.is_something_selected();

                            let caret_attr = state.get_caret().attribute;
                            let (fg, bg) = if swap_colors || shift_swap {
                                (caret_attr.background(), caret_attr.foreground())
                            } else {
                                (caret_attr.foreground(), caret_attr.background())
                            };

                            // Determine the target color at the start position.
                            let start_cell = icy_engine::Position::new(start_hb.x, start_hb.y / 2);
                            if start_cell.x < 0 || start_hb.y < 0 || start_cell.x >= width || start_cell.y >= height {
                                return;
                            }

                            let start_char = { state.get_cur_layer().unwrap().char_at(start_cell) };
                            let start_block = icy_engine::paint::HalfBlock::from_char(start_char, start_hb);
                            if !start_block.is_blocky() {
                                return;
                            }
                            let target_color = if start_block.is_top {
                                start_block.upper_block_color
                            } else {
                                start_block.lower_block_color
                            };
                            if target_color == fg {
                                return;
                            }

                            let mut visited: HashSet<icy_engine::Position> = HashSet::new();
                            let mut stack: Vec<(icy_engine::Position, icy_engine::Position)> = vec![(start_hb, start_hb)];

                            while let Some((from, to)) = stack.pop() {
                                let text_pos = icy_engine::Position::new(to.x, to.y / 2);
                                if to.x < 0 || to.y < 0 || to.x >= width || text_pos.y >= height || !visited.insert(to) {
                                    continue;
                                }

                                if use_selection {
                                    let doc_cell = text_pos + offset;
                                    if !state.is_selected(doc_cell) {
                                        continue;
                                    }
                                }

                                let cur = { state.get_cur_layer().unwrap().char_at(text_pos) };
                                let block = icy_engine::paint::HalfBlock::from_char(cur, to);

                                if block.is_blocky()
                                    && ((block.is_top && block.upper_block_color == target_color) || (!block.is_top && block.lower_block_color == target_color))
                                {
                                    let ch = block.get_half_block_char(fg, true);
                                    let _ = state.set_char_in_atomic(text_pos, ch);

                                    stack.push((to, to + icy_engine::Position::new(-1, 0)));
                                    stack.push((to, to + icy_engine::Position::new(1, 0)));
                                    stack.push((to, to + icy_engine::Position::new(0, -1)));
                                    stack.push((to, to + icy_engine::Position::new(0, 1)));
                                } else if block.is_vertically_blocky() {
                                    let ch = if from.y == to.y - 1 && block.left_block_color == target_color {
                                        Some(icy_engine::AttributedChar::new(
                                            221 as char,
                                            icy_engine::TextAttribute::new(fg, block.right_block_color),
                                        ))
                                    } else if from.y == to.y - 1 && block.right_block_color == target_color {
                                        Some(icy_engine::AttributedChar::new(
                                            222 as char,
                                            icy_engine::TextAttribute::new(fg, block.left_block_color),
                                        ))
                                    } else if from.y == to.y + 1 && block.right_block_color == target_color {
                                        Some(icy_engine::AttributedChar::new(
                                            222 as char,
                                            icy_engine::TextAttribute::new(fg, block.left_block_color),
                                        ))
                                    } else if from.y == to.y + 1 && block.left_block_color == target_color {
                                        Some(icy_engine::AttributedChar::new(
                                            221 as char,
                                            icy_engine::TextAttribute::new(fg, block.right_block_color),
                                        ))
                                    } else if from.x == to.x - 1 && block.left_block_color == target_color {
                                        Some(icy_engine::AttributedChar::new(
                                            221 as char,
                                            icy_engine::TextAttribute::new(fg, block.right_block_color),
                                        ))
                                    } else if from.x == to.x + 1 && block.right_block_color == target_color {
                                        Some(icy_engine::AttributedChar::new(
                                            222 as char,
                                            icy_engine::TextAttribute::new(fg, block.left_block_color),
                                        ))
                                    } else {
                                        None
                                    };

                                    if let Some(ch) = ch {
                                        let _ = state.set_char_in_atomic(text_pos, ch);
                                    }
                                }
                            }

                            let _ = bg; // keep symmetry with other tools; currently unused for half-block fill
                        });
                    }
                    BrushPrimaryMode::Char | BrushPrimaryMode::Colorize => {
                        let start_cell_layer = self.doc_to_layer_pos(pos);

                        self.with_edit_state(|state| {
                            let (offset, width, height) = if let Some(layer) = state.get_cur_layer() {
                                (layer.offset(), layer.width(), layer.height())
                            } else {
                                return;
                            };
                            let use_selection = state.is_something_selected();

                            if start_cell_layer.x < 0 || start_cell_layer.y < 0 || start_cell_layer.x >= width || start_cell_layer.y >= height {
                                return;
                            }

                            let base_char = { state.get_cur_layer().unwrap().char_at(start_cell_layer) };

                            let caret_attr = state.get_caret().attribute;
                            let (fg, bg) = if swap_colors || shift_swap {
                                (caret_attr.background(), caret_attr.foreground())
                            } else {
                                (caret_attr.foreground(), caret_attr.background())
                            };
                            let caret_font_page = caret_attr.font_page();

                            let mut visited: HashSet<icy_engine::Position> = HashSet::new();
                            let mut stack: Vec<icy_engine::Position> = vec![start_cell_layer];

                            while let Some(p) = stack.pop() {
                                if p.x < 0 || p.y < 0 || p.x >= width || p.y >= height || !visited.insert(p) {
                                    continue;
                                }

                                if use_selection {
                                    let doc_cell = p + offset;
                                    if !state.is_selected(doc_cell) {
                                        continue;
                                    }
                                }

                                let cur = { state.get_cur_layer().unwrap().char_at(p) };

                                // Determine if this cell matches (like src_egui FillOperation).
                                match primary {
                                    BrushPrimaryMode::Char => {
                                        if (exact && cur != base_char) || (!exact && cur.ch != base_char.ch) {
                                            continue;
                                        }
                                    }
                                    BrushPrimaryMode::Colorize => {
                                        if (exact && cur != base_char) || (!exact && cur.attribute != base_char.attribute) {
                                            continue;
                                        }
                                    }
                                    _ => {}
                                }

                                let mut repl = cur;

                                if matches!(primary, BrushPrimaryMode::Char) {
                                    repl.ch = paint_char;
                                }

                                if colorize_fg {
                                    repl.attribute.set_foreground(fg);
                                    repl.attribute.set_is_bold(caret_attr.is_bold());
                                }
                                if colorize_bg {
                                    repl.attribute.set_background(bg);
                                }

                                repl.set_font_page(caret_font_page);
                                repl.attribute.attr &= !icy_engine::attribute::INVISIBLE;

                                let _ = state.set_char_in_atomic(p, repl);

                                stack.push(p + icy_engine::Position::new(-1, 0));
                                stack.push(p + icy_engine::Position::new(1, 0));
                                stack.push(p + icy_engine::Position::new(0, -1));
                                stack.push(p + icy_engine::Position::new(0, 1));
                            }
                        });
                    }
                    _ => {}
                }

                ToolEvent::Commit("Bucket fill".to_string())
            }
        }
    }

    /// Handle mouse up based on current tool
    fn handle_tool_mouse_up(&mut self, _pos: icy_engine::Position, _pixel_position: (f32, f32), _button: iced::mouse::Button) -> ToolEvent {
        // === Paste Mode: finalize drag ===
        if self.paste_drag_active {
            self.paste_drag_active = false;
            self.end_drag_capture();

            // Calculate final offset and commit the move
            let delta = self.drag_pos.cur - self.drag_pos.start;
            let new_offset = Position::new(self.paste_drag_start_offset.x + delta.x, self.paste_drag_start_offset.y + delta.y);

            log::debug!("Paste drag ended, moving to {:?}", new_offset);

            {
                let mut screen = self.screen.lock();
                if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
                    // Clear preview offset and commit the actual move
                    edit_state.set_layer_preview_offset(None);
                    let _ = edit_state.move_layer(new_offset);
                }
            }

            // Finalize atomic undo
            self.paste_drag_undo = None;

            // Update layer bounds
            self.update_layer_bounds();

            *self.canvas.terminal.cursor_icon.write() = Some(iced::mouse::Interaction::Grab);
            return ToolEvent::Commit("Move pasted layer".to_string());
        }

        // Handle new tag placement completion
        if let Some(new_tag_index) = self.tag_add_new_index.take() {
            self.end_tag_drag();

            // Open edit dialog for the newly placed tag
            let tag = self.with_edit_state(|state| state.get_buffer().tags.get(new_tag_index).cloned());
            if let Some(tag) = tag {
                self.tag_dialog = Some(TagDialog::edit(&tag, new_tag_index));
            }
            return self.redraw_with_tag_overlays();
        }

        // Handle regular tag drag completion
        if self.tag_drag_active {
            self.end_tag_drag();
            return ToolEvent::None;
        }

        // Handle tag selection drag completion - select all tags in the rectangle
        if self.tag_selection_drag_active {
            return self.finish_tag_selection_drag();
        }

        // Handle layer drag completion
        if self.layer_drag_active {
            return self.finish_layer_drag();
        }

        if self.is_dragging && self.selection_drag != SelectionDrag::None {
            return self.finish_selection_drag();
        }

        if self.is_dragging && matches!(self.current_tool, Tool::Pencil) {
            self.end_drag_capture();
            self.paint_last_pos = None;
            // Dropping the guard groups everything into one undo entry.
            self.paint_undo = None;
            self.paint_button = iced::mouse::Button::Left;

            let desc = "Pencil stroke";
            return ToolEvent::Commit(desc.to_string());
        }

        if self.is_dragging
            && matches!(
                self.current_tool,
                Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled
            )
        {
            self.end_drag_capture();

            let desc = format!("{} drawn", self.current_tool.name());
            self.paint_undo = Some(self.with_edit_state(|state| state.begin_atomic_undo(&desc)));

            let is_half_block_mode = matches!(self.top_toolbar.brush_options.primary, BrushPrimaryMode::HalfBlock);

            if is_half_block_mode {
                // Convert layer-local half-block drag to document half-block coordinates.
                let (start_hb_doc, cur_hb_doc) = self.with_edit_state_readonly(|state| {
                    let offset = state.get_cur_layer().map(|l| l.offset()).unwrap_or_default();
                    let hb_off = icy_engine::Position::new(offset.x, offset.y * 2);
                    (self.drag_pos.start_half_block + hb_off, self.drag_pos.cur_half_block + hb_off)
                });

                let points = Self::shape_points(self.current_tool, start_hb_doc, cur_hb_doc);
                if self.shape_clear {
                    self.with_edit_state(|state| {
                        let offset = state.get_cur_layer().map(|l| l.offset()).unwrap_or_default();
                        let layer_w = state.get_cur_layer().map(|l| l.width()).unwrap_or(0);
                        let layer_h = state.get_cur_layer().map(|l| l.height()).unwrap_or(0);
                        for p in &points {
                            let cell_doc = icy_engine::Position::new(p.x, p.y / 2);
                            let layer_pos = cell_doc - offset;
                            if layer_pos.x < 0 || layer_pos.y < 0 || layer_pos.x >= layer_w || layer_pos.y >= layer_h {
                                continue;
                            }
                            let _ = state.set_char_in_atomic(layer_pos, icy_engine::AttributedChar::invisible());
                        }
                    });
                } else {
                    for p in points {
                        if p.y < 0 {
                            continue;
                        }
                        let cell_doc = icy_engine::Position::new(p.x, p.y / 2);
                        let is_top = (p.y % 2) == 0;
                        self.apply_paint_stamp_with_half_block_info(cell_doc, is_top, self.paint_button);
                    }
                }
            } else {
                let points = Self::shape_points(self.current_tool, self.drag_pos.start, self.drag_pos.cur);
                if self.shape_clear {
                    self.with_edit_state(|state| {
                        let offset = state.get_cur_layer().map(|l| l.offset()).unwrap_or_default();
                        let layer_w = state.get_cur_layer().map(|l| l.width()).unwrap_or(0);
                        let layer_h = state.get_cur_layer().map(|l| l.height()).unwrap_or(0);
                        for p in &points {
                            let layer_pos = *p - offset;
                            if layer_pos.x < 0 || layer_pos.y < 0 || layer_pos.x >= layer_w || layer_pos.y >= layer_h {
                                continue;
                            }
                            let _ = state.set_char_in_atomic(layer_pos, icy_engine::AttributedChar::invisible());
                        }
                    });
                } else {
                    for p in points {
                        self.apply_paint_stamp_with_half_block_info(p, true, self.paint_button);
                    }
                }
            }

            self.paint_undo = None;
            self.paint_button = iced::mouse::Button::Left;
            self.shape_clear = false;
            self.clear_tool_overlay();
            return ToolEvent::Commit(desc);
        }

        match self.current_tool {
            Tool::Pencil | Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled => {
                // TODO: Commit the drawn shape
                ToolEvent::Commit(format!("{} drawn", self.current_tool.name()))
            }
            _ => ToolEvent::None,
        }
    }

    /// Handle mouse move based on current tool
    fn handle_tool_mouse_move(&mut self, pos: icy_engine::Position, pixel_position: (f32, f32)) -> ToolEvent {
        // === Paste Mode: exclusive handling ===
        if self.is_paste_mode() {
            if self.paste_drag_active {
                self.drag_pos.cur = pos;
                let delta = self.drag_pos.cur - self.drag_pos.start;
                let new_offset = Position::new(self.paste_drag_start_offset.x + delta.x, self.paste_drag_start_offset.y + delta.y);

                // Update preview offset on the current layer for visual feedback
                self.with_edit_state(|state| {
                    state.set_layer_preview_offset(Some(new_offset));
                });

                // Update layer bounds to show new preview position
                self.update_layer_bounds();

                *self.canvas.terminal.cursor_icon.write() = Some(iced::mouse::Interaction::Grabbing);
            } else {
                // Not dragging - show grab cursor to indicate layer can be moved
                *self.canvas.terminal.cursor_icon.write() = Some(iced::mouse::Interaction::Grab);
            }
            return ToolEvent::Redraw;
        }

        // Update brush/pencil hover preview (shader rectangle)
        self.update_brush_preview(pos, pixel_position);

        // Pipette tool: always update hover state (even when not dragging)
        if self.current_tool == Tool::Pipette {
            self.pipette.cur_pos = Some(pos);
            self.pipette.update_modifiers();

            // Get character at position
            let ch = self.with_edit_state(|state| {
                use icy_engine::TextPane;
                state.char_at(pos)
            });
            self.pipette.cur_char = Some(ch);

            // No special overlay needed - the toolbar shows the picked colors
            return ToolEvent::Redraw;
        }

        // Tag tool: always show tag overlays (even when not dragging)
        if self.current_tool == Tool::Tag {
            // In add mode - update the new tag's position as we drag
            if self.tag_add_new_index.is_some() && self.tag_drag_active {
                // Move the new tag to current mouse position
                self.drag_pos.cur = pos;
                self.drag_pos.cur_abs = pos;

                let delta = self.drag_pos.cur_abs - self.drag_pos.start_abs;
                let moves: Vec<(usize, icy_engine::Position)> = self
                    .tag_drag_indices
                    .iter()
                    .zip(self.tag_drag_start_positions.iter())
                    .map(|(&tag_idx, &start_pos)| (tag_idx, start_pos + delta))
                    .collect();

                self.with_edit_state(|state| {
                    for (tag_idx, new_pos) in moves {
                        let _ = state.move_tag(tag_idx, new_pos);
                    }
                    state.mark_dirty();
                });
                *self.canvas.terminal.cursor_icon.write() = Some(iced::mouse::Interaction::Grabbing);
                return self.redraw_with_tag_overlays();
            }

            self.update_tag_overlays();
            if !self.is_dragging {
                // Check if hovering over a tag - show move cursor
                let hovering_tag = self.with_edit_state(|state| state.get_buffer().tags.iter().any(|t| t.contains(pos)));
                if hovering_tag {
                    *self.canvas.terminal.cursor_icon.write() = Some(iced::mouse::Interaction::Grab);
                } else {
                    *self.canvas.terminal.cursor_icon.write() = None;
                }
                return ToolEvent::Redraw;
            }
            // Continue to drag handling below if dragging
        }

        if !self.is_dragging {
            // Just hovering - update cursor for selection resize handles
            if matches!(self.current_tool, Tool::Click | Tool::Select) {
                let selection_drag = self.get_selection_drag_at(pos);
                let cursor = selection_drag.to_cursor_interaction();
                *self.canvas.terminal.cursor_icon.write() = cursor;
            } else if self.current_tool != Tool::Tag {
                // Reset cursor for other tools (Tag tool handles its own cursor above)
                *self.canvas.terminal.cursor_icon.write() = None;
            }

            if !matches!(
                self.current_tool,
                Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled
            ) {
                self.clear_tool_overlay();
            }
            return ToolEvent::None;
        }

        self.drag_pos.cur = pos;
        self.drag_pos.cur_abs = pos;

        match self.current_tool {
            Tool::Click | Tool::Font | Tool::Select => {
                // Check if we're doing a layer drag
                if self.layer_drag_active {
                    // Calculate new offset from drag delta
                    let delta = self.drag_pos.cur_abs - self.drag_pos.start_abs;
                    let new_offset = self.layer_drag_start_offset + delta;

                    // Update preview offset for visual feedback
                    self.with_edit_state(|state| {
                        if let Some(layer) = state.get_cur_layer_mut() {
                            layer.set_preview_offset(Some(new_offset));
                        }
                        state.mark_dirty();
                    });
                    // Update layer border display
                    return self.redraw_with_layer_bounds();
                }

                self.update_selection_from_drag();
                ToolEvent::Redraw
            }
            Tool::Pencil => {
                // Compute current half-block position
                let new_half_block_pos = self.compute_half_block_pos(pixel_position);

                // Check if we're in half-block mode
                let is_half_block_mode = matches!(self.top_toolbar.brush_options.primary, BrushPrimaryMode::HalfBlock);

                if is_half_block_mode {
                    // Interpolate in half-block coordinates for smooth 2x Y resolution
                    let mut c_abs = self.half_block_click_pos;

                    while c_abs != new_half_block_pos {
                        let s = (new_half_block_pos - c_abs).signum();
                        c_abs = c_abs + s;
                        self.half_block_click_pos = c_abs;

                        // Apply brush_size in half-block coordinates
                        self.apply_half_block_with_brush_size(c_abs, self.paint_button);
                    }
                    self.drag_pos.cur_half_block = new_half_block_pos;
                } else {
                    // Normal mode: interpolate in cell coordinates
                    let Some(last) = self.paint_last_pos else {
                        self.paint_last_pos = Some(pos);
                        self.half_block_click_pos = new_half_block_pos;
                        return ToolEvent::Redraw;
                    };

                    let points = icy_engine_edit::brushes::get_line_points(last, pos);
                    for p in points {
                        self.apply_paint_stamp(p, pixel_position, self.paint_button);
                    }
                    self.paint_last_pos = Some(pos);
                    self.half_block_click_pos = new_half_block_pos;
                }
                ToolEvent::Redraw
            }
            Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled => {
                // Update half-block drag positions too so half-block previews are correct.
                let new_half_block_pos = self.compute_half_block_pos(pixel_position);
                self.drag_pos.cur_half_block = new_half_block_pos;

                self.update_shape_tool_overlay_preview();
                ToolEvent::Redraw
            }
            Tool::Tag => {
                // Handle tag drag (multi-selection)
                if self.tag_drag_active && !self.tag_drag_indices.is_empty() {
                    let delta = self.drag_pos.cur_abs - self.drag_pos.start_abs;

                    // Collect moves to apply (to avoid borrow conflict)
                    let moves: Vec<(usize, icy_engine::Position)> = self
                        .tag_drag_indices
                        .iter()
                        .zip(self.tag_drag_start_positions.iter())
                        .map(|(&tag_idx, &start_pos)| (tag_idx, start_pos + delta))
                        .collect();

                    // Apply all moves
                    self.with_edit_state(|state| {
                        for (tag_idx, new_pos) in moves {
                            let _ = state.move_tag(tag_idx, new_pos);
                        }
                        state.mark_dirty();
                    });
                    // Update tag overlays
                    return self.redraw_with_tag_overlays();
                }

                // Handle selection rectangle drag
                if self.tag_selection_drag_active {
                    // Draw selection rectangle overlay
                    self.update_tag_selection_drag_overlay();
                    return ToolEvent::Redraw;
                }

                ToolEvent::None
            }
            _ => ToolEvent::None,
        }
    }

    fn update_brush_preview(&mut self, pos: icy_engine::Position, pixel_position: (f32, f32)) {
        let show_preview = matches!(self.current_tool, Tool::Pencil);
        if !show_preview {
            self.canvas.set_brush_preview(None);
            return;
        }

        let brush_size = self.top_toolbar.brush_options.brush_size.max(1) as i32;
        let half = brush_size / 2;

        // Get font dimensions for pixel conversion
        let (font_w, font_h) = {
            let screen = self.screen.lock();
            let size = screen.font_dimensions();
            (size.width as f32, size.height as f32)
        };

        let is_half_block_mode = matches!(self.top_toolbar.brush_options.primary, BrushPrimaryMode::HalfBlock);

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

    /// Get what kind of selection drag would happen at this position
    fn get_selection_drag_at(&mut self, pos: icy_engine::Position) -> SelectionDrag {
        let selection = self.with_edit_state(|state| state.selection());

        if let Some(selection) = selection {
            let rect = selection.as_rectangle();

            if rect.contains_pt(pos) {
                // Check edges/corners (within 2 chars)
                let left = pos.x - rect.left() < 2;
                let top = pos.y - rect.top() < 2;
                let right = rect.right() - pos.x < 2;
                let bottom = rect.bottom() - pos.y < 2;

                // Corners first
                if left && top {
                    return SelectionDrag::TopLeft;
                }
                if right && top {
                    return SelectionDrag::TopRight;
                }
                if left && bottom {
                    return SelectionDrag::BottomLeft;
                }
                if right && bottom {
                    return SelectionDrag::BottomRight;
                }

                // Edges
                if left {
                    return SelectionDrag::Left;
                }
                if right {
                    return SelectionDrag::Right;
                }
                if top {
                    return SelectionDrag::Top;
                }
                if bottom {
                    return SelectionDrag::Bottom;
                }

                // Inside - move
                return SelectionDrag::Move;
            }
        }

        SelectionDrag::None
    }

    /// Update selection based on current drag state
    fn update_selection_from_drag(&mut self) {
        use icy_engine::{Rectangle, Selection};

        let add_type = self.current_select_add_type();

        match self.selection_drag {
            SelectionDrag::None => {}
            SelectionDrag::Create => {
                // Create new selection from drag start to current
                let selection = Selection {
                    anchor: self.drag_pos.start_abs,
                    lead: self.drag_pos.cur_abs,
                    locked: false,
                    shape: icy_engine::Shape::Rectangle,
                    add_type,
                };
                self.with_edit_state(|state| {
                    let _ = state.set_selection(selection);
                });
            }
            SelectionDrag::Move => {
                // Move entire selection
                if let Some(start_rect) = self.start_selection {
                    let delta_x = self.drag_pos.cur_abs.x - self.drag_pos.start_abs.x;
                    let delta_y = self.drag_pos.cur_abs.y - self.drag_pos.start_abs.y;

                    let new_rect = Rectangle::from(start_rect.left() + delta_x, start_rect.top() + delta_y, start_rect.width(), start_rect.height());

                    self.with_edit_state(|state| {
                        let mut selection = Selection::from(new_rect);
                        selection.add_type = add_type;
                        let _ = state.set_selection(selection);
                    });
                }
            }
            SelectionDrag::Left => {
                self.resize_selection_left();
            }
            SelectionDrag::Right => {
                self.resize_selection_right();
            }
            SelectionDrag::Top => {
                self.resize_selection_top();
            }
            SelectionDrag::Bottom => {
                self.resize_selection_bottom();
            }
            SelectionDrag::TopLeft => {
                self.resize_selection_corner(true, true);
            }
            SelectionDrag::TopRight => {
                self.resize_selection_corner(false, true);
            }
            SelectionDrag::BottomLeft => {
                self.resize_selection_corner(true, false);
            }
            SelectionDrag::BottomRight => {
                self.resize_selection_corner(false, false);
            }
        }

        // Keep add/subtract preview consistent while resizing/moving too.
        if self.current_tool == Tool::Select {
            self.with_edit_state(|state| {
                if let Some(mut sel) = state.selection() {
                    if sel.add_type != add_type {
                        sel.add_type = add_type;
                        let _ = state.set_selection(sel);
                    }
                }
            });
        }

        // Update the shader's selection display
        self.update_selection_display();
    }

    fn resize_selection_left(&mut self) {
        use icy_engine::{Rectangle, Selection};
        if let Some(start_rect) = self.start_selection {
            let delta = self.drag_pos.start_abs.x - self.drag_pos.cur_abs.x;
            let mut new_left = start_rect.left() - delta;
            let mut new_width = start_rect.width() + delta;

            if new_width < 0 {
                new_width = new_left - start_rect.right();
                new_left = start_rect.right();
            }

            let new_rect = Rectangle::from(new_left, start_rect.top(), new_width, start_rect.height());
            self.with_edit_state(|state| {
                let _ = state.set_selection(Selection::from(new_rect));
            });
        }
    }

    fn resize_selection_right(&mut self) {
        use icy_engine::{Rectangle, Selection};
        if let Some(start_rect) = self.start_selection {
            let mut new_width = start_rect.width() - self.drag_pos.start_abs.x + self.drag_pos.cur_abs.x;
            let mut new_left = start_rect.left();

            if new_width < 0 {
                new_left = start_rect.left() + new_width;
                new_width = start_rect.left() - new_left;
            }

            let new_rect = Rectangle::from(new_left, start_rect.top(), new_width, start_rect.height());
            self.with_edit_state(|state| {
                let _ = state.set_selection(Selection::from(new_rect));
            });
        }
    }

    fn resize_selection_top(&mut self) {
        use icy_engine::{Rectangle, Selection};
        if let Some(start_rect) = self.start_selection {
            let delta = self.drag_pos.start_abs.y - self.drag_pos.cur_abs.y;
            let mut new_top = start_rect.top() - delta;
            let mut new_height = start_rect.height() + delta;

            if new_height < 0 {
                new_height = new_top - start_rect.bottom();
                new_top = start_rect.bottom();
            }

            let new_rect = Rectangle::from(start_rect.left(), new_top, start_rect.width(), new_height);
            self.with_edit_state(|state| {
                let _ = state.set_selection(Selection::from(new_rect));
            });
        }
    }

    fn resize_selection_bottom(&mut self) {
        use icy_engine::{Rectangle, Selection};
        if let Some(start_rect) = self.start_selection {
            let mut new_height = start_rect.height() - self.drag_pos.start_abs.y + self.drag_pos.cur_abs.y;
            let mut new_top = start_rect.top();

            if new_height < 0 {
                new_top = start_rect.top() + new_height;
                new_height = start_rect.top() - new_top;
            }

            let new_rect = Rectangle::from(start_rect.left(), new_top, start_rect.width(), new_height);
            self.with_edit_state(|state| {
                let _ = state.set_selection(Selection::from(new_rect));
            });
        }
    }

    /// Resize selection from a corner (changes both X and Y dimensions at once)
    fn resize_selection_corner(&mut self, resize_left: bool, resize_top: bool) {
        use icy_engine::{Rectangle, Selection};
        if let Some(start_rect) = self.start_selection {
            // Calculate new X dimension
            let (new_left, new_width) = if resize_left {
                let delta = self.drag_pos.start_abs.x - self.drag_pos.cur_abs.x;
                let mut left = start_rect.left() - delta;
                let mut width = start_rect.width() + delta;
                if width < 0 {
                    width = left - start_rect.right();
                    left = start_rect.right();
                }
                (left, width)
            } else {
                let mut width = start_rect.width() - self.drag_pos.start_abs.x + self.drag_pos.cur_abs.x;
                let mut left = start_rect.left();
                if width < 0 {
                    left = start_rect.left() + width;
                    width = start_rect.left() - left;
                }
                (left, width)
            };

            // Calculate new Y dimension
            let (new_top, new_height) = if resize_top {
                let delta = self.drag_pos.start_abs.y - self.drag_pos.cur_abs.y;
                let mut top = start_rect.top() - delta;
                let mut height = start_rect.height() + delta;
                if height < 0 {
                    height = top - start_rect.bottom();
                    top = start_rect.bottom();
                }
                (top, height)
            } else {
                let mut height = start_rect.height() - self.drag_pos.start_abs.y + self.drag_pos.cur_abs.y;
                let mut top = start_rect.top();
                if height < 0 {
                    top = start_rect.top() + height;
                    height = start_rect.top() - top;
                }
                (top, height)
            };

            let new_rect = Rectangle::from(new_left, new_top, new_width, new_height);
            self.with_edit_state(|state| {
                let _ = state.set_selection(Selection::from(new_rect));
            });
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
        let sidebar_width = constants::LEFT_BAR_WIDTH;

        // Get caret position and colors from the edit state (also used for palette mode decisions)
        let (caret_fg, caret_bg, caret_row, caret_col, buffer_height, buffer_width, format_mode, buffer_type) = {
            let mut screen_guard = self.screen.lock();
            let state = screen_guard
                .as_any_mut()
                .downcast_mut::<EditState>()
                .expect("AnsiEditor screen should always be EditState");
            state.set_caret_visible(state.selection().is_none());
            let caret = state.get_caret();
            let buffer = state.get_buffer();
            let format_mode = state.get_format_mode();
            let fg = caret.attribute.foreground();
            let bg = caret.attribute.background();
            let caret_x = caret.x;
            let caret_y = caret.y;
            let height = buffer.height();
            let width = buffer.width();
            let buffer_type = buffer.buffer_type;
            (fg, bg, caret_y as usize, caret_x as usize, height, width as usize, format_mode, buffer_type)
        };

        // Palette grid - adapts to sidebar width
        // In XBinExtended only 8 colors are available
        let palette_limit = (format_mode == icy_engine_edit::FormatMode::XBinExtended).then_some(8);
        let palette_view = self
            .palette_grid
            .view_with_width(sidebar_width, palette_limit)
            .map(AnsiEditorMessage::PaletteGrid);

        // Tool panel - calculate columns based on sidebar width
        // Use theme's main area background color
        let bg_weakest = main_area_background(&Theme::Dark);

        // In paste mode, show paste controls instead of tool panel
        let left_sidebar: iced::widget::Column<'_, AnsiEditorMessage> = if self.is_paste_mode() {
            let paste_controls = self.view_paste_sidebar_controls();
            column![palette_view, paste_controls].spacing(4)
        } else {
            let tool_panel = self.tool_panel.view_with_config(sidebar_width, bg_weakest).map(AnsiEditorMessage::ToolPanel);
            column![palette_view, tool_panel].spacing(4)
        };

        // === TOP TOOLBAR (with color switcher on the left) ===

        // Color switcher (classic icy_draw style) - shows caret's foreground/background colors
        let color_switcher = self.color_switcher.view(caret_fg, caret_bg).map(AnsiEditorMessage::ColorSwitcher);

        // Get FKeys and font/palette for toolbar
        let (fkeys, current_font, palette) = {
            let opts = self.options.read();
            let fkeys = opts.fkeys.clone();

            let mut screen_guard = self.screen.lock();
            let state = screen_guard
                .as_any_mut()
                .downcast_mut::<EditState>()
                .expect("AnsiEditor screen should always be EditState");
            let buffer = state.get_buffer();
            let caret = state.get_caret();
            let font_page = caret.font_page();
            let font = buffer.font(font_page).or_else(|| buffer.font(0)).cloned();
            let palette = buffer.palette.clone();
            (fkeys, font, palette)
        };

        // Clone font for char selector overlay (will be used later if popup is open)
        let font_for_char_selector = current_font.clone();

        // Build font panel info for Font tool
        let font_panel_info = if self.current_tool == Tool::Font {
            Some(self.build_font_panel_info())
        } else {
            None
        };

        // Build pipette panel info for Pipette tool
        let pipette_info = if self.current_tool == Tool::Pipette {
            Some(self.build_pipette_panel_info(&palette))
        } else {
            None
        };

        // Use GPU FKeyToolbar for Click tool, regular TopToolbar for other tools
        let top_toolbar_content: Element<'_, AnsiEditorMessage> = if self.current_tool == Tool::Click {
            self.fkey_toolbar
                .view(fkeys.clone(), current_font, palette.clone(), caret_fg, caret_bg, &Theme::Dark)
                .map(AnsiEditorMessage::FKeyToolbar)
        } else {
            // Get selected tag info for Tag tool toolbar
            let selected_tag_info = if self.current_tool == Tool::Tag && self.tag_selection.len() == 1 {
                let idx = self.tag_selection[0];
                let mut screen_guard = self.screen.lock();
                if let Some(state) = screen_guard.as_any_mut().downcast_mut::<EditState>() {
                    state.get_buffer().tags.get(idx).map(|tag| widget::toolbar::top::SelectedTagInfo {
                        position: tag.position,
                        replacement: tag.replacement_value.clone(),
                    })
                } else {
                    None
                }
            } else {
                None
            };

            self.top_toolbar
                .view(
                    self.current_tool,
                    &fkeys,
                    buffer_type,
                    font_for_char_selector.clone(),
                    &Theme::Dark,
                    caret_fg,
                    caret_bg,
                    &palette,
                    font_panel_info.as_ref(),
                    pipette_info.as_ref(),
                    self.tag_add_new_index.is_some(),
                    selected_tag_info,
                    self.tag_selection.len(),
                    self.is_paste_mode(),
                )
                .map(AnsiEditorMessage::TopToolbar)
        };

        let toolbar_height = constants::TOP_CONTROL_TOTAL_HEIGHT;

        let _tags_button: Element<'_, AnsiEditorMessage> = icy_engine_gui::ui::secondary_button("Tags…", Some(AnsiEditorMessage::OpenTagListDialog)).into();

        let top_toolbar = row![color_switcher, top_toolbar_content].spacing(4).align_y(Alignment::Start);

        // === CENTER: Canvas ===
        // Canvas is created FIRST so Terminal's shader renders and populates the shared cache
        let canvas = self
            .canvas
            .view_with_context_menu(self.current_tool == Tool::Click)
            .map(AnsiEditorMessage::Canvas);

        // === RIGHT PANEL ===
        // Right panel created AFTER canvas because minimap uses Terminal's render cache
        // which is populated when canvas.view() calls the Terminal shader

        // Compute viewport info for the minimap from the canvas terminal
        let viewport_info = self.compute_viewport_info();
        // Pass the terminal's render cache to the minimap for shared texture access
        let render_cache = &self.canvas.terminal.render_cache;
        let right_panel = self
            .right_panel
            .view(&self.screen, &viewport_info, Some(render_cache))
            .map(AnsiEditorMessage::RightPanel);

        // === LINE NUMBERS (optional) ===
        // Get scroll position from viewport for line numbers
        let (scroll_x, scroll_y) = {
            let vp = self.canvas.terminal.viewport.read();
            (vp.scroll_x, vp.scroll_y)
        };

        // Get font dimensions for line numbers positioning
        let (font_width, font_height) = {
            let screen: parking_lot::lock_api::MutexGuard<'_, parking_lot::RawMutex, Box<dyn Screen + 'static>> = self.screen.lock();
            let size = screen.font_dimensions();
            (size.width as f32, size.height as f32)
        };

        // Build the center area with optional line numbers overlay and tag context menu
        let mut center_layers: Vec<Element<'_, AnsiEditorMessage>> = vec![container(canvas).width(Length::Fill).height(Length::Fill).into()];

        if self.show_line_numbers {
            // Create line numbers overlay - uses RenderInfo.display_scale for actual zoom
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

        // Add tag context menu overlay if active
        if let Some((tag_index, _menu_pos)) = self.tag_context_menu {
            let context_menu = self.build_tag_context_menu(tag_index);
            center_layers.push(context_menu);
        }

        let center_area: Element<'_, AnsiEditorMessage> = iced::widget::stack(center_layers).width(Length::Fill).height(Length::Fill).into();

        // Main layout:
        // Left column: toolbar on top, then left sidebar + canvas
        // Right: right panel spanning full height

        let left_content_row = row![
            // Left sidebar - dynamic width based on palette size
            container(left_sidebar).width(Length::Fixed(sidebar_width)),
            // Center - canvas with optional line numbers
            center_area,
        ];

        let left_column = column![
            // Top toolbar - full width of left area
            container(top_toolbar)
                .width(Length::Fill)
                .height(Length::Fixed(toolbar_height))
                .style(container::rounded_box),
            // Left sidebar + canvas
            left_content_row,
        ]
        .spacing(0);

        let main_layout: Element<'_, AnsiEditorMessage> = row![
            left_column,
            // Right panel - fixed width, full height
            container(right_panel).width(Length::Fixed(RIGHT_PANEL_BASE_WIDTH)),
        ]
        .into();

        // Apply tag dialog modal overlay if active
        if let Some(tag_dialog) = &self.tag_dialog {
            let modal_content = tag_dialog.view().map(AnsiEditorMessage::TagDialog);
            icy_engine_gui::ui::modal(main_layout, modal_content, AnsiEditorMessage::TagDialog(TagDialogMessage::Cancel))
        } else if let Some(tag_list_dialog) = &self.tag_list_dialog {
            let modal_content = tag_list_dialog.view().map(AnsiEditorMessage::TagListDialog);
            icy_engine_gui::ui::modal(main_layout, modal_content, AnsiEditorMessage::TagListDialog(TagListDialogMessage::Close))
        } else if let Some(target) = self.char_selector_target {
            let current_code = match target {
                CharSelectorTarget::FKeySlot(slot) => fkeys.code_at(fkeys.current_set(), slot),
                CharSelectorTarget::BrushChar => self.top_toolbar.brush_options.paint_char as u16,
            };

            let selector_canvas = CharSelector::new(current_code)
                .view(font_for_char_selector, palette.clone(), caret_fg, caret_bg)
                .map(AnsiEditorMessage::CharSelector);

            let modal_content = icy_engine_gui::ui::modal_container(selector_canvas, CHAR_SELECTOR_WIDTH);

            // Use modal() which closes on click outside (on_blur)
            icy_engine_gui::ui::modal(main_layout, modal_content, AnsiEditorMessage::CharSelector(CharSelectorMessage::Cancel))
        } else if self.outline_selector_open {
            // Apply outline selector modal overlay if active
            let current_style = *self.options.read().font_outline_style.read();

            let selector_canvas = OutlineSelector::new(current_style).view().map(AnsiEditorMessage::OutlineSelector);

            let modal_content = icy_engine_gui::ui::modal_container(selector_canvas, outline_selector_width());

            // Use modal() which closes on click outside (on_blur)
            icy_engine_gui::ui::modal(main_layout, modal_content, AnsiEditorMessage::OutlineSelector(OutlineSelectorMessage::Cancel))
        } else {
            main_layout
        }
    }

    /// Build the tag context menu overlay
    fn build_tag_context_menu(&self, tag_index: usize) -> Element<'_, AnsiEditorMessage> {
        use iced::widget::{button, mouse_area, text};
        use icy_engine_gui::ui::TEXT_SIZE_NORMAL;

        let edit_btn = button(text("Edit").size(TEXT_SIZE_NORMAL))
            .padding([4, 12])
            .style(iced::widget::button::text)
            .on_press(AnsiEditorMessage::TagEdit(tag_index));

        let clone_btn = button(text("Clone").size(TEXT_SIZE_NORMAL))
            .padding([4, 12])
            .style(iced::widget::button::text)
            .on_press(AnsiEditorMessage::TagClone(tag_index));

        let delete_btn = button(text("Delete").size(TEXT_SIZE_NORMAL))
            .padding([4, 12])
            .style(iced::widget::button::text)
            .on_press(AnsiEditorMessage::TagDelete(tag_index));

        let mut menu_items: Vec<Element<'_, AnsiEditorMessage>> = vec![edit_btn.into(), clone_btn.into(), delete_btn.into()];

        // Add "Delete Selected" option if multiple tags are selected
        if self.tag_selection.len() > 1 {
            let delete_selected_btn = button(text(format!("Delete {} Selected", self.tag_selection.len())).size(TEXT_SIZE_NORMAL))
                .padding([4, 12])
                .style(iced::widget::button::text)
                .on_press(AnsiEditorMessage::TagDeleteSelected);
            menu_items.push(delete_selected_btn.into());
        }

        let menu_content = container(column(menu_items).spacing(2).width(Length::Fixed(150.0)))
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(iced::Background::Color(palette.background.weak.color)),
                    border: iced::Border {
                        color: palette.background.strong.color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                }
            })
            .padding(4);

        // Get menu position from the tag's screen position
        let (menu_x, menu_y) = if let Some((_, pos)) = self.tag_context_menu {
            // Convert buffer position to screen pixels
            let render_info = self.canvas.terminal.render_info.read();
            let viewport = self.canvas.terminal.viewport.read();
            let scale = render_info.display_scale;

            // Get font dimensions
            let (font_w, font_h) = {
                let screen = self.screen.lock();
                let size = screen.font_dimensions();
                (size.width as f32, size.height as f32)
            };

            let x = ((pos.x as f32 - viewport.scroll_x) * font_w * scale) as f32;
            let y = ((pos.y as f32 - viewport.scroll_y + 1.0) * font_h * scale) as f32;
            (x, y)
        } else {
            (100.0, 100.0)
        };

        // Wrap in a mouse_area to handle clicks outside (close menu)
        let menu_positioned = container(menu_content).padding(iced::Padding {
            top: menu_y,
            left: menu_x,
            right: 0.0,
            bottom: 0.0,
        });

        // Full-screen clickable area that closes the menu when clicked outside
        let backdrop = mouse_area(container(menu_positioned).width(Length::Fill).height(Length::Fill)).on_press(AnsiEditorMessage::TagContextMenuClose);

        backdrop.into()
    }

    /// Sync UI components with the current edit state
    /// Call this after operations that may change the palette or tags
    pub fn sync_ui(&mut self) {
        let (palette, format_mode, tag_count) =
            self.with_edit_state(|state| (state.get_buffer().palette.clone(), state.get_format_mode(), state.get_buffer().tags.len()));
        let palette_limit = (format_mode == icy_engine_edit::FormatMode::XBinExtended).then_some(8);
        self.palette_grid.sync_palette(&palette, palette_limit);
        self.color_switcher.sync_palette(&palette);
        // Clear invalid tag selections (tags may have been removed by undo)
        self.tag_selection.retain(|&idx| idx < tag_count);
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

        // Sync toolbar filled toggle with the selected tool.
        self.top_toolbar.filled = matches!(tool, Tool::RectangleFilled | Tool::EllipseFilled);

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
        // HalfBlock tracking is tied to the tool *options* (brush primary mode), not the tool itself.
        let wants_half_block = self.top_toolbar.brush_options.primary == BrushPrimaryMode::HalfBlock;
        let tool_allows = Self::tool_supports_half_block_mode(self.current_tool);

        let tracking = if wants_half_block && tool_allows {
            icy_engine_gui::MouseTracking::HalfBlock
        } else {
            icy_engine_gui::MouseTracking::Chars
        };

        self.canvas.terminal.set_mouse_tracking(tracking);
    }

    /// Build FontPanelInfo from current font tool state
    fn build_font_panel_info(&self) -> FontPanelInfo {
        let font_name = self.font_tool.with_selected_font(|f| f.name().to_string()).unwrap_or_default();

        let has_fonts = self.font_tool.has_fonts();

        // Build char availability for preview (chars ! to ~)
        let char_availability: Vec<(char, bool)> = ('!'..='~').map(|ch| (ch, self.font_tool.has_char(ch))).collect();

        // Get current outline style
        let outline_style = { *self.options.read().font_outline_style.read() };

        FontPanelInfo {
            font_name,
            has_fonts,
            char_availability,
            outline_style,
        }
    }

    /// Build pipette panel info from current state
    fn build_pipette_panel_info(&self, palette: &icy_engine::Palette) -> PipettePanelInfo {
        let cur_char = self.pipette.cur_char;
        let take_fg = self.pipette.take_fg;
        let take_bg = self.pipette.take_bg;

        let (fg_color, bg_color) = if let Some(ch) = cur_char {
            let fg_idx = ch.attribute.foreground();
            let bg_idx = ch.attribute.background();

            let fg_rgb = palette.color(fg_idx).rgb();
            let bg_rgb = palette.color(bg_idx).rgb();

            (Some(fg_rgb), Some(bg_rgb))
        } else {
            (None, None)
        };

        PipettePanelInfo {
            cur_char,
            take_fg,
            take_bg,
            fg_color,
            bg_color,
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
    pub font_name: String,
    /// Current format mode
    pub format_mode: icy_engine_edit::FormatMode,
    /// Currently active font slot (0 or 1 for XBinExtended)
    pub current_font_slot: usize,
    /// Font names for slots (only set for XBinExtended)
    pub slot_fonts: Option<[Option<String>; 2]>,
}
