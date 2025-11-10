use std::cmp::max;

use crate::{
    AttributedChar, BitFont, CallbackAction, EngineResult, HyperLink, IceMode, Line, Palette, Position, RenderOptions, SaveOptions, Selection, Sixel, Size,
    TerminalState, TextAttribute, TextPane, caret, rip::bgi::MouseField,
};

pub trait Screen: TextPane + Send {
    fn buffer_type(&self) -> crate::BufferType;

    fn ice_mode(&self) -> IceMode;

    fn terminal_state(&self) -> &TerminalState;

    fn palette(&self) -> &Palette;

    fn caret(&self) -> &caret::Caret;

    fn render_to_rgba(&self, options: &RenderOptions) -> (Size, Vec<u8>);

    fn get_first_visible_line(&self) -> i32;

    fn get_last_visible_line(&self) -> i32;

    fn get_first_editable_line(&self) -> i32;

    fn get_last_editable_line(&self) -> i32;

    fn get_first_editable_column(&self) -> i32;

    fn get_last_editable_column(&self) -> i32;

    fn get_font_dimensions(&self) -> Size;
    fn get_font(&self, font_number: usize) -> Option<&BitFont>;

    fn font_count(&self) -> usize;

    #[must_use]
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

    fn line_count(&self) -> usize;

    fn get_selection(&self) -> Option<Selection>;

    fn selection_mask(&self) -> &crate::SelectionMask;

    fn set_selection(&mut self, sel: Selection) -> EngineResult<()>;

    fn clear_selection(&mut self) -> EngineResult<()>;

    fn hyperlinks(&self) -> &Vec<HyperLink>;

    fn update_hyperlinks(&mut self);

    fn to_bytes(&mut self, extension: &str, options: &SaveOptions) -> EngineResult<Vec<u8>>;

    fn get_copy_text(&self) -> Option<String>;
    fn get_copy_rich_text(&self) -> Option<String>;
    fn get_clipboard_data(&self) -> Option<Vec<u8>>;

    fn mouse_fields(&self) -> &Vec<MouseField>;
}

pub trait RgbaScreen: Screen {
    fn get_resolution(&self) -> Size;
    fn set_resolution(&mut self, size: Size);
    fn screen_mut(&mut self) -> &mut [u8];
}

pub trait EditableScreen: RgbaScreen {
    fn clear_mouse_fields(&mut self);
    fn add_mouse_field(&mut self, mouse_field: MouseField);

    fn ice_mode_mut(&mut self) -> &mut IceMode;

    fn caret_mut(&mut self) -> &mut caret::Caret;

    fn caret_default_colors(&mut self) {
        let font_page = self.caret_mut().font_page();
        self.caret_mut().attribute = TextAttribute {
            font_page,
            ..Default::default()
        };
    }

    fn palette_mut(&mut self) -> &mut Palette;

    fn buffer_type_mut(&mut self) -> &mut crate::BufferType;

    fn terminal_state_mut(&mut self) -> &mut TerminalState;

    fn reset_terminal(&mut self);

    fn insert_line(&mut self, line: usize, new_line: Line);

    fn set_font(&mut self, font_number: usize, font: BitFont);

    fn remove_font(&mut self, font_number: usize) -> Option<BitFont>;

    fn clear_font_table(&mut self);

    fn set_size(&mut self, size: Size);

    fn stop_sixel_threads(&mut self);

    fn push_sixel_thread(&mut self, handle: std::thread::JoinHandle<EngineResult<Sixel>>);

    fn sixel_threads_runnning(&self) -> bool;

    fn update_sixel_threads(&mut self) -> EngineResult<bool>;

    fn lf(&mut self) -> CallbackAction {
        let was_ooe = self.caret().y > self.get_last_editable_line();
        let mut line_inserted = 0;
        self.caret_mut().x = 0;
        let y = self.caret_mut().y;
        self.caret_mut().y = y + 1;

        while self.caret().y >= self.line_count() as i32 {
            if self.terminal_state().fixed_size && self.caret().y >= self.terminal_state().get_height() {
                line_inserted += 1;
                if self.line_count() > 0 {
                    self.scroll_up();
                }
                self.caret_mut().y -= 1;
                continue;
            }
            let len = self.line_count();
            let buffer_width = self.terminal_state_mut().get_width();
            self.insert_line(len, Line::with_capacity(buffer_width));
        }

        if !self.terminal_state().is_terminal_buffer {
            if line_inserted > 0 {
                return CallbackAction::ScrollDown(line_inserted);
            }
            return CallbackAction::Update;
        }

        if self.caret().y + 1 > self.get_height() {
            self.set_height(self.caret().y + 1);
        }

        if was_ooe {
            self.limit_caret_pos();
        } else {
            self.check_scrolling_on_caret_down(false);
        }
        if line_inserted > 0 {
            return CallbackAction::ScrollDown(line_inserted);
        }
        CallbackAction::Update
    }

    /// (form feed, FF, \f, ^L), to cause a printer to eject paper to the top of the next page, or a video terminal to clear the screen.
    fn ff(&mut self) {
        self.stop_sixel_threads();
        self.reset_terminal();
        self.clear_screen();
    }

    /// (carriage return, CR, \r, ^M), moves the printing position to the start of the line.
    fn cr(&mut self) {
        self.caret_mut().x = 0;
    }

    fn eol(&mut self) {
        let x = self.get_width() - 1;
        self.caret_mut().x = x;
    }

    fn home(&mut self) {
        let pos = self.upper_left_position();
        self.caret_mut().set_position(pos);
    }

    /// Delete character at caret position, shifting remaining characters in the line left.
    /// Implements a slower fallback using only get_char/set_char APIs.
    fn del(&mut self) {
        let pos = self.caret().position();
        let line_len = self.get_line_length(pos.y);
        if pos.x < 0 || pos.y < 0 {
            return;
        }
        if pos.x >= line_len {
            return;
        }
        // Shift characters left from pos.x+1 .. line_len-1
        for x in pos.x..(line_len - 1) {
            let next = self.get_char((x + 1, pos.y).into());
            self.set_char((x, pos.y).into(), next);
        }
        // Blank out last logical character position
        let blank = AttributedChar::new(' ', self.caret().attribute);
        self.set_char((line_len - 1, pos.y).into(), blank);
    }

    /// Insert a blank character at caret, shifting existing characters right.
    /// Uses get_char/set_char only; slower but generic.
    fn ins(&mut self) {
        let pos = self.caret().position();
        if pos.x < 0 || pos.y < 0 {
            return;
        }
        let line_len = self.get_line_length(pos.y);
        if pos.x >= self.get_width() {
            return;
        }
        // Ensure we have a trailing cell to shift into; extend with blank if needed
        let blank_attr = self.caret().attribute;
        if line_len < self.get_width() {
            // Nothing required; implicit blank beyond line_len assumed, but we explicitly write one at end to avoid artifacts.
            let end_blank = AttributedChar::new(' ', blank_attr);
            self.set_char((self.get_width() - 1, pos.y).into(), end_blank);
        }
        // Shift right from last editable column down to caret.x
        let last = (self.get_width() - 1).min(line_len.max(pos.x));
        for x in (pos.x..=last).rev() {
            let src = if x == pos.x { None } else { Some(self.get_char((x - 1, pos.y).into())) };
            let to_write = src.unwrap_or(AttributedChar::new(' ', blank_attr));
            self.set_char((x, pos.y).into(), to_write);
        }
        // Advance caret after inserted blank
        let x = self.get_width().saturating_sub(1);
        self.caret_mut().x = (pos.x + 1).min(x);
    }

    /// (backspace, BS, \b, ^H), may overprint the previous character
    fn bs(&mut self) {
        let x = max(0, self.caret_mut().x - 1);
        self.caret_mut().x = x;
        self.set_char(self.caret().position(), AttributedChar::new(' ', self.caret().attribute));
    }

    fn left(&mut self, num: i32) {
        let x = self.caret().x.saturating_sub(num);
        self.caret_mut().x = x;
        self.limit_caret_pos();
    }

    fn right(&mut self, num: i32) {
        let x = self.caret_mut().x.saturating_add(num);
        self.caret_mut().x = x;
        self.limit_caret_pos();
    }

    fn up(&mut self, num: i32) {
        let y = self.caret().y.saturating_sub(num);
        self.caret_mut().y = y;
        self.check_scrolling_on_caret_up(false);
        self.limit_caret_pos();
    }

    fn down(&mut self, num: i32) {
        let y = self.caret().y + num;
        self.caret_mut().y = y;
        self.check_scrolling_on_caret_down(false);
        self.limit_caret_pos();
    }

    /// Moves the cursor down one line in the same column. If the cursor is at the bottom margin, the page scrolls up.
    fn index(&mut self) {
        let y = self.caret_mut().y;
        self.caret_mut().y = y + 1;
        self.check_scrolling_on_caret_down(true);
        self.limit_caret_pos();
    }

    /// Moves the cursor up one line in the same column. If the cursor is at the top margin, the page scrolls down.
    fn reverse_index(&mut self) {
        self.caret_mut().y -= 1;
        self.check_scrolling_on_caret_up(true);
        self.limit_caret_pos();
    }

    fn next_line(&mut self) {
        let y = self.caret_mut().y;
        self.caret_mut().y = y + 1;
        self.caret_mut().x = 0;
        self.check_scrolling_on_caret_down(true);
        self.limit_caret_pos();
    }

    fn check_scrolling_on_caret_up(&mut self, force: bool) {
        if self.terminal_state().needs_scrolling() || force {
            let last = self.get_first_editable_line();
            while self.caret().y < last {
                self.scroll_down();
                let y = self.caret_mut().y;
                self.caret_mut().y = y + 1;
            }
        }
    }

    fn check_scrolling_on_caret_down(&mut self, force: bool) {
        if (self.terminal_state().needs_scrolling() || force) && self.caret().y > self.get_last_editable_line() {
            self.scroll_up();
            self.caret_mut().y -= 1;
        }
    }

    fn print_value(&mut self, ch: u16) {
        if let Some(ch) = char::from_u32(ch as u32) {
            let ch = AttributedChar::new(ch, self.caret().attribute);
            self.print_char(ch);
        }
    }

    fn set_char(&mut self, pos: Position, ch: AttributedChar);

    fn print_char(&mut self, ch: AttributedChar) {
        let buffer_width = self.get_width();
        if self.caret().insert_mode {
            self.ins();
        }
        if self.caret().y + 1 > self.get_height() {
            self.set_height(self.caret().y + 1);
        }
        if self.terminal_state().is_terminal_buffer && self.caret().y + 1 > self.get_height() {
            self.set_height(self.caret().y + 1);
        }

        self.set_char(self.caret().position(), ch);
        let x = self.caret().x;
        self.caret_mut().x = x + 1;

        let real_width = if self.terminal_state().is_terminal_buffer {
            self.terminal_state().get_width()
        } else {
            buffer_width
        };

        if self.caret().x >= real_width {
            if let crate::AutoWrapMode::AutoWrap = self.terminal_state_mut().auto_wrap_mode {
                self.lf();
            } else {
                let y = self.caret().y;
                self.caret_mut().y = y - 1;
            }
        }
    }

    fn scroll_up(&mut self);
    fn scroll_down(&mut self);

    fn scroll_left(&mut self);
    fn scroll_right(&mut self);

    fn clear_screen(&mut self);

    fn clear_scrollback(&mut self);
    fn get_max_scrollback_offset(&self) -> usize;
    fn scrollback_position(&self) -> usize;
    fn set_scroll_position(&mut self, line: usize);

    fn clear_buffer_down(&mut self) {
        let pos = self.caret().position();
        let ch: AttributedChar = AttributedChar {
            attribute: self.caret().attribute,
            ..Default::default()
        };

        for y in pos.y..self.get_last_visible_line() {
            for x in 0..self.get_width() {
                self.set_char((x, y).into(), ch);
            }
        }
    }

    fn clear_buffer_up(&mut self) {
        let pos = self.caret().position();
        let ch: AttributedChar = AttributedChar {
            attribute: self.caret().attribute,
            ..Default::default()
        };

        for y in self.get_first_visible_line()..pos.y {
            for x in 0..self.get_width() {
                self.set_char((x, y).into(), ch);
            }
        }
    }

    fn clear_line(&mut self) {
        let mut pos = self.caret().position();
        let ch: AttributedChar = AttributedChar {
            attribute: self.caret().attribute,
            ..Default::default()
        };
        for x in 0..self.get_width() {
            pos.x = x;
            self.set_char(pos, ch);
        }
    }

    fn clear_line_end(&mut self) {
        let mut pos = self.caret().position();
        let ch: AttributedChar = AttributedChar {
            attribute: self.caret().attribute,
            ..Default::default()
        };
        for x in pos.x..self.get_width() {
            pos.x = x;
            self.set_char(pos, ch);
        }
    }

    fn clear_line_start(&mut self) {
        let mut pos = self.caret().position();
        let ch: AttributedChar = AttributedChar {
            attribute: self.caret().attribute,
            ..Default::default()
        };
        for x in 0..pos.x {
            pos.x = x;
            self.set_char(pos, ch);
        }
    }

    fn remove_terminal_line(&mut self, line: i32);

    fn insert_terminal_line(&mut self, line: i32);

    fn set_height(&mut self, height: i32);

    fn add_hyperlink(&mut self, link: crate::HyperLink);

    fn tab_forward(&mut self) {
        let x = (self.caret().x / 8 + 1) * 8;
        let w = self.get_width() - 1;
        self.caret_mut().x = x.min(w);
    }

    fn limit_caret_pos(&mut self) {
        match self.terminal_state().origin_mode {
            crate::OriginMode::UpperLeftCorner => {
                if self.terminal_state().is_terminal_buffer {
                    let first = self.get_first_visible_line();
                    self.caret_mut().y = self.caret().y.clamp(first, first + self.get_height() - 1);
                }
                let x: i32 = self.caret().x.clamp(0, (self.get_width() - 1).max(0));
                self.caret_mut().x = x;
            }
            crate::OriginMode::WithinMargins => {
                let first = self.get_first_editable_line();
                let height = self.get_last_editable_line() - first;
                let n = self.caret().y.clamp(first, (first + height - 1).max(first));
                self.caret_mut().y = n;
                let x = self.caret().x.clamp(0, (self.get_width() - 1).max(0));
                self.caret_mut().x = x;
            }
        }
    }
}
