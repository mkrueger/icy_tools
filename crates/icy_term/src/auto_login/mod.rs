mod parser;
pub use parser::*;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq)]
pub enum AutoLoginCommand {
    /// Delay for x seconds (@D)
    Delay(u32),

    /// Simulate user trying to access BBS from mailer (@E)
    /// Sends CR+CR then ESC+wait
    EmulateMailerAccess,

    /// Wait for one of the name questions (@W)
    WaitForNamePrompt,

    /// Send full user name (@N)
    SendFullName,

    /// Send first name (@F)
    SendFirstName,

    /// Send last name (@L)
    SendLastName,

    /// Send password (@P)
    SendPassword,

    /// Disable IEMSI in this session (@I)
    DisableIEMSI,

    /// Send control code directly (@num)
    SendControlCode(u8),

    /// Run external script (!FILE)
    RunScript(String),

    /// Plain text to send
    SendText(String),
}

/// Context for executing autologin commands
pub struct AutoLoginContext {
    pub full_name: String,
    pub first_name: String,
    pub last_name: String,
    pub password: String,
}

impl AutoLoginContext {
    pub fn new(full_name: String, password: String) -> Self {
        let parts: Vec<&str> = full_name.split_whitespace().collect();
        let first_name = parts.first().unwrap_or(&"").to_string();
        let last_name = parts.get(1).unwrap_or(&"").to_string();

        Self {
            full_name,
            first_name,
            last_name,
            password,
        }
    }
}

/*
/// Executor for autologin commands
pub struct AutoLoginExecutor<'a> {
    context: &'a AutoLoginContext,
    iemsi_disabled: bool,
}

impl<'a> AutoLoginExecutor<'a> {
    pub fn new(context: &'a AutoLoginContext) -> Self {
        Self {
            context,
            iemsi_disabled: false,
        }
    }

    /// Execute a list of commands and return the data to send
    pub async fn execute(&mut self, commands: Vec<AutoLoginCommand>) -> Result<Vec<u8>, String> {
        let mut output = Vec::new();

        for command in commands {
            match command {
                AutoLoginCommand::Delay(seconds) => {
                    // In actual implementation, this would delay
                    // For now, we'll just note it needs to happen
                    tokio::time::sleep(Duration::from_secs(seconds as u64)).await;
                }
                AutoLoginCommand::EmulateMailerAccess => {
                    output.push(13); // CR
                    output.push(13); // CR
                    output.push(27); // ESC
                    // Wait for response would go here
                }
                AutoLoginCommand::WaitForNamePrompt => {
                    // In actual implementation, would wait for specific prompts
                    // like "name:", "login:", "user:", etc.
                }
                AutoLoginCommand::SendFullName => {
                    output.extend_from_slice(self.context.full_name.as_bytes());
                }
                AutoLoginCommand::SendFirstName => {
                    output.extend_from_slice(self.context.first_name.as_bytes());
                }
                AutoLoginCommand::SendLastName => {
                    output.extend_from_slice(self.context.last_name.as_bytes());
                }
                AutoLoginCommand::SendPassword => {
                    output.extend_from_slice(self.context.password.as_bytes());
                }
                AutoLoginCommand::DisableIEMSI => {
                    self.iemsi_disabled = true;
                }
                AutoLoginCommand::SendControlCode(code) => {
                    output.push(code);
                }
                AutoLoginCommand::RunScript(filename) => {
                    // Load and parse the script file
                    let script_content = std::fs::read_to_string(&filename)
                        .map_err(|e| format!("Failed to read script {}: {}", filename, e))?;
                    let script_commands = AutoLoginParser::parse(&script_content)?;
                    let script_output = self.execute(script_commands).await?;
                    output.extend_from_slice(&script_output);
                }
                AutoLoginCommand::SendText(text) => {
                    output.extend_from_slice(text.as_bytes());
                }
            }
        }

        Ok(output)
    }

    pub fn is_iemsi_disabled(&self) -> bool {
        self.iemsi_disabled
    }
}
*/
