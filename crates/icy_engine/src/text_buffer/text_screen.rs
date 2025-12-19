use core::panic;
use parking_lot::Mutex;
use std::{sync::Arc, u32};

use icy_parser_core::{RipCommand, SkypixCommand};

use crate::{
    AnsiSaveOptionsV2, AttributedChar, BitFont, Caret, EditableScreen, HyperLink, IceMode, Layer, Line, Palette, Position, Rectangle, RenderOptions, Result,
    SavedCaretState, Screen, ScrollbackBuffer, Selection, SelectionMask, Sixel, Size, TerminalState, TextBuffer, TextPane, bgi::MouseField, clipboard, limits,
};

#[derive(Clone, Default)]
pub struct TextScreen {
    pub caret: Caret,
    pub buffer: TextBuffer,

    pub current_layer: usize,

    selection_opt: Option<Selection>,
    pub mouse_fields: Vec<MouseField>,

    pub saved_caret_pos: Position,
    pub saved_caret_state: SavedCaretState,
    pub scan_lines: bool,

    pub scrollback_buffer: ScrollbackBuffer,
}

impl TextScreen {
    pub fn new(size: impl Into<Size>) -> Self {
        Self {
            caret: Caret::default(),
            buffer: TextBuffer::new(size),
            current_layer: 0,
            selection_opt: None,
            mouse_fields: Vec::new(),
            saved_caret_pos: Position::default(),
            saved_caret_state: SavedCaretState::default(),
            scan_lines: false,
            scrollback_buffer: ScrollbackBuffer::new(),
        }
    }

    pub fn from_buffer(buffer: TextBuffer) -> Self {
        Self {
            caret: Caret::default(),
            buffer,
            current_layer: 0,
            selection_opt: None,
            mouse_fields: Vec::new(),
            saved_caret_pos: Position::default(),
            saved_caret_state: SavedCaretState::default(),
            scan_lines: false,
            scrollback_buffer: ScrollbackBuffer::new(),
        }
    }
}

impl TextPane for TextScreen {
    fn char_at(&self, pos: crate::Position) -> AttributedChar {
        self.buffer.char_at(pos)
    }

    fn line_count(&self) -> i32 {
        self.buffer.line_count()
    }

    fn width(&self) -> i32 {
        self.buffer.width()
    }

    fn height(&self) -> i32 {
        self.buffer.height()
    }

    fn line_length(&self, line: i32) -> i32 {
        self.buffer.line_length(line)
    }

    fn rectangle(&self) -> crate::Rectangle {
        self.buffer.rectangle()
    }

    fn size(&self) -> Size {
        self.buffer.size()
    }
}

impl Screen for TextScreen {
    fn buffer_type(&self) -> crate::BufferType {
        self.buffer.buffer_type
    }

    fn use_letter_spacing(&self) -> bool {
        self.buffer.use_letter_spacing()
    }

    fn use_aspect_ratio(&self) -> bool {
        self.buffer.use_aspect_ratio()
    }

    fn scan_lines(&self) -> bool {
        self.scan_lines
    }

    fn ice_mode(&self) -> IceMode {
        self.buffer.ice_mode
    }

    fn caret(&self) -> &Caret {
        &self.caret
    }

    fn terminal_state(&self) -> &TerminalState {
        &self.buffer.terminal_state
    }

    fn palette(&self) -> &Palette {
        &self.buffer.palette
    }

    fn render_to_rgba(&self, options: &RenderOptions) -> (Size, Vec<u8>) {
        self.buffer.render_to_rgba(options, self.scan_lines)
    }

    fn render_region_to_rgba(&self, px_region: Rectangle, options: &RenderOptions) -> (Size, Vec<u8>) {
        self.buffer.render_region_to_rgba(px_region, options, self.scan_lines)
    }

    fn font(&self, font_number: usize) -> Option<&BitFont> {
        self.buffer.font(font_number)
    }

    fn font_count(&self) -> usize {
        self.buffer.font_count()
    }

    fn font_dimensions(&self) -> Size {
        let dims = self.buffer.font_dimensions();
        if self.use_letter_spacing() && dims.width == 8 {
            Size::new(9, dims.height)
        } else {
            dims
        }
    }

    fn selection(&self) -> Option<Selection> {
        self.selection_opt
    }

    fn selection_mask(&self) -> &crate::SelectionMask {
        // Selection mask now managed by EditState
        static EMPTY_MASK: std::sync::OnceLock<SelectionMask> = std::sync::OnceLock::new();
        EMPTY_MASK.get_or_init(SelectionMask::default)
    }

    fn hyperlinks(&self) -> &Vec<HyperLink> {
        &self.buffer.layers[self.current_layer].hyperlinks
    }

    fn to_bytes(&mut self, extension: &str, options: &AnsiSaveOptionsV2) -> Result<Vec<u8>> {
        let extension = extension.to_ascii_lowercase();
        if let Some(format) = crate::formats::FileFormat::from_extension(&extension) {
            format.to_bytes(&self.buffer, options)
        } else {
            Err(crate::EngineError::UnsupportedFormat {
                description: format!("Unknown format: {}", extension),
            })
        }
    }

    fn copy_text(&self) -> Option<String> {
        let Some(selection) = &self.selection_opt else {
            return None;
        };
        clipboard::text(&self.buffer, self.buffer.buffer_type, selection)
    }

    fn copy_rich_text(&self) -> Option<String> {
        let Some(selection) = &self.selection_opt else {
            return None;
        };
        clipboard::get_rich_text(&self.buffer, selection)
    }

    fn clipboard_data(&self) -> Option<Vec<u8>> {
        clipboard::clipboard_data(&self.buffer, self.current_layer, self.selection_mask(), &self.selection_opt)
    }

    fn mouse_fields(&self) -> &Vec<MouseField> {
        &self.mouse_fields
    }

    fn version(&self) -> u64 {
        self.buffer.version()
    }

    fn default_foreground_color(&self) -> u32 {
        7
    }

    fn max_base_colors(&self) -> u32 {
        u32::MAX
    }

    fn resolution(&self) -> Size {
        let font_size = self.font_dimensions();
        let rect = self.terminal_state().size();
        let px_width = rect.width * font_size.width;
        let px_height = rect.height * font_size.height;
        Size::new(px_width, px_height)
    }

    fn virtual_size(&self) -> Size {
        let font_size = self.font_dimensions();
        let rect = self.buffer.size();
        let px_width = rect.width * font_size.width;
        let px_height = rect.height * font_size.height;
        Size::new(px_width, px_height)
    }

    fn screen(&self) -> &[u8] {
        panic!("Not supported for TextScreen");
    }

    fn set_scrollback_buffer_size(&mut self, buffer_size: usize) {
        self.scrollback_buffer.set_buffer_size(buffer_size);
    }

    fn set_selection(&mut self, sel: Selection) -> Result<()> {
        // Only mark dirty if selection actually changed
        if self.selection_opt.as_ref() != Some(&sel) {
            self.selection_opt = Some(sel);
            self.mark_dirty();
        }
        Ok(())
    }

    fn clear_selection(&mut self) -> Result<()> {
        // Only mark dirty if there was a selection to clear
        if self.selection_opt.is_some() {
            self.selection_opt = None;
            self.mark_dirty();
        }
        Ok(())
    }

    fn as_editable(&mut self) -> Option<&mut dyn EditableScreen> {
        Some(self)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Screen> {
        Box::new(self.clone())
    }
}

impl EditableScreen for TextScreen {
    fn snapshot_scrollback(&mut self) -> Option<Arc<Mutex<Box<dyn Screen>>>> {
        let mut scrollback = self.scrollback_buffer.clone();
        scrollback.snapshot_current_screen(self);
        return Some(Arc::new(Mutex::new(Box::new(scrollback))));
    }

    fn first_visible_line(&self) -> i32 {
        self.buffer.first_visible_line()
    }

    fn last_visible_line(&self) -> i32 {
        self.buffer.last_visible_line()
    }

    fn first_editable_line(&self) -> i32 {
        self.buffer.first_editable_line()
    }

    fn last_editable_line(&self) -> i32 {
        self.buffer.last_editable_line()
    }

    fn first_editable_column(&self) -> i32 {
        self.buffer.first_editable_column()
    }

    fn last_editable_column(&self) -> i32 {
        self.buffer.last_editable_column()
    }

    fn get_line(&self, line: usize) -> Option<&Line> {
        self.buffer.layers[self.current_layer].lines.get(line)
    }

    fn physical_line_count(&self) -> usize {
        self.buffer.line_count() as usize
    }

    fn set_resolution(&mut self, _size: Size) {
        panic!("Not supported for TextScreen");
    }

    fn screen_mut(&mut self) -> &mut Vec<u8> {
        panic!("Not supported for TextScreen");
    }

    fn set_graphics_type(&mut self, _graphics_type: crate::GraphicsType) {
        panic!("Not supported for TextScreen");
    }

    fn update_hyperlinks(&mut self) {
        self.buffer.update_hyperlinks();
    }

    fn clear_line(&mut self) {
        let line = self.caret.position().y;
        if let Some(l) = self.buffer.layers[self.current_layer].lines.get_mut(line as usize) {
            l.chars.clear();
        }
    }

    fn clear_line_end(&mut self) {
        let pos = self.caret.position();
        if let Some(l) = self.buffer.layers[self.current_layer].lines.get_mut(pos.y as usize) {
            l.chars.truncate(pos.x as usize);
        }
    }

    fn clear_line_start(&mut self) {
        let pos = self.caret.position();
        if let Some(l) = self.buffer.layers[self.current_layer].lines.get_mut(pos.y as usize) {
            for i in 0..pos.x.min(l.chars.len() as i32) {
                l.chars[i as usize] = AttributedChar::default();
            }
        }
    }
    fn clear_mouse_fields(&mut self) {
        self.mouse_fields.clear();
    }

    fn add_mouse_field(&mut self, mouse_field: MouseField) {
        self.mouse_fields.push(mouse_field);
    }

    fn ice_mode_mut(&mut self) -> &mut IceMode {
        &mut self.buffer.ice_mode
    }

    fn buffer_type_mut(&mut self) -> &mut crate::BufferType {
        &mut self.buffer.buffer_type
    }

    fn caret_mut(&mut self) -> &mut Caret {
        &mut self.caret
    }

    fn palette_mut(&mut self) -> &mut Palette {
        &mut self.buffer.palette
    }

    fn terminal_state_mut(&mut self) -> &mut TerminalState {
        &mut self.buffer.terminal_state
    }

    fn reset_terminal(&mut self) {
        self.buffer.reset_terminal();
        self.caret.reset();
    }

    fn set_char(&mut self, pos: Position, ch: AttributedChar) {
        self.buffer.layers[self.current_layer].set_char(pos, ch);
        self.buffer.mark_dirty();
    }

    fn set_size(&mut self, size: Size) {
        self.buffer.set_size(size);
        self.buffer.mark_dirty();
    }

    fn scroll_up(&mut self) {
        // Add top line to scrollback before scrolling (while data is still there)
        if self.terminal_state().margins_top_bottom().is_none() && self.terminal_state().is_terminal_buffer {
            let font_height = self.font_dimensions().height;
            let (size, rgba_data) = crate::scrollback_buffer::render_scrollback_region(self, font_height);
            self.scrollback_buffer.add_chunk(rgba_data, size);
        }

        let font_dims = self.font_dimensions();

        let start_line: i32 = self.first_editable_line();
        let end_line = self.last_editable_line();

        let start_column = self.first_editable_column();
        let end_column = self.last_editable_column();

        {
            let layer_ref = &mut self.buffer.layers[self.current_layer];
            for x in start_column..=end_column {
                (start_line..end_line).for_each(|y| {
                    let ch = layer_ref.char_at((x, y + 1).into());
                    layer_ref.set_char((x, y), ch);
                });
                layer_ref.set_char((x, end_line), AttributedChar::default());
            }
        }
        self.buffer.mark_dirty();

        let layer_ref = &mut self.buffer.layers[self.current_layer];

        let mut remove_indices: Vec<usize> = Vec::new();
        for (i, sixel) in layer_ref.sixels.iter_mut().enumerate() {
            let rect = sixel.as_rectangle(font_dims);
            let top = rect.start.y;

            if top == start_line {
                // This sixel's top row is being overwritten; drop it.
                remove_indices.push(i);
            } else if top > start_line && top <= end_line {
                // Shift upward one row in cell space.
                sixel.position.y -= 1;
            }
            // If top < start_line, it's above the editable region; leave it.
            // If top > end_line, outside affected scroll band; leave it.
        }

        // Remove in reverse to keep indices valid.
        for idx in remove_indices.into_iter().rev() {
            layer_ref.sixels.remove(idx);
        }
    }

    fn scroll_down(&mut self) {
        let font_dims = self.font_dimensions();

        let start_line: i32 = self.first_editable_line();
        let end_line = self.last_editable_line();

        let start_column = self.first_editable_column();
        let end_column = self.last_editable_column();

        let layer_ref = &mut self.buffer.layers[self.current_layer];
        // Shift character data downward
        for x in start_column..=end_column {
            ((start_line + 1)..=end_line).rev().for_each(|y: i32| {
                let ch = layer_ref.char_at((x, y - 1).into());
                layer_ref.set_char((x, y), ch);
            });
            layer_ref.set_char((x, start_line), AttributedChar::default());
        }

        // === NEW: vertical sixel scroll (down) ===
        let mut remove_indices: Vec<usize> = Vec::new();
        for (i, sixel) in layer_ref.sixels.iter_mut().enumerate() {
            let rect = sixel.as_rectangle(font_dims);
            let top = rect.start.y;
            let bottom = top + rect.size.height - 1;

            // Remove if its bottom row is being lost at end_line
            if bottom == end_line {
                remove_indices.push(i);
            } else if top >= start_line && bottom < end_line {
                // Fully within scroll band: move down one row
                sixel.position.y += 1;
            }
            // Else (partially outside band) leave unchanged
        }
        for idx in remove_indices.into_iter().rev() {
            layer_ref.sixels.remove(idx);
        }
    }

    fn scroll_left(&mut self) {
        let font_dims = self.font_dimensions();

        let start_line: i32 = self.first_editable_line();
        let end_line = self.last_editable_line();

        let start_column = self.first_editable_column() as usize;
        let end_column = self.last_editable_column() + 1;

        let layer_ref = &mut self.buffer.layers[self.current_layer];
        // Shift character data left
        for y in start_line..=end_line {
            let line = &mut layer_ref.lines[y as usize];
            if line.chars.len() > start_column {
                line.chars.insert(end_column as usize, AttributedChar::default());
                line.chars.remove(start_column);
            }
        }

        // === NEW: horizontal sixel scroll (left) ===
        let mut remove_indices: Vec<usize> = Vec::new();
        for (i, sixel) in layer_ref.sixels.iter_mut().enumerate() {
            let rect = sixel.as_rectangle(font_dims);
            let left = rect.start.x;
            let right = left + rect.size.width - 1;
            // We only act if vertically inside editable band (optional refinement)
            if rect.start.y < start_line || rect.start.y > end_line {
                continue;
            }

            if left == start_column as i32 {
                // Its leftmost column is lost
                remove_indices.push(i);
            } else if left > start_column as i32 && right <= end_column as i32 {
                sixel.position.x -= 1;
            }
        }
        for idx in remove_indices.into_iter().rev() {
            layer_ref.sixels.remove(idx);
        }
    }

    fn scroll_right(&mut self) {
        let font_dims = self.font_dimensions();

        let start_line: i32 = self.first_editable_line();
        let end_line = self.last_editable_line();

        let start_column = self.first_editable_column() as usize;
        let end_column = self.last_editable_column() as usize;
        let layer_ref = &mut self.buffer.layers[self.current_layer];
        // Shift character data right
        for y in start_line..=end_line {
            let line: &mut Line = &mut layer_ref.lines[y as usize];
            if line.chars.len() > start_column {
                line.chars.insert(start_column, AttributedChar::default());
                line.chars.remove(end_column + 1);
            }
        }

        // === NEW: horizontal sixel scroll (right) ===
        let mut remove_indices: Vec<usize> = Vec::new();
        for (i, sixel) in layer_ref.sixels.iter_mut().enumerate() {
            let rect = sixel.as_rectangle(font_dims);
            let left = rect.start.x;
            let right = left + rect.size.width - 1;
            if rect.start.y < start_line || rect.start.y > end_line {
                continue;
            }

            if right == end_column as i32 {
                // Rightmost column gets pushed out
                remove_indices.push(i);
            } else if left >= start_column as i32 && right < end_column as i32 {
                sixel.position.x += 1;
            }
        }
        for idx in remove_indices.into_iter().rev() {
            layer_ref.sixels.remove(idx);
        }
    }

    fn add_sixel(&mut self, pos: Position, mut sixel: Sixel) {
        sixel.position = pos;
        let font_dims = self.buffer.font_dimensions();
        let vec = &mut self.buffer.layers[0].sixels;

        let screen_rect = sixel.screen_rect(font_dims);
        let mut sixel_count = vec.len();
        // remove old sixel that are shadowed by the new one
        let mut i = 0;
        while i < sixel_count {
            let old_rect = vec[i].screen_rect(font_dims);
            if screen_rect.contains_rect(&old_rect) {
                vec.remove(i);
                sixel_count -= 1;
            } else {
                i += 1;
            }
        }
        vec.push(sixel);
        self.buffer.mark_dirty();
    }

    fn insert_line(&mut self, line: usize, new_line: crate::Line) {
        self.buffer.layers[self.current_layer].lines.insert(line as usize, new_line);
    }

    fn set_width(&mut self, width: i32) {
        let height = width.min(limits::MAX_BUFFER_WIDTH);
        self.buffer.set_width(height);
        for layer in &mut self.buffer.layers {
            layer.set_width(height);
        }
    }

    fn set_height(&mut self, height: i32) {
        let height = height.min(limits::MAX_BUFFER_HEIGHT);
        self.buffer.set_height(height);
        for layer in &mut self.buffer.layers {
            layer.set_height(height);
        }
    }

    fn add_hyperlink(&mut self, link: crate::HyperLink) {
        self.buffer.layers[self.current_layer].add_hyperlink(link);
    }

    fn set_font(&mut self, font_number: usize, font: BitFont) {
        self.buffer.set_font(font_number, font);
    }

    fn remove_font(&mut self, font_number: usize) -> Option<BitFont> {
        self.buffer.remove_font(font_number)
    }

    fn clear_font_table(&mut self) {
        self.buffer.clear_font_table();
    }

    fn clear_scrollback(&mut self) {
        self.scrollback_buffer.clear();
    }

    fn remove_terminal_line(&mut self, line: i32) {
        // DL (Delete Line) - Delete lines at cursor position
        // Lines are scrolled up within the scroll region, blank line added at bottom
        // If cursor is outside scroll region, the operation has no effect.
        let start_column = self.first_editable_column();
        let end_column = self.last_editable_column();

        let top = self.first_editable_line();
        let bottom = self.last_editable_line();

        if self.terminal_state().margins_top_bottom().is_some() {
            // If cursor is outside scroll region, do nothing
            if line < top || line > bottom {
                return;
            }

            // Shift lines up within the scroll region
            let layer_ref = &mut self.buffer.layers[self.current_layer];
            for x in start_column..=end_column {
                // Move from delete position to bottom
                for y in line..bottom {
                    let ch = layer_ref.char_at((x, y + 1).into());
                    layer_ref.set_char((x, y), ch);
                }
                // Clear the bottom line
                layer_ref.set_char((x, bottom), AttributedChar::default());
            }
        } else {
            // No scroll region - just remove the line
            if line >= self.buffer.line_count() {
                return;
            }
            self.buffer.layers[self.current_layer].remove_line(line);
        }
    }

    fn insert_terminal_line(&mut self, line: i32) {
        // IL (Insert Line) - Insert blank lines at cursor position
        // Lines are scrolled down within the scroll region, bottom line is lost
        // If cursor is outside scroll region, the operation has no effect.
        let start_column = self.first_editable_column();
        let end_column = self.last_editable_column();

        let top = self.first_editable_line();
        let bottom = self.last_editable_line();

        if self.terminal_state().margins_top_bottom().is_some() {
            // If cursor is outside scroll region, do nothing
            if line < top || line > bottom {
                return;
            }

            // Shift lines down within the scroll region (bottom line is lost)
            let layer_ref = &mut self.buffer.layers[self.current_layer];
            for x in start_column..=end_column {
                // Move from bottom-1 to cursor position (in reverse to avoid overwriting)
                // Note: bottom is the last line in scroll region (0-based, inclusive)
                // We shift lines line..bottom down to line+1..bottom+1
                // The content at 'bottom' is lost (pushed out of scroll region)
                for y in (line..bottom).rev() {
                    let ch = layer_ref.char_at((x, y).into());
                    layer_ref.set_char((x, y + 1), ch);
                }
                // Clear the inserted line
                layer_ref.set_char((x, line), AttributedChar::default());
            }
        } else {
            // No scroll region - insert line at cursor, pushing everything down
            let buffer_width = self.buffer.layers[self.current_layer].width();
            self.buffer.layers[self.current_layer].insert_line(line, Line::with_capacity(buffer_width));
        }
    }

    fn clear_screen(&mut self) {
        // Add entire screen to scrollback
        if self.terminal_state().is_terminal_buffer {
            let (size, rgba_data) = crate::scrollback_buffer::render_scrollback_region(self, self.resolution().height);
            self.scrollback_buffer.add_chunk(rgba_data, size);
        }

        self.set_caret_position(Position::default());
        let layer: &mut Layer = &mut self.buffer.layers[self.current_layer];
        layer.clear();
        if self.terminal_state().is_terminal_buffer {
            self.buffer.set_size(self.terminal_state().size());
        }
        self.buffer.mark_dirty();
    }

    fn mark_dirty(&self) {
        self.buffer.mark_dirty()
    }

    fn layer_count(&self) -> usize {
        self.buffer.layers.len()
    }

    fn get_current_layer(&self) -> usize {
        self.current_layer
    }

    fn set_current_layer(&mut self, layer: usize) -> Result<()> {
        if layer < self.buffer.layers.len() {
            self.current_layer = layer;
            Ok(())
        } else {
            Err(crate::EngineError::LayerOutOfRange {
                layer,
                max: self.buffer.layers.len(),
            })
        }
    }

    fn get_layer(&self, layer: usize) -> Option<&Layer> {
        self.buffer.layers.get(layer)
    }

    fn get_layer_mut(&mut self, layer: usize) -> Option<&mut Layer> {
        self.buffer.layers.get_mut(layer)
    }

    fn get_layer_bounds(&self, layer: usize) -> Option<(crate::Position, crate::Size)> {
        self.buffer.layers.get(layer).map(|l| (l.offset(), l.size()))
    }

    fn is_layer_paste(&self, layer: usize) -> bool {
        self.buffer.layers.get(layer).map_or(false, |l| l.role.is_paste())
    }

    fn saved_caret_pos(&mut self) -> &mut Position {
        &mut self.saved_caret_pos
    }

    fn saved_cursor_state(&mut self) -> &mut SavedCaretState {
        &mut self.saved_caret_state
    }

    fn handle_rip_command(&mut self, _cmd: RipCommand) {
        panic!("RIP not supported for text screeens.");
    }

    fn handle_skypix_command(&mut self, _cmd: SkypixCommand) {
        panic!("SkyPix not supported for text screens.");
    }

    fn handle_igs_command(&mut self, _cmd: icy_parser_core::IgsCommand) {
        panic!("IGS not supported for text screens.");
    }

    fn set_aspect_ratio(&mut self, enabled: bool) {
        self.buffer.use_aspect_ratio = enabled;
    }

    fn set_letter_spacing(&mut self, enabled: bool) {
        self.buffer.use_letter_spacing = enabled;
    }
}
