use crate::baud_emulator::BaudEmulator;
use crate::emulated_modem::{EmulatedModem, ModemCommand};
use crate::features::{AutoFileTransfer, AutoLogin};
use crate::{ConnectionInformation, ScreenMode};
use directories::UserDirs;
use icy_engine::ansi::BaudEmulation;
use icy_engine::{BufferParser, CallbackAction, EditableScreen, TextAttribute};
use icy_net::iemsi::EmsiISI;
use icy_net::rlogin::RloginConfig;
use icy_net::serial::CharSize;
use icy_net::{
    Connection, ConnectionState, ConnectionType,
    modem::{ModemConfiguration, ModemConnection},
    protocol::{Protocol, TransferProtocolType, TransferState},
    raw::RawConnection,
    serial::Serial,
    ssh::{Credentials, SSHConnection},
    telnet::{TelnetConnection, TermCaps, TerminalEmulation},
};
use log::error;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
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
    SendLogin, // Trigger auto-login
    SetBaudEmulation(BaudEmulation),
}

/// Messages sent from the terminal thread to the UI
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    Connected,
    Disconnected(Option<String>), // Optional error message
    DataReceived(Vec<u8>),
    BufferUpdated,
    TransferStarted(TransferState, bool),
    TransferProgress(TransferState),
    TransferCompleted(TransferState),
    Error(String),
    PlayMusic(icy_engine::ansi::sound::AnsiMusic),
    Beep,
    OpenLineSound,
    OpenDialSound(bool, String),
    StopSound,
    Reconnect,
    Connect(String),

    AutoTransferTriggered(TransferProtocolType, bool, Option<String>),
    EmsiLogin(Box<EmsiISI>),
    ClearPictureData,
    UpdatePictureData(icy_engine::Size, Vec<u8>, Vec<icy_engine::rip::bgi::MouseField>),
}

#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub connection_info: ConnectionInformation,
    pub terminal_type: TerminalEmulation,
    pub window_size: (u16, u16),
    pub timeout: Duration,

    /// BBS user name - the one in connection info may be empty
    /// or different (e.g. for auto-login)
    pub user_name: Option<String>,

    /// BBS password - the one in connection info may be empty
    /// or different (e.g. for auto-login)
    pub password: Option<String>,

    pub proxy_command: Option<String>,
    pub modem: Option<ModemConfig>,

    pub music_option: icy_engine::ansi::MusicOption,
    pub screen_mode: ScreenMode,

    pub baud_emulation: BaudEmulation,

    // Auto-login configuration
    pub iemsi_auto_login: bool,
    pub login_exp: String,
}

#[derive(Debug, Clone)]
pub struct ModemConfig {
    pub device: String,
    pub baud_rate: u32,
    pub char_size: CharSize,
    pub parity: icy_net::serial::Parity,
    pub stop_bits: icy_net::serial::StopBits,
    pub flow_control: icy_net::serial::FlowControl,
    // Support multiple init lines (safer & closer to original patterns)
    pub init_string: String,
    pub dial_string: String,
}

pub struct TerminalThread {
    // Shared state with UI
    edit_screen: Arc<Mutex<dyn EditableScreen>>,

    // Thread-local state
    connection: Option<Box<dyn Connection>>,
    buffer_parser: Box<dyn BufferParser>,
    current_transfer: Option<TransferState>,
    connection_time: Option<Instant>,
    baud_emulator: BaudEmulator,

    emulated_modem: EmulatedModem,

    // Communication channels
    command_rx: mpsc::UnboundedReceiver<TerminalCommand>,
    event_tx: mpsc::UnboundedSender<TerminalEvent>,

    use_utf8: bool,
    utf8_buffer: Vec<u8>,

    // Auto-features
    auto_file_transfer: AutoFileTransfer,
    auto_login: Option<AutoLogin>,
    auto_transfer: Option<(TransferProtocolType, bool, Option<String>)>, // For pending auto-transfers
}

impl TerminalThread {
    pub fn spawn(
        edit_screen: Arc<Mutex<dyn EditableScreen>>,
        buffer_parser: Box<dyn BufferParser>,
    ) -> (mpsc::UnboundedSender<TerminalCommand>, mpsc::UnboundedReceiver<TerminalEvent>) {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let mut thread = Self {
            edit_screen,
            connection: None,
            buffer_parser,
            current_transfer: None,
            connection_time: None,
            command_rx,
            event_tx: event_tx.clone(),
            use_utf8: false,
            utf8_buffer: Vec::new(),
            auto_file_transfer: AutoFileTransfer::default(),
            baud_emulator: BaudEmulator::new(),
            auto_login: None,
            auto_transfer: None,
            emulated_modem: EmulatedModem::default(),
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
        let mut poll_interval = 0;

        loop {
            tokio::select! {
                // Handle commands from UI
                Some(cmd) = self.command_rx.recv() => {
                    self.handle_command(cmd).await;
                }

                // Periodic tick for updates and reading
                _ = interval.tick() => {
                    // Process any buffered data from baud emulation first
                    if self.baud_emulator.has_buffered_data() {
                        let data = self.baud_emulator.emulate(&[]);
                        if !data.is_empty() {
                            self.process_data(&data).await;
                            let _ = self.event_tx.send(TerminalEvent::DataReceived(data));
                        }
                    }

                    // Check for pending auto-transfers
                    if let Some((protocol, is_download, filename)) = self.auto_transfer.take() {
                        if is_download {
                            self.start_download(protocol, filename).await;
                        } else {
                            // For uploads, we'd need file selection - just notify UI
                            let _ = self.event_tx.send(TerminalEvent::AutoTransferTriggered(protocol, is_download, filename));
                        }
                    }

                    // Read from connection if connected
                    if self.connection.is_some() {
                        // Handle ongoing file transfers
                        if let Some(transfer) = &mut self.current_transfer {
                            if !transfer.is_finished {
                                continue; // Skip normal reading during transfers
                            }
                        }

                        if poll_interval >= 10 {
                            poll_interval = 0;
                            if let Some(conn) = &mut self.connection {
                                match conn.poll().await {
                                    Ok(state) => {
                                        if state == ConnectionState::Disconnected {
                                            self.disconnect().await;
                                            continue;
                                        }
                                    }
                                    Err(e) => {
                                        error!("Connection poll error: {}", e);
                                        self.disconnect().await;
                                        self.process_data(format!("\n\r{}", e).as_bytes()).await;
                                        continue;
                                    }
                                }
                            }
                        } else {
                            poll_interval += 1;
                        }

                        let data = self.read_connection(&mut read_buffer).await;
                        if data > 0 && self.buffer_parser.has_renederer() {
                            if self.buffer_parser.picture_is_empty() {
                                let _ = self.event_tx.send(TerminalEvent::ClearPictureData);
                            } else if let Some((size, data)) = self.buffer_parser.get_picture_data() {
                                let _ = self.event_tx.send(TerminalEvent::UpdatePictureData(size, data, self.buffer_parser.get_mouse_fields()));
                            }
                        }
                    }
                }
            }
        }
    }

    fn perform_resize(&mut self, width: u16, height: u16) {
        if let Ok(mut state) = self.edit_screen.lock() {
            state.set_size(icy_engine::Size::new(width as i32, height as i32));
        }
        // Optionally notify UI so layout can adjust
        let _ = self.event_tx.send(TerminalEvent::BufferUpdated);
    }

    async fn handle_command(&mut self, command: TerminalCommand) {
        match command {
            TerminalCommand::Connect(config) => {
                if let Err(e) = self.connect(config).await {
                    self.process_data(format!("NO CARRIER\r\n").as_bytes()).await;

                    if let Err(err) = self.event_tx.send(TerminalEvent::Disconnected(Some(e.to_string()))) {
                        log::error!("Failed to send disconnect event: {}", err);
                        self.process_data(format!("{}", err).as_bytes()).await;
                    }
                }
            }
            TerminalCommand::Disconnect => {
                self.disconnect().await;
            }
            TerminalCommand::SendData(data) => {
                if let Some(conn) = &mut self.connection {
                    if let Err(err) = conn.send(&data).await {
                        log::error!("Failed to send data: {}", err);
                        self.disconnect().await;
                        self.process_data(format!("\n\r{}", err).as_bytes()).await;
                    }
                } else {
                    // Echo locally
                    match self.emulated_modem.process_local_input(&data) {
                        ModemCommand::Nothing => {}
                        ModemCommand::Output(output) => {
                            self.process_data(&output).await;
                        }
                        ModemCommand::PlayLineSound => {
                            let _ = self.event_tx.send(TerminalEvent::OpenLineSound);
                        }
                        ModemCommand::PlayDialSound(tone_dial, phone_number) => {
                            let _ = self.event_tx.send(TerminalEvent::OpenDialSound(tone_dial, phone_number));
                        }
                        ModemCommand::StopSound => {
                            let _ = self.event_tx.send(TerminalEvent::StopSound);
                        }
                        ModemCommand::Reconnect => {
                            self.process_data(b"\r\nRECONNECT...\r\n").await;
                            let _ = self.event_tx.send(TerminalEvent::Reconnect);
                        }
                        ModemCommand::Connect(address) => {
                            self.process_data(format!("\r\nCALLING...\r\n").as_bytes()).await;
                            let _ = self.event_tx.send(TerminalEvent::Connect(address));
                        }
                    }
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
            TerminalCommand::SendLogin => {
                self.send_login().await;
            }
            TerminalCommand::SetBaudEmulation(bps) => {
                self.baud_emulator.set_baud_rate(bps);
            }
        }
    }

    async fn connect(&mut self, config: ConnectionConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.connection.is_some() {
            self.disconnect().await;
        }

        self.use_utf8 = config.terminal_type == TerminalEmulation::Utf8Ansi;
        self.baud_emulator.set_baud_rate(config.baud_emulation);
        self.process_data(format!("ATDT{}\r\n", config.connection_info).as_bytes()).await;

        self.setup_auto_login(&config);

        let connection: Box<dyn Connection> = match config.connection_info.protocol() {
            ConnectionType::Telnet => {
                let term_caps = TermCaps {
                    terminal: config.terminal_type,
                    window_size: config.window_size,
                };
                Box::new(TelnetConnection::open(&config.connection_info.endpoint(), term_caps, config.timeout).await?)
            }
            ConnectionType::Raw => Box::new(RawConnection::open(&config.connection_info.endpoint(), config.timeout).await?),
            ConnectionType::SSH => {
                let term_caps = TermCaps {
                    terminal: config.terminal_type,
                    window_size: config.window_size,
                };
                let (user_name, password) = if config.connection_info.user_name().is_some() && config.connection_info.password().is_some() {
                    (config.connection_info.user_name(), config.connection_info.password())
                } else {
                    (config.user_name.clone(), config.password.clone())
                };

                let creds = Credentials {
                    user_name: user_name.unwrap_or_default(),
                    password: password.unwrap_or_default(),
                    proxy_command: config.proxy_command.clone(),
                };
                Box::new(SSHConnection::open(&config.connection_info.endpoint(), term_caps, creds).await?)
            }
            ConnectionType::Modem => {
                let Some(m) = &config.modem else {
                    return Err("Modem configuration is required for modem connections".into());
                };
                let serial = Serial {
                    device: m.device.clone(),
                    baud_rate: m.baud_rate,
                    char_size: m.char_size,
                    parity: m.parity,
                    stop_bits: m.stop_bits,
                    flow_control: m.flow_control,
                };
                let modem = ModemConfiguration {
                    init_string: m.init_string.clone(),
                    dial_string: m.dial_string.clone(),
                };
                Box::new(ModemConnection::open(serial, modem, config.connection_info.host.clone()).await?)
            }
            ConnectionType::Websocket => Box::new(icy_net::websocket::connect(&config.connection_info.endpoint(), false).await?),
            ConnectionType::SecureWebsocket => Box::new(icy_net::websocket::connect(&config.connection_info.endpoint(), true).await?),
            ConnectionType::Rlogin => {
                let rlogin_config = RloginConfig {
                    user_name: config.user_name.clone().unwrap_or_default(),
                    password: config.password.clone().unwrap_or_default(),
                    terminal_emulation: config.terminal_type,
                    swapped: false,
                    escape_sequence: None,
                };
                Box::new(icy_net::rlogin::RloginConnection::open(&config.connection_info.endpoint(), rlogin_config, config.timeout).await?)
            }
            ConnectionType::RloginSwapped => {
                let rlogin_config = RloginConfig {
                    user_name: config.user_name.clone().unwrap_or_default(),
                    password: config.password.clone().unwrap_or_default(),
                    terminal_emulation: config.terminal_type,
                    swapped: true,
                    escape_sequence: None,
                };
                Box::new(icy_net::rlogin::RloginConnection::open(&config.connection_info.endpoint(), rlogin_config, config.timeout).await?)
            }
            other => {
                return Err(format!("Unsupported connection type: {other:?}").into());
            }
        };

        self.connection = Some(connection);
        self.connection_time = Some(Instant::now());
        self.buffer_parser = crate::get_parser(&config.terminal_type, config.music_option, config.screen_mode, PathBuf::from(".cache"));
        // Reset auto-transfer state
        self.auto_file_transfer = AutoFileTransfer::default();

        let _ = self.event_tx.send(TerminalEvent::Connected);
        Ok(())
    }

    fn setup_auto_login(&mut self, config: &ConnectionConfig) {
        if !config.iemsi_auto_login {
            self.auto_login = None;
            return;
        }

        // Determine effective credentials with clear precedence
        let mut effective_user = config.user_name.as_ref().filter(|s| !s.is_empty()).cloned().or_else(|| {
            if config.connection_info.protocol() != ConnectionType::SSH {
                config.connection_info.user_name()
            } else {
                None
            }
        });

        let mut effective_pass = config.password.as_ref().filter(|s| !s.is_empty()).cloned().or_else(|| {
            if config.connection_info.protocol() != ConnectionType::SSH {
                config.connection_info.password()
            } else {
                None
            }
        });

        // Normalize empty strings to None
        if let Some(u) = &effective_user {
            if u.trim().is_empty() {
                effective_user = None;
            }
        }
        if let Some(p) = &effective_pass {
            if p.trim().is_empty() {
                effective_pass = None;
            }
        }

        // Decide auto-login (requires BOTH credentials and non-SSH)
        if effective_user.is_some() && effective_pass.is_some() {
            self.auto_login = Some(AutoLogin::new(
                config.login_exp.clone(),
                effective_user.clone().unwrap(),
                effective_pass.clone().unwrap(),
            ));
        }
    }

    async fn disconnect(&mut self) {
        if let Some(mut conn) = self.connection.take() {
            let _ = conn.shutdown().await;
        }
        if let Ok(mut state) = self.edit_screen.lock() {
            state.caret_mut().set_attr(TextAttribute::default());
        }
        self.process_data(b"\r\nNO CARRIER\r\n").await;

        self.baud_emulator = BaudEmulator::new();
        self.connection_time = None;
        self.utf8_buffer.clear();
        self.auto_login = None;
        self.auto_file_transfer = AutoFileTransfer::default();
        let _ = self.event_tx.send(TerminalEvent::Disconnected(None));
    }

    async fn send_login(&mut self) {
        if let Some(auto_login) = &self.auto_login {
            if let Some(conn) = &mut self.connection {
                // Send username, wait briefly, then password
                // Some BBSes need a delay between username and password
                let _ = conn.send(format!("{}\r", auto_login.user_name).as_bytes()).await;
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                let _ = conn.send(format!("{}\r", auto_login.password).as_bytes()).await;
            }
        }
    }

    async fn read_connection(&mut self, buffer: &mut [u8]) -> usize {
        if let Some(conn) = &mut self.connection {
            match conn.try_read(buffer).await {
                Ok(0) => 0,
                Ok(size) => {
                    let mut data = buffer[..size].to_vec();

                    // Apply baud emulation if enabled
                    data = self.baud_emulator.emulate(&data);

                    if !data.is_empty() {
                        self.process_data(&data).await;
                        let _ = self.event_tx.send(TerminalEvent::DataReceived(data));
                    }
                    size
                }
                Err(e) => {
                    error!("Connection read error: {e}");
                    self.disconnect().await;
                    self.process_data(format!("\n\r{}", e).as_bytes()).await;
                    0
                }
            }
        } else {
            0
        }
    }

    #[async_recursion::async_recursion(?Send)]
    async fn process_data(&mut self, data: &[u8]) {
        let mut actions = Vec::new();

        if let Ok(mut screen) = self.edit_screen.lock() {
            let caret = screen.caret().clone();
            {
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

                    // Process all complete characters with auto-features
                    for ch in to_process {
                        // Check for auto-file transfer triggers
                        if let Some((protocol_type, download)) = self.auto_file_transfer.try_transfer(ch as u8) {
                            self.auto_transfer = Some((protocol_type, download, None));
                        }

                        let mut logged_in = false;
                        if let Some(autologin) = &mut self.auto_login {
                            if let Ok(Some(login_data)) = autologin.try_login(ch as u8) {
                                if let Some(conn) = &mut self.connection {
                                    let _ = conn.send(&login_data).await;
                                }
                                if let Some(isi) = &autologin.iemsi.isi {
                                    let _ = self.event_tx.send(TerminalEvent::EmsiLogin(Box::new(isi.clone())));
                                }
                            }
                            logged_in = autologin.is_logged_in();
                        }
                        if logged_in {
                            self.auto_login = None;
                        }

                        match self.buffer_parser.print_char(&mut *screen, ch) {
                            Ok(action) => actions.push(action),
                            Err(e) => error!("Parser error: {e}"),
                        }
                    }
                } else {
                    // Legacy mode: treat each byte as a character (CP437 or similar)
                    for &byte in data {
                        // Check for auto-file transfer triggers
                        if let Some((protocol_type, download)) = self.auto_file_transfer.try_transfer(byte) {
                            self.auto_transfer = Some((protocol_type, download, None));
                        }

                        let mut logged_in = false;
                        if let Some(autologin) = &mut self.auto_login {
                            if let Ok(Some(login_data)) = autologin.try_login(byte) {
                                if let Some(conn) = &mut self.connection {
                                    let _ = conn.send(&login_data).await;
                                }
                            }
                            if let Some(isi) = &autologin.iemsi.isi {
                                let _ = self.event_tx.send(TerminalEvent::EmsiLogin(Box::new(isi.clone())));
                            }
                            logged_in = autologin.is_logged_in();
                        }

                        if logged_in {
                            self.auto_login = None;
                        }

                        match self.buffer_parser.print_char(&mut *screen, byte as char) {
                            Ok(action) => actions.push(action),
                            Err(e) => error!("Parser error: {e}"),
                        }
                    }
                }

                screen.update_hyperlinks();
            }
            *screen.caret_mut() = caret;

            while screen.sixel_threads_runnning() {
                thread::sleep(Duration::from_millis(50));
                let _ = screen.update_sixel_threads();
            }
        }

        for action in actions {
            self.handle_parser_action(action).await;
        }

        let _ = self.event_tx.send(TerminalEvent::BufferUpdated);
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
            CallbackAction::PlayMusic(music) => {
                let _ = self.event_tx.send(TerminalEvent::PlayMusic(music));
            }
            CallbackAction::Beep => {
                let _ = self.event_tx.send(TerminalEvent::Beep);
            }
            CallbackAction::ResizeTerminal(width, height) => {
                // Avoid async recursion by calling sync helper
                self.perform_resize(width as u16, height as u16);
            }
            CallbackAction::XModemTransfer(file_name) => {
                // Set up auto-transfer for X-Modem
                self.auto_transfer = Some((TransferProtocolType::XModem, true, Some(file_name)));
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
                    let _ = self.event_tx.send(TerminalEvent::TransferStarted(state.clone(), false));

                    // Run the file transfer
                    if let Err(e) = self.run_file_transfer(prot.as_mut(), state).await {
                        let _ = self.event_tx.send(TerminalEvent::Error(format!("Transfer failed: {}", e)));
                    }
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
                    let _ = self.event_tx.send(TerminalEvent::TransferStarted(state.clone(), true));

                    // Run the file transfer
                    if let Err(e) = self.run_file_transfer(prot.as_mut(), state).await {
                        let _ = self.event_tx.send(TerminalEvent::Error(format!("Transfer failed: {}", e)));
                    }
                }
                Err(e) => {
                    let _ = self.event_tx.send(TerminalEvent::Error(format!("Failed to start download: {}", e)));
                }
            }
        }
    }

    async fn run_file_transfer(&mut self, prot: &mut dyn Protocol, mut transfer_state: TransferState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut last_progress_update = Instant::now();

        // Temporarily disable baud emulation for file transfers if desired
        // Or keep it enabled for authentic experience
        let transfer_baud_emulation = self.baud_emulator.baud_emulation; // Store current setting

        // Optional: You might want to apply different rates for file transfers
        // For example, file transfers often used hardware flow control and could achieve
        // closer to the theoretical maximum rate

        while !transfer_state.is_finished {
            // Check for cancel command
            if let Ok(command) = self.command_rx.try_recv() {
                if matches!(command, TerminalCommand::CancelTransfer) {
                    transfer_state.is_finished = true;
                    if let Some(conn) = &mut self.connection {
                        prot.cancel_transfer(&mut **conn).await?;
                    }
                    break;
                }
            }

            // Update transfer
            if let Some(conn) = &mut self.connection {
                // If baud emulation is active, we might want to slow down the transfer
                // This depends on whether the protocol handles its own timing
                if transfer_baud_emulation != BaudEmulation::Off {
                    // Add a small delay based on baud rate
                    if let BaudEmulation::Rate(bps) = transfer_baud_emulation {
                        // Calculate delay for typical block size (e.g., 1K for XModem)
                        let block_size = 1024.0; // bytes
                        let bytes_per_second = bps as f64 / 10.0;
                        let delay_ms = (block_size / bytes_per_second * 1000.0) as u64;

                        // Add a small delay to simulate transfer speed
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms.min(100))).await;
                    }
                }

                prot.update_transfer(&mut **conn, &mut transfer_state).await?;

                // Send progress updates every 500ms
                if last_progress_update.elapsed() > Duration::from_millis(500) {
                    self.current_transfer = Some(transfer_state.clone());
                    let _ = self.event_tx.send(TerminalEvent::TransferProgress(transfer_state.clone()));
                    last_progress_update = Instant::now();
                }
            }
        }

        // Copy downloaded files to the download directory
        copy_downloaded_files(&mut transfer_state)?;

        self.current_transfer = Some(transfer_state.clone());
        let _ = self.event_tx.send(TerminalEvent::TransferCompleted(transfer_state));
        self.current_transfer = None;

        Ok(())
    }
}

// Helper function to create a terminal thread for the UI
pub fn create_terminal_thread(
    edit_screen: Arc<Mutex<dyn EditableScreen>>,
    terminal_type: TerminalEmulation,
) -> (mpsc::UnboundedSender<TerminalCommand>, mpsc::UnboundedReceiver<TerminalEvent>) {
    use icy_engine::ansi::MusicOption;
    let parser = crate::get_parser(
        &terminal_type,
        MusicOption::Off,
        ScreenMode::default(),
        PathBuf::from(".cache"), // cache directory
    );

    TerminalThread::spawn(edit_screen, parser)
}

fn copy_downloaded_files(transfer_state: &mut TransferState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(dirs) = UserDirs::new() {
        if let Some(upload_location) = dirs.download_dir() {
            let mut lines = Vec::new();
            for (name, path) in &transfer_state.recieve_state.finished_files {
                let mut dest = upload_location.join(name);

                let mut i = 1;
                let new_name = PathBuf::from(name);
                while dest.exists() {
                    if let Some(stem) = new_name.file_stem() {
                        if let Some(ext) = new_name.extension() {
                            dest = dest.with_file_name(format!("{}.{}.{}", stem.to_string_lossy(), i, ext.to_string_lossy()));
                        } else {
                            dest = dest.with_file_name(format!("{}.{}", stem.to_string_lossy(), i));
                        }
                    }
                    i += 1;
                }
                std::fs::copy(&path, &dest)?;
                std::fs::remove_file(&path)?;
                lines.push(format!("File copied to: {}", dest.display()));
            }
            for line in lines {
                transfer_state.recieve_state.log_info(line);
            }
        } else {
            error!("Failed to get user download directory");
        }
    } else {
        error!("Failed to get user directories");
    }

    Ok(())
}
