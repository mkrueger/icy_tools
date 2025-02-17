use std::time::Duration;

use crate::{Buffer, CallbackAction, Caret, EngineResult};

use super::{cmd::IgsCommands, DrawExecutor};

pub type LoopParameters = Vec<Vec<String>>;

pub fn count_params(params: &LoopParameters) -> i32 {
    params.iter().map(|x| x.len() as i32).sum()
}

pub struct Loop {
    i: i32,
    from: i32,
    to: i32,
    step: i32,
    delay: i32,
    cur_cmd: usize,
    command: Vec<IgsCommands>,
    parsed_string: String,
    parameters: LoopParameters,
}

impl Loop {
    pub fn new(from: i32, to: i32, step: i32, delay: i32, cmd_str: String, parsed_string: String, loop_parameters: LoopParameters) -> EngineResult<Self> {
        let mut command = Vec::new();
        for ch in cmd_str.chars() {
            command.push(IgsCommands::from_char(ch)?);
        }
        Ok(Self {
            i: from,
            from,
            to,
            step,
            delay,
            cur_cmd: 0,
            command,
            parsed_string,
            parameters: loop_parameters,
        })
    }

    pub fn next_step(&mut self, exe: &mut DrawExecutor, buf: &mut Buffer, caret: &mut Caret) -> Option<EngineResult<CallbackAction>> {
        let is_running = if self.from < self.to { self.i < self.to } else { self.i > self.to };
        if !is_running {
            return None;
        }
        let mut parameters = Vec::new();
        for p in &self.parameters[self.cur_cmd % self.parameters.len()] {
            let mut p = p.clone();
            let mut add_step_value = false;
            let mut subtract_const_value = false;
            let mut subtract_x_step = false;
            if p.starts_with('+') {
                add_step_value = true;
                p.remove(0);
            } else if p.starts_with('-') {
                subtract_const_value = true;
                p.remove(0);
            } else if p.starts_with('!') {
                subtract_x_step = true;
                p.remove(0);
            }

            let x = (self.i).abs();
            let y = (self.to - 1 - self.i).abs();
            let mut value = if p == "x" {
                x
            } else if p == "y" {
                y
            } else {
                match p.parse::<i32>() {
                    Err(_) => {
                        continue;
                    }
                    Ok(i) => i,
                }
            };

            if add_step_value {
                value += x;
            }
            if subtract_const_value {
                value = x - value;
            }
            if subtract_x_step {
                value -= x;
            }
            parameters.push(value);
        }

        // println!("step: {:?} => {:?}", self.loop_parameters[cur_parameter], parameters);
        let res = exe.execute_command(buf, caret, self.command[self.cur_cmd], &parameters, &self.parsed_string);
        // todo: correct delay?
        std::thread::sleep(Duration::from_millis(200 * self.delay as u64));
        self.cur_cmd += 1;
        if self.cur_cmd >= self.command.len() {
            self.cur_cmd = 0;
            if self.from < self.to {
                self.i += self.step;
            } else {
                self.i -= self.step;
            }
        }
        match res {
            Ok(r) => Some(Ok(r)),
            Err(err) => Some(Err(err)),
        }
    }
}
