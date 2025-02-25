#![allow(clippy::match_same_arms)]
use super::BufferParser;
use crate::{AttributedChar, Buffer, CallbackAction, Caret, EngineResult, Position, TextPane, UnicodeConverter};

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

    fn fill_to_eol(buf: &mut Buffer, caret: &Caret) {
        if caret.get_position().x <= 0 {
            return;
        }
        let sx = caret.get_position().x;
        let sy = caret.get_position().y;

        let attr = buf.get_char((sx, sy)).attribute;

        for x in sx..buf.terminal_state.get_width() {
            let p = Position::new(x, sy);
            let mut ch = buf.get_char(p);
            if ch.attribute != attr {
                break;
            }
            ch.attribute = caret.attribute;
            buf.layers[0].set_char(p, ch);
        }
    }

    fn _reset_on_row_change(&mut self, caret: &mut Caret) {
        self._reset_screen();
        caret.reset_color_attribute();
    }

    fn print_char(&mut self, buf: &mut Buffer, caret: &mut Caret, ch: AttributedChar) {
        buf.layers[0].set_char(caret.pos, ch);
        self.caret_right(buf, caret);
    }

    fn caret_down(&mut self, buf: &mut Buffer, caret: &mut Caret) {
        caret.index(buf, 0);

        /*       caret.pos.y += 1;
        if caret.pos.y >= buf.terminal_state.get_height() {
            caret.pos.y = 0;
        }
        self.reset_on_row_change(caret);*/
    }

    fn caret_up(buf: &Buffer, caret: &mut Caret) {
        if caret.pos.y > 0 {
            caret.pos.y = caret.pos.y.saturating_sub(1);
        } else {
            caret.pos.y = buf.terminal_state.get_height() - 1;
        }
    }

    fn caret_right(&mut self, buf: &mut Buffer, caret: &mut Caret) {
        caret.pos.x += 1;
        if caret.pos.x >= buf.terminal_state.get_width() {
            caret.pos.x = 0;
            self.caret_down(buf, caret);
        }
    } // 9PSYRNY2
    // ADGJ

    #[allow(clippy::unused_self)]
    fn caret_left(&self, buf: &Buffer, caret: &mut Caret) {
        if caret.pos.x > 0 {
            caret.pos.x = caret.pos.x.saturating_sub(1);
        } else {
            caret.pos.x = buf.terminal_state.get_width() - 1;
            Parser::caret_up(buf, caret);
        }
    }

    fn interpret_char(&mut self, buf: &mut Buffer, caret: &mut Caret, ch: u8) -> CallbackAction {
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
        let ach = AttributedChar::new(print_ch as char, caret.attribute);
        self.print_char(buf, caret, ach);

        CallbackAction::Update
    }
}

#[derive(Default)]
pub struct CharConverter {}

impl UnicodeConverter for CharConverter {
    fn convert_from_unicode(&self, ch: char, _font_page: usize) -> char {
        if ch == ' ' {
            return ' ';
        }
        match constants::UNICODE_TO_VIEWDATA.get(&ch) {
            Some(out_ch) => *out_ch,
            _ => ch,
        }
    }

    fn convert_to_unicode(&self, attributed_char: AttributedChar) -> char {
        match constants::VIEWDATA_TO_UNICODE.get(attributed_char.ch as usize) {
            Some(out_ch) => *out_ch,
            _ => attributed_char.ch,
        }
    }
}

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut Buffer, current_layer: usize, caret: &mut Caret, ch: char) -> EngineResult<CallbackAction> {
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
                self.caret_left(buf, caret);
            }
            // cursor forward
            9 => {
                self.caret_right(buf, caret);
            }
            // cursor down
            10 => {
                self.caret_down(buf, caret);
            }
            // cursor up
            11 => {
                Parser::caret_up(buf, caret);
            }
            // clear text window
            12 => {
                buf.reset_terminal();
                buf.layers[current_layer].clear();
                caret.pos = Position::default();
                caret.reset_color_attribute();
            }
            // return
            13 => {
                caret.cr(buf);
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
                caret.home(buf);
            }
            31 => {
                // Home the graphics cursor to the top left of the screen.
            }
            127 => {
                // Backspace and delete
                caret.bs(buf, current_layer);
            }
            129..=135 => {
                // Alpha Red, Green, Yellow, Blue, Magenta, Cyan, White
                self.is_in_graphic_mode = false;
                caret.attribute.set_is_concealed(false);
                self.held_graphics_character = ' ';
                caret.attribute.set_foreground(1 + (ch - 129) as u32);
                Parser::fill_to_eol(buf, caret);

                self.print_char(buf, caret, AttributedChar::new(' ', caret.attribute));
            }
            // Flash
            136 => {
                caret.attribute.set_is_blinking(true);
                Parser::fill_to_eol(buf, caret);
                self.print_char(buf, caret, AttributedChar::new(' ', caret.attribute));
            }
            // Steady
            137 => {
                caret.attribute.set_is_blinking(false);
                Parser::fill_to_eol(buf, caret);
                self.print_char(buf, caret, AttributedChar::new(' ', caret.attribute));
            }

            // normal height
            140 => {
                caret.attribute.set_is_double_height(false);
                Parser::fill_to_eol(buf, caret);
                self.print_char(buf, caret, AttributedChar::new(' ', caret.attribute));
            }

            // double height
            141 => {
                caret.attribute.set_is_double_height(true);
                Parser::fill_to_eol(buf, caret);
                self.print_char(buf, caret, AttributedChar::new(' ', caret.attribute));
            }

            145..=151 => {
                // Graphics Red, Green, Yellow, Blue, Magenta, Cyan, White
                if !self.is_in_graphic_mode {
                    self.is_in_graphic_mode = true;
                    self.held_graphics_character = ' ';
                }
                caret.attribute.set_is_concealed(false);
                caret.attribute.set_foreground(1 + (ch - 145) as u32);
                Parser::fill_to_eol(buf, caret);
                self.print_char(buf, caret, AttributedChar::new(' ', caret.attribute));
            }

            // conceal
            152 => {
                if !self.is_in_graphic_mode {
                    caret.attribute.set_is_concealed(true);
                    Parser::fill_to_eol(buf, caret);
                }
                self.print_char(buf, caret, AttributedChar::new(' ', caret.attribute));
            }

            // Contiguous Graphics
            153 => {
                self.is_contiguous = true;
                self.is_in_graphic_mode = true;
                self.print_char(buf, caret, AttributedChar::new(' ', caret.attribute));
            }
            // Separated Graphics
            154 => {
                self.is_contiguous = false;
                self.is_in_graphic_mode = true;
                self.print_char(buf, caret, AttributedChar::new(' ', caret.attribute));
            }

            // Black Background
            156 => {
                caret.attribute.set_is_concealed(false);
                caret.attribute.set_background(0);
                Parser::fill_to_eol(buf, caret);
                self.print_char(buf, caret, AttributedChar::new(' ', caret.attribute));
            }

            // New Background
            157 => {
                caret.attribute.set_background(caret.attribute.get_foreground());
                Parser::fill_to_eol(buf, caret);
                self.print_char(buf, caret, AttributedChar::new(' ', caret.attribute));
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
                return Ok(self.interpret_char(buf, caret, ch));
            }
        }
        self.got_esc = false;
        Ok(CallbackAction::Update)
    }
}
