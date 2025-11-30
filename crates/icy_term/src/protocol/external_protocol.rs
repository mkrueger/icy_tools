use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use icy_net::Connection;
use icy_net::protocol::{Protocol, TransferState};
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
}

impl ExternalProtocol {
    pub fn new(name: String, send_command: String, recv_command: String, download_dir: PathBuf) -> Self {
        Self {
            send_command,
            recv_command,
            download_dir,
            name,
        }
    }

    fn expand_command(&self, command: &str, files: &[PathBuf]) -> String {
        let files_str = files
            .iter()
            .map(|p| {
                let s = p.to_string_lossy();
                if s.contains(' ') { format!("\"{}\"", s) } else { s.to_string() }
            })
            .collect::<Vec<_>>()
            .join(" ");

        command.replace("%D", &self.download_dir.to_string_lossy()).replace("%F", &files_str)
    }

    async fn run_command(&self, com: &mut dyn Connection, command: &str, working_dir: Option<&PathBuf>) -> icy_net::Result<()> {
        log::info!("Running external protocol command: {}", command);
        if let Some(dir) = working_dir {
            log::info!("Working directory: {}", dir.display());
        }

        // Parse command into program and args
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err("Empty command".into());
        }

        let program = parts[0];
        let args = &parts[1..];

        let mut cmd = Command::new(program);
        cmd.args(args).stdin(Stdio::piped()).stdout(Stdio::piped());

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { format!("Failed to start process: {}", e).into() })?;

        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        let mut stdout = child.stdout.take().expect("Failed to open stdout");

        let mut read_buf = [0u8; 4096];
        let mut stdout_buf = [0u8; 4096];

        loop {
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
            }
        }

        Ok(())
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
        // TODO: Kill the running process if any
        Ok(())
    }
}
