use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use icy_net::protocol::{Protocol, TransferState};
use icy_net::Connection;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

/// An external protocol that runs a command and pipes stdin/stdout to the connection.
///
/// Supports the following placeholders in commands:
/// - `%D` - Download directory
/// - `%F` - Files to upload (space-separated, quoted if necessary)
pub struct ExternalProtocol {
    /// Command to run for sending files (upload)
    send_command: String,
    /// Command to run for receiving files (download)
    recv_command: String,
    /// Download directory
    download_dir: PathBuf,
    /// Protocol name for display
    name: String,
    /// Cancel flag (shared for async cancellation)
    cancel_requested: Arc<AtomicBool>,
    /// Current child process ID (0 if none)
    child_pid: Arc<AtomicU32>,
}

impl ExternalProtocol {
    pub fn new(name: String, send_command: String, recv_command: String, download_dir: PathBuf) -> Self {
        Self {
            send_command,
            recv_command,
            download_dir,
            name,
            cancel_requested: Arc::new(AtomicBool::new(false)),
            child_pid: Arc::new(AtomicU32::new(0)),
        }
    }

    fn expand_command(&self, command: &str, files: &[PathBuf]) -> String {
        let files_str = files
            .iter()
            .map(|p| {
                let s = p.to_string_lossy();
                if s.contains(' ') {
                    format!("\"{}\"", s)
                } else {
                    s.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        command.replace("%D", &self.download_dir.to_string_lossy()).replace("%F", &files_str)
    }

    fn parse_command(command: &str) -> Result<Vec<String>, String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut chars = command.chars().peekable();
        let mut quote = None;

        while let Some(ch) = chars.next() {
            match quote {
                Some(delimiter) if ch == delimiter => quote = None,
                Some(delimiter) if ch == '\\' => match chars.peek().copied() {
                    Some(next) if next == delimiter || next == '\\' => current.push(chars.next().unwrap()),
                    _ => current.push(ch),
                },
                Some(_) => current.push(ch),
                None if ch == '\'' || ch == '"' => quote = Some(ch),
                None if ch.is_whitespace() => {
                    if !current.is_empty() {
                        parts.push(std::mem::take(&mut current));
                    }
                }
                None if ch == '\\' => match chars.peek().copied() {
                    Some(next) if next.is_whitespace() || next == '\'' || next == '"' || next == '\\' => current.push(chars.next().unwrap()),
                    _ => current.push(ch),
                },
                None => current.push(ch),
            }
        }

        if let Some(delimiter) = quote {
            return Err(format!("Unterminated {delimiter} quote in external protocol command"));
        }
        if !current.is_empty() {
            parts.push(current);
        }
        Ok(parts)
    }

    async fn run_command(&mut self, com: &mut dyn Connection, command: &str, working_dir: Option<&PathBuf>) -> icy_net::Result<()> {
        // Reset cancel flag at start
        self.cancel_requested.store(false, Ordering::SeqCst);
        self.child_pid.store(0, Ordering::SeqCst);

        log::info!("Running external protocol command: {}", command);
        if let Some(dir) = working_dir {
            log::info!("Working directory: {}", dir.display());
        }

        // Parse command into program and args without invoking a shell.
        let parts = Self::parse_command(command).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;
        if parts.is_empty() {
            return Err("Empty command".into());
        }

        let program = &parts[0];
        let args = &parts[1..];

        let mut cmd = Command::new(program);
        cmd.args(args).stdin(Stdio::piped()).stdout(Stdio::piped());

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { format!("Failed to start process: {}", e).into() })?;

        // Store PID for cancellation
        if let Some(pid) = child.id() {
            self.child_pid.store(pid, Ordering::SeqCst);
            log::info!("External protocol process started with PID: {}", pid);
        }

        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        let mut stdout = child.stdout.take().expect("Failed to open stdout");

        let mut read_buf = [0u8; 4096];
        let mut stdout_buf = [0u8; 4096];
        let cancel_flag = self.cancel_requested.clone();

        loop {
            // Check cancel flag
            if cancel_flag.load(Ordering::SeqCst) {
                log::info!("External protocol transfer cancelled");
                // Kill the child process
                let _ = child.kill().await;
                self.child_pid.store(0, Ordering::SeqCst);
                return Err("Transfer cancelled".into());
            }

            tokio::select! {
                // Read from connection, write to process stdin
                result = com.read(&mut read_buf) => {
                    match result {
                        Ok(0) => break, // Connection closed
                        Ok(n) => {
                            if let Err(e) = stdin.write_all(&read_buf[..n]).await {
                                log::error!("Failed to write to process stdin: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            log::error!("Connection read error: {}", e);
                            break;
                        }
                    }
                }
                // Read from process stdout, write to connection
                result = stdout.read(&mut stdout_buf) => {
                    match result {
                        Ok(0) => break, // Process closed stdout
                        Ok(n) => {
                            if let Err(e) = com.send(&stdout_buf[..n]).await {
                                log::error!("Failed to send to connection: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            log::error!("Process stdout read error: {}", e);
                            break;
                        }
                    }
                }
                // Check if process exited
                result = child.wait() => {
                    match result {
                        Ok(status) => {
                            log::info!("External protocol process exited with: {}", status);
                            break;
                        }
                        Err(e) => {
                            log::error!("Error waiting for process: {}", e);
                            break;
                        }
                    }
                }
                // Periodic check for cancellation (every 100ms)
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    // Just continue the loop to check cancel flag
                }
            }
        }

        self.child_pid.store(0, Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ExternalProtocol;

    #[test]
    fn parses_quoted_command_arguments() {
        assert_eq!(
            ExternalProtocol::parse_command(r#""C:\Program Files\transfer.exe" --send "C:\Users\Mike\Test File.zip""#).unwrap(),
            [r#"C:\Program Files\transfer.exe"#, "--send", r#"C:\Users\Mike\Test File.zip"#]
        );
    }

    #[test]
    fn preserves_unquoted_windows_backslashes() {
        assert_eq!(
            ExternalProtocol::parse_command(r#"transfer.exe C:\Users\Mike\file.zip"#).unwrap(),
            ["transfer.exe", r#"C:\Users\Mike\file.zip"#]
        );
    }

    #[test]
    fn rejects_unterminated_quotes() {
        assert!(ExternalProtocol::parse_command(r#"transfer.exe "unfinished"#).is_err());
    }
}

#[async_trait]
impl Protocol for ExternalProtocol {
    async fn update_transfer(&mut self, _com: &mut dyn Connection, _transfer_state: &mut TransferState) -> icy_net::Result<()> {
        // External protocols handle their own transfer state
        Ok(())
    }

    async fn initiate_send(&mut self, com: &mut dyn Connection, files: &[PathBuf]) -> icy_net::Result<TransferState> {
        let command = self.expand_command(&self.send_command, files);
        let state = TransferState::new(self.name.clone());

        self.run_command(com, &command, None).await?;

        Ok(state)
    }

    async fn initiate_recv(&mut self, com: &mut dyn Connection) -> icy_net::Result<TransferState> {
        let command = self.expand_command(&self.recv_command, &[]);
        let state = TransferState::new(self.name.clone());

        // Set working directory to download directory for receive commands
        self.run_command(com, &command, Some(&self.download_dir.clone())).await?;

        Ok(state)
    }

    async fn cancel_transfer(&mut self, _com: &mut dyn Connection) -> icy_net::Result<()> {
        log::info!("Cancel requested for external protocol");
        // Set the cancel flag - the run_command loop will pick this up
        self.cancel_requested.store(true, Ordering::SeqCst);

        // Also try to kill the process directly using the stored PID
        let pid = self.child_pid.load(Ordering::SeqCst);
        if pid != 0 {
            log::info!("Killing process {}", pid);
            #[cfg(unix)]
            {
                // Use kill command to send SIGTERM
                let _ = std::process::Command::new("kill").arg("-TERM").arg(pid.to_string()).spawn();
            }
            #[cfg(windows)]
            {
                // Use taskkill on Windows
                let _ = std::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).spawn();
            }
        }

        Ok(())
    }
}
