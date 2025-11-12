use crate::{
    AttributedChar, BitFont, BufferType, Caret, DOS_DEFAULT_PALETTE, EditableScreen, EngineResult, HyperLink, IceMode, Layer, Line, Palette, Position,
    Rectangle, RenderOptions, RgbaScreen, SaveOptions, Screen, Selection, SelectionMask, Size, TerminalState, TextPane,
    rip::bgi::{DEFAULT_BITFONT, MouseField},
};
use std::collections::HashMap;
use std::thread::JoinHandle;

pub struct PaletteScreenBuffer {
    pub pixel_size: Size,
    pub screen: Vec<u8>,
    pub char_screen_size: Size,

    // Text layer for char storage and compatibility
    layer: Layer,

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

    // Dirty tracking for rendering optimization
    buffer_dirty: std::sync::atomic::AtomicBool,
    buffer_version: std::sync::atomic::AtomicU64,
}

impl PaletteScreenBuffer {
    /// Creates a new PaletteScreenBuffer with pixel dimensions
    /// px_width, px_height: pixel dimensions (e.g., 640x350 for RIP graphics)
    pub fn new(px_width: i32, px_height: i32, font: BitFont) -> Self {
        // Calculate character grid dimensions from pixel size
        let char_cols = px_width / font.size().width;
        let char_rows = px_height / font.size().height;

        // Allocate RGBA pixel buffer (4 bytes per pixel)
        let screen = vec![0u8; px_width as usize * px_height as usize];

        // Create text layer with character dimensions
        let mut layer = Layer::new("", Size::new(char_cols, char_rows));
        layer.lines.clear();
        for _ in 0..char_rows {
            layer.lines.push(Line::new());
        }

        let mut font_table = HashMap::new();
        font_table.insert(0, font);

        Self {
            pixel_size: Size::new(px_width, px_height),        // Store character dimensions
            char_screen_size: Size::new(char_cols, char_rows), // Store pixel dimensions
            screen,
            layer,
            font_table,
            palette: Palette::from_slice(&DOS_DEFAULT_PALETTE),
            caret: Caret::default(),
            ice_mode: IceMode::Unlimited,
            terminal_state: TerminalState::from(Size::new(char_cols, char_rows)),
            buffer_type: BufferType::CP437,
            hyperlinks: Vec::new(),
            selection_mask: SelectionMask::default(),
            mouse_fields: Vec::new(),
            buffer_dirty: std::sync::atomic::AtomicBool::new(true),
            buffer_version: std::sync::atomic::AtomicU64::new(0),
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
        let mut fg_color = ch.attribute.get_foreground() as u32;
        if ch.attribute.is_bold() && fg_color < 8 {
            fg_color += 8;
        }

        let bg_color = ch.attribute.get_background() as u32;

        let font = if let Some(font) = self.get_font(ch.get_font_page()) {
            font
        } else if let Some(font) = self.get_font(0) {
            font
        } else {
            &DEFAULT_BITFONT
        };

        // Calculate pixel position
        let pixel_x = x * font.size().width;
        let pixel_y = y * font.size().height;

        // Get glyph data from font
        let glyph = font.get_glyph(ch.ch);

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
    fn get_char(&self, pos: Position) -> AttributedChar {
        if pos.x < 0 || pos.y < 0 || pos.x >= self.char_screen_size.width || pos.y >= self.char_screen_size.height {
            return AttributedChar::default();
        }

        let y = pos.y as usize;
        let x = pos.x as usize;

        if y < self.layer.lines.len() {
            let line = &self.layer.lines[y];
            if x < line.chars.len() {
                return line.chars[x];
            }
        }

        AttributedChar::default()
    }

    fn get_size(&self) -> Size {
        // Return character dimensions (resolution already stores char cols/rows)
        self.char_screen_size
    }

    fn get_line_count(&self) -> i32 {
        self.get_height()
    }

    fn get_width(&self) -> i32 {
        self.char_screen_size.width
    }

    fn get_height(&self) -> i32 {
        self.char_screen_size.height
    }

    fn get_line_length(&self, line: i32) -> i32 {
        if line < 0 || line >= self.char_screen_size.height {
            return 0;
        }

        let y = line as usize;
        if y < self.layer.lines.len() {
            self.layer.lines[y].chars.len() as i32
        } else {
            0
        }
    }

    fn get_rectangle(&self) -> Rectangle {
        Rectangle::from_coords(0, 0, self.get_width() - 1, self.get_height() - 1)
    }
}

impl Screen for PaletteScreenBuffer {
    fn ice_mode(&self) -> IceMode {
        self.ice_mode
    }

    fn terminal_state(&self) -> &TerminalState {
        &self.terminal_state
    }

    fn render_to_rgba(&self, _options: &RenderOptions) -> (Size, Vec<u8>) {
        // Screen is already in RGBA format, return a
        let mut pixels = Vec::with_capacity(self.pixel_size.width as usize * self.pixel_size.height as usize * 4);
        let pal = self.palette().clone();
        for i in &self.screen {
            let (r, g, b) = pal.get_rgb(*i as u32);
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            pixels.push(255);
        }
        (self.pixel_size, pixels)
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

    fn get_font_dimensions(&self) -> Size {
        if let Some(font) = self.get_font(0) { font.size() } else { Size::new(8, 16) }
    }

    fn get_font(&self, font_number: usize) -> Option<&BitFont> {
        self.font_table.get(&font_number)
    }

    fn font_count(&self) -> usize {
        self.font_table.len()
    }

    fn line_count(&self) -> usize {
        self.layer.lines.len()
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

    fn set_selection(&mut self, _selection: Selection) -> EngineResult<()> {
        Ok(())
    }

    fn clear_selection(&mut self) -> EngineResult<()> {
        Ok(())
    }

    fn hyperlinks(&self) -> &Vec<HyperLink> {
        &self.hyperlinks
    }

    fn update_hyperlinks(&mut self) {
        // No-op for now
    }

    fn to_bytes(&mut self, _file_name: &str, _options: &SaveOptions) -> EngineResult<Vec<u8>> {
        // Return empty for now, could implement PNG export later
        Ok(Vec::new())
    }

    fn get_copy_text(&self) -> Option<String> {
        None
    }

    fn get_copy_rich_text(&self) -> Option<String> {
        None
    }

    fn get_clipboard_data(&self) -> Option<Vec<u8>> {
        None
    }
    fn mouse_fields(&self) -> &Vec<MouseField> {
        &self.mouse_fields
    }
}

impl RgbaScreen for PaletteScreenBuffer {
    fn get_resolution(&self) -> Size {
        self.pixel_size
    }

    fn set_resolution(&mut self, size: Size) {
        self.pixel_size = size;
        let size = self.get_font(0).unwrap().size();
        self.char_screen_size = Size::new(size.width / size.width, size.height / size.height);
        self.screen.resize((size.width as usize) * (size.height as usize), 0);
    }

    fn screen(&self) -> &[u8] {
        &self.screen
    }

    fn screen_mut(&mut self) -> &mut Vec<u8> {
        &mut self.screen
    }
}

impl EditableScreen for PaletteScreenBuffer {
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

    fn insert_line(&mut self, line: usize, new_line: Line) {
        if line <= self.layer.lines.len() {
            self.layer.lines.insert(line, new_line);
        }
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
        self.layer.set_size(size);
        self.layer.lines.resize_with(size.height as usize, Line::new);
    }

    fn stop_sixel_threads(&mut self) {
        // No-op for this implementation
    }

    fn push_sixel_thread(&mut self, _thread: JoinHandle<EngineResult<crate::Sixel>>) {
        // No-op for this implementation
    }

    fn sixel_threads_runnning(&self) -> bool {
        false
    }

    fn update_sixel_threads(&mut self) -> EngineResult<bool> {
        Ok(false)
    }

    fn scroll_up(&mut self) {
        if !self.layer.lines.is_empty() {
            self.layer.lines.remove(0);
            self.layer.lines.push(Line::new());
        }
    }

    fn scroll_down(&mut self) {
        if !self.layer.lines.is_empty() {
            self.layer.lines.pop();
            self.layer.lines.insert(0, Line::new());
        }
    }

    fn scroll_left(&mut self) {
        for line in &mut self.layer.lines {
            if !line.chars.is_empty() {
                line.chars.remove(0);
            }
        }
    }

    fn scroll_right(&mut self) {
        for line in &mut self.layer.lines {
            line.chars.insert(0, AttributedChar::default());
        }
    }

    fn clear_screen(&mut self) {
        self.caret_mut().set_position(Position::default());
        self.stop_sixel_threads();
        self.layer.clear();
        self.terminal_state_mut().cleared_screen = true;

        // Clear text layer
        for line in &mut self.layer.lines {
            line.chars.clear();
        }

        // Clear pixel buffer
        self.screen.fill(0);
        self.mark_dirty();
    }

    fn clear_scrollback(&mut self) {
        // No scrollback in this implementation
    }

    fn get_max_scrollback_offset(&self) -> usize {
        0
    }

    fn scrollback_position(&self) -> usize {
        0
    }

    fn set_scroll_position(&mut self, _position: usize) {
        // No-op, no scrollback
    }

    fn remove_terminal_line(&mut self, line: i32) {
        if line >= 0 && (line as usize) < self.layer.lines.len() {
            self.layer.lines.remove(line as usize);
        }
    }

    fn insert_terminal_line(&mut self, line: i32) {
        if line >= 0 && (line as usize) <= self.layer.lines.len() {
            self.layer.lines.insert(line as usize, Line::new());
        }
    }

    fn set_char(&mut self, pos: Position, ch: AttributedChar) {
        if pos.x < 0 || pos.y < 0 || pos.x >= self.char_screen_size.width || pos.y >= self.char_screen_size.height {
            return;
        }
        let y = pos.y as usize;
        let x = pos.x as usize;

        // Ensure line exists
        while y >= self.layer.lines.len() {
            self.layer.lines.push(Line::new());
        }

        let line = &mut self.layer.lines[y];

        // Ensure line has enough chars
        while x >= line.chars.len() {
            line.chars.push(AttributedChar::default());
        }

        // Store in text layer
        line.chars[x] = ch;

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

        // Resize text layer
        self.layer.lines.resize_with(height as usize, Line::new);

        // Update screen size
        self.char_screen_size.height = height;
    }

    fn add_hyperlink(&mut self, hyperlink: HyperLink) {
        self.hyperlinks.push(hyperlink);
    }

    fn get_version(&self) -> u64 {
        self.buffer_version.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn is_dirty(&self) -> bool {
        self.buffer_dirty.load(std::sync::atomic::Ordering::Acquire)
    }

    fn clear_dirty(&self) {
        self.buffer_dirty.store(false, std::sync::atomic::Ordering::Release)
    }

    fn mark_dirty(&self) {
        self.buffer_dirty.store(true, std::sync::atomic::Ordering::Release);
        self.buffer_version.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}
