use crate::ConnectionInformation;
use crate::TransferProtocol;
use crate::emulated_modem::{EmulatedModem, ModemCommand};
use crate::features::{AutoTransferScanner, IEmsiAutoLogin};
use crate::scripting::ScriptRunner;
use crate::ui::open_serial_dialog::BAUD_RATES;
use directories::UserDirs;
use icy_engine::{CreationOptions, GraphicsType, Screen, ScreenMode, ScreenSink, Sixel};
use icy_engine_gui::music::sound_effects::sound_data;
use icy_engine_gui::util::BaudEmulator;
use icy_engine_gui::util::QueuedCommand;
use icy_engine_gui::util::QueueingSink;
use icy_net::iemsi::EmsiISI;
use icy_net::rlogin::RloginConfig;
use icy_net::{
    Connection, ConnectionState, ConnectionType,
    modem::{ModemConfiguration, ModemConnection},
    protocol::{Protocol, TransferState},
    raw::RawConnection,
    serial::{Serial, SerialConnection},
    ssh::{Credentials, SSHConnection},
    telnet::{TelnetConnection, TermCaps, TerminalEmulation},
};
use icy_parser_core::*;
use icy_parser_core::{AnsiMusic, CommandParser, TerminalRequest};
use icy_parser_core::{BaudEmulation, MusicOption};
use log::error;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::mpsc;

/// Minimum pause duration in milliseconds to display in status bar
const MIN_PAUSE_DISPLAY_MS: u64 = 500;

/// Messages sent to the terminal thread
#[derive(Debug, Clone)]
pub enum TerminalCommand {
    Connect(ConnectionConfig),
    OpenSerial(Serial),
    AutoDetectSerial(Serial),
    Disconnect,
    SendData(Vec<u8>),
    StartUpload(TransferProtocol, Vec<PathBuf>),
    StartDownload(TransferProtocol, Option<String>),
    CancelTransfer,
    Resize(u16, u16),
    SetBaudEmulation(BaudEmulation),
    StartCapture(String),
    StopCapture,
    SetDownloadDirectory(PathBuf),
    /// Run a Lua script file
    RunScript(PathBuf),
    /// Run Lua script code directly (from string)
    RunScriptCode(String),
    /// Stop the currently running script
    StopScript,
    /// Change terminal settings (terminal type, screen mode, ansi music) during session
    SetTerminalSettings {
        terminal_type: TerminalEmulation,
        screen_mode: ScreenMode,
        ansi_music: MusicOption,
    },
}

/// Messages sent from the terminal thread to the UI
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    Connected,
    Disconnected(Option<String>), // Optional error message
    TransferStarted(TransferState, bool),
    TransferProgress(TransferState),
    TransferCompleted(TransferState),
    /// External protocol transfer started (protocol_name, is_download)
    ExternalTransferStarted(String, bool),
    /// External protocol transfer completed (protocol_name, is_download, success, error_message)
    ExternalTransferCompleted(String, bool, bool, Option<String>),
    Error(String, String),
    PlayMusic(AnsiMusic),
    Beep,
    OpenLineSound,
    OpenDialSound(bool, String),
    StopSound,
    Reconnect,
    Connect(String),
    /// Send credentials from current address (mode: 0=both, 1=username, 2=password)
    SendCredentials(i32),

    AutoTransferTriggered(String, bool, Option<String>),
    EmsiLogin(Box<EmsiISI>),

    /// Play a GIST sound effect (BellsAndWhistles)
    PlayGist(Vec<i16>),
    /// Play chip music on a specific voice
    PlayChipMusic {
        sound_data: Vec<i16>,
        voice: u8,
        volume: u8,
        pitch: u8,
    },

    InformDelay(u64), // Delay in milliseconds
    ContinueAfterDelay,

    /// Fade out sound on specific voice (soft stop)
    SndOff(u8),
    /// Immediately stop sound on specific voice (hard stop)
    StopSnd(u8),
    /// Fade out all voices
    SndOffAll,
    /// Immediately stop all voices
    StopSndAll,

    /// Terminal settings have been changed
    TerminalSettingsChanged {
        terminal_type: TerminalEmulation,
        screen_mode: ScreenMode,
        ansi_music: MusicOption,
    },

    /// Script execution started
    ScriptStarted(PathBuf),
    /// Script execution finished
    ScriptFinished(Result<(), String>),
    /// Request to quit the application
    Quit,
    /// Serial baud rate detected
    SerialBaudDetected(u32),
    /// Serial auto-detection complete (even if failed)
    SerialAutoDetectComplete,
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
    pub modem: Option<ModemConfiguration>,

    pub ansi_music: MusicOption,
    pub screen_mode: ScreenMode,

    pub baud_emulation: BaudEmulation,

    // Auto-login configuration
    pub iemsi_auto_login: bool,
    pub auto_login_exp: String,
    pub max_scrollback_lines: usize,

    /// Transfer protocols for auto-transfer detection
    pub transfer_protocols: Vec<crate::TransferProtocol>,
}

pub struct TerminalThread {
    // Shared state with UI
    edit_screen: Arc<Mutex<Box<dyn Screen>>>,

    // Thread-local state
    connection: Option<Box<dyn Connection>>,
    parser: Box<dyn CommandParser + Send>,
    current_transfer: Option<TransferState>,
    connection_time: Option<Instant>,
    baud_emulator: BaudEmulator,

    emulated_modem: EmulatedModem,

    // Communication channels
    command_rx: mpsc::UnboundedReceiver<TerminalCommand>,
    command_tx: mpsc::UnboundedSender<TerminalCommand>,
    event_tx: mpsc::UnboundedSender<TerminalEvent>,

    use_utf8: bool,
    utf8_buffer: Vec<u8>,

    // Auto-features
    auto_transfer_scanner: AutoTransferScanner,
    iemsi_auto_login: Option<IEmsiAutoLogin>,
    auto_transfer: Option<(String, bool, Option<String>)>, // For pending auto-transfers (protocol_id, is_download, filename)
    transfer_protocols: Vec<TransferProtocol>,             // Stored protocol list for auto-transfer lookup

    // Capture state with buffering
    capture_writer: Option<BufWriter<tokio::fs::File>>,

    // Command queue for granular locking
    command_queue: VecDeque<QueuedCommand>,

    // Download directory
    download_directory: Option<PathBuf>,

    /// Double-stepping mode for IGS G commands (0 = off, 1-3 = vsync delays)
    double_step_vsyncs: Option<u8>,

    /// Script runner for Lua scripts
    script_runner: Option<ScriptRunner>,

    /// Address book for scripting
    address_book: Arc<Mutex<crate::data::AddressBook>>,

    /// Current terminal emulation type (shared for scripting)
    terminal_emulation: Arc<Mutex<icy_net::telnet::TerminalEmulation>>,

    // IGS sound state
    /// Loop count for effects 0-4
    igs_effect_loop: u32,
    /// Mutable copy of all 20 IGS sound effects (can be altered at runtime)
    igs_sound_data: Vec<Vec<i16>>,
}

impl TerminalThread {
    pub fn spawn(
        edit_screen: Arc<Mutex<Box<dyn Screen>>>,
        parser: Box<dyn CommandParser + Send>,
        address_book: Arc<Mutex<crate::data::AddressBook>>,
    ) -> (mpsc::UnboundedSender<TerminalCommand>, mpsc::UnboundedReceiver<TerminalEvent>) {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let mut thread = Self {
            edit_screen,
            connection: None,
            parser,
            current_transfer: None,
            connection_time: None,
            command_rx,
            command_tx: command_tx.clone(),
            event_tx: event_tx.clone(),
            use_utf8: false,
            utf8_buffer: Vec::new(),
            auto_transfer_scanner: AutoTransferScanner::default(),
            transfer_protocols: Vec::new(),
            baud_emulator: BaudEmulator::new(),
            iemsi_auto_login: None,
            auto_transfer: None,
            emulated_modem: EmulatedModem::default(),
            capture_writer: None,
            command_queue: VecDeque::new(),
            download_directory: None,
            double_step_vsyncs: None,
            script_runner: None,
            address_book,
            terminal_emulation: Arc::new(Mutex::new(icy_net::telnet::TerminalEmulation::Ansi)),
            igs_effect_loop: 5,
            igs_sound_data: Self::init_sound_data(),
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

    /// Initialize mutable copy of all 20 IGS sound effects
    fn init_sound_data() -> Vec<Vec<i16>> {
        (0..20)
            .map(|i| sound_data(i).map(|data| data.to_vec()).unwrap_or_else(|| vec![0i16; 56]))
            .collect()
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
                    // Check if script finished
                    self.check_script_finished();

                    // Process any buffered data from baud emulation first
                    if self.baud_emulator.has_buffered_data() {
                        let data = self.baud_emulator.emulate(Vec::new());
                        if !data.is_empty() {
                            self.write_to_capture(&data).await;
                            self.process_data(&data).await;
                        }
                    }

                    // Check for pending auto-transfers
                    if let Some((protocol_id, is_download, filename)) = self.auto_transfer.take() {
                        // First try to find protocol in the stored list, then fall back to internal protocols
                        let protocol = self.transfer_protocols.iter()
                            .find(|p| p.id == protocol_id)
                            .cloned()
                            .or_else(|| TransferProtocol::from_internal_id(&protocol_id));

                        if let Some(protocol) = protocol {
                            if is_download {
                                self.start_download(protocol, filename).await;
                            } else {
                                // For uploads, we'd need file selection - just notify UI
                                self.send_event(TerminalEvent::AutoTransferTriggered(protocol_id, is_download, filename));
                            }
                        } else {
                            log::warn!("Unknown protocol id for auto-transfer: {}", protocol_id);
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

                        if poll_interval >= 3 {  // Poll every ~48ms (3 * 16ms) instead of 160ms
                            poll_interval = 0;
                            if let Some(conn) = &mut self.connection {
                                match conn.poll().await {
                                    Ok(state) => {
                                        if state == ConnectionState::Disconnected {
                                            log::error!("Poll failed.");
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

                        let _ = self.read_connection(&mut read_buffer).await;
                    }
                }
            }
        }
    }

    fn perform_resize(&mut self, width: u16, height: u16) {
        let mut state = self.edit_screen.lock();
        if let Some(editable) = state.as_editable() {
            editable.set_size(icy_engine::Size::new(width as i32, height as i32));
        }
    }

    async fn handle_command(&mut self, command: TerminalCommand) {
        match command {
            TerminalCommand::Connect(config) => {
                // let auto_login = config.auto_login_exp.to_string();
                // let user_name = config.user_name.clone();
                // let password = config.password.clone();

                if let Err(e) = self.connect(config).await {
                    log::error!("{}", e);
                    self.process_data(format!("NO CARRIER\r\n").as_bytes()).await;
                    self.send_event(TerminalEvent::Disconnected(Some(e.to_string())));
                }
                /*
                if !auto_login.is_empty() {
                    match AutoLoginParser::parse(&auto_login) {
                        Ok(commands) => {
                            self.auto_login(&commands, user_name, password).await;
                        }
                        Err(err) => {
                            log::error!("Failed to parse auto-login expression: {}", err);
                        }
                    }
                }*/
            }

            TerminalCommand::OpenSerial(serial) => {
                if let Err(e) = self.open_serial(serial).await {
                    log::error!("{}", e);
                    self.process_data(format!("FAILED.\r\n").as_bytes()).await;
                    self.send_event(TerminalEvent::Disconnected(Some(e.to_string())));
                }
            }

            TerminalCommand::AutoDetectSerial(serial) => {
                self.auto_detect_serial(serial).await;
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
                            self.send_event(TerminalEvent::OpenLineSound);
                        }
                        ModemCommand::PlayDialSound(tone_dial, phone_number) => {
                            self.send_event(TerminalEvent::OpenDialSound(tone_dial, phone_number));
                        }
                        ModemCommand::StopSound => {
                            self.send_event(TerminalEvent::StopSound);
                        }
                        ModemCommand::Reconnect => {
                            self.process_data(b"\r\nRECONNECT...\r\n").await;
                            self.send_event(TerminalEvent::Reconnect);
                        }
                        ModemCommand::Connect(address) => {
                            self.process_data(format!("\r\nCALLING...\r\n").as_bytes()).await;
                            self.send_event(TerminalEvent::Connect(address));
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
            TerminalCommand::SetBaudEmulation(bps) => {
                self.baud_emulator.set_baud_rate(bps);
            }
            TerminalCommand::StartCapture(file_name) => match tokio::fs::File::create(&file_name).await {
                Ok(file) => {
                    self.capture_writer = Some(BufWriter::new(file));
                }
                Err(e) => {
                    log::error!("Failed to create capture file {}: {}", file_name, e);
                    self.send_event(TerminalEvent::Error(format!("Failed to create capture file: {}", file_name), format!("{}", e)));
                }
            },
            TerminalCommand::StopCapture => {
                if let Some(mut writer) = self.capture_writer.take() {
                    let _ = writer.flush().await; // Ensure final flush
                }
            }
            TerminalCommand::SetDownloadDirectory(dir) => {
                self.download_directory = Some(dir);
            }
            TerminalCommand::RunScript(path) => {
                self.run_script(path);
            }
            TerminalCommand::RunScriptCode(code) => {
                self.run_script_code(code);
            }
            TerminalCommand::StopScript => {
                self.stop_script();
            }
            TerminalCommand::SetTerminalSettings {
                terminal_type,
                screen_mode,
                ansi_music,
            } => {
                self.set_terminal_settings(terminal_type, screen_mode, ansi_music).await;
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
                Box::new(ModemConnection::open(m.clone(), config.connection_info.host.clone()).await?)
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

        let (mut new_screen, parser) = config
            .screen_mode
            .create_screen(config.terminal_type, Some(CreationOptions { ansi_music: config.ansi_music }));
        {
            new_screen.set_scrollback_buffer_size(config.max_scrollback_lines);
            let mut screen = self.edit_screen.lock();
            *screen = new_screen;
        }
        self.parser = parser;
        // Update terminal emulation for scripting
        *self.terminal_emulation.lock() = config.terminal_type;
        // Build auto-transfer scanner from protocol list and store protocols for later lookup
        self.transfer_protocols = config.transfer_protocols.clone();
        self.auto_transfer_scanner = AutoTransferScanner::from_protocols(&self.transfer_protocols);
        self.send_event(TerminalEvent::Connected);

        Ok(())
    }

    async fn open_serial(&mut self, serial: Serial) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.connection.is_some() {
            self.disconnect().await;
        }

        self.process_data(format!("Opening serial port {}...\r\n", serial.device).as_bytes()).await;

        let connection: Box<dyn Connection> = Box::new(SerialConnection::open(serial)?);

        self.connection = Some(connection);
        self.connection_time = Some(Instant::now());

        self.send_event(TerminalEvent::Connected);

        Ok(())
    }

    async fn auto_detect_serial(&mut self, serial: Serial) {
        // Disconnect any existing connection first
        if self.connection.is_some() {
            self.disconnect().await;
        }

        self.process_data(format!("Auto-detecting baud rate on {}...\r\n", serial.device).as_bytes())
            .await;

        // Common baud rates to try, ordered by likelihood
        let mut detected_baud: Option<u32> = None;

        for &baud_rate in BAUD_RATES.iter().rev() {
            let mut test_serial = serial.clone();
            test_serial.baud_rate = baud_rate;

            self.process_data(format!("Trying {} baud...", baud_rate).as_bytes()).await;

            match SerialConnection::open(test_serial) {
                Ok(mut conn) => {
                    // Send a CR and wait briefly for response
                    if conn.send(b"Hello World\r").await.is_ok() {
                        // Try to read any response
                        let found_res = self.try_read_response(&mut detected_baud, baud_rate, &mut conn).await;
                        // Explicitly close connection before trying next baud rate
                        let _ = conn.shutdown().await;
                        drop(conn);
                        if found_res {
                            break;
                        }
                    }
                    // Small delay to let the port fully close
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(_) => {
                    self.process_data(format!(" failed to open\r\n").as_bytes()).await;
                }
            }
        }

        if let Some(baud) = detected_baud {
            self.send_event(TerminalEvent::SerialBaudDetected(baud));
        } else {
            self.process_data(format!("Auto-detection complete. No response detected.\r\n").as_bytes())
                .await;
        }
        // Always notify that auto-detection is complete so the dialog can be shown again
        self.send_event(TerminalEvent::SerialAutoDetectComplete);
    }

    async fn try_read_response(&mut self, detected_baud: &mut Option<u32>, baud_rate: u32, conn: &mut SerialConnection) -> bool {
        let mut buf = [0u8; 64];
        for _ in 0..3 {
            match tokio::time::timeout(Duration::from_millis(1000), conn.read(&mut buf)).await {
                Ok(Ok(n)) if n > 0 => {
                    // Check if response contains printable ASCII (valid at this baud rate)
                    let printable_count = buf[..n]
                        .iter()
                        .filter(|&&b| (b as char).is_ascii_alphabetic() || b == b'\r' || b == b'\n')
                        .count();
                    // println!("Read {}/{} bytes at {} baud: {}", n, printable_count, baud_rate, String::from_utf8_lossy(&buf[..n]));
                    if printable_count == n {
                        // More than half printable - likely correct baud rate
                        self.process_data(format!(" detected!\r\n").as_bytes()).await;
                        *detected_baud = Some(baud_rate);
                        let _ = conn.shutdown().await;
                        return true;
                    } else {
                        self.process_data(format!(" garbage response\r\n").as_bytes()).await;
                        return false;
                    }
                }
                _ => {
                    self.process_data(format!(" no response\r\n").as_bytes()).await;
                    return false;
                }
            }
        }
        return false;
    }

    fn setup_auto_login(&mut self, config: &ConnectionConfig) {
        if !config.iemsi_auto_login {
            self.iemsi_auto_login = None;
            return;
        }

        // Determine effective credentials with clear precedence
        let mut effective_user = config.user_name.as_ref().filter(|s: &&String| !s.is_empty()).cloned().or_else(|| {
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
            let user = effective_user.clone().unwrap();
            let pass = effective_pass.clone().unwrap();
            if !user.is_empty() && !pass.is_empty() {
                self.iemsi_auto_login = Some(IEmsiAutoLogin::new(user, pass));
            }
        }
    }

    async fn disconnect(&mut self) {
        if let Some(mut conn) = self.connection.take() {
            let _ = conn.shutdown().await;
        }
        {
            let mut state = self.edit_screen.lock();
            if let Some(editable) = state.as_editable() {
                editable.caret_default_colors();
            }
        }
        self.process_data(b"\r\nNO CARRIER\r\n").await;

        self.baud_emulator = BaudEmulator::new();
        self.connection_time = None;
        self.utf8_buffer.clear();
        self.iemsi_auto_login = None;
        self.auto_transfer_scanner = AutoTransferScanner::default();
        self.transfer_protocols.clear();
        self.send_event(TerminalEvent::Disconnected(None));
    }

    async fn read_connection(&mut self, buffer: &mut [u8]) -> Vec<u8> {
        if let Some(conn) = &mut self.connection {
            match conn.try_read(buffer).await {
                Ok(0) => Vec::new(),
                Ok(size) => {
                    let mut data = buffer[..size].to_vec();
                    // Apply baud emulation if enabled
                    data = self.baud_emulator.emulate(data);
                    if !data.is_empty() {
                        self.write_to_capture(&data).await;
                        self.process_data(&data).await;
                    }
                    data
                }
                Err(e) => {
                    error!("Connection read error: {e}");
                    self.disconnect().await;
                    self.process_data(format!("\n\r{}", e).as_bytes()).await;
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        }
    }

    #[async_recursion::async_recursion(?Send)]
    async fn process_data(&mut self, data: &[u8]) {
        // Check for auto-features before parsing
        for &byte in data {
            if let Some((protocol_id, is_download)) = self.auto_transfer_scanner.try_transfer(byte) {
                self.auto_transfer = Some((protocol_id, is_download, None));
            }

            let mut logged_in = false;
            if let Some(autologin) = &mut self.iemsi_auto_login {
                if let Ok(Some(login_data)) = autologin.try_login(byte) {
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
                self.iemsi_auto_login = None;
            }
        }

        // Parse data into command queue (reuse existing sink to preserve queue)
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
                        to_process.extend_from_slice(valid_str.as_bytes());
                        i = self.utf8_buffer.len(); // Consumed everything
                    }
                    Err(e) => {
                        // Partial UTF-8 sequence or error
                        if e.valid_up_to() > 0 {
                            // Process the valid part
                            to_process.extend_from_slice(&remaining[..e.valid_up_to()]);
                            i += e.valid_up_to();
                        }

                        // Check if we have an incomplete sequence at the end
                        if let Some(error_len) = e.error_len() {
                            // Invalid UTF-8 sequence - but it might be intentional high-ASCII!
                            // In BBS/ANSI context, bytes 128-255 are often CP437 characters
                            // not UTF-8. Only replace if we're sure it's supposed to be UTF-8.

                            // For now, pass through the raw bytes instead of replacing
                            // This preserves box-drawing and other high-ASCII characters
                            to_process.extend_from_slice(&remaining[..error_len]);
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

            // Parse the complete UTF-8 data
            let mut sink = QueueingSink::new(&mut self.command_queue);
            self.parser.parse(&to_process, &mut sink);
        } else {
            // Legacy mode: parse bytes directly - this preserves high-ASCII
            let mut sink = QueueingSink::new(&mut self.command_queue);
            self.parser.parse(data, &mut sink);
        }

        // Process the command queue with granular locking
        self.process_command_queue().await;
    }

    /// Check if command needs async processing (delays, sound, etc.)
    /// Returns true if command was handled
    async fn try_process_async_command(&mut self, cmd: &QueuedCommand) -> bool {
        match cmd {
            QueuedCommand::Igs(IgsCommand::Pause { pause_type }) => {
                if pause_type.is_double_step_config() {
                    self.double_step_vsyncs = pause_type.get_double_step_vsyncs();
                } else {
                    let delay_ms = pause_type.ms().min(10_000);
                    if delay_ms > MIN_PAUSE_DISPLAY_MS {
                        self.send_event(TerminalEvent::InformDelay(delay_ms));
                    }
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    if delay_ms > MIN_PAUSE_DISPLAY_MS {
                        self.send_event(TerminalEvent::ContinueAfterDelay);
                    }
                }
                true
            }

            QueuedCommand::Skypix(SkypixCommand::Delay { jiffies }) => {
                let delay_ms = 1000 * (*jiffies) as u64 / 60;
                self.send_event(TerminalEvent::InformDelay(delay_ms));
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                self.send_event(TerminalEvent::ContinueAfterDelay);
                true
            }

            QueuedCommand::Skypix(SkypixCommand::CrcTransfer { mode, filename, .. }) => {
                let file_name = if filename.is_empty() { None } else { Some(filename.clone()) };
                log::info!("SkyPix CRC transfer initiated: mode={:?}, filename={:?}", mode, file_name);
                self.send_event(TerminalEvent::AutoTransferTriggered("@xmodem".to_string(), true, file_name));
                true
            }

            QueuedCommand::Igs(IgsCommand::SetEffectLoops { count }) => {
                self.igs_effect_loop = *count;
                true
            }

            QueuedCommand::Igs(IgsCommand::AlterSoundEffect {
                play,
                sound_effect,
                element_num,
                negative_flag,
                thousands,
                hundreds,
            }) => {
                let snd_num = (*sound_effect as usize).min(19);
                let elem_num = (*element_num as usize).min(55);
                let thousands_clamped = (*thousands as i32).min(32) * 1000;
                let mut value = thousands_clamped + (*hundreds as i32);
                if *negative_flag != 0 {
                    value = -value;
                }
                if let Some(sound) = self.igs_sound_data.get_mut(snd_num) {
                    if elem_num < sound.len() {
                        sound[elem_num] = value as i16;
                    }
                }
                if *play {
                    if let Some(sound) = self.igs_sound_data.get(snd_num) {
                        let _ = self.event_tx.send(TerminalEvent::PlayGist(sound.clone()));
                    }
                }
                true
            }

            QueuedCommand::Igs(IgsCommand::RestoreSoundEffect { sound_effect }) => {
                let snd_num = (*sound_effect as usize).min(19);
                if let Some(original) = sound_data(snd_num) {
                    if let Some(sound) = self.igs_sound_data.get_mut(snd_num) {
                        *sound = original.to_vec();
                    }
                }
                true
            }

            QueuedCommand::Igs(IgsCommand::ChipMusic {
                sound_effect,
                voice,
                volume,
                pitch,
                timing,
                stop_type,
            }) => {
                if *pitch > 0 {
                    let snd_num = *sound_effect as usize;
                    if let Some(sound) = self.igs_sound_data.get(snd_num) {
                        let _ = self.event_tx.send(TerminalEvent::PlayChipMusic {
                            sound_data: sound.clone(),
                            voice: *voice,
                            volume: *volume,
                            pitch: *pitch,
                        });
                    }
                }
                if *timing > 0 {
                    let wait_ms = (*timing as u64 * 1000) / 200;
                    tokio::time::sleep(Duration::from_millis(wait_ms)).await;
                }
                match *stop_type {
                    StopType::SndOff => {
                        let _ = self.event_tx.send(TerminalEvent::SndOff(*voice));
                    }
                    StopType::StopSnd => {
                        let _ = self.event_tx.send(TerminalEvent::StopSnd(*voice));
                    }
                    StopType::SndOffAll => {
                        let _ = self.event_tx.send(TerminalEvent::SndOffAll);
                    }
                    StopType::StopSndAll => {
                        let _ = self.event_tx.send(TerminalEvent::StopSndAll);
                    }
                    StopType::NoEffect => {}
                }
                true
            }

            QueuedCommand::Igs(IgsCommand::StopAllSound) => {
                let _ = self.event_tx.send(TerminalEvent::StopSndAll);
                true
            }

            QueuedCommand::Igs(IgsCommand::BellsAndWhistles { sound_effect }) => {
                let snd_num = (*sound_effect as usize).min(19);
                if let Some(sound) = self.igs_sound_data.get(snd_num) {
                    if snd_num <= 4 {
                        for _ in 0..self.igs_effect_loop {
                            let _ = self.event_tx.send(TerminalEvent::PlayGist(sound.clone()));
                            tokio::time::sleep(Duration::from_millis(200)).await;
                        }
                    } else {
                        let _ = self.event_tx.send(TerminalEvent::PlayGist(sound.clone()));
                    }
                }
                true
            }

            QueuedCommand::Igs(IgsCommand::Noise { .. }) | QueuedCommand::Igs(IgsCommand::LoadMidiBuffer { .. }) => true,

            QueuedCommand::Music(music) => {
                let _ = self.event_tx.send(TerminalEvent::PlayMusic(music.clone()));
                true
            }

            QueuedCommand::Bell => {
                let _ = self.event_tx.send(TerminalEvent::Beep);
                true
            }

            QueuedCommand::DeviceControl(DeviceControlString::Sixel {
                aspect_ratio,
                zero_color,
                grid_size,
                sixel_data,
            }) => {
                match Sixel::parse_from(aspect_ratio.clone(), zero_color.clone(), grid_size.clone(), sixel_data) {
                    Ok(sixel) => {
                        {
                            let mut screen = self.edit_screen.lock();
                            let pos = screen.caret_position();
                            if let Some(editable) = screen.as_editable() {
                                editable.add_sixel(pos, sixel);
                            }
                        }
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                    Err(err) => {
                        log::error!("Error loading sixel: {}", err);
                    }
                }
                true
            }

            QueuedCommand::TerminalRequest(request) => {
                self.handle_terminal_request(request.clone()).await;
                true
            }

            QueuedCommand::Igs(IgsCommand::AskIG { query }) => {
                match query {
                    AskQuery::VersionNumber => {
                        if let Some(conn) = &mut self.connection {
                            let _ = conn.send(icy_engine::igs::IGS_VERSION.as_bytes()).await;
                        }
                    }
                    AskQuery::CurrentResolution => {
                        let screen = self.edit_screen.lock();
                        if let GraphicsType::IGS(mode) = screen.graphics_type() {
                            if let Some(conn) = &mut self.connection {
                                let _ = conn.send(format!("{}:", mode as u8).as_bytes()).await;
                            }
                        }
                    }
                    _ => {}
                }
                true
            }

            QueuedCommand::ResizeTerminal(width, height) => {
                let mut screen = self.edit_screen.lock();
                if let Some(editable) = screen.as_editable() {
                    editable.set_size(icy_engine::Size::new(*width as i32, *height as i32));
                }
                true
            }

            // These need screen lock
            _ => false,
        }
    }

    /// Process commands from queue with granular locking
    /// Commands are processed in batches with max 10ms lock duration
    /// Async commands (delays, sound) are processed outside of locks
    async fn process_command_queue(&mut self) {
        const MAX_LOCK_DURATION_MS: u64 = 10;

        loop {
            // Get next command
            let Some(cmd) = self.command_queue.pop_front() else {
                break;
            };

            // Try to process as async command first
            if self.try_process_async_command(&cmd).await {
                continue;
            }

            // Process commands that need screen lock
            let mut had_grab_screen = false;
            {
                let lock_start = Instant::now();
                let mut screen = self.edit_screen.lock();

                if let Some(editable) = screen.as_editable() {
                    let mut screen_sink = ScreenSink::new(editable);

                    // Process first command
                    had_grab_screen |= cmd.process_screen_command(&mut screen_sink);

                    // Process more commands while within time budget
                    while lock_start.elapsed().as_millis() < MAX_LOCK_DURATION_MS as u128 {
                        // Check if next command needs async processing (without removing)
                        match self.command_queue.front() {
                            None => break,
                            Some(cmd) if cmd.needs_async_processing() => break,
                            _ => {}
                        }

                        // Safe to pop - we know it exists and doesn't need async
                        let next_cmd = self.command_queue.pop_front().unwrap();

                        had_grab_screen |= next_cmd.process_screen_command(&mut screen_sink);

                        // Break early on GrabScreen for double-stepping
                        if had_grab_screen && self.double_step_vsyncs.is_some() {
                            break;
                        }
                    }

                    // Update hyperlinks before releasing lock
                    editable.update_hyperlinks();
                }
            }

            // Apply double-stepping delay if GrabScreen was processed
            if had_grab_screen {
                if let Some(vsyncs) = self.double_step_vsyncs {
                    let delay_ms = (vsyncs as u64) * 1000 / 60;
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }

    async fn start_upload(&mut self, protocol: TransferProtocol, files: Vec<PathBuf>) {
        let download_dir = self.download_directory.clone().unwrap_or_else(|| PathBuf::from("."));
        let is_external = !protocol.is_internal();
        let protocol_name = protocol.get_name();

        let Some(mut prot) = protocol.create(download_dir) else {
            self.send_event(TerminalEvent::Error(
                format!("Upload failed."),
                format!("Protocol '{}' not configured", protocol.id),
            ));
            return;
        };

        // For external protocols, send the event before initiating to show UI immediately
        if is_external {
            self.send_event(TerminalEvent::ExternalTransferStarted(protocol_name.clone(), false));
        }

        let result = if let Some(conn) = &mut self.connection {
            prot.initiate_send(&mut **conn, &files).await
        } else {
            return;
        };

        match result {
            Ok(state) => {
                if is_external {
                    // External protocol completed
                    self.send_event(TerminalEvent::ExternalTransferCompleted(protocol_name, false, true, None));
                } else {
                    self.current_transfer = Some(state.clone());
                    self.send_event(TerminalEvent::TransferStarted(state.clone(), false));

                    // Run the file transfer
                    if let Err(e) = self.run_file_transfer(prot.as_mut(), state).await {
                        log::error!("Upload error: {}", e);
                        self.send_event(TerminalEvent::Error(format!("Upload failed."), format!("{}", e)));
                    }
                }
            }
            Err(e) => {
                if is_external {
                    self.send_event(TerminalEvent::ExternalTransferCompleted(protocol_name, false, false, Some(format!("{}", e))));
                } else {
                    self.send_event(TerminalEvent::Error(format!("Upload failed."), format!("{}", e)));
                }
            }
        }
    }

    async fn start_download(&mut self, protocol: TransferProtocol, filename: Option<String>) {
        let download_dir = self.download_directory.clone().unwrap_or_else(|| PathBuf::from("."));
        let is_external = !protocol.is_internal();
        let protocol_name = protocol.get_name();

        let Some(mut prot) = protocol.create(download_dir) else {
            self.send_event(TerminalEvent::Error(
                format!("Download failed."),
                format!("Protocol '{}' not configured", protocol.id),
            ));
            return;
        };

        // For external protocols, send the event before initiating to show UI immediately
        if is_external {
            self.send_event(TerminalEvent::ExternalTransferStarted(protocol_name.clone(), true));
        }

        let result = if let Some(conn) = &mut self.connection {
            prot.initiate_recv(&mut **conn).await
        } else {
            return;
        };

        match result {
            Ok(mut state) => {
                if is_external {
                    // External protocol completed
                    self.send_event(TerminalEvent::ExternalTransferCompleted(protocol_name, true, true, None));
                } else {
                    if let Some(name) = filename {
                        state.recieve_state.file_name = name;
                    }
                    self.current_transfer = Some(state.clone());
                    self.send_event(TerminalEvent::TransferStarted(state.clone(), true));

                    // Run the file transfer
                    if let Err(e) = self.run_file_transfer(prot.as_mut(), state).await {
                        log::error!("Download error: {}", e);
                        self.send_event(TerminalEvent::Error(format!("Download failed."), format!("{}", e)));
                    }
                }
            }
            Err(e) => {
                if is_external {
                    self.send_event(TerminalEvent::ExternalTransferCompleted(protocol_name, true, false, Some(format!("{}", e))));
                } else {
                    self.send_event(TerminalEvent::Error(format!("Download failed."), format!("{}", e)));
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
                    self.send_event(TerminalEvent::TransferProgress(transfer_state.clone()));
                    last_progress_update = Instant::now();
                }
            }
        }

        // Copy downloaded files to the download directory
        copy_downloaded_files(&mut transfer_state, self.download_directory.as_ref())?;

        self.current_transfer = Some(transfer_state.clone());
        self.send_event(TerminalEvent::TransferCompleted(transfer_state));
        self.current_transfer = None;

        Ok(())
    }

    fn send_event(&mut self, evt: TerminalEvent) {
        if let Err(err) = self.event_tx.send(evt) {
            log::error!("Failed to send terminal event: {}", err);
        }
    }

    async fn write_to_capture(&mut self, data: &[u8]) {
        if let Some(writer) = &mut self.capture_writer {
            if let Err(e) = writer.write(data).await {
                log::error!("Failed to write to capture file: {}", e);
                // Close the capture file on error
                self.capture_writer = None;
            }
        }
    }

    async fn handle_terminal_request(&mut self, request: TerminalRequest) {
        let response: Option<Vec<u8>> = match &request {
            TerminalRequest::DeviceAttributes => {
                // respond with IcyTerm as ASCII followed by the package version.

                let version = format!(
                    "\x1b[=73;99;121;84;101;114;109;{};{};{}c",
                    env!("CARGO_PKG_VERSION_MAJOR"),
                    env!("CARGO_PKG_VERSION_MINOR"),
                    env!("CARGO_PKG_VERSION_PATCH")
                );
                Some(version.into_bytes())
            }
            TerminalRequest::SecondaryDeviceAttributes => {
                let major: i32 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap_or(0);
                let minor: i32 = env!("CARGO_PKG_VERSION_MINOR").parse().unwrap_or(0);
                let patch: i32 = env!("CARGO_PKG_VERSION_PATCH").parse().unwrap_or(0);
                let version = major * 100 + minor * 10 + patch;
                let hardware_options = 1 | 4 | 8 | 128;
                Some(format!("\x1b[>65;{};{}c", version, hardware_options).into_bytes())
            }
            TerminalRequest::ExtendedDeviceAttributes => {
                /*
                    1 - Loadable fonts are availabe via Device Control Strings
                    2 - Bright Background (ie: DECSET 32) is supported
                    3 - Palette entries may be modified via an Operating System Command
                        string
                    4 - Pixel operations are supported (currently, sixel and PPM
                        graphics)
                    5 - The current font may be selected via CSI Ps1 ; Ps2 sp D
                    6 - Extended palette is available
                    7 - Mouse is available
                */

                Some(b"\x1B[<1;2;3;4;5;6;7c".to_vec())
            }
            TerminalRequest::DeviceStatusReport => Some(b"\x1B[0n".to_vec()),
            TerminalRequest::CursorPositionReport => {
                let screen = self.edit_screen.lock();
                let pos = screen.caret_position();
                let y = pos.y.min(screen.get_height() as i32 - 1) + 1;
                let x = pos.x.min(screen.get_width() as i32 - 1) + 1;
                Some(format!("\x1B[{};{}R", y, x).into_bytes())
            }
            TerminalRequest::ScreenSizeReport => {
                let screen = self.edit_screen.lock();
                let height = screen.get_height();
                let width = screen.get_width();
                Some(format!("\x1B[{};{}R", height, width).into_bytes())
            }
            TerminalRequest::RequestTabStopReport => {
                let screen = self.edit_screen.lock();
                let mut response = b"\x1BP2$u".to_vec();
                let tab_count = screen.terminal_state().tab_count();
                for i in 0..tab_count {
                    let tab = screen.terminal_state().get_tabs()[i];
                    response.extend_from_slice((tab + 1).to_string().as_bytes());
                    if i < tab_count.saturating_sub(1) {
                        response.push(b'/');
                    }
                }
                response.extend_from_slice(b"\x1B\\");
                Some(response)
            }
            TerminalRequest::AnsiModeReport(_) => Some(b"\x1B[?0$y".to_vec()),
            TerminalRequest::DecPrivateModeReport(_) => Some(b"\x1B[?0$y".to_vec()),
            TerminalRequest::RequestChecksumRectangularArea {
                id,
                page: _,
                top,
                left,
                bottom,
                right,
            } => {
                let screen = self.edit_screen.lock();
                let checksum = icy_engine::decrqcra_checksum(&**screen, *top as i32, *left as i32, *bottom as i32, *right as i32);
                Some(format!("\x1BP{}!~{:04X}\x1B\\", id, checksum).into_bytes())
            }
            TerminalRequest::FontStateReport => {
                let screen = self.edit_screen.lock();
                let state = screen.terminal_state();
                let font_selection_result = match state.font_selection_state {
                    icy_engine::FontSelectionState::NoRequest => 99,
                    icy_engine::FontSelectionState::Success => 0,
                    icy_engine::FontSelectionState::Failure => 1,
                };
                Some(
                    format!(
                        "\x1B[=1;{};{};{};{};{}n",
                        font_selection_result,
                        state.normal_attribute_font_slot,
                        state.high_intensity_attribute_font_slot,
                        state.blink_attribute_font_slot,
                        state.high_intensity_blink_attribute_font_slot
                    )
                    .into_bytes(),
                )
            }
            TerminalRequest::FontModeReport => {
                let screen = self.edit_screen.lock();
                let state = screen.terminal_state();
                let mut params = Vec::new();

                if state.origin_mode == icy_engine::OriginMode::WithinMargins {
                    params.push("6");
                }
                if state.auto_wrap_mode == icy_engine::AutoWrapMode::AutoWrap {
                    params.push("7");
                }
                if screen.caret().visible {
                    params.push("25");
                }
                if screen.ice_mode() == icy_engine::IceMode::Ice {
                    params.push("33");
                }
                if screen.caret().blinking {
                    params.push("35");
                }

                match state.mouse_mode() {
                    icy_engine::MouseMode::OFF => {}
                    icy_engine::MouseMode::X10 => params.push("9"),
                    icy_engine::MouseMode::VT200 => params.push("1000"),
                    icy_engine::MouseMode::VT200_Highlight => params.push("1001"),
                    icy_engine::MouseMode::ButtonEvents => params.push("1002"),
                    icy_engine::MouseMode::AnyEvents => params.push("1003"),
                }

                if state.mouse_state.focus_out_event_enabled {
                    params.push("1004");
                }
                if state.mouse_state.alternate_scroll_enabled {
                    params.push("1007");
                }

                match state.mouse_state.extended_mode {
                    icy_engine::ExtMouseMode::None => {}
                    icy_engine::ExtMouseMode::ExtendedUTF8 => params.push("1005"),
                    icy_engine::ExtMouseMode::SGR => params.push("1006"),
                    icy_engine::ExtMouseMode::URXVT => params.push("1015"),
                    icy_engine::ExtMouseMode::PixelPosition => params.push("1016"),
                }

                let mode_report = if params.is_empty() {
                    "\x1B[=2;n".to_string()
                } else {
                    format!("\x1B[=2;{}n", params.join(";"))
                };
                Some(mode_report.into_bytes())
            }
            TerminalRequest::FontDimensionReport => {
                let screen = self.edit_screen.lock();
                let dim = screen.get_font_dimensions();
                Some(format!("\x1B[=3;{};{}n", dim.height, dim.width).into_bytes())
            }
            TerminalRequest::MacroSpaceReport => Some(b"\x1B[32767*{".to_vec()),
            TerminalRequest::MemoryChecksumReport(pid, checksum) => Some(format!("\x1BP{}!~{:04X}\x1B\\", pid, checksum).into_bytes()),
            TerminalRequest::RipRequestTerminalId => {
                let screen = self.edit_screen.lock();
                if screen.graphics_type() == GraphicsType::Rip {
                    Some(icy_engine::RIP_TERMINAL_ID.as_bytes().to_vec())
                } else {
                    None
                }
            }
            TerminalRequest::RipQueryFile(_) => {
                // TODO
                None
            }
            TerminalRequest::RipQueryFileSize(_) => {
                // TODO
                None
            }
            TerminalRequest::RipQueryFileDate(_) => {
                // TODO
                None
            }
            TerminalRequest::RipReadFile(_) => {
                // TODO
                None
            }
        };

        // Send response directly if available
        if let Some(data) = response {
            if let Some(conn) = &mut self.connection {
                /*
                // Debug output with filtered control chars
                let debug_str = data
                    .iter()
                    .map(|&b| {
                        match b {
                            0x1B => "<ESC>".to_string(),
                            0x00..=0x1F => format!("<{:02X}>", b),
                            0x7F => "<DEL>".to_string(),
                            0x80..=0xFF => format!("[{:02X}]", b), // High ASCII
                            _ => (b as char).to_string(),
                        }
                    })
                    .collect::<String>();
                println!("Sending response: {}", debug_str);
                */
                let _ = conn.send(&data).await;
            }
        }
    }

    /*
        async fn auto_login(&mut self, commands: &[AutoLoginCommand], user_name: Option<String>, password: Option<String>) {
            // Extract user name parts
            let full_name = user_name.clone().unwrap_or_default();
            let parts: Vec<&str> = full_name.split_whitespace().collect();
            let first_name = parts.first().unwrap_or(&"").to_string();
            let last_name = parts.get(1..).map(|parts| parts.join(" ")).unwrap_or_default();
            let password = password.unwrap_or_default();

            for command in commands {
                match command {
                    AutoLoginCommand::Delay(seconds) => {
                        tokio::time::sleep(tokio::time::Duration::from_secs(*seconds as u64)).await;
                    }

                    AutoLoginCommand::EmulateMailerAccess => {
                        // Send CR+CR then ESC
                        if let Some(conn) = &mut self.connection {
                            tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
                            let _ = conn.send(&[13, 13, 27]).await;
                            // Wait briefly for response
                            tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
                        }
                    }

                    AutoLoginCommand::WaitForNamePrompt => {
                        // Wait for name/login prompts like "name:", "login:", "user:", etc.
                        // Timeout after 10 seconds
                        let timeout = tokio::time::Duration::from_secs(10);
                        let start = tokio::time::Instant::now();
                        let mut buffer = vec![0u8; 4096];
                        let mut accumulated_text = String::new();

                        // Get buffer type for unicode conversion
                        let buffer_type = {
                            let screen = self.edit_screen.lock();
                            screen.buffer_type()
                        };

                        loop {
                            // Check timeout
                            if start.elapsed() >= timeout {
                                log::warn!("WaitForNamePrompt: Timeout waiting for prompt");
                                break;
                            }

                            // Try to read data with a small timeout
                            tokio::select! {
                                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                                    // Continue loop to check for timeout
                                }
                                data = self.read_connection(&mut buffer) => {
                                    if data.is_empty() {
                                        continue;
                                    }

                                    // Convert bytes to unicode string
                                    for &byte in &data {
                                        let ch = buffer_type.convert_to_unicode(byte as char);
                                        accumulated_text.push(ch.to_ascii_lowercase());
                                    }
                                    // Check for name/login prompts (case-insensitive)
                                    let prompt_patterns = [
                                        "name",
                                        "login",
                                        "user",
                                    ];
                                    for pattern in &prompt_patterns {
                                        if accumulated_text.contains(pattern) {
                                            log::info!("WaitForNamePrompt: Detected prompt pattern '{}'", pattern);
                                            return; // Exit the command - prompt detected
                                        }
                                    }

                                    // Keep only last 512 characters to avoid unbounded growth
                                    if accumulated_text.len() > 512 {
                                        accumulated_text = accumulated_text.chars().skip(accumulated_text.len() - 512).collect();
                                    }
                                }
                            }
                        }
                    }

                    AutoLoginCommand::SendFullName => {
                        if let Some(conn) = &mut self.connection {
                            let _ = conn.send(full_name.as_bytes()).await;
                        }
                    }

                    AutoLoginCommand::SendFirstName => {
                        if let Some(conn) = &mut self.connection {
                            let _ = conn.send(first_name.as_bytes()).await;
                        }
                    }

                    AutoLoginCommand::SendLastName => {
                        if let Some(conn) = &mut self.connection {
                            let _ = conn.send(last_name.as_bytes()).await;
                        }
                    }

                    AutoLoginCommand::SendPassword => {
                        if let Some(conn) = &mut self.connection {
                            let _ = conn.send(password.as_bytes()).await;
                        }
                    }

                    AutoLoginCommand::DisableIEMSI => {
                        // Disable IEMSI for this session
                        self.iemsi_auto_login = None;
                    }

                    AutoLoginCommand::SendControlCode(code) => {
                        if let Some(conn) = &mut self.connection {
                            let _ = conn.send(&[*code]).await;
                        }
                    }

                    AutoLoginCommand::RunScript(_filename) => {
                        // TODO: Implement script execution
                    }

                    AutoLoginCommand::SendText(text) => {
                        if let Some(conn) = &mut self.connection {
                            let _ = conn.send(text.as_bytes()).await;
                        }
                    }
                }
            }
        }
    */
}

// Helper function to create a terminal thread for the UI
pub fn create_terminal_thread(
    edit_screen: Arc<Mutex<Box<dyn Screen>>>,
    address_book: Arc<Mutex<crate::data::AddressBook>>,
) -> (mpsc::UnboundedSender<TerminalCommand>, mpsc::UnboundedReceiver<TerminalEvent>) {
    let parser = icy_parser_core::AnsiParser::new();

    TerminalThread::spawn(edit_screen, Box::new(parser), address_book)
}

fn copy_downloaded_files(transfer_state: &mut TransferState, download_dir: Option<&PathBuf>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let upload_location = if let Some(dir) = download_dir {
        dir.clone()
    } else if let Some(dirs) = UserDirs::new() {
        if let Some(default_dir) = dirs.download_dir() {
            default_dir.to_path_buf()
        } else {
            return Err("Failed to get user download directory".into());
        }
    } else {
        return Err("Failed to get user directories".into());
    };

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

    Ok(())
}

impl TerminalThread {
    /// Start running a Lua script from file
    fn run_script(&mut self, path: PathBuf) {
        // Stop any existing script
        self.stop_script();

        // Create a new script runner
        let mut runner = ScriptRunner::new(
            self.edit_screen.clone(),
            self.command_tx.clone(),
            self.event_tx.clone(),
            self.address_book.clone(),
            self.terminal_emulation.clone(),
        );

        match runner.run_file(&path) {
            Ok(()) => {
                self.send_event(TerminalEvent::ScriptStarted(path));
                self.script_runner = Some(runner);
            }
            Err(e) => {
                self.send_event(TerminalEvent::ScriptFinished(Err(e)));
            }
        }
    }

    /// Start running Lua script code directly (from string)
    fn run_script_code(&mut self, code: String) {
        // Stop any existing script
        self.stop_script();

        // Create a new script runner
        let mut runner = ScriptRunner::new(
            self.edit_screen.clone(),
            self.command_tx.clone(),
            self.event_tx.clone(),
            self.address_book.clone(),
            self.terminal_emulation.clone(),
        );

        match runner.run_script(code) {
            Ok(()) => {
                // Use a placeholder path for code-based scripts
                self.send_event(TerminalEvent::ScriptStarted(PathBuf::from("<mcp_script>")));
                self.script_runner = Some(runner);
            }
            Err(e) => {
                self.send_event(TerminalEvent::ScriptFinished(Err(e)));
            }
        }
    }

    /// Stop the currently running script
    fn stop_script(&mut self) {
        if let Some(mut runner) = self.script_runner.take() {
            runner.stop();
            self.send_event(TerminalEvent::ScriptFinished(Ok(())));
        }
    }

    /// Check if a script finished and send event
    fn check_script_finished(&mut self) {
        if let Some(runner) = &mut self.script_runner {
            if let Some(result) = runner.get_result() {
                let event = match result {
                    crate::scripting::ScriptResult::Success => TerminalEvent::ScriptFinished(Ok(())),
                    crate::scripting::ScriptResult::Error(e) => TerminalEvent::ScriptFinished(Err(e)),
                    crate::scripting::ScriptResult::Stopped => TerminalEvent::ScriptFinished(Ok(())),
                };
                self.send_event(event);
                self.script_runner = None;
            }
        }
    }

    /// Set terminal settings (terminal type, screen mode, ansi music) during session
    /// This reinitializes the screen and parser similar to connect()
    async fn set_terminal_settings(&mut self, terminal_type: TerminalEmulation, screen_mode: ScreenMode, ansi_music: MusicOption) {
        self.use_utf8 = terminal_type == TerminalEmulation::Utf8Ansi;

        // Create new screen and parser for the new terminal type
        let (mut new_screen, parser) = screen_mode.create_screen(terminal_type, Some(CreationOptions { ansi_music }));

        // Use a default scrollback buffer size
        new_screen.set_scrollback_buffer_size(10000);
        new_screen.mark_dirty();

        // Replace screen and parser
        {
            let mut screen = self.edit_screen.lock();
            *screen = new_screen;
        }
        self.parser = parser;

        // Update terminal emulation for scripting
        *self.terminal_emulation.lock() = terminal_type;

        // Notify UI of the change
        self.send_event(TerminalEvent::TerminalSettingsChanged {
            terminal_type,
            screen_mode,
            ansi_music,
        });
    }
}
