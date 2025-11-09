#![allow(clippy::match_same_arms)]
use super::BufferParser;
use crate::{AttributedChar, CallbackAction, EditableScreen, EngineResult, Position};

mod constants;

#[cfg(test)]
mod tests;

/// BBC MODE 7 implementation. Spec here:
/// <https://www.bbcbasic.co.uk/bbcwin/manual/bbcwin8.html>
/// <https://central.kaserver5.org/Kasoft/Typeset/BBC/Ch28.html>
/// <https://www.bbcbasic.co.uk/bbcwin/manual/bbcwinh.html>
pub struct Parser {
    got_esc: bool,

    hold_graphics: bool,
    held_graphics_character: char,

    is_contiguous: bool,

    is_in_graphic_mode: bool,

    _graphics_bg: u32,
    _alpha_bg: u32,
}

impl Default for Parser {
    fn default() -> Self {
        Self {
            got_esc: false,
            hold_graphics: false,
            held_graphics_character: ' ',
            is_contiguous: true,
            is_in_graphic_mode: false,
            _graphics_bg: 0,
            _alpha_bg: 0,
        }
    }
}

impl Parser {
    fn _reset_screen(&mut self) {
        self.got_esc = false;

        self.hold_graphics = false;
        self.held_graphics_character = ' ';

        self.is_contiguous = true;
        self.is_in_graphic_mode = false;
        self._graphics_bg = 0;
        self._alpha_bg = 0;
    }

    fn fill_to_eol(buf: &mut dyn EditableScreen) {
        if buf.caret().position().x <= 0 {
            return;
        }
        let sx = buf.caret().position().x;
        let sy = buf.caret().position().y;

        let attr = buf.get_char((sx, sy).into()).attribute;

        for x in sx..buf.terminal_state().get_width() {
            let p = Position::new(x, sy);
            let mut ch = buf.get_char(p);
            if ch.attribute != attr {
                break;
            }
            ch.attribute = buf.caret().attribute;
            buf.set_char(p, ch);
        }
    }

    fn _reset_on_row_change(&mut self, buf: &mut dyn EditableScreen) {
        self._reset_screen();
        buf.caret_default_colors();
    }

    fn print_char(&mut self, buf: &mut dyn EditableScreen, ch: AttributedChar) {
        buf.set_char(buf.caret().position(), ch);
        self.caret_right(buf);
    }

    fn caret_down(&mut self, buf: &mut dyn EditableScreen) {
        buf.index();
    }

    fn caret_up(buf: &mut dyn EditableScreen) {
        let y = if buf.caret().y > 0 {
            buf.caret().y.saturating_sub(1)
        } else {
            buf.terminal_state().get_height() - 1
        };
        buf.caret_mut().y = y;
    }

    fn caret_right(&mut self, buf: &mut dyn EditableScreen) {
        let x = buf.caret().x;
        buf.caret_mut().x = x + 1;
        if buf.caret().x >= buf.terminal_state().get_width() {
            buf.caret_mut().x = 0;
            buf.down(1);
        }
    } // 9PSYRNY2
    // ADGJ

    #[allow(clippy::unused_self)]
    fn caret_left(&self, buf: &mut dyn EditableScreen) {
        if buf.caret().x > 0 {
            let x = buf.caret().x.saturating_sub(1);
            buf.caret_mut().x = x;
        } else {
            let x = buf.terminal_state().get_width().saturating_sub(1);
            buf.caret_mut().x = x;
            Parser::caret_up(buf);
        }
    }

    fn interpret_char(&mut self, buf: &mut dyn EditableScreen, ch: u8) -> CallbackAction {
        if !self.hold_graphics {
            self.held_graphics_character = ' ';
        }

        let mut print_ch = ch;
        //if self.is_in_graphic_mode
        {
            let offset = if self.is_contiguous { 128 } else { 192 };
            if (160..=191).contains(&print_ch) {
                print_ch = print_ch - 160 + offset;
            }
            if (225..=255).contains(&print_ch) {
                print_ch = print_ch - 225 + 31 + offset;
            }
        }
        let ach = AttributedChar::new(print_ch as char, buf.caret().attribute);
        self.print_char(buf, ach);

        CallbackAction::Update
    }
}

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut dyn EditableScreen, ch: char) -> EngineResult<CallbackAction> {
        let ch = ch as u8;
        match ch {
            0 => {
                // Does nothing
            }
            1 => {
                // Send the next character to the printer ONLY.
                // TODO?
            }
            2 => {
                // Enable the printer.
            }
            3 => {
                // Disable the printer.
            }
            4 => {
                // Write text at the text cursor position.
            }
            5 => {
                // Write text at the graphics cursor position.
                // Note: Does nothing in Mode 7
            }
            6 => {
                // Enable output to the screen.
            }
            7 => {
                // Bell
                return Ok(CallbackAction::Beep);
            }
            // cursor backward
            8 => {
                self.caret_left(buf);
            }
            // cursor forward
            9 => {
                self.caret_right(buf);
            }
            // cursor down
            10 => {
                self.caret_down(buf);
            }
            // cursor up
            11 => {
                Parser::caret_up(buf);
            }
            // clear text window
            12 => {
                buf.reset_terminal();
                buf.clear_screen();
            }
            // return
            13 => {
                buf.cr();
            }
            14 => {
                // Enable the auto-paging mode.
            }
            15 => {
                // Disable the auto-paging mode.
            }
            16 => {
                // Clear the graphics area
            }
            17 => {
                // Define a text colour
            }
            18 => {
                // Define a graphics colour
            }
            19 => {
                // Modify the colour palette
            }
            20 => {
                // Restore the default logical colours.
            }
            21 => {
                // Disable output to the screen
            }
            22 => {
                // Select the screen mode - identical to
            }
            23 => {
                // Create user-defined characters and screen modes
            }
            24 => {
                // Define a graphics viewport
            }
            25 => {
                // Identical to PLOT.
            }
            26 => {
                // Restore the default text and graphics viewports.
            }
            27 => {
                // Send the next character to the screen.
            }
            28 => {
                // Define a text viewport
            }
            29 => {
                // Set the graphics origin - identical to ORIGIN.
            }
            30 => {
                // Home the text cursor to the top left of the screen.
                buf.home();
            }
            31 => {
                // Home the graphics cursor to the top left of the screen.
            }
            127 => {
                // Backspace and delete
                buf.bs();
            }
            129..=135 => {
                // Alpha Red, Green, Yellow, Blue, Magenta, Cyan, White
                self.is_in_graphic_mode = false;
                buf.caret_mut().attribute.set_is_concealed(false);
                self.held_graphics_character = ' ';
                buf.caret_mut().set_foreground(1 + (ch - 129) as u32);
                Parser::fill_to_eol(buf);
                let ch = AttributedChar::new(' ', buf.caret().attribute);
                self.print_char(buf, ch);
            }
            // Flash
            136 => {
                buf.caret_mut().attribute.set_is_blinking(true);
                Parser::fill_to_eol(buf);
                let ch = AttributedChar::new(' ', buf.caret().attribute);
                self.print_char(buf, ch);
            }
            // Steady
            137 => {
                buf.caret_mut().attribute.set_is_blinking(false);
                Parser::fill_to_eol(buf);
                let ch = AttributedChar::new(' ', buf.caret().attribute);
                self.print_char(buf, ch);
            }

            // normal height
            140 => {
                buf.caret_mut().attribute.set_is_double_height(false);
                Parser::fill_to_eol(buf);
                let ch = AttributedChar::new(' ', buf.caret().attribute);
                self.print_char(buf, ch);
            }

            // double height
            141 => {
                buf.caret_mut().attribute.set_is_double_height(true);
                Parser::fill_to_eol(buf);
                let ch = AttributedChar::new(' ', buf.caret().attribute);
                self.print_char(buf, ch);
            }

            145..=151 => {
                // Graphics Red, Green, Yellow, Blue, Magenta, Cyan, White
                if !self.is_in_graphic_mode {
                    self.is_in_graphic_mode = true;
                    self.held_graphics_character = ' ';
                }
                buf.caret_mut().attribute.set_is_concealed(false);
                buf.caret_mut().attribute.set_foreground(1 + (ch - 145) as u32);
                Parser::fill_to_eol(buf);
                let ch = AttributedChar::new(' ', buf.caret().attribute);
                self.print_char(buf, ch);
            }

            // conceal
            152 => {
                if !self.is_in_graphic_mode {
                    buf.caret_mut().attribute.set_is_concealed(true);
                    Parser::fill_to_eol(buf);
                }
                let ch = AttributedChar::new(' ', buf.caret().attribute);
                self.print_char(buf, ch);
            }

            // Contiguous Graphics
            153 => {
                self.is_contiguous = true;
                self.is_in_graphic_mode = true;
                let ch = AttributedChar::new(' ', buf.caret().attribute);
                self.print_char(buf, ch);
            }
            // Separated Graphics
            154 => {
                self.is_contiguous = false;
                self.is_in_graphic_mode = true;
                let ch = AttributedChar::new(' ', buf.caret().attribute);
                self.print_char(buf, ch);
            }

            // Black Background
            156 => {
                buf.caret_mut().attribute.set_is_concealed(false);
                buf.caret_mut().attribute.set_background(0);
                Parser::fill_to_eol(buf);
                let ch = AttributedChar::new(' ', buf.caret().attribute);
                self.print_char(buf, ch);
            }

            // New Background
            157 => {
                let fg = buf.caret().attribute.get_foreground();
                buf.caret_mut().attribute.set_background(fg);
                Parser::fill_to_eol(buf);
                let ch = AttributedChar::new(' ', buf.caret().attribute);
                self.print_char(buf, ch);
            }

            // Hold Graphics
            158 => {
                self.hold_graphics = true;
                self.is_in_graphic_mode = true;
            }

            // Release Graphics
            159 => {
                self.hold_graphics = false;
                self.is_in_graphic_mode = false;
            }

            _ => {
                return Ok(self.interpret_char(buf, ch));
            }
        }
        self.got_esc = false;
        Ok(CallbackAction::Update)
    }
}
