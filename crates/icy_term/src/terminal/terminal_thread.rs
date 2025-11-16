use crate::baud_emulator::BaudEmulator;
use crate::emulated_modem::{EmulatedModem, ModemCommand};
use crate::features::{AutoFileTransfer, IEmsiAutoLogin};
use crate::{ConnectionInformation, ScreenMode};
use directories::UserDirs;
use icy_engine::{AutoWrapMode, ExtMouseMode, FontSelectionState, IceMode, MouseMode, OriginMode, RIP_TERMINAL_ID};
use icy_engine::{EditableScreen, GraphicsType, PaletteScreenBuffer, ScreenSink, TextScreen};
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
use icy_parser_core::*;
use icy_parser_core::{AnsiMusic, CommandParser, CommandSink, TerminalCommand as ParserCommand, TerminalRequest};
use icy_parser_core::{BaudEmulation, MusicOption};
use log::error;
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
    SetBaudEmulation(BaudEmulation),
    StartCapture(String),
    StopCapture,
}

/// Messages sent from the terminal thread to the UI
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    Connected,
    Disconnected(Option<String>), // Optional error message
    TransferStarted(TransferState, bool),
    TransferProgress(TransferState),
    TransferCompleted(TransferState),
    Error(String, String),
    PlayMusic(AnsiMusic),
    Beep,
    OpenLineSound,
    OpenDialSound(bool, String),
    StopSound,
    Reconnect,
    Connect(String),

    AutoTransferTriggered(TransferProtocolType, bool, Option<String>),
    EmsiLogin(Box<EmsiISI>),
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

    pub music_option: MusicOption,
    pub screen_mode: ScreenMode,

    pub baud_emulation: BaudEmulation,

    // Auto-login configuration
    pub iemsi_auto_login: bool,
    pub auto_login_exp: String,
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

/// Custom CommandSink implementation for terminal thread
struct TerminalSink<'a> {
    screen_sink: ScreenSink<'a>,
    event_tx: &'a mpsc::UnboundedSender<TerminalEvent>,
    output: &'a mut Vec<u8>,
}

impl<'a> TerminalSink<'a> {
    fn new(screen: &'a mut dyn EditableScreen, event_tx: &'a mpsc::UnboundedSender<TerminalEvent>, output: &'a mut Vec<u8>) -> Self {
        Self {
            screen_sink: ScreenSink::new(screen),
            event_tx,
            output,
        }
    }
}

impl<'a> CommandSink for TerminalSink<'a> {
    fn print(&mut self, text: &[u8]) {
        self.screen_sink.print(text);
    }

    fn emit(&mut self, cmd: ParserCommand) {
        // Handle terminal-specific events
        match cmd {
            ParserCommand::Bell => {
                let _ = self.event_tx.send(TerminalEvent::Beep);
            }
            ParserCommand::CsiResizeTerminal(height, width) => {
                self.screen_sink.screen_mut().set_size(icy_engine::Size::new(width as i32, height as i32));
            }
            _ => {
                // Delegate all other commands to screen_sink
                self.screen_sink.emit(cmd);
            }
        }
    }

    fn play_music(&mut self, music: AnsiMusic) {
        let _ = self.event_tx.send(TerminalEvent::PlayMusic(music));
    }

    fn emit_rip(&mut self, cmd: RipCommand) {
        self.screen_sink.emit_rip(cmd);
    }
    fn emit_skypix(&mut self, cmd: SkypixCommand) {
        self.screen_sink.emit_skypix(cmd);
    }
    fn emit_igs(&mut self, cmd: IgsCommand) {
        self.screen_sink.emit_igs(cmd);
    }
    fn device_control(&mut self, dcs: DeviceControlString<'_>) {
        self.screen_sink.device_control(dcs);
    }
    fn operating_system_command(&mut self, osc: OperatingSystemCommand<'_>) {
        self.screen_sink.operating_system_command(osc);
    }

    /// Emit an Application Program String (APS) sequence: ESC _ ... ESC \
    /// Default implementation does nothing.
    fn aps(&mut self, _data: &[u8]) {
        // ignore for now
    }

    fn report_error(&mut self, error: ParseError) {
        log::error!("Parse Error:{:?}", error);
    }

    fn request(&mut self, request: TerminalRequest) {
        use icy_parser_core::TerminalRequest;

        match request {
            TerminalRequest::DeviceAttributes => {
                // respond with IcyTerm as ASCII followed by the package version.
                let version = format!(
                    "\x1b[=73;99;121;84;101;114;109;{};{};{}c",
                    env!("CARGO_PKG_VERSION_MAJOR"),
                    env!("CARGO_PKG_VERSION_MINOR"),
                    env!("CARGO_PKG_VERSION_PATCH")
                );
                self.output.extend_from_slice(version.as_bytes());
            }
            TerminalRequest::SecondaryDeviceAttributes => {
                // Terminal type: 65 = VT525-compatible
                // Version: major * 100 + minor * 10 + patch
                let major: i32 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap_or(0);
                let minor: i32 = env!("CARGO_PKG_VERSION_MINOR").parse().unwrap_or(0);
                let patch: i32 = env!("CARGO_PKG_VERSION_PATCH").parse().unwrap_or(0);
                let version = major * 100 + minor * 10 + patch;

                // Hardware options: 0 (software terminal, no hardware options)
                // Could use bit flags here for features:
                //   1 = 132 columns
                //   2 = Printer port
                //   4 = Sixel graphics
                //   8 = Selective erase
                //   16 = User-defined keys
                //   32 = National replacement character sets
                //   64 = Technical character set
                //   128 = Locator port (mouse)
                let hardware_options = 1 | 4 | 8 | 128;
                let response = format!("\x1b[>65;{};{}c", version, hardware_options);
                self.output.extend_from_slice(response.as_bytes());
            }
            TerminalRequest::ExtendedDeviceAttributes => {
                // Extended Device Attributes: ESC[<...c response
                // Report extended terminal capabilities:
                //   1 - Loadable fonts are available via Device Control Strings
                //   2 - Bright Background (ie: DECSET 32) is supported
                //   3 - Palette entries may be modified via an Operating System Command string
                //   4 - Pixel operations are supported (sixel and PPM graphics)
                //   5 - The current font may be selected via CSI Ps1 ; Ps2 sp D
                //   6 - Extended palette is available
                //   7 - Mouse is available
                self.output.extend_from_slice(b"\x1B[<1;2;3;4;5;6;7c");
            }
            TerminalRequest::DeviceStatusReport => {
                // Device Status Report - terminal OK
                self.output.extend_from_slice(b"\x1B[0n");
            }
            TerminalRequest::CursorPositionReport => {
                // Cursor Position Report
                let screen = self.screen_sink.screen();
                let y = screen.caret().y.min(screen.terminal_state().get_height() - 1) + 1;
                let x = screen.caret().x.min(screen.terminal_state().get_width() - 1) + 1;
                self.output.extend_from_slice(format!("\x1B[{};{}R", y, x).as_bytes());
            }
            TerminalRequest::ScreenSizeReport => {
                // Screen Size Report
                let screen = self.screen_sink.screen();
                let height = screen.terminal_state().get_height();
                let width = screen.terminal_state().get_width();
                self.output.extend_from_slice(format!("\x1B[{};{}R", height, width).as_bytes());
            }
            TerminalRequest::RequestTabStopReport => {
                // Tab Stop Report in DCS format
                let screen = self.screen_sink.screen();
                self.output.extend_from_slice(b"\x1BP2$u");
                for i in 0..screen.terminal_state().tab_count() {
                    let tab = screen.terminal_state().get_tabs()[i];
                    self.output.extend_from_slice((tab + 1).to_string().as_bytes());
                    if i < screen.terminal_state().tab_count().saturating_sub(1) {
                        self.output.push(b'/');
                    }
                }
                self.output.extend_from_slice(b"\x1B\\");
            }
            TerminalRequest::AnsiModeReport(_mode) => {
                // ANSI mode report - for now report mode not recognized
                self.output.extend_from_slice(b"\x1B[?0$y");
            }
            TerminalRequest::DecPrivateModeReport(_mode) => {
                // DEC private mode report - for now report mode not recognized
                self.output.extend_from_slice(b"\x1B[?0$y");
            }
            TerminalRequest::RequestChecksumRectangularArea(ppage, pt, pl, pb, pr) => {
                let checksum = icy_engine::decrqcra_checksum(self.screen_sink.screen(), pt as i32, pl as i32, pb as i32, pr as i32);
                self.output.extend_from_slice(format!("\x1BP{}!~{checksum:04X}\x1B\\", ppage).as_bytes());
            }
            TerminalRequest::FontStateReport => {
                // Font state report: ESC[=1n response
                let screen = self.screen_sink.screen();
                let font_selection_result = match screen.terminal_state().font_selection_state {
                    FontSelectionState::NoRequest => 99,
                    FontSelectionState::Success => 0,
                    FontSelectionState::Failure => 1,
                };

                let response = format!(
                    "\x1B[=1;{};{};{};{};{}n",
                    font_selection_result,
                    screen.terminal_state().normal_attribute_font_slot,
                    screen.terminal_state().high_intensity_attribute_font_slot,
                    screen.terminal_state().blink_attribute_font_slot,
                    screen.terminal_state().high_intensity_blink_attribute_font_slot
                );
                self.output.extend_from_slice(response.as_bytes());
            }
            TerminalRequest::FontModeReport => {
                // Font mode report: ESC[=2n response
                let screen = self.screen_sink.screen();
                let mut params = Vec::new();

                if screen.terminal_state().origin_mode == OriginMode::WithinMargins {
                    params.push("6");
                }
                if screen.terminal_state().auto_wrap_mode == AutoWrapMode::AutoWrap {
                    params.push("7");
                }
                if screen.caret().visible {
                    params.push("25");
                }
                if screen.ice_mode() == IceMode::Ice {
                    params.push("33");
                }
                if screen.caret().blinking {
                    params.push("35");
                }

                match screen.terminal_state().mouse_mode() {
                    MouseMode::OFF => {}
                    MouseMode::X10 => params.push("9"),
                    MouseMode::VT200 => params.push("1000"),
                    MouseMode::VT200_Highlight => params.push("1001"),
                    MouseMode::ButtonEvents => params.push("1002"),
                    MouseMode::AnyEvents => params.push("1003"),
                }

                if screen.terminal_state().mouse_state.focus_out_event_enabled {
                    params.push("1004");
                }

                if screen.terminal_state().mouse_state.alternate_scroll_enabled {
                    params.push("1007");
                }

                match screen.terminal_state().mouse_state.extended_mode {
                    ExtMouseMode::None => {}
                    ExtMouseMode::Extended => params.push("1005"),
                    ExtMouseMode::SGR => params.push("1006"),
                    ExtMouseMode::URXVT => params.push("1015"),
                    ExtMouseMode::PixelPosition => params.push("1016"),
                }

                let mode_report = if params.is_empty() {
                    "\x1B[=2;n".to_string()
                } else {
                    format!("\x1B[=2;{}n", params.join(";"))
                };
                self.output.extend_from_slice(mode_report.as_bytes());
            }
            TerminalRequest::FontDimensionReport => {
                // Font dimension report: ESC[=3n response
                let screen = self.screen_sink.screen();
                let dim = screen.get_font_dimensions();
                let response = format!("\x1B[=3;{};{}n", dim.height, dim.width);
                self.output.extend_from_slice(response.as_bytes());
            }
            TerminalRequest::MacroSpaceReport => {
                // Macro Space Report: ESC[?62n response
                // Report 32767 bytes available (standard response)
                self.output.extend_from_slice(b"\x1B[32767*{");
            }
            TerminalRequest::MemoryChecksumReport(pid, checksum) => {
                // Memory Checksum Report: ESC[?63;{Pid}n response
                // Checksum was calculated by the parser from macro memory
                let response = format!("\x1BP{}!~{:04X}\x1B\\", pid, checksum);
                self.output.extend_from_slice(response.as_bytes());
            }

            TerminalRequest::RipRequestTerminalId => {
                if self.screen_sink.screen().graphics_type() == icy_engine::GraphicsType::Rip {
                    self.output.extend_from_slice(RIP_TERMINAL_ID.as_bytes());
                }
            }
            // RIPscrip and IGS requests are not implemented
            _ => {}
        }
    }
}

pub struct TerminalThread {
    // Shared state with UI
    edit_screen: Arc<Mutex<Box<dyn EditableScreen>>>,

    // Thread-local state
    connection: Option<Box<dyn Connection>>,
    parser: Box<dyn CommandParser + Send>,
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
    iemsi_auto_login: Option<IEmsiAutoLogin>,
    auto_transfer: Option<(TransferProtocolType, bool, Option<String>)>, // For pending auto-transfers

    // Capture state
    capture_writer: Option<std::fs::File>,

    output_buffer: Vec<u8>,
}

impl TerminalThread {
    pub fn spawn(
        edit_screen: Arc<Mutex<Box<dyn EditableScreen>>>,
        parser: Box<dyn CommandParser + Send>,
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
            event_tx: event_tx.clone(),
            use_utf8: false,
            utf8_buffer: Vec::new(),
            auto_file_transfer: AutoFileTransfer::default(),
            baud_emulator: BaudEmulator::new(),
            iemsi_auto_login: None,
            auto_transfer: None,
            emulated_modem: EmulatedModem::default(),
            capture_writer: None,
            output_buffer: Vec::new(),
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
                            self.write_to_capture(&data);
                            self.process_data(&data).await;
                        }
                    }

                    // Check for pending auto-transfers
                    if let Some((protocol, is_download, filename)) = self.auto_transfer.take() {
                        if is_download {
                            self.start_download(protocol, filename).await;
                        } else {
                            // For uploads, we'd need file selection - just notify UI
                            self.send_event(TerminalEvent::AutoTransferTriggered(protocol, is_download, filename));
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

                        let _ = self.read_connection(&mut read_buffer).await;
                    }
                }
            }
        }
    }

    fn perform_resize(&mut self, width: u16, height: u16) {
        if let Ok(mut state) = self.edit_screen.lock() {
            state.set_size(icy_engine::Size::new(width as i32, height as i32));
        }
    }

    async fn handle_command(&mut self, command: TerminalCommand) {
        match command {
            TerminalCommand::Connect(config) => {
                // let auto_login = config.auto_login_exp.to_string();
                // let user_name = config.user_name.clone();
                // let password = config.password.clone();

                if let Err(e) = self.connect(config).await {
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
            TerminalCommand::StartCapture(file_name) => match std::fs::File::create(&file_name) {
                Ok(file) => {
                    self.capture_writer = Some(file);
                    log::info!("Started capturing to {}", file_name);
                }
                Err(e) => {
                    log::error!("Failed to create capture file {}: {}", file_name, e);
                    self.send_event(TerminalEvent::Error(format!("Failed to create capture file: {}", file_name), format!("{}", e)));
                }
            },
            TerminalCommand::StopCapture => {
                self.capture_writer = None;
                log::info!("Stopped capturing");
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
        if let Ok(mut screen) = self.edit_screen.lock() {
            let screen_mode = config.screen_mode;

            match config.terminal_type {
                TerminalEmulation::Rip => {
                    let buf = PaletteScreenBuffer::new(GraphicsType::Rip);
                    *screen = Box::new(buf) as Box<dyn icy_engine::EditableScreen>;
                }
                TerminalEmulation::Skypix => {
                    let buf = PaletteScreenBuffer::new(GraphicsType::Skypix);
                    //   buf.set_size((80, 42).into());
                    *screen = Box::new(buf) as Box<dyn icy_engine::EditableScreen>;
                }
                TerminalEmulation::AtariST => {
                    let (res, _igs) = if let ScreenMode::AtariST(res, igs) = config.screen_mode {
                        (res, igs)
                    } else {
                        (icy_engine::TerminalResolution::Low, false)
                    };
                    let buf = PaletteScreenBuffer::new(GraphicsType::IGS(res));
                    *screen = Box::new(buf) as Box<dyn icy_engine::EditableScreen>;
                }

                _ => {
                    *screen = Box::new(TextScreen::new(screen_mode.get_window_size())) as Box<dyn icy_engine::EditableScreen>;
                }
            }

            screen.terminal_state_mut().is_terminal_buffer = true;

            screen_mode.apply_to_edit_screen(&mut **screen);
        }
        self.parser = crate::get_parser(&config.terminal_type, config.music_option, config.screen_mode);

        // Reset auto-transfer state
        self.auto_file_transfer = AutoFileTransfer::default();

        self.send_event(TerminalEvent::Connected);

        Ok(())
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
            self.iemsi_auto_login = Some(IEmsiAutoLogin::new(effective_user.clone().unwrap(), effective_pass.clone().unwrap()));
        }
    }

    async fn disconnect(&mut self) {
        if let Some(mut conn) = self.connection.take() {
            let _ = conn.shutdown().await;
        }
        if let Ok(mut state) = self.edit_screen.lock() {
            state.caret_default_colors();
        }
        self.process_data(b"\r\nNO CARRIER\r\n").await;

        self.baud_emulator = BaudEmulator::new();
        self.connection_time = None;
        self.utf8_buffer.clear();
        self.iemsi_auto_login = None;
        self.auto_file_transfer = AutoFileTransfer::default();
        self.send_event(TerminalEvent::Disconnected(None));
    }

    async fn read_connection(&mut self, buffer: &mut [u8]) -> Vec<u8> {
        if let Some(conn) = &mut self.connection {
            match conn.try_read(buffer).await {
                Ok(0) => Vec::new(),
                Ok(size) => {
                    let mut data = buffer[..size].to_vec();

                    // Apply baud emulation if enabled
                    data = self.baud_emulator.emulate(&data);

                    if !data.is_empty() {
                        self.write_to_capture(&data);
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
            if let Some((protocol_type, download)) = self.auto_file_transfer.try_transfer(byte) {
                self.auto_transfer = Some((protocol_type, download, None));
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

        // Vector for collecting terminal query responses

        if let Ok(mut screen) = self.edit_screen.lock() {
            {
                let mut sink = TerminalSink::new(&mut **screen, &self.event_tx, &mut self.output_buffer);

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
                                    // Invalid UTF-8 sequence, replace with replacement character
                                    to_process.extend_from_slice("�".as_bytes());
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
                    self.parser.parse(&to_process, &mut sink);
                } else {
                    // Legacy mode: parse bytes directly
                    self.parser.parse(data, &mut sink);
                }

                screen.update_hyperlinks();
            }

            while screen.sixel_threads_runnning() {
                let _ = screen.update_sixel_threads();
                tokio::task::yield_now().await;
            }
        }

        // Send any terminal query responses collected during parsing
        if !self.output_buffer.is_empty() {
            if let Some(conn) = &mut self.connection {
                let _ = conn.send(&self.output_buffer).await;
            }
            self.output_buffer.clear();
        }
    }

    async fn start_upload(&mut self, protocol: TransferProtocolType, files: Vec<PathBuf>) {
        if let Some(conn) = &mut self.connection {
            let mut prot = protocol.create();
            match prot.initiate_send(&mut **conn, &files).await {
                Ok(state) => {
                    self.current_transfer = Some(state.clone());
                    self.send_event(TerminalEvent::TransferStarted(state.clone(), false));

                    // Run the file transfer
                    if let Err(e) = self.run_file_transfer(prot.as_mut(), state).await {
                        log::error!("Upload error: {}", e);
                        self.send_event(TerminalEvent::Error(format!("Upload failed."), format!("{}", e)));
                    }
                }
                Err(e) => {
                    self.send_event(TerminalEvent::Error(format!("Upload failed."), format!("{}", e)));
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
                    self.send_event(TerminalEvent::TransferStarted(state.clone(), true));

                    // Run the file transfer
                    if let Err(e) = self.run_file_transfer(prot.as_mut(), state).await {
                        log::error!("Download error: {}", e);
                        self.send_event(TerminalEvent::Error(format!("Download failed."), format!("{}", e)));
                    }
                }
                Err(e) => {
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
        copy_downloaded_files(&mut transfer_state)?;

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

    fn write_to_capture(&mut self, data: &[u8]) {
        if let Some(writer) = &mut self.capture_writer {
            use std::io::Write;
            if let Err(e) = writer.write_all(data) {
                log::error!("Failed to write to capture file: {}", e);
                // Close the capture file on error
                self.capture_writer = None;
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
                        let buffer_type = if let Ok(screen) = self.edit_screen.lock() {
                            screen.buffer_type()
                        } else {
                            icy_engine::BufferType::CP437 // Default fallback
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
    edit_screen: Arc<Mutex<Box<dyn EditableScreen>>>,
    terminal_type: TerminalEmulation,
) -> (mpsc::UnboundedSender<TerminalCommand>, mpsc::UnboundedReceiver<TerminalEvent>) {
    use icy_parser_core::MusicOption;
    let parser = crate::get_parser(&terminal_type, MusicOption::Off, ScreenMode::default());

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
