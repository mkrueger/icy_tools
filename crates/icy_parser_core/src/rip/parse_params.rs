use super::*;

impl RipParser {
    pub fn parse_params(&mut self, ch: u8, sink: &mut dyn CommandSink) -> bool {
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
        if self.builder.got_escape {
            self.builder.got_escape = false;
        } else {
            if ch == b'|' {
                self.emit_command(sink);
                self.builder.reset();
                self.state = State::GotPipe;
                return true;
            }
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
            (0, b'T') | (0, b'$') | (1, b'R') => {
                self.builder.string_param.push(ch as char);
                Ok(false)
            }

            // FileQuery: mode(2) + res(4) then filename string
            (1, b'F') if self.builder.param_state < 6 => {
                let result = self
                    .builder
                    .parse_base36_complete(ch, self.builder.param_state / 2, if self.builder.param_state < 2 { 2 } else { 4 });
                match result {
                    Ok(_) => Ok(false),
                    Err(e) => Err(e),
                }
            }
            (1, b'F') => {
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

            // Button: states 0..11 (x0,y0,x1,y1,hotkey are 2-digit; flags,res are 1-digit), then text
            (1, b'U') => {
                if self.builder.param_state <= 11 {
                    if let Some(digit) = BASE36_LUT[ch as usize] {
                        match self.builder.param_state {
                            0..=1 => {
                                // x0 (2 digits)
                                if self.builder.u16_params.is_empty() {
                                    self.builder.u16_params.resize(7, 0);
                                }
                                self.builder.u16_params[0] = self.builder.u16_params[0].wrapping_mul(36).wrapping_add(digit);
                            }
                            2..=3 => {
                                // y0 (2 digits)
                                self.builder.u16_params[1] = self.builder.u16_params[1].wrapping_mul(36).wrapping_add(digit);
                            }
                            4..=5 => {
                                // x1 (2 digits)
                                self.builder.u16_params[2] = self.builder.u16_params[2].wrapping_mul(36).wrapping_add(digit);
                            }
                            6..=7 => {
                                // y1 (2 digits)
                                self.builder.u16_params[3] = self.builder.u16_params[3].wrapping_mul(36).wrapping_add(digit);
                            }
                            8..=9 => {
                                // hotkey (2 digits)
                                self.builder.u16_params[4] = self.builder.u16_params[4].wrapping_mul(36).wrapping_add(digit);
                            }
                            10 => {
                                // flags (1 digit)
                                self.builder.u16_params[5] = digit;
                            }
                            11 => {
                                // res (1 digit)
                                self.builder.u16_params[6] = digit;
                            }
                            _ => {}
                        }
                        self.builder.param_state += 1;
                        Ok(false)
                    } else {
                        Err(())
                    }
                } else {
                    // Text part: Don't add terminator characters to the text
                    // The main parse_params function handles | and \n terminators
                    self.builder.string_param.push(ch as char);
                    Ok(false)
                }
            }

            // Mouse: states 0..11 (6 two-digit + 2 one-digit), 12..16 (res 5 digits), then text
            (1, b'M') => {
                if self.builder.param_state <= 16 {
                    if let Some(digit) = BASE36_LUT[ch as usize] {
                        match self.builder.param_state {
                            0..=1 => {
                                // num
                                if self.builder.u16_params.is_empty() {
                                    self.builder.u16_params.resize(8, 0);
                                }
                                self.builder.u16_params[0] = self.builder.u16_params[0].wrapping_mul(36).wrapping_add(digit);
                            }
                            2..=3 => {
                                // x0
                                self.builder.u16_params[1] = self.builder.u16_params[1].wrapping_mul(36).wrapping_add(digit);
                            }
                            4..=5 => {
                                // y0
                                self.builder.u16_params[2] = self.builder.u16_params[2].wrapping_mul(36).wrapping_add(digit);
                            }
                            6..=7 => {
                                // x1
                                self.builder.u16_params[3] = self.builder.u16_params[3].wrapping_mul(36).wrapping_add(digit);
                            }
                            8..=9 => {
                                // y1
                                self.builder.u16_params[4] = self.builder.u16_params[4].wrapping_mul(36).wrapping_add(digit);
                            }
                            10 => {
                                // clk (1 digit)
                                self.builder.u16_params[5] = digit;
                            }
                            11 => {
                                // clr (1 digit)
                                self.builder.u16_params[6] = digit;
                            }
                            12..=16 => {
                                // res (5 digits)
                                self.builder.u16_params[7] = self.builder.u16_params[7].wrapping_mul(36).wrapping_add(digit);
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

            // LoadIcon: states 0..8 (x,y,mode are 2-digit; clipboard is 1-digit; res is 2-digit), then filename
            (1, b'I') => {
                if self.builder.param_state <= 8 {
                    if let Some(digit) = BASE36_LUT[ch as usize] {
                        match self.builder.param_state {
                            0..=1 => {
                                // x (2 digits)
                                if self.builder.u16_params.is_empty() {
                                    self.builder.u16_params.resize(5, 0);
                                }
                                self.builder.u16_params[0] = self.builder.u16_params[0].wrapping_mul(36).wrapping_add(digit);
                            }
                            2..=3 => {
                                // y (2 digits)
                                self.builder.u16_params[1] = self.builder.u16_params[1].wrapping_mul(36).wrapping_add(digit);
                            }
                            4..=5 => {
                                // mode (2 digits)
                                self.builder.u16_params[2] = self.builder.u16_params[2].wrapping_mul(36).wrapping_add(digit);
                            }
                            6 => {
                                // clipboard (1 digit)
                                self.builder.u16_params[3] = digit;
                            }
                            7..=8 => {
                                // res (2 digits)
                                if self.builder.param_state == 7 {
                                    self.builder.u16_params[4] = digit;
                                } else {
                                    self.builder.u16_params[4] = self.builder.u16_params[4].wrapping_mul(36).wrapping_add(digit);
                                }
                            }
                            _ => {}
                        }
                        self.builder.param_state += 1;
                        Ok(false)
                    } else {
                        Err(())
                    }
                } else {
                    // Filename text
                    self.builder.string_param.push(ch as char);
                    Ok(false)
                }
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
                if let Some(digit) = BASE36_LUT[ch as usize] {
                    self.builder.u16_params.push(digit);
                    self.builder.param_state += 1;
                    Ok(false)
                } else {
                    Err(())
                }
            }
            (0, b'w') => {
                // param_state == 9: final single digit parameter (size)
                if let Some(digit) = BASE36_LUT[ch as usize] {
                    self.builder.u16_params.push(digit);
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
                if let Some(digit) = BASE36_LUT[ch as usize] {
                    match self.builder.param_state {
                        0..=1 => {
                            // style
                            if self.builder.u16_params.is_empty() {
                                self.builder.u16_params.push(0);
                            }
                            self.builder.u16_params[0] = self.builder.u16_params[0].wrapping_mul(36).wrapping_add(digit);
                        }
                        2..=5 => {
                            // user_pat
                            if self.builder.u16_params.len() < 2 {
                                self.builder.u16_params.resize(2, 0);
                            }
                            self.builder.u16_params[1] = self.builder.u16_params[1].wrapping_mul(36).wrapping_add(digit);
                        }
                        6..=7 => {
                            // thick
                            if self.builder.u16_params.len() < 3 {
                                self.builder.u16_params.resize(3, 0);
                            }
                            self.builder.u16_params[2] = self.builder.u16_params[2].wrapping_mul(36).wrapping_add(digit);
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
                if let Some(digit) = BASE36_LUT[ch as usize] {
                    if self.builder.param_state % 2 == 0 {
                        self.builder.u16_params.push(digit);
                    } else {
                        let idx = self.builder.u16_params.len() - 1;
                        self.builder.u16_params[idx] = self.builder.u16_params[idx].wrapping_mul(36).wrapping_add(digit);
                    }
                    self.builder.param_state += 1;
                    Ok(self.builder.param_state >= 32)
                } else {
                    Err(())
                }
            }

            // P, p, l - Polygon/PolyLine (variable length based on npoints)
            (0, b'P') | (0, b'p') | (0, b'l') if self.builder.param_state < 2 => {
                if let Some(digit) = BASE36_LUT[ch as usize] {
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
                if let Some(digit) = BASE36_LUT[ch as usize] {
                    if self.builder.param_state % 2 == 0 {
                        self.builder.u16_params.push(digit);
                    } else {
                        let idx = self.builder.u16_params.len() - 1;
                        self.builder.u16_params[idx] = self.builder.u16_params[idx].wrapping_mul(36).wrapping_add(digit);
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
                if let Some(digit) = BASE36_LUT[ch as usize] {
                    self.builder.u16_params.push(digit);
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
                if let Some(digit) = BASE36_LUT[ch as usize] {
                    let state = self.builder.param_state;

                    // states 0-1: wid, 2-3: hgt, 4-5: orient (params 0,1,2)
                    if state <= 5 {
                        let idx = state / 2;
                        if self.builder.u16_params.len() <= idx {
                            self.builder.u16_params.resize(idx + 1, 0);
                        }
                        if state % 2 == 0 {
                            self.builder.u16_params[idx] = digit;
                        } else {
                            self.builder.u16_params[idx] = self.builder.u16_params[idx].wrapping_mul(36).wrapping_add(digit);
                        }
                    }
                    // states 6-9: flags (4 digits, param 3)
                    else if state <= 9 {
                        let idx = 3;
                        if self.builder.u16_params.len() <= idx {
                            self.builder.u16_params.resize(idx + 1, 0);
                        }
                        self.builder.u16_params[idx] = self.builder.u16_params[idx].wrapping_mul(36).wrapping_add(digit);
                    }
                    // states 10-29: bevsize, dfore, dback, bright, dark, surface, grp_no, flags2, uline_col, corner_col (params 4-13, all 2 digits)
                    else if state <= 29 {
                        let idx = 4 + (state - 10) / 2;
                        if self.builder.u16_params.len() <= idx {
                            self.builder.u16_params.resize(idx + 1, 0);
                        }
                        if (state - 10) % 2 == 0 {
                            self.builder.u16_params[idx] = digit;
                        } else {
                            self.builder.u16_params[idx] = self.builder.u16_params[idx].wrapping_mul(36).wrapping_add(digit);
                        }
                    }
                    // states 30-36: res (7 digits, param 14)
                    else if state <= 36 {
                        let idx = 14;
                        if self.builder.u16_params.len() <= idx {
                            self.builder.u16_params.resize(idx + 1, 0);
                        }
                        self.builder.u16_params[idx] = self.builder.u16_params[idx].wrapping_mul(36).wrapping_add(digit);
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
                    if let Some(digit) = BASE36_LUT[ch as usize] {
                        if self.builder.u16_params.len() == 0 {
                            self.builder.u16_params.push(0);
                        }
                        self.builder.u16_params[0] = self.builder.u16_params[0].wrapping_mul(36).wrapping_add(digit);
                        self.builder.param_state += 1;
                        Ok(false)
                    } else {
                        Err(())
                    }
                }
                // states 3, 4: res (2 digits)
                else if self.builder.param_state <= 4 {
                    if let Some(digit) = BASE36_LUT[ch as usize] {
                        if self.builder.u16_params.len() < 2 {
                            self.builder.u16_params.resize(2, 0);
                        }
                        self.builder.u16_params[1] = self.builder.u16_params[1].wrapping_mul(36).wrapping_add(digit);
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
                    if let Some(digit) = BASE36_LUT[ch as usize] {
                        if self.builder.param_state == 0 {
                            // mode: 1 digit
                            if self.builder.u16_params.is_empty() {
                                self.builder.u16_params.resize(2, 0);
                            }
                            self.builder.u16_params[0] = digit;
                        } else {
                            // res: 3 digits (states 1..3)
                            self.builder.u16_params[1] = self.builder.u16_params[1].wrapping_mul(36).wrapping_add(digit);
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
                if let Some(digit) = BASE36_LUT[ch as usize] {
                    let idx = match self.builder.param_state {
                        0..=1 => self.builder.param_state as usize, // mode, proto
                        2..=3 => 2,                                 // file_type
                        _ => 3,                                     // res
                    };
                    if self.builder.u16_params.len() <= idx {
                        self.builder.u16_params.resize(idx + 1, 0);
                    }
                    self.builder.u16_params[idx] = self.builder.u16_params[idx].wrapping_mul(36).wrapping_add(digit);
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
