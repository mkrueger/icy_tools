use crate::auto_login::AutoLoginCommand;

pub struct AutoLoginParser;

impl AutoLoginParser {
    pub fn parse(input: &str) -> Result<Vec<AutoLoginCommand>, String> {
        let mut commands = Vec::new();
        let mut chars = input.chars().peekable();
        let mut text_buffer = String::new();

        while let Some(ch) = chars.next() {
            match ch {
                '@' => {
                    // Flush any pending text
                    if !text_buffer.is_empty() {
                        commands.push(AutoLoginCommand::SendText(text_buffer.clone()));
                        text_buffer.clear();
                    }

                    // Parse @ command
                    if let Some(cmd_char) = chars.next() {
                        match cmd_char.to_ascii_uppercase() {
                            'D' => {
                                // Parse delay duration
                                let mut num_str = String::new();
                                while let Some(&digit) = chars.peek() {
                                    if digit.is_ascii_digit() {
                                        num_str.push(digit);
                                        chars.next();
                                    } else {
                                        break;
                                    }
                                }
                                let seconds = if num_str.is_empty() {
                                    1 // Default to 1 second if no number specified
                                } else {
                                    num_str.parse::<u32>().map_err(|_| format!("Invalid delay value: {}", num_str))?
                                };
                                commands.push(AutoLoginCommand::Delay(seconds));
                            }
                            'E' => commands.push(AutoLoginCommand::EmulateMailerAccess),
                            'W' => commands.push(AutoLoginCommand::WaitForNamePrompt),
                            'N' => commands.push(AutoLoginCommand::SendFullName),
                            'F' => commands.push(AutoLoginCommand::SendFirstName),
                            'L' => commands.push(AutoLoginCommand::SendLastName),
                            'P' => commands.push(AutoLoginCommand::SendPassword),
                            'I' => commands.push(AutoLoginCommand::DisableIEMSI),
                            c if c.is_ascii_digit() => {
                                // Parse control code number
                                let mut num_str = String::from(c);
                                while let Some(&digit) = chars.peek() {
                                    if digit.is_ascii_digit() {
                                        num_str.push(digit);
                                        chars.next();
                                    } else {
                                        break;
                                    }
                                }
                                let code = num_str.parse::<u8>().map_err(|_| format!("Invalid control code: {}", num_str))?;
                                commands.push(AutoLoginCommand::SendControlCode(code));
                            }
                            _ => {
                                return Err(format!("Unknown command: @{}", cmd_char));
                            }
                        }
                    } else {
                        return Err("Unexpected end of input after @".to_string());
                    }
                }
                '!' => {
                    // Flush any pending text
                    if !text_buffer.is_empty() {
                        commands.push(AutoLoginCommand::SendText(text_buffer.clone()));
                        text_buffer.clear();
                    }

                    // Check if it's a control code or script
                    if let Some(&next_char) = chars.peek() {
                        if next_char.is_ascii_digit() {
                            // Parse control code
                            chars.next(); // consume the digit
                            let mut num_str = String::from(next_char);
                            while let Some(&digit) = chars.peek() {
                                if digit.is_ascii_digit() {
                                    num_str.push(digit);
                                    chars.next();
                                } else {
                                    break;
                                }
                            }
                            let code = num_str.parse::<u8>().map_err(|_| format!("Invalid control code: !{}", num_str))?;
                            commands.push(AutoLoginCommand::SendControlCode(code));
                        } else {
                            // Parse script filename
                            let mut filename = String::new();
                            while let Some(&ch) = chars.peek() {
                                if ch.is_whitespace() || ch == '@' || ch == '!' {
                                    break;
                                }
                                filename.push(ch);
                                chars.next();
                            }
                            if filename.is_empty() {
                                return Err("Expected filename after !".to_string());
                            }
                            commands.push(AutoLoginCommand::RunScript(filename));
                        }
                    } else {
                        return Err("Unexpected end of input after !".to_string());
                    }
                }
                _ => {
                    // Regular text character
                    text_buffer.push(ch);
                }
            }
        }

        // Flush any remaining text
        if !text_buffer.is_empty() {
            commands.push(AutoLoginCommand::SendText(text_buffer));
        }

        Ok(commands)
    }
}
