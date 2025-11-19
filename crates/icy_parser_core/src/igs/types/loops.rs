use crate::{CommandSink, IgsCommand, IgsCommandType};

/// Describes what command the loop executes each iteration.
///
/// This maps to the 5th parameter of the IGS `&` loop command.
/// According to the spec:
///
/// - Either a single command identifier like `L`, `G`, `W`, etc.
/// - Or a "Chain Gang" specification using `>...@`, where the position
///   of the command in the chain (0..127) is selected by a stepping parameter.
#[derive(Debug, Clone, PartialEq)]
pub enum LoopTarget {
    /// Single command identifier executed each iteration.
    ///
    /// Example: `G#&>0,100,1,0,L,2,0,0,x,y:` loops the `L` (line) command
    /// with parameters stepping from 0 to 100.
    Single(IgsCommandType),

    /// Chain-Gang: a sequence of commands indexed by a loop parameter.
    ///
    /// The spec allows `>...@` as parameter 5, where each position in the
    /// string (0 to 127) represents a command. The loop parameter `x` or `y`
    /// selects which position in the chain to execute.
    ///
    /// Example: `G#&>0,3,1,0,>CL@,6,0,0,x,y:` executes:
    ///   - iteration 0: `C` (color set)
    ///   - iteration 1: `L` (line)
    ///   - iteration 2: `C` again
    ///   - iteration 3: `L` again
    ChainGang {
        /// Command identifiers extracted from the chain.
        commands: Vec<IgsCommandType>,
    },
}

/// Optional modifiers that customize loop behavior.
///
/// These flags are parsed from suffixes on the command identifier in the `&` header,
/// e.g., `G#&>...,W|@,...` where `|` enables XOR stepping and `@` enables refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoopModifiers {
    /// XOR stepping: combines loop iterations using XOR drawing mode.
    ///
    /// The spec mentions: "XOR stepping example: G#G 1,3,0,0,50,50:
    /// G#&>198,0,2,0,G|4,2,6,x,x:" This allows overlaying shapes
    /// in XOR mode for animation effects.
    pub xor_stepping: bool,

    /// For `W` (write text) command: re-read text from stream each iteration.
    ///
    /// Without this flag, the last `W` text is reused. With the flag,
    /// IG reads a new text value from the parameter stream for each iteration.
    /// Useful for building dynamic text lists in loops.
    pub refresh_text_each_iteration: bool,
}

/// Binary operator for arithmetic loop parameter expressions.
///
/// The IGS spec allows loop parameters to be computed expressions:
/// - `+N` adds N to the current step value
/// - `-N` subtracts N from the current step value
/// - `!N` subtracts the step value from N (reverse subtraction)
///
/// Example: `G#&>0,10,1,0,L,4,x,y,+5,x:` uses `x` (step from 0 to 10)
/// and `+5` (step + 5) as parameters to the line command.
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ParamOperator {
    /// `+CONST` – add the constant to the current step value.
    Add = b'+',
    /// `-CONST` – subtract the constant from the current step value.
    Subtract = b'-',
    /// `!CONST` – subtract the step value from the constant (reverse subtraction).
    SubtractStep = b'!',
}

/// A single parameter value or placeholder in a loop's READ/DATA-like section.
///
/// The spec describes the loop as having 6 fixed parameters, then a stream
/// of parameter values for the target command. These tokens represent what
/// gets passed to the target command each iteration.
///
/// Example: `G#&>0,100,10,0,L,4,10,20,x,y:` has param_count=4 and four tokens:
/// `Number(10)`, `Number(20)`, `StepForward`, `StepReverse`.
#[derive(Debug, Clone, PartialEq)]
pub enum LoopParamToken {
    /// Constant numeric value that never changes across iterations.
    ///
    /// Example: In `G#&>0,10,1,0,L,4,5,10,x,y:`, the `5` and `10` are constants.
    Number(i32),

    /// `x` – stepping variable: varies from `from` to `to` in `step` increments.
    ///
    /// From the spec: "if you use a `x` as a parameter it will be stepped
    /// in the direction of the FROM TO values". In the loop
    /// `G#&>10,20,2,0,L,4,x,y,100,200:`, the first parameter (x) steps
    /// as 10, 12, 14, 16, 18, 20.
    StepForward,

    /// `y` – reverse stepping variable: varies opposite to the FROM→TO direction.
    ///
    /// From the spec: "if you use a `y` the loop will step the value in a
    /// reverse direction". In `G#&>10,20,2,0,L,4,x,y,100,200:`, the second
    /// parameter (y) steps as 20, 18, 16, 14, 12, 10.
    StepReverse,

    /// `r` – random value within the range set by the `X 2` command.
    ///
    /// The spec describes: `X 2,0,50:` sets random range to 0–50.
    /// Using `r` in a loop then produces random values in that range
    /// for each iteration.
    Random,

    /// Arithmetic expression combining a step variable with a constant.
    ///
    /// Examples:
    /// - `Expr(Add, 5)` for `+5`: current step + 5
    /// - `Expr(Subtract, 10)` for `-10`: current step - 10
    /// - `Expr(SubtractStep, 99)` for `!99`: 99 - current step
    ///
    /// Useful for offset coordinates or other computed values.
    Expr(ParamOperator, i32),

    /// Group separator `:` in the parameter stream.
    ///
    /// The spec uses `:` to logically separate parameter groups,
    /// similar to BASIC's `READ`/`DATA` structure. Useful for readability
    /// and future extensions but typically ignored during execution.
    GroupSeparator,

    /// Text string for `W@` (write text with refresh) command.
    ///
    /// When the loop target is `W` with the `@` modifier (refresh_text_each_iteration),
    /// the parameter stream contains text strings terminated by `@` that are cycled
    /// through on each iteration.
    ///
    /// Example: `G#&>20,140,20,0,W@,2,0,x,A. Item 1@B. Item 2@C. Item 3@`
    /// The texts "A. Item 1", "B. Item 2", "C. Item 3" are stored as `Text` tokens
    /// and displayed sequentially as the loop progresses.
    Text(Vec<u8>),
}

/// Complete parsed representation of an IGS `G#&` loop command.
///
/// The `&` loop command allows executing a target command repeatedly with
/// stepping parameters. The spec defines it as:
///
/// ```text
/// G#&>FROM,TO,STEP,DELAY,TARGET_COMMAND,PARAM_COUNT,<param values...>
/// ```
///
/// Where:
/// - **Parameter 1 (FROM)**: Start value for stepping (inclusive).
/// - **Parameter 2 (TO)**: End value for stepping (inclusive).
/// - **Parameter 3 (STEP)**: Step size (always positive; direction from FROM/TO).
/// - **Parameter 4 (DELAY)**: Pause between iterations in 1/200 seconds.
/// - **Parameter 5 (TARGET_COMMAND)**: Either a single command or a chain gang.
/// - **Parameter 6 (PARAM_COUNT)**: Number of parameter values for the target.
/// - **Remaining**: The parameter stream (READ/DATA-like section).
///
/// The loop executes roughly as:
/// ```pseudocode
/// current = from
/// while (from <= to ? current <= to : current >= to):
///     compute parameters from the stream (x→current, y→opposite, etc.)
///     execute target_command with these parameters
///     delay for DELAY * (1/200) seconds
///     current += (from <= to ? step : -step)
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct LoopCommandData {
    /// Start value for the stepping variable (inclusive).
    pub from: i32,

    /// End value for the stepping variable (inclusive).
    pub to: i32,

    /// Step increment (always positive; direction determined by from/to relation).
    ///
    /// From spec: "3rd parameter = step value, positive number only."
    /// If from < to, the step value is added each iteration (forward).
    /// If from > to, the step value is subtracted each iteration (backward).
    pub step: i32,

    /// Delay between iterations in units of 1/200 seconds.
    ///
    /// From spec: "4th parameter = DELAY in 200 hundredths of a between each step."
    /// A delay of 200 pauses for 1 second.
    pub delay: i32,

    /// Command or chain of commands to execute each iteration.
    pub target: LoopTarget,

    /// Modifiers affecting loop behavior (XOR stepping, text refresh, etc.).
    pub modifiers: LoopModifiers,

    /// Number of parameter values expected for the target command.
    ///
    /// From spec: "6th parameter = number of parameters command that [the loop]
    /// requires". If the target command needs 4 parameters, param_count=4.
    /// The loop will consume that many tokens from the `params` vector per iteration.
    pub param_count: u16,

    /// The parameter stream: stepping variables, constants, expressions, etc.
    ///
    /// Each iteration, the loop extracts param_count tokens, evaluates them
    /// (replacing `x`/`y` with the current step value), and passes them
    /// to the target command.
    pub params: Vec<LoopParamToken>,
}

impl LoopCommandData {
    pub fn iteration_count(&self) -> u32 {
        if self.step == 0 {
            return 0;
        }
        ((self.to - self.from).abs() / self.step.abs() + 1) as u32
    }

    pub fn is_reverse(&self) -> bool {
        (self.to < self.from && self.step > 0) || (self.to > self.from && self.step < 0)
    }

    pub fn run(&self, sink: &mut dyn CommandSink, rnd_min: i32, rnd_max: i32) {
        if self.step == 0 {
            return;
        }

        let forward = self.from <= self.to;
        let step_value = if forward { self.step.abs() } else { -self.step.abs() };

        let mut current = self.from;
        let mut param_index = 0;

        loop {
            // Check loop termination
            if forward && current > self.to {
                break;
            }
            if !forward && current < self.to {
                break;
            }

            // Collect parameters for this iteration
            let mut iteration_params = Vec::new();
            let reverse_value = self.to + self.from - current; // y = from + to - x

            for _ in 0..self.param_count {
                if param_index >= self.params.len() {
                    break;
                }

                let token = &self.params[param_index];
                param_index += 1;

                let value = match token {
                    LoopParamToken::Number(n) => *n,
                    LoopParamToken::StepForward => current,
                    LoopParamToken::StepReverse => reverse_value,
                    LoopParamToken::Random => {
                        // Use the midpoint of the random range as a placeholder
                        // In a real execution environment, this would generate an actual random value
                        rnd_min + (rnd_max - rnd_min) / 2
                    }
                    LoopParamToken::Expr(op, constant) => match op {
                        ParamOperator::Add => current + constant,
                        ParamOperator::Subtract => current - constant,
                        ParamOperator::SubtractStep => constant - current,
                    },
                    LoopParamToken::GroupSeparator => {
                        // Skip group separators, don't count toward param_count
                        continue;
                    }
                    LoopParamToken::Text(_) => {
                        // Text tokens are handled specially for W@ commands
                        // For now, skip in regular parameter collection
                        continue;
                    }
                };

                iteration_params.push(value);
            }

            // Build and emit the command based on target
            match &self.target {
                LoopTarget::Single(cmd_type) => {
                    if let Some(cmd) = Self::build_command(*cmd_type, &iteration_params) {
                        sink.emit_igs(cmd);
                    }
                }
                LoopTarget::ChainGang { commands } => {
                    // Use current value to index into the chain
                    let index = (current.abs() as usize) % commands.len();
                    let cmd_type = commands[index];

                    if let Some(cmd) = Self::build_command(cmd_type, &iteration_params) {
                        sink.emit_igs(cmd);
                    }
                }
            }

            // Apply delay if specified (delay is in 1/200 seconds)
            if self.delay > 0 {
                // In a real implementation, this would pause execution
                // For parsing/serialization, we just note the delay
                // The actual delay implementation would be in the rendering layer
            }

            // Advance to next iteration
            current += step_value;
        }
    }

    /// Helper method to construct an IgsCommand from a command type and parameters.
    /// This is a simplified implementation - a full implementation would need to
    /// match all command types and their specific parameter requirements.
    fn build_command(cmd_type: IgsCommandType, params: &[i32]) -> Option<IgsCommand> {
        use IgsCommandType::*;

        match cmd_type {
            Line => {
                if params.len() >= 4 {
                    Some(IgsCommand::Line {
                        x1: params[0].into(),
                        y1: params[1].into(),
                        x2: params[2].into(),
                        y2: params[3].into(),
                    })
                } else {
                    None
                }
            }
            LineDrawTo => {
                if params.len() >= 2 {
                    Some(IgsCommand::LineDrawTo {
                        x: params[0].into(),
                        y: params[1].into(),
                    })
                } else {
                    None
                }
            }
            Circle => {
                if params.len() >= 3 {
                    Some(IgsCommand::Circle {
                        x: params[0].into(),
                        y: params[1].into(),
                        radius: params[2].into(),
                    })
                } else {
                    None
                }
            }
            Box => {
                if params.len() >= 5 {
                    Some(IgsCommand::Box {
                        x1: params[0].into(),
                        y1: params[1].into(),
                        x2: params[2].into(),
                        y2: params[3].into(),
                        rounded: params[4] != 0,
                    })
                } else {
                    None
                }
            }
            PolyMarker => {
                if params.len() >= 2 {
                    Some(IgsCommand::PolymarkerPlot {
                        x: params[0].into(),
                        y: params[1].into(),
                    })
                } else {
                    None
                }
            }
            // Add more command types as needed
            // For now, return None for unimplemented types
            _ => None,
        }
    }
}
