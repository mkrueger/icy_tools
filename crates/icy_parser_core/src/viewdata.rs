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

use crate::{Blink, Color, CommandParser, CommandSink, SgrAttribute, TerminalCommand};

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
    fn reset_state(&mut self) {
        self.got_esc = false;
        self.hold_graphics = false;
        self.held_graphics_character = b' ';
        self.is_contiguous = true;
        self.is_in_graphic_mode = false;
    }

    /// Process a character with current mode settings
    fn process_char(&mut self, ch: u8) -> u8 {
        if self.got_esc || ch < 0x20 {
            // Control code or ESC sequence - print held graphic or space
            if self.hold_graphics { self.held_graphics_character } else { b' ' }
        } else if self.is_in_graphic_mode {
            // Graphics mode - remap characters to graphics set
            if (0x20..0x40).contains(&ch) || (0x60..0x80).contains(&ch) {
                let mut mapped = if ch < 0x40 { ch - 0x20 } else { ch - 0x40 };

                // Add offset for contiguous/separated graphics
                mapped += if self.is_contiguous { 0x80 } else { 0xC0 };

                self.held_graphics_character = mapped;
                mapped
            } else {
                ch
            }
        } else {
            // Alpha mode - use character as-is
            ch
        }
    }
}

impl CommandParser for ViewdataParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        for &byte in input {
            // Handle control codes first
            match byte {
                // Cursor movement
                0x08 => {
                    // Cursor left
                    sink.emit(TerminalCommand::CsiCursorBack(1));
                    self.got_esc = false;
                    continue;
                }
                0x09 => {
                    // Cursor right
                    sink.emit(TerminalCommand::CsiCursorForward(1));
                    self.got_esc = false;
                    continue;
                }
                0x0A => {
                    // Cursor down (resets state on new row)
                    sink.emit(TerminalCommand::CsiCursorDown(1));
                    self.reset_state();
                    continue;
                }
                0x0B => {
                    // Cursor up
                    sink.emit(TerminalCommand::CsiCursorUp(1));
                    self.got_esc = false;
                    continue;
                }
                0x0C => {
                    // Form feed / clear screen
                    sink.emit(TerminalCommand::CsiEraseInDisplay(crate::EraseInDisplayMode::All));
                    sink.emit(TerminalCommand::CsiCursorPosition(1, 1));
                    self.reset_state();
                    continue;
                }
                0x0D => {
                    // Carriage return
                    sink.emit(TerminalCommand::CarriageReturn);
                    self.got_esc = false;
                    continue;
                }
                0x11 => {
                    // Show cursor
                    // Note: Cursor visibility is typically handled by the consumer
                    self.got_esc = false;
                    continue;
                }
                0x14 => {
                    // Hide cursor
                    // Note: Cursor visibility is typically handled by the consumer
                    self.got_esc = false;
                    continue;
                }
                0x1E => {
                    // Home cursor
                    sink.emit(TerminalCommand::CsiCursorPosition(1, 1));
                    self.got_esc = false;
                    continue;
                }
                0x1B => {
                    // ESC - next byte is a command
                    self.got_esc = true;
                    continue;
                }
                0x00..=0x07 | 0x0E..=0x10 | 0x12..=0x13 | 0x15..=0x1D | 0x1F => {
                    // Other control codes - ignore but reset ESC state
                    self.got_esc = false;
                    continue;
                }
                _ => {}
            }

            // Handle ESC sequences
            if self.got_esc {
                match byte {
                    b'A'..=b'G' => {
                        // Alpha colors: Red, Green, Yellow, Blue, Magenta, Cyan, White
                        self.is_in_graphic_mode = false;
                        self.held_graphics_character = b' ';
                        let color = 1 + (byte - b'A') as u8;
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(color))));
                        // Also turn off concealed
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Concealed(false)));
                    }
                    b'Q'..=b'W' => {
                        // Graphics colors: Red, Green, Yellow, Blue, Magenta, Cyan, White
                        if !self.is_in_graphic_mode {
                            self.is_in_graphic_mode = true;
                            self.held_graphics_character = b' ';
                        }
                        let color = 1 + (byte - b'Q') as u8;
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(color))));
                        // Also turn off concealed
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Concealed(false)));
                    }
                    b'H' => {
                        // Flash/blink on
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(Blink::Slow)));
                    }
                    b'I' => {
                        // Steady (blink off)
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(Blink::Off)));
                    }
                    b'M' => {
                        // Double height (not directly supported in TerminalCommand)
                        // Consumer would need to handle this
                    }
                    b'L' => {
                        // Normal height (cancel double height)
                        // Consumer would need to handle this
                    }
                    b'X' => {
                        // Conceal (only in alpha mode)
                        if !self.is_in_graphic_mode {
                            sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Concealed(true)));
                        }
                    }
                    b'Y' => {
                        // Contiguous graphics
                        self.is_contiguous = true;
                        self.is_in_graphic_mode = true;
                    }
                    b'Z' => {
                        // Separated graphics
                        self.is_contiguous = false;
                    }
                    b'^' => {
                        // Hold graphics
                        self.hold_graphics = true;
                        self.is_in_graphic_mode = true;
                    }
                    b'_' => {
                        // Release graphics
                        self.hold_graphics = false;
                    }
                    b'\\' => {
                        // Black background
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(0))));
                        // Turn off concealed
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Concealed(false)));
                    }
                    b']' => {
                        // New background (use current foreground as background)
                        // This is a special case that would need consumer support
                        // We'll just emit the command and let consumer handle it
                    }
                    _ => {
                        // Unknown ESC sequence - ignore
                    }
                }

                // Process the character for display (space or held graphic)
                let display_char = self.process_char(byte);
                if display_char != b' ' || byte >= 0x20 {
                    sink.emit(TerminalCommand::Printable(&[display_char]));
                } else {
                    // Emit space for control codes
                    sink.emit(TerminalCommand::Printable(b" "));
                }

                // Update hold graphics state
                if !self.hold_graphics {
                    self.held_graphics_character = b' ';
                }

                self.got_esc = false;
            } else {
                // Regular printable character
                let display_char = self.process_char(byte);
                sink.emit(TerminalCommand::Printable(&[display_char]));

                // Update held graphics character if in graphics mode
                if !self.hold_graphics {
                    self.held_graphics_character = b' ';
                }
            }
        }
    }
}
