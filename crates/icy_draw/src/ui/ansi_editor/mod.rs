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

mod canvas_view;
mod channels_view;
mod char_selector;
mod color_switcher_gpu;
pub mod constants;
mod edit_layer_dialog;
mod file_settings_dialog;
mod fkey_layout;
mod fkey_toolbar;
mod fkey_toolbar_gpu;
mod font_selector_dialog;
mod font_slot_manager_dialog;
mod font_tool;
mod glyph_renderer;
mod layer_view;
mod line_numbers;
pub mod menu_bar;
mod minimap_view;
mod outline_selector;
mod palette_grid;
mod reference_image_dialog;
mod right_panel;
mod segmented_control_gpu;
mod segmented_layout;
mod tag_dialog;
mod tag_list_dialog;
mod tdf_font_selector;
mod tool_panel;
mod tool_panel_wrapper;
mod top_toolbar;

pub use canvas_view::*;
pub use char_selector::*;
pub use color_switcher_gpu::*;
pub use edit_layer_dialog::*;
pub use file_settings_dialog::*;
// Old Canvas-based FKeyToolbar - replaced by GPU version
// pub use fkey_toolbar::*;
pub use fkey_toolbar_gpu::*;
pub use font_selector_dialog::*;
pub use font_slot_manager_dialog::*;
pub use font_tool::FontToolState;
use icy_engine_edit::EditState;
use icy_engine_edit::tools::{self, Tool, ToolEvent};
pub use layer_view::*;
pub use minimap_view::*;
pub use palette_grid::*;
pub use reference_image_dialog::*;
pub use right_panel::*;
use tag_dialog::{TagDialog, TagDialogMessage};
use tag_list_dialog::{TagListDialog, TagListDialogMessage, TagListItem};
pub use tdf_font_selector::{TdfFontSelectorDialog, TdfFontSelectorMessage};
// Use shared GPU-accelerated tool panel via wrapper
pub use tool_panel_wrapper::{ToolPanel, ToolPanelMessage};
pub use top_toolbar::*;

use std::path::PathBuf;
use std::sync::Arc;

use iced::{
    Alignment, Element, Length, Task, Theme,
    widget::{column, container, row},
};
use icy_engine::formats::{FileFormat, LoadData};
use icy_engine::{MouseButton, Position, Screen, Tag, TagRole, TextBuffer, TextPane};
use icy_engine_gui::crt_shader_state::{is_command_pressed, is_ctrl_pressed, is_shift_pressed};
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
use outline_selector::{OutlineSelector, OutlineSelectorMessage, outline_selector_width};

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
}

static mut NEXT_ID: u64 = 0;

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

        // scroll_y is in content coordinates - convert to half-block lines (2x)
        let font_height = render_info.font_height.max(1.0);
        let scroll_offset_half_lines = (viewport.scroll_y / font_height * 2.0).floor() as i32;

        // Get absolute half-block coordinates (with scroll offset)
        let abs_half_block = icy_engine::Position::new(cell_x, half_block_y + scroll_offset_half_lines);

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
        // half_block_is_top is passed in directly for HalfBlock mode with brush_size

        self.with_edit_state(|state| {
            let (offset, layer_w, layer_h) = if let Some(layer) = state.get_cur_layer() {
                (layer.offset(), layer.width(), layer.height())
            } else {
                return;
            };
            let use_selection = state.is_something_selected();

            let caret_attr = state.get_caret().attribute;
            let swap_for_colors = swap_colors && !matches!(primary, BrushPrimaryMode::Shading);
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
                        Tool::Erase => {
                            let _ = state.set_char_in_atomic(layer_pos, icy_engine::AttributedChar::invisible());
                        }
                        Tool::Pencil
                        | Tool::Brush
                        | Tool::Line
                        | Tool::RectangleOutline
                        | Tool::RectangleFilled
                        | Tool::EllipseOutline
                        | Tool::EllipseFilled => {
                            use icy_engine_edit::brushes::{BrushMode as EngineBrushMode, ColorMode as EngineColorMode, DrawContext, PointRole};

                            let brush_mode = match primary {
                                BrushPrimaryMode::Char => EngineBrushMode::Char(paint_char),
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
        let id = unsafe {
            NEXT_ID = NEXT_ID.wrapping_add(1);
            NEXT_ID
        };

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
        let canvas = CanvasView::new(screen.clone(), shared_monitor_settings);

        let initial_fkey_set = {
            let mut opts = options.write();
            opts.fkeys.clamp_current_set();
            opts.fkeys.current_set
        };

        let mut top_toolbar = TopToolbar::new();
        top_toolbar.select_options.current_fkey_page = initial_fkey_set;

        Self {
            id,
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

            paint_undo: None,
            paint_last_pos: None,
            paint_button: iced::mouse::Button::Left,

            half_block_click_pos: icy_engine::Position::default(),

            shape_clear: false,
        }
    }

    fn clear_tool_overlay(&mut self) {
        self.canvas.set_tool_overlay_mask(None, 1.0);
    }

    fn cancel_shape_drag(&mut self) -> bool {
        if self.is_dragging
            && matches!(
                self.current_tool,
                Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled
            )
        {
            self.is_dragging = false;
            self.selection_drag = SelectionDrag::None;
            self.start_selection = None;
            self.paint_button = iced::mouse::Button::Left;
            self.shape_clear = false;
            self.clear_tool_overlay();
            return true;
        }
        false
    }

    fn shape_points(tool: Tool, p0: icy_engine::Position, p1: icy_engine::Position) -> Vec<icy_engine::Position> {
        use std::collections::HashSet;

        match tool {
            Tool::Line => icy_engine_edit::brushes::line::get_line_points(p0, p1),
            Tool::RectangleOutline | Tool::RectangleFilled => {
                let min_x = p0.x.min(p1.x);
                let max_x = p0.x.max(p1.x);
                let min_y = p0.y.min(p1.y);
                let max_y = p0.y.max(p1.y);

                let mut pts = Vec::new();
                if matches!(tool, Tool::RectangleFilled) {
                    for y in min_y..=max_y {
                        for x in min_x..=max_x {
                            pts.push(icy_engine::Position::new(x, y));
                        }
                    }
                } else {
                    // Outline/perimeter
                    for x in min_x..=max_x {
                        pts.push(icy_engine::Position::new(x, min_y));
                        if max_y != min_y {
                            pts.push(icy_engine::Position::new(x, max_y));
                        }
                    }
                    for y in (min_y + 1)..=(max_y - 1) {
                        pts.push(icy_engine::Position::new(min_x, y));
                        if max_x != min_x {
                            pts.push(icy_engine::Position::new(max_x, y));
                        }
                    }
                }
                pts
            }
            Tool::EllipseOutline | Tool::EllipseFilled => {
                let min_x = p0.x.min(p1.x);
                let max_x = p0.x.max(p1.x);
                let min_y = p0.y.min(p1.y);
                let max_y = p0.y.max(p1.y);

                let center = icy_engine::Position::new((min_x + max_x) / 2, (min_y + max_y) / 2);
                let radius_x = (max_x - min_x) / 2;
                let radius_y = (max_y - min_y) / 2;

                if matches!(tool, Tool::EllipseOutline) {
                    let mut set: HashSet<(i32, i32)> = HashSet::new();
                    let points = icy_engine_edit::brushes::ellipse::get_ellipse_points(center, radius_x, radius_y);
                    for (p, _) in points {
                        set.insert((p.x, p.y));
                    }
                    set.into_iter().map(|(x, y)| icy_engine::Position::new(x, y)).collect()
                } else {
                    // Match `fill_ellipse` scanline logic.
                    if radius_x <= 0 || radius_y <= 0 {
                        return vec![center];
                    }

                    let ry_f = radius_y as f64;
                    let rx_f = radius_x as f64;
                    let mut pts = Vec::new();

                    for dy in -radius_y..=radius_y {
                        let dy_f = dy as f64;
                        let x_extent = (rx_f * (1.0 - (dy_f / ry_f).powi(2)).sqrt()).round() as i32;
                        let y = center.y + dy;
                        for dx in -x_extent..=x_extent {
                            let x = center.x + dx;
                            pts.push(icy_engine::Position::new(x, y));
                        }
                    }
                    pts
                }
            }
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

        let is_half_block_mode = matches!(
            self.top_toolbar.brush_options.primary,
            crate::ui::ansi_editor::top_toolbar::BrushPrimaryMode::HalfBlock
        );

        let (mask_w, mask_h, cell_height_scale, p0, p1, (r, g, b)) = {
            let mut screen = self.screen.lock();
            let palette = screen.palette().clone();

            let edit_state = screen
                .as_any_mut()
                .downcast_mut::<icy_engine_edit::EditState>()
                .expect("screen should be EditState");

            let buffer = edit_state.get_buffer();
            let w = buffer.width().max(1) as u32;
            let h_cells = buffer.height().max(1) as u32;

            let caret_attr = edit_state.get_caret().attribute;
            let (fg, bg) = (caret_attr.foreground(), caret_attr.background());
            let col_idx: u32 = if self.shape_clear {
                0
            } else if self.paint_button == iced::mouse::Button::Right {
                bg
            } else {
                fg
            };
            let rgb = palette.rgb(col_idx);

            if is_half_block_mode {
                // Convert layer-local half-block drag positions to document half-block coordinates.
                let offset = edit_state.get_cur_layer().map(|l| l.offset()).unwrap_or_default();
                let hb_off = icy_engine::Position::new(offset.x, offset.y * 2);
                let start = self.drag_pos.start_half_block + hb_off;
                let cur = self.drag_pos.cur_half_block + hb_off;
                (w, h_cells * 2, 0.5, start, cur, rgb)
            } else {
                (w, h_cells, 1.0, self.drag_pos.start, self.drag_pos.cur, rgb)
            }
        };

        let alpha: u8 = 153; // ~0.6 like Moebius overlay
        let mut rgba = vec![0u8; (mask_w * mask_h * 4) as usize];
        let points = Self::shape_points(self.current_tool, p0, p1);

        for p in points {
            if p.x < 0 || p.y < 0 {
                continue;
            }
            let x = p.x as u32;
            let y = p.y as u32;
            if x >= mask_w || y >= mask_h {
                continue;
            }
            let idx = ((y * mask_w + x) * 4) as usize;
            rgba[idx] = r;
            rgba[idx + 1] = g;
            rgba[idx + 2] = b;
            rgba[idx + 3] = alpha;
        }

        self.canvas.set_tool_overlay_mask(Some((rgba, mask_w, mask_h)), cell_height_scale);
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

    fn has_tag_at(&mut self, pos: icy_engine::Position) -> bool {
        self.with_edit_state(|state| state.get_buffer().tags.iter().any(|t| t.contains(pos)))
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

    /// Save the document to the given path
    pub fn save(&mut self, path: &std::path::Path) -> Result<(), String> {
        let mut screen = self.screen.lock();
        if let Some(edit_state) = screen.as_any_mut().downcast_ref::<EditState>() {
            // Determine format from extension
            let format = FileFormat::from_path(path).ok_or_else(|| "Unknown file format".to_string())?;

            // Get buffer and save with default options
            let buffer = edit_state.get_buffer();
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
        if let Some(edit_state) = screen.as_any_mut().downcast_ref::<EditState>() {
            // Use ICY format for autosave to preserve all data (layers, fonts, etc.)
            let format = FileFormat::IcyDraw;
            let buffer = edit_state.get_buffer();
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
        self.tool_panel.needs_animation() || self.minimap_drag_pointer.is_some()
    }

    /// Get the current marker state for menu display
    pub fn get_marker_menu_state(&self) -> crate::ui::ansi_editor::menu_bar::MarkerMenuState {
        crate::ui::ansi_editor::menu_bar::MarkerMenuState {
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
        // Get direct viewport data from the terminal
        let vp = self.canvas.terminal.viewport.read();

        let content_width = vp.content_width.max(1.0);
        let content_height = vp.content_height.max(1.0);
        let visible_width = vp.visible_content_width();
        let visible_height = vp.visible_content_height();

        // Normalized position: where we are scrolled to (0.0-1.0)
        let x = vp.scroll_x / content_width;
        let y = vp.scroll_y / content_height;

        // Normalized size: how much of the content is visible (0.0-1.0)
        let width = (visible_width / content_width).min(1.0);
        let height = (visible_height / content_height).min(1.0);

        ViewportInfo { x, y, width, height }
    }

    /// Scroll the canvas to a normalized position (0.0-1.0)
    /// The viewport will be centered on this position
    fn scroll_canvas_to_normalized(&mut self, norm_x: f32, norm_y: f32) {
        let vp = self.canvas.terminal.viewport.read();
        let content_width = vp.content_width;
        let content_height = vp.content_height;
        let visible_width = vp.visible_content_width();
        let visible_height = vp.visible_content_height();
        drop(vp);

        // Convert normalized position to content coordinates
        // Center the viewport on the clicked position
        let target_x = norm_x * content_width - visible_width / 2.0;
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
                            length: 0,
                            alignment: std::fmt::Alignment::Left,
                            tag_placement: placement.to_engine(),
                            tag_role: TagRole::Displaycode,
                            attribute,
                        };

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
                            state.add_new_tag(new_tag)
                        }) {
                            log::warn!("Failed to add tag: {}", err);
                            return Task::none();
                        }

                        self.handle_tool_event(ToolEvent::Commit("Add tag".to_string()));
                        Task::none()
                    }
                }
            }
            AnsiEditorMessage::ToolPanel(msg) => {
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
                        self.top_toolbar.update(msg).map(AnsiEditorMessage::TopToolbar)
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

                        self.top_toolbar.update(TopToolbarMessage::ToggleFilled(v)).map(AnsiEditorMessage::TopToolbar)
                    }
                    _ => self.top_toolbar.update(msg).map(AnsiEditorMessage::TopToolbar),
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
                self.update_layer_bounds();
                Task::none()
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
                if let Some((norm_x, norm_y)) = self.right_panel.minimap.handle_click(
                    iced::Size::new(0.0, 0.0),
                    iced::Point::new(pointer_x, pointer_y),
                    Some(render_cache),
                ) {
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
                self.update_markers();
                Task::none()
            }
            AnsiEditorMessage::ClearGuide => {
                self.guide = None;
                self.update_markers();
                Task::none()
            }
            AnsiEditorMessage::SetRaster(x, y) => {
                if x <= 0 && y <= 0 {
                    self.raster = None;
                } else {
                    self.raster = Some((x as f32, y as f32));
                    self.show_raster = true;
                }
                self.update_markers();
                Task::none()
            }
            AnsiEditorMessage::ClearRaster => {
                self.raster = None;
                self.update_markers();
                Task::none()
            }
            AnsiEditorMessage::ToggleGuide => {
                self.show_guide = !self.show_guide;
                self.update_markers();
                Task::none()
            }
            AnsiEditorMessage::ToggleRaster => {
                self.show_raster = !self.show_raster;
                self.update_markers();
                Task::none()
            }
            AnsiEditorMessage::ToggleLineNumbers => {
                self.show_line_numbers = !self.show_line_numbers;
                Task::none()
            }
            AnsiEditorMessage::ToggleLayerBorders => {
                self.show_layer_borders = !self.show_layer_borders;
                self.update_layer_bounds();
                Task::none()
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
                if let Ok(cur_layer) = edit_state.get_current_layer() {
                    if let Some(layer) = buffer.layers.get(cur_layer) {
                        let offset = layer.base_offset();
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

        // Click and Font tools handle character input directly, skip tool shortcuts
        let skip_tool_shortcuts = matches!(self.current_tool, Tool::Click | Tool::Font);

        // Check for tool shortcuts (single character keys) - but not when in text input mode
        if !skip_tool_shortcuts && !modifiers.control() && !modifiers.alt() {
            if let iced::keyboard::Key::Character(c) = &key {
                if let Some(ch) = c.chars().next() {
                    // Find tool with this shortcut
                    for (slot_idx, pair) in tools::TOOL_SLOTS.iter().enumerate() {
                        if pair.primary.shortcut() == Some(ch) {
                            self.change_tool(tools::click_tool_slot(slot_idx, self.current_tool));
                            self.tool_panel.set_tool(self.current_tool);
                            return;
                        }
                        if pair.secondary.shortcut() == Some(ch) {
                            self.change_tool(tools::click_tool_slot(slot_idx, self.current_tool));
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
                    // Font tool: just move to next line
                    self.with_edit_state(|state| {
                        let pos = state.get_caret().position();
                        state.set_caret_position(Position::new(0, pos.y + 1));
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
                // Clear selection
                self.with_edit_state(|state| {
                    let _ = state.clear_selection();
                });
                self.update_selection_display();
                ToolEvent::Redraw
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
        let result: Result<Position, icy_engine::EngineError> = {
            let mut screen = self.screen.lock();
            let edit_state = screen
                .as_any_mut()
                .downcast_mut::<EditState>()
                .expect("AnsiEditor screen should always be EditState");

            let caret_pos = edit_state.get_caret().position();

            match TdfEditStateRenderer::new(edit_state, caret_pos.x, caret_pos.y) {
                Ok(mut renderer) => {
                    let render_options = retrofont::RenderOptions {
                        outline_style,
                        ..Default::default()
                    };

                    // Access font through library and render
                    let lib = self.font_tool.font_library.read();
                    if let Some(font) = lib.get_font(font_idx as usize) {
                        match font.render_glyph(&mut renderer, ch, &render_options) {
                            Ok(_) => Ok(renderer.position()),
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

    /// Handle tool-specific key events based on current tool
    fn handle_tool_key(&mut self, key: &iced::keyboard::Key, modifiers: &iced::keyboard::Modifiers) -> ToolEvent {
        use iced::keyboard::key::Named;

        // Moebius-style: Escape cancels an in-progress shape drag and clears the preview overlay.
        if let iced::keyboard::Key::Named(Named::Escape) = key {
            if self.cancel_shape_drag() {
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
                            self.update_selection_display();
                            return ToolEvent::Redraw;
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
                let tool_event = self.handle_tool_mouse_down(position, pixel_position, button, modifiers);
                self.handle_tool_event(tool_event);
            }
            CanvasMouseEvent::Release {
                position,
                pixel_position,
                button,
            } => {
                let tool_event = self.handle_tool_mouse_up(position, pixel_position, button);
                self.handle_tool_event(tool_event);
            }
            CanvasMouseEvent::Move { position, pixel_position } => {
                let tool_event = self.handle_tool_mouse_move(position, pixel_position);
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
    fn handle_tool_mouse_down(
        &mut self,
        pos: icy_engine::Position,
        pixel_position: (f32, f32),
        button: iced::mouse::Button,
        _modifiers: icy_engine::KeyModifiers,
    ) -> ToolEvent {
        match self.current_tool {
            Tool::Tag => {
                if button == iced::mouse::Button::Left && !self.has_tag_at(pos) {
                    self.tag_list_dialog = None;
                    self.tag_dialog = Some(TagDialog::new(pos));
                    ToolEvent::Redraw
                } else {
                    ToolEvent::None
                }
            }
            Tool::Click | Tool::Font => {
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
                use crate::ui::ansi_editor::top_toolbar::{SelectionMode, SelectionModifier};
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
            Tool::Pencil | Tool::Brush | Tool::Erase => {
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
                    let desc = match self.current_tool {
                        Tool::Pencil => "Pencil",
                        Tool::Brush => "Brush",
                        Tool::Erase => "Erase",
                        _ => "Paint",
                    };
                    self.paint_undo = Some(self.with_edit_state(|state| state.begin_atomic_undo(desc)));
                }

                self.paint_last_pos = Some(pos);
                self.paint_button = button;

                // Check if we're in half-block mode
                let is_half_block_mode = matches!(
                    self.top_toolbar.brush_options.primary,
                    crate::ui::ansi_editor::top_toolbar::BrushPrimaryMode::HalfBlock
                );

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
                // Pick character/color at position
                // TODO: Actually pick from buffer
                ToolEvent::Status(format!("Picked at ({}, {})", pos.x, pos.y))
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
                            let (fg, bg) = if swap_colors {
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
                            let (fg, bg) = if swap_colors {
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
            _ => ToolEvent::None,
        }
    }

    /// Handle mouse up based on current tool
    fn handle_tool_mouse_up(&mut self, _pos: icy_engine::Position, _pixel_position: (f32, f32), _button: iced::mouse::Button) -> ToolEvent {
        if self.is_dragging && self.selection_drag != SelectionDrag::None {
            self.is_dragging = false;

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

                self.update_selection_display();
            }

            self.selection_drag = SelectionDrag::None;
            self.start_selection = None;
            return ToolEvent::Redraw;
        }

        if self.is_dragging && matches!(self.current_tool, Tool::Pencil | Tool::Brush | Tool::Erase) {
            self.is_dragging = false;
            self.paint_last_pos = None;
            // Dropping the guard groups everything into one undo entry.
            self.paint_undo = None;
            self.paint_button = iced::mouse::Button::Left;

            let desc = match self.current_tool {
                Tool::Pencil => "Pencil stroke",
                Tool::Brush => "Brush stroke",
                Tool::Erase => "Erase stroke",
                _ => "Stroke",
            };
            return ToolEvent::Commit(desc.to_string());
        }

        if self.is_dragging
            && matches!(
                self.current_tool,
                Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled
            )
        {
            self.is_dragging = false;

            let desc = format!("{} drawn", self.current_tool.name());
            self.paint_undo = Some(self.with_edit_state(|state| state.begin_atomic_undo(&desc)));

            let is_half_block_mode = matches!(
                self.top_toolbar.brush_options.primary,
                crate::ui::ansi_editor::top_toolbar::BrushPrimaryMode::HalfBlock
            );

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
            Tool::Pencil | Tool::Brush | Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled => {
                // TODO: Commit the drawn shape
                ToolEvent::Commit(format!("{} drawn", self.current_tool.name()))
            }
            _ => ToolEvent::None,
        }
    }

    /// Handle mouse move based on current tool
    fn handle_tool_mouse_move(&mut self, pos: icy_engine::Position, pixel_position: (f32, f32)) -> ToolEvent {
        // Update brush/pencil hover preview (shader rectangle)
        self.update_brush_preview(pos, pixel_position);

        if !self.is_dragging {
            // Just hovering
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
                self.update_selection_from_drag();
                ToolEvent::Redraw
            }
            Tool::Pencil | Tool::Brush | Tool::Erase => {
                // Compute current half-block position
                let new_half_block_pos = self.compute_half_block_pos(pixel_position);

                // Check if we're in half-block mode
                let is_half_block_mode = matches!(
                    self.top_toolbar.brush_options.primary,
                    crate::ui::ansi_editor::top_toolbar::BrushPrimaryMode::HalfBlock
                );

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

                    let points = icy_engine_edit::brushes::line::get_line_points(last, pos);
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
            _ => ToolEvent::None,
        }
    }

    fn update_brush_preview(&mut self, pos: icy_engine::Position, pixel_position: (f32, f32)) {
        let show_preview = matches!(self.current_tool, Tool::Pencil | Tool::Brush | Tool::Erase);
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

        let is_half_block_mode = matches!(
            self.top_toolbar.brush_options.primary,
            crate::ui::ansi_editor::top_toolbar::BrushPrimaryMode::HalfBlock
        );

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
                self.resize_selection_left();
                self.resize_selection_top();
            }
            SelectionDrag::TopRight => {
                self.resize_selection_right();
                self.resize_selection_top();
            }
            SelectionDrag::BottomLeft => {
                self.resize_selection_left();
                self.resize_selection_bottom();
            }
            SelectionDrag::BottomRight => {
                self.resize_selection_right();
                self.resize_selection_bottom();
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
        let tool_panel = self.tool_panel.view_with_config(sidebar_width, bg_weakest).map(AnsiEditorMessage::ToolPanel);

        let left_sidebar: iced::widget::Column<'_, AnsiEditorMessage> = column![palette_view, tool_panel,].spacing(4);

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

        // Use GPU FKeyToolbar for Click tool, regular TopToolbar for other tools
        let top_toolbar_content: Element<'_, AnsiEditorMessage> = if self.current_tool == Tool::Click {
            self.fkey_toolbar
                .view(fkeys.clone(), current_font, palette.clone(), caret_fg, caret_bg, &Theme::Dark)
                .map(AnsiEditorMessage::FKeyToolbar)
        } else {
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
                )
                .map(AnsiEditorMessage::TopToolbar)
        };

        let toolbar_height = constants::TOP_CONTROL_TOTAL_HEIGHT;

        let tags_button: Element<'_, AnsiEditorMessage> = icy_engine_gui::ui::secondary_button("Tags…", Some(AnsiEditorMessage::OpenTagListDialog)).into();

        let top_toolbar = row![color_switcher, top_toolbar_content].spacing(4).align_y(Alignment::Start);

        // === CENTER: Canvas ===
        // Canvas is created FIRST so Terminal's shader renders and populates the shared cache
        let canvas = self.canvas.view().map(AnsiEditorMessage::Canvas);

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

        // Build the center area with optional line numbers overlay
        let center_area: Element<'_, AnsiEditorMessage> = if self.show_line_numbers {
            // Create line numbers overlay - uses RenderInfo.display_scale for actual zoom
            let line_numbers_overlay = line_numbers::line_numbers_overlay(
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

            iced::widget::stack![container(canvas).width(Length::Fill).height(Length::Fill), line_numbers_overlay,]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            container(canvas).width(Length::Fill).height(Length::Fill).into()
        };

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

            // Use slot 0 as placeholder for CharSelector (it only needs the current_code for display)
            let selector_canvas = CharSelector::new(0, current_code)
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

    /// Sync UI components with the current edit state
    /// Call this after operations that may change the palette
    pub fn sync_ui(&mut self) {
        let (palette, format_mode) = self.with_edit_state(|state| (state.get_buffer().palette.clone(), state.get_format_mode()));
        let palette_limit = (format_mode == icy_engine_edit::FormatMode::XBinExtended).then_some(8);
        self.palette_grid.sync_palette(&palette, palette_limit);
        self.color_switcher.sync_palette(&palette);
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
        if self.current_tool == tool {
            return;
        }

        // Cancels any in-progress shape preview/drag when switching tools.
        let _ = self.cancel_shape_drag();

        let is_visble = matches!(tool, Tool::Click | Tool::Font);
        self.with_edit_state(|state| state.set_caret_visible(is_visble));

        // Sync toolbar filled toggle with the selected tool.
        self.top_toolbar.filled = matches!(tool, Tool::RectangleFilled | Tool::EllipseFilled);

        self.current_tool = tool;

        // Clear tool hover preview when switching tools.
        if !matches!(tool, Tool::Pencil | Tool::Brush | Tool::Erase) {
            self.canvas.set_brush_preview(None);
        }
        // Fonts are loaded centrally via FontLibrary - no per-editor loading needed
    }

    /// Build FontPanelInfo from current font tool state
    fn build_font_panel_info(&self) -> FontPanelInfo {
        let font_name = self.font_tool.with_selected_font(|f| f.name().to_string()).unwrap_or_default();

        let has_fonts = self.font_tool.has_fonts();
        let selected_font_index = self.font_tool.selected_font;

        // Build font names list
        let font_names: Vec<String> = (0..self.font_tool.font_count())
            .filter_map(|i| self.font_tool.font_name(i).map(|n| n.to_string()))
            .collect();

        // Build char availability for preview (chars ! to ~)
        let char_availability: Vec<(char, bool)> = ('!'..='~').map(|ch| (ch, self.font_tool.has_char(ch))).collect();

        // Get current outline style
        let outline_style = { *self.options.read().font_outline_style.read() };

        FontPanelInfo {
            font_name,
            selected_font_index,
            has_fonts,
            font_names,
            char_availability,
            outline_style,
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
