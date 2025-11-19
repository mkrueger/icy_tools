//! IGS (Interactive Graphics System) parser
//!
//! IGS is a graphics system developed for Atari ST BBS systems.
//! Commands start with 'G#' and use single-letter command codes followed by parameters.

use crate::{CommandParser, CommandSink, Vt52Parser};

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
    ReadLoopTextStrings,          // reading text strings for W@ loops after numeric params, terminated by @
    ReadZoneString(Vec<i32>),     // extended command X 4 zone string reading after numeric params
    ReadFillPattern(i32),         // extended command X 7 pattern data reading after id,pattern
}

pub struct IgsParser {
    state: State,
    params: Vec<IgsParameter>,
    current_param: i32,
    is_current_param_random: bool,
    text_buffer: Vec<u8>,

    loop_command: Vec<u8>,
    loop_parameters: Vec<Vec<Vec<u8>>>,
    loop_tokens: Vec<Vec<u8>>,
    loop_token_buffer: Vec<u8>,
    reading_chain_gang: bool, // True when reading >XXX@ chain-gang identifier

    vt52_parser: Vt52Parser,
    skip_next_lf: bool, // used for skipping LF in igs line G>....\n otherwise screen would scroll.
}

impl IgsParser {
    pub fn new() -> Self {
        Self {
            state: State::Default,
            params: Vec::new(),
            current_param: 0,
            is_current_param_random: false,
            text_buffer: Vec::new(),
            loop_command: Vec::new(),
            loop_parameters: Vec::new(),
            loop_tokens: Vec::new(),
            loop_token_buffer: Vec::new(),
            reading_chain_gang: false,
            vt52_parser: Vt52Parser::new(crate::vt52::VT52Mode::Mixed),
            skip_next_lf: false,
        }
    }

    fn reset_params(&mut self) {
        self.params.clear();
        self.current_param = 0;
        self.text_buffer.clear();
    }

    /// Parse i32 from byte slice (ASCII numeric string)
    #[inline]
    fn parse_i32_from_bytes(bytes: &[u8]) -> i32 {
        std::str::from_utf8(bytes).ok().and_then(|s| s.parse::<i32>().ok()).unwrap_or(0)
    }

    fn push_current_param(&mut self) {
        let param = if self.is_current_param_random {
            IgsParameter::Random
        } else {
            IgsParameter::Value(self.current_param)
        };
        self.params.push(param);
        self.current_param = 0;
        self.is_current_param_random = false;
    }

    fn create_loop_command(&mut self, sink: &mut dyn CommandSink) -> Option<IgsCommand> {
        // & from,to,step,delay,cmd,param_count,(params...)
        if self.params.len() < 6 {
            sink.report_errror(
                crate::ParseError::InvalidParameter {
                    command: "LoopCommand",
                    value: format!("{}", self.params.len()),
                    expected: Some("6 parameter".to_string()),
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
            let param_count = self.params[4].value() as u16;

            let mut params_tokens: Vec<LoopParamToken> = Vec::new();
            // Remaining numeric params are converted to tokens unless already substituted tokens in loop_parameters
            if self.params.len() > 5 {
                for p in &self.params[5..] {
                    params_tokens.push(LoopParamToken::Number(p.value()));
                }
            }
            // Add any textual parameter tokens captured (x,y,+n etc.)
            for token_group in &self.loop_parameters {
                for token in token_group {
                    if token == b":" {
                        params_tokens.push(LoopParamToken::GroupSeparator);
                    } else if token == b"x" {
                        params_tokens.push(LoopParamToken::StepForward);
                    } else if token == b"y" {
                        params_tokens.push(LoopParamToken::StepReverse);
                    } else if token == b"r" {
                        params_tokens.push(LoopParamToken::Random);
                    } else if let Some(&first) = token.first() {
                        // Check if token starts with a prefix operator (+, -, !)
                        if first == b'+' || first == b'-' || first == b'!' {
                            let operator = match first {
                                b'+' => ParamOperator::Add,
                                b'-' => ParamOperator::Subtract,
                                b'!' => ParamOperator::SubtractStep,
                                _ => unreachable!(),
                            };
                            let value = Self::parse_i32_from_bytes(&token[1..]);
                            params_tokens.push(LoopParamToken::Expr(operator, value));
                        } else {
                            let value = Self::parse_i32_from_bytes(token);
                            params_tokens.push(LoopParamToken::Number(value));
                        }
                    } else {
                        params_tokens.push(LoopParamToken::Number(0));
                    }
                }
            }

            let mut modifiers = LoopModifiers::default();
            let original_ident = String::from_utf8_lossy(&command_identifier);
            let mut base_ident = original_ident.as_ref();
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
                let commands: Vec<IgsCommandType> = inner.chars().filter_map(|ch| IgsCommandType::try_from(ch as u8).ok()).collect();
                LoopTarget::ChainGang { commands }
            } else {
                let ch = base_ident.chars().next().unwrap_or(' ');
                let cmd_type = IgsCommandType::try_from(ch as u8).unwrap_or(IgsCommandType::WriteText);
                LoopTarget::Single(cmd_type)
            };

            Some(IgsCommand::Loop(LoopCommandData {
                from: from.value(),
                to: to.value(),
                step: step.value(),
                delay: delay.value(),
                target,
                modifiers,
                param_count,
                params: params_tokens,
            }))
        }
    }

    fn emit_loop_command_with_texts(&mut self, sink: &mut dyn CommandSink) {
        use crate::igs::{LoopCommandData, LoopModifiers, LoopParamToken, LoopTarget};

        // Parse loop command from self.loop_tokens which now includes TEXT: markers
        if self.loop_tokens.len() < 6 {
            return;
        }

        let from = Self::parse_i32_from_bytes(&self.loop_tokens[0]);
        let to = Self::parse_i32_from_bytes(&self.loop_tokens[1]);
        let step = Self::parse_i32_from_bytes(&self.loop_tokens[2]);
        let delay = Self::parse_i32_from_bytes(&self.loop_tokens[3]);
        let raw_identifier = &self.loop_tokens[4];

        // Extract param_count from command identifier if it has modifiers + digits
        let mut param_count_from_token = None;
        if let Some(pos) = raw_identifier.iter().position(|&c| c == b'|' || c == b'@') {
            let mod_part = &raw_identifier[pos..];
            if let Some(digit_pos) = mod_part.iter().position(|&c| c.is_ascii_digit()) {
                param_count_from_token = Some(Self::parse_i32_from_bytes(&mod_part[digit_pos..]) as usize);
            }
        }
        let param_count = param_count_from_token.unwrap_or_else(|| {
            if self.loop_tokens.len() > 5 {
                Self::parse_i32_from_bytes(&self.loop_tokens[5]) as usize
            } else {
                0
            }
        });
        let params_start = if param_count_from_token.is_some() { 5 } else { 6 };

        // Parse target and modifiers from command identifier
        let mut modifiers = LoopModifiers::default();
        let original_ident = raw_identifier;
        let mut base_ident: &[u8] = original_ident.as_slice();

        // Check if this is a chain-gang command (>...@)
        let is_chain_gang = base_ident.starts_with(&[b'>']) && base_ident.contains(&b'@');

        let target = if is_chain_gang {
            // For chain-gangs, find the closing @ of the chain
            if let Some(chain_end_pos) = base_ident.iter().position(|&c| c == b'@') {
                let after_chain = &base_ident[chain_end_pos + 1..];
                // Parse modifiers that come after the chain-gang's @
                for &ch in after_chain {
                    match ch {
                        b'|' => modifiers.xor_stepping = true,
                        b'@' => modifiers.refresh_text_each_iteration = true,
                        _ => {}
                    }
                }
                // base_ident includes the chain-gang with its closing @
                base_ident = &base_ident[..=chain_end_pos];
                // Create ChainGang target
                let inner = &base_ident[1..base_ident.len().saturating_sub(1)];
                let commands: Vec<IgsCommandType> = inner.iter().filter_map(|&ch| IgsCommandType::try_from(ch).ok()).collect();
                LoopTarget::ChainGang { commands }
            } else {
                LoopTarget::Single(IgsCommandType::WriteText)
            }
        } else {
            // For single commands, parse modifiers normally
            if let Some(pos) = base_ident.iter().position(|&c| c == b'|' || c == b'@') {
                let (ident_part, mod_part) = base_ident.split_at(pos);
                base_ident = ident_part;
                for &ch in mod_part {
                    match ch {
                        b'|' => modifiers.xor_stepping = true,
                        b'@' => modifiers.refresh_text_each_iteration = true,
                        _ => {}
                    }
                }
            }
            let ch = base_ident.first().copied().unwrap_or(b' ');
            let cmd_type = IgsCommandType::try_from(ch).unwrap_or(IgsCommandType::WriteText);
            LoopTarget::Single(cmd_type)
        };

        // Convert parameters including text strings to typed tokens
        let mut params: Vec<LoopParamToken> = Vec::new();
        for token in &self.loop_tokens[params_start..] {
            if token.starts_with(b"TEXT:") {
                // Extract text content after "TEXT:" prefix
                let text = token[5..].to_vec();
                params.push(LoopParamToken::Text(text));
            } else if token == b":" {
                params.push(LoopParamToken::GroupSeparator);
            } else if token == b"x" {
                params.push(LoopParamToken::StepForward);
            } else if token == b"y" {
                params.push(LoopParamToken::StepReverse);
            } else if token == b"r" {
                params.push(LoopParamToken::Random);
            } else {
                // Check if token starts with a prefix operator (+, -, !)
                if let Some(&first) = token.first() {
                    if first == b'+' || first == b'-' || first == b'!' {
                        let operator = match first {
                            b'+' => ParamOperator::Add,
                            b'-' => ParamOperator::Subtract,
                            b'!' => ParamOperator::SubtractStep,
                            _ => unreachable!(),
                        };
                        let value = Self::parse_i32_from_bytes(&token[1..]);
                        params.push(LoopParamToken::Expr(operator, value));
                    } else {
                        let value = Self::parse_i32_from_bytes(token);
                        params.push(LoopParamToken::Number(value));
                    }
                } else {
                    params.push(LoopParamToken::Number(0));
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
        self.reading_chain_gang = false;
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
            match self.state {
                State::Default => {
                    match byte {
                        b'G' => {
                            if self.vt52_parser.is_default_state() {
                                self.state = State::GotG;
                            } else {
                                self.vt52_parser.parse(&[byte], sink);
                            }
                        }
                        _ => {
                            // Delegate all other bytes to VT52 parser
                            self.vt52_parser.parse(&[byte], sink);
                        }
                    }
                }
                State::GotG => {
                    self.skip_next_lf = true;
                    if byte == b'#' {
                        self.state = State::GotIgsStart;
                        self.reset_params();
                    } else {
                        // False alarm, just 'G' followed by something else
                        sink.print(&[b'G', byte]);
                        self.state = State::Default;
                    }
                }
                State::GotIgsStart => {
                    if byte == b'&' {
                        // Loop command
                        // Use specialized token parser for loop command because parameters include substitution tokens.
                        self.state = State::ReadLoopTokens;
                        self.loop_tokens.clear();
                        self.loop_token_buffer.clear();
                    } else if let Ok(cmd_type) = IgsCommandType::try_from(byte) {
                        self.state = State::ReadParams(cmd_type);
                    } else {
                        // Unknown command
                        if !(byte < 0x20 || byte == 0x7F) {
                            sink.report_errror(
                                crate::ParseError::InvalidParameter {
                                    command: "IGS",
                                    value: format!("{}", byte).to_string(),
                                    expected: Some("valid IGS command character".to_string()),
                                },
                                crate::ErrorLevel::Error,
                            );
                        }
                        self.state = State::Default;
                    }
                }
                State::ReadParams(cmd_type) => {
                    match byte {
                        b'r' => {
                            self.is_current_param_random = true;
                        }
                        b'R' => {
                            // Big random - für jetzt als Random behandeln, kann später erweitert werden
                            self.is_current_param_random = true;
                        }
                        b'0'..=b'9' => {
                            self.current_param = self.current_param.wrapping_mul(10).wrapping_add((byte - b'0') as i32);
                        }
                        b',' => {
                            self.push_current_param();
                            // For WriteText: after 2 params (x, y), next non-separator char starts text
                            if cmd_type == IgsCommandType::WriteText && self.params.len() == 2 {
                                // W>x,y,text@ - text follows immediately after second comma
                                self.state = State::ReadTextString(self.params[0].value(), self.params[1].value(), 0);
                                self.text_buffer.clear();
                            }
                        }
                        b'@' if cmd_type == IgsCommandType::WriteText => {
                            // For WriteText: @ starts text after x,y params
                            self.push_current_param();
                            if self.params.len() == 2 {
                                // W>x,y@text@ format
                                self.state = State::ReadTextString(self.params[0].value(), self.params[1].value(), 0);
                                self.text_buffer.clear();
                            } else {
                                // Invalid - WriteText needs exactly 2 params before @
                                self.reset_params();
                                self.state = State::Default;
                            }
                        }
                        b':' => {
                            // Command terminator
                            self.push_current_param();
                            self.emit_command(cmd_type, sink);
                            self.state = State::GotIgsStart;
                        }
                        b' ' | b'>' | b'\r' | b'\n' | b'_' => {
                            // Whitespace/formatting - ignore
                            // Special handling: extended command X 4 (DefineZone) starts string after 7 numeric params
                            if let State::ReadParams(IgsCommandType::ExtendedCommand) = self.state {
                                if !self.params.is_empty() && self.params[0].value() == 4 && self.params.len() == 7 {
                                    // Switch into zone string reading state (length already captured)
                                    let int_params: Vec<i32> = self.params.iter().map(|p| p.value()).collect();
                                    self.state = State::ReadZoneString(int_params);
                                    self.text_buffer.clear();
                                } else if !self.params.is_empty() && self.params[0].value() == 7 && self.params.len() == 2 {
                                    // Switch to fill pattern reading state
                                    let pattern = self.params[1].value();
                                    self.state = State::ReadFillPattern(pattern);
                                    self.text_buffer.clear();
                                }
                            }
                        }
                        _ => {
                            // Extended command X 4 zone string may contain arbitrary characters until ':'
                            if let State::ReadParams(IgsCommandType::ExtendedCommand) = self.state {
                                if !self.params.is_empty() && self.params[0].value() == 4 && self.params.len() == 7 {
                                    let int_params: Vec<i32> = self.params.iter().map(|p| p.value()).collect();
                                    self.state = State::ReadZoneString(int_params);
                                    self.text_buffer.clear();
                                    self.text_buffer.push(byte);
                                } else if !self.params.is_empty() && self.params[0].value() == 7 && self.params.len() == 2 {
                                    let pattern = self.params[1].value();
                                    self.state = State::ReadFillPattern(pattern);
                                    self.text_buffer.clear();
                                    self.text_buffer.push(byte);
                                } else {
                                    // Invalid for other extended commands
                                    sink.report_errror(
                                        crate::ParseError::InvalidParameter {
                                            command: "ExtendedCommand",
                                            value: format!("{}", byte).to_string(),
                                            expected: Some("digit, ',', ':' oder gültiger Text für X4/X7".to_string()),
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
                                        value: format!("{}", byte).to_string(),
                                        expected: Some("Ziffer, ',', ':' oder Whitespace".to_string()),
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
                    match byte {
                        b':' | b'\n' => {
                            // Terminator: build DefineZone command (X 4)
                            if zone_params.len() == 7 {
                                let zone_id = zone_params[1];
                                let x1 = zone_params[2].into();
                                let y1 = zone_params[3].into();
                                let x2 = zone_params[4].into();
                                let y2 = zone_params[5].into();
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
                            self.state = if byte == b'\n' { State::Default } else { State::GotIgsStart };
                        }
                        _ => {
                            self.text_buffer.push(byte);
                        }
                    }
                }
                State::ReadLoopTokens => {
                    let ch = byte as char;
                    match ch {
                        ':' => {
                            if !self.loop_token_buffer.is_empty() {
                                self.loop_tokens.push(self.loop_token_buffer.clone());
                                self.loop_token_buffer.clear();
                            }

                            // Check if we have enough tokens and if we've collected all expected parameters
                            if self.loop_tokens.len() >= 6 {
                                let param_count = Self::parse_i32_from_bytes(&self.loop_tokens[5]) as usize;
                                // Count actual parameters (excluding ':' markers)
                                let current_param_count = self.loop_tokens[6..].iter().filter(|s| s.as_slice() != b":").count();

                                // Check if this is a W@ loop that expects text strings
                                let raw_identifier = &self.loop_tokens[4];
                                let is_write_text_with_refresh = {
                                    let mut base_ident = raw_identifier.as_slice();
                                    // Strip modifiers to get base command
                                    if let Some(pos) = base_ident.iter().position(|&c| c == b'|' || c == b'@') {
                                        base_ident = &base_ident[..pos];
                                    }
                                    base_ident == b"W" && raw_identifier.contains(&b'@')
                                };

                                // For W@ loops, calculate how many text parameters are expected
                                // Based on IG217.C: iteration_count = (to - from) / step + 1
                                // Text params are the LAST parameters, after any numeric/symbolic params
                                if is_write_text_with_refresh && current_param_count < param_count {
                                    let from = Self::parse_i32_from_bytes(&self.loop_tokens[0]);
                                    let to = Self::parse_i32_from_bytes(&self.loop_tokens[1]);
                                    let step = Self::parse_i32_from_bytes(&self.loop_tokens[2]);

                                    // Calculate iteration count
                                    let iteration_count = if step != 0 { ((to - from).abs() / step.abs() + 1).max(0) as usize } else { 0 };

                                    // Expected text count is the minimum of:
                                    // 1. Remaining params to collect (param_count - current_param_count)
                                    // 2. Iteration count (one text per iteration)
                                    let expected_text_count = (param_count - current_param_count).min(iteration_count);

                                    if expected_text_count > 0 {
                                        // W@ loop expects text strings - switch to text reading mode
                                        self.loop_tokens.push(b":".to_vec()); // Mark the separator
                                        self.state = State::ReadLoopTextStrings;
                                    } else {
                                        // No texts expected, emit immediately
                                        self.loop_tokens.push(b":".to_vec());
                                    }
                                } else if current_param_count >= param_count {
                                    let from = Self::parse_i32_from_bytes(&self.loop_tokens[0]);
                                    let to = Self::parse_i32_from_bytes(&self.loop_tokens[1]);
                                    let step = Self::parse_i32_from_bytes(&self.loop_tokens[2]);
                                    let delay = Self::parse_i32_from_bytes(&self.loop_tokens[3]);
                                    let raw_identifier = &self.loop_tokens[4];

                                    // Extract param_count from command identifier if it has modifiers + digits
                                    let mut extracted_param_count = None;
                                    if let Some(pos) = raw_identifier.iter().position(|&c| c == b'|' || c == b'@') {
                                        let mod_part = &raw_identifier[pos..];
                                        if let Some(digit_pos) = mod_part.iter().position(|&c| c.is_ascii_digit()) {
                                            extracted_param_count = Some(Self::parse_i32_from_bytes(&mod_part[digit_pos..]) as usize);
                                        }
                                    }
                                    let param_count = extracted_param_count.unwrap_or(param_count);

                                    // Parse target and modifiers from command identifier
                                    // For chain-gangs (>XXX@), the @ is part of the identifier, not a modifier
                                    // Modifiers come AFTER the chain-gang's closing @
                                    let mut modifiers = LoopModifiers::default();
                                    let original_ident = raw_identifier;
                                    let mut base_ident = original_ident.as_slice();
                                    let mut target = LoopTarget::Single(IgsCommandType::WriteText);

                                    // Check if this is a chain-gang command (>...@)
                                    let is_chain_gang = base_ident.starts_with(&[b'>']) && base_ident.contains(&b'@');

                                    if is_chain_gang {
                                        // For chain-gangs, find the closing @ of the chain
                                        if let Some(chain_end_pos) = base_ident.iter().position(|&c| c == b'@') {
                                            let after_chain = &base_ident[chain_end_pos + 1..];
                                            // Parse modifiers that come after the chain-gang's @
                                            for &ch in after_chain {
                                                match ch {
                                                    b'|' => modifiers.xor_stepping = true,
                                                    b'@' => modifiers.refresh_text_each_iteration = true,
                                                    _ => {}
                                                }
                                            }
                                            // base_ident includes the chain-gang with its closing @
                                            base_ident = &base_ident[..=chain_end_pos];
                                            // Create ChainGang target with the base_ident (which includes @)
                                            let inner = &base_ident[1..base_ident.len().saturating_sub(1)];
                                            let commands: Vec<IgsCommandType> = inner.iter().filter_map(|&ch| IgsCommandType::try_from(ch).ok()).collect();
                                            target = LoopTarget::ChainGang { commands };
                                        }
                                    } else {
                                        // For single commands, parse modifiers normally
                                        if let Some(pos) = base_ident.iter().position(|&c| c == b'|' || c == b'@') {
                                            let (ident_part, mod_part) = base_ident.split_at(pos);
                                            base_ident = ident_part;
                                            for &ch in mod_part {
                                                match ch {
                                                    b'|' => modifiers.xor_stepping = true,
                                                    b'@' => modifiers.refresh_text_each_iteration = true,
                                                    _ => {}
                                                }
                                            }
                                        }
                                    }

                                    if matches!(target, LoopTarget::Single(IgsCommandType::WriteText)) {
                                        target = if base_ident.starts_with(&[b'>']) && original_ident.contains(&b'@') {
                                            let inner = &base_ident[1..];
                                            let commands: Vec<IgsCommandType> = inner.iter().filter_map(|&ch| IgsCommandType::try_from(ch).ok()).collect();
                                            LoopTarget::ChainGang { commands }
                                        } else {
                                            let ch = base_ident.first().copied().unwrap_or(b' ');
                                            let cmd_type = IgsCommandType::try_from(ch).unwrap_or(IgsCommandType::WriteText);
                                            LoopTarget::Single(cmd_type)
                                        };
                                    }

                                    // Determine parameter start position:
                                    // If command identifier contains modifiers followed by digits, params start at token 5
                                    // Otherwise params start at token 6
                                    let has_modifier_with_digits = {
                                        let has_modifiers = raw_identifier.contains(&b'|') || raw_identifier.contains(&b'@');
                                        if has_modifiers {
                                            // Check if there's a digit after the modifiers
                                            if let Some(pos) = raw_identifier.iter().position(|&c| c == b'|' || c == b'@') {
                                                let after_mods = &raw_identifier[pos..];
                                                after_mods.iter().any(|&c| c.is_ascii_digit())
                                            } else {
                                                false
                                            }
                                        } else {
                                            false
                                        }
                                    };
                                    let params_start = if has_modifier_with_digits { 5 } else { 6 };

                                    // Convert parameters into typed tokens, preserving ':' position
                                    let mut params: Vec<LoopParamToken> = Vec::new();
                                    for token in &self.loop_tokens[params_start..] {
                                        if token == b":" {
                                            params.push(LoopParamToken::GroupSeparator);
                                        } else if token == b"x" {
                                            params.push(LoopParamToken::StepForward);
                                        } else if token == b"y" {
                                            params.push(LoopParamToken::StepReverse);
                                        } else if token == b"r" {
                                            params.push(LoopParamToken::Random);
                                        } else if let Some(&first) = token.first() {
                                            // Check if token starts with a prefix operator (+, -, !)
                                            if first == b'+' || first == b'-' || first == b'!' {
                                                let operator = match first {
                                                    b'+' => ParamOperator::Add,
                                                    b'-' => ParamOperator::Subtract,
                                                    b'!' => ParamOperator::SubtractStep,
                                                    _ => unreachable!(),
                                                };
                                                let value = Self::parse_i32_from_bytes(&token[1..]);
                                                params.push(LoopParamToken::Expr(operator, value));
                                            } else {
                                                let value = Self::parse_i32_from_bytes(token);
                                                params.push(LoopParamToken::Number(value));
                                            }
                                        } else {
                                            params.push(LoopParamToken::Number(0));
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
                                    self.loop_tokens.push(b":".to_vec());
                                }
                            }
                        }
                        '\n' => {
                            if !self.loop_token_buffer.is_empty() {
                                self.loop_tokens.push(self.loop_token_buffer.clone());
                                self.loop_token_buffer.clear();
                            }

                            // For W@ loops with text strings, newlines are part of the text content
                            // Don't emit the loop on newline if we're expecting text strings
                            // Check if this might be a W@ loop that's still reading text
                            let is_w_at_loop = self.loop_tokens.len() >= 5 && {
                                let ident = &self.loop_tokens[4];
                                ident.starts_with(&[b'W']) && ident.contains(&b'@')
                            };

                            // Process tokens even if incomplete on newline
                            // Need at least 5 tokens: from, to, step, delay, command_identifier (may contain modifiers + param_count)
                            if self.loop_tokens.len() >= 5 && !is_w_at_loop {
                                use crate::igs::{LoopCommandData, LoopModifiers, LoopParamToken, LoopTarget};

                                let from = Self::parse_i32_from_bytes(&self.loop_tokens[0]);
                                let to = Self::parse_i32_from_bytes(&self.loop_tokens[1]);
                                let step = Self::parse_i32_from_bytes(&self.loop_tokens[2]);
                                let delay = Self::parse_i32_from_bytes(&self.loop_tokens[3]);
                                let raw_identifier = &self.loop_tokens[4];

                                // Parse modifiers and param_count from command identifier token
                                // Format can be: "W", "W@2", "L|4", "W|@1", ">CL@", etc.
                                let mut modifiers = LoopModifiers::default();
                                let original_ident = raw_identifier.as_slice();
                                let mut base_ident = original_ident;
                                let mut param_count_from_token = None;

                                if let Some(pos) = base_ident.iter().position(|&c| c == b'|' || c == b'@') {
                                    let (ident_part, mod_part) = base_ident.split_at(pos);
                                    base_ident = ident_part;

                                    // Extract modifiers and any trailing param_count digits
                                    let mut digits_start = None;
                                    for (i, &ch) in mod_part.iter().enumerate() {
                                        match ch {
                                            b'|' => modifiers.xor_stepping = true,
                                            b'@' => modifiers.refresh_text_each_iteration = true,
                                            b'0'..=b'9' => {
                                                if digits_start.is_none() {
                                                    digits_start = Some(i);
                                                }
                                            }
                                            _ => {}
                                        }
                                    }

                                    // Extract param_count if present after modifiers
                                    if let Some(start) = digits_start {
                                        let param_str = &mod_part[start..];
                                        param_count_from_token = Some(Self::parse_i32_from_bytes(param_str) as usize);
                                    }
                                }

                                // Use param_count from token if present, otherwise from token 5
                                let param_count = param_count_from_token.unwrap_or_else(|| {
                                    if self.loop_tokens.len() > 5 {
                                        Self::parse_i32_from_bytes(&self.loop_tokens[5]) as usize
                                    } else {
                                        0
                                    }
                                });

                                let target = if base_ident.starts_with(&[b'>']) && original_ident.contains(&b'@') {
                                    let inner = &base_ident[1..];
                                    let commands: Vec<IgsCommandType> = inner.iter().filter_map(|&ch| IgsCommandType::try_from(ch).ok()).collect();
                                    LoopTarget::ChainGang { commands }
                                } else {
                                    let ch = base_ident.first().copied().unwrap_or(b' ');
                                    let cmd_type = IgsCommandType::try_from(ch).unwrap_or(IgsCommandType::WriteText);
                                    LoopTarget::Single(cmd_type)
                                };

                                // Determine where parameters start:
                                // If param_count was extracted from token 4 (has modifiers), params start at token 5
                                // Otherwise params start at token 6 (after explicit param_count token)
                                let params_start = if param_count_from_token.is_some() { 5 } else { 6 };

                                let mut params: Vec<LoopParamToken> = Vec::new();
                                for token in &self.loop_tokens[params_start..] {
                                    if token.as_slice() == b":" {
                                        params.push(LoopParamToken::GroupSeparator);
                                    } else if token.as_slice() == b"x" {
                                        params.push(LoopParamToken::StepForward);
                                    } else if token.as_slice() == b"y" {
                                        params.push(LoopParamToken::StepReverse);
                                    } else if token.as_slice() == b"r" {
                                        params.push(LoopParamToken::Random);
                                    } else if let Some(&first) = token.first() {
                                        // Check if token starts with a prefix operator (+, -, !)
                                        if first == b'+' || first == b'-' || first == b'!' {
                                            let operator = match first {
                                                b'+' => ParamOperator::Add,
                                                b'-' => ParamOperator::Subtract,
                                                b'!' => ParamOperator::SubtractStep,
                                                _ => unreachable!(),
                                            };
                                            let value = Self::parse_i32_from_bytes(&token[1..]);
                                            params.push(LoopParamToken::Expr(operator, value));
                                        } else {
                                            let value = Self::parse_i32_from_bytes(token);
                                            params.push(LoopParamToken::Number(value));
                                        }
                                    } else {
                                        params.push(LoopParamToken::Number(0));
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
                                self.loop_token_buffer.push(ch as u8);
                            } else if !self.loop_token_buffer.is_empty() {
                                self.loop_tokens.push(self.loop_token_buffer.clone());
                                self.loop_token_buffer.clear();
                            }

                            // After comma, check if we should switch to W@ text mode
                            // This must happen BEFORE the next token starts being collected
                            if self.loop_tokens.len() >= 5 {
                                // Extract param_count from token 4 (command identifier) if it has modifiers
                                let raw_identifier = &self.loop_tokens[4];
                                let mut param_count_from_token = None;
                                if let Some(pos) = raw_identifier.iter().position(|&c| c == b'|' || c == b'@') {
                                    let mod_part = &raw_identifier[pos..];
                                    if let Some(digit_pos) = mod_part.iter().position(|&c| c.is_ascii_digit()) {
                                        param_count_from_token = Some(Self::parse_i32_from_bytes(&mod_part[digit_pos..]) as usize);
                                    }
                                }

                                let param_count = if let Some(pc) = param_count_from_token {
                                    pc
                                } else if self.loop_tokens.len() >= 6 {
                                    Self::parse_i32_from_bytes(&self.loop_tokens[5]) as usize
                                } else {
                                    0
                                };

                                let params_start = if param_count_from_token.is_some() { 5 } else { 6 };
                                let current_param_count = if params_start < self.loop_tokens.len() {
                                    self.loop_tokens[params_start..].iter().filter(|s| s.as_slice() != b":").count()
                                } else {
                                    0
                                };

                                // Check if this is a W@ loop
                                let is_write_text_with_refresh = {
                                    let mut base_ident = raw_identifier.as_slice();
                                    if let Some(pos) = base_ident.iter().position(|&c| c == b'|' || c == b'@') {
                                        base_ident = &base_ident[..pos];
                                    }
                                    base_ident == b"W" && raw_identifier.contains(&b'@')
                                };

                                // Calculate if text parameters are expected
                                // For W@ loops: param_count specifies number of NUMERIC params
                                // After numeric params, there are iteration_count text strings (one per iteration)
                                if is_write_text_with_refresh {
                                    let from = Self::parse_i32_from_bytes(&self.loop_tokens[0]);
                                    let to = Self::parse_i32_from_bytes(&self.loop_tokens[1]);
                                    let step = Self::parse_i32_from_bytes(&self.loop_tokens[2]);

                                    let iteration_count = if step != 0 { ((to - from).abs() / step.abs() + 1).max(0) as usize } else { 0 };

                                    // Switch to text mode after collecting all numeric params
                                    // There will be iteration_count text strings following
                                    if iteration_count > 0 && current_param_count >= param_count {
                                        // Switch to text mode NOW, before next character
                                        self.state = State::ReadLoopTextStrings;
                                    }
                                }
                            }
                        }
                        ')' => {
                            // Closing paren marks command index in chain-gang parameters
                            // Keep it as part of the token for display purposes
                            if !self.loop_token_buffer.is_empty() {
                                self.loop_token_buffer.push(ch as u8);
                                self.loop_tokens.push(self.loop_token_buffer.clone());
                                self.loop_token_buffer.clear();
                            }
                        }
                        '@' => {
                            // @ can end a chain-gang identifier or be a modifier
                            self.loop_token_buffer.push(ch as u8);
                            if self.reading_chain_gang {
                                // This @ ends the chain-gang identifier
                                // Push token and clear flag
                                self.loop_tokens.push(self.loop_token_buffer.clone());
                                self.loop_token_buffer.clear();
                                self.reading_chain_gang = false;
                            }
                            // Note: Don't push token here for command identifier modifiers
                            // The modifier and following param_count are read together until comma
                        }
                        '|' => {
                            // | is XOR modifier for command identifier
                            self.loop_token_buffer.push(ch as u8);
                            // Note: Don't push token here
                            // The modifier and following param_count are read together until comma
                        }
                        ' ' | '\r' | '_' => {
                            // ignore these formatting chars entirely for loop tokens
                        }
                        '>' => {
                            // '>' can be part of chain-gang identifier (e.g., >CL@) or a formatting char
                            // If buffer is empty and we're at the command identifier position, it starts a chain-gang
                            if self.loop_token_buffer.is_empty() && self.loop_tokens.len() == 4 {
                                // We're at the command identifier position (5th token, index 4)
                                self.loop_token_buffer.push(ch as u8);
                                self.reading_chain_gang = true;
                            }
                            // Otherwise ignore as formatting
                        }
                        _ => {
                            self.loop_token_buffer.push(ch as u8);
                        }
                    }
                }
                State::ReadLoopTextStrings => {
                    // Reading text strings for W@ loops
                    // Each text is terminated by @, and we continue until we have collected all expected texts
                    match byte {
                        b':' => {
                            // Colon can be part of text OR terminate the loop
                            // It terminates only if we've collected all expected texts (one per iteration)
                            let text_count = self.loop_tokens.iter().filter(|t| t.starts_with(b"TEXT:")).count();

                            let from = Self::parse_i32_from_bytes(&self.loop_tokens[0]);
                            let to = Self::parse_i32_from_bytes(&self.loop_tokens[1]);
                            let step = Self::parse_i32_from_bytes(&self.loop_tokens[2]);
                            let iteration_count = if step != 0 { ((to - from).abs() / step.abs() + 1).max(0) as usize } else { 0 };

                            if text_count >= iteration_count {
                                // We have all parameters - colon terminates
                                if !self.text_buffer.is_empty() {
                                    let text = self.text_buffer.clone();
                                    let mut text_token = b"TEXT:".to_vec();
                                    text_token.extend_from_slice(&text);
                                    self.loop_tokens.push(text_token);
                                    self.text_buffer.clear();
                                }
                                self.emit_loop_command_with_texts(sink);
                                self.state = State::GotIgsStart;
                            } else {
                                // Colon is part of the text
                                self.text_buffer.push(byte);
                            }
                        }
                        b'\n' => {
                            // Newline can also terminate or be part of text
                            // If we've collected enough text strings (one per iteration), emit
                            let text_count = self.loop_tokens.iter().filter(|t| t.starts_with(b"TEXT:")).count();

                            let from = Self::parse_i32_from_bytes(&self.loop_tokens[0]);
                            let to = Self::parse_i32_from_bytes(&self.loop_tokens[1]);
                            let step = Self::parse_i32_from_bytes(&self.loop_tokens[2]);
                            let iteration_count = if step != 0 { ((to - from).abs() / step.abs() + 1).max(0) as usize } else { 0 };

                            if text_count >= iteration_count {
                                // We have all parameters - emit and end
                                if !self.text_buffer.is_empty() {
                                    let text = self.text_buffer.clone();
                                    let mut text_token = b"TEXT:".to_vec();
                                    text_token.extend_from_slice(&text);
                                    self.loop_tokens.push(text_token);
                                    self.text_buffer.clear();
                                }
                                self.emit_loop_command_with_texts(sink);
                                self.state = State::Default;
                            } else {
                                // Newline is part of text string
                                self.text_buffer.push(byte);
                            }
                        }
                        b'@' => {
                            // End of current text string
                            let text = self.text_buffer.clone();
                            let mut text_token = b"TEXT:".to_vec();
                            text_token.extend_from_slice(&text);
                            self.loop_tokens.push(text_token);
                            self.text_buffer.clear();

                            // Check if we have collected enough text strings
                            // For W@ loops: expect iteration_count text strings
                            let text_count = self.loop_tokens.iter().filter(|t| t.starts_with(b"TEXT:")).count();

                            let from = Self::parse_i32_from_bytes(&self.loop_tokens[0]);
                            let to = Self::parse_i32_from_bytes(&self.loop_tokens[1]);
                            let step = Self::parse_i32_from_bytes(&self.loop_tokens[2]);
                            let iteration_count = if step != 0 { ((to - from).abs() / step.abs() + 1).max(0) as usize } else { 0 };

                            if text_count >= iteration_count {
                                // We have all parameters - wait for : or newline to terminate
                                // Don't emit yet, wait for terminator
                            }
                            // Otherwise continue reading next text string
                        }
                        _ => {
                            self.text_buffer.push(byte);
                        }
                    }
                }
                State::ReadFillPattern(pattern) => match byte {
                    b':' | b'\n' => {
                        sink.emit_igs(IgsCommand::LoadFillPattern {
                            pattern: pattern as u8,
                            data: self.text_buffer.clone(),
                        });
                        self.reset_params();
                        self.state = if byte == b'\n' { State::Default } else { State::GotIgsStart };
                    }
                    _ => self.text_buffer.push(byte),
                },
                State::ReadTextString(_x, _y, _just) => {
                    if byte == b'@' || byte == b'\n' {
                        // End of text string
                        self.emit_command(IgsCommandType::WriteText, sink);
                        self.state = if byte == b'\n' { State::Default } else { State::GotIgsStart };
                    } else {
                        self.text_buffer.push(byte);
                    }
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
