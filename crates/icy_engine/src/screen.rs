use parking_lot::Mutex;
use std::{cmp::max, sync::Arc};

use icy_parser_core::{IgsCommand, RipCommand, SkypixCommand};

use crate::{
    AttributedChar, BitFont, EngineResult, HyperLink, IceMode, Layer, Line, MouseField, Palette, Position, Rectangle, RenderOptions, SaveOptions, Selection,
    Sixel, Size, TerminalResolution, TerminalState, TextAttribute, TextPane, caret,
};

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GraphicsType {
    Text,
    Rip,
    IGS(TerminalResolution),
    Skypix,
}

impl GraphicsType {
    pub fn scan_lines(&self) -> bool {
        match self {
            GraphicsType::Text => false,
            GraphicsType::Rip => false,
            GraphicsType::IGS(res) => match res {
                TerminalResolution::Low => false,
                TerminalResolution::Medium => true,
                TerminalResolution::High => false,
            },
            GraphicsType::Skypix => false,
        }
    }

    pub fn default_fg_color(&self) -> u32 {
        match self {
            GraphicsType::Text => 7,
            GraphicsType::Rip => 7,
            GraphicsType::IGS(res) => res.default_fg_color() as u32,
            GraphicsType::Skypix => 7,
        }
    }
}

/// Core trait for anything that can be displayed
/// Viewing interface - all screens must implement this
pub trait Screen: TextPane + Send + Sync {
    // Core identity
    fn buffer_type(&self) -> crate::BufferType;

    fn graphics_type(&self) -> crate::GraphicsType {
        crate::GraphicsType::Text
    }

    /// Gets the current resolution of the screen in pixels
    fn get_resolution(&self) -> Size;

    /// Gets the virtual size of the screen (including scrollback)
    fn virtual_size(&self) -> Size {
        self.get_resolution() // Default: no scrollback
    }

    fn get_font_dimensions(&self) -> Size;

    fn scan_lines(&self) -> bool;

    // Rendering
    fn render_to_rgba(&self, options: &RenderOptions) -> (Size, Vec<u8>);

    fn render_region_to_rgba(&self, _region: Rectangle, _options: &RenderOptions) -> (Size, Vec<u8>) {
        // Default implementation: render full and crop
        //let (full_size, full_pixels) = self.render_to_rgba(options);
        //crop_region(&full_pixels, full_size, region)

        todo!("Implement render_region_to_rgba for specific screen types");
    }

    // Visual state
    fn palette(&self) -> &Palette;
    fn ice_mode(&self) -> IceMode;
    fn get_font(&self, font_number: usize) -> Option<&BitFont>;
    fn font_count(&self) -> usize;

    // Version for change tracking
    fn get_version(&self) -> u64;

    // Default foreground color
    fn default_foreground_color(&self) -> u32;
    fn max_base_colors(&self) -> u32;

    // Optional text content access (for copy/paste)
    fn get_copy_text(&self) -> Option<String> {
        None
    }

    fn get_copy_rich_text(&self) -> Option<String> {
        None
    }

    fn get_clipboard_data(&self) -> Option<Vec<u8>> {
        None
    }

    // Optional interactive elements
    fn hyperlinks(&self) -> &Vec<HyperLink>;

    fn mouse_fields(&self) -> &Vec<MouseField>;

    // Selection support
    fn get_selection(&self) -> Option<Selection>;
    fn selection_mask(&self) -> &crate::SelectionMask;

    // Selection management (mutable)
    fn set_selection(&mut self, sel: Selection) -> EngineResult<()>;
    fn clear_selection(&mut self) -> EngineResult<()>;

    // Terminal state (read-only for viewing)
    fn terminal_state(&self) -> &TerminalState;
    fn caret(&self) -> &caret::Caret;

    fn caret_position(&self) -> Position {
        self.caret().position()
    }

    fn to_bytes(&mut self, extension: &str, options: &SaveOptions) -> EngineResult<Vec<u8>>;

    // Access to editor if this screen is editable
    fn as_editable(&mut self) -> Option<&mut dyn EditableScreen> {
        None
    }

    // Downcast support for accessing concrete types
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    // Direct pixel access (for some operations)
    fn screen(&self) -> &[u8];

    // Scrollback buffer management
    fn set_scrollback_buffer_size(&mut self, _buffer_size: usize);

    /// Clone the screen into a Box for frame storage in animations
    /// Note: Some screen types may not support cloning (e.g., graphics screens)
    fn clone_box(&self) -> Box<dyn Screen> {
        unimplemented!("clone_box not supported for this screen type")
    }
}

/// Trait for screens that can be edited
/// Extends Screen with editing operations
pub trait EditableScreen: Screen {
    // Utility methods for editing operations
    fn get_first_visible_line(&self) -> i32;
    fn get_last_visible_line(&self) -> i32;
    fn get_first_editable_line(&self) -> i32;
    fn get_last_editable_line(&self) -> i32;
    fn get_first_editable_column(&self) -> i32;
    fn get_last_editable_column(&self) -> i32;

    // Line access for editing
    fn get_line(&self, line: usize) -> Option<&Line>;
    fn line_count(&self) -> usize;

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

    // Mouse field management
    fn clear_mouse_fields(&mut self);
    fn add_mouse_field(&mut self, mouse_field: MouseField);

    // Mutable state access
    fn ice_mode_mut(&mut self) -> &mut IceMode;
    fn caret_mut(&mut self) -> &mut caret::Caret;
    fn palette_mut(&mut self) -> &mut Palette;
    fn buffer_type_mut(&mut self) -> &mut crate::BufferType;
    fn terminal_state_mut(&mut self) -> &mut TerminalState;

    // Graphics type management
    fn set_graphics_type(&mut self, graphics_type: crate::GraphicsType);

    // Resolution management
    fn set_resolution(&mut self, size: Size);
    fn reset_resolution(&mut self) {}

    // Caret and terminal management
    fn reset_terminal(&mut self);

    fn caret_default_colors(&mut self) {
        let font_page = self.caret_mut().font_page();
        self.caret_mut().attribute = TextAttribute {
            font_page,
            foreground_color: self.default_foreground_color(),
            ..Default::default()
        };
    }

    fn sgr_reset(&mut self) {
        self.caret_default_colors();
        self.caret_mut().attribute.set_is_bold(false);
        self.terminal_state_mut().inverse_video = false;
    }

    // Font management
    fn set_font(&mut self, font_number: usize, font: BitFont);
    fn remove_font(&mut self, font_number: usize) -> Option<BitFont>;
    fn clear_font_table(&mut self);

    // Size management
    fn set_size(&mut self, size: Size);
    fn set_height(&mut self, height: i32);

    // Change tracking
    fn mark_dirty(&self);

    // Layer management
    /// Returns the number of layers
    fn layer_count(&self) -> usize {
        1 // Default: single layer
    }

    /// Returns the current layer index
    fn get_current_layer(&self) -> usize {
        0 // Default: layer 0
    }

    /// Sets the current layer index
    fn set_current_layer(&mut self, _layer: usize) -> EngineResult<()> {
        Ok(()) // Default: no-op (single layer)
    }

    /// Returns a reference to the layer at the given index, if it exists
    fn get_layer(&self, _layer: usize) -> Option<&Layer> {
        None // Default: no layers
    }

    /// Returns a mutable reference to the layer at the given index, if it exists
    fn get_layer_mut(&mut self, _layer: usize) -> Option<&mut Layer> {
        None // Default: no layers
    }

    // Line operations
    fn insert_line(&mut self, line: usize, new_line: Line);
    fn remove_terminal_line(&mut self, line: i32);
    fn insert_terminal_line(&mut self, line: i32);

    // Character operations
    fn set_char(&mut self, pos: Position, ch: AttributedChar);

    fn print_char(&mut self, ch: AttributedChar) {
        if self.caret().insert_mode {
            self.ins();
        }
        let is_terminal = self.terminal_state().is_terminal_buffer;
        if !is_terminal && self.caret().y + 1 > self.get_height() {
            self.set_height(self.caret().y + 1);
        }
        let mut caret_pos = self.caret_position();

        self.set_char(caret_pos, ch);
        caret_pos.x += 1;
        // left/right margin only valued inside margins - this way it's possible to print beyond right margin for updating UI
        // without resetting margins
        let in_margins = self.get_first_editable_line() <= caret_pos.y && caret_pos.y <= self.get_last_editable_line();
        let last_col = if in_margins { self.get_last_editable_column() } else { self.get_width() - 1 };

        let should_break_line = caret_pos.x > last_col;
        if should_break_line {
            // lf needs to be in margins, if there are some.
            caret_pos.x = last_col;
            if self.terminal_state_mut().auto_wrap_mode == crate::AutoWrapMode::AutoWrap {
                self.lf();
                return;
            }
        }
        self.set_caret_position(caret_pos);
    }

    fn print_value(&mut self, ch: u16) {
        if let Some(ch) = char::from_u32(ch as u32) {
            let ch = AttributedChar::new(ch, self.caret().attribute);
            self.print_char(ch);
        }
    }

    // Scrolling
    fn scroll_up(&mut self);
    fn scroll_down(&mut self);
    fn scroll_left(&mut self);
    fn scroll_right(&mut self);

    // Scrollback management

    fn snapshot_scrollback(&mut self) -> Option<Arc<Mutex<Box<dyn Screen>>>> {
        None
    }

    fn clear_scrollback(&mut self);

    // Clear operations
    fn clear_screen(&mut self);
    fn clear_line(&mut self);
    fn clear_line_end(&mut self);
    fn clear_line_start(&mut self);

    fn clear_buffer_down(&mut self) {
        let pos = self.caret_position();
        let ch: AttributedChar = AttributedChar {
            attribute: self.caret().attribute,
            ..Default::default()
        };

        for y in pos.y..=self.get_last_visible_line() {
            // <= statt <
            for x in 0..self.get_width() {
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

        for y in self.get_first_visible_line()..pos.y {
            for x in 0..self.get_width() {
                self.set_char((x, y).into(), ch);
            }
        }
        for x in 0..=pos.x {
            self.set_char((x, pos.y).into(), ch);
        }
    }

    // Hyperlink management
    fn update_hyperlinks(&mut self);
    fn add_hyperlink(&mut self, link: crate::HyperLink);

    // Sixel support
    fn add_sixel(&mut self, pos: Position, sixel: Sixel);

    // Caret positioning
    fn set_caret_position(&mut self, pos: Position) {
        self.caret_mut().set_position(pos);
    }

    // Terminal control sequences
    fn lf(&mut self) {
        let in_margin = self.terminal_state().in_margin(self.caret().position());
        let mut pos = self.caret().position();

        pos.x = self.get_first_editable_column();
        pos.y += 1;

        if self.terminal_state().is_terminal_buffer {
            while pos.y >= self.get_height() {
                self.scroll_up();
                pos.y -= 1;
                continue;
            }
        } else {
            if pos.y + 1 > self.get_height() {
                self.set_height(pos.y + 1);
            }
            self.set_caret_position(pos);
            return;
        }
        self.set_caret_position(pos);
        self.check_scrolling_on_caret_down(false, in_margin);
        self.limit_caret_pos(in_margin);
    }

    fn ff(&mut self) {
        self.reset_terminal();
        self.clear_screen();
    }

    fn cr(&mut self) {
        let in_margin = self.terminal_state().in_margin(self.caret().position());
        self.caret_mut().x = 0;
        self.limit_caret_pos(in_margin);
    }

    fn eol(&mut self) {
        let x = self.get_width() - 1;
        self.caret_mut().x = x;
    }

    fn home(&mut self) {
        let pos = self.upper_left_position();
        self.set_caret_position(pos);
    }

    fn del(&mut self) {
        let pos = self.caret_position();
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

    fn ins(&mut self) {
        let pos = self.caret_position();
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
    }

    fn bs(&mut self) {
        if let crate::AutoWrapMode::AutoWrap = self.terminal_state().auto_wrap_mode
            && self.caret().x == 0
        {
            // At column 0: decide if we can wrap to previous line.
            // Determine the "origin" (first line) depending on origin mode.
            let origin_line = match self.terminal_state().origin_mode {
                crate::OriginMode::UpperLeftCorner => self.get_first_visible_line(),
                crate::OriginMode::WithinMargins => self.get_first_editable_line(),
            };

            // If already at origin line -> no operation (NOP).
            if self.caret().y <= origin_line {
                return;
            }

            // Move to previous line.
            let prev_y = self.caret().y - 1;
            self.caret_mut().y = prev_y;

            // Choose target column: last used character on that line if any, else 0.
            let last_len = self.get_line_length(prev_y);
            let target_x = if last_len > 0 {
                // We delete the last logical character cell.
                (last_len - 1).min(self.get_width() - 1)
            } else {
                0
            };
            self.caret_mut().x = target_x;

            // Delete (blank) that character.
            self.set_char(self.caret_position(), AttributedChar::new(' ', self.caret().attribute));
        } else {
            let x = max(0, self.caret_mut().x - 1);
            self.caret_mut().x = x;
            self.set_char(self.caret_position(), AttributedChar::new(' ', self.caret().attribute));
        }
    }

    fn left(&mut self, num: i32, scroll: bool) {
        let in_margin = self.terminal_state().in_margin(self.caret().position());
        if let crate::AutoWrapMode::AutoWrap = self.terminal_state().auto_wrap_mode
            && self.caret().x == 0
        {
            // At column 0: wrap to previous line end if above origin line
            let origin_line = match self.terminal_state().origin_mode {
                crate::OriginMode::UpperLeftCorner => self.get_first_visible_line(),
                crate::OriginMode::WithinMargins => self.get_first_editable_line(),
            };
            if self.caret().y <= origin_line {
                // Already at origin line -> no-op
                return;
            }
            self.caret_mut().y -= 1;
            self.caret_mut().x = (self.get_width() - 1).max(0);
        } else {
            let x = self.caret().x.saturating_sub(num);
            self.caret_mut().x = x;
        }
        if scroll {
            self.check_scrolling_on_caret_down(false, in_margin);
        }
        self.limit_caret_pos(in_margin);
    }

    fn right(&mut self, num: i32, scroll: bool) {
        let last_col = (self.get_width() - 1).max(0);
        let in_margin = self.terminal_state().in_margin(self.caret().position());

        if let crate::AutoWrapMode::AutoWrap = self.terminal_state().auto_wrap_mode
            && self.caret().x >= last_col
        {
            self.caret_mut().x = last_col;
            self.lf();
            return;
        } else {
            let x = self.caret_mut().x.saturating_add(num);
            self.caret_mut().x = x;
        }
        if scroll {
            self.check_scrolling_on_caret_down(false, in_margin);
        }
        self.limit_caret_pos(in_margin);
    }

    fn up(&mut self, num: i32, scroll: bool) {
        let y = self.caret().y.saturating_sub(num);
        let in_margin = self.terminal_state().in_margin(self.caret().position());
        self.caret_mut().y = y;
        if scroll {
            self.check_scrolling_on_caret_up(false, in_margin);
        }
        self.limit_caret_pos(in_margin);
    }

    fn down(&mut self, num: i32, scroll: bool) {
        let y = self.caret().y + num;
        let in_margin = self.terminal_state().in_margin(self.caret().position());
        self.caret_mut().y = y;
        if scroll {
            self.check_scrolling_on_caret_down(false, in_margin);
        }
        self.limit_caret_pos(in_margin);
    }

    fn index(&mut self) {
        let mut pos = self.caret_position();
        let in_margin = self.terminal_state().in_margin(self.caret().position());
        pos.y += 1;
        self.set_caret_position(pos);
        self.check_scrolling_on_caret_down(true, in_margin);
        self.limit_caret_pos(in_margin);
    }

    fn next_line(&mut self, scroll: bool) {
        let mut pos = self.caret_position();
        let in_margin = self.terminal_state().in_margin(self.caret().position());
        pos.y += 1;
        pos.x = 0;
        self.set_caret_position(pos);
        if scroll {
            self.check_scrolling_on_caret_down(true, in_margin);
        }
        self.limit_caret_pos(in_margin);
    }

    fn check_scrolling_on_caret_up(&mut self, force: bool, was_in_margin: bool) {
        if self.terminal_state().needs_scrolling() || force {
            let last: i32 = if was_in_margin { self.get_first_editable_line() } else { 0 };
            while self.caret_position().y < last {
                self.scroll_down();
                let mut pos = self.caret_position();
                pos.y += 1;
                self.set_caret_position(pos);
            }
        }
    }

    fn check_scrolling_on_caret_down(&mut self, force: bool, was_in_margin: bool) {
        let last = if was_in_margin {
            self.get_last_editable_line()
        } else {
            self.get_height() - 1
        };
        if (self.terminal_state().needs_scrolling() || force) && self.caret_position().y > last {
            self.scroll_up();
            let mut pos = self.caret_position();
            pos.y -= 1;
            self.set_caret_position(pos);
        }
    }

    fn tab_forward(&mut self) {
        let mut pos = self.caret_position();
        let x = self.terminal_state().next_tab_stop(pos.x);
        let w = self.get_width() - 1;
        pos.x = x.min(w);
        self.set_caret_position(pos);
    }

    fn limit_caret_pos(&mut self, was_in_margin: bool) {
        let mut pos = self.caret_position();
        if !was_in_margin || self.terminal_state().origin_mode == crate::OriginMode::UpperLeftCorner {
            if self.terminal_state().is_terminal_buffer {
                let first = self.get_first_visible_line();
                pos.y = pos.y.clamp(first, first + self.get_height() - 1);
            }
            let x: i32 = pos.x.clamp(0, (self.get_width() - 1).max(0));
            pos.x = x;
        } else {
            let first = self.get_first_editable_line();
            let height = self.get_last_editable_line() - first;
            let n = pos.y.clamp(first, (first + height - 1).max(first));
            pos.y = n;
            // Respect left/right margins when origin is within margins
            let left = self.get_first_editable_column().max(0);
            let right = self.get_last_editable_column().min(self.get_width() - 1).max(left);
            let x = pos.x.clamp(left, right);
            pos.x = x;
        }
        self.set_caret_position(pos);
    }

    fn saved_caret_pos(&mut self) -> &mut Position;
    fn saved_cursor_state(&mut self) -> &mut SavedCaretState;

    // Protocol command handlers
    fn handle_rip_command(&mut self, cmd: RipCommand);
    fn handle_skypix_command(&mut self, cmd: SkypixCommand);
    fn handle_igs_command(&mut self, cmd: IgsCommand);

    // Direct pixel access mut
    fn screen_mut(&mut self) -> &mut Vec<u8>;
}

#[derive(Clone, Default)]
pub struct SavedCaretState {
    pub caret: crate::Caret,
    pub origin_mode: crate::OriginMode,
    pub auto_wrap_mode: crate::AutoWrapMode,
}
