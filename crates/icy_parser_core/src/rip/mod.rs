//! RIPscrip (Remote Imaging Protocol Script) parser
//!
//! RIPscrip is a graphics-based BBS protocol that extends ANSI art with vector graphics,
//! buttons, and mouse support. Commands start with !| and use base-36 encoded parameters.

use crate::{AnsiParser, CommandParser, CommandSink};
mod command;
pub use command::RipCommand;

/// Helper function to parse a base-36 character into a digit
#[inline]
fn parse_base36_digit(ch: u8) -> Option<i32> {
    match ch {
        b'0'..=b'9' => Some((ch - b'0') as i32),
        b'A'..=b'Z' => Some((ch - b'A' + 10) as i32),
        b'a'..=b'z' => Some((ch - b'a' + 10) as i32),
        _ => None,
    }
}

/// Convert a number to base-36 representation with a fixed length
pub fn to_base_36(len: usize, number: i32) -> String {
    let mut res = String::new();
    let mut number = number;
    for _ in 0..len {
        let num2 = (number % 36) as u8;
        let ch2 = if num2 < 10 { (num2 + b'0') as char } else { (num2 - 10 + b'A') as char };

        res = ch2.to_string() + res.as_str();
        number /= 36;
    }
    res
}

#[derive(Default, Clone, Debug, PartialEq)]
enum State {
    #[default]
    Default,
    GotExclaim,
    GotPipe,
    _ReadCommand,
    ReadLevel1,
    ReadLevel9,
    ReadParams,
    SkipToEOL(Box<State>),   // Store the state to return to after EOL
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

#[derive(Default)]
struct CommandBuilder {
    cmd_char: u8,
    level: u8,
    param_state: usize,
    npoints: i32,

    // Reusable buffers for command parameters
    i32_params: Vec<i32>,
    string_param: String,
    char_param: u8,
}

impl CommandBuilder {
    fn reset(&mut self) {
        self.cmd_char = 0;
        self.level = 0;
        self.param_state = 0;
        self.npoints = 0;
        self.i32_params.clear();
        self.string_param.clear();
        self.char_param = 0;
    }

    fn _parse_base36_2digit(&mut self, ch: u8, target_idx: usize) -> Result<bool, ()> {
        let digit = parse_base36_digit(ch).ok_or(())?;
        if self.param_state % 2 == 0 {
            if self.i32_params.len() <= target_idx {
                self.i32_params.resize(target_idx + 1, 0);
            }
            self.i32_params[target_idx] = digit;
        } else {
            self.i32_params[target_idx] = self.i32_params[target_idx].wrapping_mul(36).wrapping_add(digit);
        }
        self.param_state += 1;
        Ok(false) // Not done yet
    }

    fn parse_base36_complete(&mut self, ch: u8, target_idx: usize, final_state: usize) -> Result<bool, ()> {
        let digit = parse_base36_digit(ch).ok_or(())?;
        if self.param_state % 2 == 0 {
            if self.i32_params.len() <= target_idx {
                self.i32_params.resize(target_idx + 1, 0);
            }
            self.i32_params[target_idx] = digit;
        } else {
            self.i32_params[target_idx] = self.i32_params[target_idx].wrapping_mul(36).wrapping_add(digit);
        }
        self.param_state += 1;
        Ok(self.param_state > final_state)
    }
}

pub struct RipParser {
    mode: ParserMode,
    state: State,
    builder: CommandBuilder,
    ansi_parser: AnsiParser,
    enabled: bool, // RIPscrip processing enabled/disabled
}

impl RipParser {
    pub fn new() -> Self {
        Self {
            mode: ParserMode::default(),
            state: State::Default,
            builder: CommandBuilder::default(),
            ansi_parser: AnsiParser::new(),
            enabled: true, // RIPscrip starts enabled
        }
    }

    fn emit_command(&mut self, sink: &mut dyn CommandSink) {
        let cmd = match (self.builder.level, self.builder.cmd_char) {
            // Level 0 commands
            (0, b'w') if self.builder.i32_params.len() >= 5 => RipCommand::TextWindow {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
                wrap: self.builder.i32_params[4] != 0,
                size: *self.builder.i32_params.get(5).unwrap_or(&0),
            },
            (0, b'v') if self.builder.i32_params.len() >= 4 => RipCommand::ViewPort {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
            },
            (0, b'*') => RipCommand::ResetWindows,
            (0, b'e') => RipCommand::EraseWindow,
            (0, b'E') => RipCommand::EraseView,
            (0, b'g') if self.builder.i32_params.len() >= 2 => RipCommand::GotoXY {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
            },
            (0, b'H') => RipCommand::Home,
            (0, b'>') => RipCommand::EraseEOL,
            (0, b'c') if !self.builder.i32_params.is_empty() => RipCommand::Color { c: self.builder.i32_params[0] },
            (0, b'Q') => RipCommand::SetPalette {
                colors: self.builder.i32_params.clone(),
            },
            (0, b'a') if self.builder.i32_params.len() >= 2 => RipCommand::OnePalette {
                color: self.builder.i32_params[0],
                value: self.builder.i32_params[1],
            },
            (0, b'W') if !self.builder.i32_params.is_empty() => RipCommand::WriteMode {
                mode: self.builder.i32_params[0],
            },
            (0, b'm') if self.builder.i32_params.len() >= 2 => RipCommand::Move {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
            },
            (0, b'T') => RipCommand::Text {
                text: self.builder.string_param.clone(),
            },
            (0, b'@') if self.builder.i32_params.len() >= 2 => RipCommand::TextXY {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                text: self.builder.string_param.clone(),
            },
            (0, b'Y') if self.builder.i32_params.len() >= 4 => RipCommand::FontStyle {
                font: self.builder.i32_params[0],
                direction: self.builder.i32_params[1],
                size: self.builder.i32_params[2],
                res: self.builder.i32_params[3],
            },
            (0, b'X') if self.builder.i32_params.len() >= 2 => RipCommand::Pixel {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
            },
            (0, b'L') if self.builder.i32_params.len() >= 4 => RipCommand::Line {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
            },
            (0, b'R') if self.builder.i32_params.len() >= 4 => RipCommand::Rectangle {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
            },
            (0, b'B') if self.builder.i32_params.len() >= 4 => RipCommand::Bar {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
            },
            (0, b'C') if self.builder.i32_params.len() >= 3 => RipCommand::Circle {
                x_center: self.builder.i32_params[0],
                y_center: self.builder.i32_params[1],
                radius: self.builder.i32_params[2],
            },
            (0, b'O') if self.builder.i32_params.len() >= 6 => RipCommand::Oval {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                st_ang: self.builder.i32_params[2],
                end_ang: self.builder.i32_params[3],
                x_rad: self.builder.i32_params[4],
                y_rad: self.builder.i32_params[5],
            },
            (0, b'o') if self.builder.i32_params.len() >= 4 => RipCommand::FilledOval {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                x_rad: self.builder.i32_params[2],
                y_rad: self.builder.i32_params[3],
            },
            (0, b'A') if self.builder.i32_params.len() >= 5 => RipCommand::Arc {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                st_ang: self.builder.i32_params[2],
                end_ang: self.builder.i32_params[3],
                radius: self.builder.i32_params[4],
            },
            (0, b'V') if self.builder.i32_params.len() >= 6 => RipCommand::OvalArc {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                st_ang: self.builder.i32_params[2],
                end_ang: self.builder.i32_params[3],
                x_rad: self.builder.i32_params[4],
                y_rad: self.builder.i32_params[5],
            },
            (0, b'I') if self.builder.i32_params.len() >= 5 => RipCommand::PieSlice {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                st_ang: self.builder.i32_params[2],
                end_ang: self.builder.i32_params[3],
                radius: self.builder.i32_params[4],
            },
            (0, b'i') if self.builder.i32_params.len() >= 6 => RipCommand::OvalPieSlice {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                st_ang: self.builder.i32_params[2],
                end_ang: self.builder.i32_params[3],
                x_rad: self.builder.i32_params[4],
                y_rad: self.builder.i32_params[5],
            },
            (0, b'Z') if self.builder.i32_params.len() >= 9 => RipCommand::Bezier {
                x1: self.builder.i32_params[0],
                y1: self.builder.i32_params[1],
                x2: self.builder.i32_params[2],
                y2: self.builder.i32_params[3],
                x3: self.builder.i32_params[4],
                y3: self.builder.i32_params[5],
                x4: self.builder.i32_params[6],
                y4: self.builder.i32_params[7],
                cnt: self.builder.i32_params[8],
            },
            (0, b'P') => RipCommand::Polygon {
                points: self.builder.i32_params.clone(),
            },
            (0, b'p') => RipCommand::FilledPolygon {
                points: self.builder.i32_params.clone(),
            },
            (0, b'l') => RipCommand::PolyLine {
                points: self.builder.i32_params.clone(),
            },
            (0, b'F') if self.builder.i32_params.len() >= 3 => RipCommand::Fill {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                border: self.builder.i32_params[2],
            },
            (0, b'=') if self.builder.i32_params.len() >= 3 => RipCommand::LineStyle {
                style: self.builder.i32_params[0],
                user_pat: self.builder.i32_params[1],
                thick: self.builder.i32_params[2],
            },
            (0, b'S') if self.builder.i32_params.len() >= 2 => RipCommand::FillStyle {
                pattern: self.builder.i32_params[0],
                color: self.builder.i32_params[1],
            },
            (0, b's') if self.builder.i32_params.len() >= 9 => RipCommand::FillPattern {
                c1: self.builder.i32_params[0],
                c2: self.builder.i32_params[1],
                c3: self.builder.i32_params[2],
                c4: self.builder.i32_params[3],
                c5: self.builder.i32_params[4],
                c6: self.builder.i32_params[5],
                c7: self.builder.i32_params[6],
                c8: self.builder.i32_params[7],
                col: self.builder.i32_params[8],
            },
            (0, b'$') => RipCommand::TextVariable {
                text: self.builder.string_param.clone(),
            },
            (0, b'#') => RipCommand::NoMore,

            // Level 1 commands
            (1, b'M') if self.builder.i32_params.len() >= 8 => RipCommand::Mouse {
                num: self.builder.i32_params[0],
                x0: self.builder.i32_params[1],
                y0: self.builder.i32_params[2],
                x1: self.builder.i32_params[3],
                y1: self.builder.i32_params[4],
                clk: self.builder.i32_params[5],
                clr: self.builder.i32_params[6],
                res: self.builder.i32_params[7],
                text: self.builder.string_param.clone(),
            },
            (1, b'K') => RipCommand::MouseFields,
            (1, b'T') if self.builder.i32_params.len() >= 5 => RipCommand::BeginText {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
                res: self.builder.i32_params[4],
            },
            (1, b't') => RipCommand::RegionText {
                justify: !self.builder.i32_params.is_empty() && self.builder.i32_params[0] != 0,
                text: self.builder.string_param.clone(),
            },
            (1, b'E') => RipCommand::EndText,
            (1, b'C') if self.builder.i32_params.len() >= 5 => RipCommand::GetImage {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
                res: self.builder.i32_params[4],
            },
            (1, b'P') if self.builder.i32_params.len() >= 4 => RipCommand::PutImage {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                mode: self.builder.i32_params[2],
                res: self.builder.i32_params[3],
            },
            (1, b'W') => RipCommand::WriteIcon {
                res: self.builder.char_param,
                data: self.builder.string_param.clone(),
            },
            (1, b'I') if self.builder.i32_params.len() >= 5 => RipCommand::LoadIcon {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                mode: self.builder.i32_params[2],
                clipboard: self.builder.i32_params[3],
                res: self.builder.i32_params[4],
                file_name: self.builder.string_param.clone(),
            },
            (1, b'B') if self.builder.i32_params.len() >= 15 => RipCommand::ButtonStyle {
                wid: self.builder.i32_params[0],
                hgt: self.builder.i32_params[1],
                orient: self.builder.i32_params[2],
                flags: self.builder.i32_params[3],
                bevsize: self.builder.i32_params[4],
                dfore: self.builder.i32_params[5],
                dback: self.builder.i32_params[6],
                bright: self.builder.i32_params[7],
                dark: self.builder.i32_params[8],
                surface: self.builder.i32_params[9],
                grp_no: self.builder.i32_params[10],
                flags2: self.builder.i32_params[11],
                uline_col: self.builder.i32_params[12],
                corner_col: self.builder.i32_params[13],
                res: self.builder.i32_params[14],
            },
            (1, b'U') if self.builder.i32_params.len() >= 7 => RipCommand::Button {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
                hotkey: self.builder.i32_params[4],
                flags: self.builder.i32_params[5],
                res: self.builder.i32_params[6],
                text: self.builder.string_param.clone(),
            },
            (1, b'D') if self.builder.i32_params.len() >= 2 => RipCommand::Define {
                flags: self.builder.i32_params[0],
                res: self.builder.i32_params[1],
                text: self.builder.string_param.clone(),
            },
            (1, 0x1B) if self.builder.i32_params.len() >= 2 => RipCommand::Query {
                mode: self.builder.i32_params[0],
                res: self.builder.i32_params[1],
                text: self.builder.string_param.clone(),
            },
            (1, b'G') if self.builder.i32_params.len() >= 6 => RipCommand::CopyRegion {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
                res: self.builder.i32_params[4],
                dest_line: self.builder.i32_params[5],
            },
            (1, b'R') => RipCommand::ReadScene {
                file_name: self.builder.string_param.clone(),
            },
            (1, b'F') => RipCommand::FileQuery {
                file_name: self.builder.string_param.clone(),
            },

            // Level 9 commands
            (9, 0x1B) if self.builder.i32_params.len() >= 4 => RipCommand::EnterBlockMode {
                mode: self.builder.i32_params[0],
                proto: self.builder.i32_params[1],
                file_type: self.builder.i32_params[2],
                res: self.builder.i32_params[3],
                file_name: self.builder.string_param.clone(),
            },

            _ => {
                // Unknown command - don't emit anything
                return;
            }
        };

        sink.emit_rip(cmd);
    }

    fn parse_params(&mut self, ch: u8, sink: &mut dyn CommandSink) -> bool {
        // Handle command termination
        if ch == b'\r' {
            return true;
        }
        if ch == b'\n' {
            self.emit_command(sink);
            self.builder.reset();
            self.state = State::Default;
            // Stay in RIP mode after command completes
            return true;
        }
        if ch == b'|' {
            self.emit_command(sink);
            self.builder.reset();
            self.state = State::GotPipe;
            return true;
        }

        // Parse parameters based on command
        let result = match (self.builder.level, self.builder.cmd_char) {
            // Commands with no parameters
            (0, b'*') | (0, b'e') | (0, b'E') | (0, b'H') | (0, b'>') | (0, b'#') | (1, b'K') | (1, b'E') => {
                // Immediate commands - complete immediately
                self.emit_command(sink);
                self.builder.reset();
                self.state = State::GotExclaim;
                return true;
            }

            // Text commands (consume rest as string)
            (0, b'T') | (0, b'$') | (1, b'R') | (1, b'F') => {
                self.builder.string_param.push(ch as char);
                Ok(false)
            }

            // TextXY, Button - initial params then string
            (0, b'@') if self.builder.param_state < 4 => {
                let result = self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 3);
                // Don't signal completion even if params are done - we still need the string
                match result {
                    Ok(_) => Ok(false),
                    Err(e) => Err(e),
                }
            }
            (0, b'@') => {
                self.builder.string_param.push(ch as char);
                Ok(false)
            }

            // Button - 7 params (14 digits) then string
            (1, b'U') if self.builder.param_state < 14 => {
                let result = self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 13);
                // Don't signal completion even if params are done - we still need the string
                match result {
                    Ok(_) => Ok(false),
                    Err(e) => Err(e),
                }
            }
            (1, b'U') => {
                self.builder.string_param.push(ch as char);
                Ok(false)
            }

            // Mouse: states 0..11 (6 two-digit + 2 one-digit), 12..16 (res 5 digits), then text
            (1, b'M') => {
                if self.builder.param_state <= 16 {
                    if let Some(digit) = parse_base36_digit(ch) {
                        match self.builder.param_state {
                            0..=1 => {
                                // num
                                if self.builder.i32_params.is_empty() {
                                    self.builder.i32_params.resize(8, 0);
                                }
                                self.builder.i32_params[0] = self.builder.i32_params[0].wrapping_mul(36).wrapping_add(digit);
                            }
                            2..=3 => {
                                // x0
                                self.builder.i32_params[1] = self.builder.i32_params[1].wrapping_mul(36).wrapping_add(digit);
                            }
                            4..=5 => {
                                // y0
                                self.builder.i32_params[2] = self.builder.i32_params[2].wrapping_mul(36).wrapping_add(digit);
                            }
                            6..=7 => {
                                // x1
                                self.builder.i32_params[3] = self.builder.i32_params[3].wrapping_mul(36).wrapping_add(digit);
                            }
                            8..=9 => {
                                // y1
                                self.builder.i32_params[4] = self.builder.i32_params[4].wrapping_mul(36).wrapping_add(digit);
                            }
                            10 => {
                                // clk (1 digit)
                                self.builder.i32_params[5] = digit;
                            }
                            11 => {
                                // clr (1 digit)
                                self.builder.i32_params[6] = digit;
                            }
                            12..=16 => {
                                // res (5 digits)
                                self.builder.i32_params[7] = self.builder.i32_params[7].wrapping_mul(36).wrapping_add(digit);
                            }
                            _ => {}
                        }
                        self.builder.param_state += 1;
                        Ok(false)
                    } else {
                        Err(())
                    }
                } else {
                    // After 17 digits, rest is text
                    self.builder.string_param.push(ch as char);
                    Ok(false)
                }
            }

            // WriteIcon - char then string
            (1, b'W') if self.builder.param_state == 0 => {
                self.builder.char_param = ch;
                self.builder.param_state += 1;
                Ok(false)
            }
            (1, b'W') => {
                self.builder.string_param.push(ch as char);
                Ok(false)
            }

            // LoadIcon - 5 params (10 digits) then string
            (1, b'I') if self.builder.param_state < 10 => {
                let result = self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 9);
                // Don't signal completion even if params are done - we still need the string
                match result {
                    Ok(_) => Ok(false),
                    Err(e) => Err(e),
                }
            }
            (1, b'I') => {
                self.builder.string_param.push(ch as char);
                Ok(false)
            }

            // Simple 2-digit parameter commands
            (0, b'c') => self.builder.parse_base36_complete(ch, 0, 1),
            (0, b'W') => self.builder.parse_base36_complete(ch, 0, 1),

            // 4-digit parameter commands
            (0, b'g') | (0, b'm') | (0, b'X') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 3),

            // 6-digit parameter commands
            (0, b'a') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 3),
            (0, b'C') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 5),
            (0, b'F') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 5),

            // 8-digit parameter commands
            (0, b'v') | (0, b'L') | (0, b'R') | (0, b'B') | (0, b'o') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 7),

            // TextWindow: 4 two-digit params, then wrap (1 digit), then size (1 digit)
            (0, b'w') if self.builder.param_state < 8 => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 8),
            (0, b'w') if self.builder.param_state == 8 => {
                if let Some(digit) = parse_base36_digit(ch) {
                    self.builder.i32_params.push(digit);
                    self.builder.param_state += 1;
                    Ok(false)
                } else {
                    Err(())
                }
            }
            (0, b'w') => {
                // param_state == 9: final single digit parameter (size)
                if let Some(digit) = parse_base36_digit(ch) {
                    self.builder.i32_params.push(digit);
                    self.builder.param_state += 1;
                    Ok(true)
                } else {
                    Err(())
                }
            }

            // A - Arc (10 digits: 5 params)
            (0, b'A') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 9),
            (0, b'I') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 9),

            // O, V, i - Oval commands (12 digits: 6 params)
            (0, b'O') | (0, b'V') | (0, b'i') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 11),

            // Y - Font Style (8 digits)
            (0, b'Y') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 7),

            // Z - Bezier (18 digits: 9 params)
            (0, b'Z') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 17),

            // = - Line Style: states 0..1 (style), 2..5 (user_pat), 6..7 (thick)
            (0, b'=') => {
                if let Some(digit) = parse_base36_digit(ch) {
                    match self.builder.param_state {
                        0..=1 => {
                            // style
                            if self.builder.i32_params.is_empty() {
                                self.builder.i32_params.push(0);
                            }
                            self.builder.i32_params[0] = self.builder.i32_params[0].wrapping_mul(36).wrapping_add(digit);
                        }
                        2..=5 => {
                            // user_pat
                            if self.builder.i32_params.len() < 2 {
                                self.builder.i32_params.resize(2, 0);
                            }
                            self.builder.i32_params[1] = self.builder.i32_params[1].wrapping_mul(36).wrapping_add(digit);
                        }
                        6..=7 => {
                            // thick
                            if self.builder.i32_params.len() < 3 {
                                self.builder.i32_params.resize(3, 0);
                            }
                            self.builder.i32_params[2] = self.builder.i32_params[2].wrapping_mul(36).wrapping_add(digit);
                        }
                        _ => {}
                    }
                    self.builder.param_state += 1;
                    Ok(self.builder.param_state > 7)
                } else {
                    Err(())
                }
            }

            // S - Fill Style (4 digits)
            (0, b'S') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 3),

            // s - Fill Pattern (18 digits)
            (0, b's') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 17),

            // Q - Set Palette (32 digits for 16 colors)
            (0, b'Q') => {
                if let Some(digit) = parse_base36_digit(ch) {
                    if self.builder.param_state % 2 == 0 {
                        self.builder.i32_params.push(digit);
                    } else {
                        let idx = self.builder.i32_params.len() - 1;
                        self.builder.i32_params[idx] = self.builder.i32_params[idx].wrapping_mul(36).wrapping_add(digit);
                    }
                    self.builder.param_state += 1;
                    Ok(self.builder.param_state >= 32)
                } else {
                    Err(())
                }
            }

            // P, p, l - Polygon/PolyLine (variable length based on npoints)
            (0, b'P') | (0, b'p') | (0, b'l') if self.builder.param_state < 2 => {
                if let Some(digit) = parse_base36_digit(ch) {
                    if self.builder.param_state == 0 {
                        self.builder.npoints = digit;
                    } else {
                        self.builder.npoints = self.builder.npoints.wrapping_mul(36).wrapping_add(digit);
                    }
                    self.builder.param_state += 1;
                    Ok(false)
                } else {
                    Err(())
                }
            }
            (0, b'P') | (0, b'p') | (0, b'l') => {
                if let Some(digit) = parse_base36_digit(ch) {
                    if self.builder.param_state % 2 == 0 {
                        self.builder.i32_params.push(digit);
                    } else {
                        let idx = self.builder.i32_params.len() - 1;
                        self.builder.i32_params[idx] = self.builder.i32_params[idx].wrapping_mul(36).wrapping_add(digit);
                    }
                    self.builder.param_state += 1;
                    let expected = 2 + self.builder.npoints * 4;
                    Ok(self.builder.param_state >= expected as usize)
                } else {
                    Err(())
                }
            }

            // Level 1 commands
            // BeginText, GetImage, PutImage: 5 params (10 digits)
            (1, b'T') | (1, b'C') | (1, b'P') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 9),

            // RegionText: 1 digit (justify) then text
            (1, b't') if self.builder.param_state == 0 => {
                if let Some(digit) = parse_base36_digit(ch) {
                    self.builder.i32_params.push(digit);
                    self.builder.param_state += 1;
                    Ok(false)
                } else {
                    Err(())
                }
            }
            (1, b't') => {
                self.builder.string_param.push(ch as char);
                Ok(false)
            }

            // ButtonStyle: 37 states total (0..36)
            // states 0..=35: parse 2-digit pairs for first 3 params, then 4-digit flags, then 2-digit pairs for remaining params, then 7-digit res
            // state 36: done
            (1, b'B') => {
                if let Some(digit) = parse_base36_digit(ch) {
                    let state = self.builder.param_state;

                    // states 0-1: wid, 2-3: hgt, 4-5: orient (params 0,1,2)
                    if state <= 5 {
                        let idx = state / 2;
                        if self.builder.i32_params.len() <= idx {
                            self.builder.i32_params.resize(idx + 1, 0);
                        }
                        if state % 2 == 0 {
                            self.builder.i32_params[idx] = digit;
                        } else {
                            self.builder.i32_params[idx] = self.builder.i32_params[idx].wrapping_mul(36).wrapping_add(digit);
                        }
                    }
                    // states 6-9: flags (4 digits, param 3)
                    else if state <= 9 {
                        let idx = 3;
                        if self.builder.i32_params.len() <= idx {
                            self.builder.i32_params.resize(idx + 1, 0);
                        }
                        self.builder.i32_params[idx] = self.builder.i32_params[idx].wrapping_mul(36).wrapping_add(digit);
                    }
                    // states 10-29: bevsize, dfore, dback, bright, dark, surface, grp_no, flags2, uline_col, corner_col (params 4-13, all 2 digits)
                    else if state <= 29 {
                        let idx = 4 + (state - 10) / 2;
                        if self.builder.i32_params.len() <= idx {
                            self.builder.i32_params.resize(idx + 1, 0);
                        }
                        if (state - 10) % 2 == 0 {
                            self.builder.i32_params[idx] = digit;
                        } else {
                            self.builder.i32_params[idx] = self.builder.i32_params[idx].wrapping_mul(36).wrapping_add(digit);
                        }
                    }
                    // states 30-36: res (7 digits, param 14)
                    else if state <= 36 {
                        let idx = 14;
                        if self.builder.i32_params.len() <= idx {
                            self.builder.i32_params.resize(idx + 1, 0);
                        }
                        self.builder.i32_params[idx] = self.builder.i32_params[idx].wrapping_mul(36).wrapping_add(digit);
                    }

                    self.builder.param_state += 1;
                    Ok(self.builder.param_state > 36)
                } else {
                    Err(())
                }
            }

            (1, b'G') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 11),

            // Define: flags (3 digits) + res (2 digits) then text
            (1, b'D') => {
                // states 0..=2: flags (3 digits)
                if self.builder.param_state <= 2 {
                    if let Some(digit) = parse_base36_digit(ch) {
                        if self.builder.i32_params.len() == 0 {
                            self.builder.i32_params.push(0);
                        }
                        self.builder.i32_params[0] = self.builder.i32_params[0].wrapping_mul(36).wrapping_add(digit);
                        self.builder.param_state += 1;
                        Ok(false)
                    } else {
                        Err(())
                    }
                }
                // states 3, 4: res (2 digits)
                else if self.builder.param_state <= 4 {
                    if let Some(digit) = parse_base36_digit(ch) {
                        if self.builder.i32_params.len() < 2 {
                            self.builder.i32_params.resize(2, 0);
                        }
                        self.builder.i32_params[1] = self.builder.i32_params[1].wrapping_mul(36).wrapping_add(digit);
                        self.builder.param_state += 1;
                        Ok(false)
                    } else {
                        Err(())
                    }
                }
                // state >= 5: everything is text
                else {
                    self.builder.string_param.push(ch as char);
                    Ok(false)
                }
            }

            // Query: state 0 (mode), states 1..3 (res), then text
            (1, 0x1B) => {
                if self.builder.param_state <= 3 {
                    if let Some(digit) = parse_base36_digit(ch) {
                        if self.builder.param_state == 0 {
                            // mode: 1 digit
                            if self.builder.i32_params.is_empty() {
                                self.builder.i32_params.resize(2, 0);
                            }
                            self.builder.i32_params[0] = digit;
                        } else {
                            // res: 3 digits (states 1..3)
                            self.builder.i32_params[1] = self.builder.i32_params[1].wrapping_mul(36).wrapping_add(digit);
                        }
                        self.builder.param_state += 1;
                        Ok(false)
                    } else {
                        // first non-digit belongs to text
                        self.builder.string_param.push(ch as char);
                        self.builder.param_state = 4;
                        Ok(false)
                    }
                } else {
                    self.builder.string_param.push(ch as char);
                    Ok(false)
                }
            }

            // Level 9: EnterBlockMode: mode(1), proto(1), file_type(2), res(4) then text
            (9, 0x1B) if self.builder.param_state < 8 => {
                if let Some(digit) = parse_base36_digit(ch) {
                    let idx = match self.builder.param_state {
                        0..=1 => self.builder.param_state as usize, // mode, proto
                        2..=3 => 2,                                 // file_type
                        _ => 3,                                     // res
                    };
                    if self.builder.i32_params.len() <= idx {
                        self.builder.i32_params.resize(idx + 1, 0);
                    }
                    self.builder.i32_params[idx] = self.builder.i32_params[idx].wrapping_mul(36).wrapping_add(digit);
                    self.builder.param_state += 1;
                    Ok(false)
                } else {
                    // non-digit starts filename
                    self.builder.string_param.push(ch as char);
                    self.builder.param_state = 8;
                    Ok(false)
                }
            }
            (9, 0x1B) => {
                self.builder.string_param.push(ch as char);
                Ok(false)
            }

            _ => Err(()),
        };

        match result {
            Ok(true) => {
                // Command complete
                self.emit_command(sink);
                self.builder.reset();
                self.state = State::GotExclaim;
                true
            }
            Ok(false) => {
                // Continue parsing
                true
            }
            Err(()) => {
                // Parse error - abort command and return to NonRip mode
                self.builder.reset();
                self.mode = ParserMode::NonRip;
                self.state = State::Default;
                false
            }
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
            if self.mode == ParserMode::Rip
                && ch == b'\\'
                && !matches!(
                    self.state,
                    State::SkipToEOL(_) | State::Default | State::GotEscape | State::GotEscBracket | State::ReadAnsiNumber(_)
                )
            {
                self.state = State::SkipToEOL(Box::new(self.state.clone()));
                continue;
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
                        self.ansi_parser.parse(&[ch], sink);
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
                        self.mode = ParserMode::NonRip;
                        self.state = State::Default;
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
                State::SkipToEOL(return_state) => {
                    if ch == b'\n' {
                        // Return to the saved state
                        self.state = (**return_state).clone();
                    }
                    // Ignore everything else until newline
                }
                State::_ReadCommand => {
                    // Shouldn't reach here
                    self.mode = ParserMode::NonRip;
                    self.state = State::Default;
                }
            }
        }
    }
}
