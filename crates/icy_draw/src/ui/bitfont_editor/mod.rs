//! BitFont Editor for icy_draw
//!
//! Provides a pixel-based editor for bitmap fonts (.psf, .fXX, .yaff files).
//! Features:
//! - Glyph selector grid (256 characters)
//! - Pixel edit grid with click/drag drawing
//! - Toolbar with Clear, Inverse, Move, Flip operations
//! - Resize font dimensions
//! - Undo/Redo support
//! - Tool system: Click, Select, Rectangle, Fill
//! - Keyboard cursor navigation
//! - Save as .yaff format (human-readable)

mod undo;

use std::path::PathBuf;

use iced::{
    Color, Element, Length, Point, Rectangle, Size, Task, Theme,
    keyboard::{self, Key},
    mouse::{self, Cursor},
    widget::{
        button, canvas::{self, Canvas, Frame, Path, Stroke, Action}, column, container, row, scrollable, slider, text, Space,
    },
};
use icy_engine::BitFont;

use self::undo::BitFontUndoOperation;

/// Scale factor for pixel cells in the edit grid
const EDIT_CELL_SIZE: f32 = 20.0;
/// Border between cells
const EDIT_CELL_BORDER: f32 = 2.0;
/// Scale factor for glyph preview in selector
const GLYPH_PREVIEW_SCALE: f32 = 3.0;
/// Ruler size
const RULER_SIZE: f32 = 20.0;

// ═══════════════════════════════════════════════════════════════════════════
// Tool System
// ═══════════════════════════════════════════════════════════════════════════

/// Available tools in the BitFont editor
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BitFontTool {
    /// Click tool - draw/erase single pixels, keyboard cursor navigation
    #[default]
    Click,
    /// Selection tool - select rectangular areas
    Select,
    /// Rectangle outline tool
    RectangleOutline,
    /// Filled rectangle tool
    RectangleFilled,
    /// Flood fill tool
    Fill,
}

impl BitFontTool {
    /// Get the display name for this tool
    pub fn name(&self) -> &'static str {
        match self {
            BitFontTool::Click => "Click",
            BitFontTool::Select => "Select",
            BitFontTool::RectangleOutline => "Rectangle",
            BitFontTool::RectangleFilled => "Filled Rect",
            BitFontTool::Fill => "Fill",
        }
    }

    /// Get the icon character for this tool
    pub fn icon(&self) -> &'static str {
        match self {
            BitFontTool::Click => "✎",
            BitFontTool::Select => "▢",
            BitFontTool::RectangleOutline => "□",
            BitFontTool::RectangleFilled => "■",
            BitFontTool::Fill => "◧",
        }
    }

    /// Get keyboard shortcut
    pub fn shortcut(&self) -> char {
        match self {
            BitFontTool::Click => 'C',
            BitFontTool::Select => 'S',
            BitFontTool::RectangleOutline | BitFontTool::RectangleFilled => 'R',
            BitFontTool::Fill => 'F',
        }
    }
}

/// Tool slots for the toolbar (some tools toggle between variants)
pub const BITFONT_TOOL_SLOTS: &[(BitFontTool, Option<BitFontTool>)] = &[
    (BitFontTool::Click, None),
    (BitFontTool::Select, None),
    (BitFontTool::RectangleOutline, Some(BitFontTool::RectangleFilled)),
    (BitFontTool::Fill, None),
];

/// Messages for the BitFont editor
#[derive(Clone, Debug)]
pub enum BitFontEditorMessage {
    /// Select a glyph by character code
    SelectGlyph(char),
    /// Set or clear a pixel at (x, y) - true = set, false = clear
    SetPixel(i32, i32, bool),
    /// Start a pixel edit operation (for undo grouping)
    StartEdit,
    /// End a pixel edit operation
    EndEdit,
    /// Clear the selected glyph (or selection)
    Clear,
    /// Inverse the selected glyph (or selection)
    Inverse,
    /// Move glyph up
    MoveUp,
    /// Move glyph down
    MoveDown,
    /// Move glyph left
    MoveLeft,
    /// Move glyph right
    MoveRight,
    /// Flip glyph horizontally
    FlipX,
    /// Flip glyph vertically
    FlipY,
    /// Set new font width
    SetWidth(i32),
    /// Set new font height
    SetHeight(i32),
    /// Apply resize
    ApplyResize,
    /// Undo last operation
    Undo,
    /// Redo last undone operation
    Redo,
    /// Canvas interaction
    CanvasEvent(CanvasEvent),
    
    // ═══════════════════════════════════════════════════════════════════════
    // Tool & Cursor messages
    // ═══════════════════════════════════════════════════════════════════════
    /// Select a tool
    SelectTool(BitFontTool),
    /// Toggle rectangle fill mode
    ToggleRectFilled,
    /// Move cursor by delta
    MoveCursor(i32, i32),
    /// Toggle pixel at cursor position
    TogglePixelAtCursor,
    /// Set pixel at cursor (true = on, false = off)
    SetPixelAtCursor(bool),
    /// Extend selection with shift+arrows
    ExtendSelection(i32, i32),
    /// Clear current selection
    ClearSelection,
    /// Select all pixels in glyph
    SelectAll,
    /// Fill selection with pixels
    FillSelection,
    /// Erase selection (clear pixels)
    EraseSelection,
    /// Inverse selection pixels
    InverseSelection,
    /// Go to next character (+)
    NextChar,
    /// Go to previous character (-)
    PrevChar,
}

/// Canvas interaction events
#[derive(Clone, Debug)]
pub enum CanvasEvent {
    LeftPressed(Point),
    RightPressed(Point),
    LeftReleased,
    RightReleased,
    CursorMoved(Point),
}

/// State for the BitFont editor
/// 
/// We store our own editable glyph data since BitFont doesn't expose mutable access
/// to glyph pixels directly. When saving, we'll convert back to yaff format.
pub struct BitFontEditor {
    /// The original font (for reference, not directly edited)
    pub font: BitFont,
    /// Editable glyph data: 256 glyphs, each as Vec<Vec<bool>> (height x width)
    glyph_data: Vec<Vec<Vec<bool>>>,
    /// Font width
    font_width: i32,
    /// Font height
    font_height: i32,
    /// Currently selected character
    pub selected_char: char,
    /// File path (if loaded from file)
    pub file_path: Option<PathBuf>,
    /// Whether the font has been modified
    pub is_modified: bool,
    /// Undo stack
    undo_stack: Vec<Box<dyn BitFontUndoOperation>>,
    /// Redo stack
    redo_stack: Vec<Box<dyn BitFontUndoOperation>>,
    /// Old glyph data for current edit operation
    old_edit_data: Option<Vec<Vec<bool>>>,
    /// Target width for resize
    target_width: i32,
    /// Target height for resize
    target_height: i32,
    /// Is left mouse button pressed (for dragging)
    is_left_pressed: bool,
    /// Is right mouse button pressed (for dragging)
    is_right_pressed: bool,
    /// Edit grid canvas cache
    edit_cache: canvas::Cache,
    /// Glyph selector canvas cache
    selector_cache: canvas::Cache,
    
    // ═══════════════════════════════════════════════════════════════════════
    // Tool & Cursor state
    // ═══════════════════════════════════════════════════════════════════════
    /// Current tool
    pub current_tool: BitFontTool,
    /// Cursor position in the edit grid (x, y)
    pub cursor_pos: (i32, i32),
    /// Selection rectangle: (x1, y1, x2, y2) - None if no selection
    pub selection: Option<(i32, i32, i32, i32)>,
    /// Drag start position for shapes/selection
    drag_start: Option<(i32, i32)>,
    /// Whether we're currently extending selection with shift
    is_selecting: bool,
}

impl BitFontEditor {
    /// Create a new BitFont editor with a default font
    pub fn new() -> Self {
        let font = BitFont::default();
        let size = font.size();
        let glyph_data = Self::extract_glyph_data(&font, size.width, size.height);
        
        Self {
            font,
            glyph_data,
            font_width: size.width,
            font_height: size.height,
            selected_char: 'A',
            file_path: None,
            is_modified: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            old_edit_data: None,
            target_width: size.width,
            target_height: size.height,
            is_left_pressed: false,
            is_right_pressed: false,
            edit_cache: canvas::Cache::new(),
            selector_cache: canvas::Cache::new(),
            // New tool & cursor state
            current_tool: BitFontTool::Click,
            cursor_pos: (0, 0),
            selection: None,
            drag_start: None,
            is_selecting: false,
        }
    }

    /// Create a BitFont editor from a file
    pub fn from_file(path: PathBuf) -> Result<Self, String> {
        let data = std::fs::read(&path).map_err(|e| format!("Failed to read file: {}", e))?;
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Font").to_string();
        let font = BitFont::from_bytes(name, &data).map_err(|e| format!("Failed to parse font: {}", e))?;
        let size = font.size();
        let glyph_data = Self::extract_glyph_data(&font, size.width, size.height);
        
        Ok(Self {
            font,
            glyph_data,
            font_width: size.width,
            font_height: size.height,
            selected_char: 'A',
            file_path: Some(path),
            is_modified: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            old_edit_data: None,
            target_width: size.width,
            target_height: size.height,
            is_left_pressed: false,
            is_right_pressed: false,
            edit_cache: canvas::Cache::new(),
            selector_cache: canvas::Cache::new(),
            // New tool & cursor state
            current_tool: BitFontTool::Click,
            cursor_pos: (0, 0),
            selection: None,
            drag_start: None,
            is_selecting: false,
        })
    }

    /// Extract all glyph pixel data from a BitFont
    fn extract_glyph_data(font: &BitFont, width: i32, height: i32) -> Vec<Vec<Vec<bool>>> {
        let mut glyphs = Vec::with_capacity(256);
        
        for ch_code in 0..256u32 {
            let ch = char::from_u32(ch_code).unwrap_or(' ');
            let mut pixels = vec![vec![false; width as usize]; height as usize];
            
            if let Some(glyph) = font.get_glyph(ch) {
                for (y, row) in glyph.bitmap.pixels.iter().enumerate() {
                    if y >= height as usize {
                        break;
                    }
                    for (x, &pixel) in row.iter().enumerate() {
                        if x >= width as usize {
                            break;
                        }
                        pixels[y][x] = pixel;
                    }
                }
            }
            
            glyphs.push(pixels);
        }
        
        glyphs
    }

    /// Get the pixel data for a character
    pub fn get_glyph_pixels(&self, ch: char) -> &Vec<Vec<bool>> {
        let idx = (ch as u32).min(255) as usize;
        &self.glyph_data[idx]
    }

    /// Get mutable pixel data for a character
    fn get_glyph_pixels_mut(&mut self, ch: char) -> &mut Vec<Vec<bool>> {
        let idx = (ch as u32).min(255) as usize;
        &mut self.glyph_data[idx]
    }

    /// Set glyph pixel data
    pub fn set_glyph_pixels(&mut self, ch: char, data: Vec<Vec<bool>>) {
        let idx = (ch as u32).min(255) as usize;
        self.glyph_data[idx] = data;
        self.is_modified = true;
        self.edit_cache.clear();
        self.selector_cache.clear();
    }

    /// Set a single pixel
    fn set_pixel(&mut self, x: i32, y: i32, value: bool) {
        if x < 0 || y < 0 || x >= self.font_width || y >= self.font_height {
            return;
        }

        let pixels = self.get_glyph_pixels_mut(self.selected_char);
        if (y as usize) < pixels.len() {
            let row = &mut pixels[y as usize];
            if (x as usize) < row.len() && row[x as usize] != value {
                row[x as usize] = value;
                self.is_modified = true;
                self.edit_cache.clear();
                self.selector_cache.clear();
            }
        }
    }

    /// Start an edit operation (for undo grouping)
    fn start_edit(&mut self) {
        self.old_edit_data = Some(self.get_glyph_pixels(self.selected_char).clone());
    }

    /// End an edit operation and push to undo stack if changed
    fn end_edit(&mut self) {
        if let Some(old_data) = self.old_edit_data.take() {
            let new_data = self.get_glyph_pixels(self.selected_char).clone();
            if old_data != new_data {
                let op = undo::EditGlyph::new(self.selected_char, old_data, new_data);
                self.undo_stack.push(Box::new(op));
                self.redo_stack.clear();
            }
        }
    }

    /// Push an undo operation and execute its redo
    fn push_undo(&mut self, mut op: Box<dyn BitFontUndoOperation>) {
        op.redo(self);
        self.undo_stack.push(op);
        self.redo_stack.clear();
        self.is_modified = true;
        self.edit_cache.clear();
        self.selector_cache.clear();
    }

    /// Get font size
    pub fn font_size(&self) -> (i32, i32) {
        (self.font_width, self.font_height)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // UndoHandler-like interface
    // ═══════════════════════════════════════════════════════════════════════

    /// Get description of next undo operation
    pub fn undo_description(&self) -> Option<String> {
        self.undo_stack.last().map(|op| op.get_description())
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Perform undo
    pub fn undo(&mut self) {
        if let Some(mut op) = self.undo_stack.pop() {
            op.undo(self);
            self.redo_stack.push(op);
            self.edit_cache.clear();
            self.selector_cache.clear();
        }
    }

    /// Get description of next redo operation
    pub fn redo_description(&self) -> Option<String> {
        self.redo_stack.last().map(|op| op.get_description())
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Perform redo
    pub fn redo(&mut self) {
        if let Some(mut op) = self.redo_stack.pop() {
            op.redo(self);
            self.undo_stack.push(op);
            self.edit_cache.clear();
            self.selector_cache.clear();
        }
    }

    /// Get undo stack length (for modified tracking)
    pub fn undo_stack_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Handle update messages
    pub fn update(&mut self, message: BitFontEditorMessage) -> Task<BitFontEditorMessage> {
        match message {
            BitFontEditorMessage::SelectGlyph(ch) => {
                self.selected_char = ch;
                self.edit_cache.clear();
            }
            BitFontEditorMessage::SetPixel(x, y, value) => {
                self.set_pixel(x, y, value);
            }
            BitFontEditorMessage::StartEdit => {
                self.start_edit();
            }
            BitFontEditorMessage::EndEdit => {
                self.end_edit();
            }
            BitFontEditorMessage::Clear => {
                let op = undo::ClearGlyph::new(self.selected_char);
                self.push_undo(Box::new(op));
            }
            BitFontEditorMessage::Inverse => {
                let op = undo::InverseGlyph::new(self.selected_char);
                self.push_undo(Box::new(op));
            }
            BitFontEditorMessage::MoveUp => {
                let op = undo::MoveGlyph::new(self.selected_char, 0, -1);
                self.push_undo(Box::new(op));
            }
            BitFontEditorMessage::MoveDown => {
                let op = undo::MoveGlyph::new(self.selected_char, 0, 1);
                self.push_undo(Box::new(op));
            }
            BitFontEditorMessage::MoveLeft => {
                let op = undo::MoveGlyph::new(self.selected_char, -1, 0);
                self.push_undo(Box::new(op));
            }
            BitFontEditorMessage::MoveRight => {
                let op = undo::MoveGlyph::new(self.selected_char, 1, 0);
                self.push_undo(Box::new(op));
            }
            BitFontEditorMessage::FlipX => {
                let op = undo::FlipGlyph::new(self.selected_char, true);
                self.push_undo(Box::new(op));
            }
            BitFontEditorMessage::FlipY => {
                let op = undo::FlipGlyph::new(self.selected_char, false);
                self.push_undo(Box::new(op));
            }
            BitFontEditorMessage::SetWidth(w) => {
                self.target_width = w.clamp(1, 16);
            }
            BitFontEditorMessage::SetHeight(h) => {
                self.target_height = h.clamp(1, 32);
            }
            BitFontEditorMessage::ApplyResize => {
                if self.target_width != self.font_width || self.target_height != self.font_height {
                    let op = undo::ResizeFont::new(self.font_width, self.font_height, self.target_width, self.target_height);
                    self.push_undo(Box::new(op));
                }
            }
            BitFontEditorMessage::Undo => {
                self.undo();
            }
            BitFontEditorMessage::Redo => {
                self.redo();
            }
            BitFontEditorMessage::CanvasEvent(event) => {
                self.handle_canvas_event(event);
            }
            
            // ═══════════════════════════════════════════════════════════════
            // Tool & Cursor handling
            // ═══════════════════════════════════════════════════════════════
            BitFontEditorMessage::SelectTool(tool) => {
                self.current_tool = tool;
                self.selection = None;
                self.edit_cache.clear();
            }
            BitFontEditorMessage::ToggleRectFilled => {
                self.current_tool = match self.current_tool {
                    BitFontTool::RectangleOutline => BitFontTool::RectangleFilled,
                    BitFontTool::RectangleFilled => BitFontTool::RectangleOutline,
                    other => other,
                };
            }
            BitFontEditorMessage::MoveCursor(dx, dy) => {
                self.move_cursor(dx, dy);
            }
            BitFontEditorMessage::TogglePixelAtCursor => {
                self.toggle_pixel_at_cursor();
            }
            BitFontEditorMessage::SetPixelAtCursor(value) => {
                self.set_pixel_at_cursor(value);
            }
            BitFontEditorMessage::ExtendSelection(dx, dy) => {
                self.extend_selection(dx, dy);
            }
            BitFontEditorMessage::ClearSelection => {
                self.selection = None;
                self.is_selecting = false;
                self.edit_cache.clear();
            }
            BitFontEditorMessage::SelectAll => {
                self.selection = Some((0, 0, self.font_width - 1, self.font_height - 1));
                self.edit_cache.clear();
            }
            BitFontEditorMessage::FillSelection => {
                self.fill_selection(true);
            }
            BitFontEditorMessage::EraseSelection => {
                self.fill_selection(false);
            }
            BitFontEditorMessage::InverseSelection => {
                self.inverse_selection();
            }
            BitFontEditorMessage::NextChar => {
                let next = ((self.selected_char as u32) + 1).min(255);
                self.selected_char = char::from_u32(next).unwrap_or(self.selected_char);
                self.edit_cache.clear();
            }
            BitFontEditorMessage::PrevChar => {
                let prev = (self.selected_char as u32).saturating_sub(1);
                self.selected_char = char::from_u32(prev).unwrap_or(self.selected_char);
                self.edit_cache.clear();
            }
        }
        Task::none()
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Cursor & Selection helpers
    // ═══════════════════════════════════════════════════════════════════════

    /// Move cursor by delta, clamping to bounds
    fn move_cursor(&mut self, dx: i32, dy: i32) {
        let (x, y) = self.cursor_pos;
        let new_x = (x + dx).clamp(0, self.font_width - 1);
        let new_y = (y + dy).clamp(0, self.font_height - 1);
        self.cursor_pos = (new_x, new_y);
        self.edit_cache.clear();
    }

    /// Toggle pixel at current cursor position
    fn toggle_pixel_at_cursor(&mut self) {
        let (x, y) = self.cursor_pos;
        let current = self.get_glyph_pixels(self.selected_char)
            .get(y as usize)
            .and_then(|row| row.get(x as usize))
            .copied()
            .unwrap_or(false);
        self.start_edit();
        self.set_pixel(x, y, !current);
        self.end_edit();
    }

    /// Set pixel at cursor to specific value
    fn set_pixel_at_cursor(&mut self, value: bool) {
        let (x, y) = self.cursor_pos;
        self.start_edit();
        self.set_pixel(x, y, value);
        self.end_edit();
    }

    /// Extend selection from cursor with shift+arrows
    fn extend_selection(&mut self, dx: i32, dy: i32) {
        if !self.is_selecting {
            // Start new selection from cursor
            self.is_selecting = true;
            let (x, y) = self.cursor_pos;
            self.selection = Some((x, y, x, y));
        }

        // Move cursor
        self.move_cursor(dx, dy);

        // Extend selection to include new cursor position
        if let Some((x1, y1, _, _)) = self.selection {
            let (cx, cy) = self.cursor_pos;
            self.selection = Some((x1, y1, cx, cy));
        }
        self.edit_cache.clear();
    }

    /// Fill selection (or whole glyph if no selection) with value
    fn fill_selection(&mut self, value: bool) {
        self.start_edit();
        let (x1, y1, x2, y2) = self.selection.unwrap_or((0, 0, self.font_width - 1, self.font_height - 1));
        let (min_x, max_x) = (x1.min(x2), x1.max(x2));
        let (min_y, max_y) = (y1.min(y2), y1.max(y2));
        
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                self.set_pixel(x, y, value);
            }
        }
        self.end_edit();
    }

    /// Inverse pixels in selection (or whole glyph)
    fn inverse_selection(&mut self) {
        self.start_edit();
        let (x1, y1, x2, y2) = self.selection.unwrap_or((0, 0, self.font_width - 1, self.font_height - 1));
        let (min_x, max_x) = (x1.min(x2), x1.max(x2));
        let (min_y, max_y) = (y1.min(y2), y1.max(y2));
        
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let current = self.get_glyph_pixels(self.selected_char)
                    .get(y as usize)
                    .and_then(|row| row.get(x as usize))
                    .copied()
                    .unwrap_or(false);
                self.set_pixel(x, y, !current);
            }
        }
        self.end_edit();
    }

    /// Handle canvas interaction events
    fn handle_canvas_event(&mut self, event: CanvasEvent) {
        match event {
            CanvasEvent::LeftPressed(pos) => {
                self.is_left_pressed = true;
                self.start_edit();
                if let Some((x, y)) = self.pos_to_pixel(pos) {
                    self.set_pixel(x, y, true);
                }
            }
            CanvasEvent::RightPressed(pos) => {
                self.is_right_pressed = true;
                self.start_edit();
                if let Some((x, y)) = self.pos_to_pixel(pos) {
                    self.set_pixel(x, y, false);
                }
            }
            CanvasEvent::LeftReleased => {
                if self.is_left_pressed {
                    self.is_left_pressed = false;
                    self.end_edit();
                }
            }
            CanvasEvent::RightReleased => {
                if self.is_right_pressed {
                    self.is_right_pressed = false;
                    self.end_edit();
                }
            }
            CanvasEvent::CursorMoved(pos) => {
                if let Some((x, y)) = self.pos_to_pixel(pos) {
                    if self.is_left_pressed {
                        self.set_pixel(x, y, true);
                    } else if self.is_right_pressed {
                        self.set_pixel(x, y, false);
                    }
                }
            }
        }
    }

    /// Convert canvas position to pixel coordinates
    fn pos_to_pixel(&self, pos: Point) -> Option<(i32, i32)> {
        let x = ((pos.x - RULER_SIZE) / (EDIT_CELL_SIZE + EDIT_CELL_BORDER)) as i32;
        let y = ((pos.y - RULER_SIZE) / (EDIT_CELL_SIZE + EDIT_CELL_BORDER)) as i32;
        
        if x >= 0 && x < self.font_width && y >= 0 && y < self.font_height {
            Some((x, y))
        } else {
            None
        }
    }

    /// Build the editor view
    pub fn view(&self) -> Element<'_, BitFontEditorMessage> {
        // Tool panel - simple row of tool buttons
        let tool_panel = row![
            self.tool_button(BitFontTool::Click),
            self.tool_button(BitFontTool::Select),
            self.tool_button_rect(),
            self.tool_button(BitFontTool::Fill),
        ]
        .spacing(4)
        .padding(4);

        // Edit grid canvas
        let edit_grid_width = RULER_SIZE + (EDIT_CELL_SIZE + EDIT_CELL_BORDER) * self.font_width as f32;
        let edit_grid_height = RULER_SIZE + (EDIT_CELL_SIZE + EDIT_CELL_BORDER) * self.font_height as f32;
        
        let edit_canvas = Canvas::new(EditGridCanvas {
            editor: self,
        })
        .width(Length::Fixed(edit_grid_width))
        .height(Length::Fixed(edit_grid_height));

        // Toolbar buttons
        let toolbar = column![
            row![
                button("Clear").on_press(BitFontEditorMessage::Clear),
                button("Inverse").on_press(BitFontEditorMessage::Inverse),
            ].spacing(4),
            row![
                Space::new().width(Length::Fill),
                button("⬆").on_press(BitFontEditorMessage::MoveUp),
                Space::new().width(Length::Fill),
            ],
            row![
                button("⬅").on_press(BitFontEditorMessage::MoveLeft),
                button("➡").on_press(BitFontEditorMessage::MoveRight),
            ].spacing(4),
            row![
                Space::new().width(Length::Fill),
                button("⬇").on_press(BitFontEditorMessage::MoveDown),
                Space::new().width(Length::Fill),
            ],
            row![
                button("Flip X").on_press(BitFontEditorMessage::FlipX),
                button("Flip Y").on_press(BitFontEditorMessage::FlipY),
            ].spacing(4),
            // Resize controls
            text("Resize:").size(14),
            row![
                text("W:").size(12),
                slider(1..=16, self.target_width, BitFontEditorMessage::SetWidth).width(80),
                text(format!("{}", self.target_width)).size(12),
            ].spacing(4),
            row![
                text("H:").size(12),
                slider(1..=32, self.target_height, BitFontEditorMessage::SetHeight).width(80),
                text(format!("{}", self.target_height)).size(12),
            ].spacing(4),
            if self.target_width != self.font_width || self.target_height != self.font_height {
                button("Apply Resize").on_press(BitFontEditorMessage::ApplyResize)
            } else {
                button("Apply Resize")
            },
        ]
        .spacing(8)
        .padding(8);

        // Cursor position and tool info
        let cursor_info = text(format!(
            "Cursor: ({}, {}) | Tool: {} | {}",
            self.cursor_pos.0 + 1,
            self.cursor_pos.1 + 1,
            self.current_tool.name(),
            if self.selection.is_some() { "Selection active" } else { "" }
        ))
        .size(12);

        // Top section: tool panel + edit grid + toolbar
        let edit_section = row![
            column![
                tool_panel,
                container(edit_canvas).padding(8),
            ],
            container(toolbar).padding(8),
        ]
        .spacing(16);

        // Glyph selector (256 glyphs in a scrollable grid)
        let glyph_selector = self.view_glyph_selector();

        // Main layout
        let content = column![
            container(edit_section).center_x(Length::Fill),
            cursor_info,
            text(format!("Character: {} (0x{:02X}) | +/- to navigate", self.selected_char, self.selected_char as u32)).size(14),
            container(glyph_selector).height(Length::Fill),
        ]
        .spacing(8)
        .padding(16);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Create a tool button
    fn tool_button(&self, tool: BitFontTool) -> Element<'_, BitFontEditorMessage> {
        let is_selected = self.current_tool == tool;
        let label = format!("{} {}", tool.icon(), tool.shortcut());
        
        let btn = button(text(label).size(14))
            .padding([4, 8])
            .on_press(BitFontEditorMessage::SelectTool(tool));
        
        if is_selected {
            btn.style(|theme: &Theme, status| {
                let palette = theme.extended_palette();
                let mut style = button::primary(theme, status);
                style.background = Some(palette.primary.strong.color.into());
                style
            }).into()
        } else {
            btn.into()
        }
    }

    /// Create rectangle tool button (toggles between outline/filled)
    fn tool_button_rect(&self) -> Element<'_, BitFontEditorMessage> {
        let is_outline = self.current_tool == BitFontTool::RectangleOutline;
        let is_filled = self.current_tool == BitFontTool::RectangleFilled;
        let is_selected = is_outline || is_filled;
        
        let icon = if is_filled { "■" } else { "□" };
        let label = format!("{} R", icon);
        
        let msg = if is_selected {
            BitFontEditorMessage::ToggleRectFilled
        } else {
            BitFontEditorMessage::SelectTool(BitFontTool::RectangleOutline)
        };
        
        let btn = button(text(label).size(14))
            .padding([4, 8])
            .on_press(msg);
        
        if is_selected {
            btn.style(|theme: &Theme, status| {
                let palette = theme.extended_palette();
                let mut style = button::primary(theme, status);
                style.background = Some(palette.primary.strong.color.into());
                style
            }).into()
        } else {
            btn.into()
        }
    }

    /// Build the glyph selector view
    fn view_glyph_selector(&self) -> Element<'_, BitFontEditorMessage> {
        let glyph_width = GLYPH_PREVIEW_SCALE * self.font_width as f32;
        let glyph_height = GLYPH_PREVIEW_SCALE * self.font_height as f32;

        // Create 16 rows of 16 glyphs each
        let mut rows: Vec<Element<'_, BitFontEditorMessage>> = Vec::new();
        
        for row_idx in 0..16 {
            let mut row_items: Vec<Element<'_, BitFontEditorMessage>> = Vec::new();
            
            for col_idx in 0..16 {
                let ch_code = row_idx * 16 + col_idx;
                let ch = char::from_u32(ch_code as u32).unwrap_or(' ');
                
                let is_selected = ch == self.selected_char;
                
                let glyph_canvas = Canvas::new(GlyphPreviewCanvas {
                    editor: self,
                    ch,
                    is_selected,
                })
                .width(Length::Fixed(glyph_width + 4.0))
                .height(Length::Fixed(glyph_height + 4.0));
                
                let glyph_button = button(glyph_canvas)
                    .padding(0)
                    .on_press(BitFontEditorMessage::SelectGlyph(ch));
                
                row_items.push(glyph_button.into());
            }
            
            rows.push(row(row_items).spacing(2).into());
        }

        scrollable(column(rows).spacing(2))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Get status information for the status bar
    pub fn status_info(&self) -> (String, String, String) {
        (
            format!("Char: {} (0x{:02X})", self.selected_char, self.selected_char as u32),
            format!("{}×{}", self.font_width, self.font_height),
            format!(
                "Undo: {} Redo: {}",
                self.undo_stack.len(),
                self.redo_stack.len()
            ),
        )
    }

    /// Resize all glyphs to new dimensions
    pub fn resize_glyphs(&mut self, new_width: i32, new_height: i32) {
        for glyph in &mut self.glyph_data {
            let mut new_pixels = vec![vec![false; new_width as usize]; new_height as usize];
            
            for (y, row) in glyph.iter().enumerate() {
                if y >= new_height as usize {
                    break;
                }
                for (x, &pixel) in row.iter().enumerate() {
                    if x >= new_width as usize {
                        break;
                    }
                    new_pixels[y][x] = pixel;
                }
            }
            
            *glyph = new_pixels;
        }
        
        self.font_width = new_width;
        self.font_height = new_height;
        self.target_width = new_width;
        self.target_height = new_height;
        self.is_modified = true;
        self.edit_cache.clear();
        self.selector_cache.clear();
    }
}

/// Canvas for the pixel edit grid
struct EditGridCanvas<'a> {
    editor: &'a BitFontEditor,
}

impl<'a> canvas::Program<BitFontEditorMessage> for EditGridCanvas<'a> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<canvas::Geometry> {
        let geometry = self.editor.edit_cache.draw(renderer, bounds.size(), |frame| {
            let (width, height) = self.editor.font_size();
            let pixels = self.editor.get_glyph_pixels(self.editor.selected_char);

            // Background
            frame.fill_rectangle(
                Point::ORIGIN,
                frame.size(),
                Color::from_rgb(0.15, 0.15, 0.15),
            );

            // Draw rulers
            for x in 0..width {
                let text_pos = Point::new(
                    RULER_SIZE + (x as f32 + 0.5) * (EDIT_CELL_SIZE + EDIT_CELL_BORDER),
                    RULER_SIZE / 2.0,
                );
                frame.fill_text(canvas::Text {
                    content: format!("{}", x + 1),
                    position: text_pos,
                    color: Color::from_rgb(0.7, 0.7, 0.7),
                    size: iced::Pixels(12.0),
                    ..Default::default()
                });
            }

            for y in 0..height {
                let text_pos = Point::new(
                    RULER_SIZE / 2.0,
                    RULER_SIZE + (y as f32 + 0.5) * (EDIT_CELL_SIZE + EDIT_CELL_BORDER),
                );
                frame.fill_text(canvas::Text {
                    content: format!("{}", y + 1),
                    position: text_pos,
                    color: Color::from_rgb(0.7, 0.7, 0.7),
                    size: iced::Pixels(12.0),
                    ..Default::default()
                });
            }

            // Draw pixel grid
            for y in 0..height as usize {
                for x in 0..width as usize {
                    let cell_x = RULER_SIZE + x as f32 * (EDIT_CELL_SIZE + EDIT_CELL_BORDER);
                    let cell_y = RULER_SIZE + y as f32 * (EDIT_CELL_SIZE + EDIT_CELL_BORDER);

                    let is_set = pixels.get(y).and_then(|row| row.get(x)).copied().unwrap_or(false);
                    
                    let color = if is_set {
                        Color::from_rgb(0.8, 0.8, 0.8)
                    } else {
                        Color::from_rgb(0.1, 0.1, 0.1)
                    };

                    frame.fill_rectangle(
                        Point::new(cell_x, cell_y),
                        Size::new(EDIT_CELL_SIZE, EDIT_CELL_SIZE),
                        color,
                    );
                }
            }

            // Draw selection highlight
            if let Some((x1, y1, x2, y2)) = self.editor.selection {
                let (min_x, max_x) = (x1.min(x2), x1.max(x2));
                let (min_y, max_y) = (y1.min(y2), y1.max(y2));
                
                let sel_x = RULER_SIZE + min_x as f32 * (EDIT_CELL_SIZE + EDIT_CELL_BORDER) - 1.0;
                let sel_y = RULER_SIZE + min_y as f32 * (EDIT_CELL_SIZE + EDIT_CELL_BORDER) - 1.0;
                let sel_w = (max_x - min_x + 1) as f32 * (EDIT_CELL_SIZE + EDIT_CELL_BORDER) + 2.0;
                let sel_h = (max_y - min_y + 1) as f32 * (EDIT_CELL_SIZE + EDIT_CELL_BORDER) + 2.0;
                
                let selection_path = Path::rectangle(
                    Point::new(sel_x, sel_y),
                    Size::new(sel_w, sel_h),
                );
                frame.stroke(
                    &selection_path,
                    Stroke::default()
                        .with_color(Color::from_rgb(0.2, 0.6, 1.0))
                        .with_width(2.0),
                );
            }

            // Draw cursor
            let (cx, cy) = self.editor.cursor_pos;
            let cursor_x = RULER_SIZE + cx as f32 * (EDIT_CELL_SIZE + EDIT_CELL_BORDER) - 2.0;
            let cursor_y = RULER_SIZE + cy as f32 * (EDIT_CELL_SIZE + EDIT_CELL_BORDER) - 2.0;
            let cursor_path = Path::rectangle(
                Point::new(cursor_x, cursor_y),
                Size::new(EDIT_CELL_SIZE + 4.0, EDIT_CELL_SIZE + 4.0),
            );
            frame.stroke(
                &cursor_path,
                Stroke::default()
                    .with_color(Color::from_rgb(0.0, 1.0, 1.0))
                    .with_width(2.0),
            );
        });

        vec![geometry]
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: Cursor,
    ) -> Option<Action<BitFontEditorMessage>> {
        // Handle keyboard events
        if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event {
            match key {
                // Arrow keys - move cursor or extend selection
                Key::Named(keyboard::key::Named::ArrowUp) => {
                    if modifiers.shift() {
                        return Some(Action::publish(BitFontEditorMessage::ExtendSelection(0, -1)));
                    } else {
                        return Some(Action::publish(BitFontEditorMessage::MoveCursor(0, -1)));
                    }
                }
                Key::Named(keyboard::key::Named::ArrowDown) => {
                    if modifiers.shift() {
                        return Some(Action::publish(BitFontEditorMessage::ExtendSelection(0, 1)));
                    } else {
                        return Some(Action::publish(BitFontEditorMessage::MoveCursor(0, 1)));
                    }
                }
                Key::Named(keyboard::key::Named::ArrowLeft) => {
                    if modifiers.shift() {
                        return Some(Action::publish(BitFontEditorMessage::ExtendSelection(-1, 0)));
                    } else {
                        return Some(Action::publish(BitFontEditorMessage::MoveCursor(-1, 0)));
                    }
                }
                Key::Named(keyboard::key::Named::ArrowRight) => {
                    if modifiers.shift() {
                        return Some(Action::publish(BitFontEditorMessage::ExtendSelection(1, 0)));
                    } else {
                        return Some(Action::publish(BitFontEditorMessage::MoveCursor(1, 0)));
                    }
                }
                // Space/Enter - toggle pixel at cursor
                Key::Named(keyboard::key::Named::Space) | Key::Named(keyboard::key::Named::Enter) => {
                    return Some(Action::publish(BitFontEditorMessage::TogglePixelAtCursor));
                }
                // Escape - clear selection
                Key::Named(keyboard::key::Named::Escape) => {
                    return Some(Action::publish(BitFontEditorMessage::ClearSelection));
                }
                // Plus/Minus - next/prev character
                Key::Character(c) if c.as_str() == "+" || c.as_str() == "=" => {
                    return Some(Action::publish(BitFontEditorMessage::NextChar));
                }
                Key::Character(c) if c.as_str() == "-" => {
                    return Some(Action::publish(BitFontEditorMessage::PrevChar));
                }
                _ => {}
            }
        }

        // Handle mouse events
        let cursor_pos = cursor.position_in(bounds)?;

        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                Some(Action::publish(BitFontEditorMessage::CanvasEvent(CanvasEvent::LeftPressed(cursor_pos))))
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                Some(Action::publish(BitFontEditorMessage::CanvasEvent(CanvasEvent::RightPressed(cursor_pos))))
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                Some(Action::publish(BitFontEditorMessage::CanvasEvent(CanvasEvent::LeftReleased)))
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right)) => {
                Some(Action::publish(BitFontEditorMessage::CanvasEvent(CanvasEvent::RightReleased)))
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                Some(Action::publish(BitFontEditorMessage::CanvasEvent(CanvasEvent::CursorMoved(cursor_pos))))
            }
            _ => None,
        }
    }
}

/// Canvas for a single glyph preview in the selector
struct GlyphPreviewCanvas<'a> {
    editor: &'a BitFontEditor,
    ch: char,
    is_selected: bool,
}

impl<'a> canvas::Program<BitFontEditorMessage> for GlyphPreviewCanvas<'a> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        
        let (width, height) = self.editor.font_size();
        let pixels = self.editor.get_glyph_pixels(self.ch);

        // Background
        let bg_color = if self.is_selected {
            Color::from_rgb(0.3, 0.3, 0.5)
        } else {
            Color::from_rgb(0.1, 0.1, 0.1)
        };
        frame.fill_rectangle(Point::ORIGIN, frame.size(), bg_color);

        // Draw pixels
        let fg_color = if self.is_selected {
            Color::from_rgb(1.0, 1.0, 0.5)
        } else {
            Color::from_rgb(0.7, 0.7, 0.7)
        };

        for y in 0..height as usize {
            for x in 0..width as usize {
                let is_set = pixels.get(y).and_then(|row| row.get(x)).copied().unwrap_or(false);
                if is_set {
                    frame.fill_rectangle(
                        Point::new(
                            2.0 + x as f32 * GLYPH_PREVIEW_SCALE,
                            2.0 + y as f32 * GLYPH_PREVIEW_SCALE,
                        ),
                        Size::new(GLYPH_PREVIEW_SCALE, GLYPH_PREVIEW_SCALE),
                        fg_color,
                    );
                }
            }
        }

        // Border for selected
        if self.is_selected {
            let border = Path::rectangle(Point::ORIGIN, frame.size());
            frame.stroke(&border, Stroke::default().with_color(Color::from_rgb(1.0, 1.0, 0.0)).with_width(2.0));
        }

        vec![frame.into_geometry()]
    }
}
