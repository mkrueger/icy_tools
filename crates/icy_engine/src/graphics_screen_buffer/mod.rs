use icy_parser_core::{RipCommand, SkypixCommand};

pub mod skypix_impl;

use libyaff::GlyphDefinition;

use crate::{
    AttributedChar, BitFont, BufferType, Caret, DOS_DEFAULT_PALETTE, EditableScreen, EngineResult, GraphicsType, HyperLink, IceMode, Line, Palette, Position,
    Rectangle, RenderOptions, SaveOptions, SavedCaretState, Screen, ScrollbackBuffer, Selection, SelectionMask, Size, TerminalResolutionExt, TerminalState,
    TextPane,
    bgi::{Bgi, DEFAULT_BITFONT, MouseField},
    igs,
    rip_impl::{RIP_FONT, RIP_SCREEN_SIZE},
};
use skypix_impl::SKYPIX_SCREEN_SIZE;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct GraphicsScreenBuffer {
    pub pixel_size: Size,
    pub screen: Vec<u8>,
    pub char_screen_size: Size,

    // Rendering properties
    font_table: HashMap<usize, BitFont>,
    palette: Palette,
    caret: Caret,
    ice_mode: IceMode,
    terminal_state: TerminalState,
    buffer_type: BufferType,
    hyperlinks: Vec<HyperLink>,
    selection_mask: SelectionMask,

    // Font dimensions in pixels
    mouse_fields: Vec<MouseField>,

    // BGI graphics handler
    pub bgi: Bgi,

    // IGS state (only used for IGS graphics)
    _igs_state: Option<igs::vdi_paint::VdiPaint>,

    // Dirty tracking for rendering optimization
    buffer_dirty: std::sync::atomic::AtomicBool,
    buffer_version: std::sync::atomic::AtomicU64,

    saved_pos: Position,
    saved_cursor_state: SavedCaretState,
    graphics_type: GraphicsType,

    // Scan lines for Atari ST Medium resolution (doubles line height)
    pub scan_lines: bool,
    pub scrollback_buffer: Arc<Mutex<Box<dyn Screen>>>,
}

impl GraphicsScreenBuffer {
    /// Creates a new PaletteScreenBuffer with pixel dimensions
    /// px_width, px_height: pixel dimensions (e.g., 640x350 for RIP graphics)
    pub fn new(graphics_type: GraphicsType) -> Self {
        let (px_width, px_height) = match graphics_type {
            GraphicsType::Text => {
                panic!()
            }
            GraphicsType::Rip => (RIP_SCREEN_SIZE.width, RIP_SCREEN_SIZE.height),
            GraphicsType::IGS(term_res) => {
                let res = term_res.get_resolution();
                (res.width, res.height)
            }
            GraphicsType::Skypix => (SKYPIX_SCREEN_SIZE.width, SKYPIX_SCREEN_SIZE.height),
        };

        let mut font_table: HashMap<usize, BitFont> = HashMap::new();
        match graphics_type {
            GraphicsType::Rip => {
                font_table.insert(0, RIP_FONT.clone());
                font_table.insert(1, crate::EGA_7x8.clone());
                font_table.insert(2, crate::VGA_8x14.clone());
                font_table.insert(3, crate::VGA_7x14.clone());
                font_table.insert(4, crate::VGA_16x14.clone());
            }
            GraphicsType::IGS(_) => {
                font_table.insert(0, igs::ATARI_ST_FONT_8x8.clone());
            }
            GraphicsType::Skypix => {
                font_table.insert(0, RIP_FONT.clone());
            }
            GraphicsType::Text => unreachable!(),
        };

        let font = font_table.get(&0).unwrap();
        // Calculate character grid dimensions from pixel size
        let char_cols = px_width / font.size().width;
        let char_rows = px_height / font.size().height;

        // Allocate pixel buffer and fill with background color (0)
        let screen = vec![0u8; px_width as usize * px_height as usize];

        // Set appropriate default palette based on graphics type
        let palette = match graphics_type {
            GraphicsType::IGS(res) => res.get_palette().clone(),
            _ => Palette::from_slice(&DOS_DEFAULT_PALETTE),
        };

        let mut terminal_state = TerminalState::from(Size::new(char_cols, char_rows));

        // Set appropriate default caret colors based on graphics type
        let mut caret = Caret::default();
        caret.use_pixel_positioning = true;
        match graphics_type {
            GraphicsType::IGS(_) => {
                caret.attribute.set_foreground(1);
                caret.attribute.set_background(0);
                terminal_state.cr_is_if = true;
            }
            _ => {
                // Standard VGA: 0=Black, 7=White
                // Keep default (foreground=7, background=0)
            }
        }

        let scan_lines = match graphics_type {
            GraphicsType::IGS(term_res) => term_res.use_scanlines(),
            GraphicsType::Skypix => true,
            _ => false,
        };

        Self {
            pixel_size: Size::new(px_width, px_height),        // Store character dimensions
            char_screen_size: Size::new(char_cols, char_rows), // Store pixel dimensions
            screen,
            font_table,
            palette,
            caret,
            ice_mode: IceMode::Unlimited,
            terminal_state,
            buffer_type: BufferType::CP437,
            hyperlinks: Vec::new(),
            selection_mask: SelectionMask::default(),
            mouse_fields: Vec::new(),
            bgi: Bgi::new(PathBuf::new(), Size::new(px_width, px_height)),
            _igs_state: None,
            buffer_dirty: std::sync::atomic::AtomicBool::new(true),
            buffer_version: std::sync::atomic::AtomicU64::new(0),
            saved_pos: Position::default(),
            saved_cursor_state: SavedCaretState::default(),
            graphics_type,
            scan_lines,
            scrollback_buffer: Arc::new(Mutex::new(Box::new(ScrollbackBuffer::new()))),
        }
    }

    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Render a character directly to the RGBA buffer
    fn render_char_to_buffer(&mut self, pos: Position, ch: AttributedChar) -> Option<GlyphDefinition> {
        let pixel_x = pos.x;
        let pixel_y = pos.y;

        // Get colors from palette
        let mut fg_color = ch.attribute.get_foreground() as u32;
        if ch.attribute.is_bold() && fg_color < 8 {
            fg_color += 8;
        }

        let bg_color = ch.attribute.get_background() as u32; // Apply color limit

        let font = if let Some(font) = self.get_font(ch.get_font_page()) {
            font
        } else if let Some(font) = self.get_font(0) {
            font
        } else {
            &DEFAULT_BITFONT
        };
        let font_size = font.size();

        // Get glyph data from font
        let Some(glyph) = font.get_glyph(ch.ch) else {
            log::error!("NO GLYPH for char '{}'", ch.ch);
            return None;
        };

        let fill_width = if font.yaff_font.spacing == Some(libyaff::FontSpacing::Proportional) {
            // For proportional fonts, use the actual advance width
            let glyph_width = if glyph.bitmap.pixels.is_empty() { 0 } else { glyph.bitmap.pixels[0].len() } as i32;
            let left_bearing = glyph.left_bearing.unwrap_or(0);
            let right_bearing = glyph.right_bearing.unwrap_or(0);
            // Special handling for empty glyphs (like space)
            if glyph_width == 0 && left_bearing > 0 {
                left_bearing
            } else {
                left_bearing + glyph_width + right_bearing
            }
        } else {
            font_size.width as i32
        };

        // Render the character (always fill font_size area with background/foreground)
        for row in 0..font_size.height {
            for col in 0..fill_width {
                let px = pixel_x + col;
                let py = pixel_y + row;

                if px >= self.pixel_size.width || py >= self.pixel_size.height {
                    continue;
                }

                // Check if pixel is set in font glyph
                let is_foreground = if row < glyph.bitmap.pixels.len() as i32 && col < glyph.bitmap.pixels[row as usize].len() as i32 {
                    glyph.bitmap.pixels[row as usize][col as usize]
                } else {
                    false
                };

                let color = if is_foreground { fg_color } else { bg_color };

                // Write to RGBA buffer
                let offset = py * self.pixel_size.width + px;
                self.screen[offset as usize] = color as u8;
            }
        }
        Some(glyph)
    }

    pub fn get_pixel_dimensions(&self) -> (usize, usize) {
        (self.char_screen_size.width as usize, self.char_screen_size.height as usize)
    }
}

impl TextPane for GraphicsScreenBuffer {
    fn get_char(&self, _pos: Position) -> AttributedChar {
        // won't work for rgba screens.
        AttributedChar::default()
    }

    fn get_line_count(&self) -> i32 {
        self.char_screen_size.height
    }

    fn get_width(&self) -> i32 {
        self.char_screen_size.width
    }

    fn get_height(&self) -> i32 {
        self.char_screen_size.height
    }

    fn get_size(&self) -> Size {
        self.char_screen_size
    }

    fn get_line_length(&self, _line: i32) -> i32 {
        // won't work for rgba screens.
        0
    }

    fn get_rectangle(&self) -> crate::Rectangle {
        crate::Rectangle::from_coords(0, 0, self.char_screen_size.width - 1, self.char_screen_size.height - 1)
    }
}

impl Screen for GraphicsScreenBuffer {
    fn ice_mode(&self) -> IceMode {
        self.ice_mode
    }

    fn terminal_state(&self) -> &TerminalState {
        &self.terminal_state
    }

    /// Override: Convert pixel coordinates to character grid coordinates
    fn caret_position(&self) -> Position {
        let pixel_pos = self.caret.position();
        let font_size = self.get_font_dimensions();
        Position::new(pixel_pos.x / font_size.width, pixel_pos.y / font_size.height)
    }

    fn graphics_type(&self) -> crate::GraphicsType {
        self.graphics_type
    }

    fn scan_lines(&self) -> bool {
        self.scan_lines
    }

    fn render_region_to_rgba(&self, px_region: Rectangle, _options: &RenderOptions) -> (Size, Vec<u8>) {
        let pal = self.palette().clone();

        // Clamp region to screen bounds
        let x = px_region.start.x.max(0).min(self.pixel_size.width);
        let y = px_region.start.y.max(0).min(self.pixel_size.height);
        let width = px_region.size.width.min(self.pixel_size.width - x);
        let height = px_region.size.height.min(self.pixel_size.height - y);

        if width <= 0 || height <= 0 {
            return (Size::new(0, 0), Vec::new());
        }

        if self.scan_lines {
            // Double the height for scan lines
            let doubled_height = height * 2;
            let mut pixels = Vec::with_capacity(width as usize * doubled_height as usize * 4);

            for py in 0..height {
                let src_y = y + py;
                let row_start = (src_y * self.pixel_size.width + x) as usize;

                // Render the line once
                let start_pos = pixels.len();
                for px in 0..width {
                    let idx = row_start + px as usize;
                    let (r, g, b) = pal.get_rgb(self.screen[idx] as u32);
                    pixels.push(r);
                    pixels.push(g);
                    pixels.push(b);
                    pixels.push(255);
                }

                // Copy the rendered line for scanline effect
                let end_pos = pixels.len();
                pixels.extend_from_within(start_pos..end_pos);
            }

            (Size::new(width, doubled_height), pixels)
        } else {
            // Standard rendering without scan lines
            let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);

            for py in 0..height {
                let src_y = y + py;
                let row_start = (src_y * self.pixel_size.width + x) as usize;

                for px in 0..width {
                    let idx = row_start + px as usize;
                    let (r, g, b) = pal.get_rgb(self.screen[idx] as u32);
                    pixels.push(r);
                    pixels.push(g);
                    pixels.push(b);
                    pixels.push(255);
                }
            }

            (Size::new(width, height), pixels)
        }
    }

    fn render_to_rgba(&self, _options: &RenderOptions) -> (Size, Vec<u8>) {
        let pal = self.palette().clone();

        if self.scan_lines {
            // Double the height for scan lines (Atari ST Medium resolution)
            let doubled_height = self.pixel_size.height * 2;
            let mut pixels = Vec::with_capacity(self.pixel_size.width as usize * doubled_height as usize * 4);

            for y in 0..self.pixel_size.height {
                let row_start = (y * self.pixel_size.width) as usize;
                let row_end = row_start + self.pixel_size.width as usize;

                // Render the line once
                let start_pos = pixels.len();
                for i in row_start..row_end {
                    let (r, g, b) = pal.get_rgb(self.screen[i] as u32);
                    pixels.push(r);
                    pixels.push(g);
                    pixels.push(b);
                    pixels.push(255);
                }

                // Copy the rendered line
                let end_pos = pixels.len();
                pixels.extend_from_within(start_pos..end_pos);
            }

            (Size::new(self.pixel_size.width, doubled_height), pixels)
        } else {
            // Standard rendering without scan lines
            let mut pixels = Vec::with_capacity(self.pixel_size.width as usize * self.pixel_size.height as usize * 4);
            for i in &self.screen {
                let (r, g, b) = pal.get_rgb(*i as u32);
                pixels.push(r);
                pixels.push(g);
                pixels.push(b);
                pixels.push(255);
            }
            (self.pixel_size, pixels)
        }
    }

    fn get_font_dimensions(&self) -> Size {
        if let Some(font) = self.get_font(self.caret.font_page()) {
            font.size()
        } else if let Some(font) = self.get_font(0) {
            font.size()
        } else {
            Size::new(8, 16)
        }
    }

    fn get_font(&self, font_number: usize) -> Option<&BitFont> {
        self.font_table.get(&font_number)
    }

    fn font_count(&self) -> usize {
        self.font_table.len()
    }

    fn caret(&self) -> &Caret {
        &self.caret
    }

    fn palette(&self) -> &Palette {
        &self.palette
    }

    fn buffer_type(&self) -> BufferType {
        self.buffer_type
    }

    fn get_selection(&self) -> Option<Selection> {
        None
    }

    fn selection_mask(&self) -> &SelectionMask {
        &self.selection_mask
    }

    fn hyperlinks(&self) -> &Vec<HyperLink> {
        &self.hyperlinks
    }

    fn get_version(&self) -> u64 {
        self.buffer_version.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn default_foreground_color(&self) -> u32 {
        self.graphics_type.default_fg_color()
    }

    fn max_base_colors(&self) -> u32 {
        if let GraphicsType::IGS(t) = self.graphics_type {
            t.get_max_colors()
        } else {
            self.palette.len() as u32
        }
    }

    fn get_resolution(&self) -> Size {
        self.pixel_size
    }

    fn screen(&self) -> &[u8] {
        &self.screen
    }

    fn set_scrollback_buffer_size(&mut self, buffer_size: usize) {
        if let Ok(mut sb) = self.scrollback_buffer.lock() {
            if let Some(scrollback) = sb.as_any_mut().downcast_mut::<ScrollbackBuffer>() {
                scrollback.set_buffer_size(buffer_size);
            }
        }
    }

    fn set_selection(&mut self, _selection: Selection) -> EngineResult<()> {
        Ok(())
    }

    fn clear_selection(&mut self) -> EngineResult<()> {
        Ok(())
    }

    fn mouse_fields(&self) -> &Vec<MouseField> {
        &self.mouse_fields
    }

    fn to_bytes(&mut self, _file_name: &str, _options: &SaveOptions) -> EngineResult<Vec<u8>> {
        // Return empty for now, could implement PNG export later
        Ok(Vec::new())
    }

    fn as_editable(&mut self) -> Option<&mut dyn EditableScreen> {
        Some(self)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl EditableScreen for GraphicsScreenBuffer {
    fn snapshot_scrollback(&mut self) -> Option<Arc<Mutex<Box<dyn Screen>>>> {
        if let Ok(mut sb) = self.scrollback_buffer.lock() {
            if let Some(scrollback) = sb.as_any_mut().downcast_mut::<ScrollbackBuffer>() {
                scrollback.snapshot_current_screen(self);
            }
        }
        Some(self.scrollback_buffer.clone())
    }

    fn get_first_visible_line(&self) -> i32 {
        0
    }

    fn get_last_visible_line(&self) -> i32 {
        self.char_screen_size.height - 1
    }

    fn get_first_editable_line(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            if let Some((start, _)) = self.terminal_state.get_margins_top_bottom() {
                return start;
            }
        }
        0
    }

    fn get_last_editable_line(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            if let Some((_, end)) = self.terminal_state.get_margins_top_bottom() {
                return end;
            }
        }
        self.char_screen_size.height - 1
    }

    fn get_first_editable_column(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            if let Some((start, _)) = self.terminal_state.get_margins_left_right() {
                return start;
            }
        }
        0
    }

    fn get_last_editable_column(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            if let Some((_, end)) = self.terminal_state.get_margins_left_right() {
                return end;
            }
        }
        self.char_screen_size.width - 1
    }

    fn get_line(&self, _line: usize) -> Option<&Line> {
        // won't work for rgba screens.
        None
    }

    fn line_count(&self) -> usize {
        // won't work for rgba screens.
        0
    }

    fn set_resolution(&mut self, size: Size) {
        self.pixel_size = size;
        let font_size = self.get_font(0).unwrap().size();
        self.char_screen_size = Size::new(self.pixel_size.width / font_size.width, self.pixel_size.height / font_size.height);
        // Fill with background color from caret (0 for white in most palettes)
        let bg_color = self.caret.attribute.get_background();
        self.screen.clear();
        self.screen
            .resize((self.pixel_size.width as usize) * (self.pixel_size.height as usize), bg_color as u8);
    }

    fn screen_mut(&mut self) -> &mut Vec<u8> {
        &mut self.screen
    }

    fn set_graphics_type(&mut self, graphics_type: crate::GraphicsType) {
        self.graphics_type = graphics_type;

        self.scan_lines = match graphics_type {
            GraphicsType::IGS(term_res) => term_res.use_scanlines(),
            _ => false,
        };

        match graphics_type {
            GraphicsType::IGS(res) => {
                self.caret.attribute.set_foreground(1);
                self.caret.attribute.set_background(0);
                self.terminal_state.cr_is_if = true;
                self.set_resolution(res.get_resolution());
            }
            _ => {
                // Keep current caret settings
            }
        }

        self.buffer_dirty.store(true, std::sync::atomic::Ordering::Relaxed);
        self.buffer_version.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn update_hyperlinks(&mut self) {
        // No-op for now
    }

    /// Override: Convert character grid coordinates to pixel coordinates
    fn set_caret_position(&mut self, pos: Position) {
        let font_size = self.get_font_dimensions();
        let pixel_pos = Position::new(pos.x * font_size.width, pos.y * font_size.height);
        self.caret.set_position(pixel_pos);
    }

    /// Override: print_char that works with pixel coordinates
    fn print_char(&mut self, ch: AttributedChar) {
        let font_size = self.get_font_dimensions();

        if self.caret.insert_mode {
            self.ins();
        }
        let mut pos = self.caret.position();

        if let Some(font) = self.get_font(self.caret.font_page()) {
            if let Some(shift) = font.yaff_font.global_shift_up {
                pos.y = pos.y.saturating_sub(shift as i32);
            }
        }

        if let Some(glyph) = self.render_char_to_buffer(pos, ch) {
            let advance_width = if self.graphics_type() == GraphicsType::Skypix {
                // For proportional fonts with bearing information, use glyph metrics
                // For monospace fonts, use font_size.width
                let font = if let Some(font) = self.get_font(ch.get_font_page()) {
                    font
                } else if let Some(font) = self.get_font(0) {
                    font
                } else {
                    &DEFAULT_BITFONT
                };

                // For proportional fonts, use glyph metrics
                if font.yaff_font.spacing == Some(libyaff::FontSpacing::Proportional) {
                    // Calculate advance width using glyph bearings
                    let glyph_width = glyph.bitmap.pixels.get(0).map(|row| row.len() as i32).unwrap_or(0);
                    let left_bearing = glyph.left_bearing.unwrap_or(0);
                    let right_bearing = glyph.right_bearing.unwrap_or(0);

                    // For empty glyphs (like space), left_bearing is the total advance
                    if glyph_width == 0 && left_bearing > 0 {
                        left_bearing
                    } else {
                        left_bearing + glyph_width + right_bearing
                    }
                } else {
                    // For monospace fonts, always use font_size.width
                    font_size.width
                }
            } else {
                font_size.width
            };

            self.caret.x += advance_width;
        }

        // Check for wrap
        if self.caret.x >= self.pixel_size.width {
            if self.terminal_state.auto_wrap_mode == crate::AutoWrapMode::AutoWrap {
                self.caret.x = 0;
                self.caret.y += font_size.height;
            } else {
                self.lf();
                return;
            }
        }

        self.mark_dirty();
    }

    /// Override: Line feed - move down by font height in pixels
    fn lf(&mut self) {
        let font_size = self.get_font_dimensions();
        let in_margin = self.terminal_state.in_margin(self.caret.position());
        self.caret.x = 0;
        self.caret.y += font_size.height;

        if self.terminal_state.is_terminal_buffer {
            while self.caret.y >= self.pixel_size.height {
                self.scroll_up();
                self.caret.y -= font_size.height;
            }
            // Call limit_caret_pos to respect margins
            self.limit_caret_pos(in_margin);
        }
    }

    fn reset_resolution(&mut self) {
        // Get original resolution from graphics_type
        let (px_width, px_height) = match self.graphics_type {
            GraphicsType::Text => {
                return; // No reset for text mode
            }
            GraphicsType::Rip => (RIP_SCREEN_SIZE.width, RIP_SCREEN_SIZE.height),
            GraphicsType::IGS(term_res) => {
                let res = term_res.get_resolution();
                (res.width, res.height)
            }
            GraphicsType::Skypix => (800, 600),
        };

        // Reset to original resolution
        let original_size = Size::new(px_width, px_height);
        self.set_resolution(original_size);
    }

    fn add_sixel(&mut self, _pos: Position, _sixel: crate::Sixel) {
        // TODO: implement me? Are there sixels here?
    }

    fn clear_mouse_fields(&mut self) {
        self.mouse_fields.clear();
    }

    fn add_mouse_field(&mut self, mouse_field: MouseField) {
        self.mouse_fields.push(mouse_field);
    }

    fn ice_mode_mut(&mut self) -> &mut IceMode {
        &mut self.ice_mode
    }

    fn caret_mut(&mut self) -> &mut Caret {
        &mut self.caret
    }

    fn palette_mut(&mut self) -> &mut Palette {
        &mut self.palette
    }

    fn buffer_type_mut(&mut self) -> &mut BufferType {
        &mut self.buffer_type
    }

    fn terminal_state_mut(&mut self) -> &mut TerminalState {
        &mut self.terminal_state
    }

    fn reset_terminal(&mut self) {
        self.terminal_state.reset_terminal(self.terminal_state.get_size());
        self.caret.reset();
    }

    fn insert_line(&mut self, _line: usize, _new_line: Line) {
        // currently unused in rgba screens.
    }

    fn set_font(&mut self, font_number: usize, font: BitFont) {
        self.font_table.insert(font_number, font);
    }

    fn remove_font(&mut self, font_number: usize) -> Option<BitFont> {
        self.font_table.remove(&font_number)
    }

    fn clear_font_table(&mut self) {
        self.font_table.clear();
    }

    fn set_size(&mut self, size: Size) {
        self.char_screen_size = size;
    }

    /// Scroll the screen up by one line (move content up, clear bottom line)
    fn scroll_up(&mut self) {
        let font = self.get_font_dimensions();
        let line_height = font.height as usize;
        let screen_width = self.pixel_size.width as usize;
        let screen_height = self.pixel_size.height as usize;

        if line_height == 0 || line_height >= screen_height {
            return;
        }

        let row_len = screen_width; // bytes per pixel row (1 byte per pixel)
        let movable_rows = screen_height - line_height;

        // Shift all rows up using memmove semantics (copy_within handles overlap)
        self.screen.copy_within(line_height * row_len..screen_height * row_len, 0);

        // Clear the freed bottom region
        self.screen[movable_rows * row_len..screen_height * row_len].fill(0);

        self.mark_dirty();
    }

    /// Scroll the screen down by one line (move content down, clear top line)
    fn scroll_down(&mut self) {
        let font = self.get_font_dimensions();
        let line_height = font.height as usize;
        let screen_width = self.pixel_size.width as usize;
        let screen_height = self.pixel_size.height as usize;

        if line_height == 0 || line_height >= screen_height {
            return;
        }

        let row_len = screen_width;
        let movable_rows = screen_height - line_height;

        // Shift rows down using memmove semantics
        self.screen.copy_within(0..movable_rows * row_len, line_height * row_len);

        // Clear the freed top region
        self.screen[0..line_height * row_len].fill(0);

        self.mark_dirty();
    }

    /// Scroll the screen left by one column (move content left, clear right column)
    fn scroll_left(&mut self) {
        let font = self.get_font_dimensions();
        let char_width = font.width as usize;
        let screen_width = self.pixel_size.width as usize;
        let screen_height = self.pixel_size.height as usize;

        if char_width == 0 || char_width >= screen_width {
            return;
        }

        for y in 0..screen_height as usize {
            let row_start = y * screen_width;
            // Shift row content left
            self.screen.copy_within(row_start + char_width..row_start + screen_width, row_start);
            // Clear vacated right area
            self.screen[row_start + screen_width - char_width..row_start + screen_width].fill(0);
        }

        self.mark_dirty();
    }

    /// Scroll the screen right by one column (move content right, clear left column)
    fn scroll_right(&mut self) {
        let font = self.get_font_dimensions();
        let char_width = font.width as usize;
        let screen_width = self.pixel_size.width as usize;
        let screen_height = self.pixel_size.height as usize;

        if char_width == 0 || char_width >= screen_width {
            return;
        }

        for y in 0..screen_height as usize {
            let row_start = y * screen_width;
            // Shift content right
            self.screen
                .copy_within(row_start..row_start + screen_width - char_width, row_start + char_width);
            // Clear vacated left area
            self.screen[row_start..row_start + char_width].fill(0);
        }

        self.mark_dirty();
    }

    fn clear_screen(&mut self) {
        self.set_caret_position(Position::default());
        self.terminal_state_mut().cleared_screen = true;

        // Clear pixel buffer
        self.screen.fill(self.caret.attribute.get_background() as u8);
        self.mark_dirty();
    }

    fn clear_scrollback(&mut self) {
        // No scrollback in this implementation
    }

    fn remove_terminal_line(&mut self, _line: i32) {
        // atm unused in rgba screens.
    }

    fn insert_terminal_line(&mut self, _line: i32) {
        // atm unused in rgba screens.
    }

    fn set_char(&mut self, pos: Position, ch: AttributedChar) {
        if pos.x < 0 || pos.y < 0 || pos.x >= self.char_screen_size.width || pos.y >= self.char_screen_size.height {
            return;
        }

        // Render directly to RGBA buffer
        self.render_char_to_buffer(pos, ch);

        // Mark buffer as dirty
        self.mark_dirty();
    }

    fn set_height(&mut self, height: i32) {
        if height == self.char_screen_size.height {
            return;
        }
        log::error!("error: setting height to {:?}", height);

        let height = height.max(1);

        // Update screen size
        self.char_screen_size.height = height;
    }

    fn add_hyperlink(&mut self, hyperlink: HyperLink) {
        self.hyperlinks.push(hyperlink);
    }

    fn mark_dirty(&self) {
        self.buffer_dirty.store(true, std::sync::atomic::Ordering::Release);
        self.buffer_version.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn saved_caret_pos(&mut self) -> &mut Position {
        &mut self.saved_pos
    }

    fn saved_cursor_state(&mut self) -> &mut SavedCaretState {
        &mut self.saved_cursor_state
    }

    fn handle_rip_command(&mut self, _cmd: RipCommand) {
        // self.handle_rip_command_impl(cmd);
    }

    fn handle_skypix_command(&mut self, cmd: SkypixCommand) {
        self.handle_skypix_command_impl(cmd);
    }

    fn handle_igs_command(&mut self, _cmd: icy_parser_core::IgsCommand) {
        // self.handle_igs_command_impl(cmd);
    }

    fn clear_buffer_down(&mut self) {
        let bg_color = self.caret.attribute.get_background() as u8;
        let screen_width = self.pixel_size.width as usize;
        let screen_height = self.pixel_size.height as usize;

        // Clear from current caret.y to end of screen
        let start_y = self.caret.y as usize;
        let clear_start = start_y * screen_width;
        let clear_end = screen_height * screen_width;

        if clear_start < clear_end {
            self.screen[clear_start..clear_end].fill(bg_color);
        }
    }

    fn clear_buffer_up(&mut self) {
        let bg_color = self.caret.attribute.get_background() as u8;
        let screen_width = self.pixel_size.width as usize;
        let font_size = self.get_font_dimensions();

        // Clear from top of screen to current line (caret.y + font_size.height)
        let end_y = ((self.caret.y as usize) + (font_size.height as usize)).min(self.pixel_size.height as usize);
        let clear_end = end_y * screen_width;

        if clear_end > 0 {
            self.screen[0..clear_end].fill(bg_color);
        }
    }

    fn clear_line(&mut self) {
        let font_size = self.get_font_dimensions();
        let bg_color = self.caret.attribute.get_background() as u8;
        let screen_width = self.pixel_size.width as usize;

        // Clear entire line from x=0 to screen width, for font_size.height rows
        let start_y = self.caret.y as usize;
        let end_y = (start_y + font_size.height as usize).min(self.pixel_size.height as usize);

        for y in start_y..end_y {
            let row_start = y * screen_width;
            let row_end = row_start + screen_width;
            self.screen[row_start..row_end].fill(bg_color);
        }
    }

    fn clear_line_end(&mut self) {
        let font_size = self.get_font_dimensions();
        let bg_color = self.caret.attribute.get_background() as u8;
        let screen_width = self.pixel_size.width as usize;

        // Clear from caret.x to end of line, for font_size.height rows
        let start_x = self.caret.x as usize;
        let start_y = self.caret.y as usize;
        let end_y = (start_y + font_size.height as usize).min(self.pixel_size.height as usize);

        for y in start_y..end_y {
            let row_start = y * screen_width;
            let clear_start = row_start + start_x;
            let clear_end = row_start + screen_width;
            self.screen[clear_start..clear_end].fill(bg_color);
        }
    }

    fn clear_line_start(&mut self) {
        let font_size = self.get_font_dimensions();
        let bg_color = self.caret.attribute.get_background() as u8;
        let screen_width = self.pixel_size.width as usize;

        // Clear from 0 to caret.x, for font_size.height rows
        let end_x = self.caret.x as usize;
        let start_y = self.caret.y as usize;
        let end_y = (start_y + font_size.height as usize).min(self.pixel_size.height as usize);

        for y in start_y..end_y {
            let row_start = y * screen_width;
            let clear_end = row_start + end_x;
            self.screen[row_start..clear_end].fill(bg_color);
        }
    }

    /// Override: Move left, handling autowrap and pixel coordinates
    fn left(&mut self, num: i32, scroll: bool) {
        let font_size = self.get_font_dimensions();
        let in_margin = self.terminal_state.in_margin(self.caret.position());

        if let crate::AutoWrapMode::AutoWrap = self.terminal_state().auto_wrap_mode {
            if self.caret().x == 0 {
                // At column 0: wrap to previous line end if above origin line
                let origin_line = match self.terminal_state().origin_mode {
                    crate::OriginMode::UpperLeftCorner => self.get_first_visible_line(),
                    crate::OriginMode::WithinMargins => self.get_first_editable_line(),
                };

                let char_y = self.caret().y / font_size.height;
                if char_y <= origin_line {
                    // Already at origin line -> no-op
                    return;
                }

                self.caret_mut().y -= font_size.height;
                self.caret_mut().x = ((self.get_width() - 1).max(0)) * font_size.width;
                if scroll {
                    self.check_scrolling_on_caret_up(false);
                }
                self.limit_caret_pos(in_margin);
                return;
            }
        }

        // Move left by num characters (in pixels)
        let char_x = self.caret().x / font_size.width;
        let new_char_x = char_x.saturating_sub(num);
        self.caret_mut().x = new_char_x * font_size.width;
        if scroll {
            self.check_scrolling_on_caret_up(false);
        }
        self.limit_caret_pos(in_margin);
    }

    /// Override: Move right, handling autowrap and pixel coordinates
    fn right(&mut self, num: i32, scroll: bool) {
        let font_size = self.get_font_dimensions();
        let last_col = (self.get_width() - 1).max(0);
        let in_margin = self.terminal_state.in_margin(self.caret.position());

        if let crate::AutoWrapMode::AutoWrap = self.terminal_state().auto_wrap_mode {
            let char_x = self.caret().x / font_size.width;
            if char_x >= last_col {
                // At end of line: move to start of next line, scrolling if needed
                self.caret_mut().x = 0;
                self.caret_mut().y += font_size.height;
                // Use existing scrolling logic to handle terminal buffers
                self.check_scrolling_on_caret_down(true);
                if scroll {
                    self.check_scrolling_on_caret_up(false);
                }
                self.limit_caret_pos(in_margin);
                return;
            }
        }

        // Move right by num characters (in pixels)
        let char_x = self.caret().x / font_size.width;
        let new_char_x = char_x.saturating_add(num);
        self.caret_mut().x = new_char_x * font_size.width;
        if scroll {
            self.check_scrolling_on_caret_up(false);
        }
        self.limit_caret_pos(in_margin);
    }

    /// Override: Move up, handling pixel coordinates
    fn up(&mut self, num: i32, scroll: bool) {
        let in_margin: bool = self.terminal_state.in_margin(self.caret.position());

        let font_size = self.get_font_dimensions();
        let char_y = self.caret().y / font_size.height;
        let new_char_y = char_y.saturating_sub(num);
        self.caret_mut().y = new_char_y * font_size.height;
        if scroll {
            self.check_scrolling_on_caret_up(false);
        }
        self.limit_caret_pos(in_margin);
    }

    /// Override: Move down, handling pixel coordinates
    fn down(&mut self, num: i32, scroll: bool) {
        let in_margin = self.terminal_state.in_margin(self.caret.position());

        let font_size = self.get_font_dimensions();
        let char_y = self.caret().y / font_size.height;
        let new_char_y = char_y + num;
        self.caret_mut().y = new_char_y * font_size.height;
        if scroll {
            self.check_scrolling_on_caret_down(false);
        }
        self.limit_caret_pos(in_margin);
    }
}
