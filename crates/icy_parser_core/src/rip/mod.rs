//! RIPscrip (Remote Imaging Protocol Script) parser
//!
//! RIPscrip is a graphics-based BBS protocol that extends ANSI art with vector graphics,
//! buttons, and mouse support. Commands start with !| and use base-36 encoded parameters.

use crate::{AnsiParser, CommandParser, CommandSink};
mod command;
pub use command::{BlockTransferMode, FileQueryMode, ImagePasteMode, QueryMode, RipCommand, WriteMode};

mod builder;
use builder::*;

mod emit;
mod parse_params;

#[derive(Default, Clone, Debug, PartialEq)]
enum State {
    #[default]
    Default,
    GotExclaim,
    GotPipe,
    ReadLevel1,
    ReadLevel9,
    ReadParams,
    GotEscape,               // Got ESC character
    GotEscBracket,           // Got ESC[
    ReadAnsiNumber(Vec<u8>), // Reading number after ESC[
}

#[derive(Default, Clone, Debug, PartialEq)]
enum ParserMode {
    #[default]
    NonRip, // Use ANSI parser for text
    Rip, // RIP command mode
}

pub struct RipParser {
    mode: ParserMode,
    state: State,
    builder: CommandBuilder,
    ansi_parser: AnsiParser,
    enabled: bool, // RIPscrip processing enabled/disabled
    got_backslash: bool,
    win_eol: bool,
}

impl RipParser {
    pub fn new() -> Self {
        Self {
            mode: ParserMode::default(),
            state: State::Default,
            builder: CommandBuilder::default(),
            ansi_parser: AnsiParser::new(),
            enabled: true, // RIPscrip starts enabled
            got_backslash: false,
            win_eol: false,
        }
    }
}

impl Default for RipParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandParser for RipParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        for &ch in input {
            // Check for backslash (line continuation) in any RIP state
            if self.mode == ParserMode::Rip {
                if ch == b'\r' {
                    self.win_eol = true;
                    continue;
                }
                if self.win_eol {
                    self.win_eol = false;
                    if b'\n' != ch {
                        // cancel line continuation
                        self.got_backslash = false;
                        log::error!("Expected \\n after \\r in RIP command, got: {}", ch);
                    }
                }

                if self.got_backslash {
                    self.got_backslash = false;
                    match ch {
                        b'\n' => {
                            // Line continuation - skip this character
                            continue;
                        }
                        b'\\' | b'|' | b'!' => {
                            // Escaped backslash or pipe - treat as normal character
                            if self.state == State::ReadParams {
                                self.builder.got_escape = true;
                            }
                        }
                        _ => {
                            // Not a continuation - treat previous backslash as normal character
                            self.state = State::Default;
                        }
                    }
                } else if ch == b'\\' {
                    self.got_backslash = true;
                    continue;
                }
            }

            match &self.state.clone() {
                State::Default => {
                    match self.mode {
                        ParserMode::NonRip => {
                            if ch == 0x1B {
                                // ESC character - check if it's an ANSI RIP control sequence
                                self.state = State::GotEscape;
                            } else if ch == b'!' && self.enabled {
                                self.mode = ParserMode::Rip;
                                self.state = State::GotExclaim;
                            } else {
                                // Pass through to ANSI parser
                                self.ansi_parser.parse(&[ch], sink);
                            }
                        }
                        ParserMode::Rip => {
                            if ch == 0x1B {
                                // ESC character - check if it's an ANSI RIP control sequence
                                self.state = State::GotEscape;
                            } else if ch == b'!' {
                                self.state = State::GotExclaim;
                            } else {
                                // In RIP mode without !, treat as error and go back to NonRip
                                self.mode = ParserMode::NonRip;
                                self.ansi_parser.parse(&[ch], sink);
                            }
                        }
                    }
                }
                State::GotEscape => {
                    if ch == b'[' {
                        self.state = State::GotEscBracket;
                    } else {
                        // Not ESC[ - pass to ANSI parser
                        self.state = State::Default;
                        self.ansi_parser.parse(b"\x1B", sink);
                        self.ansi_parser.parse(&[ch], sink);
                    }
                }
                State::GotEscBracket => {
                    if ch == b'!' {
                        // ESC[! - Query version (same as ESC[0!)
                        sink.request(crate::TerminalRequest::RipRequestTerminalId);
                        self.state = State::Default;
                    } else if ch.is_ascii_digit() {
                        // Start reading number
                        self.state = State::ReadAnsiNumber(vec![ch]);
                    } else {
                        // Unknown ESC[ sequence - pass to ANSI parser
                        self.state = State::Default;
                        self.ansi_parser.parse(b"\x1B[", sink);
                        self.ansi_parser.parse(&[ch], sink);
                    }
                }
                State::ReadAnsiNumber(digits) => {
                    if ch == b'!' {
                        // Complete ESC[<number>! sequence
                        let num_str = String::from_utf8_lossy(digits);
                        if let Ok(num) = num_str.parse::<i32>() {
                            match num {
                                0 => {
                                    // ESC[0! - Query version
                                    sink.request(crate::TerminalRequest::RipRequestTerminalId);
                                }
                                1 => {
                                    // ESC[1! - Disable RIPscrip (handled internally)
                                    self.enabled = false;
                                }
                                2 => {
                                    // ESC[2! - Enable RIPscrip (handled internally)
                                    self.enabled = true;
                                }
                                _ => {
                                    // Unknown number - pass to ANSI parser
                                    self.ansi_parser.parse(b"\x1B[", sink);
                                    self.ansi_parser.parse(digits, sink);
                                    self.ansi_parser.parse(b"!", sink);
                                }
                            }
                        } else {
                            // Failed to parse number - pass to ANSI parser
                            self.ansi_parser.parse(b"\x1B[", sink);
                            self.ansi_parser.parse(digits, sink);
                            self.ansi_parser.parse(b"!", sink);
                        }
                        self.state = State::Default;
                    } else if ch.is_ascii_digit() {
                        // Continue reading number
                        let mut new_digits = digits.clone();
                        new_digits.push(ch);
                        self.state = State::ReadAnsiNumber(new_digits);
                    } else {
                        // Not a digit or ! - unknown sequence, pass to ANSI parser
                        self.state = State::Default;
                        self.ansi_parser.parse(b"\x1B[", sink);
                        self.ansi_parser.parse(digits, sink);
                        self.ansi_parser.parse(&[ch], sink);
                    }
                }
                State::GotExclaim => {
                    if ch == b'!' {
                        // Double ! - stay in GotExclaim
                        continue;
                    } else if ch == b'|' {
                        self.state = State::GotPipe;
                    } else if ch == b'\n' || ch == b'\r' {
                        // End of line after ! - reset to NonRip mode
                        self.mode = ParserMode::NonRip;
                        self.state = State::Default;
                    //                        self.ansi_parser.parse(&[ch], sink);
                    } else {
                        // Not a RIP command - emit ! and continue in NonRip mode
                        self.mode = ParserMode::NonRip;
                        self.state = State::Default;
                        self.ansi_parser.parse(b"!", sink);
                        self.ansi_parser.parse(&[ch], sink);
                    }
                }
                State::GotPipe => {
                    // Read command character
                    if ch == b'1' {
                        self.builder.level = 1;
                        self.state = State::ReadLevel1;
                    } else if ch == b'9' {
                        self.builder.level = 9;
                        self.state = State::ReadLevel9;
                    } else if ch == b'#' {
                        // No more RIP
                        self.builder.cmd_char = b'#';
                        self.builder.level = 0;
                        self.emit_command(sink);
                        self.builder.reset();
                        // Unfortunately, real-world files have multiple |# commands, so stay in RIP mode
                        // and go back to GotExclaim state to allow |#|#|# sequences
                        self.state = State::GotExclaim;
                    } else {
                        // Level 0 command
                        self.builder.level = 0;
                        self.builder.cmd_char = ch;
                        self.state = State::ReadParams;
                    }
                }
                State::ReadLevel1 => {
                    self.builder.cmd_char = ch;
                    self.state = State::ReadParams;
                }
                State::ReadLevel9 => {
                    self.builder.cmd_char = ch;
                    self.state = State::ReadParams;
                }
                State::ReadParams => {
                    if !self.parse_params(ch, sink) {
                        // Parse error - already reset by parse_params
                    }
                }
            }
        }
    }
}
