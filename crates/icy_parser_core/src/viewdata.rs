//! Viewdata/Prestel parser
//!
//! Viewdata is a videotex standard used primarily in the UK (Prestel) and other countries.
//! Key features:
//! - 40x24 character display
//! - ESC-based control codes for colors, graphics modes, and text attributes
//! - Alpha and graphics modes with separate foreground colors
//! - Mosaic graphics characters (contiguous and separated)
//! - Hold graphics mode to retain last graphic character
//! - Double height text support
//! - Concealed text mode
//!
//! Reference: <https://www.blunham.com/Radar/Teletext/PDFs/Viewdata1976Spec.pdf>

use crate::{Blink, Color, CommandParser, CommandSink, Direction, SgrAttribute, TerminalCommand, ViewDataCommand};

/// Viewdata/Prestel parser
pub struct ViewdataParser {
    /// ESC sequence state
    got_esc: bool,
    /// Hold graphics mode - retain last graphics character
    hold_graphics: bool,
    /// Last graphics character to hold
    held_graphics_character: u8,
    /// Contiguous vs separated graphics mode
    is_contiguous: bool,
    /// Currently in graphics mode (vs alpha mode)
    is_in_graphic_mode: bool,
}

impl Default for ViewdataParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewdataParser {
    pub fn new() -> Self {
        Self {
            got_esc: false,
            hold_graphics: false,
            held_graphics_character: b' ',
            is_contiguous: true,
            is_in_graphic_mode: false,
        }
    }

    /// Reset parser state (called on new row or clear screen)
    fn reset_screen(&mut self) {
        self.got_esc = false;
        self.hold_graphics = false;
        self.held_graphics_character = b' ';
        self.is_contiguous = true;
        self.is_in_graphic_mode = false;
    }

    fn reset_on_row_change(&mut self, sink: &mut dyn CommandSink) {
        self.reset_screen();
        sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Reset));
    }

    #[inline(always)]
    fn interpret_char(&mut self, sink: &mut dyn CommandSink, ch: u8) {
        if self.got_esc {
            match ch {
                b'\\' => {
                    // Black Background
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Concealed(false)));
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(0))));
                    sink.emit_view_data(ViewDataCommand::FillToEol);
                }
                b']' => {
                    sink.emit_view_data(ViewDataCommand::SetBgToFg);
                    sink.emit_view_data(ViewDataCommand::FillToEol);
                }
                b'I' => {
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(Blink::Off)));
                    sink.emit_view_data(ViewDataCommand::FillToEol);
                }
                b'L' => {
                    sink.emit_view_data(ViewDataCommand::DoubleHeight(false));
                    sink.emit_view_data(ViewDataCommand::FillToEol);
                }
                b'X' => {
                    if !self.is_in_graphic_mode {
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Concealed(true)));
                        sink.emit_view_data(ViewDataCommand::FillToEol);
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
            self.held_graphics_character = b' ';
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
            self.held_graphics_character = print_ch;
        }
        sink.emit_view_data(ViewDataCommand::SetChar(print_ch));
        if sink.emit_view_data(ViewDataCommand::MoveCaret(Direction::Right)) {
            self.reset_on_row_change(sink);
        }

        if self.got_esc {
            match ch {
                b'A'..=b'G' => {
                    // Alpha Red, Green, Yellow, Blue, Magenta, Cyan, White
                    self.is_in_graphic_mode = false;
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Concealed(false)));
                    self.held_graphics_character = b' ';
                    let color = 1 + (ch - b'A') as u8;
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(color))));
                    sink.emit_view_data(ViewDataCommand::FillToEol);
                }
                b'Q'..=b'W' => {
                    // Graphics Red, Green, Yellow, Blue, Magenta, Cyan, White
                    if !self.is_in_graphic_mode {
                        self.is_in_graphic_mode = true;
                        self.held_graphics_character = b' ';
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Concealed(false)));
                    let color = 1 + (ch - b'Q') as u8;
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(color))));
                    sink.emit_view_data(ViewDataCommand::FillToEol);
                }
                b'H' => {
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(Blink::Slow)));
                    sink.emit_view_data(ViewDataCommand::FillToEol);
                }

                b'M' => {
                    sink.emit_view_data(ViewDataCommand::DoubleHeight(true));
                    sink.emit_view_data(ViewDataCommand::FillToEol);
                }

                b'_' => {
                    self.hold_graphics = false;
                }

                _ => {}
            }
        }

        self.got_esc = false;
    }
}

impl CommandParser for ViewdataParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        for &byte in input {
            match byte {
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
                    sink.emit_view_data(ViewDataCommand::MoveCaret(Direction::Left));
                }
                0b000_1001 => {
                    // Caret right 0x09
                    if sink.emit_view_data(ViewDataCommand::MoveCaret(Direction::Right)) {
                        self.reset_on_row_change(sink);
                    }
                }
                0b000_1010 => {
                    // Caret down 0x0A
                    sink.emit_view_data(ViewDataCommand::MoveCaret(Direction::Down));
                    self.reset_on_row_change(sink);
                }
                0b000_1011 => {
                    // Caret up 0x0B
                    sink.emit_view_data(ViewDataCommand::MoveCaret(Direction::Up));
                }
                0b000_1100 => {
                    // 12 / 0x0C - Form feed/clear screen
                    // Preserve caret visibility (e.g., if hidden by 0x14)
                    sink.emit_view_data(ViewDataCommand::ViewDataClearScreen);
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Reset));
                    self.reset_screen();
                }
                0b000_1101 => {
                    // 13 / 0x0D
                    sink.emit(TerminalCommand::CarriageReturn);
                }
                0b000_1110 => {
                    continue;
                } // TODO: SO - switch to G1 char set
                0b000_1111 => {
                    continue;
                } // TODO: SI - switch to G0 char set

                // control codes 1
                0b001_0000 => {} // ignore
                0b001_0001 => sink.emit(TerminalCommand::CsiDecSetMode(crate::DecMode::CursorVisible, true)),
                0b001_0010 => {} // ignore
                0b001_0011 => {} // ignore
                0b001_0100 => sink.emit(TerminalCommand::CsiDecSetMode(crate::DecMode::CursorVisible, false)),
                0b001_0101 => {} // NAK
                0b001_0110 => {} // ignore
                0b001_0111 => {} // ignore
                0b001_1000 => {} // CAN
                0b001_1001 => {} // ignore
                0b001_1010 => {} // ignore
                0b001_1011 => {
                    self.got_esc = true;
                    continue;
                } // 0x1B ESC
                0b001_1100 => {
                    continue;
                } // TODO: SS2 - switch to G2 char set
                0b001_1101 => {
                    continue;
                } // TODO: SS3 - switch to G3 char set
                0b001_1110 => {
                    // 28 / 0x1E
                    sink.emit(TerminalCommand::CsiCursorPosition(1, 1));
                }
                0b001_1111 => {} // ignore
                _ => {
                    self.interpret_char(sink, byte);
                    continue;
                }
            }
            self.got_esc = false;
        }
    }
}
