#[derive(Debug)]
pub struct EmulatedModem {
    line_open: bool,
    local_command_buffer: Vec<u8>,
    echo_enabled: bool,
    verbose_mode: bool,
    speaker_volume: u8, // 0 = off, 1 = low, 2 = medium, 3 = high
    auto_answer: bool,
}

pub enum ModemCommand {
    Nothing,
    Output(Vec<u8>),
    PlayLineSound,
    PlayDialSound(bool, String),
    StopSound,
    Reconnect,
    Connect(String),
}

impl Default for EmulatedModem {
    fn default() -> Self {
        Self {
            line_open: false,
            local_command_buffer: Vec::new(),
            echo_enabled: true,
            verbose_mode: true,
            speaker_volume: 1,
            auto_answer: false,
        }
    }
}

impl EmulatedModem {
    pub fn reset(&mut self) {
        self.local_command_buffer.clear();
        self.echo_enabled = true;
        self.verbose_mode = true;
        self.speaker_volume = 1;
        self.auto_answer = false;
    }

    fn response(&self, code: &str, message: &str) -> Vec<u8> {
        if self.verbose_mode {
            format!("\r\n{}\r\n", message).as_bytes().to_vec()
        } else {
            format!("{}\r", code).as_bytes().to_vec()
        }
    }

    fn ok_response(&self) -> Vec<u8> {
        self.response("0", "OK")
    }

    fn error_response(&self) -> Vec<u8> {
        self.response("4", "ERROR")
    }
    /*
    fn connect_response(&self, speed: u32) -> Vec<u8> {
        if self.verbose_mode {
            format!("\r\nCONNECT {}\r\n", speed).as_bytes().to_vec()
        } else {
            b"1\r".to_vec()
        }
    }*/

    fn parse_single_command(&mut self, cmd_bytes: &[u8]) -> (usize, ModemCommand) {
        if cmd_bytes.is_empty() {
            return (0, ModemCommand::Output(self.ok_response()));
        }

        // Only convert the command letter(s) to uppercase, not the parameters
        let cmd_str = String::from_utf8_lossy(cmd_bytes);

        // Check for multi-character commands first
        if cmd_str.len() >= 2 && cmd_str[0..2].to_uppercase() == "&F" {
            self.reset();
            return (2, ModemCommand::Output(self.ok_response()));
        }
        if cmd_str.len() >= 2 && cmd_str[0..2].to_uppercase() == "&C" {
            let (consumed, _) = parse_numeric_param(&cmd_str[2..], 1);
            return (2 + consumed, ModemCommand::Output(self.ok_response()));
        }
        if cmd_str.len() >= 2 && cmd_str[0..2].to_uppercase() == "&D" {
            let (consumed, _) = parse_numeric_param(&cmd_str[2..], 2);
            return (2 + consumed, ModemCommand::Output(self.ok_response()));
        }
        if cmd_str.len() >= 2 && cmd_str[0..2].to_uppercase() == "&K" {
            let (consumed, _) = parse_numeric_param(&cmd_str[2..], 3);
            return (2 + consumed, ModemCommand::Output(self.ok_response()));
        }

        // Get first character in uppercase for command matching
        let first_char = cmd_str.chars().next().unwrap().to_ascii_uppercase();

        // Dial commands - preserve case for phone numbers/URLs
        if first_char == 'D' {
            return self.parse_dial_command(&cmd_str);
        }

        // Single letter commands with optional numeric parameter
        match first_char {
            'E' => {
                // Echo command
                let (consumed, value) = parse_numeric_param(&cmd_str[1..], 1);
                self.echo_enabled = value != 0;
                (1 + consumed, ModemCommand::Output(self.ok_response()))
            }
            'H' => {
                // Hook command (hangup)
                let (consumed, _value) = parse_numeric_param(&cmd_str[1..], 0);
                self.line_open = false;
                (1 + consumed, ModemCommand::Output(self.ok_response()))
            }
            'L' => {
                // Speaker volume
                let (consumed, value) = parse_numeric_param(&cmd_str[1..], 1);
                self.speaker_volume = value.min(3);
                (1 + consumed, ModemCommand::Output(self.ok_response()))
            }
            'M' => {
                // Speaker control
                let (consumed, value) = parse_numeric_param(&cmd_str[1..], 1);
                self.speaker_volume = if value == 0 { 0 } else { self.speaker_volume.max(1) };
                (1 + consumed, ModemCommand::Output(self.ok_response()))
            }
            'Q' => {
                // Quiet mode (result codes)
                let (consumed, _value) = parse_numeric_param(&cmd_str[1..], 0);
                (1 + consumed, ModemCommand::Output(self.ok_response()))
            }
            'S' => {
                // S-registers
                self.parse_s_register(&cmd_str)
            }
            'V' => {
                // Verbose mode
                let (consumed, value) = parse_numeric_param(&cmd_str[1..], 1);
                self.verbose_mode = value != 0;
                (1 + consumed, ModemCommand::Output(self.ok_response()))
            }
            'X' => {
                // Extended result codes
                let (consumed, _value) = parse_numeric_param(&cmd_str[1..], 4);
                (1 + consumed, ModemCommand::Output(self.ok_response()))
            }
            'Z' => {
                // Reset modem
                let (consumed, _) = parse_numeric_param(&cmd_str[1..], 0);
                self.reset();
                (1 + consumed, ModemCommand::Output(self.ok_response()))
            }
            '&' => {
                // & without recognized command
                (1, ModemCommand::Output(self.error_response()))
            }
            _ => {
                // Unknown command
                (0, ModemCommand::Output(self.error_response()))
            }
        }
    }

    fn parse_dial_command(&mut self, cmd: &str) -> (usize, ModemCommand) {
        // Check DL with uppercase
        if cmd.len() >= 2 && cmd[0..2].to_uppercase() == "DL" {
            return (2, ModemCommand::Reconnect);
        }

        // Determine dial mode - only uppercase the command letters, not the data
        let (use_tone_dial, number_start) = if cmd.len() >= 2 && cmd[0..2].to_uppercase() == "DT" {
            (true, 2)
        } else if cmd.len() >= 2 && cmd[0..2].to_uppercase() == "DP" {
            (false, 2)
        } else {
            (true, 1) // Default to tone
        };

        // Keep the original case for the phone number/URL
        let phone_number = cmd[number_start..].trim();
        self.line_open = true;

        let command = if phone_number.is_empty() {
            ModemCommand::PlayLineSound
        } else if phone_number.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '(' || c == ')' || c == ' ') {
            // Phone number with common separators
            let clean_number = phone_number.chars().filter(|c| c.is_ascii_digit()).collect::<String>();
            ModemCommand::PlayDialSound(use_tone_dial, clean_number)
        } else {
            // Try to parse as connection address - preserve case!
            if let Ok(_) = crate::ConnectionInformation::parse(phone_number) {
                ModemCommand::Connect(phone_number.to_string())
            } else {
                // Fallback to dial sound
                ModemCommand::PlayDialSound(use_tone_dial, phone_number.to_string())
            }
        };

        (cmd.len(), command)
    }

    fn parse_s_register(&mut self, cmd: &str) -> (usize, ModemCommand) {
        if cmd.len() < 2 {
            return (1, ModemCommand::Output(self.error_response()));
        }

        // Parse register number
        let mut pos = 1;
        let mut reg_num = 0u8;

        while pos < cmd.len() && cmd.chars().nth(pos).unwrap().is_ascii_digit() {
            reg_num = reg_num * 10 + (cmd.as_bytes()[pos] - b'0');
            pos += 1;
        }

        // Check for = (set) or ? (query)
        if pos < cmd.len() {
            let next_char = cmd.chars().nth(pos).unwrap().to_ascii_uppercase();
            match next_char {
                '=' => {
                    // Set S-register
                    pos += 1;
                    let (consumed, _value) = parse_numeric_param(&cmd[pos..], 0);
                    (pos + consumed, ModemCommand::Output(self.ok_response()))
                }
                '?' => {
                    // Query S-register
                    let response = format!("{:03}\r\n", 0).as_bytes().to_vec();
                    (pos + 1, ModemCommand::Output(response))
                }
                _ => (pos, ModemCommand::Output(self.error_response())),
            }
        } else {
            // Just Sn without = or ?, treat as select register
            (pos, ModemCommand::Output(self.ok_response()))
        }
    }

    fn parse_at_command(&mut self, command: &str) -> ModemCommand {
        // Handle empty AT - check case-insensitive
        if command.to_uppercase() == "AT" {
            return ModemCommand::Output(self.ok_response());
        }

        // Parse command after AT - only check AT prefix case-insensitive
        if !command[0..2.min(command.len())].to_uppercase().starts_with("AT") {
            return ModemCommand::Output(self.error_response());
        }

        let cmd_part = &command[2..];

        // Handle compound commands - rest of the function stays the same
        let mut pos = 0;
        let bytes = cmd_part.as_bytes();
        let mut has_error = false;
        let mut has_valid_command = false;
        let mut non_output_command = None;

        while pos < bytes.len() {
            let (consumed, response) = self.parse_single_command(&bytes[pos..]);

            if consumed == 0 {
                pos += 1;
                has_error = true;
                continue;
            }

            match response {
                ModemCommand::Output(data) => {
                    if data == self.error_response() {
                        has_error = true;
                    } else {
                        has_valid_command = true;
                    }
                }
                other => {
                    non_output_command = Some(other);
                    has_valid_command = true;
                }
            }
            pos += consumed;
        }

        if let Some(cmd) = non_output_command {
            return cmd;
        }

        if has_error {
            return ModemCommand::Output(self.error_response());
        }

        if has_valid_command {
            return ModemCommand::Output(self.ok_response());
        }

        ModemCommand::Output(self.ok_response())
    }

    pub fn process_local_input(&mut self, data: &[u8]) -> ModemCommand {
        if self.line_open {
            self.line_open = false;
            return ModemCommand::StopSound;
        }

        let mut out_vec = Vec::new();

        for &byte in data {
            if byte == 27 {
                self.local_command_buffer.clear();
                return ModemCommand::Nothing;
            }

            match byte {
                8 => {
                    if !self.local_command_buffer.is_empty() {
                        self.local_command_buffer.pop();
                        if self.echo_enabled {
                            out_vec.extend_from_slice(&[8, b' ', 8]);
                        }
                    }
                }
                13 => {
                    if data.len() > 1 {
                        out_vec.push(b'\r');
                        continue;
                    }

                    if self.local_command_buffer.is_empty() {
                        if self.echo_enabled {
                            out_vec.extend_from_slice(b"\r\n");
                        }
                        return if out_vec.is_empty() {
                            ModemCommand::Nothing
                        } else {
                            ModemCommand::Output(out_vec)
                        };
                    }

                    // Don't uppercase the entire command - preserve case for URLs
                    let command = String::from_utf8_lossy(&self.local_command_buffer).trim().to_string();
                    self.local_command_buffer.clear();

                    let result = self.parse_at_command(&command);

                    match result {
                        ModemCommand::Output(mut response) => {
                            out_vec.append(&mut response);
                            return ModemCommand::Output(out_vec);
                        }
                        other => return other,
                    }
                }
                32..=126 => {
                    self.local_command_buffer.push(byte);
                    if self.echo_enabled {
                        out_vec.push(byte);
                    }
                }
                _ => {}
            }
        }

        if !out_vec.is_empty() {
            ModemCommand::Output(out_vec)
        } else {
            ModemCommand::Nothing
        }
    }
}

fn parse_numeric_param(s: &str, default: u8) -> (usize, u8) {
    if s.is_empty() {
        return (0, default);
    }

    let mut value = 0u8;
    let mut consumed = 0;

    for ch in s.chars() {
        if ch.is_ascii_digit() {
            value = value.saturating_mul(10).saturating_add(ch as u8 - b'0');
            consumed += 1;
        } else {
            break;
        }
    }

    if consumed == 0 {
        (0, default)
    } else {
        (consumed, value)
    }
}
