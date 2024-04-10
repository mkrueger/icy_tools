#![allow(clippy::match_same_arms)]
use super::BufferParser;
use crate::{AttributedChar, Buffer, CallbackAction, Caret, EngineResult, Position, TextPane, UnicodeConverter};

mod constants;

#[cfg(test)]
mod tests;

/// <https://www.blunham.com/Radar/Teletext/PDFs/Viewdata1976Spec.pdf>
pub struct Parser {
    got_esc: bool,

    hold_graphics: bool,
    held_graphics_character: char,

    is_contiguous: bool,

    is_in_graphic_mode: bool,

    graphics_bg: u32,
    alpha_bg: u32,
}

impl Default for Parser {
    fn default() -> Self {
        Self {
            got_esc: false,
            hold_graphics: false,
            held_graphics_character: ' ',
            is_contiguous: true,
            is_in_graphic_mode: false,
            graphics_bg: 0,
            alpha_bg: 0,
        }
    }
}

impl Parser {
    fn reset_screen(&mut self) {
        self.got_esc = false;

        self.hold_graphics = false;
        self.held_graphics_character = ' ';

        self.is_contiguous = true;
        self.is_in_graphic_mode = false;
        self.graphics_bg = 0;
        self.alpha_bg = 0;
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

    fn reset_on_row_change(&mut self, caret: &mut Caret) {
        self.reset_screen();
        caret.reset_color_attribute();
    }

    fn print_char(&mut self, buf: &mut Buffer, caret: &mut Caret, ch: AttributedChar) {
        buf.layers[0].set_char(caret.pos, ch);
        self.caret_right(buf, caret);
    }

    fn caret_down(&mut self, buf: &Buffer, caret: &mut Caret) {
        caret.pos.y += 1;
        if caret.pos.y >= buf.terminal_state.get_height() {
            caret.pos.y = 0;
        }
        self.reset_on_row_change(caret);
    }

    fn caret_up(buf: &Buffer, caret: &mut Caret) {
        if caret.pos.y > 0 {
            caret.pos.y = caret.pos.y.saturating_sub(1);
        } else {
            caret.pos.y = buf.terminal_state.get_height() - 1;
        }
    }

    fn caret_right(&mut self, buf: &Buffer, caret: &mut Caret) {
        caret.pos.x += 1;
        if caret.pos.x >= buf.terminal_state.get_width() {
            caret.pos.x = 0;
            self.caret_down(buf, caret);
        }
    }

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
        if self.got_esc {
            match ch {
                b'\\' => {
                    // Black Background
                    caret.attribute.set_is_concealed(false);
                    caret.attribute.set_background(0);
                    Parser::fill_to_eol(buf, caret);
                }
                b']' => {
                    caret.attribute.set_background(caret.attribute.get_foreground());
                    Parser::fill_to_eol(buf, caret);
                }
                b'I' => {
                    caret.attribute.set_is_blinking(false);
                    Parser::fill_to_eol(buf, caret);
                }
                b'L' => {
                    caret.attribute.set_is_double_height(false);
                    Parser::fill_to_eol(buf, caret);
                }
                b'X' => {
                    if !self.is_in_graphic_mode {
                        caret.attribute.set_is_concealed(true);
                        Parser::fill_to_eol(buf, caret);
                    }
                }
                b'Y' => {
                    self.is_contiguous = true;
                    self.is_in_graphic_mode = true;
                }
                b'Z' => self.is_contiguous = false,
                b'^' => {
                    self.hold_graphics = true;
                    self.is_in_graphic_mode = true;
                }
                _ => {}
            }
        }
        if !self.hold_graphics {
            self.held_graphics_character = ' ';
        }

        let mut print_ch = ch;
        if self.got_esc || ch < 0x20 {
            print_ch = if self.hold_graphics { self.held_graphics_character as u8 } else { b' ' };
        } else if self.is_in_graphic_mode {
            if (0x20..0x40).contains(&ch) || (0x60..0x80).contains(&ch) {
                if print_ch < 0x40 {
                    print_ch -= 0x20;
                } else {
                    print_ch -= 0x40;
                }

                if self.is_contiguous {
                    print_ch += 0x80;
                } else {
                    print_ch += 0xC0;
                }
            }
            self.held_graphics_character = print_ch as char;
        }
        let ach = AttributedChar::new(print_ch as char, caret.attribute);
        self.print_char(buf, caret, ach);

        if self.got_esc {
            match ch {
                b'A'..=b'G' => {
                    // Alpha Red, Green, Yellow, Blue, Magenta, Cyan, White
                    self.is_in_graphic_mode = false;
                    caret.attribute.set_is_concealed(false);
                    self.held_graphics_character = ' ';
                    caret.attribute.set_foreground(1 + (ch - b'A') as u32);
                    Parser::fill_to_eol(buf, caret);
                }
                b'Q'..=b'W' => {
                    // Graphics Red, Green, Yellow, Blue, Magenta, Cyan, White
                    if !self.is_in_graphic_mode {
                        self.is_in_graphic_mode = true;
                        self.held_graphics_character = ' ';
                    }
                    caret.attribute.set_is_concealed(false);
                    caret.attribute.set_foreground(1 + (ch - b'Q') as u32);
                    Parser::fill_to_eol(buf, caret);
                }
                b'H' => {
                    caret.attribute.set_is_blinking(true);
                    Parser::fill_to_eol(buf, caret);
                }

                b'M' => {
                    caret.attribute.set_is_double_height(true);
                    Parser::fill_to_eol(buf, caret);
                }

                b'_' => {
                    self.hold_graphics = false;
                }

                _ => {}
            }
            self.got_esc = false;
        }
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
            // control codes 0
            0b000_0000 => {}                                           // ignore
            0b000_0001 => {}                                           // ignore
            0b000_0010 => {}                                           // STX
            0b000_0011 => {}                                           // ETX
            0b000_0100 => {}                                           // ignore
            0b000_0101 => { /*return Ok(Some("1\0".to_string())); */ } // ENQ - send identity number <= 16 digits - ignore doesn't work properly 2022
            0b000_0110 => {}                                           // ACK
            0b000_0111 => {}                                           // ignore
            0b000_1000 => {
                // Caret left 0x08
                self.caret_left(buf, caret);
            }
            0b000_1001 => {
                // Caret right 0x09
                self.caret_right(buf, caret);
            }
            0b000_1010 => {
                // Caret down 0x0A
                self.caret_down(buf, caret);
            }
            0b000_1011 => {
                // Caret up 0x0B
                Parser::caret_up(buf, caret);
            }
            0b000_1100 => {
                // 12 / 0x0C
                buf.reset_terminal();
                buf.layers[current_layer].clear();
                caret.pos = Position::default();
                caret.reset_color_attribute();

                self.reset_screen();
            }
            0b000_1101 => {
                // 13 / 0x0D
                caret.cr(buf);
            }
            0b000_1110 => {
                return Ok(CallbackAction::NoUpdate);
            } // TODO: SO - switch to G1 char set
            0b000_1111 => {
                return Ok(CallbackAction::NoUpdate);
            } // TODO: SI - switch to G0 char set

            // control codes 1
            0b001_0000 => {} // ignore
            0b001_0001 => caret.set_is_visible(true),
            0b001_0010 => {} // ignore
            0b001_0011 => {} // ignore
            0b001_0100 => caret.set_is_visible(false),
            0b001_0101 => {} // NAK
            0b001_0110 => {} // ignore
            0b001_0111 => {} // ignore
            0b001_1000 => {} // CAN
            0b001_1001 => {} // ignore
            0b001_1010 => {} // ignore
            0b001_1011 => {
                self.got_esc = true;
                return Ok(CallbackAction::NoUpdate);
            } // 0x1B ESC
            0b001_1100 => {
                return Ok(CallbackAction::NoUpdate);
            } // TODO: SS2 - switch to G2 char set
            0b001_1101 => {
                return Ok(CallbackAction::NoUpdate);
            } // TODO: SS3 - switch to G3 char set
            0b001_1110 => {
                // 28 / 0x1E
                caret.home(buf);
            }
            0b001_1111 => {} // ignore
            _ => {
                return Ok(self.interpret_char(buf, caret, ch));
            }
        }
        self.got_esc = false;
        Ok(CallbackAction::Update)
    }
}
