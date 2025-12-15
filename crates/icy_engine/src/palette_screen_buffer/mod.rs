use icy_parser_core::{RipCommand, SkypixCommand};

pub mod bgi;
pub mod rip_impl;

pub mod igs;
pub use igs::{TerminalResolution, TerminalResolutionExt};

use crate::{
    AnsiSaveOptionsV2, AttributedChar, BitFont, BufferType, Caret, DOS_DEFAULT_PALETTE, EditableScreen, GraphicsType, HyperLink, IceMode, Line, Palette,
    Position, Rectangle, RenderOptions, Result, SavedCaretState, Screen, ScrollbackBuffer, Selection, SelectionMask, Size, TerminalState, TextPane,
    amiga_screen_buffer::skypix_impl::SKYPIX_SCREEN_SIZE,
    bgi::{Bgi, DEFAULT_BITFONT, MouseField},
    limits,
    palette_screen_buffer::rip_impl::RIP_SCREEN_SIZE,
};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::{collections::HashMap, sync::Arc};

pub use rip_impl::RIP_TERMINAL_ID;

pub struct PaletteScreenBuffer {
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
    igs_state: Option<igs::vdi_paint::VdiPaint>,

    // Dirty tracking for rendering optimization
    buffer_dirty: std::sync::atomic::AtomicBool,
    buffer_version: std::sync::atomic::AtomicU64,

    saved_pos: Position,
    saved_cursor_state: SavedCaretState,
    graphics_type: GraphicsType,

    // Scan lines for Atari ST Medium resolution (doubles line height)
    pub scrollback_buffer: ScrollbackBuffer,
}

impl PaletteScreenBuffer {
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
                font_table.insert(0, crate::rip::FONT.clone());
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
            igs_state: None,
            buffer_dirty: std::sync::atomic::AtomicBool::new(true),
            buffer_version: std::sync::atomic::AtomicU64::new(0),
            saved_pos: Position::default(),
            saved_cursor_state: SavedCaretState::default(),
            graphics_type,
            scrollback_buffer: ScrollbackBuffer::new(),
        }
    }

    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Render a character directly to the RGBA buffer
    fn render_char_to_buffer(&mut self, pos: Position, ch: AttributedChar) {
        if pos.x < 0 || pos.y < 0 || pos.x >= self.char_screen_size.width || pos.y >= self.char_screen_size.height {
            return;
        }

        let x = pos.x;
        let y = pos.y;

        // Get colors from palette
        let mut fg_color = ch.attribute.foreground() as u32;
        if ch.attribute.is_bold() && fg_color < 8 {
            fg_color += 8;
        }

        let bg_color = ch.attribute.background() as u32; // Apply color limit

        let font = if let Some(font) = self.font(ch.font_page()) {
            font
        } else if let Some(font) = self.font(0) {
            font
        } else {
            &DEFAULT_BITFONT
        };

        // Calculate pixel position
        let pixel_x = x * font.size().width;
        let pixel_y = y * font.size().height;

        // Get glyph data from font
        let glyph = font.glyph(ch.ch);
        // Render the character
        let font_size = font.size();

        // let glyph_size = self.font.size();
        for row in 0..font_size.height {
            for col in 0..font_size.width {
                let px = pixel_x + col;
                let py = pixel_y + row;

                if px >= self.pixel_size.width || py >= self.pixel_size.height {
                    continue;
                }

                // Check if pixel is set in font glyph
                let is_foreground = if let Some(g) = &glyph {
                    // Use bitmap.pixels[row][col] if in bounds
                    if row < g.bitmap.pixels.len() as i32 && col < g.bitmap.pixels[row as usize].len() as i32 {
                        g.bitmap.pixels[row as usize][col as usize]
                    } else {
                        false
                    }
                } else {
                    log::error!("NO GLYPH for char '{}'", ch.ch);
                    false
                };

                // Clone colors to avoid move in loop
                let color = if is_foreground { fg_color } else { bg_color };

                // Write to RGBA buffer
                let offset = py * self.pixel_size.width + px;
                self.screen[offset as usize] = color as u8;
            }
        }
    }

    pub fn get_pixel_dimensions(&self) -> (usize, usize) {
        (self.char_screen_size.width as usize, self.char_screen_size.height as usize)
    }
}

impl TextPane for PaletteScreenBuffer {
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

impl Screen for PaletteScreenBuffer {
    fn ice_mode(&self) -> IceMode {
        self.ice_mode
    }

    fn terminal_state(&self) -> &TerminalState {
        &self.terminal_state
    }

    fn graphics_type(&self) -> crate::GraphicsType {
        self.graphics_type
    }

    fn scan_lines(&self) -> bool {
        self.graphics_type().scan_lines()
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

        let scan_lines = options.override_scan_lines.unwrap_or(self.scan_lines());
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
        if let Some(font) = self.font(0) { font.size() } else { Size::new(8, 16) }
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

    fn copy_text(&self) -> Option<String> {
        None
    }

    fn copy_rich_text(&self) -> Option<String> {
        None
    }

    fn clipboard_data(&self) -> Option<Vec<u8>> {
        None
    }
    fn mouse_fields(&self) -> &Vec<MouseField> {
        &self.mouse_fields
    }

    fn version(&self) -> u64 {
        self.buffer_version.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn default_foreground_color(&self) -> u32 {
        self.graphics_type.default_fg_color()
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

    fn as_editable(&mut self) -> Option<&mut dyn EditableScreen> {
        Some(self)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_bytes(&mut self, _file_name: &str, _options: &AnsiSaveOptionsV2) -> Result<Vec<u8>> {
        // Return empty for now, could implement PNG export later
        Ok(Vec::new())
    }
}

impl EditableScreen for PaletteScreenBuffer {
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
        None
    }

    fn line_count(&self) -> usize {
        0
    }

    fn set_graphics_type(&mut self, graphics_type: crate::GraphicsType) {
        self.graphics_type = graphics_type;

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
        self.caret.reset();
        self.caret.set_foreground(self.default_foreground_color());
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

        // Add top line to scrollback before scrolling (while data is still there)
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

        self.mark_dirty();
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

        self.mark_dirty();
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

        self.mark_dirty();
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

        self.mark_dirty();
    }

    fn clear_screen(&mut self) {
        // Add entire screen to scrollback
        if self.terminal_state.is_terminal_buffer {
            let (size, rgba_data) = crate::scrollback_buffer::render_scrollback_region(self, self.pixel_size.height);
            self.scrollback_buffer.add_chunk(rgba_data, size);
        }

        self.set_caret_position(Position::default());
        self.terminal_state_mut().cleared_screen = true;

        // Clear pixel buffer
        self.screen.fill(self.caret.attribute.background() as u8);
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

    fn handle_rip_command(&mut self, cmd: RipCommand) {
        self.handle_rip_command_impl(cmd);
    }

    fn handle_skypix_command(&mut self, _cmd: SkypixCommand) {
        // ATM NO-OP handled by graphics_screen_buffer
    }

    fn handle_igs_command(&mut self, cmd: icy_parser_core::IgsCommand) {
        self.handle_igs_command_impl(cmd);
    }

    fn clear_buffer_down(&mut self) {
        let pos = self.caret_position();
        let ch: AttributedChar = AttributedChar {
            attribute: self.caret().attribute,
            ..Default::default()
        };

        for y in pos.y..self.last_visible_line() {
            for x in 0..self.width() {
                self.set_char((x, y).into(), ch);
            }
        }
    }

    fn clear_buffer_up(&mut self) {
        let pos = self.caret_position();
        let ch: AttributedChar = AttributedChar {
            attribute: self.caret().attribute,
            ..Default::default()
        };

        for y in self.first_visible_line()..pos.y {
            for x in 0..self.width() {
                self.set_char((x, y).into(), ch);
            }
        }
    }

    fn clear_line(&mut self) {
        let mut pos = self.caret_position();
        let ch: AttributedChar = AttributedChar {
            attribute: self.caret().attribute,
            ..Default::default()
        };
        for x in 0..self.width() {
            pos.x = x;
            self.set_char(pos, ch);
        }
    }

    fn clear_line_end(&mut self) {
        let mut pos = self.caret_position();
        let ch: AttributedChar = AttributedChar {
            attribute: self.caret().attribute,
            ..Default::default()
        };
        for x in pos.x..self.width() {
            pos.x = x;
            self.set_char(pos, ch);
        }
    }

    fn clear_line_start(&mut self) {
        let mut pos = self.caret_position();
        let ch: AttributedChar = AttributedChar {
            attribute: self.caret().attribute,
            ..Default::default()
        };
        for x in 0..pos.x {
            pos.x = x;
            self.set_char(pos, ch);
        }
    }
}
