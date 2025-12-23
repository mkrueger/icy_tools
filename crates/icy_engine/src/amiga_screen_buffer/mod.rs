use icy_parser_core::{RipCommand, SkypixCommand};

pub mod sky_paint;
pub mod skypix_impl;

use crate::{
    AttributedChar, BitFont, BufferType, Caret, DOS_DEFAULT_PALETTE, EditableScreen, GraphicsType, HyperLink, IceMode, Line, Palette, Position, Rectangle,
    RenderOptions, Result, SaveOptions, SavedCaretState, Screen, ScrollbackBuffer, Selection, SelectionMask, Size, TerminalResolutionExt, TerminalState,
    TextPane,
    bgi::{Bgi, DEFAULT_BITFONT, MouseField},
    igs, limits,
    rip_impl::RIP_SCREEN_SIZE,
};
use parking_lot::Mutex;
use skypix_impl::{SKYPIX_DEFAULT_FONT, SKYPIX_SCREEN_SIZE};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct AmigaScreenBuffer {
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

    // SkyPaint graphics handler (for Skypix protocol)
    pub sky_paint: sky_paint::SkyPaint,

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
    pub scrollback_buffer: ScrollbackBuffer,
    pub text_mode: TextMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextMode {
    Jam1,
    Jam2,
}

impl AmigaScreenBuffer {
    /// Creates a new PaletteScreenBuffer with pixel dimensions
    /// px_width, px_height: pixel dimensions (e.g., 640x350 for RIP graphics)
    pub fn new(graphics_type: GraphicsType) -> Self {
        let (px_width, px_height) = match graphics_type {
            GraphicsType::Text => {
                panic!()
            }
            GraphicsType::Rip => (RIP_SCREEN_SIZE.width, RIP_SCREEN_SIZE.height),
            GraphicsType::IGS(term_res) => {
                let res = term_res.resolution();
                (res.width, res.height)
            }
            GraphicsType::Skypix => (SKYPIX_SCREEN_SIZE.width, SKYPIX_SCREEN_SIZE.height),
        };

        let mut font_table: HashMap<usize, BitFont> = HashMap::new();
        match graphics_type {
            GraphicsType::Rip => {
                font_table.insert(0, crate::rip::FONT.clone());
                font_table.insert(1, crate::rip::EGA_7x8.clone());
                font_table.insert(2, crate::rip::VGA_8x14.clone());
                font_table.insert(3, crate::rip::VGA_7x14.clone());
                font_table.insert(4, crate::rip::VGA_16x14.clone());
            }
            GraphicsType::IGS(_) => {
                font_table.insert(0, igs::ATARI_ST_FONT_8x8.clone());
            }
            GraphicsType::Skypix => {
                font_table.insert(0, SKYPIX_DEFAULT_FONT.clone());
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
            GraphicsType::IGS(res) => res.palette().clone(),
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
            sky_paint: sky_paint::SkyPaint::new(),
            _igs_state: None,
            buffer_dirty: std::sync::atomic::AtomicBool::new(true),
            buffer_version: std::sync::atomic::AtomicU64::new(0),
            saved_pos: Position::default(),
            saved_cursor_state: SavedCaretState::default(),
            graphics_type,
            scan_lines,
            scrollback_buffer: ScrollbackBuffer::new(),
            text_mode: TextMode::Jam2,
        }
    }

    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Render a character directly to the RGBA buffer
    fn render_char_to_buffer(&mut self, pos: Position, ch: AttributedChar) -> Option<()> {
        let pixel_x = pos.x;
        let pixel_y = pos.y;

        // Get colors from palette, swap if inverse video mode is
        let (fg_color, bg_color) = (ch.attribute.foreground() as u32, ch.attribute.background() as u32);

        let font_size = self.font_dimensions();
        let transparent_bg = self.text_mode == TextMode::Jam1;
        let pixel_width = self.pixel_size.width;
        let pixel_height = self.pixel_size.height;

        // Copy glyph data to avoid borrow conflict
        let (glyph_data, glyph_width, glyph_height) = {
            let font = if let Some(font) = self.font(ch.font_page() as usize) {
                font
            } else if let Some(font) = self.font(0) {
                font
            } else {
                &DEFAULT_BITFONT
            };
            let glyph = font.glyph(ch.ch);
            (glyph.data, glyph.width as i32, glyph.height as usize)
        };

        let render_width = if transparent_bg { glyph_width } else { font_size.width as i32 };

        for row in 0..font_size.height {
            for col in 0..render_width {
                let px = pixel_x + col;
                let py = pixel_y + row;

                if px < 0 || px >= pixel_width || py < 0 || py >= pixel_height {
                    continue;
                }

                // Check if pixel is set in font glyph using packed byte data
                let is_foreground = if col >= 0 && (row as usize) < glyph_height && col < glyph_width {
                    (glyph_data[row as usize] & (0x80 >> col)) != 0
                } else {
                    false
                };

                // Skip background pixels for transparent mode (Skypix)
                if transparent_bg && !is_foreground {
                    continue;
                }

                let color = if is_foreground { fg_color } else { bg_color };

                // Write to RGBA buffer
                let offset = py * pixel_width + px;
                self.screen[offset as usize] = color as u8;
            }
        }
        Some(())
    }

    pub fn get_pixel_dimensions(&self) -> (usize, usize) {
        (self.char_screen_size.width as usize, self.char_screen_size.height as usize)
    }
}

impl TextPane for AmigaScreenBuffer {
    fn char_at(&self, _pos: Position) -> AttributedChar {
        // won't work for rgba screens.
        AttributedChar::default()
    }

    fn line_count(&self) -> i32 {
        self.char_screen_size.height
    }

    fn width(&self) -> i32 {
        self.char_screen_size.width
    }

    fn height(&self) -> i32 {
        self.char_screen_size.height
    }

    fn size(&self) -> Size {
        self.char_screen_size
    }

    fn line_length(&self, _line: i32) -> i32 {
        // won't work for rgba screens.
        0
    }

    fn rectangle(&self) -> crate::Rectangle {
        crate::Rectangle::from_coords(0, 0, self.char_screen_size.width - 1, self.char_screen_size.height - 1)
    }
}

impl Screen for AmigaScreenBuffer {
    fn ice_mode(&self) -> IceMode {
        self.ice_mode
    }

    fn terminal_state(&self) -> &TerminalState {
        &self.terminal_state
    }

    /// Override: Convert pixel coordinates to character grid coordinates
    fn caret_position(&self) -> Position {
        let pixel_pos = self.caret.position();
        let font_size = self.font_dimensions();
        // Position::new(pixel_pos.x / font_size.width, ((pixel_pos.y as f32) / font_size.height as f32).ceil() as i32)
        Position::new(pixel_pos.x / font_size.width, pixel_pos.y / font_size.height)
    }

    fn graphics_type(&self) -> crate::GraphicsType {
        self.graphics_type
    }

    fn scan_lines(&self) -> bool {
        self.scan_lines
    }

    fn render_region_to_rgba(&self, px_region: Rectangle, options: &RenderOptions) -> (Size, Vec<u8>) {
        // Use cached palette as packed RGBA u32 for single write per pixel
        let palette_cache_rgba = self.palette.palette_cache_rgba();

        // Clamp region to screen bounds
        let x = px_region.start.x.clamp(0, self.pixel_size.width);
        let y = px_region.start.y.clamp(0, self.pixel_size.height);
        let width = px_region.size.width.clamp(0, self.pixel_size.width - x);
        let height = px_region.size.height.clamp(0, self.pixel_size.height - y);

        if width <= 0 || height <= 0 {
            return (Size::new(0, 0), Vec::new());
        }

        let scan_lines = options.override_scan_lines.unwrap_or(self.scan_lines);
        let width_usize = width as usize;
        let screen_width = self.pixel_size.width as usize;
        let palette_len = palette_cache_rgba.len();
        // Default color: black with full alpha (RGBA)
        const DEFAULT_COLOR: u32 = 0xFF000000;

        let out_height = if scan_lines { height * 2 } else { height };
        let total_pixels = width_usize * out_height as usize;

        // Allocate as u32 array for single-write operations
        let mut pixels_u32 = vec![0u32; total_pixels];

        let mut dst_idx = 0usize;
        for py in 0..height {
            let src_y = (y + py) as usize;
            let row_start = src_y * screen_width + x as usize;

            // SAFETY: We've clamped x, y, width, height to screen bounds
            // so row_start + px is always < screen.len()
            // and palette_idx < palette_len is checked before access
            unsafe {
                for px in 0..width_usize {
                    let palette_idx = *self.screen.get_unchecked(row_start + px) as usize;
                    let rgba = if palette_idx < palette_len {
                        *palette_cache_rgba.get_unchecked(palette_idx)
                    } else {
                        DEFAULT_COLOR
                    };
                    *pixels_u32.get_unchecked_mut(dst_idx) = rgba;
                    dst_idx += 1;
                }
            }

            if scan_lines {
                // Copy the rendered line for scanline effect (same line duplicated)
                let src_start = dst_idx - width_usize;
                pixels_u32.copy_within(src_start..dst_idx, dst_idx);
                dst_idx += width_usize;
            }
        }

        // SAFETY: Reinterpret Vec<u32> as Vec<u8> - u32 is 4 bytes, same memory layout
        let pixels = unsafe {
            let ptr = pixels_u32.as_mut_ptr() as *mut u8;
            let len = pixels_u32.len() * 4;
            let cap = pixels_u32.capacity() * 4;
            std::mem::forget(pixels_u32);
            Vec::from_raw_parts(ptr, len, cap)
        };

        (Size::new(width, out_height), pixels)
    }

    fn render_to_rgba(&self, options: &RenderOptions) -> (Size, Vec<u8>) {
        self.render_region_to_rgba(Rectangle::from_min_size((0, 0), self.resolution()), options)
    }

    fn font_dimensions(&self) -> Size {
        if let Some(font) = self.font(self.caret.font_page() as usize) {
            font.size()
        } else if let Some(font) = self.font(0) {
            font.size()
        } else {
            Size::new(8, 16)
        }
    }

    fn set_font_dimensions(&mut self, _size: Size) {
        // nothing
    }

    fn font(&self, font_number: usize) -> Option<&BitFont> {
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

    fn selection(&self) -> Option<Selection> {
        None
    }

    fn selection_mask(&self) -> &SelectionMask {
        &self.selection_mask
    }

    fn hyperlinks(&self) -> &Vec<HyperLink> {
        &self.hyperlinks
    }

    fn default_foreground_color(&self) -> u32 {
        2
    }

    fn max_base_colors(&self) -> u32 {
        if let GraphicsType::IGS(t) = self.graphics_type {
            t.max_colors()
        } else {
            self.palette.len() as u32
        }
    }

    fn resolution(&self) -> Size {
        self.pixel_size
    }

    fn screen(&self) -> &[u8] {
        &self.screen
    }

    fn set_scrollback_buffer_size(&mut self, buffer_size: usize) {
        self.scrollback_buffer.set_buffer_size(buffer_size);
    }

    fn set_selection(&mut self, _selection: Selection) -> Result<()> {
        Ok(())
    }

    fn clear_selection(&mut self) -> Result<()> {
        Ok(())
    }

    fn mouse_fields(&self) -> &Vec<MouseField> {
        &self.mouse_fields
    }

    fn to_bytes(&mut self, _file_name: &str, _options: &SaveOptions) -> Result<Vec<u8>> {
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

impl EditableScreen for AmigaScreenBuffer {
    fn snapshot_scrollback(&mut self) -> Option<Arc<Mutex<Box<dyn Screen>>>> {
        let mut scrollback = self.scrollback_buffer.clone();
        scrollback.snapshot_current_screen(self);
        return Some(Arc::new(Mutex::new(Box::new(scrollback))));
    }

    fn first_visible_line(&self) -> i32 {
        0
    }

    fn last_visible_line(&self) -> i32 {
        self.char_screen_size.height - 1
    }

    fn first_editable_line(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            if let Some((start, _)) = self.terminal_state.margins_top_bottom() {
                return start;
            }
        }
        0
    }

    fn last_editable_line(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            if let Some((_, end)) = self.terminal_state.margins_top_bottom() {
                return end;
            }
        }
        self.char_screen_size.height - 1
    }

    fn first_editable_column(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            if let Some((start, _)) = self.terminal_state.margins_left_right() {
                return start;
            }
        }
        0
    }

    fn last_editable_column(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            if let Some((_, end)) = self.terminal_state.margins_left_right() {
                return end;
            }
        }
        self.char_screen_size.width - 1
    }

    fn get_line(&self, _line: usize) -> Option<&Line> {
        // won't work for rgba screens.
        None
    }

    fn physical_line_count(&self) -> usize {
        // won't work for rgba screens.
        0
    }

    fn set_resolution(&mut self, size: Size) {
        self.pixel_size = size;
        let font_size = self.font(0).unwrap().size();
        self.char_screen_size = Size::new(self.pixel_size.width / font_size.width, self.pixel_size.height / font_size.height);
        // Fill with background color from caret (0 for white in most palettes)
        let bg_color = self.caret.attribute.background();
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
                self.set_resolution(res.resolution());
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
        let font_size = self.font_dimensions();
        let pixel_pos = Position::new(pos.x * font_size.width, pos.y * font_size.height);
        self.caret.set_position(pixel_pos);
    }

    /// Override: print_char that works with pixel coordinates
    fn print_char(&mut self, ch: AttributedChar) {
        let font_size = self.font_dimensions();

        // Check if we need to scroll BEFORE rendering to avoid cutting off text
        if self.terminal_state.is_terminal_buffer && self.caret.y + font_size.height > self.pixel_size.height {
            while self.caret.y + font_size.height > self.pixel_size.height {
                self.scroll_up();
                self.caret.y -= font_size.height;
            }
        }

        if self.caret.insert_mode {
            self.ins();
        }
        let pos = self.caret.position();

        if self.render_char_to_buffer(pos, ch).is_some() {
            let advance_width = font_size.width;
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
    }

    /// Override: Line feed - move down by font height in pixels
    fn lf(&mut self) {
        if self.text_mode == TextMode::Jam1 {
            return; // No LF in Jam1 mode
        }
        let font_size = self.font_dimensions();
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

    fn cr(&mut self) {
        if self.text_mode == TextMode::Jam1 {
            return; // No LF in Jam1 mode
        }
        self.caret_mut().x = 0;
        self.limit_caret_pos(false);
    }

    fn reset_resolution(&mut self) {
        // Get original resolution from graphics_type
        let (px_width, px_height) = match self.graphics_type {
            GraphicsType::Text => {
                return; // No reset for text mode
            }
            GraphicsType::Rip => (RIP_SCREEN_SIZE.width, RIP_SCREEN_SIZE.height),
            GraphicsType::IGS(term_res) => {
                let res = term_res.resolution();
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
        self.terminal_state.reset_terminal(self.terminal_state.size());
        self.terminal_state_mut().cr_is_if = false;
        self.caret.reset();
        self.caret.set_foreground(self.default_foreground_color());
        self.caret.set_font_page(0);
        self.caret.shape = crate::CaretShape::Underline;
        self.text_mode = TextMode::Jam2;
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
        let font = self.font_dimensions();
        let line_height = font.height as usize;
        let screen_width = self.pixel_size.width as usize;
        let screen_height = self.pixel_size.height as usize;

        if line_height == 0 || line_height >= screen_height {
            return;
        }

        // Add top line to scrollback BEFORE scrolling (while data is still there)
        if self.terminal_state().margins_top_bottom().is_none() && self.terminal_state.is_terminal_buffer {
            let (size, rgba_data) = crate::scrollback_buffer::render_scrollback_region(self, line_height as i32);
            self.scrollback_buffer.add_chunk(rgba_data, size);
        }

        let row_len = screen_width; // bytes per pixel row (1 byte per pixel)
        let movable_rows = screen_height - line_height;

        // Shift all rows up using memmove semantics (copy_within handles overlap)
        self.screen.copy_within(line_height * row_len..screen_height * row_len, 0);

        // Clear the freed bottom region
        self.screen[movable_rows * row_len..screen_height * row_len].fill(0);
    }

    /// Scroll the screen down by one line (move content down, clear top line)
    fn scroll_down(&mut self) {
        let font = self.font_dimensions();
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
    }

    /// Scroll the screen left by one column (move content left, clear right column)
    fn scroll_left(&mut self) {
        let font = self.font_dimensions();
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
    }

    /// Scroll the screen right by one column (move content right, clear left column)
    fn scroll_right(&mut self) {
        let font = self.font_dimensions();
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
    }

    fn clear_screen(&mut self) {
        // Add entire screen to scrollback BEFORE clearing
        if self.terminal_state.is_terminal_buffer {
            let (size, rgba_data) = crate::scrollback_buffer::render_scrollback_region(self, self.pixel_size.height);
            self.scrollback_buffer.add_chunk(rgba_data, size);
        }

        self.set_caret_position(Position::default());
        self.terminal_state_mut().cleared_screen = true;

        // Clear pixel buffer
        self.screen.fill(self.caret.attribute.background() as u8);
        self.terminal_state_mut().cr_is_if = false;
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
    }

    fn set_width(&mut self, width: i32) {
        let width = width.min(limits::MAX_BUFFER_WIDTH);
        if width == self.char_screen_size.width {
            return;
        }
        let width = width.max(1);

        // Update screen size
        self.char_screen_size.width = width;
    }

    fn set_height(&mut self, height: i32) {
        let height = height.min(limits::MAX_BUFFER_HEIGHT);
        if height == self.char_screen_size.height {
            return;
        }
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
        let bg_color = self.caret.attribute.background() as u8;
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
        let bg_color = self.caret.attribute.background() as u8;
        let screen_width = self.pixel_size.width as usize;
        let font_size = self.font_dimensions();

        // Clear from top of screen to current line (caret.y + font_size.height)
        let end_y = ((self.caret.y as usize) + (font_size.height as usize)).min(self.pixel_size.height as usize);
        let clear_end = end_y * screen_width;

        if clear_end > 0 {
            self.screen[0..clear_end].fill(bg_color);
        }
    }

    fn clear_line(&mut self) {
        let font_size = self.font_dimensions();
        let bg_color = self.caret.attribute.background() as u8;
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
        let font_size = self.font_dimensions();
        let bg_color = self.caret.attribute.background() as u8;
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
        let font_size = self.font_dimensions();
        let bg_color = self.caret.attribute.background() as u8;
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
    fn left(&mut self, num: i32, scroll: bool, auto_wrap: bool) {
        let font_size = self.font_dimensions();

        if auto_wrap && self.caret.x == 0 {
            // At column 0: wrap to previous line end if above origin line
            if self.caret.y <= 0 {
                // Already at origin line -> no-op
                return;
            }

            self.caret.y -= font_size.height;
            self.caret.x = (self.pixel_size.width - font_size.width).max(0);
            if scroll {
                self.check_scrolling_on_caret_up(false, false);
            }
            self.limit_caret_pos(false);
            return;
        }

        // Move left by num characters (in pixels)
        self.caret.x = (self.caret.x - num * font_size.width).max(0);
        if scroll {
            self.check_scrolling_on_caret_up(false, false);
        }
        self.limit_caret_pos(false);
    }

    fn sgr_reset(&mut self) {
        self.caret_default_colors();
        self.caret.set_font_page(0);
        self.caret_mut().attribute.set_is_bold(false);
        self.terminal_state_mut().inverse_video = false;
        self.text_mode = TextMode::Jam2;
    }

    /// Override: Move right, handling autowrap and pixel coordinates
    fn right(&mut self, num: i32, scroll: bool, auto_wrap: bool) {
        let font_size = self.font_dimensions();
        let last_pixel_x = self.pixel_size.width - font_size.width;

        if auto_wrap && self.caret.x >= last_pixel_x {
            // At end of line: move to start of next line, scrolling if needed
            self.caret.x = 0;
            self.caret.y += font_size.height;
            // Use existing scrolling logic to handle terminal buffers
            self.check_scrolling_on_caret_down(true, false);
            if scroll {
                self.check_scrolling_on_caret_up(false, false);
            }
            self.limit_caret_pos(false);
            return;
        }

        // Move right by num characters (in pixels)
        self.caret.x = (self.caret.x + num * font_size.width).min(self.pixel_size.width - 1);
        if scroll {
            self.check_scrolling_on_caret_up(false, false);
        }
        self.limit_caret_pos(false);
    }

    /// Override: Move up, handling pixel coordinates
    fn up(&mut self, num: i32, scroll: bool, _auto_wrap: bool) {
        let font_size = self.font_dimensions();
        self.caret_mut().y -= num * font_size.height;
        if scroll {
            self.check_scrolling_on_caret_up(false, false);
        }
    }

    /// Override: Move down, handling pixel coordinates
    fn down(&mut self, num: i32, scroll: bool, _auto_wrap: bool) {
        let font_size = self.font_dimensions();
        self.caret_mut().y += num * font_size.height;
        if scroll {
            self.check_scrolling_on_caret_down(false, false);
        }
    }

    fn check_scrolling_on_caret_up(&mut self, force: bool, _in_margin: bool) {
        let font_size = self.font_dimensions();
        if self.terminal_state().needs_scrolling() || force {
            while self.caret.y < 0 {
                self.scroll_down();
                self.caret.y += font_size.height;
            }
        }
    }

    fn check_scrolling_on_caret_down(&mut self, force: bool, _in_margin: bool) {
        let font_size = self.font_dimensions();
        // Scroll up if caret.y + font_height would exceed screen height
        if self.terminal_state().needs_scrolling() || force {
            while self.caret.y + font_size.height > self.pixel_size.height {
                self.scroll_up();
                self.caret.y -= font_size.height;
            }
        }
    }

    fn limit_caret_pos(&mut self, _was_in_margin: bool) {
        // Amiga screens have no margins - just clamp to screen bounds in pixel coordinates
        self.caret.x = self.caret.x.clamp(0, (self.pixel_size.width - 1).max(0));
        self.caret.y = self.caret.y.clamp(0, (self.pixel_size.height - 1).max(0));
    }
}
