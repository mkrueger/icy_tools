#![allow(clippy::match_same_arms)]
use super::BufferParser;
use crate::{AttributedChar, CallbackAction, EditableScreen, EngineResult, Position, Size};

pub(crate) mod constants;

pub const VIEWDATA_SCREEN_SIZE: Size = Size { width: 40, height: 24 };

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

    fn reset_on_row_change(&mut self, buf: &mut dyn EditableScreen) {
        self.reset_screen();
        buf.caret_default_colors();
    }

    fn print_char(&mut self, buf: &mut dyn EditableScreen, ch: AttributedChar) {
        buf.set_char(buf.caret().position(), ch);
        self.caret_right(buf);
    }

    fn caret_down(&mut self, buf: &mut dyn EditableScreen) {
        let y = buf.caret().y;
        buf.caret_mut().y = y + 1;
        if buf.caret().y >= buf.terminal_state().get_height() {
            buf.caret_mut().y = 0;
        }
        self.reset_on_row_change(buf);
    }

    fn caret_up(&self, buf: &mut dyn EditableScreen) {
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
            self.caret_down(buf);
        }
    }

    #[allow(clippy::unused_self)]
    fn caret_left(&self, buf: &mut dyn EditableScreen) {
        if buf.caret().x > 0 {
            let x = buf.caret().x.saturating_sub(1);
            buf.caret_mut().x = x;
        } else {
            let x = buf.terminal_state().get_width().saturating_sub(1);
            buf.caret_mut().x = x;
            self.caret_up(buf);
        }
    }

    fn interpret_char(&mut self, buf: &mut dyn EditableScreen, ch: u8) -> CallbackAction {
        if self.got_esc {
            match ch {
                b'\\' => {
                    // Black Background
                    buf.caret_mut().attribute.set_is_concealed(false);
                    buf.caret_mut().attribute.set_background(0);
                    Parser::fill_to_eol(buf);
                }
                b']' => {
                    let fg = buf.caret_mut().attribute.get_foreground();
                    buf.caret_mut().attribute.set_background(fg);
                    Parser::fill_to_eol(buf);
                }
                b'I' => {
                    buf.caret_mut().attribute.set_is_blinking(false);
                    Parser::fill_to_eol(buf);
                }
                b'L' => {
                    buf.caret_mut().attribute.set_is_double_height(false);
                    Parser::fill_to_eol(buf);
                }
                b'X' => {
                    if !self.is_in_graphic_mode {
                        buf.caret_mut().attribute.set_is_concealed(true);
                        Parser::fill_to_eol(buf);
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
        let ach = AttributedChar::new(print_ch as char, buf.caret().attribute);
        self.print_char(buf, ach);

        if self.got_esc {
            match ch {
                b'A'..=b'G' => {
                    // Alpha Red, Green, Yellow, Blue, Magenta, Cyan, White
                    self.is_in_graphic_mode = false;
                    buf.caret_mut().attribute.set_is_concealed(false);
                    self.held_graphics_character = ' ';
                    buf.caret_mut().attribute.set_foreground(1 + (ch - b'A') as u32);
                    Parser::fill_to_eol(buf);
                }
                b'Q'..=b'W' => {
                    // Graphics Red, Green, Yellow, Blue, Magenta, Cyan, White
                    if !self.is_in_graphic_mode {
                        self.is_in_graphic_mode = true;
                        self.held_graphics_character = ' ';
                    }
                    buf.caret_mut().attribute.set_is_concealed(false);
                    buf.caret_mut().attribute.set_foreground(1 + (ch - b'Q') as u32);
                    Parser::fill_to_eol(buf);
                }
                b'H' => {
                    buf.caret_mut().attribute.set_is_blinking(true);
                    Parser::fill_to_eol(buf);
                }

                b'M' => {
                    buf.caret_mut().attribute.set_is_double_height(true);
                    Parser::fill_to_eol(buf);
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

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut dyn EditableScreen, ch: char) -> EngineResult<CallbackAction> {
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
                self.caret_left(buf);
            }
            0b000_1001 => {
                // Caret right 0x09
                self.caret_right(buf);
            }
            0b000_1010 => {
                // Caret down 0x0A
                self.caret_down(buf);
            }
            0b000_1011 => {
                // Caret up 0x0B
                self.caret_up(buf);
            }
            0b000_1100 => {
                // 12 / 0x0C - Form feed/clear screen
                // Preserve caret visibility (e.g., if hidden by 0x14)
                let was_visible = buf.caret().visible;
                buf.reset_terminal();
                buf.caret_mut().visible = was_visible;
                buf.clear_screen();
                buf.caret_default_colors();

                self.reset_screen();
            }
            0b000_1101 => {
                // 13 / 0x0D
                buf.cr();
            }
            0b000_1110 => {
                return Ok(CallbackAction::None);
            } // TODO: SO - switch to G1 char set
            0b000_1111 => {
                return Ok(CallbackAction::None);
            } // TODO: SI - switch to G0 char set

            // control codes 1
            0b001_0000 => {} // ignore
            0b001_0001 => buf.caret_mut().visible = true,
            0b001_0010 => {} // ignore
            0b001_0011 => {} // ignore
            0b001_0100 => buf.caret_mut().visible = false,
            0b001_0101 => {} // NAK
            0b001_0110 => {} // ignore
            0b001_0111 => {} // ignore
            0b001_1000 => {} // CAN
            0b001_1001 => {} // ignore
            0b001_1010 => {} // ignore
            0b001_1011 => {
                self.got_esc = true;
                return Ok(CallbackAction::None);
            } // 0x1B ESC
            0b001_1100 => {
                return Ok(CallbackAction::None);
            } // TODO: SS2 - switch to G2 char set
            0b001_1101 => {
                return Ok(CallbackAction::None);
            } // TODO: SS3 - switch to G3 char set
            0b001_1110 => {
                // 28 / 0x1E
                buf.home();
            }
            0b001_1111 => {} // ignore
            _ => {
                return Ok(self.interpret_char(buf, ch));
            }
        }
        self.got_esc = false;
        Ok(CallbackAction::Update)
    }
}
