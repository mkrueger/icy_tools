use crate::{CommandSink, IgsCommand, IgsCommandType, IgsParameter, ParameterBounds, PauseType};

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
/// `Number(10)`, `Number(20)`, `Number(StepForward)`, `Number(StepReverse)`.
#[derive(Debug, Clone, PartialEq)]
pub enum LoopParamToken {
    /// Constant numeric value or loop variable (x, y, r, R).
    ///
    /// Example: In `G#&>0,10,1,0,L,4,5,10,x,y:`, the `5` and `10` are constants,
    /// `x` is `IgsParameter::StepForward`, `y` is `IgsParameter::StepReverse`.
    /// Can also be `IgsParameter::Random` for random values in loops.
    Number(IgsParameter),

    /// Arithmetic expression combining a parameter with a constant.
    ///
    /// Examples:
    /// - `Expr(Add, Value(5))` for `+5`: current step + 5
    /// - `Expr(Subtract, Value(10))` for `-10`: current step - 10
    /// - `Expr(SubtractStep, Value(99))` for `!99`: 99 - current step
    /// - `Expr(Subtract, StepForward)` for `-x`: 0 - x
    ///
    /// Useful for offset coordinates or other computed values.
    /// The parameter can be a value, random, or even a step variable (x, y).
    Expr(ParamOperator, IgsParameter),

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
/// text_cursor = 0
/// ring_cursor = 0
/// while (from <= to ? current <= to : current >= to):
///     collect param_count values from params ring (wrapping at end)
///     compute parameters (x→current, y→opposite, r→random, etc.)
///     if W@ (WriteText with refresh): use next text from text_tokens ring
///     execute target_command with these parameters
///     delay for DELAY * (1/200) seconds
///     current += (from <= to ? step : -step)
/// ```
///
/// **Text Refresh (`W@`)**: When `refresh_text_each_iteration` is true and the
/// target is `WriteText`, the loop cycles through all `Text` tokens found in
/// the parameter stream. Example:
/// ```text
/// G#&>20,140,20,0,W@,2,0,x,First@Second@Third@
/// ```
/// This displays "First" at y=20, "Second" at y=40, "Third" at y=60, etc.
///
/// **Parameter Ring**: All parameters (excluding `GroupSeparator` and `Text`)
/// form a ring buffer. The loop consumes `param_count` values per iteration,
/// wrapping back to the start when reaching the end. This matches IG 2.17's
/// `lp_eff_ct` behavior.
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

    pub fn run(&self, sink: &mut dyn CommandSink, bounds: &ParameterBounds) {
        if self.step == 0 {
            return;
        }
        let forward = self.from <= self.to;
        let step_value = if forward { self.step.abs() } else { -self.step.abs() };

        let mut current = self.from;

        // Parameter ring cursor - persists across iterations like IG 2.17's lp_eff_ct
        let mut ring_cursor = 0usize;
        let total_params = self.params.len();

        if total_params == 0 {
            return; // No parameters to process
        }

        // Extract all text tokens for W@ (WriteText with refresh) commands
        let text_tokens: Vec<&Vec<u8>> = self
            .params
            .iter()
            .filter_map(|token| if let LoopParamToken::Text(text) = token { Some(text) } else { None })
            .collect();

        let mut text_cursor = 0usize;

        // Enable XOR mode if modifier is set
        if self.modifiers.xor_stepping {
            sink.begin_igs_xor_mode();
        }

        loop {
            // Check loop termination
            if forward && current > self.to {
                break;
            }
            if !forward && current < self.to {
                break;
            }

            // Evaluate ALL param_count parameters for this iteration
            // The command may be executed multiple times per iteration if param_count
            // is larger than what the command needs
            let reverse_value = self.to + self.from - current; // y = from + to - x

            // Collect ALL param_count values from the parameter ring
            let mut all_params = Vec::new();
            let mut params_collected = 0;

            while params_collected < self.param_count as usize {
                let token: &LoopParamToken = &self.params[ring_cursor];
                ring_cursor = (ring_cursor + 1) % total_params;

                let value = match token {
                    LoopParamToken::Number(param) => param.evaluate(bounds, current, reverse_value),
                    LoopParamToken::Expr(op, param) => {
                        let constant = param.evaluate(bounds, current, reverse_value);
                        match op {
                            ParamOperator::Add => current + constant,
                            ParamOperator::Subtract => current - constant,
                            ParamOperator::SubtractStep => constant - current,
                        }
                    }
                    LoopParamToken::GroupSeparator => {
                        // Skip group separators, they don't count towards param_count
                        continue;
                    }
                    LoopParamToken::Text(_) => {
                        // Text tokens handled separately for W@ commands - skip them here
                        continue;
                    }
                };

                all_params.push(value);
                params_collected += 1;
            }

            // Now execute the command as many times as needed based on param requirements
            // For example, Circle needs 3 params, so if param_count=12, execute 4 times
            match &self.target {
                LoopTarget::Single(cmd_type) => {
                    // Special handling for WriteText with refresh modifier
                    if *cmd_type == IgsCommandType::WriteText && self.modifiers.refresh_text_each_iteration {
                        // W@ command: use x,y from params and cycle through text tokens
                        if all_params.len() >= 2 && !text_tokens.is_empty() {
                            let text = text_tokens[text_cursor % text_tokens.len()].clone();
                            text_cursor += 1;

                            sink.emit_igs(IgsCommand::WriteText {
                                x: all_params[0].into(),
                                y: all_params[1].into(),
                                text,
                            });
                        }
                    } else {
                        // Use first parameter to determine parameter count for variable commands
                        let first_param = all_params.first().copied().unwrap_or(0);
                        let params_per_command = cmd_type.get_parameter_count(first_param);
                        if params_per_command > 0 {
                            let mut offset = 0;
                            while offset + params_per_command <= all_params.len() {
                                let cmd_params = &all_params[offset..offset + params_per_command];

                                // Convert i32 params to IgsParameter
                                let igs_params: Vec<IgsParameter> = cmd_params.iter().map(|&v| IgsParameter::Value(v)).collect();
                                if let Some(cmd) = cmd_type.create_command(sink, &igs_params, &[]) {
                                    sink.emit_igs(cmd);
                                }
                                offset += params_per_command;
                            }
                        }
                    }
                }
                LoopTarget::ChainGang { commands } => {
                    // Use first parameter to index into chain
                    if !all_params.is_empty() {
                        let index: usize = (all_params[0].abs() as usize) % commands.len();
                        let cmd_type = commands[index];

                        // For chain gang, skip the index parameter and use remaining params
                        let remaining_params = &all_params[1..];
                        if !remaining_params.is_empty() {
                            let first_param = remaining_params[0];
                            let params_per_command = cmd_type.get_parameter_count(first_param);

                            if params_per_command <= remaining_params.len() {
                                // Convert i32 params to IgsParameter
                                let igs_params: Vec<IgsParameter> = remaining_params[..params_per_command].iter().map(|&v| IgsParameter::Value(v)).collect();

                                if let Some(cmd) = cmd_type.create_command(sink, &igs_params, &[]) {
                                    sink.emit_igs(cmd);
                                }
                            }
                        }
                    }
                }
            }

            // Apply delay if specified (delay is in 1/200 seconds)
            // IGS spec: "4th parameter = DELAY in 200 hundredths of a between each step."
            // Convert to milliseconds: delay * 1000 / 200 = delay * 5
            if self.delay > 0 {
                let delay_ms = (self.delay as u32) * 5;
                sink.emit_igs(IgsCommand::Pause {
                    pause_type: PauseType::MilliSeconds(delay_ms),
                });
            }

            // Advance to next iteration
            current += step_value;
        }

        // Disable XOR mode if it was enabled
        if self.modifiers.xor_stepping {
            sink.end_igs_xor_mode();
        }
    }
}
