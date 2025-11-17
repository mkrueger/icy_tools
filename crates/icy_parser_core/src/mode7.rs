//! Mode7 (Teletext/BBC Micro) parser
//!
//! Mode7 is the teletext display mode used by the BBC Micro and British teletext services.
//! Key features:
//! - Control codes 0-31 for cursor movement and screen operations
//! - Control codes 128-159 for teletext/Mode7 attributes (colors, graphics, etc.)
//! - VDU sequences for advanced operations (colors, positioning, modes)
//! - Alpha and graphics modes with mosaic characters
//! - Hold graphics mode to retain last graphic character
//! - Double height text support
//! - Contiguous and separated graphics modes
//!
//! References:
//! - <https://www.bbcbasic.co.uk/bbcwin/manual/bbcwin8.html>
//! - <https://central.kaserver5.org/Kasoft/Typeset/BBC/Ch28.html>

use crate::{Blink, Color, CommandParser, CommandSink, Direction, EraseInDisplayMode, SgrAttribute, TerminalCommand};

/// Mode7/Teletext parser for BBC Micro and compatible systems
pub struct Mode7Parser {
    /// ESC (VDU 27) state - next char goes directly to screen
    got_esc: bool,
    /// VDU multi-byte sequence buffer
    vdu_queue: Vec<u8>,
    /// Expected total bytes for current VDU sequence
    vdu_expected: usize,

    /// Hold graphics mode - retain last graphics character
    hold_graphics: bool,
    /// Last graphics character to hold
    held_graphics_character: u8,
    /// Contiguous vs separated graphics mode
    is_contiguous: bool,
    /// Currently in graphics mode (vs alpha mode)
    is_in_graphic_mode: bool,

    /// VDU disabled state
    vdu_disabled: bool,

    /// Current foreground color
    current_fg: u8,
    /// Current background color  
    current_bg: u8,
}

impl Default for Mode7Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Mode7Parser {
    pub fn new() -> Self {
        Self {
            got_esc: false,
            vdu_queue: Vec::new(),
            vdu_expected: 0,
            hold_graphics: false,
            held_graphics_character: b' ',
            is_contiguous: true,
            is_in_graphic_mode: false,
            vdu_disabled: false,
            current_fg: 7, // White
            current_bg: 0, // Black
        }
    }

    /// Reset parser state (called on new line or clear screen)
    fn reset_state(&mut self) {
        self.is_in_graphic_mode = false;
        self.hold_graphics = false;
        self.held_graphics_character = b' ';
        self.is_contiguous = true;
        self.current_fg = 7;
        self.current_bg = 0;
    }

    /// Display space or held graphics for control positions
    fn display_control_char(&self, sink: &mut dyn CommandSink) {
        let display_ch = if self.hold_graphics && self.is_in_graphic_mode {
            self.held_graphics_character
        } else {
            b' '
        };
        sink.print(&[display_ch]);
    }

    /// Process graphics character with contiguous/separated mapping
    fn process_graphics_char(&mut self, ch: u8) -> u8 {
        if !self.is_in_graphic_mode {
            // In alpha mode, graphics chars display as spaces
            return b' ';
        }

        // Store as held graphics if in range
        if (160..=191).contains(&ch) || (224..=255).contains(&ch) {
            self.held_graphics_character = ch;
        }

        // Map to block graphics character
        if self.is_contiguous {
            // Contiguous graphics mapping
            if (160..=191).contains(&ch) {
                ch - 32 // Map to 128-159
            } else if (224..=255).contains(&ch) {
                ch - 64 // Map to 160-191
            } else {
                ch
            }
        } else {
            // Separated graphics mapping
            if (160..=191).contains(&ch) {
                ch + 32 // Map to 192-223
            } else {
                ch // 224-255 already in right range
            }
        }
    }

    /// Handle completed VDU sequence
    fn handle_vdu_sequence(&mut self, sink: &mut dyn CommandSink) {
        if self.vdu_queue.is_empty() {
            return;
        }

        match self.vdu_queue[0] {
            17 if self.vdu_queue.len() >= 2 => {
                // VDU 17,n - COLOUR n
                let color = self.vdu_queue[1];
                if color < 128 {
                    // Foreground
                    self.current_fg = color & 15;
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(
                        self.current_fg,
                    ))));
                } else {
                    // Background
                    self.current_bg = (color - 128) & 15;
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(
                        self.current_bg,
                    ))));
                }
            }
            22 if self.vdu_queue.len() >= 2 => {
                // VDU 22,n - MODE n (screen mode change)
                // Reset parser state
                self.reset_state();
                sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::All));
                sink.emit(TerminalCommand::CsiCursorPosition(1, 1));
            }
            31 if self.vdu_queue.len() >= 3 => {
                // VDU 31,x,y - TAB(x,y)
                let x = self.vdu_queue[1] as u16;
                let y = self.vdu_queue[2] as u16;
                sink.emit(TerminalCommand::CsiCursorPosition(y + 1, x + 1));
            }
            _ => {
                // Other VDU sequences - ignore for now
            }
        }

        self.vdu_queue.clear();
        self.vdu_expected = 0;
    }
}

impl CommandParser for Mode7Parser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        for &byte in input {
            // Handle VDU disabled state
            if self.vdu_disabled && byte != 6 {
                continue;
            }

            // Handle ESC (VDU 27) - next char goes directly to screen
            if self.got_esc {
                self.got_esc = false;
                sink.print(&[byte]);
                continue;
            }

            // Handle multi-byte VDU sequences
            if self.vdu_expected > 0 {
                self.vdu_queue.push(byte);
                if self.vdu_queue.len() >= self.vdu_expected {
                    self.handle_vdu_sequence(sink);
                }
                continue;
            }

            match byte {
                0 => {}     // Null - does nothing
                1..=3 => {} // Printer control - not implemented

                4 => {
                    // Write text at text cursor (graphics cursor mode off)
                }
                5 => {
                    // Write text at graphics cursor (does nothing in Mode 7)
                }
                6 => {
                    // Enable screen output
                    self.vdu_disabled = false;
                }
                7 => {
                    // Bell
                    sink.emit(TerminalCommand::Bell);
                }
                8 => {
                    // Cursor left
                    sink.emit(TerminalCommand::CsiMoveCursor(Direction::Left, 1));
                }
                9 => {
                    // Cursor right
                    sink.emit(TerminalCommand::CsiMoveCursor(Direction::Right, 1));
                }
                10 => {
                    // Cursor down (resets line state)
                    sink.emit(TerminalCommand::CsiMoveCursor(Direction::Down, 1));
                    self.reset_state();
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(
                        self.current_fg,
                    ))));
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(
                        self.current_bg,
                    ))));
                }
                11 => {
                    // Cursor up (resets line state)
                    sink.emit(TerminalCommand::CsiMoveCursor(Direction::Up, 1));
                    self.reset_state();
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(
                        self.current_fg,
                    ))));
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(
                        self.current_bg,
                    ))));
                }
                12 => {
                    // Clear screen (CLS)
                    sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::All));
                    sink.emit(TerminalCommand::CsiCursorPosition(1, 1));
                    self.reset_state();
                }
                13 => {
                    // Carriage return (resets line state)
                    sink.emit(TerminalCommand::CarriageReturn);
                    self.reset_state();
                }
                14..=16 => {} // Paging and graphics area clear - not implemented

                17 => {
                    // COLOUR n - expect 1 more byte
                    self.vdu_expected = 2;
                    self.vdu_queue.clear();
                    self.vdu_queue.push(byte);
                }
                18 => {
                    // GCOL mode,colour - expect 2 more bytes (ignored in Mode 7)
                    self.vdu_expected = 3;
                    self.vdu_queue.clear();
                    self.vdu_queue.push(byte);
                }
                19 => {
                    // VDU 19 - palette - expect 5 more bytes
                    self.vdu_expected = 6;
                    self.vdu_queue.clear();
                    self.vdu_queue.push(byte);
                }
                20 => {
                    // Restore default colors
                    self.current_fg = 7;
                    self.current_bg = 0;
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(7))));
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(0))));
                }
                21 => {
                    // Disable screen output
                    self.vdu_disabled = true;
                }
                22 => {
                    // MODE - expect 1 more byte
                    self.vdu_expected = 2;
                    self.vdu_queue.clear();
                    self.vdu_queue.push(byte);
                }
                23 => {
                    // Various - expect 9 more bytes
                    self.vdu_expected = 10;
                    self.vdu_queue.clear();
                    self.vdu_queue.push(byte);
                }
                24 => {
                    // Graphics viewport - expect 8 more bytes
                    self.vdu_expected = 9;
                    self.vdu_queue.clear();
                    self.vdu_queue.push(byte);
                }
                25 => {
                    // PLOT - expect 4 more bytes
                    self.vdu_expected = 5;
                    self.vdu_queue.clear();
                    self.vdu_queue.push(byte);
                }
                26 => {
                    // Reset viewports / home
                    sink.emit(TerminalCommand::CsiCursorPosition(1, 1));
                    self.reset_state();
                }
                27 => {
                    // Next char to screen (ESC)
                    self.got_esc = true;
                }
                28 => {
                    // Text viewport - expect 4 more bytes
                    self.vdu_expected = 5;
                    self.vdu_queue.clear();
                    self.vdu_queue.push(byte);
                }
                29 => {
                    // Graphics origin - expect 4 more bytes
                    self.vdu_expected = 5;
                    self.vdu_queue.clear();
                    self.vdu_queue.push(byte);
                }
                30 => {
                    // Home cursor
                    sink.emit(TerminalCommand::CsiCursorPosition(1, 1));
                    self.reset_state();
                }
                31 => {
                    // TAB(x,y) - expect 2 more bytes
                    self.vdu_expected = 3;
                    self.vdu_queue.clear();
                    self.vdu_queue.push(byte);
                }

                127 => {
                    // Destructive backspace
                    sink.emit(TerminalCommand::Backspace);
                    sink.print(b" ");
                    sink.emit(TerminalCommand::Backspace);
                }

                // Mode 7 control codes (128-159)
                129..=135 => {
                    // Alpha colors: Red, Green, Yellow, Blue, Magenta, Cyan, White
                    self.is_in_graphic_mode = false;
                    self.current_fg = 1 + (byte - 129);
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(
                        self.current_fg,
                    ))));
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Concealed(false)));
                    self.display_control_char(sink);
                }
                136 => {
                    // Flash
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(Blink::Slow)));
                    self.display_control_char(sink);
                }
                137 => {
                    // Steady
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(Blink::Off)));
                    self.display_control_char(sink);
                }
                140 => {
                    // Normal height (cancel double height)
                    // Double height would be handled by consumer
                    self.display_control_char(sink);
                }
                141 => {
                    // Double height
                    // Double height would be handled by consumer
                    self.display_control_char(sink);
                }
                145..=151 => {
                    // Graphics colors: Red, Green, Yellow, Blue, Magenta, Cyan, White
                    self.is_in_graphic_mode = true;
                    self.current_fg = 1 + (byte - 145);
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(
                        self.current_fg,
                    ))));
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Concealed(false)));
                    self.display_control_char(sink);
                }
                152 => {
                    // Conceal display
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Concealed(true)));
                    self.display_control_char(sink);
                }
                153 => {
                    // Contiguous graphics
                    self.is_contiguous = true;
                    self.display_control_char(sink);
                }
                154 => {
                    // Separated graphics
                    self.is_contiguous = false;
                    self.display_control_char(sink);
                }
                156 => {
                    // Black background
                    self.current_bg = 0;
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(0))));
                    self.display_control_char(sink);
                }
                157 => {
                    // New background (use current foreground color)
                    self.current_bg = self.current_fg;
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(
                        self.current_bg,
                    ))));
                    self.display_control_char(sink);
                }
                158 => {
                    // Hold graphics
                    self.hold_graphics = true;
                    self.display_control_char(sink);
                }
                159 => {
                    // Release graphics
                    self.hold_graphics = false;
                    self.display_control_char(sink);
                }

                // Printable characters
                32..=126 => {
                    // Normal ASCII printable
                    sink.print(&[byte]);
                }

                // Graphics characters
                160..=255 => {
                    let mapped = self.process_graphics_char(byte);
                    sink.print(&[mapped]);
                }

                _ => {
                    // Other control codes - emit as-is
                    sink.print(&[byte]);
                }
            }
        }
    }
}
