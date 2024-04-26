use std::path::PathBuf;

use self::bgi::{Bgi, MouseField};

use super::{ansi, BufferParser};
use crate::{ansi::EngineState, Buffer, CallbackAction, Caret, EngineResult, ParserError, Rectangle, Size};

pub mod bgi;
mod commands;

#[cfg(test)]
mod tests;

#[derive(Default, Debug)]
enum State {
    #[default]
    Default,
    GotRipStart,
    ReadCommand(usize),
    ReadParams,
    SkipEOL,
    EndRip,
}

#[derive(Default)]
pub enum WriteMode {
    #[default]
    Normal,
    Xor,
}

pub trait Command {
    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn parse(&mut self, _state: &mut i32, _ch: char) -> EngineResult<bool> {
        Err(anyhow::Error::msg("Invalid state"))
    }

    fn to_rip_string(&self) -> String;

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn run(&self, _buf: &mut Buffer, _caret: &mut Caret, _bgi: &mut Bgi) -> EngineResult<CallbackAction> {
        println!("not implemented RIP: {:?}", self.to_rip_string());
        Ok(CallbackAction::NoUpdate)
    }
}

pub struct Parser {
    fallback_parser: Box<ansi::Parser>,
    enable_rip: bool,
    state: State,

    parameter_state: i32,
    _text_window: Option<Rectangle>,
    _viewport: Option<Rectangle>,
    _current_write_mode: WriteMode,

    rip_counter: i32,
    rip_commands: Vec<Box<dyn Command>>,
    command: Option<Box<dyn Command>>,
    last_rip_update: i32,
    pub bgi: Bgi,
    pub record_rip_commands: bool,
}

impl Parser {
    pub fn new(fallback_parser: Box<ansi::Parser>, file_path: PathBuf) -> Self {
        Self {
            fallback_parser,
            enable_rip: true,
            state: State::Default,
            parameter_state: 0,
            _text_window: None,
            _viewport: None,
            _current_write_mode: WriteMode::Normal,
            rip_commands: Vec::new(),
            command: None,
            bgi: Bgi::new(file_path),
            last_rip_update: 0,
            record_rip_commands: false,
            rip_counter: 0,
        }
    }

    pub fn clear(&mut self) {
        // clear viewport
    }

    fn record_rip_command(&mut self, buf: &mut Buffer, caret: &mut Caret, cmd: Box<dyn Command>) -> EngineResult<CallbackAction> {
        self.rip_counter = self.rip_counter.wrapping_add(1);
        let result = cmd.run(buf, caret, &mut self.bgi);
        if !self.record_rip_commands {
            return result;
        }
        self.rip_commands.push(cmd);
        result
    }

    fn parse_parameter(&mut self, buf: &mut Buffer, caret: &mut Caret, ch: char) -> Option<Result<CallbackAction, anyhow::Error>> {
        if ch == '\\' {
            self.state = State::SkipEOL;
            return Some(Ok(CallbackAction::NoUpdate));
        }
        if ch == '\r' {
            return Some(Ok(CallbackAction::NoUpdate));
        }
        if ch == '\n' {
            self.state = State::Default;
            if let Some(t) = self.command.take() {
                return Some(self.record_rip_command(buf, caret, t));
            }
            return Some(Ok(CallbackAction::NoUpdate));
        }
        if ch == '|' {
            self.state = State::ReadCommand(0);
            if let Some(t) = self.command.take() {
                return Some(self.record_rip_command(buf, caret, t));
            }
            return Some(Ok(CallbackAction::NoUpdate));
        }
        match self.command.as_mut().unwrap().parse(&mut self.parameter_state, ch) {
            Ok(true) => {
                self.parameter_state += 1;
            }
            Ok(false) => {
                if let Some(t) = self.command.take() {
                    self.state = State::GotRipStart;
                    return Some(self.record_rip_command(buf, caret, t));
                }
            }
            Err(e) => {
                log::error!("Error in RipScript: {}", e);
                self.state = State::Default;
                return Some(Ok(CallbackAction::NoUpdate));
            }
        }
        None
    }
}

static RIP_TERMINAL_ID: &str = "RIPSCRIP015410\0";

impl Parser {
    pub fn start_command(&mut self, cmd: Box<dyn Command>) {
        self.command = Some(cmd);
        self.parameter_state = 0;
        self.state = State::ReadParams;
    }

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn push_command(&mut self, buf: &mut Buffer, caret: &mut Caret, cmd: Box<dyn Command>) -> EngineResult<CallbackAction> {
        self.state = State::GotRipStart;
        self.record_rip_command(buf, caret, cmd)
    }
}

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut Buffer, current_layer: usize, caret: &mut Caret, ch: char) -> EngineResult<CallbackAction> {
        if buf.terminal_state.cleared_screen {
            buf.terminal_state.cleared_screen = false;
            self.bgi.graph_defaults();
            self.rip_counter += 1;
        }

        match self.state {
            State::ReadParams => {
                if let Some(value) = self.parse_parameter(buf, caret, ch) {
                    return value;
                }
                return Ok(CallbackAction::NoUpdate);
            }
            State::SkipEOL => {
                if ch == '\r' {
                    return Ok(CallbackAction::NoUpdate);
                }
                if ch == '\n' {
                    self.state = State::ReadParams;
                    return Ok(CallbackAction::NoUpdate);
                }
                if let Some(value) = self.parse_parameter(buf, caret, ch) {
                    return value;
                }
                self.state = State::ReadParams;
                return Ok(CallbackAction::NoUpdate);
            }
            State::EndRip => {
                if ch == '\r' {
                    return Ok(CallbackAction::NoUpdate);
                }
                if ch == '\n' {
                    self.state = State::Default;
                    return Ok(CallbackAction::NoUpdate);
                }

                if ch == '|' {
                    self.state = State::ReadCommand(0);
                    return Ok(CallbackAction::NoUpdate);
                }

                self.state = State::Default;
                return Ok(CallbackAction::NoUpdate);
            }

            State::ReadCommand(level) => {
                if ch == '!' {
                    self.state = State::GotRipStart;
                    return Ok(CallbackAction::NoUpdate);
                }

                if level == 1 {
                    match ch {
                        'M' => self.start_command(Box::<commands::Mouse>::default()),
                        'K' => return self.push_command(buf, caret, Box::<commands::MouseFields>::default()),
                        'T' => self.start_command(Box::<commands::BeginText>::default()),
                        't' => self.start_command(Box::<commands::RegionText>::default()),
                        'E' => return self.push_command(buf, caret, Box::<commands::EndText>::default()),
                        'C' => self.start_command(Box::<commands::GetImage>::default()),
                        'P' => self.start_command(Box::<commands::PutImage>::default()),
                        'W' => self.start_command(Box::<commands::WriteIcon>::default()),
                        'I' => self.start_command(Box::<commands::LoadIcon>::default()),
                        'B' => self.start_command(Box::<commands::ButtonStyle>::default()),
                        'U' => self.start_command(Box::<commands::Button>::default()),
                        'D' => self.start_command(Box::<commands::Define>::default()),
                        '\x1B' => self.start_command(Box::<commands::Query>::default()),
                        'G' => self.start_command(Box::<commands::CopyRegion>::default()),
                        'R' => self.start_command(Box::<commands::ReadScene>::default()),
                        'F' => self.start_command(Box::<commands::FileQuery>::default()),

                        _ => {
                            log::error!("Error in RipScript: Unknown level 1 command: {}", ch);
                            self.state = State::Default;
                            return Ok(CallbackAction::NoUpdate);
                        }
                    }
                    return Ok(CallbackAction::NoUpdate);
                }
                if level == 9 {
                    if let '\x1B' = ch {
                        self.start_command(Box::<commands::EnterBlockMode>::default());
                    } else {
                        log::error!("Error in RipScript: Unknown level 1 command: {}", ch);
                        self.state = State::Default;
                        return Ok(CallbackAction::NoUpdate);
                    }
                    return Ok(CallbackAction::NoUpdate);
                }

                match ch {
                    'w' => self.start_command(Box::<commands::TextWindow>::default()),
                    'v' => self.start_command(Box::<commands::ViewPort>::default()),
                    '*' => return self.push_command(buf, caret, Box::<commands::ResetWindows>::default()),
                    'e' => return self.push_command(buf, caret, Box::<commands::EraseWindow>::default()),
                    'E' => return self.push_command(buf, caret, Box::<commands::EraseView>::default()),
                    'g' => self.start_command(Box::<commands::GotoXY>::default()),
                    'H' => return self.push_command(buf, caret, Box::<commands::Home>::default()),
                    '>' => return self.push_command(buf, caret, Box::<commands::EraseEOL>::default()),
                    'c' => self.start_command(Box::<commands::Color>::default()),
                    'Q' => self.start_command(Box::<commands::SetPalette>::default()),
                    'a' => self.start_command(Box::<commands::OnePalette>::default()),
                    'W' => self.start_command(Box::<commands::WriteMode>::default()),
                    'm' => self.start_command(Box::<commands::Move>::default()),
                    'T' => self.start_command(Box::<commands::Text>::default()),
                    '@' => self.start_command(Box::<commands::TextXY>::default()),
                    'Y' => self.start_command(Box::<commands::FontStyle>::default()),
                    'X' => self.start_command(Box::<commands::Pixel>::default()),
                    'L' => self.start_command(Box::<commands::Line>::default()),
                    'R' => self.start_command(Box::<commands::Rectangle>::default()),
                    'B' => self.start_command(Box::<commands::Bar>::default()),
                    'C' => self.start_command(Box::<commands::Circle>::default()),
                    'O' => self.start_command(Box::<commands::Oval>::default()),
                    'o' => self.start_command(Box::<commands::FilledOval>::default()),
                    'A' => self.start_command(Box::<commands::Arc>::default()),
                    'V' => self.start_command(Box::<commands::OvalArc>::default()),
                    'I' => self.start_command(Box::<commands::PieSlice>::default()),
                    'i' => self.start_command(Box::<commands::OvalPieSlice>::default()),
                    'Z' => self.start_command(Box::<commands::Bezier>::default()),
                    'P' => self.start_command(Box::<commands::Polygon>::default()),
                    'p' => self.start_command(Box::<commands::FilledPolygon>::default()),
                    'l' => self.start_command(Box::<commands::PolyLine>::default()),
                    'F' => self.start_command(Box::<commands::Fill>::default()),
                    '=' => self.start_command(Box::<commands::LineStyle>::default()),
                    'S' => self.start_command(Box::<commands::FillStyle>::default()),
                    's' => self.start_command(Box::<commands::FillPattern>::default()),
                    '1' => {
                        self.state = State::ReadCommand(1);
                        return Ok(CallbackAction::NoUpdate);
                    }
                    '9' => {
                        self.state = State::ReadCommand(9);
                        return Ok(CallbackAction::NoUpdate);
                    }
                    '$' => self.start_command(Box::<commands::TextVariable>::default()),
                    '#' => {
                        // RIP_NO_MORE
                        self.state = State::EndRip;
                        return Ok(CallbackAction::NoUpdate);
                    }
                    _ => {
                        self.state = State::Default;
                        if self.bgi.suspend_text {
                            return Ok(CallbackAction::NoUpdate);
                        }
                        self.fallback_parser.print_char(buf, current_layer, caret, '!')?;
                        self.fallback_parser.print_char(buf, current_layer, caret, '|')?;
                        return self.fallback_parser.print_char(buf, current_layer, caret, ch);
                    }
                }
                return Ok(CallbackAction::NoUpdate);
            }
            State::GotRipStart => {
                // got !
                if ch == '!' {
                    return Ok(CallbackAction::NoUpdate);
                }
                if ch == '\n' || ch == '\r' {
                    return Ok(CallbackAction::Update);
                }
                if ch != '|' {
                    self.state = State::Default;
                    if self.bgi.suspend_text {
                        return Ok(CallbackAction::NoUpdate);
                    }

                    self.fallback_parser.print_char(buf, current_layer, caret, '!')?;
                    return self.fallback_parser.print_char(buf, current_layer, caret, ch);
                }
                self.state = State::ReadCommand(0);
                return Ok(CallbackAction::NoUpdate);
            }
            State::Default => {
                match self.fallback_parser.state {
                    EngineState::ReadCSISequence(_) => {
                        if let '!' = ch {
                            // Select Graphic Rendition
                            self.fallback_parser.state = EngineState::Default;
                            if self.fallback_parser.parsed_numbers.is_empty() {
                                return Ok(CallbackAction::SendString(RIP_TERMINAL_ID.to_string()));
                            }

                            match self.fallback_parser.parsed_numbers.first() {
                                Some(0) => {
                                    return Ok(CallbackAction::SendString(RIP_TERMINAL_ID.to_string()));
                                }
                                Some(1) => {
                                    self.enable_rip = false;
                                }
                                Some(2) => {
                                    self.enable_rip = true;
                                }
                                _ => {
                                    return Err(ParserError::InvalidRipAnsiQuery(self.fallback_parser.parsed_numbers[0]).into());
                                }
                            }
                            return Ok(CallbackAction::NoUpdate);
                        }
                    }
                    EngineState::Default => {
                        if !self.enable_rip {
                            return self.fallback_parser.print_char(buf, current_layer, caret, ch);
                        }

                        if let '!' = ch {
                            self.state = State::GotRipStart;
                            return Ok(CallbackAction::NoUpdate);
                        }
                    }
                    _ => {}
                }
            }
        }
        if self.bgi.suspend_text {
            return Ok(CallbackAction::NoUpdate);
        }
        self.fallback_parser.print_char(buf, current_layer, caret, ch)
    }

    fn get_mouse_fields(&self) -> Vec<MouseField> {
        self.bgi.get_mouse_fields()
    }

    fn get_picture_data(&mut self) -> Option<(Size, Vec<u8>)> {
        if self.last_rip_update == self.rip_counter {
            return None;
        }
        self.last_rip_update = self.rip_counter;
        let mut pixels = Vec::new();
        let pal = self.bgi.get_palette().clone();
        for i in &self.bgi.screen {
            if *i == 0 {
                pixels.push(0);
                pixels.push(0);
                pixels.push(0);
                pixels.push(0);
                continue;
            }
            let (r, g, b) = pal.get_rgb(*i as u32);
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            pixels.push(255);
        }
        Some((self.bgi.window, pixels))
    }
}

fn to_base_36(len: usize, number: i32) -> String {
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

fn parse_base_36(number: &mut i32, ch: char) -> EngineResult<()> {
    if let Some(digit) = ch.to_digit(36) {
        *number = *number * 36 + digit as i32;
        Ok(())
    } else {
        Err(anyhow::Error::msg("Invalid base 36 digit"))
    }
}
