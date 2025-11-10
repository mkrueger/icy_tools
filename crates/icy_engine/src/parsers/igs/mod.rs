use super::BufferParser;
use crate::{CallbackAction, EditableScreen, EngineResult, Position};

mod cmd;
use cmd::IgsCommands;

pub mod paint;
use igs_loop::{Loop, LoopParameters, count_params};
pub use paint::*;

pub mod patterns;
pub use patterns::*;

mod igs_loop;
mod sound;
mod vdi;

#[cfg(test)]
mod tests;
const IGS_VERSION: &str = "2.19";

#[derive(Default, Debug)]
enum State {
    #[default]
    Default,
    GotIgsStart,
    ReadCommandStart,
    SkipNewLine,
    ReadCommand(IgsCommands),

    // VT52
    EscapeSequence,
    // true == fg
    ReadColor(bool),
    //
    VT52SetCursorPos(i32),
}

#[derive(Default, Debug)]
enum LoopState {
    #[default]
    Start,
    ReadCommand,
    ReadCount,
    ReadParameter,
    ChainGangStart,
    EndChain,
}

pub struct Parser {
    state: State,
    parsed_numbers: Vec<i32>,

    parsed_string: String,

    loop_parameter_count: i32,
    loop_state: LoopState,
    loop_cmd: char,
    loop_parameters: LoopParameters,
    chain_gang: String,

    command_executor: DrawExecutor,
    got_double_colon: bool,
    cur_loop: Option<Loop>,
    saved_caret_pos: Position,
    wrap_text: bool,
}

impl Parser {
    pub fn new(resolution: TerminalResolution) -> Self {
        Self {
            state: State::Default,
            parsed_numbers: Vec::new(),
            command_executor: DrawExecutor::new(resolution),
            parsed_string: String::new(),
            loop_state: LoopState::Start,
            loop_parameters: Vec::new(),
            loop_cmd: ' ',
            got_double_colon: false,
            cur_loop: None,
            chain_gang: String::new(),
            loop_parameter_count: 0,
            saved_caret_pos: Position::default(),
            wrap_text: false,
        }
    }

    fn write_char(&mut self, buf: &mut dyn EditableScreen, ch: char) -> EngineResult<CallbackAction> {
        let caret_pos = buf.caret().position();

        let p = Position::new(caret_pos.x * 8, caret_pos.y * 8);
        self.command_executor.fill_color = buf.caret().attribute.background_color as u8;
        self.command_executor.fill_rect(buf, p.x, p.y, p.x + 8, p.y + 8);

        self.command_executor.text_color = buf.caret().attribute.foreground_color as u8;
        self.command_executor.write_text(buf, p, &ch.to_string());

        buf.caret_mut().x = caret_pos.x + 1;

        Ok(CallbackAction::Update)
    }
}

impl BufferParser for Parser {
    fn get_next_action(&mut self, buffer: &mut dyn EditableScreen) -> Option<CallbackAction> {
        if let Some(l) = &mut self.cur_loop {
            if let Some(x) = l.next_step(&mut self.command_executor, buffer) {
                if let Ok(act) = x {
                    return Some(act);
                }
                return None;
            }
            self.cur_loop = None;
        }
        None
    }

    fn print_char(&mut self, buf: &mut dyn EditableScreen, ch: char) -> EngineResult<CallbackAction> {
        match &self.state {
            State::ReadCommand(command) => {
                if *command == IgsCommands::WriteText && self.parsed_numbers.len() >= 3 {
                    if ch == '@' {
                        let parameters: Vec<_> = self.parsed_numbers.drain(..).collect();
                        let res = self.command_executor.execute_command(buf, *command, &parameters, &self.parsed_string);
                        self.state = State::ReadCommandStart;
                        self.parsed_string.clear();
                        return res;
                    }
                    self.parsed_string.push(ch);
                    if ch == '\n' {
                        self.parsed_string.clear();
                        self.state = State::ReadCommandStart;
                        return Ok(CallbackAction::NoUpdate);
                    }
                    return Ok(CallbackAction::NoUpdate);
                }
                if *command == IgsCommands::LoopCommand && self.parsed_numbers.len() >= 4 {
                    match self.loop_state {
                        LoopState::Start => {
                            if ch == ',' {
                                self.loop_state = LoopState::ReadCommand;
                            }
                        }
                        LoopState::ChainGangStart => {
                            if ch == '@' {
                                self.loop_state = LoopState::EndChain;
                            } else {
                                self.chain_gang.push(ch);
                            }
                        }
                        LoopState::EndChain => {
                            if ch == ',' {
                                self.loop_state = LoopState::ReadCount;
                            }
                        }
                        LoopState::ReadCommand => {
                            if ch == '>' {
                                self.loop_state = LoopState::ChainGangStart;
                            } else if ch == '@' || ch == '|' || ch == ',' {
                                self.loop_state = LoopState::ReadCount;
                                self.parsed_string.clear();
                            } else {
                                self.loop_cmd = ch;
                            }
                        }
                        LoopState::ReadCount => match ch {
                            '0'..='9' => {
                                self.loop_parameter_count = parse_next_number(self.loop_parameter_count, ch as u8);
                            }
                            ',' => {
                                self.loop_parameters.clear();
                                self.loop_parameters.push(vec![String::new()]);
                                self.got_double_colon = false;
                                self.loop_state = LoopState::ReadParameter;
                            }
                            _ => {
                                self.state = State::Default;
                            }
                        },
                        LoopState::ReadParameter => match ch {
                            '_' | '\n' | '\r' => { /* ignore */ }
                            ',' => {
                                if self.loop_parameter_count <= count_params(&self.loop_parameters) {
                                    self.state = State::ReadCommandStart;

                                    let mut l = Loop::new(
                                        self.parsed_numbers[0],
                                        self.parsed_numbers[1],
                                        self.parsed_numbers[2],
                                        self.parsed_numbers[3],
                                        if self.chain_gang.is_empty() {
                                            self.loop_cmd.to_string()
                                        } else {
                                            self.chain_gang.clone()
                                        },
                                        self.parsed_string.clone(),
                                        self.loop_parameters.clone(),
                                    )?;

                                    if let Some(x) = l.next_step(&mut self.command_executor, buf) {
                                        self.cur_loop = Some(l);
                                        return x;
                                    }
                                    return Ok(CallbackAction::Update);
                                }
                                self.loop_parameters.last_mut().unwrap().push(String::new());
                            }
                            ':' => {
                                if self.loop_parameter_count <= count_params(&self.loop_parameters) {
                                    self.state = State::ReadCommandStart;
                                    let mut l = Loop::new(
                                        self.parsed_numbers[0],
                                        self.parsed_numbers[1],
                                        self.parsed_numbers[2],
                                        self.parsed_numbers[3],
                                        if self.chain_gang.is_empty() {
                                            self.loop_cmd.to_string()
                                        } else {
                                            self.chain_gang.clone()
                                        },
                                        self.parsed_string.clone(),
                                        self.loop_parameters.clone(),
                                    )?;

                                    if let Some(x) = l.next_step(&mut self.command_executor, buf) {
                                        self.cur_loop = Some(l);
                                        return x;
                                    }
                                    return Ok(CallbackAction::Update);
                                }
                                self.loop_parameters.last_mut().unwrap().push(String::new());
                            }
                            _ => {
                                if let Some((pos, _)) = self.chain_gang.chars().enumerate().find(|(_i, x)| *x == ch) {
                                    let is_next_chain = if let Some(p) = self.loop_parameters.last() {
                                        if let Some(last_par) = p.last() { *last_par == pos.to_string() } else { false }
                                    } else {
                                        false
                                    };
                                    if is_next_chain {
                                        self.loop_parameter_count -= 1;
                                        let _n = self.loop_parameters.last_mut().unwrap().pop();
                                        //self.loop_parameters.push(vec![_n.unwrap()]);
                                        if self.loop_parameters.len() > 1 || !self.loop_parameters.last().unwrap().is_empty() {
                                            self.loop_parameters.push(vec![String::new()]);
                                        }
                                        return Ok(CallbackAction::NoUpdate);
                                    }
                                }
                                if let Some(p) = self.loop_parameters.last_mut() {
                                    if let Some(last_par) = p.last_mut() {
                                        last_par.push(ch);
                                    } else {
                                        p.push(ch.to_string());
                                    }
                                }
                            }
                        },
                    }
                    return Ok(CallbackAction::NoUpdate);
                }
                match ch {
                    ' ' | '>' | '\r' => { /* ignore */ }
                    '_' => {
                        self.got_double_colon = false;
                    }
                    '\n' => {
                        if self.got_double_colon {
                            self.got_double_colon = false;
                            self.state = State::SkipNewLine;
                        }
                    }
                    '0'..='9' => {
                        self.got_double_colon = false;
                        let d = match self.parsed_numbers.pop() {
                            Some(number) => number,
                            _ => 0,
                        };
                        self.parsed_numbers.push(parse_next_number(d, ch as u8));
                    }
                    ',' => {
                        self.got_double_colon = false;
                        self.parsed_numbers.push(0);
                    }
                    ':' => {
                        // workaround for polyline bug.
                        if *command == IgsCommands::PolyLine && self.parsed_numbers.len() == 1 {
                            self.got_double_colon = false;
                            self.parsed_numbers.push(0);
                            return Ok(CallbackAction::NoUpdate);
                        }
                        self.got_double_colon = true;
                        let parameters: Vec<_> = self.parsed_numbers.drain(..).collect();
                        let res = self.command_executor.execute_command(buf, *command, &parameters, &self.parsed_string);
                        self.state = State::ReadCommandStart;
                        return res;
                    }
                    _ => {
                        self.got_double_colon = false;
                        self.state = State::Default;
                    }
                }
                Ok(CallbackAction::NoUpdate)
            }
            State::ReadCommandStart => {
                self.parsed_numbers.clear();
                match ch {
                    '\r' => Ok(CallbackAction::NoUpdate),
                    '\n' => {
                        self.state = State::SkipNewLine;
                        Ok(CallbackAction::NoUpdate)
                    }

                    '&' => {
                        self.state = State::ReadCommand(IgsCommands::LoopCommand);
                        self.loop_parameter_count = 0;
                        self.chain_gang.clear();
                        self.loop_state = LoopState::Start;
                        Ok(CallbackAction::NoUpdate)
                    }

                    _ => match IgsCommands::from_char(ch) {
                        Ok(cmd) => {
                            self.state = State::ReadCommand(cmd);
                            Ok(CallbackAction::NoUpdate)
                        }
                        Err(err) => {
                            self.state = State::Default;
                            Err(anyhow::anyhow!("{err}"))
                        }
                    },
                }
            }
            State::GotIgsStart => {
                if ch == '#' {
                    self.state = State::ReadCommandStart;
                    return Ok(CallbackAction::NoUpdate);
                }
                self.state = State::Default;
                let _ = self.write_char(buf, 'G');
                self.write_char(buf, ch)
            }
            State::SkipNewLine => {
                self.state = State::Default;
                if ch == '\r' {
                    return Ok(CallbackAction::NoUpdate);
                }
                if ch == 'G' {
                    self.state = State::GotIgsStart;
                    return Ok(CallbackAction::NoUpdate);
                }
                self.write_char(buf, ch)
            }

            State::VT52SetCursorPos(x_pos) => {
                let pos = (ch as u8) - b' ';
                if *x_pos < 0 {
                    State::VT52SetCursorPos(pos as i32);
                    return Ok(CallbackAction::NoUpdate);
                }
                buf.caret_mut().set_position_xy(*x_pos, pos as i32);
                self.state = State::Default;
                Ok(CallbackAction::Update)
            }
            State::ReadColor(fg) => {
                let color = ((ch as u8) - b'0') as u32;
                if *fg {
                    buf.caret_mut().attribute.set_foreground(color);
                } else {
                    buf.caret_mut().attribute.set_background(color);
                }
                self.state = State::Default;
                Ok(CallbackAction::Update)
            }
            State::EscapeSequence => {
                match ch {
                    'A' => {
                        if buf.caret().y > 0 {
                            buf.caret_mut().y -= 1;
                        }
                    }
                    'B' => {
                        let size = buf.get_size();
                        if buf.caret().y < size.height {
                            let y = buf.caret().y;
                            buf.caret_mut().y = y + 1;
                        }
                    }
                    'C' => {
                        let size = buf.get_size();
                        if buf.caret().x < size.width {
                            let x = buf.caret_mut().x;
                            buf.caret_mut().x = x + 1;
                        }
                    }
                    'D' => {
                        if buf.caret().x > 0 {
                            let x = buf.caret().x;
                            buf.caret_mut().x = x - 1;
                        }
                    }
                    'E' => {
                        buf.clear_screen();
                    }
                    'F' => { // Enter graphics mode
                    }
                    'G' => { // Leave graphics mode
                    }
                    'H' => {
                        buf.caret_mut().set_position(Position::default());
                    }
                    'I' => {
                        if buf.caret().y > 0 {
                            buf.caret_mut().y -= 1;
                        } else {
                            self.command_executor.scroll(buf, -8);
                        }
                    }
                    'J' => {
                        // erase to end of screen
                        buf.clear_buffer_down();
                    }
                    'K' => {
                        // erase to end of line
                        buf.clear_line_end();
                    }
                    'Y' => {
                        self.state = State::VT52SetCursorPos(-1);
                        return Ok(CallbackAction::NoUpdate);
                    }
                    'Z' => { // Identify terminal
                    }
                    '[' => { // Enter hold-screen mode
                    }
                    '\\' => { // Exit hold screen mode
                    }
                    '=' => { // Alt keypad mode
                    }
                    '>' => { // Exit alt keypad mode
                    }
                    'b' => {
                        // FG Color mode
                        self.state = State::ReadColor(true);
                        return Ok(CallbackAction::NoUpdate);
                    }
                    'c' => {
                        // BG Color mode
                        self.state = State::ReadColor(false);
                        return Ok(CallbackAction::NoUpdate);
                    }
                    'd' => {
                        // Clear to start of screen
                        buf.clear_line_start();
                    }
                    'e' => {
                        // Enable cursor
                        buf.caret_mut().visible = true;
                    }
                    'f' => {
                        // Disable cursor
                        buf.caret_mut().visible = false;
                    }
                    'j' => {
                        // Save cursor pos
                        self.saved_caret_pos = buf.caret().position();
                    }
                    'k' => {
                        // Restore cursor pos
                        buf.caret_mut().set_position(self.saved_caret_pos);
                    }
                    'l' => {
                        // Clear line
                        buf.clear_line();
                        buf.caret_mut().x = 0;
                    }
                    'o' => {
                        // Clear to start of line
                        buf.clear_line_start();
                    }
                    'p' => { // Reverse video
                    }
                    'q' => { // Normal video
                    }
                    'v' => {
                        // Wrap on
                        self.wrap_text = true;
                    }
                    'w' => {
                        // Wrap off
                        self.wrap_text = false;
                    }
                    _ => {
                        // Ignore
                        log::info!("Ignoring VT-52 escape sequence: {}", ch);
                    }
                }
                self.state = State::Default;
                Ok(CallbackAction::Update)
            }
            State::Default => match ch as u8 {
                b'G' => {
                    self.state = State::GotIgsStart;
                    Ok(CallbackAction::NoUpdate)
                }
                0..=6 => Ok(CallbackAction::NoUpdate),
                0x07 => Ok(CallbackAction::Beep),
                0x0B | 0x0C => {
                    buf.bs();
                    Ok(CallbackAction::NoUpdate)
                }
                0x0D => {
                    buf.lf();
                    Ok(CallbackAction::NoUpdate)
                }
                0x0E..=0x1A => Ok(CallbackAction::NoUpdate),
                0x1B => {
                    self.state = State::EscapeSequence;
                    Ok(CallbackAction::NoUpdate)
                }
                0x1C..=0x1F => Ok(CallbackAction::NoUpdate),
                _ => self.write_char(buf, ch),
            },
        }
    }
}

pub fn parse_next_number(x: i32, ch: u8) -> i32 {
    x.saturating_mul(10).saturating_add(ch as i32).saturating_sub(b'0' as i32)
}
