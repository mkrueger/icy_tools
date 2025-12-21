use crate::{
    AttributedChar, BitFont, Caret, HyperLink, IceMode, Palette, Position, Rectangle, RenderOptions, Result, SaveOptions, Screen, Selection, SelectionMask,
    Size, TerminalState, TextPane, bgi::MouseField,
};

/// Render a region from (0,0) with specified height for scrollback buffer.
/// This is a helper function to avoid code duplication across screen buffer types.
pub fn render_scrollback_region(screen: &dyn Screen, height: i32) -> (Size, Vec<u8>) {
    let region = Rectangle::from(0, 0, screen.resolution().width, height);
    let mut opt = RenderOptions::default();
    opt.override_scan_lines = Some(false);
    screen.render_region_to_rgba(region, &opt)
}

#[derive(Clone, Default)]
pub struct ScrollbackChunk {
    pub rgba_data: Vec<u8>,
    pub size: Size,
}

#[derive(Clone, Default)]
pub struct ScrollbackBuffer {
    buffer_size: usize,

    pub chunks: Vec<ScrollbackChunk>,
    pub cur_screen: ScrollbackChunk,
    /// Current screen size in characters.
    pub cur_screen_size: Size,

    pub font_dimensions: Size,
    pub palette: Palette,
    pub selection: Option<Selection>,
    pub selection_mask: SelectionMask,
    pub terminal_state: TerminalState,
    pub caret: Caret,
    pub version: u64,
    pub scan_lines: bool,
}

impl ScrollbackBuffer {
    pub fn new() -> Self {
        ScrollbackBuffer {
            chunks: Vec::new(),
            cur_screen: ScrollbackChunk {
                rgba_data: Vec::new(),
                size: Size { width: 0, height: 0 },
            },
            font_dimensions: Size::new(8, 16),
            palette: Palette::default(),
            selection: None,
            selection_mask: SelectionMask::default(),
            terminal_state: TerminalState::from(Size::new(80, 25)),
            caret: Caret::default(),
            version: 0,
            cur_screen_size: Size::new(0, 0),
            scan_lines: false,
            buffer_size: 2000,
        }
    }

    pub fn set_buffer_size(&mut self, buffer_size: usize) {
        self.buffer_size = buffer_size;
    }

    pub fn add_chunk(&mut self, rgba_data: Vec<u8>, size: Size) {
        if rgba_data.is_empty() {
            return;
        }
        let chunk = ScrollbackChunk { rgba_data, size };
        self.chunks.push(chunk);
        if self.chunks.len() > self.buffer_size {
            self.chunks.remove(0);
        }
    }

    pub fn clear(&mut self) {
        self.chunks.clear();
    }

    pub fn snapshot_current_screen(&mut self, screen: &dyn Screen) {
        let mut opt = RenderOptions::default();
        opt.override_scan_lines = Some(false);

        let (size, rgba_data) = screen.render_region_to_rgba(Rectangle::new(Position::new(0, 0), screen.resolution()), &opt);

        self.cur_screen = ScrollbackChunk { rgba_data, size };

        // Inherit properties from the screen being snapshotted
        self.scan_lines = screen.scan_lines();

        self.cur_screen_size = screen.size();
        self.font_dimensions = screen.font_dimensions();
        self.palette = screen.palette().clone();
        self.terminal_state = screen.terminal_state().clone();
    }

    fn total_height(&self) -> i32 {
        self.chunks.iter().map(|c| c.size.height).sum::<i32>() + self.cur_screen.size.height
    }
}

impl TextPane for ScrollbackBuffer {
    fn char_at(&self, _pos: Position) -> AttributedChar {
        // ScrollbackBuffer doesn't have character-level access, return default
        AttributedChar::default()
    }

    fn line_count(&self) -> i32 {
        self.total_height() / self.font_dimensions.height
    }

    fn width(&self) -> i32 {
        self.cur_screen_size.width / self.font_dimensions.width
    }

    fn height(&self) -> i32 {
        self.line_count()
    }

    fn size(&self) -> Size {
        Size::new(self.width(), self.height())
    }

    fn line_length(&self, _line: i32) -> i32 {
        self.width()
    }

    fn rectangle(&self) -> Rectangle {
        Rectangle::from(0, 0, self.width(), self.height())
    }
}

impl Screen for ScrollbackBuffer {
    fn buffer_type(&self) -> crate::BufferType {
        crate::BufferType::CP437
    }

    fn resolution(&self) -> Size {
        // Resolution is the size of the current visible screen in pixels (not including scrollback)
        // cur_screen.size is already in pixels from render_region_to_rgba
        let mut h = self.cur_screen.size.height;
        if self.scan_lines {
            h *= 2;
        }
        Size::new(self.cur_screen.size.width, h)
    }

    fn virtual_size(&self) -> Size {
        // Virtual size includes all scrollback chunks + current screen in pixels
        // chunk.size is already in pixels from render_region_to_rgba
        let width = if self.cur_screen.size.width > 0 {
            self.cur_screen.size.width
        } else if let Some(first_chunk) = self.chunks.first() {
            first_chunk.size.width
        } else {
            0
        };
        let mut height = self.total_height();
        if self.scan_lines {
            height *= 2;
        }

        Size::new(width, height)
    }

    fn font_dimensions(&self) -> Size {
        self.font_dimensions
    }
    fn set_font_dimensions(&mut self, size: Size) {
        self.font_dimensions = size;
    }

    fn scan_lines(&self) -> bool {
        self.scan_lines
    }

    fn render_region_to_rgba(&self, mut px_region: Rectangle, _options: &RenderOptions) -> (Size, Vec<u8>) {
        if self.scan_lines {
            px_region.start.y /= 2;
            px_region.size.height /= 2;
        }

        // Target width is always the current screen width
        let target_width = self.cur_screen.size.width.max(1);
        let total_height = self.total_height();

        // Clamp region to valid bounds
        let x = px_region.start.x.max(0).min(target_width);
        let y = px_region.start.y.max(0).min(total_height);
        let region_width: i32 = px_region.size.width.max(0).min(target_width - x);
        let region_height = px_region.size.height.max(0).min(total_height - y);

        // Early exit for empty region
        if region_width <= 0 || region_height <= 0 {
            return (Size::new(0, 0), Vec::new());
        }

        // Output buffer for the requested region
        let mut region_data = Vec::with_capacity((region_width * region_height * 4) as usize);

        let mut current_y = 0;

        // Helper function to scale a line horizontally
        let scale_line = |src_data: &[u8], src_width: i32, dst_width: i32, x_offset: i32, copy_width: i32| -> Vec<u8> {
            let mut output = Vec::with_capacity((copy_width * 4) as usize);

            if src_width == dst_width {
                // No scaling needed - direct copy
                let src_offset = (x_offset * 4) as usize;
                let src_end = src_offset + (copy_width * 4) as usize;
                if src_end <= src_data.len() {
                    output.extend_from_slice(&src_data[src_offset..src_end]);
                }
            } else {
                // Scaling needed - use nearest neighbor sampling
                let scale_factor = src_width as f32 / dst_width as f32;

                for dst_x in x_offset..(x_offset + copy_width) {
                    // Map destination x to source x
                    let src_x = (dst_x as f32 * scale_factor) as i32;
                    let src_x = src_x.min(src_width - 1).max(0);

                    let src_pixel_offset = (src_x * 4) as usize;
                    if src_pixel_offset + 4 <= src_data.len() {
                        output.extend_from_slice(&src_data[src_pixel_offset..src_pixel_offset + 4]);
                    } else {
                        // Fallback to black pixel
                        output.extend_from_slice(&[0, 0, 0, 255]);
                    }
                }
            }

            output
        };

        // Process scrollback chunks
        for chunk in &self.chunks {
            let chunk_height = chunk.size.height;
            let chunk_width = chunk.size.width;

            // Skip chunks completely above the region
            if current_y + chunk_height <= y {
                current_y += chunk_height;
                continue;
            }

            // Stop if we've passed the region
            if current_y >= y + region_height {
                break;
            }

            // Extract lines from this chunk that overlap with the region
            let start_line = (y - current_y).max(0);
            let end_line = (y + region_height - current_y).min(chunk_height);

            for line in start_line..end_line {
                // Get the source line data
                let src_line_offset = (line * chunk_width) as usize * 4;
                let src_line_end = src_line_offset + (chunk_width as usize * 4);

                if src_line_end <= chunk.rgba_data.len() {
                    let src_line_data = &chunk.rgba_data[src_line_offset..src_line_end];

                    // Scale the line if needed and extract the requested region
                    let scaled_line = scale_line(src_line_data, chunk_width, target_width, x, region_width);

                    region_data.extend_from_slice(&scaled_line);
                } else {
                    // Fill with black if data is missing
                    region_data.extend(vec![0u8; (region_width * 4) as usize]);
                }
            }

            current_y += chunk_height;
        }

        // Process current screen if needed
        if current_y < y + region_height && !self.cur_screen.rgba_data.is_empty() {
            let screen_height = self.cur_screen.size.height;
            let screen_width = self.cur_screen.size.width;
            let start_line = (y - current_y).max(0);
            let end_line = (y + region_height - current_y).min(screen_height);

            for line in start_line..end_line {
                let src_line_offset = (line * screen_width) as usize * 4;
                let src_line_end = src_line_offset + (screen_width as usize * 4);

                if src_line_end <= self.cur_screen.rgba_data.len() {
                    let src_line_data = &self.cur_screen.rgba_data[src_line_offset..src_line_end];

                    // Current screen should already be at target width, but check anyway
                    let scaled_line = scale_line(src_line_data, screen_width, target_width, x, region_width);

                    region_data.extend_from_slice(&scaled_line);
                } else {
                    // Fill with black if data is missing
                    region_data.extend(vec![0u8; (region_width * 4) as usize]);
                }
            }
        }

        // Apply scan_lines if needed (double the height)
        if self.scan_lines {
            let mut doubled_data = Vec::with_capacity(region_data.len() * 2);
            let line_size = (region_width * 4) as usize;

            // Process each line
            for line_idx in 0..(region_height as usize) {
                let line_start = line_idx * line_size;
                let line_end = line_start + line_size;

                if line_end <= region_data.len() {
                    let line = &region_data[line_start..line_end];
                    // Add the line twice for scan_lines effect
                    doubled_data.extend_from_slice(line);
                    doubled_data.extend_from_slice(line);
                }
            }

            return (Size::new(region_width, region_height * 2), doubled_data);
        }

        (Size::new(region_width, region_height), region_data)
    }

    fn render_to_rgba(&self, options: &RenderOptions) -> (Size, Vec<u8>) {
        self.render_region_to_rgba(Rectangle::from_min_size((0, 0), self.resolution()), options)
    }

    fn palette(&self) -> &Palette {
        &self.palette
    }

    fn ice_mode(&self) -> IceMode {
        IceMode::Unlimited
    }

    fn font(&self, _font_number: usize) -> Option<&BitFont> {
        None
    }

    fn font_count(&self) -> usize {
        0
    }

    fn version(&self) -> u64 {
        self.version
    }

    fn default_foreground_color(&self) -> u32 {
        7
    }

    fn max_base_colors(&self) -> u32 {
        16
    }

    fn hyperlinks(&self) -> &Vec<HyperLink> {
        const EMPTY: &Vec<HyperLink> = &Vec::new();
        EMPTY
    }

    fn mouse_fields(&self) -> &Vec<MouseField> {
        const EMPTY: &Vec<MouseField> = &Vec::new();
        EMPTY
    }

    fn selection(&self) -> Option<Selection> {
        self.selection
    }

    fn selection_mask(&self) -> &SelectionMask {
        &self.selection_mask
    }

    fn set_selection(&mut self, sel: Selection) -> Result<()> {
        // Only increment version if selection actually changed
        if self.selection.as_ref() != Some(&sel) {
            self.selection = Some(sel);
            self.version += 1;
        }
        Ok(())
    }

    fn clear_selection(&mut self) -> Result<()> {
        // Only increment version if there was a selection to clear
        if self.selection.is_some() {
            self.selection = None;
            self.version += 1;
        }
        Ok(())
    }

    fn terminal_state(&self) -> &TerminalState {
        &self.terminal_state
    }

    fn caret(&self) -> &Caret {
        &self.caret
    }

    fn to_bytes(&mut self, _extension: &str, _options: &SaveOptions) -> Result<Vec<u8>> {
        // ScrollbackBuffer doesn't support saving
        Ok(Vec::new())
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn screen(&self) -> &[u8] {
        &self.cur_screen.rgba_data
    }

    fn clone_box(&self) -> Box<dyn Screen> {
        Box::new(self.clone())
    }

    fn set_scrollback_buffer_size(&mut self, _buffer_size: usize) {}
}

/// Helper method to render a region from a screen and add it to the scrollback buffer.
/// Always starts at x=0, renders full width with the specified height.
pub fn add_screen_region(buffer: &mut ScrollbackBuffer, screen: &dyn Screen, height: i32) {
    let region = Rectangle::from(0, 0, screen.resolution().width, height);
    let mut opt = RenderOptions::default();
    opt.override_scan_lines = Some(false);
    let (size, rgba_data) = screen.render_region_to_rgba(region, &opt);
    buffer.add_chunk(rgba_data, size);
}
