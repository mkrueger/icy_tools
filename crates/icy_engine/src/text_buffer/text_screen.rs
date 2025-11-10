use crate::{
    AttributedChar, BitFont, Caret, EditableScreen, EngineResult, HyperLink, IceMode, Line, Palette, Position, RenderOptions, RgbaScreen, SaveOptions, Screen,
    Selection, SelectionMask, Sixel, Size, TerminalState, TextBuffer, TextPane, clipboard, rip::bgi::MouseField,
};

pub struct TextScreen {
    pub caret: Caret,
    pub buffer: TextBuffer,

    pub current_layer: usize,

    pub selection_opt: Option<Selection>,
    pub selection_mask: SelectionMask,
    pub mouse_fields: Vec<MouseField>,
}

impl TextScreen {
    pub fn new(size: impl Into<Size>) -> Self {
        Self {
            caret: Caret::default(),
            buffer: TextBuffer::new(size),
            current_layer: 0,
            selection_opt: None,
            selection_mask: SelectionMask::default(),
            mouse_fields: Vec::new(),
        }
    }
}

impl TextPane for TextScreen {
    fn get_char(&self, pos: crate::Position) -> AttributedChar {
        self.buffer.get_char(pos)
    }

    fn get_line_count(&self) -> i32 {
        self.buffer.get_line_count()
    }

    fn get_width(&self) -> i32 {
        self.buffer.get_width()
    }

    fn get_height(&self) -> i32 {
        self.buffer.get_height()
    }

    fn get_line_length(&self, line: i32) -> i32 {
        self.buffer.get_line_length(line)
    }

    fn get_rectangle(&self) -> crate::Rectangle {
        self.buffer.get_rectangle()
    }

    fn get_size(&self) -> Size {
        self.buffer.get_size()
    }
}

impl Screen for TextScreen {
    fn buffer_type(&self) -> crate::BufferType {
        self.buffer.buffer_type
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
        self.buffer.render_to_rgba(options)
    }

    fn get_first_visible_line(&self) -> i32 {
        self.buffer.get_first_visible_line()
    }

    fn get_last_visible_line(&self) -> i32 {
        self.buffer.get_last_visible_line()
    }

    fn get_first_editable_line(&self) -> i32 {
        self.buffer.get_first_editable_line()
    }

    fn get_last_editable_line(&self) -> i32 {
        self.buffer.get_last_editable_line()
    }

    fn get_first_editable_column(&self) -> i32 {
        self.buffer.get_first_editable_column()
    }

    fn get_last_editable_column(&self) -> i32 {
        self.buffer.get_last_editable_column()
    }

    fn line_count(&self) -> usize {
        self.buffer.get_line_count() as usize
    }

    fn get_font(&self, font_number: usize) -> Option<&BitFont> {
        self.buffer.get_font(font_number)
    }

    fn font_count(&self) -> usize {
        self.buffer.font_count()
    }

    fn get_font_dimensions(&self) -> Size {
        self.buffer.get_font_dimensions()
    }

    fn get_selection(&self) -> Option<Selection> {
        self.selection_opt
    }

    fn selection_mask(&self) -> &crate::SelectionMask {
        &self.selection_mask
    }

    fn set_selection(&mut self, sel: Selection) -> EngineResult<()> {
        self.selection_opt = Some(sel);
        Ok(())
    }

    fn clear_selection(&mut self) -> EngineResult<()> {
        self.selection_opt = None;
        Ok(())
    }

    fn hyperlinks(&self) -> &Vec<HyperLink> {
        &self.buffer.layers[self.current_layer].hyperlinks
    }

    fn update_hyperlinks(&mut self) {
        self.buffer.update_hyperlinks();
    }

    fn to_bytes(&mut self, extension: &str, options: &SaveOptions) -> EngineResult<Vec<u8>> {
        self.buffer.to_bytes(extension, options)
    }

    fn get_copy_text(&self) -> Option<String> {
        let Some(selection) = &self.selection_opt else {
            return None;
        };
        clipboard::get_text(&self.buffer, self.buffer.buffer_type, selection)
    }

    fn get_copy_rich_text(&self) -> Option<String> {
        let Some(selection) = &self.selection_opt else {
            return None;
        };
        clipboard::get_rich_text(&self.buffer, selection)
    }

    fn get_clipboard_data(&self) -> Option<Vec<u8>> {
        clipboard::get_clipboard_data(&self.buffer, self.current_layer, &self.selection_mask, &self.selection_opt)
    }

    fn mouse_fields(&self) -> &Vec<MouseField> {
        &self.mouse_fields
    }

    fn upper_left_position(&self) -> Position {
        match self.terminal_state().origin_mode {
            crate::OriginMode::UpperLeftCorner => Position {
                x: 0,
                y: self.get_first_visible_line(),
            },
            crate::OriginMode::WithinMargins => Position {
                x: 0,
                y: self.get_first_editable_line(),
            },
        }
    }
}

impl RgbaScreen for TextScreen {
    fn get_resolution(&self) -> Size {
        let font_size = self.get_font(0).unwrap().size;
        let rect = self.get_size();
        let px_width = rect.width * font_size.width;
        let px_height = rect.height * font_size.height;
        Size::new(px_width, px_height)
    }

    fn screen(&self) -> &[u8] {
        panic!("Not supported for TextScreen");
    }

    fn screen_mut(&mut self) -> &mut Vec<u8> {
        panic!("Not supported for TextScreen");
    }

    fn set_resolution(&mut self, _size: Size) {
        panic!("Not supported for TextScreen");
    }
}

impl EditableScreen for TextScreen {
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
    }

    fn set_char(&mut self, pos: Position, ch: AttributedChar) {
        self.buffer.layers[self.current_layer].set_char(pos, ch);
    }

    fn set_size(&mut self, size: Size) {
        self.buffer.set_size(size);
    }

    fn scroll_up(&mut self) {
        if self.terminal_state().get_margins_top_bottom().is_none() {
            let line = self.buffer.layers[0].lines.get(0).cloned().unwrap_or(Line::new());
            self.buffer.push_to_scrollback(line);
        }

        let font_dims = self.get_font_dimensions();

        let start_line: i32 = self.get_first_editable_line();
        let end_line = self.get_last_editable_line();

        let start_column = self.get_first_editable_column();
        let end_column = self.get_last_editable_column();

        let layer_ref = &mut self.buffer.layers[self.current_layer];
        for x in start_column..=end_column {
            (start_line..end_line).for_each(|y| {
                let ch = layer_ref.get_char((x, y + 1).into());
                layer_ref.set_char((x, y), ch);
            });
            layer_ref.set_char((x, end_line), AttributedChar::default());
        }

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
        let font_dims = self.get_font_dimensions();

        let start_line: i32 = self.get_first_editable_line();
        let end_line = self.get_last_editable_line();

        let start_column = self.get_first_editable_column();
        let end_column = self.get_last_editable_column();

        let layer_ref = &mut self.buffer.layers[self.current_layer];
        // Shift character data downward
        for x in start_column..=end_column {
            ((start_line + 1)..=end_line).rev().for_each(|y| {
                let ch = layer_ref.get_char((x, y - 1).into());
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
        let font_dims = self.get_font_dimensions();

        let start_line: i32 = self.get_first_editable_line();
        let end_line = self.get_last_editable_line();

        let start_column = self.get_first_editable_column() as usize;
        let end_column = self.get_last_editable_column() + 1;

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
        let font_dims = self.get_font_dimensions();

        let start_line: i32 = self.get_first_editable_line();
        let end_line = self.get_last_editable_line();

        let start_column = self.get_first_editable_column() as usize;
        let end_column = self.get_last_editable_column() as usize;

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

    fn insert_line(&mut self, line: usize, new_line: crate::Line) {
        self.buffer.layers[self.current_layer].lines.insert(line as usize, new_line);
    }

    fn set_height(&mut self, height: i32) {
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

    fn stop_sixel_threads(&mut self) {
        self.buffer.stop_sixel_threads();
    }

    fn push_sixel_thread(&mut self, handle: std::thread::JoinHandle<EngineResult<Sixel>>) {
        self.buffer.sixel_threads.push_back(handle);
    }

    fn sixel_threads_runnning(&self) -> bool {
        !self.buffer.sixel_threads.is_empty()
    }

    fn update_sixel_threads(&mut self) -> EngineResult<bool> {
        self.buffer.update_sixel_threads()
    }

    fn clear_scrollback(&mut self) {
        self.buffer.clear_scrollback();
    }

    fn get_max_scrollback_offset(&self) -> usize {
        self.buffer.get_max_scrollback_offset()
    }
    fn scrollback_position(&self) -> usize {
        self.buffer.scrollback_position()
    }

    fn set_scroll_position(&mut self, line: usize) {
        self.buffer.set_scroll_position(line);
    }

    fn remove_terminal_line(&mut self, line: i32) {
        if line >= self.get_line_count() {
            return;
        }
        self.buffer.layers[self.current_layer].remove_line(line);
        if let Some((_, end)) = self.terminal_state_mut().get_margins_top_bottom() {
            let buffer_width = self.buffer.layers[self.current_layer].get_width();
            self.buffer.layers[self.current_layer].insert_line(end, Line::with_capacity(buffer_width));
        }
    }

    fn insert_terminal_line(&mut self, line: i32) {
        if let Some((_, end)) = self.terminal_state_mut().get_margins_top_bottom() {
            if end < self.buffer.layers[self.current_layer].lines.len() as i32 {
                self.buffer.layers[self.current_layer].lines.remove(end as usize);
            }
        }
        let buffer_width = self.buffer.layers[self.current_layer].get_width();
        self.buffer.layers[self.current_layer].insert_line(line, Line::with_capacity(buffer_width));
    }

    fn clear_screen(&mut self) {
        self.caret_mut().set_position(Position::default());
        self.stop_sixel_threads();
        let layer = &mut self.buffer.layers[self.current_layer];
        layer.clear();
        self.terminal_state_mut().cleared_screen = true;
        if self.terminal_state().is_terminal_buffer {
            self.buffer.set_size(self.terminal_state().get_size());
        }
    }
}
