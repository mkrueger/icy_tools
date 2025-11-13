//! CTRL-A (Wildcat!) parser
//!
//! Uses `^A` (0x01) followed by a command character:
//! - Color codes: K,B,G,C,R,M,Y,W (foreground) and 0,4,2,6,1,5,3,7 (background)
//! - Cursor: L (clear screen), ' (home), < (left), ] (down)
//! - Attributes: H (bold/high intensity), I (blink), E (high background), N (normal)
//! - Other: J (clear down), > (clear to EOL), | (CR), A (literal ^A), Z (EOF)

use crate::{CommandParser, CommandSink, TerminalCommand};

const CTRL_A: u8 = 0x01;

/// Wildcat! CTRL-A parser
pub struct CtrlAParser {
    in_sequence: bool,
    is_bold: bool,
    high_bg: bool,
}

// Foreground color codes: KBGCRMYW = Black, Blue, Green, Cyan, Red, Magenta, Yellow, White
pub const FG_CODES: &[u8] = b"KBGCRMYW";
// Background color codes: 04261537
pub const BG_CODES: &[u8] = b"04261537";

impl Default for CtrlAParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CtrlAParser {
    pub fn new() -> Self {
        Self {
            in_sequence: false,
            is_bold: false,
            high_bg: false,
        }
    }

    fn find_color_index(codes: &[u8], ch: u8) -> Option<u8> {
        codes.iter().position(|&c| c == ch).map(|i| i as u8)
    }
}

impl CommandParser for CtrlAParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        let mut start = 0;

        for (i, &byte) in input.iter().enumerate() {
            if self.in_sequence {
                self.in_sequence = false;

                // Emit any text before this command
                if start < i - 1 {
                    sink.emit(TerminalCommand::Printable(&input[start..i - 1]));
                }

                match byte {
                    // Cursor movement
                    b'L' => sink.emit(TerminalCommand::CsiEraseInDisplay(crate::EraseInDisplayMode::All)),
                    b'\'' => sink.emit(TerminalCommand::CsiCursorPosition(1, 1)), // Home
                    b'J' => sink.emit(TerminalCommand::CsiEraseInDisplay(crate::EraseInDisplayMode::CursorToEnd)),
                    b'>' => sink.emit(TerminalCommand::CsiEraseInLine(crate::EraseInLineMode::CursorToEnd)),
                    b'<' => sink.emit(TerminalCommand::CsiCursorBack(1)),
                    b']' => sink.emit(TerminalCommand::CsiCursorDown(1)),
                    b'|' => sink.emit(TerminalCommand::Printable(b"\r")),

                    // Literal CTRL-A
                    b'A' => sink.emit(TerminalCommand::Printable(&[CTRL_A])),

                    // Attributes
                    b'H' => {
                        self.is_bold = true;
                        // Bold is represented as high intensity foreground (add 8 to color)
                        // This will be applied to the next foreground color command
                    }
                    b'I' => sink.emit(TerminalCommand::CsiSelectGraphicRendition(crate::SgrAttribute::Blink(crate::Blink::Slow))),
                    b'E' => {
                        self.high_bg = true;
                        // Enable iCE colors mode (background intensity instead of blink)
                        sink.emit(TerminalCommand::CsiDecPrivateModeSet(crate::DecPrivateMode::IceColors));
                    }
                    b'N' => {
                        self.is_bold = false;
                        self.high_bg = false;
                        // Disable iCE colors mode
                        sink.emit(TerminalCommand::CsiDecPrivateModeReset(crate::DecPrivateMode::IceColors));
                        // Reset attributes
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(crate::SgrAttribute::Reset));
                    }

                    // End of file marker
                    b'Z' => { /* EOF - do nothing */ }

                    // Color codes
                    ch if (b'K'..=b'Z').contains(&ch) || ch.is_ascii_digit() => {
                        if let Some(fg) = Self::find_color_index(FG_CODES, ch) {
                            let color = fg + if self.is_bold { 8 } else { 0 };
                            sink.emit(TerminalCommand::CsiSelectGraphicRendition(crate::SgrAttribute::Foreground(crate::Color::Base(
                                color,
                            ))));
                        } else if let Some(bg) = Self::find_color_index(BG_CODES, ch) {
                            let color = bg + if self.high_bg { 8 } else { 0 };
                            sink.emit(TerminalCommand::CsiSelectGraphicRendition(crate::SgrAttribute::Background(crate::Color::Base(
                                color,
                            ))));
                        } else if ch >= 128 {
                            // Cursor right (128-255 = move right by N-127)
                            let count = (ch - 127) as u16;
                            sink.emit(TerminalCommand::CsiCursorForward(count));
                        }
                    }

                    _ => {
                        // Unknown CTRL-A sequence, emit as-is
                        sink.emit(TerminalCommand::Printable(&[CTRL_A, byte]));
                    }
                }

                start = i + 1;
            } else if byte == CTRL_A {
                // Emit any text before this
                if start < i {
                    sink.emit(TerminalCommand::Printable(&input[start..i]));
                }
                self.in_sequence = true;
                start = i + 1; // Skip the CTRL_A byte
            }
        }

        // Emit any remaining text
        if start < input.len() && !self.in_sequence {
            sink.emit(TerminalCommand::Printable(&input[start..]));
        }
    }
}
