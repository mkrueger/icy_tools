//! PETSCII (PET Standard Code of Information Interchange) parser
//!
//! PETSCII is the character encoding used by Commodore computers (C64, C128, VIC-20, PET, etc.).
//! Key features:
//! - Control codes for cursor movement, colors, and screen operations
//! - Two character sets: unshifted (uppercase + graphics) and shifted (uppercase + lowercase)
//! - Reverse video mode
//! - C128 extended ESC sequences
//! - Character code remapping based on shift mode

use crate::{Color, CommandParser, CommandSink, Direction, EraseInDisplayMode, EraseInLineMode, SgrAttribute, TerminalCommand};

pub const C64_TERMINAL_SIZE: (i32, i32) = (40, 25);

// C64 color palette indices
const BLACK: u8 = 0x00;
const WHITE: u8 = 0x01;
const RED: u8 = 0x02;
const CYAN: u8 = 0x03;
const PURPLE: u8 = 0x04;
const GREEN: u8 = 0x05;
const BLUE: u8 = 0x06;
const YELLOW: u8 = 0x07;
const ORANGE: u8 = 0x08;
const BROWN: u8 = 0x09;
const PINK: u8 = 0x0a;
const GREY1: u8 = 0x0b;
const GREY2: u8 = 0x0c;
const LIGHT_GREEN: u8 = 0x0d;
const LIGHT_BLUE: u8 = 0x0e;
const GREY3: u8 = 0x0f;

/// PETSCII parser for Commodore 8-bit computer systems
pub struct PetsciiParser {
    /// Escape sequence state (for C128 ESC sequences)
    got_esc: bool,
    /// Reverse video mode
    reverse_mode: bool,
    /// Underline mode (C128)
    underline_mode: bool,
    /// Shift mode: true = shifted (upper+lowercase), false = unshifted (upper+graphics)
    shift_mode: bool,
    /// Capital shift: forces uppercase in shifted mode
    c_shift: bool,
}

impl Default for PetsciiParser {
    fn default() -> Self {
        Self::new()
    }
}

impl PetsciiParser {
    pub fn new() -> Self {
        Self {
            got_esc: false,
            reverse_mode: false,
            underline_mode: false,
            shift_mode: false,
            c_shift: false,
        }
    }

    #[inline]
    pub fn apply_reverse(&self, ch: u8) -> u8 {
        if self.reverse_mode { ch | 0x80 } else { ch }
    }
    /// Convert PETSCII byte to internal screen code
    fn petscii_to_internal(&self, code: u8) -> Option<u8> {
        let mapped = match code {
            0x20..=0x3F => code,
            0x40..=0x5F => code - 0x40,
            0x60..=0x7F => code - 0x20, // Lowercase/graphics range
            0xA0..=0xBF => code - 0x40,
            0xC0..=0xFE => code - 0x80,
            _ => return None,
        };
        Some(mapped)
    }

    /// Emit a printable character with current modes applied
    fn emit_char(&self, sink: &mut dyn CommandSink, byte: u8) {
        if let Some(tch) = self.petscii_to_internal(byte) {
            let tch = self.apply_reverse(tch);
            sink.print(&[tch]);
        }
    }

    /// Produce a human-readable description of a PETSCII byte including its role
    /// (control, color change, shift toggle, printable mapping, etc.) and current parser modes.
    pub fn debug_description(&self, byte: u8) -> String {
        let mode = format!(
            "[shift={} c_shift={} reverse={} underline={}]",
            if self.shift_mode { "on" } else { "off" },
            if self.c_shift { "on" } else { "off" },
            if self.reverse_mode { "on" } else { "off" },
            if self.underline_mode { "on" } else { "off" },
        );

        let desc = match byte {
            0x02 => "Enable underline",
            0x03 => "Disable underline",
            0x05 => "Set foreground WHITE",
            0x07 => "Bell (BEEP)",
            0x08 => "Capital shift OFF",
            0x09 => "Capital shift ON",
            0x0A => "CR (Carriage Return)",
            0x0D | 0x8D => "LF (Line Feed) + reset reverse",
            0x0E => "Shift mode: UNshifted (upper+graphics)",
            0x0F => "Shift mode: SHIFTED (upper+lowercase)",
            0x11 => "Cursor DOWN",
            0x12 => "Reverse ON",
            0x13 => "Home cursor",
            0x14 => "Backspace",
            0x1B => "ESC (C128 escape sequence follows)",
            0x1C => "Set foreground RED",
            0x1D => "Cursor RIGHT",
            0x1E => "Set foreground GREEN",
            0x1F => "Set foreground BLUE",
            0x81 => "Set foreground ORANGE",
            0x8E => "SHIFT IN (shifted mode)",
            0x90 => "Set foreground BLACK",
            0x91 => "Cursor UP",
            0x92 => "Reverse OFF",
            0x93 => "Clear screen",
            0x95 => "Set foreground BROWN",
            0x96 => "Set foreground PINK",
            0x97 => "Set foreground GREY1",
            0x98 => "Set foreground GREY2",
            0x99 => "Set foreground LIGHT_GREEN",
            0x9A => "Set foreground LIGHT_BLUE",
            0x9B => "Set foreground GREY3",
            0x9C => "Set foreground PURPLE",
            0x9D => "Cursor LEFT",
            0x9E => "Set foreground YELLOW",
            0x9F => "Set foreground CYAN",
            0xFF => "Printable PI character",
            b if matches!(b, 0x20..=0x7F | 0xA0..=0xBF | 0xC0..=0xFE) => {
                // Attempt mapping to internal code to show transformed character
                match self.petscii_to_internal(byte) {
                    Some(mapped) => {
                        // Show resulting code with potential reverse application
                        let rev_mapped = self.apply_reverse(mapped);
                        let display = if rev_mapped.is_ascii_graphic() { rev_mapped as char } else { '·' };
                        return format!(
                            "Printable PETSCII '{}' (0x{:02X}) -> internal 0x{:02X} '{}' {}",
                            byte as char, byte, mapped, display, mode
                        );
                    }
                    None => return format!("Printable PETSCII '{}' (0x{:02X}) [mapping error] {}", byte as char, byte, mode),
                }
            }
            _ => "Unknown / Unsupported control",
        };

        format!("{} (0x{:02X}) {}", desc, byte, mode)
    }
}

impl CommandParser for PetsciiParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        let mut start = 0;

        for (i, &byte) in input.iter().enumerate() {
            //println!("byte {:02X} {}", byte, self.debug_description(byte));
            // Handle C128 escape sequences
            if self.got_esc {
                self.got_esc = false;

                // Emit any pending printable text
                if i > 0 && start < i - 1 {
                    for &b in &input[start..i - 1] {
                        self.emit_char(sink, b);
                    }
                }

                // Handle C128 ESC codes
                match byte {
                    b'O' => {}                                                                              // Cancel quote/insert (not implemented)
                    b'Q' => sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::CursorToEnd)),       // Clear to end of line
                    b'P' => sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::StartToCursor)),     // Clear to start of line
                    b'@' => sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::CursorToEnd)), // Clear screen down
                    b'J' => sink.emit(TerminalCommand::CarriageReturn),
                    b'K' => sink.emit(TerminalCommand::LineFeed), // EOL
                    b'A' => {}                                    // Auto-insert mode (not implemented)
                    b'C' => {}                                    // Disable auto-insert mode (not implemented)
                    b'D' => sink.emit(TerminalCommand::CsiDeleteLine(1)),
                    b'I' => sink.emit(TerminalCommand::CsiInsertLine(1)),
                    b'Y' => {} // Set default tab stops (not implemented)
                    b'Z' => sink.emit(TerminalCommand::CsiClearAllTabs),
                    b'L' => {} // Enable scrolling (not implemented)
                    b'M' => {} // Disable scrolling (not implemented)
                    b'V' => {} // Scroll up (not implemented)
                    b'W' => {} // Scroll down (not implemented)
                    b'G' => {} // Enable bell (not implemented)
                    b'H' => {} // Disable bell (not implemented)
                    b'E' => {} // Cursor non-flashing (not implemented)
                    b'F' => {} // Cursor flashing (not implemented)
                    b'B' => {} // Set bottom window (not implemented)
                    b'T' => {} // Set top window (not implemented)
                    b'X' => {} // Swap 40/80 columns (not implemented)
                    b'U' => {} // Underline cursor (not implemented)
                    b'S' => {} // Block cursor (not implemented)
                    b'R' => {} // Screen reverse video (not implemented)
                    b'N' => {} // Screen normal video (not implemented)
                    _ => {}    // Unknown escape code
                }

                start = i + 1;
                continue;
            }

            // Check for control characters and special PETSCII codes
            match byte {
                // Underline control (C128)
                0x02 => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    self.underline_mode = true;
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Underline(crate::Underline::Single)));
                    start = i + 1;
                }
                0x03 => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    self.underline_mode = false;
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Underline(crate::Underline::Off)));
                    start = i + 1;
                }

                // Color codes
                0x05 => {
                    // WHITE
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(WHITE))));
                    start = i + 1;
                }
                0x1C => {
                    // RED
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(RED))));
                    start = i + 1;
                }
                0x1E => {
                    // GREEN
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(GREEN))));
                    start = i + 1;
                }
                0x1F => {
                    // BLUE
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(BLUE))));
                    start = i + 1;
                }
                0x81 => {
                    // ORANGE
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(ORANGE))));
                    start = i + 1;
                }
                0x90 => {
                    // BLACK
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(BLACK))));
                    start = i + 1;
                }
                0x95 => {
                    // BROWN
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(BROWN))));
                    start = i + 1;
                }
                0x96 => {
                    // PINK
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(PINK))));
                    start = i + 1;
                }
                0x97 => {
                    // GREY1
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(GREY1))));
                    start = i + 1;
                }
                0x98 => {
                    // GREY2
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(GREY2))));
                    start = i + 1;
                }
                0x99 => {
                    // LIGHT_GREEN
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(LIGHT_GREEN))));
                    start = i + 1;
                }
                0x9A => {
                    // LIGHT_BLUE
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(LIGHT_BLUE))));
                    start = i + 1;
                }
                0x9B => {
                    // GREY3
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(GREY3))));
                    start = i + 1;
                }
                0x9C => {
                    // PURPLE
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(PURPLE))));
                    start = i + 1;
                }
                0x9E => {
                    // YELLOW
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(YELLOW))));
                    start = i + 1;
                }
                0x9F => {
                    // CYAN
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(CYAN))));
                    start = i + 1;
                }

                // Bell
                0x07 => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::Bell);
                    start = i + 1;
                }

                // Capital shift control
                0x08 => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    self.c_shift = false;
                    start = i + 1;
                }
                0x09 => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    self.c_shift = true;
                    start = i + 1;
                }

                // Carriage return
                0x0A => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CarriageReturn);
                    start = i + 1;
                }

                // Line feed (resets reverse mode)
                0x0D => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::LineFeed);
                    self.reverse_mode = false;
                    start = i + 1;
                }

                // Line feed (no reverse reset)
                0x8D => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::LineFeed);
                    start = i + 1;
                }

                // Shift mode control
                0x0E => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    self.shift_mode = false; // Unshifted (uppercase + graphics)
                    sink.emit(TerminalCommand::SetFontPage(0));
                    start = i + 1;
                }
                0x0F | 0x8E => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    self.shift_mode = true; // Shifted (uppercase + lowercase)
                    sink.emit(TerminalCommand::SetFontPage(1));
                    start = i + 1;
                }

                // Cursor movement
                0x11 => {
                    // DOWN
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiMoveCursor(Direction::Down, 1));
                    start = i + 1;
                }
                0x91 => {
                    // UP
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiMoveCursor(Direction::Up, 1));
                    start = i + 1;
                }
                0x1D => {
                    // RIGHT
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiMoveCursor(Direction::Right, 1));
                    start = i + 1;
                }
                0x9D => {
                    // LEFT
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiMoveCursor(Direction::Left, 1));
                    start = i + 1;
                }

                // Reverse mode control
                0x12 => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    self.reverse_mode = true;
                    start = i + 1;
                }
                0x92 => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    self.reverse_mode = false;
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Inverse(false)));
                    start = i + 1;
                }

                // Home cursor
                0x13 => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiCursorPosition(1, 1));
                    start = i + 1;
                }

                // Backspace
                0x14 => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    // Backspace on C64 wraps around to end of previous line
                    sink.emit(TerminalCommand::CsiMoveCursor(Direction::Left, 1));
                    sink.emit(TerminalCommand::Delete);
                    start = i + 1;
                }

                // ESC (C128 escape sequences)
                0x1B => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    self.got_esc = true;
                    start = i + 1;
                }

                // Clear screen
                0x93 => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::All));
                    sink.emit(TerminalCommand::CsiCursorPosition(1, 1));
                    start = i + 1;
                }

                // Insert character
                0x94 => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.print(&[b' ']);
                    start = i + 1;
                }

                // PI character (special printable)
                0xFF => {
                    if start < i {
                        for &b in &input[start..i] {
                            self.emit_char(sink, b);
                        }
                    }
                    sink.print(&[94]); // PI character mapped to 94
                    start = i + 1;
                }

                _ => {
                    // Regular printable character - will be handled in bulk at the end
                }
            }
        }

        // Emit any remaining printable characters
        if start < input.len() && !self.got_esc {
            for &byte in &input[start..] {
                self.emit_char(sink, byte);
            }
        }
    }
}
