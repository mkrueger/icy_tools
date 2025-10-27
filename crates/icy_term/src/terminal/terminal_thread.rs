use crate::ScreenMode;
use icy_engine::editor::EditState;
use icy_engine::{BufferParser, CallbackAction};
use icy_net::{
    Connection, ConnectionType,
    modem::{ModemConfiguration, ModemConnection},
    protocol::{TransferProtocolType, TransferState},
    raw::RawConnection,
    serial::Serial,
    ssh::{Credentials, SSHConnection},
    telnet::{TelnetConnection, TermCaps, TerminalEmulation},
};
use log::{debug, error, trace, warn};
use std::backtrace;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use web_time::{Duration, Instant};

/// Messages sent to the terminal thread
#[derive(Debug, Clone)]
pub enum TerminalCommand {
    Connect(ConnectionConfig),
    Disconnect,
    SendData(Vec<u8>),
    StartUpload(TransferProtocolType, Vec<PathBuf>),
    StartDownload(TransferProtocolType, Option<String>),
    CancelTransfer,
    Resize(u16, u16),
}

/// Messages sent from the terminal thread to the UI
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    Connected,
    Disconnected(Option<String>), // Optional error message
    DataReceived(Vec<u8>),
    BufferUpdated,
    TransferStarted(TransferState),
    TransferProgress(TransferState),
    TransferCompleted(TransferState),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub connection_type: ConnectionType,
    pub address: String,
    pub terminal_type: TerminalEmulation,
    pub window_size: (u16, u16),
    pub timeout: Duration,
    pub user_name: Option<String>,
    pub password: Option<String>,
    pub proxy_command: Option<String>,
    pub modem: Option<ModemConfig>,
    pub use_utf8: bool,
}

#[derive(Debug, Clone)]
pub struct ModemConfig {
    pub device: String,
    pub baud_rate: u32,
    pub char_size: u8,
    pub parity: icy_net::serial::Parity,
    pub stop_bits: icy_net::serial::StopBits,
    pub flow_control: icy_net::serial::FlowControl,
    // Support multiple init lines (safer & closer to original patterns)
    pub init_string: Vec<String>,
    pub dial_string: String,
}

pub struct TerminalThread {
    // Shared state with UI
    edit_state: Arc<Mutex<EditState>>,

    // Thread-local state
    connection: Option<Box<dyn Connection>>,
    buffer_parser: Box<dyn BufferParser>,
    current_transfer: Option<TransferState>,
    connection_time: Option<Instant>,

    // Communication channels
    command_rx: mpsc::UnboundedReceiver<TerminalCommand>,
    event_tx: mpsc::UnboundedSender<TerminalEvent>,

    use_utf8: bool,
    utf8_buffer: Vec<u8>,
    local_command_buffer: Vec<u8>,
}

impl TerminalThread {
    pub fn spawn(
        edit_state: Arc<Mutex<EditState>>,
        buffer_parser: Box<dyn BufferParser>,
    ) -> (mpsc::UnboundedSender<TerminalCommand>, mpsc::UnboundedReceiver<TerminalEvent>) {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let mut thread = Self {
            edit_state,
            connection: None,
            buffer_parser,
            current_transfer: None,
            connection_time: None,
            command_rx,
            event_tx: event_tx.clone(),
            use_utf8: false,
            utf8_buffer: Vec::new(),
            local_command_buffer: Vec::new(),
        };

        // Spawn the async runtime for the terminal thread
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime");

            runtime.block_on(async move {
                thread.run().await;
            });
        });

        (command_tx, event_rx)
    }

    async fn run(&mut self) {
        let mut read_buffer = vec![0u8; 64 * 1024];
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(16)); // ~60fps

        loop {
            tokio::select! {
                // Handle commands from UI
                Some(cmd) = self.command_rx.recv() => {
                    self.handle_command(cmd).await;
                }

                // Periodic tick for updates and reading
                _ = interval.tick() => {
                    // Read from connection if connected
                    if self.connection.is_some() {
                        self.read_connection(&mut read_buffer).await;
                    }
                }
            }
        }
    }

    fn perform_resize(&mut self, width: u16, height: u16) {
        if let Ok(mut state) = self.edit_state.lock() {
            state.get_buffer_mut().set_size((width as i32, height as i32));
        }
        // Optionally notify UI so layout can adjust
        let _ = self.event_tx.send(TerminalEvent::BufferUpdated);
    }

    async fn handle_command(&mut self, command: TerminalCommand) {
        match command {
            TerminalCommand::Connect(config) => {
                if let Err(e) = self.connect(config).await {
                    let _ = self.event_tx.send(TerminalEvent::Disconnected(Some(e.to_string())));
                }
            }
            TerminalCommand::Disconnect => {
                self.disconnect().await;
            }
            TerminalCommand::SendData(data) => {
                if let Some(conn) = &mut self.connection {
                    if let Err(e) = conn.send(&data).await {
                        self.disconnect().await;
                    }
                } else {
                    // Echo locally
                    self.process_local_input(&data).await;
                }
            }
            TerminalCommand::StartUpload(protocol, files) => {
                self.start_upload(protocol, files).await;
            }
            TerminalCommand::StartDownload(protocol, filename) => {
                self.start_download(protocol, filename).await;
            }
            TerminalCommand::CancelTransfer => {
                self.current_transfer = None;
            }
            TerminalCommand::Resize(width, height) => {
                self.perform_resize(width, height);
            }
        }
    }

    async fn connect(&mut self, config: ConnectionConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.connection.is_some() {
            self.disconnect().await;
        }

        trace!(
            "Connecting: type={:?} addr={} window={:?}",
            config.connection_type, config.address, config.window_size
        );
        self.use_utf8 = config.use_utf8;

        let connection: Box<dyn Connection> = match config.connection_type {
            ConnectionType::Telnet => {
                let term_caps = TermCaps {
                    terminal: config.terminal_type,
                    window_size: config.window_size,
                };
                Box::new(TelnetConnection::open(&config.address, term_caps, config.timeout).await?)
            }
            ConnectionType::Raw => Box::new(RawConnection::open(&config.address, config.timeout).await?),
            ConnectionType::SSH => {
                let term_caps = TermCaps {
                    terminal: config.terminal_type,
                    window_size: config.window_size,
                };
                let creds = Credentials {
                    user_name: config.user_name.unwrap_or_default().clone(),
                    password: config.password.unwrap_or_default().clone(),
                    proxy_command: config.proxy_command.clone(),
                };
                Box::new(SSHConnection::open(&config.address, term_caps, creds).await?)
            }
            ConnectionType::Modem => {
                let Some(m) = &config.modem else {
                    return Err("Modem configuration is required for modem connections".into());
                };
                let serial = Serial {
                    device: m.device.clone(),
                    baud_rate: m.baud_rate,
                    char_size: match m.char_size {
                        5 => icy_net::serial::CharSize::Bits5,
                        6 => icy_net::serial::CharSize::Bits6,
                        7 => icy_net::serial::CharSize::Bits7,
                        _ => icy_net::serial::CharSize::Bits8,
                    },
                    parity: m.parity,
                    stop_bits: m.stop_bits,
                    flow_control: m.flow_control,
                };
                let modem = ModemConfiguration {
                    init_string: String::new().clone(),
                    dial_string: m.dial_string.clone(),
                };
                Box::new(ModemConnection::open(serial, modem, config.address.clone()).await?)
            }
            ConnectionType::Websocket => Box::new(icy_net::websocket::connect(&config.address, false).await?),
            ConnectionType::SecureWebsocket => Box::new(icy_net::websocket::connect(&config.address, true).await?),
            other => return Err(format!("Unsupported connection type: {other:?}").into()),
        };

        self.connection = Some(connection);
        self.connection_time = Some(Instant::now());

        let _ = self.event_tx.send(TerminalEvent::Connected);
        debug!("Connected successfully");
        Ok(())
    }

    async fn disconnect(&mut self) {
        println!("Disconnecting... {}", backtrace::Backtrace::force_capture());

        if let Some(mut conn) = self.connection.take() {
            let _ = conn.shutdown().await;
        }

        self.connection_time = None;
        self.utf8_buffer.clear();
        let _ = self.event_tx.send(TerminalEvent::Disconnected(None));
    }

    async fn read_connection(&mut self, buffer: &mut [u8]) {
        if let Some(conn) = &mut self.connection {
            match conn.try_read(buffer).await {
                Ok(0) => {}
                Ok(size) => {
                    let data = buffer[..size].to_vec();
                    self.process_data(&data).await;
                    let _ = self.event_tx.send(TerminalEvent::DataReceived(data));
                }
                Err(e) => {
                    error!("Connection read error: {e}");
                    self.disconnect().await;
                }
            }
        }
    }

    #[async_recursion::async_recursion(?Send)]
    async fn process_data(&mut self, data: &[u8]) {
        let mut actions = Vec::new();

        if let Ok(mut state) = self.edit_state.lock() {
            let mut caret = state.get_caret().clone();
            {
                let buffer = state.get_buffer_mut();

                if self.use_utf8 {
                    // UTF-8 mode: decode multi-byte sequences
                    let mut to_process = Vec::new();

                    // Append new data to any incomplete sequence from before
                    self.utf8_buffer.extend_from_slice(data);

                    let mut i = 0;
                    while i < self.utf8_buffer.len() {
                        // Try to decode a UTF-8 character starting at position i
                        let remaining = &self.utf8_buffer[i..];

                        match std::str::from_utf8(remaining) {
                            Ok(valid_str) => {
                                // All remaining bytes form valid UTF-8
                                for ch in valid_str.chars() {
                                    to_process.push(ch);
                                }
                                i = self.utf8_buffer.len(); // Consumed everything
                            }
                            Err(e) => {
                                // Partial UTF-8 sequence or error
                                if e.valid_up_to() > 0 {
                                    // Process the valid part
                                    let valid_str = unsafe {
                                        // Safe because we know valid_up_to() bytes are valid UTF-8
                                        std::str::from_utf8_unchecked(&remaining[..e.valid_up_to()])
                                    };
                                    for ch in valid_str.chars() {
                                        to_process.push(ch);
                                    }
                                    i += e.valid_up_to();
                                }

                                // Check if we have an incomplete sequence at the end
                                if let Some(error_len) = e.error_len() {
                                    // Invalid UTF-8 sequence, skip these bytes
                                    // Could also replace with � (U+FFFD)
                                    to_process.push('\u{FFFD}'); // Replacement character
                                    i += error_len;
                                } else {
                                    // Incomplete sequence at end, keep it for next time
                                    break;
                                }
                            }
                        }
                    }

                    // Keep any incomplete sequence for next call
                    if i < self.utf8_buffer.len() {
                        self.utf8_buffer = self.utf8_buffer[i..].to_vec();
                    } else {
                        self.utf8_buffer.clear();
                    }

                    // Process all complete characters
                    for ch in to_process {
                        match self.buffer_parser.print_char(buffer, 0, &mut caret, ch) {
                            Ok(action) => actions.push(action),
                            Err(e) => error!("Parser error: {e}"),
                        }
                    }
                } else {
                    // Legacy mode: treat each byte as a character (CP437 or similar)
                    for &byte in data {
                        match self.buffer_parser.print_char(buffer, 0, &mut caret, byte as char) {
                            Ok(action) => actions.push(action),
                            Err(e) => error!("Parser error: {e}"),
                        }
                    }
                }
            }
            *state.get_caret_mut() = caret;
        }

        for action in actions {
            self.handle_parser_action(action).await;
        }

        let _ = self.event_tx.send(TerminalEvent::BufferUpdated);
    }

    async fn process_local_input(&mut self, data: &[u8]) {
        for &byte in data {
            // Check for ESC sequence - clear buffer if found
            if byte == 27 {
                return;
            }

            // Only allow printable ASCII, backspace, and carriage return
            match byte {
                8 => {
                    // Backspace - remove last character from buffer
                    if !self.local_command_buffer.is_empty() {
                        self.local_command_buffer.pop();
                        // Echo backspace to terminal (backspace, space, backspace to clear)
                        self.process_data(&[8, b' ', 8]).await;
                    }
                }
                13 => {
                    // Echo the carriage return and line feed
                    self.process_data(b"\r\n").await;

                    // Enter pressed - process command
                    let command = String::from_utf8_lossy(&self.local_command_buffer);
                    // Process AT command
                    let response = if command.trim().to_uppercase().starts_with("AT") {
                        // Valid AT command - for now just return OK
                        "OK\r\n"
                    } else if command.trim().is_empty() {
                        // Empty command - no response
                        ""
                    } else {
                        // Invalid command
                        "ERROR\r\n"
                    };

                    // Send response
                    if !response.is_empty() {
                        self.process_data(response.as_bytes()).await;
                    }

                    // Clear command buffer
                    self.local_command_buffer.clear();
                }
                32..=126 => {
                    // Printable ASCII character - add to buffer and echo
                    self.local_command_buffer.push(byte);
                    self.process_data(&[byte]).await;
                }
                _ => {
                    // Ignore all other characters
                }
            }
        }
    }

    async fn handle_parser_action(&mut self, action: CallbackAction) {
        match action {
            CallbackAction::SendString(s) => {
                if let Some(conn) = &mut self.connection {
                    let _ = conn.send(s.as_bytes()).await;
                } else {
                    // Echo locally when disconnected
                    self.process_data(s.as_bytes()).await;
                }
            }
            CallbackAction::Beep => {
                // Optional: send an event for UI beep
            }
            CallbackAction::ResizeTerminal(width, height) => {
                // Avoid async recursion by calling sync helper
                self.perform_resize(width as u16, height as u16);
            }
            _ => {}
        }
    }

    async fn start_upload(&mut self, protocol: TransferProtocolType, files: Vec<PathBuf>) {
        if let Some(conn) = &mut self.connection {
            let mut prot = protocol.create();
            match prot.initiate_send(&mut **conn, &files).await {
                Ok(state) => {
                    self.current_transfer = Some(state.clone());
                    let _ = self.event_tx.send(TerminalEvent::TransferStarted(state));
                    // TODO: Handle actual transfer
                }
                Err(e) => {
                    let _ = self.event_tx.send(TerminalEvent::Error(format!("Failed to start upload: {}", e)));
                }
            }
        }
    }

    async fn start_download(&mut self, protocol: TransferProtocolType, filename: Option<String>) {
        if let Some(conn) = &mut self.connection {
            let mut prot = protocol.create();
            match prot.initiate_recv(&mut **conn).await {
                Ok(mut state) => {
                    if let Some(name) = filename {
                        state.recieve_state.file_name = name;
                    }
                    self.current_transfer = Some(state.clone());
                    let _ = self.event_tx.send(TerminalEvent::TransferStarted(state));
                    // TODO: Handle actual transfer
                }
                Err(e) => {
                    let _ = self.event_tx.send(TerminalEvent::Error(format!("Failed to start download: {}", e)));
                }
            }
        }
    }
}

// Helper function to create a terminal thread for the UI
pub fn create_terminal_thread(
    edit_state: Arc<Mutex<EditState>>,
    terminal_type: TerminalEmulation,
) -> (mpsc::UnboundedSender<TerminalCommand>, mpsc::UnboundedReceiver<TerminalEvent>) {
    use icy_engine::ansi::MusicOption;
    let parser = crate::get_parser(
        &terminal_type,
        MusicOption::Off,
        ScreenMode::default(),
        PathBuf::from(".cache"), // cache directory
    );

    TerminalThread::spawn(edit_state, parser)
}
