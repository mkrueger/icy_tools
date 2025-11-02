#[derive(Debug, Default)]
pub struct EmulatedModem {
    line_open: bool,
    local_command_buffer: Vec<u8>,
}

pub enum ModemCommand {
    Nothing,
    Output(Vec<u8>),
    PlayLineSound,
    PlayDialSound(String),
    StopSound,
}

impl EmulatedModem {
    pub fn reset(&mut self) {
        self.local_command_buffer.clear();
    }

    pub fn process_local_input(&mut self, data: &[u8]) -> ModemCommand {
        if self.line_open {
            self.line_open = false;
            return ModemCommand::StopSound;
        }
        for &byte in data {
            // Check for ESC sequence - clear buffer if found
            if byte == 27 {
                return ModemCommand::Nothing;
            }

            // Only allow printable ASCII, backspace, and carriage return
            match byte {
                8 => {
                    // Backspace - remove last character from buffer
                    if !self.local_command_buffer.is_empty() {
                        self.local_command_buffer.pop();
                        // Echo backspace to terminal (backspace, space, backspace to clear)
                        return ModemCommand::Output(vec![8, b' ', 8]);
                    }
                }
                13 => {
                    // Enter pressed - process command
                    let command = String::from_utf8_lossy(&self.local_command_buffer).trim().to_ascii_uppercase();
                    self.local_command_buffer.clear();
                    if command.starts_with("ATD") {
                        let phone_number = command[3..].trim();
                        self.line_open = true;
                        if phone_number.is_empty() {
                            return ModemCommand::PlayLineSound;
                        } else {
                            return ModemCommand::PlayDialSound(phone_number.to_string());
                        }
                    }

                    // Process AT command
                    let response = if command.is_empty() || command.starts_with("AT") {
                        // Valid AT command - for now just return OK
                        "\r\nOK\r\n"
                    } else {
                        // Invalid command
                        "\r\nERROR\r\n"
                    };

                    // Send response
                    if !response.is_empty() {
                        return ModemCommand::Output(response.as_bytes().to_vec());
                    }

                    // Clear command buffer
                    self.local_command_buffer.clear();
                }
                _ => {
                    // Printable ASCII character - add to buffer and echo
                    self.local_command_buffer.push(byte);
                    return ModemCommand::Output(vec![byte]);
                }
            }
        }
        ModemCommand::Nothing
    }
}
