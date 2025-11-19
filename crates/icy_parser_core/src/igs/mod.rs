//! IGS (Interactive Graphics System) parser
//!
//! IGS is a graphics system developed for Atari ST BBS systems.
//! Commands start with 'G#' and use single-letter command codes followed by parameters.

use crate::{Color, CommandParser, CommandSink, DecPrivateMode, Direction, EraseInDisplayMode, EraseInLineMode, SgrAttribute, TerminalCommand};

mod types;
pub use types::*;

mod command_type;
pub use command_type::IgsCommandType;

mod command;
pub use command::*;

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Default,
    GotG,
    GotIgsStart,
    ReadParams(IgsCommandType),
    ReadTextString(i32, i32, u8), // x, y, justification
    ReadLoopTokens,               // specialized loop command token parsing
    ReadZoneString(Vec<i32>),     // extended command X 4 zone string reading after numeric params
    ReadFillPattern(i32),         // extended command X 7 pattern data reading after id,pattern

    // VT52 states
    Escape,
    ReadFgColor,
    ReadBgColor,
    ReadCursorLine,
    ReadCursorRow(i32), // row position
    ReadInsertLineCount,
}

pub struct IgsParser {
    state: State,
    params: Vec<i32>,
    current_param: i32,
    text_buffer: String,

    loop_command: String,
    loop_parameters: Vec<Vec<String>>,
    loop_tokens: Vec<String>,
    loop_token_buffer: String,
    reading_chain_gang: bool, // True when reading >XXX@ chain-gang identifier

    reverse_video: bool,
    skip_next_lf: bool, // used for skipping LF in igs line G>....\n otherwise screen would scroll.
}

impl IgsParser {
    pub fn new() -> Self {
        Self {
            state: State::Default,
            params: Vec::new(),
            current_param: 0,
            text_buffer: String::new(),
            loop_command: String::new(),
            loop_parameters: Vec::new(),
            loop_tokens: Vec::new(),
            loop_token_buffer: String::new(),
            reading_chain_gang: false,
            skip_next_lf: false,
            reverse_video: false,
        }
    }

    fn reset_params(&mut self) {
        self.params.clear();
        self.current_param = 0;
        self.text_buffer.clear();
    }

    /// Parse VT52 hex color code from ASCII byte
    #[inline]
    fn parse_vt52_color(byte: u8) -> Option<Color> {
        if byte <= 0x0F {
            // ATARI ST extension
            Some(Color::Base(byte as u8))
        } else if byte >= b'0' && byte <= b'0' + 15 {
            // Support for backwards compatibilility with VT52
            let index = byte.wrapping_sub(b'0');
            Some(Color::Base(index as u8))
        } else {
            None
        }
    }

    /// Parse VT52 cursor row position from ASCII byte (1 based)
    #[inline]
    fn parse_cursor_row(byte: u8) -> Option<i32> {
        /* ATARI:
        if byte > 0 && byte <= 132 { // 132 is the max rows for ATARI ST VT52
            Some(byte as i32)
        } else {
            None
        }*/
        // Original VT-52 would be :
        if byte >= b' ' && byte <= b'p' { Some((byte - b' ') as i32 + 1) } else { None }
    }

    /// Parse ATARI cursor line position from ASCII byte (1 based)
    #[inline]
    fn parse_cursor_line(byte: u8) -> Option<i32> {
        // ATARI:
        /*if byte > 0 && byte <= 25 {
            Some(byte as i32)
        } else */

        // Original VT-52 would be :
        if byte >= b' ' && byte <= b'8' { Some((byte - b' ') as i32 + 1) } else { None }
    }

    fn push_current_param(&mut self) {
        self.params.push(self.current_param);
        self.current_param = 0;
    }

    fn create_loop_command(&mut self, sink: &mut dyn CommandSink) -> Option<IgsCommand> {
        // & from,to,step,delay,cmd,param_count,(params...)
        if self.params.len() < 6 {
            sink.report_errror(
                crate::ParseError::InvalidParameter {
                    command: "LoopCommand",
                    value: self.params.len() as u16,
                    expected: Some("6 parameter"),
                },
                crate::ErrorLevel::Error,
            );
            None
        } else {
            let from = self.params[0];
            let to = self.params[1];
            let step = self.params[2];
            let delay = self.params[3];
            let command_identifier = self.loop_command.clone();
            let param_count = self.params[4] as u16;

            let mut params_tokens: Vec<LoopParamToken> = Vec::new();
            // Remaining numeric params are converted to tokens unless already substituted tokens in loop_parameters
            if self.params.len() > 5 {
                for p in &self.params[5..] {
                    params_tokens.push(LoopParamToken::Number(*p));
                }
            }
            // Add any textual parameter tokens captured (x,y,+n etc.)
            for token_group in &self.loop_parameters {
                for token in token_group {
                    match token.as_str() {
                        ":" => params_tokens.push(LoopParamToken::GroupSeparator),
                        "x" | "y" => params_tokens.push(LoopParamToken::Symbol(token.chars().next().unwrap())),
                        _ => {
                            // Check if token starts with a prefix operator (+, -, !)
                            let has_prefix = token.starts_with('+') || token.starts_with('-') || token.starts_with('!');
                            if !has_prefix && token.parse::<i32>().is_ok() {
                                params_tokens.push(LoopParamToken::Number(token.parse::<i32>().unwrap()));
                            } else {
                                params_tokens.push(LoopParamToken::Expr(token.clone()));
                            }
                        }
                    }
                }
            }

            let mut modifiers = LoopModifiers::default();
            let original_ident = command_identifier.as_str();
            let mut base_ident = original_ident;
            if let Some(pos) = base_ident.find(|c| c == '|' || c == '@') {
                let (ident_part, mod_part) = base_ident.split_at(pos);
                base_ident = ident_part;
                for ch in mod_part.chars() {
                    match ch {
                        '|' => modifiers.xor_stepping = true,
                        '@' => modifiers.refresh_text_each_iteration = true,
                        _ => {}
                    }
                }
            }

            let target = if base_ident.starts_with('>') && original_ident.ends_with('@') {
                let inner: String = base_ident.chars().skip(1).collect();
                let commands: Vec<char> = inner.chars().collect();
                LoopTarget::ChainGang {
                    raw: original_ident.to_string(),
                    commands,
                }
            } else {
                let ch = base_ident.chars().next().unwrap_or(' ');
                LoopTarget::Single(ch)
            };

            Some(IgsCommand::Loop(LoopCommandData {
                from,
                to,
                step,
                delay,
                target,
                modifiers,
                param_count,
                params: params_tokens,
            }))
        }
    }

    fn emit_command(&mut self, cmd_type: IgsCommandType, sink: &mut dyn CommandSink) {
        let command = if cmd_type == IgsCommandType::LoopCommand {
            // Special handling for Loop command to use collected tokens
            self.create_loop_command(sink)
        } else {
            cmd_type.create_command(sink, &self.params, &self.text_buffer)
        };

        if let Some(cmd) = command {
            sink.emit_igs(cmd);
        }

        self.reset_params();
    }
}

impl Default for IgsParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandParser for IgsParser {
    fn parse(&mut self, data: &[u8], sink: &mut dyn CommandSink) {
        for &byte in data {
            let ch = byte as char;

            match self.state {
                State::Default => {
                    match byte {
                        b'G' => {
                            self.state = State::GotG;
                        }
                        0x1B => {
                            // ESC - VT52 escape sequence
                            self.state = State::Escape;
                        }
                        0x08 | 0x0B | 0x0C => {
                            // Backspace
                            sink.emit(TerminalCommand::Backspace);
                        }
                        0x0D => {
                            // Carriage return / Line feed
                            sink.emit(TerminalCommand::CarriageReturn);
                        }
                        0x0A => {
                            if self.skip_next_lf {
                                self.skip_next_lf = false;
                                continue;
                            }
                            sink.emit(TerminalCommand::LineFeed);
                        } /*
                        0x07 => {
                        sink.emit(TerminalCommand::Bell);
                        }*/
                        0x00..=0x0F => {
                            // TOS direct foreground color codes (0x00-0x0F)
                            // 0x07 (Bell) is excluded to maintain standard ASCII compatibility
                            let color = Color::Base(byte % 16);
                            let sgr: SgrAttribute = if self.reverse_video {
                                SgrAttribute::Background(color)
                            } else {
                                SgrAttribute::Foreground(color)
                            };
                            sink.emit(TerminalCommand::CsiSelectGraphicRendition(sgr));
                        } /*
                        0x09 => {
                        sink.emit(TerminalCommand::Tab);
                        }*/
                        0x0E..=0x1A | 0x1C..=0x1F => {
                            // Ignore control characters
                        }
                        _ => {
                            // Regular character
                            sink.print(&[byte]);
                        }
                    }
                }
                State::GotG => {
                    self.skip_next_lf = true;
                    if ch == '#' {
                        self.state = State::GotIgsStart;
                        self.reset_params();
                    } else {
                        // False alarm, just 'G' followed by something else
                        sink.print(b"G");
                        if byte >= 0x20 {
                            sink.print(&[byte]);
                        }
                        self.state = State::Default;
                    }
                }
                State::GotIgsStart => {
                    if ch == '&' {
                        // Loop command
                        // Use specialized token parser for loop command because parameters include substitution tokens.
                        self.state = State::ReadLoopTokens;
                        self.loop_tokens.clear();
                        self.loop_token_buffer.clear();
                    } else if let Some(cmd_type) = IgsCommandType::from_char(ch) {
                        self.state = State::ReadParams(cmd_type);
                    } else {
                        // Unknown command
                        if !ch.is_control() {
                            sink.report_errror(
                                crate::ParseError::InvalidParameter {
                                    command: "IGS",
                                    value: byte as u16,
                                    expected: Some("valid IGS command character"),
                                },
                                crate::ErrorLevel::Error,
                            );
                        }
                        self.state = State::Default;
                    }
                }
                State::ReadParams(cmd_type) => {
                    match ch {
                        '0'..='9' => {
                            self.current_param = self.current_param.wrapping_mul(10).wrapping_add((byte - b'0') as i32);
                        }
                        ',' => {
                            self.push_current_param();
                            // For WriteText: after 2 params (x, y), next non-separator char starts text
                            if cmd_type == IgsCommandType::WriteText && self.params.len() == 2 {
                                // W>x,y,text@ - text follows immediately after second comma
                                self.state = State::ReadTextString(self.params[0], self.params[1], 0);
                                self.text_buffer.clear();
                            }
                        }
                        '@' if cmd_type == IgsCommandType::WriteText => {
                            // For WriteText: @ starts text after x,y params
                            self.push_current_param();
                            if self.params.len() == 2 {
                                // W>x,y@text@ format
                                self.state = State::ReadTextString(self.params[0], self.params[1], 0);
                                self.text_buffer.clear();
                            } else {
                                // Invalid - WriteText needs exactly 2 params before @
                                self.reset_params();
                                self.state = State::Default;
                            }
                        }
                        ':' => {
                            // Command terminator
                            self.push_current_param();
                            self.emit_command(cmd_type, sink);
                            self.state = State::GotIgsStart;
                        }
                        ' ' | '>' | '\r' | '\n' | '_' => {
                            // Whitespace/formatting - ignore
                            // Special handling: extended command X 4 (DefineZone) starts string after 7 numeric params
                            if let State::ReadParams(IgsCommandType::ExtendedCommand) = self.state {
                                if !self.params.is_empty() && self.params[0] == 4 && self.params.len() == 7 {
                                    // Switch into zone string reading state (length already captured)
                                    self.state = State::ReadZoneString(self.params.clone());
                                    self.text_buffer.clear();
                                } else if !self.params.is_empty() && self.params[0] == 7 && self.params.len() == 2 {
                                    // Switch to fill pattern reading state
                                    let pattern = self.params[1];
                                    self.state = State::ReadFillPattern(pattern);
                                    self.text_buffer.clear();
                                }
                            }
                        }
                        _ => {
                            // Extended command X 4 zone string may contain arbitrary characters until ':'
                            if let State::ReadParams(IgsCommandType::ExtendedCommand) = self.state {
                                if !self.params.is_empty() && self.params[0] == 4 && self.params.len() == 7 {
                                    self.state = State::ReadZoneString(self.params.clone());
                                    self.text_buffer.clear();
                                    self.text_buffer.push(ch);
                                } else if !self.params.is_empty() && self.params[0] == 7 && self.params.len() == 2 {
                                    let pattern = self.params[1];
                                    self.state = State::ReadFillPattern(pattern);
                                    self.text_buffer.clear();
                                    self.text_buffer.push(ch);
                                } else {
                                    // Invalid for other extended commands
                                    sink.report_errror(
                                        crate::ParseError::InvalidParameter {
                                            command: "ExtendedCommand",
                                            value: ch as u16,
                                            expected: Some("digit, ',', ':' oder gültiger Text für X4/X7"),
                                        },
                                        crate::ErrorLevel::Error,
                                    );
                                    self.reset_params();
                                    self.state = State::Default;
                                }
                            } else {
                                // Invalid character in numeric parameter phase for non-extended commands
                                sink.report_errror(
                                    crate::ParseError::InvalidParameter {
                                        command: "IGS:ReadParams",
                                        value: ch as u16,
                                        expected: Some("Ziffer, ',', ':' oder Whitespace"),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                self.reset_params();
                                self.state = State::Default;
                            }
                        }
                    }
                }
                State::ReadZoneString(ref zone_params) => {
                    match ch {
                        ':' | '\n' => {
                            // Terminator: build DefineZone command (X 4)
                            if zone_params.len() == 7 {
                                let zone_id = zone_params[1];
                                let x1 = zone_params[2];
                                let y1 = zone_params[3];
                                let x2 = zone_params[4];
                                let y2 = zone_params[5];
                                let length = zone_params[6] as u16;
                                let string = self.text_buffer.clone();
                                sink.emit_igs(IgsCommand::DefineZone {
                                    zone_id,
                                    x1,
                                    y1,
                                    x2,
                                    y2,
                                    length,
                                    string,
                                });
                            }
                            self.reset_params();
                            self.state = if ch == '\n' { State::Default } else { State::GotIgsStart };
                        }
                        _ => {
                            self.text_buffer.push(ch);
                        }
                    }
                }
                State::ReadLoopTokens => {
                    match ch {
                        ':' => {
                            if !self.loop_token_buffer.is_empty() {
                                self.loop_tokens.push(self.loop_token_buffer.clone());
                                self.loop_token_buffer.clear();
                            }

                            // Check if we have enough tokens and if we've collected all expected parameters
                            if self.loop_tokens.len() >= 6 {
                                let parse_i32 = |s: &str| s.parse::<i32>().unwrap_or(0);
                                let param_count = parse_i32(&self.loop_tokens[5]) as usize;
                                // Count actual parameters (excluding ':' markers)
                                let current_param_count = self.loop_tokens[6..].iter().filter(|s| *s != ":").count();

                                // If we have collected all parameters, emit the command
                                if current_param_count >= param_count {
                                    let from = parse_i32(&self.loop_tokens[0]);
                                    let to = parse_i32(&self.loop_tokens[1]);
                                    let step = parse_i32(&self.loop_tokens[2]);
                                    let delay = parse_i32(&self.loop_tokens[3]);
                                    let raw_identifier = self.loop_tokens[4].clone();

                                    // Parse target and modifiers from command identifier
                                    // For chain-gangs (>XXX@), the @ is part of the identifier, not a modifier
                                    // Modifiers come AFTER the chain-gang's closing @
                                    let mut modifiers = LoopModifiers::default();
                                    let original_ident = raw_identifier.as_str();
                                    let mut base_ident = original_ident;
                                    let mut target = LoopTarget::Single(' ');

                                    // Check if this is a chain-gang command (>...@)
                                    let is_chain_gang = base_ident.starts_with('>') && base_ident.contains('@');

                                    if is_chain_gang {
                                        // For chain-gangs, find the closing @ of the chain
                                        if let Some(chain_end_pos) = base_ident.find('@') {
                                            let after_chain = &base_ident[chain_end_pos + 1..];
                                            // Parse modifiers that come after the chain-gang's @
                                            for ch in after_chain.chars() {
                                                match ch {
                                                    '|' => modifiers.xor_stepping = true,
                                                    '@' => modifiers.refresh_text_each_iteration = true,
                                                    _ => {}
                                                }
                                            }
                                            // base_ident includes the chain-gang with its closing @
                                            base_ident = &base_ident[..=chain_end_pos];
                                            // Create ChainGang target with the base_ident (which includes @)
                                            let inner: String = base_ident.chars().skip(1).take(base_ident.len().saturating_sub(2)).collect();
                                            let commands: Vec<char> = inner.chars().collect();
                                            target = LoopTarget::ChainGang {
                                                raw: base_ident.to_string(),
                                                commands,
                                            };
                                        }
                                    } else {
                                        // For single commands, parse modifiers normally
                                        if let Some(pos) = base_ident.find(|c| c == '|' || c == '@') {
                                            let (ident_part, mod_part) = base_ident.split_at(pos);
                                            base_ident = ident_part;
                                            for ch in mod_part.chars() {
                                                match ch {
                                                    '|' => modifiers.xor_stepping = true,
                                                    '@' => modifiers.refresh_text_each_iteration = true,
                                                    _ => {}
                                                }
                                            }
                                        }
                                    }

                                    if matches!(target, LoopTarget::Single(' ')) {
                                        target = if base_ident.starts_with('>') && original_ident.ends_with('@') {
                                            let inner: String = base_ident.chars().skip(1).collect();
                                            let commands: Vec<char> = inner.chars().collect();
                                            LoopTarget::ChainGang {
                                                raw: original_ident.to_string(),
                                                commands,
                                            }
                                        } else {
                                            let ch = base_ident.chars().next().unwrap_or(' ');
                                            LoopTarget::Single(ch)
                                        };
                                    }

                                    // Convert parameters into typed tokens, preserving ':' position
                                    let mut params: Vec<LoopParamToken> = Vec::new();
                                    for token in &self.loop_tokens[6..] {
                                        if token == ":" {
                                            params.push(LoopParamToken::GroupSeparator);
                                        } else if token == "x" || token == "y" {
                                            params.push(LoopParamToken::Symbol(token.chars().next().unwrap()));
                                        } else {
                                            // Check if token starts with a prefix operator (+, -, !)
                                            let has_prefix = token.starts_with('+') || token.starts_with('-') || token.starts_with('!');
                                            if !has_prefix && token.parse::<i32>().is_ok() {
                                                params.push(LoopParamToken::Number(token.parse::<i32>().unwrap()));
                                            } else {
                                                params.push(LoopParamToken::Expr(token.clone()));
                                            }
                                        }
                                    }

                                    let data = LoopCommandData {
                                        from,
                                        to,
                                        step,
                                        delay,
                                        target,
                                        modifiers,
                                        param_count: param_count as u16,
                                        params,
                                    };

                                    sink.emit_igs(IgsCommand::Loop(data));
                                    self.loop_tokens.clear();
                                    self.loop_token_buffer.clear();
                                    self.state = State::GotIgsStart;
                                } else {
                                    // Add ':' as a marker and continue reading
                                    self.loop_tokens.push(":".to_string());
                                }
                            }
                        }
                        '\n' => {
                            if !self.loop_token_buffer.is_empty() {
                                self.loop_tokens.push(self.loop_token_buffer.clone());
                                self.loop_token_buffer.clear();
                            }
                            // Process tokens even if incomplete on newline
                            if self.loop_tokens.len() >= 6 {
                                use crate::igs::{LoopCommandData, LoopModifiers, LoopParamToken, LoopTarget};

                                let parse_i32 = |s: &str| s.parse::<i32>().unwrap_or(0);
                                let from = parse_i32(&self.loop_tokens[0]);
                                let to = parse_i32(&self.loop_tokens[1]);
                                let step = parse_i32(&self.loop_tokens[2]);
                                let delay = parse_i32(&self.loop_tokens[3]);
                                let raw_identifier = self.loop_tokens[4].clone();
                                let param_count = parse_i32(&self.loop_tokens[5]) as usize;

                                let mut modifiers = LoopModifiers::default();
                                let original_ident = raw_identifier.as_str();
                                let mut base_ident = original_ident;
                                if let Some(pos) = base_ident.find(|c| c == '|' || c == '@') {
                                    let (ident_part, mod_part) = base_ident.split_at(pos);
                                    base_ident = ident_part;
                                    for ch in mod_part.chars() {
                                        match ch {
                                            '|' => modifiers.xor_stepping = true,
                                            '@' => modifiers.refresh_text_each_iteration = true,
                                            _ => {}
                                        }
                                    }
                                }

                                let target = if base_ident.starts_with('>') && original_ident.ends_with('@') {
                                    let inner: String = base_ident.chars().skip(1).collect();
                                    let commands: Vec<char> = inner.chars().collect();
                                    LoopTarget::ChainGang {
                                        raw: original_ident.to_string(),
                                        commands,
                                    }
                                } else {
                                    let ch = base_ident.chars().next().unwrap_or(' ');
                                    LoopTarget::Single(ch)
                                };

                                let mut params: Vec<LoopParamToken> = Vec::new();
                                for token in &self.loop_tokens[6..] {
                                    if token == ":" {
                                        params.push(LoopParamToken::GroupSeparator);
                                    } else if token == "x" || token == "y" {
                                        params.push(LoopParamToken::Symbol(token.chars().next().unwrap()));
                                    } else {
                                        // Check if token starts with a prefix operator (+, -, !)
                                        let has_prefix = token.starts_with('+') || token.starts_with('-') || token.starts_with('!');
                                        if !has_prefix && token.parse::<i32>().is_ok() {
                                            params.push(LoopParamToken::Number(token.parse::<i32>().unwrap()));
                                        } else {
                                            params.push(LoopParamToken::Expr(token.clone()));
                                        }
                                    }
                                }

                                let data = LoopCommandData {
                                    from,
                                    to,
                                    step,
                                    delay,
                                    target,
                                    modifiers,
                                    param_count: param_count as u16,
                                    params,
                                };

                                sink.emit_igs(IgsCommand::Loop(data));
                            }
                            self.loop_tokens.clear();
                            self.loop_token_buffer.clear();
                            self.reading_chain_gang = false;
                            self.state = State::Default;
                        }
                        ',' => {
                            // Comma acts as parameter separator
                            if self.reading_chain_gang {
                                self.loop_token_buffer.push(ch);
                            } else if !self.loop_token_buffer.is_empty() {
                                self.loop_tokens.push(self.loop_token_buffer.clone());
                                self.loop_token_buffer.clear();
                            }
                        }
                        ')' => {
                            // Closing paren marks command index in chain-gang parameters
                            // Keep it as part of the token for display purposes
                            if !self.loop_token_buffer.is_empty() {
                                self.loop_token_buffer.push(ch);
                                self.loop_tokens.push(self.loop_token_buffer.clone());
                                self.loop_token_buffer.clear();
                            }
                        }
                        '@' => {
                            // @ can end a chain-gang identifier or be a modifier
                            self.loop_token_buffer.push(ch);
                            if self.reading_chain_gang {
                                // This @ ends the chain-gang identifier
                                // Push token and clear flag
                                self.loop_tokens.push(self.loop_token_buffer.clone());
                                self.loop_token_buffer.clear();
                                self.reading_chain_gang = false;
                            }
                        }
                        ' ' | '\r' | '_' => {
                            // ignore these formatting chars entirely for loop tokens
                        }
                        '>' => {
                            // '>' can be part of chain-gang identifier (e.g., >CL@) or a formatting char
                            // If buffer is empty and we're at the command identifier position, it starts a chain-gang
                            if self.loop_token_buffer.is_empty() && self.loop_tokens.len() == 4 {
                                // We're at the command identifier position (5th token, index 4)
                                self.loop_token_buffer.push(ch);
                                self.reading_chain_gang = true;
                            }
                            // Otherwise ignore as formatting
                        }
                        _ => {
                            self.loop_token_buffer.push(ch);
                        }
                    }
                }
                State::ReadFillPattern(pattern) => match ch {
                    ':' | '\n' => {
                        sink.emit_igs(IgsCommand::LoadFillPattern {
                            pattern: pattern as u8,
                            data: self.text_buffer.clone(),
                        });
                        self.reset_params();
                        self.state = if ch == '\n' { State::Default } else { State::GotIgsStart };
                    }
                    _ => self.text_buffer.push(ch),
                },
                State::ReadTextString(_x, _y, _just) => {
                    if ch == '@' || ch == '\n' {
                        // End of text string
                        self.emit_command(IgsCommandType::WriteText, sink);
                        self.state = if ch == '\n' { State::Default } else { State::GotIgsStart };
                    } else {
                        self.text_buffer.push(ch);
                    }
                }

                // VT52 escape sequences
                State::Escape => {
                    match ch {
                        'A' => {
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Up, 1));
                            self.state = State::Default;
                        }
                        'B' => {
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Down, 1));
                            self.state = State::Default;
                        }
                        'C' => {
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Right, 1));
                            self.state = State::Default;
                        }
                        'D' => {
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Left, 1));
                            self.state = State::Default;
                        }
                        'E' => {
                            sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::All));
                            sink.emit(TerminalCommand::CsiCursorPosition(1, 1));
                            self.state = State::Default;
                        }
                        'H' => {
                            sink.emit(TerminalCommand::CsiCursorPosition(1, 1));
                            self.state = State::Default;
                        }
                        'I' => {
                            // VT52 Reverse line feed (cursor up and insert)
                            sink.emit(TerminalCommand::EscReverseIndex);
                            self.state = State::Default;
                        }
                        'J' => {
                            sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::CursorToEnd));
                            self.state = State::Default;
                        }
                        'K' => {
                            sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::CursorToEnd));
                            self.state = State::Default;
                        }
                        'Y' => {
                            self.state = State::ReadCursorLine;
                        }
                        '3' | 'b' => {
                            self.state = State::ReadFgColor;
                        }
                        '4' | 'c' => {
                            self.state = State::ReadBgColor;
                        }
                        'e' => {
                            sink.emit(TerminalCommand::CsiDecPrivateModeSet(DecPrivateMode::CursorVisible));
                            self.state = State::Default;
                        }
                        'f' => {
                            sink.emit(TerminalCommand::CsiDecPrivateModeReset(DecPrivateMode::CursorVisible));
                            self.state = State::Default;
                        }
                        'j' => {
                            sink.emit(TerminalCommand::CsiSaveCursorPosition);
                            self.state = State::Default;
                        }
                        'k' => {
                            sink.emit(TerminalCommand::CsiRestoreCursorPosition);
                            self.state = State::Default;
                        }
                        'L' => {
                            // VT52 Insert Line
                            sink.emit(TerminalCommand::CsiInsertLine(1));
                            self.state = State::Default;
                        }
                        'M' => {
                            // VT52 Delete Line
                            sink.emit(TerminalCommand::CsiDeleteLine(1));
                            self.state = State::Default;
                        }
                        'p' => {
                            // VT52 Reverse video
                            self.reverse_video = true;
                            self.state = State::Default;
                        }
                        'q' => {
                            // VT52 Normal video
                            self.reverse_video = false;
                            self.state = State::Default;
                        }
                        'v' => {
                            // VT52 Wrap on
                            sink.emit(TerminalCommand::CsiDecPrivateModeSet(DecPrivateMode::AutoWrap));
                            self.state = State::Default;
                        }
                        'w' => {
                            // VT52 Wrap off
                            sink.emit(TerminalCommand::CsiDecPrivateModeReset(DecPrivateMode::AutoWrap));
                            self.state = State::Default;
                        }
                        'd' => {
                            // VT52 Clear to start of screen
                            sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::StartToCursor));
                            self.state = State::Default;
                        }
                        'o' => {
                            // VT52 Clear to start of line
                            sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::StartToCursor));
                            self.state = State::Default;
                        }
                        'i' => {
                            // Insert line ESC form: mode implicitly 0, next byte is count
                            self.state = State::ReadInsertLineCount;
                        }
                        'l' => {
                            // Clear line ESC form: mode implicitly 0
                            sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::All));
                            self.state = State::Default;
                        }
                        'r' => {
                            // Remember cursor ESC form: value implicitly 0
                            sink.emit_igs(IgsCommand::RememberCursor { value: 0 });
                            self.state = State::Default;
                        }
                        'm' => {
                            // IGS command that can be invoked with ESC prefix instead of G#
                            // ESC m x,y:  - cursor motion
                            if let Some(cmd_type) = IgsCommandType::from_char(ch) {
                                self.state = State::ReadParams(cmd_type);
                            } else {
                                self.state = State::Default;
                            }
                        }
                        _ => {
                            // Unknown escape sequence, ignore
                            self.state = State::Default;
                        }
                    }
                }
                State::ReadFgColor => {
                    if let Some(color) = Self::parse_vt52_color(byte) {
                        let sgr = if self.reverse_video {
                            SgrAttribute::Background(color)
                        } else {
                            SgrAttribute::Foreground(color)
                        };
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(sgr));
                    } else {
                        sink.report_errror(
                            crate::ParseError::InvalidParameter {
                                command: "VT52 Foreground Color",
                                value: byte as u16,
                                expected: Some("valid color code (0x00-0x0F)"),
                            },
                            crate::ErrorLevel::Error,
                        );
                    }
                    self.state = State::Default;
                }

                State::ReadBgColor => {
                    if let Some(color) = Self::parse_vt52_color(byte) {
                        let sgr = if self.reverse_video {
                            SgrAttribute::Foreground(color)
                        } else {
                            SgrAttribute::Background(color)
                        };
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(sgr));
                    } else {
                        sink.report_errror(
                            crate::ParseError::InvalidParameter {
                                command: "VT52 Background Color",
                                value: byte as u16,
                                expected: Some("valid color code (0x00-0x0F)"),
                            },
                            crate::ErrorLevel::Error,
                        );
                    }
                    self.state = State::Default;
                }

                State::ReadCursorLine => {
                    if let Some(line) = Self::parse_cursor_line(byte) {
                        self.state = State::ReadCursorRow(line);
                    } else {
                        sink.report_errror(
                            crate::ParseError::InvalidParameter {
                                command: "VT52 Cursor Position (Column)",
                                value: byte as u16,
                                expected: Some("valid position byte (>= 32)"),
                            },
                            crate::ErrorLevel::Error,
                        );
                        self.state = State::Default;
                    }
                }
                State::ReadCursorRow(row) => {
                    if let Some(col) = Self::parse_cursor_row(byte) {
                        sink.emit(TerminalCommand::CsiCursorPosition(row as u16, col as u16));
                    } else {
                        sink.report_errror(
                            crate::ParseError::InvalidParameter {
                                command: "VT52 Cursor Position (Row)",
                                value: byte as u16,
                                expected: Some("valid position byte (>= 32)"),
                            },
                            crate::ErrorLevel::Error,
                        );
                    }
                    self.state = State::Default;
                }
                State::ReadInsertLineCount => {
                    let count = byte;
                    sink.emit(TerminalCommand::CsiInsertLine(count as u16));
                    self.state = State::Default;
                }
            }
        }
        // Flush pending ESC-style parameter commands without explicit ':' terminator (e.g. ESC m1,20)
        if let State::ReadParams(cmd_type) = self.state {
            match cmd_type {
                IgsCommandType::CursorMotion | IgsCommandType::InverseVideo | IgsCommandType::LineWrap => {
                    // Ensure last param captured
                    if self.current_param != 0 || !self.params.is_empty() {
                        self.push_current_param();
                    }
                    if !self.params.is_empty() {
                        self.emit_command(cmd_type, sink);
                        self.state = State::Default;
                    }
                }
                _ => {}
            }
        }
    }
}
