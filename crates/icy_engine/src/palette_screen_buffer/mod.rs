use crate::{
    AttributedChar, BitFont, BufferType, Caret, DOS_DEFAULT_PALETTE, EditableScreen, EngineResult, HyperLink, IceMode, Layer, Line, Palette, Position,
    Rectangle, RenderOptions, RgbaScreen, SaveOptions, Screen, Selection, SelectionMask, Size, TerminalState, TextPane,
};
use std::thread::JoinHandle;

pub struct PaletteScreenBuffer {
    pub resolution: Size,
    pub screen: Vec<u8>,

    // Text layer for char storage and compatibility
    layer: Layer,

    // Rendering properties
    font: BitFont,
    palette: Palette,
    caret: Caret,
    ice_mode: IceMode,
    terminal_state: TerminalState,
    buffer_type: BufferType,
    hyperlinks: Vec<HyperLink>,
    selection_mask: SelectionMask,

    // Font dimensions in pixels
    char_width: usize,
    char_height: usize,
}

impl PaletteScreenBuffer {
    pub fn new(px_width: i32, px_height: i32, font: BitFont) -> Self {
        let screen_size = Size::new(px_width, px_height);
        let char_width = 8; // Default DOS font width
        let char_height = 16; // Default DOS font height

        // Calculate pixel dimensions
        let pixel_width = px_width as usize * char_width;
        let pixel_height = px_height as usize * char_height;
        let screen = vec![0u8; pixel_width * pixel_height];

        // Create text layer
        let mut layer = Layer::new("", screen_size);
        layer.lines.clear();
        for _ in 0..px_height {
            layer.lines.push(Line::new());
        }
        let font_size = font.size;
        Self {
            resolution: screen_size,
            screen,
            layer,
            font,
            palette: Palette::from_slice(&DOS_DEFAULT_PALETTE),
            caret: Caret::default(),
            ice_mode: IceMode::Unlimited,
            terminal_state: TerminalState::from((px_width / font_size.width, px_height / font_size.height)),
            buffer_type: BufferType::CP437,
            hyperlinks: Vec::new(),
            selection_mask: SelectionMask::default(),
            char_width,
            char_height,
        }
    }

    pub fn with_font(mut self, font: BitFont) -> Self {
        self.font = font;
        self.char_width = self.font.size.width as usize;
        self.char_height = self.font.size.height as usize;

        // Resize pixel buffer
        let pixel_width = self.resolution.width as usize * self.char_width;
        let pixel_height = self.resolution.height as usize * self.char_height;
        self.screen = vec![0u8; pixel_width * pixel_height * 4];

        self
    }

    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Render a character directly to the RGBA buffer
    fn render_char_to_buffer(&mut self, pos: Position, ch: AttributedChar) {
        if pos.x < 0 || pos.y < 0 || pos.x >= self.resolution.width || pos.y >= self.resolution.height {
            return;
        }

        let x = pos.x as usize;
        let y = pos.y as usize;

        // Get colors from palette
        let fg_idx = ch.attribute.get_foreground() as u32;
        let bg_idx = ch.attribute.get_background() as u32;

        let fg_color = self.palette.get_color(fg_idx);
        let bg_color = self.palette.get_color(bg_idx);

        // Calculate pixel position
        let pixel_x = x * self.char_width;
        let pixel_y = y * self.char_height;

        // Get glyph data from font
        let glyph = self.font.get_glyph(ch.ch);

        let pixel_width = self.resolution.width as usize * self.char_width;

        // Render the character
        for row in 0..self.char_height {
            for col in 0..self.char_width {
                let px = pixel_x + col;
                let py = pixel_y + row;

                if px >= pixel_width || py >= (self.resolution.height as usize * self.char_height) {
                    continue;
                }

                // Check if pixel is set in font glyph
                let is_foreground = if let Some(g) = glyph {
                    if row < g.data.len() * 8 / self.char_width {
                        let byte_idx = (row * self.char_width + col) / 8;
                        let bit_idx = 7 - ((row * self.char_width + col) % 8);

                        if byte_idx < g.data.len() {
                            (g.data[byte_idx] >> bit_idx) & 1 == 1
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                // Clone colors to avoid move in loop
                let color = if is_foreground { fg_color.clone() } else { bg_color.clone() };

                // Write to RGBA buffer
                let offset = (py * pixel_width + px) * 4;
                if offset + 3 < self.screen.len() {
                    self.screen[offset] = color.r;
                    self.screen[offset + 1] = color.g;
                    self.screen[offset + 2] = color.b;
                    self.screen[offset + 3] = 255; // Full opacity
                }
            }
        }
    }

    pub fn clear(&mut self) {
        // Clear text layer
        for line in &mut self.layer.lines {
            line.chars.clear();
        }

        // Clear pixel buffer
        self.screen.fill(0);
    }

    pub fn get_pixel_dimensions(&self) -> (usize, usize) {
        let width = self.resolution.width as usize * self.char_width;
        let height = self.resolution.height as usize * self.char_height;
        (width, height)
    }
}

impl TextPane for PaletteScreenBuffer {
    fn get_char(&self, pos: Position) -> AttributedChar {
        if pos.x < 0 || pos.y < 0 || pos.x >= self.resolution.width || pos.y >= self.resolution.height {
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
        let w = self.resolution.width;
        let h = self.resolution.height;
        let font = self.get_font_dimensions();

        Size::new(w / font.width, h / font.height)
    }

    fn get_line_count(&self) -> i32 {
        self.get_height()
    }

    fn get_width(&self) -> i32 {
        let w = self.resolution.width;
        let font = self.get_font_dimensions();
        w / font.width
    }

    fn get_height(&self) -> i32 {
        let h = self.resolution.height;
        let font = self.get_font_dimensions();
        h / font.height
    }

    fn get_line_length(&self, line: i32) -> i32 {
        if line < 0 || line >= self.resolution.height {
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
        let mut pixels = Vec::new();
        let pal = self.palette().clone();
        for i in &self.screen {
            if *i == 0 {
                pixels.push(0);
                pixels.push(0);
                pixels.push(0);
                pixels.push(0);
                continue;
            }
            let (r, g, b) = pal.get_rgb(*i as u32);
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            pixels.push(255);
        }
        (self.get_size(), pixels)
    }

    fn get_first_visible_line(&self) -> i32 {
        0
    }

    fn get_last_visible_line(&self) -> i32 {
        self.resolution.height - 1
    }

    fn get_first_editable_line(&self) -> i32 {
        0
    }

    fn get_last_editable_line(&self) -> i32 {
        self.resolution.height - 1
    }

    fn get_first_editable_column(&self) -> i32 {
        0
    }

    fn get_last_editable_column(&self) -> i32 {
        self.resolution.width - 1
    }

    fn get_font_dimensions(&self) -> Size {
        Size::new(self.char_width as i32, self.char_height as i32)
    }

    fn get_font(&self, _font_idx: usize) -> Option<&BitFont> {
        Some(&self.font)
    }

    fn font_count(&self) -> usize {
        1
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
}

impl RgbaScreen for PaletteScreenBuffer {
    fn get_resolution(&self) -> Size {
        self.resolution
    }

    fn screen_mut(&mut self) -> &mut [u8] {
        &mut self.screen
    }
}

impl EditableScreen for PaletteScreenBuffer {
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
        self.terminal_state = TerminalState::from(self.get_size());
    }

    fn insert_line(&mut self, line: usize, new_line: Line) {
        if line <= self.layer.lines.len() {
            self.layer.lines.insert(line, new_line);
        }
    }

    fn set_font(&mut self, _font_idx: usize, font: BitFont) {
        self.font = font;
        self.char_width = self.font.size.width as usize;
        self.char_height = self.font.size.height as usize;
    }

    fn remove_font(&mut self, _font_idx: usize) -> Option<BitFont> {
        None // Only one font supported
    }

    fn clear_font_table(&mut self) {
        // No-op, only one font supported
    }

    fn set_size(&mut self, size: Size) {
        self.resolution = size;

        // Resize pixel buffer
        let pixel_width = size.width as usize * self.char_width;
        let pixel_height = size.height as usize * self.char_height;
        self.screen.resize(pixel_width * pixel_height * 4, 0);

        // Resize text layer
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
        self.clear();
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
        if pos.x < 0 || pos.y < 0 || pos.x >= self.resolution.width || pos.y >= self.resolution.height {
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
    }

    fn print_char(&mut self, ch: AttributedChar) {
        let pos = Position::new(self.caret.x, self.caret.y);

        self.set_char(pos, ch);

        // Advance caret
        self.caret.x = self.caret.x + 1;

        // Handle line wrap
        if self.caret.x >= self.resolution.width {
            self.caret.x = 0;
            self.caret.y = self.caret.y + 1;
        }
    }

    fn set_height(&mut self, height: i32) {
        let height = height.max(1);

        // Resize text layer
        self.layer.lines.resize_with(height as usize, Line::new);

        // Update screen size
        self.resolution.height = height;

        // Resize RGBA buffer
        let pixel_width = self.resolution.width as usize * self.char_width;
        let pixel_height = height as usize * self.char_height;
        self.screen.resize(pixel_width * pixel_height * 4, 0);
    }

    fn add_hyperlink(&mut self, hyperlink: HyperLink) {
        self.hyperlinks.push(hyperlink);
    }
}
