use crate::auto_login::{AutoLoginCommand, AutoLoginParser};
use crate::emulated_modem::{EmulatedModem, ModemCommand};
use crate::features::AutoTransferScanner;
use crate::scripting::ScriptRunner;
use crate::ui::open_serial_dialog::BAUD_RATES;
use crate::TransferProtocol;
use crate::{normalize_screen_mode, ConnectionInformation, SshAuthenticationMode};
use base64::{engine::general_purpose, Engine as _};
use directories::UserDirs;
use icy_engine::{CreationOptions, GraphicsType, Screen, ScreenMode, ScreenSink, Sixel, Size};
use icy_engine_gui::music::audio_apc::{self, AudioApcCommand, AudioFeatureQuery};
use icy_engine_gui::music::sound_effects::sound_data;
use icy_engine_gui::util::BaudEmulator;
use icy_engine_gui::util::QueuedCommand;
use icy_engine_gui::util::QueueingSink;
use icy_net::iemsi::{complete_iemsi_handshake, EmsiISI, ICITerminalSettings, ICIUserSettings, IEmsi};
use icy_net::rlogin::RloginConfig;
use icy_net::{
    modem::{ModemConfiguration, ModemConnection, ModemResponseType},
    protocol::{Protocol, TransferState},
    raw::RawConnection,
    serial::{Serial, SerialConnection},
    ssh::{Credentials, PrivateKeyCredential, SSHConnection, SecretString, SshAuthentication, SshConnectionOptions},
    telnet::{TelnetConnection, TermCaps, TerminalEmulation},
    Connection, ConnectionState, ConnectionType,
};
use icy_parser_core::{AnsiMusic, CommandParser, TerminalRequest};
use icy_parser_core::{AskQuery, CaretShape, DeviceControlString, IgsCommand, SkypixCommand, StopType};
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
const MAX_CACHED_MEDIA_SIZE: usize = 12 * 1024 * 1024;
const MAX_CACHE_FILENAME_LEN: usize = 128;

/// Client pixel buffers a board may load a frame into, addressed by `B=`.
const PIXEL_BUFFERS: usize = 2;

enum CachedMediaCommand<'a> {
    Store { filename: &'a str, encoded: &'a str },
    DrawJxl { filename: &'a str, options: &'a str },
    LoadBlob { buffer: usize, encoded: &'a str },
    PasteBlob { buffer: usize, options: &'a str },
    List { pattern: &'a str },
}

fn image_buffer(options: &str) -> Option<usize> {
    options
        .split(';')
        .find_map(|option| option.strip_prefix("B="))
        .and_then(|buffer| buffer.parse::<usize>().ok())
        .filter(|buffer| *buffer < PIXEL_BUFFERS)
}

/// Validates a cache-relative name. Audio doors namespace their uploads
/// (`sfx/12`), so `/`-separated segments are allowed but traversal is not.
fn valid_cache_filename(filename: &str) -> bool {
    if filename.is_empty() || filename.len() > MAX_CACHE_FILENAME_LEN {
        return false;
    }
    filename.split('/').all(|segment| {
        !segment.is_empty()
            && segment != "."
            && segment != ".."
            && segment.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    })
}

fn parse_cached_media_command(data: &[u8]) -> Option<CachedMediaCommand<'_>> {
    let command = std::str::from_utf8(data).ok()?;
    if let Some(arguments) = command.strip_prefix("SyncTERM:C;S;") {
        let (filename, encoded) = arguments.split_once(';')?;
        return valid_cache_filename(filename).then_some(CachedMediaCommand::Store { filename, encoded });
    }
    if let Some(arguments) = command.strip_prefix("SyncTERM:C;DrawJXL;") {
        let (options, filename) = arguments.rsplit_once(';').map_or(("", arguments), |(options, filename)| (options, filename));
        return valid_cache_filename(filename).then_some(CachedMediaCommand::DrawJxl { filename, options });
    }
    if let Some(arguments) = command.strip_prefix("SyncTERM:C;LoadJXLBlob;") {
        let (options, encoded) = arguments.split_once(';')?;
        return image_buffer(options).map(|buffer| CachedMediaCommand::LoadBlob { buffer, encoded });
    }
    if let Some(options) = command.strip_prefix("SyncTERM:P;Paste;") {
        return image_buffer(options).map(|buffer| CachedMediaCommand::PasteBlob { buffer, options });
    }
    // Last of the `C;` verbs, so it cannot swallow `LoadJXLBlob`.
    if let Some(arguments) = command.strip_prefix("SyncTERM:C;L") {
        return Some(CachedMediaCommand::List {
            pattern: arguments.strip_prefix(';').unwrap_or("*"),
        });
    }
    None
}

/// The cache entries matching `pattern`, each with its digest, so a board can send
/// only what the caller is missing.
fn cache_listing(cache_directory: &std::path::Path, pattern: &str) -> String {
    let mut entries = Vec::new();
    collect_cache_entries(cache_directory, "", &mut entries);
    entries.sort();
    entries
        .iter()
        .filter(|name| cache_glob_matches(pattern, name))
        .filter_map(|name| {
            let data = std::fs::read(cache_directory.join(name)).ok()?;
            Some(format!("{name}\t{:x}\n", md5::compute(&data)))
        })
        .collect()
}

fn collect_cache_entries(directory: &std::path::Path, prefix: &str, entries: &mut Vec<String>) {
    let Ok(listing) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in listing.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let relative = format!("{prefix}{name}");
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            collect_cache_entries(&entry.path(), &format!("{relative}/"), entries);
        } else {
            entries.push(relative);
        }
    }
}

/// The listing glob, which in practice is a plain prefix such as `gfx/*`.
fn cache_glob_matches(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        Some((head, tail)) => name.len() >= head.len() + tail.len() && name.starts_with(head) && name.ends_with(tail),
        None => pattern == name,
    }
}

async fn store_cached_media(cache_directory: &std::path::Path, filename: &str, encoded: &str) -> Result<(), String> {
    if encoded.len() > MAX_CACHED_MEDIA_SIZE * 4 / 3 + 4 {
        return Err(format!("cached media file {filename} is too large"));
    }
    let bytes = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| format!("cached media file {filename} has invalid base64"))?;
    if bytes.len() > MAX_CACHED_MEDIA_SIZE {
        return Err(format!("cached media file {filename} is too large"));
    }
    let destination = cache_directory.join(filename);
    let parent = destination.parent().unwrap_or(cache_directory).to_path_buf();
    tokio::fs::create_dir_all(&parent)
        .await
        .map_err(|err| format!("unable to create media cache directory: {err}"))?;
    let leaf = destination.file_name().and_then(|name| name.to_str()).unwrap_or("media");
    let temporary = parent.join(format!(".{leaf}.new"));
    let _ = tokio::fs::remove_file(&temporary).await;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .await
        .map_err(|err| format!("unable to create cached media file {filename}: {err}"))?;
    if let Err(err) = file.write_all(&bytes).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(format!("unable to store cached media file {filename}: {err}"));
    }
    if let Err(err) = file.sync_all().await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(format!("unable to sync cached media file {filename}: {err}"));
    }
    drop(file);
    if cfg!(windows) && destination.exists() {
        let _ = tokio::fs::remove_file(&destination).await;
    }
    if let Err(err) = tokio::fs::rename(&temporary, &destination).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(format!("unable to publish cached media file {filename}: {err}"));
    }
    Ok(())
}

async fn read_cached_media(cache_directory: &std::path::Path, filename: &str) -> Option<Vec<u8>> {
    let source = cache_directory.join(filename);
    let metadata = tokio::fs::metadata(&source).await.ok()?;
    if metadata.len() > MAX_CACHED_MEDIA_SIZE as u64 {
        return None;
    }
    tokio::fs::read(source).await.ok()
}

fn mode_report_status(enabled: Option<bool>) -> u8 {
    match enabled {
        Some(true) => 1,
        Some(false) => 2,
        None => 0,
    }
}

fn ansi_mode_report_status(screen: &dyn Screen, mode: u16) -> u8 {
    mode_report_status(match mode {
        4 => Some(screen.caret().insert_mode),
        20 => Some(screen.terminal_state().lf_expand),
        _ => None,
    })
}

fn dec_mode_report_status(screen: &dyn Screen, mode: u16) -> u8 {
    let state = screen.terminal_state();
    mode_report_status(match mode {
        4 => Some(matches!(state.scroll_state, icy_engine::TerminalScrolling::Smooth)),
        5 => Some(state.inverse_video),
        6 => Some(matches!(state.origin_mode, icy_engine::OriginMode::WithinMargins)),
        7 => Some(matches!(state.auto_wrap_mode, icy_engine::AutoWrapMode::AutoWrap)),
        9 => Some(matches!(state.mouse_mode(), icy_engine::MouseMode::X10)),
        25 => Some(screen.caret().visible),
        33 => Some(state.ice_colors),
        35 => Some(screen.caret().blinking),
        69 => Some(state.dec_left_right_margins()),
        80 => Some(!state.sixel_at_cursor),
        1000 => Some(matches!(state.mouse_mode(), icy_engine::MouseMode::VT200)),
        1001 => Some(matches!(state.mouse_mode(), icy_engine::MouseMode::VT200_Highlight)),
        1002 => Some(matches!(state.mouse_mode(), icy_engine::MouseMode::ButtonEvents)),
        1003 => Some(matches!(state.mouse_mode(), icy_engine::MouseMode::AnyEvents)),
        1004 => Some(state.mouse_state.focus_out_event_enabled),
        1005 => Some(matches!(state.mouse_state.extended_mode, icy_engine::ExtMouseMode::ExtendedUTF8)),
        1006 => Some(matches!(state.mouse_state.extended_mode, icy_engine::ExtMouseMode::SGR)),
        1007 => Some(state.mouse_state.alternate_scroll_enabled),
        1015 => Some(matches!(state.mouse_state.extended_mode, icy_engine::ExtMouseMode::URXVT)),
        1016 => Some(matches!(state.mouse_state.extended_mode, icy_engine::ExtMouseMode::PixelPosition)),
        1070 => Some(!state.sixel_shared_palette),
        2004 => Some(state.bracketed_paste_mode),
        2026 => Some(state.synchronized_output()),
        _ => None,
    })
}

fn decrqss_status(screen: &dyn Screen, selector: &[u8]) -> Option<String> {
    let state = screen.terminal_state();
    match selector {
        b"r" => {
            let (top, bottom) = state.margins_top_bottom().unwrap_or((0, screen.height() - 1));
            Some(format!("{};{}r", top + 1, bottom + 1))
        }
        b"s" => {
            let (left, right) = state.margins_left_right().unwrap_or((0, screen.width() - 1));
            Some(format!("{};{}s", left + 1, right + 1))
        }
        b"t" => Some(format!("{}t", screen.height())),
        b"$|" => Some(format!("{}$|", screen.width())),
        b"*|" => Some(format!("{}*|", screen.height())),
        b" q" => {
            let caret = screen.caret();
            let style = match (caret.shape, caret.blinking) {
                (CaretShape::Block, true) => 1,
                (CaretShape::Block, false) => 2,
                (CaretShape::Underline, true) => 3,
                (CaretShape::Underline, false) => 4,
                (CaretShape::Bar, true) => 5,
                (CaretShape::Bar, false) => 6,
            };
            Some(format!("{style} q"))
        }
        b"*r" => {
            let baud = match state.baud_emulation() {
                BaudEmulation::Rate(300) => 1,
                BaudEmulation::Rate(600) => 2,
                BaudEmulation::Rate(1200) => 3,
                BaudEmulation::Rate(2400) => 4,
                BaudEmulation::Rate(4800) => 5,
                BaudEmulation::Rate(9600) => 6,
                BaudEmulation::Rate(19200) => 7,
                BaudEmulation::Rate(38400) => 8,
                BaudEmulation::Rate(57600) => 9,
                BaudEmulation::Rate(76800) => 10,
                BaudEmulation::Rate(115_200) => 11,
                BaudEmulation::Off | BaudEmulation::Rate(_) => 0,
            };
            Some(format!("0;{baud}*r"))
        }
        b"m" => {
            let attr = screen.caret().attribute;
            let mut params = vec!["0".to_string()];
            if attr.is_bold() {
                params.push("1".to_string());
            }
            if attr.is_faint() {
                params.push("2".to_string());
            }
            if attr.is_italic() {
                params.push("3".to_string());
            }
            if attr.is_underlined() {
                params.push("4".to_string());
            }
            if attr.is_blinking() {
                params.push("5".to_string());
            }
            if state.inverse_video {
                params.push("7".to_string());
            }
            if attr.is_concealed() {
                params.push("8".to_string());
            }
            if attr.is_crossed_out() {
                params.push("9".to_string());
            }
            append_sgr_color(&mut params, attr.foreground_color(), true);
            append_sgr_color(&mut params, attr.background_color(), false);
            Some(format!("{}m", params.join(";")))
        }
        _ => None,
    }
}

fn append_sgr_color(params: &mut Vec<String>, color: icy_engine::AttributeColor, foreground: bool) {
    let base = if foreground { 30 } else { 40 };
    match color {
        icy_engine::AttributeColor::Palette(index) if index < 8 => params.push((base + u32::from(index)).to_string()),
        icy_engine::AttributeColor::Palette(index) if index < 16 => params.push((base + 60 + u32::from(index - 8)).to_string()),
        icy_engine::AttributeColor::Palette(index) | icy_engine::AttributeColor::ExtendedPalette(index) => {
            params.extend([if foreground { "38" } else { "48" }.to_string(), "5".to_string(), index.to_string()]);
        }
        icy_engine::AttributeColor::Rgb(r, g, b) => {
            params.extend([
                if foreground { "38" } else { "48" }.to_string(),
                "2".to_string(),
                r.to_string(),
                g.to_string(),
                b.to_string(),
            ]);
        }
        icy_engine::AttributeColor::Transparent => params.push(if foreground { "39" } else { "49" }.to_string()),
    }
}

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
    /// Feed a captured file through the receive path, as if the host had sent it
    PlayFile(PathBuf),
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
    /// External protocol transfer started (`protocol_name`, `is_download`)
    ExternalTransferStarted(String, bool),
    /// External protocol transfer completed (`protocol_name`, `is_download`, success, `error_message`)
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

    /// Play a GIST sound effect (`BellsAndWhistles`)
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

    /// A `SyncTERM:A;` audio command, with the cache directory used by `Load`.
    AudioApc(AudioApcCommand, Option<PathBuf>),

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
    /// Request UI redraw after screen changes
    RequestRedraw,
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

    pub ssh_authentication: SshAuthenticationMode,
    pub ssh_private_key: Option<PathBuf>,
    pub ssh_key_passphrase: Option<String>,

    pub proxy_command: Option<String>,
    /// Optional SOCKS5 proxy for TCP-based connections (Tor/I2P).
    pub proxy: Option<icy_net::proxy::ProxyConfig>,
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

    /// Whether mouse reporting is enabled for this connection
    pub mouse_reporting_enabled: bool,
    pub lf_expand: bool,
    pub custom_palette: Option<Vec<[u8; 3]>>,
    pub default_cursor_shape: CaretShape,
    pub default_cursor_blinking: bool,
    pub cache_directory: Option<PathBuf>,
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
    iemsi_scanner: Option<IEmsi>,
    iemsi_user_settings: Option<ICIUserSettings>,
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

    /// Modem configuration for hangup command
    modem_config: Option<ModemConfiguration>,
    cache_directory: Option<PathBuf>,
    /// Bytes injected by `PlayFile`, drained into the receive path by the run loop.
    injected_data: Vec<u8>,
    /// Completion generation preceding the last queue on each channel.
    audio_queue_generation: [u32; audio_apc::CHANNELS],
    /// Generation an `Update` command is waiting to see advance.
    audio_notify_generation: [Option<u32>; audio_apc::CHANNELS],
    /// Frames held by `SyncTERM:C;LoadJXLBlob` until a `SyncTERM:P;Paste` places them.
    pixel_buffers: [Option<Vec<u8>>; PIXEL_BUFFERS],
}

impl TerminalThread {
    fn new(
        edit_screen: Arc<Mutex<Box<dyn Screen>>>,
        parser: Box<dyn CommandParser + Send>,
        address_book: Arc<Mutex<crate::data::AddressBook>>,
        command_tx: mpsc::UnboundedSender<TerminalCommand>,
        command_rx: mpsc::UnboundedReceiver<TerminalCommand>,
        event_tx: mpsc::UnboundedSender<TerminalEvent>,
    ) -> Self {
        Self {
            edit_screen,
            connection: None,
            parser,
            current_transfer: None,
            connection_time: None,
            command_rx,
            command_tx,
            event_tx,
            use_utf8: false,
            utf8_buffer: Vec::new(),
            auto_transfer_scanner: AutoTransferScanner::default(),
            transfer_protocols: Vec::new(),
            baud_emulator: BaudEmulator::new(),
            iemsi_scanner: None,
            iemsi_user_settings: None,
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
            modem_config: None,
            cache_directory: None,
            injected_data: Vec::new(),
            audio_queue_generation: [0; audio_apc::CHANNELS],
            audio_notify_generation: [None; audio_apc::CHANNELS],
            pixel_buffers: std::array::from_fn(|_| None),
        }
    }

    pub fn spawn(
        edit_screen: Arc<Mutex<Box<dyn Screen>>>,
        parser: Box<dyn CommandParser + Send>,
        address_book: Arc<Mutex<crate::data::AddressBook>>,
    ) -> (mpsc::UnboundedSender<TerminalCommand>, mpsc::UnboundedReceiver<TerminalEvent>) {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let mut thread = Self::new(edit_screen, parser, address_book, command_tx.clone(), command_rx, event_tx.clone());

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
        (0..20).map(|i| sound_data(i).map_or_else(|| vec![0i16; 56], |data| data.to_vec())).collect()
    }

    async fn run(&mut self) {
        let mut read_buffer = vec![0u8; 64 * 1024];
        let mut pending_data: Vec<u8> = Vec::new();
        let mut pending_offset: usize = 0;
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

                    self.poll_audio_notifications().await;

                    if !self.injected_data.is_empty() {
                        pending_data.append(&mut self.injected_data);
                    }

                    // Process pending data with baud emulation
                    if pending_offset < pending_data.len() {
                        let remaining = pending_data.len() - pending_offset;
                        let bytes_to_send = self.baud_emulator.calculate_bytes_to_send(remaining);
                        if bytes_to_send > 0 {
                            let end = pending_offset + bytes_to_send;
                            let chunk = &pending_data[pending_offset..end];
                            self.write_to_capture(chunk).await;
                            self.process_data(chunk).await;
                            pending_offset = end;

                            // Clear buffer when fully processed
                            if pending_offset >= pending_data.len() {
                                pending_data.clear();
                                pending_offset = 0;
                            }
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
                            log::warn!("Unknown protocol id for auto-transfer: {protocol_id}");
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
                                        error!("Connection poll error: {e}");
                                        self.disconnect().await;
                                        self.process_data(format!("\n\r{e}").as_bytes()).await;
                                        continue;
                                    }
                                }
                            }
                        } else {
                            poll_interval += 1;
                        }

                        // Read new data and add to pending buffer
                        if let Some(new_data) = self.read_connection_raw(&mut read_buffer).await {
                            if !new_data.is_empty() {
                                pending_data.extend_from_slice(&new_data);
                            }
                        }
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
                let commands = if config.auto_login_exp.is_empty() {
                    None
                } else {
                    match AutoLoginParser::parse(&config.auto_login_exp) {
                        Ok(commands) => Some(commands),
                        Err(error) => {
                            log::error!("Failed to parse auto-login expression: {error}");
                            None
                        }
                    }
                };
                let user_name = config.user_name.clone();
                let password = config.password.clone();
                let terminal_type = config.terminal_type;

                match self.connect(config).await {
                    Ok(()) => {
                        if let Some(commands) = commands {
                            self.auto_login(&commands, user_name, password, terminal_type).await;
                        }
                    }
                    Err(e) => {
                        log::error!("{e}");
                        self.process_data("NO CARRIER\r\n".to_string().as_bytes()).await;
                        self.send_event(TerminalEvent::Disconnected(Some(e.to_string())));
                    }
                }
            }

            TerminalCommand::OpenSerial(serial) => {
                if let Err(e) = self.open_serial(serial).await {
                    log::error!("{e}");
                    self.process_data("FAILED.\r\n".to_string().as_bytes()).await;
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
                        log::error!("Failed to send data: {err}");
                        self.disconnect().await;
                        self.process_data(format!("\n\r{err}").as_bytes()).await;
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
                            self.process_data("\r\nCALLING...\r\n".to_string().as_bytes()).await;
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
                    log::error!("Failed to create capture file {file_name}: {e}");
                    self.send_event(TerminalEvent::Error(format!("Failed to create capture file: {file_name}"), format!("{e}")));
                }
            },
            TerminalCommand::StopCapture => {
                if let Some(mut writer) = self.capture_writer.take() {
                    let _ = writer.flush().await; // Ensure final flush
                }
            }
            TerminalCommand::PlayFile(path) => match tokio::fs::read(&path).await {
                Ok(data) => {
                    log::info!("Playing {} ({} bytes) into the terminal", path.display(), data.len());
                    self.injected_data.extend_from_slice(&data);
                }
                Err(err) => {
                    log::error!("Failed to read play file {}: {}", path.display(), err);
                    self.send_event(TerminalEvent::Error(format!("Failed to read {}", path.display()), format!("{err}")));
                }
            },
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
                self.set_terminal_settings(terminal_type, screen_mode, ansi_music);
            }
        }
    }

    async fn connect(&mut self, config: ConnectionConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.connection.is_some() {
            self.disconnect().await;
        }

        self.use_utf8 = config.terminal_type == TerminalEmulation::Utf8Ansi;
        self.baud_emulator.set_baud_rate(config.baud_emulation);
        self.cache_directory.clone_from(&config.cache_directory);

        if !matches!(config.connection_info.protocol(), ConnectionType::Modem) {
            self.process_data(format!("ATDT{}\r\n", config.connection_info).as_bytes()).await;
        }
        self.setup_auto_login(&config);

        let connection: Box<dyn Connection> = match config.connection_info.protocol() {
            ConnectionType::Telnet => {
                let term_caps = TermCaps {
                    terminal: config.terminal_type,
                    window_size: config.window_size,
                };
                Box::new(TelnetConnection::open_with_proxy(&config.connection_info.endpoint(), term_caps, config.timeout, config.proxy.as_ref()).await?)
            }
            ConnectionType::Raw => Box::new(RawConnection::open_with_proxy(&config.connection_info.endpoint(), config.timeout, config.proxy.as_ref()).await?),
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

                let user_name = user_name.unwrap_or_default();
                let password = password.unwrap_or_default();
                let passphrase = config.ssh_key_passphrase.map(SecretString::new);
                let authentication = match config.ssh_authentication {
                    SshAuthenticationMode::Password => SshAuthentication::Password {
                        password: SecretString::new(password),
                    },
                    SshAuthenticationMode::PrivateKey => SshAuthentication::PrivateKey {
                        path: config.ssh_private_key.ok_or("SSH private-key authentication requires a key file")?,
                        passphrase,
                    },
                    SshAuthenticationMode::Agent => SshAuthentication::Agent { public_key: None },
                    SshAuthenticationMode::Auto => SshAuthentication::Auto {
                        private_keys: config
                            .ssh_private_key
                            .map(|path| vec![PrivateKeyCredential { path, passphrase }])
                            .unwrap_or_default(),
                        use_agent: true,
                        password: (!password.is_empty()).then(|| SecretString::new(password)),
                    },
                };
                let creds = Credentials {
                    user_name,
                    authentication,
                    proxy_command: config.proxy_command.clone(),
                };
                let mut options = SshConnectionOptions::insecure_compatibility(creds);
                options.proxy = config.proxy.clone();
                Box::new(SSHConnection::open_with_options(&config.connection_info.endpoint(), term_caps, options).await?)
            }
            ConnectionType::Modem => {
                let Some(m) = &config.modem else {
                    return Err("Modem configuration is required for modem connections".into());
                };
                let modem_config = m.clone();
                let mut modem_conn: Box<dyn Connection> = Box::new(ModemConnection::open(modem_config.clone()).await?);

                // Send init command and wait for OK
                modem_config.init_command.send(modem_conn.as_mut()).await?;

                // Wait for OK response with timeout
                let init_timeout = Duration::from_secs(10);
                match self
                    .wait_for_modem_response(
                        &mut modem_conn,
                        &modem_config,
                        init_timeout,
                        &[ModemResponseType::Ok],
                        &[ModemResponseType::Error],
                    )
                    .await
                {
                    Ok(ModemResponseType::Ok) => {}
                    Ok(response) => {
                        return Err(format!("Modem init failed with response: {response:?}").into());
                    }
                    Err(e) => {
                        return Err(format!("Modem init timeout or error: {e}").into());
                    }
                }

                // Send dial command and wait for CONNECT
                let phone_number = config.connection_info.endpoint();
                modem_config.dial_prefix.send(modem_conn.as_mut()).await?;
                modem_conn.send(phone_number.as_bytes()).await?;
                modem_config.dial_suffix.send(modem_conn.as_mut()).await?;

                // Wait for CONNECT with longer timeout (60 seconds for dial)
                let dial_timeout = Duration::from_mins(1);
                let error_responses = [
                    ModemResponseType::NoCarrier,
                    ModemResponseType::Error,
                    ModemResponseType::NoDialtone,
                    ModemResponseType::Busy,
                    ModemResponseType::NoAnswer,
                ];
                match self
                    .wait_for_modem_response(&mut modem_conn, &modem_config, dial_timeout, &[ModemResponseType::Connect], &error_responses)
                    .await
                {
                    Ok(ModemResponseType::Connect) => {}
                    Ok(response) => {
                        modem_config.hangup_command.send(modem_conn.as_mut()).await.ok();
                        return Err(format!("Dial failed: {response:?}").into());
                    }
                    Err(e) => {
                        modem_config.hangup_command.send(modem_conn.as_mut()).await.ok();
                        return Err(format!("Dial timeout or error: {e}").into());
                    }
                }

                // Store modem config for hangup on disconnect
                self.modem_config = Some(modem_config);
                modem_conn
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
                Box::new(icy_net::rlogin::RloginConnection::open_with_proxy(&config.connection_info.endpoint(), rlogin_config, config.timeout, config.proxy.as_ref()).await?)
            }
            ConnectionType::RloginSwapped => {
                let rlogin_config = RloginConfig {
                    user_name: config.user_name.clone().unwrap_or_default(),
                    password: config.password.clone().unwrap_or_default(),
                    terminal_emulation: config.terminal_type,
                    swapped: true,
                    escape_sequence: None,
                };
                Box::new(icy_net::rlogin::RloginConnection::open_with_proxy(&config.connection_info.endpoint(), rlogin_config, config.timeout, config.proxy.as_ref()).await?)
            }
            other => {
                return Err(format!("Unsupported connection type: {other:?}").into());
            }
        };

        self.connection = Some(connection);
        self.connection_time = Some(Instant::now());

        let screen_mode = normalize_screen_mode(config.terminal_type, config.screen_mode);
        let (mut new_screen, parser) = screen_mode.create_screen(config.terminal_type, Some(CreationOptions { ansi_music: config.ansi_music }));
        {
            new_screen.set_scrollback_buffer_size(config.max_scrollback_lines);
            new_screen.caret_mut().shape = config.default_cursor_shape;
            new_screen.caret_mut().blinking = config.default_cursor_blinking;
            if let Some(colors) = config.custom_palette.as_ref().filter(|colors| colors.len() == 16) {
                let colors: Vec<icy_engine::Color> = colors.iter().map(|[r, g, b]| icy_engine::Color::new(*r, *g, *b)).collect();
                *new_screen.palette_mut() = icy_engine::Palette::from_slice(&colors);
            }
            // Set mouse tracking enabled based on connection config
            new_screen.terminal_state_mut().mouse_state.mouse_tracking_enabled = config.mouse_reporting_enabled;
            new_screen.terminal_state_mut().lf_expand = config.lf_expand;
            let mut screen = self.edit_screen.lock();
            *screen = new_screen;
        }
        self.parser = parser;
        // Update terminal emulation for scripting
        *self.terminal_emulation.lock() = config.terminal_type;
        // Build auto-transfer scanner from protocol list and store protocols for later lookup
        self.transfer_protocols.clone_from(&config.transfer_protocols);
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

            self.process_data(format!("Trying {baud_rate} baud...").as_bytes()).await;

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
                    self.process_data(" failed to open\r\n".to_string().as_bytes()).await;
                }
            }
        }

        if let Some(baud) = detected_baud {
            self.send_event(TerminalEvent::SerialBaudDetected(baud));
        } else {
            self.process_data("Auto-detection complete. No response detected.\r\n".to_string().as_bytes())
                .await;
        }
        // Always notify that auto-detection is complete so the dialog can be shown again
        self.send_event(TerminalEvent::SerialAutoDetectComplete);
    }

    async fn try_read_response(&mut self, detected_baud: &mut Option<u32>, baud_rate: u32, conn: &mut SerialConnection) -> bool {
        let mut buf = [0u8; 64];
        if (0..3).next().is_some() {
            match tokio::time::timeout(Duration::from_secs(1), conn.read(&mut buf)).await {
                Ok(Ok(n)) if n > 0 => {
                    // Check if response contains printable ASCII (valid at this baud rate)
                    let printable_count = buf[..n]
                        .iter()
                        .filter(|&&b| (b as char).is_ascii_alphabetic() || b == b'\r' || b == b'\n')
                        .count();
                    // println!("Read {}/{} bytes at {} baud: {}", n, printable_count, baud_rate, String::from_utf8_lossy(&buf[..n]));
                    if printable_count == n {
                        // More than half printable - likely correct baud rate
                        self.process_data(" detected!\r\n".to_string().as_bytes()).await;
                        *detected_baud = Some(baud_rate);
                        let _ = conn.shutdown().await;
                        return true;
                    }
                    self.process_data(" garbage response\r\n".to_string().as_bytes()).await;
                    return false;
                }
                _ => {
                    self.process_data(" no response\r\n".to_string().as_bytes()).await;
                    return false;
                }
            }
        }
        false
    }

    fn setup_auto_login(&mut self, config: &ConnectionConfig) {
        if !config.iemsi_auto_login {
            self.iemsi_scanner = None;
            self.iemsi_user_settings = None;
            return;
        }

        // Determine effective credentials with clear precedence
        let mut effective_user = config.user_name.as_ref().filter(|s: &&String| !s.is_empty()).cloned().or_else(|| {
            if config.connection_info.protocol() == ConnectionType::SSH {
                None
            } else {
                config.connection_info.user_name()
            }
        });

        let mut effective_pass = config.password.as_ref().filter(|s| !s.is_empty()).cloned().or_else(|| {
            if config.connection_info.protocol() == ConnectionType::SSH {
                None
            } else {
                config.connection_info.password()
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
                self.iemsi_scanner = Some(IEmsi::default());
                self.iemsi_user_settings = Some(ICIUserSettings {
                    name: user,
                    password: pass,
                    ..Default::default()
                });
            }
        }
    }

    async fn disconnect(&mut self) {
        if let Some(mut conn) = self.connection.take() {
            // For modem connections, send hangup command before closing
            if conn.get_connection_type() == ConnectionType::Modem {
                if let Some(modem_config) = &self.modem_config {
                    let _ = modem_config.hangup_command.send(conn.as_mut()).await;
                }
            }
            let _ = conn.shutdown().await;
        }
        self.modem_config = None;
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
        self.iemsi_scanner = None;
        self.iemsi_user_settings = None;
        self.auto_transfer_scanner = AutoTransferScanner::default();
        self.transfer_protocols.clear();
        self.send_event(TerminalEvent::Disconnected(None));
    }

    /// Wait for a modem response, displaying all modem output to the user.
    /// Returns the first matching response (success or error) or an error on timeout.
    async fn wait_for_modem_response(
        &mut self,
        modem_conn: &mut Box<dyn Connection>,
        modem_config: &ModemConfiguration,
        timeout_duration: Duration,
        success_responses: &[ModemResponseType],
        error_responses: &[ModemResponseType],
    ) -> Result<ModemResponseType, Box<dyn std::error::Error + Send + Sync>> {
        let start = Instant::now();
        let mut response_buffer = String::new();
        let mut read_buf = [0u8; 256];

        while start.elapsed() < timeout_duration {
            // Try to read data from modem
            match modem_conn.try_read(&mut read_buf).await {
                Ok(0) => {
                    // No data available, small sleep to avoid busy loop
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Ok(n) => {
                    let data = &read_buf[..n];
                    // Display modem output to user
                    self.process_data(data).await;

                    // Add to response buffer for pattern matching
                    if let Ok(text) = std::str::from_utf8(data) {
                        response_buffer.push_str(text);
                    } else {
                        // Try to handle as ASCII
                        for &byte in data {
                            if byte.is_ascii() {
                                response_buffer.push(byte as char);
                            }
                        }
                    }

                    // Check for success responses
                    for success in success_responses {
                        if let Some(pattern) = modem_config.modem_responses.get(success) {
                            let pattern_str = pattern.to_string();
                            if response_buffer.to_uppercase().contains(&pattern_str.to_uppercase()) {
                                return Ok(success.clone());
                            }
                        }
                    }

                    // Check for error responses
                    for error in error_responses {
                        if let Some(pattern) = modem_config.modem_responses.get(error) {
                            let pattern_str = pattern.to_string();
                            if response_buffer.to_uppercase().contains(&pattern_str.to_uppercase()) {
                                return Ok(error.clone());
                            }
                        }
                    }
                }
                Err(e) => {
                    return Err(format!("Modem read error: {e}").into());
                }
            }
        }

        Err("Timeout waiting for modem response".into())
    }

    /// Read raw data from connection without processing or baud emulation
    async fn read_connection_raw(&mut self, buffer: &mut [u8]) -> Option<Vec<u8>> {
        if let Some(conn) = &mut self.connection {
            match conn.try_read(buffer).await {
                Ok(0) => None,
                Ok(size) => Some(buffer[..size].to_vec()),
                Err(e) => {
                    error!("Connection read error: {e}");
                    self.disconnect().await;
                    self.process_data(format!("\n\r{e}").as_bytes()).await;
                    None
                }
            }
        } else {
            None
        }
    }

    #[async_recursion::async_recursion(?Send)]
    async fn process_data(&mut self, data: &[u8]) {
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

        // Check for auto-features after display
        for &byte in data {
            if let Some((protocol_id, is_download)) = self.auto_transfer_scanner.try_transfer(byte) {
                self.auto_transfer = Some((protocol_id, is_download, None));
            }

            // IEMSI auto-login: scan for EMSI_IRQ
            if let Some(scanner) = &mut self.iemsi_scanner {
                if scanner.scan_byte(byte) {
                    // EMSI_IRQ detected - perform handshake
                    if let (Some(conn), Some(user_settings)) = (&mut self.connection, &self.iemsi_user_settings) {
                        println!("[IEMSI] EMSI_IRQ detected, starting handshake...");
                        let terminal_settings = ICITerminalSettings::default();
                        match complete_iemsi_handshake(conn, user_settings, &terminal_settings, 5000).await {
                            Ok(Some(isi)) => {
                                println!("[IEMSI] Handshake successful, received ISI: {isi:?}");
                                log::info!("[IEMSI] Login successful: {}", isi.name);
                                let _ = self.event_tx.send(TerminalEvent::EmsiLogin(Box::new(isi)));
                                // Clear scanner after login
                                self.iemsi_scanner = None;
                                self.iemsi_user_settings = None;
                            }
                            Ok(None) => {
                                println!("[IEMSI] Handshake failed or timed out");
                                log::warn!("[IEMSI] Handshake failed or timed out");
                                scanner.reset();
                            }
                            Err(e) => {
                                println!("[IEMSI] Handshake error: {e}");
                                log::error!("[IEMSI] Handshake error: {e}");
                                scanner.reset();
                            }
                        }
                    }
                }
            }
        }
    }

    /// Check if command needs async processing (delays, sound, etc.)
    /// Returns true if command was handled
    async fn try_process_async_command(&mut self, cmd: &QueuedCommand) -> bool {
        match cmd {
            QueuedCommand::Aps(data) => {
                if self.process_audio_apc(data).await {
                    return true;
                }
                if self.process_image_apc(data).await {
                    if let Some(editable) = self.edit_screen.lock().as_editable() {
                        editable.mark_dirty();
                    }
                    self.send_event(TerminalEvent::RequestRedraw);
                }
                true
            }
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
                log::info!("SkyPix CRC transfer initiated: mode={mode:?}, filename={file_name:?}");
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

            QueuedCommand::Igs(IgsCommand::Noise { .. } | IgsCommand::LoadMidiBuffer { .. }) => true,

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
                let (shared, position, mut decoder) = {
                    let mut screen = self.edit_screen.lock();
                    let position = if screen.terminal_state().sixel_at_cursor {
                        screen.caret_position()
                    } else {
                        icy_engine::Position::default()
                    };
                    let Some(editable) = screen.as_editable() else {
                        return true;
                    };
                    let state = editable.terminal_state_mut();
                    (state.sixel_shared_palette, position, std::mem::take(&mut state.sixel_decoder))
                };
                let decoded = if shared {
                    Sixel::parse_from_with_decoder(&mut decoder, *aspect_ratio, *zero_color, *grid_size, sixel_data)
                } else {
                    Sixel::parse_from(*aspect_ratio, *zero_color, *grid_size, sixel_data)
                };
                match decoded {
                    Ok(mut sixel) => {
                        sixel.apply_raster_scale();
                        let displayed_height = sixel.height();
                        {
                            let mut screen = self.edit_screen.lock();
                            if shared {
                                if let Some(editable) = screen.as_editable() {
                                    editable.terminal_state_mut().sixel_decoder = decoder;
                                }
                            }
                            if let Some(editable) = screen.as_editable() {
                                editable.add_sixel(position, sixel);
                                if editable.terminal_state().sixel_at_cursor {
                                    let font_height = editable.font_dimensions().height.max(1);
                                    let rows = (displayed_height + font_height - 1) / font_height;
                                    editable.set_caret_position(icy_engine::Position::new(position.x, position.y + rows));
                                }
                            }
                        }
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                    Err(err) => {
                        if shared {
                            if let Some(editable) = self.edit_screen.lock().as_editable() {
                                editable.terminal_state_mut().sixel_decoder = decoder;
                            }
                        }
                        log::error!("Error loading sixel: {err}");
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
                        let mode = {
                            let screen = self.edit_screen.lock();
                            if let GraphicsType::IGS(mode) = screen.graphics_type() {
                                Some(mode)
                            } else {
                                None
                            }
                        };
                        if let Some(mode) = mode {
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

    /// Handles the `SyncTERM:A;` audio family and the `SyncTERM:Q;libsndfile*`
    /// capability probes. Returns true when the APC was an audio command.
    async fn process_audio_apc(&mut self, data: &[u8]) -> bool {
        let Ok(payload) = std::str::from_utf8(data) else {
            return false;
        };

        if let Some(query) = audio_apc::parse_feature_query(payload) {
            let reply = match query {
                AudioFeatureQuery::Sndfile => format!("\x1b[=7;{};1n", audio_apc::FEATURE_SNDFILE),
                AudioFeatureQuery::SndfileFormat { major, subtype } => {
                    let available = u8::from(audio_apc::supports_format(major, subtype));
                    format!("\x1b[=7;{};{major};{subtype};{available}n", audio_apc::FEATURE_SNDFILE_FORMAT)
                }
            };
            if let Some(conn) = &mut self.connection {
                let _ = conn.send(reply.as_bytes()).await;
            }
            return true;
        }

        let Some(command) = audio_apc::parse_audio_apc(payload) else {
            // Any other SyncTERM:A payload is still ours; swallow it rather than
            // letting it fall through to the image decoder.
            return payload.starts_with("SyncTERM:A;");
        };

        if let AudioApcCommand::Update { channel } = command {
            self.audio_notify_generation[channel as usize] = Some(self.audio_queue_generation[channel as usize]);
            return true;
        }
        if let AudioApcCommand::Queue { channel, .. } = &command {
            self.audio_queue_generation[*channel as usize] = audio_apc::status().completion_generation(*channel);
        }
        self.send_event(TerminalEvent::AudioApc(command, self.cache_directory.clone()));
        true
    }

    /// Emits `CSI = 7 ; <ch> ; 0 n` once for each armed channel that has drained.
    async fn poll_audio_notifications(&mut self) {
        let status = audio_apc::status();
        for channel in 0..audio_apc::CHANNELS {
            let Some(generation) = self.audio_notify_generation[channel] else {
                continue;
            };
            if status.completion_generation(channel as u8) == generation {
                continue;
            }
            self.audio_notify_generation[channel] = None;
            if let Some(conn) = &mut self.connection {
                let _ = conn.send(format!("\x1b[=7;{channel};0n").as_bytes()).await;
            }
        }
    }

    /// Decodes an image APC (inline blob or cached JXL frame) and adds the result to the screen.
    ///
    /// The decode runs without the screen lock held, so the render thread is never blocked by it.
    /// Returns true when a new overlay was placed.
    async fn process_image_apc(&mut self, data: &[u8]) -> bool {
        let (font, screen_size) = {
            let mut screen = self.edit_screen.lock();
            let Some(editable) = screen.as_editable() else {
                return false;
            };
            (editable.font_dimensions(), Size::new(editable.width(), editable.height()))
        };

        let decoded = match parse_cached_media_command(data) {
            Some(CachedMediaCommand::LoadBlob { buffer, encoded }) => {
                self.pixel_buffers[buffer] = general_purpose::STANDARD
                    .decode(encoded)
                    .ok()
                    .filter(|bytes| bytes.len() <= MAX_CACHED_MEDIA_SIZE);
                return false;
            }
            Some(CachedMediaCommand::PasteBlob { buffer, options }) => {
                let Some(bytes) = self.pixel_buffers[buffer].as_deref() else {
                    return false;
                };
                icy_engine::decode_image_blob(bytes, true, options, font, screen_size)
            }
            Some(CachedMediaCommand::List { pattern }) => {
                let listing = self
                    .cache_directory
                    .as_deref()
                    .map(|directory| cache_listing(directory, pattern))
                    .unwrap_or_default();
                if let Some(conn) = &mut self.connection {
                    let _ = conn.send(format!("\x1b_SyncTERM:C;L\n{listing}\x1b\\").as_bytes()).await;
                }
                return false;
            }
            Some(command) => {
                let Some(cache_directory) = self.cache_directory.clone() else {
                    return false;
                };
                match command {
                    CachedMediaCommand::Store { filename, encoded } => {
                        if let Err(err) = store_cached_media(&cache_directory, filename, encoded).await {
                            log::warn!("{err}");
                        }
                        return false;
                    }
                    CachedMediaCommand::DrawJxl { filename, options } => {
                        let Some(bytes) = read_cached_media(&cache_directory, filename).await else {
                            return false;
                        };
                        icy_engine::decode_image_blob(&bytes, true, options, font, screen_size)
                    }
                    CachedMediaCommand::LoadBlob { .. } | CachedMediaCommand::PasteBlob { .. } | CachedMediaCommand::List { .. } => return false,
                }
            }
            None => icy_engine::decode_image_apc(data, font, screen_size),
        };

        let Some((position, sixel)) = decoded else {
            return false;
        };
        let mut screen = self.edit_screen.lock();
        if let Some(editable) = screen.as_editable() {
            editable.add_sixel(position, sixel);
            true
        } else {
            false
        }
    }

    /// Process commands from queue with granular locking
    /// Commands are processed in batches with max 10ms lock duration
    /// Async commands (delays, sound) are processed outside of locks
    async fn process_command_queue(&mut self) {
        const MAX_LOCK_DURATION_MS: u64 = 10;
        let mut had_updates = false;
        while let Some(cmd) = self.command_queue.pop_front() {
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
                    had_updates = true;
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

        if had_updates {
            if let Some(editable) = self.edit_screen.lock().as_editable() {
                editable.mark_dirty();
            }
            self.send_event(TerminalEvent::RequestRedraw);
        }
    }

    async fn start_upload(&mut self, protocol: TransferProtocol, files: Vec<PathBuf>) {
        let download_dir = self.download_directory.clone().unwrap_or_else(|| PathBuf::from("."));
        let is_external = !protocol.is_internal();
        let protocol_name = protocol.get_name();

        let Some(mut prot) = protocol.create(download_dir) else {
            self.send_event(TerminalEvent::Error(
                "Upload failed.".to_string(),
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
                        log::error!("Upload error: {e}");
                        self.send_event(TerminalEvent::Error("Upload failed.".to_string(), format!("{e}")));
                    }
                }
            }
            Err(e) => {
                if is_external {
                    self.send_event(TerminalEvent::ExternalTransferCompleted(protocol_name, false, false, Some(format!("{e}"))));
                } else {
                    self.send_event(TerminalEvent::Error("Upload failed.".to_string(), format!("{e}")));
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
                "Download failed.".to_string(),
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
                        log::error!("Download error: {e}");
                        self.send_event(TerminalEvent::Error("Download failed.".to_string(), format!("{e}")));
                    }
                }
            }
            Err(e) => {
                if is_external {
                    self.send_event(TerminalEvent::ExternalTransferCompleted(protocol_name, true, false, Some(format!("{e}"))));
                } else {
                    self.send_event(TerminalEvent::Error("Download failed.".to_string(), format!("{e}")));
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

        // Copy downloaded files to the download
        if let Err(e) = copy_downloaded_files(&mut transfer_state, self.download_directory.as_ref()) {
            log::error!("Failed to copy downloaded files: {e}");
            self.send_event(TerminalEvent::Error("File copy failed".to_string(), format!("{e}")));
        }

        self.current_transfer = Some(transfer_state.clone());
        self.send_event(TerminalEvent::TransferCompleted(transfer_state));
        self.current_transfer = None;

        Ok(())
    }

    fn send_event(&mut self, evt: TerminalEvent) {
        if let Err(err) = self.event_tx.send(evt) {
            log::error!("Failed to send terminal event: {err}");
        }
    }

    async fn write_to_capture(&mut self, data: &[u8]) {
        if let Some(writer) = &mut self.capture_writer {
            if let Err(e) = writer.write(data).await {
                log::error!("Failed to write to capture file: {e}");
                // Close the capture file on error
                self.capture_writer = None;
            }
        }
    }

    async fn handle_terminal_request(&mut self, request: TerminalRequest) {
        use std::fmt::Write as _;
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
                Some(format!("\x1b[>65;{version};{hardware_options}c").into_bytes())
            }
            TerminalRequest::ExtendedDeviceAttributes => {
                /*
                    1 - Loadable fonts are availabe via Device Control Strings
                    2 - Bright Background (ie: DECSET 32) is supported
                    3 - Palette entries may be modified via an Operating System Command
                        string
                    4 - Pixel operations are supported (Sixel and inline PPM graphics)
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
                let y = pos.y.min(screen.height() - 1) + 1;
                let x = pos.x.min(screen.width() - 1) + 1;
                Some(format!("\x1B[{y};{x}R").into_bytes())
            }
            TerminalRequest::ScreenSizeReport => {
                let screen = self.edit_screen.lock();
                let height = screen.height();
                let width = screen.width();
                Some(format!("\x1B[{height};{width}R").into_bytes())
            }
            TerminalRequest::TextAreaPixelSizeReport => {
                let screen = self.edit_screen.lock();
                let font = screen.font_dimensions();
                Some(format!("\x1B[4;{};{}t", screen.height() * font.height, screen.width() * font.width).into_bytes())
            }
            TerminalRequest::CellPixelSizeReport => {
                let screen = self.edit_screen.lock();
                let font = screen.font_dimensions();
                Some(format!("\x1B[6;{};{}t", font.height, font.width).into_bytes())
            }
            TerminalRequest::GraphicsSizeReport => {
                let screen = self.edit_screen.lock();
                let font = screen.font_dimensions();
                let width = screen.width() * font.width;
                let height = screen.height() * font.height;
                Some(format!("\x1B[?2;0;{width};{height}S").into_bytes())
            }
            TerminalRequest::RequestStatusString(selector) => {
                let screen = self.edit_screen.lock();
                let status = decrqss_status(&**screen, selector);
                let supported = u8::from(status.is_some());
                let payload = status.unwrap_or_else(|| String::from_utf8_lossy(selector).into_owned());
                Some(format!("\x1BP{supported}$r{payload}\x1B\\").into_bytes())
            }
            TerminalRequest::JxlSupportReport => Some(b"\x1B[=1;1-n".to_vec()),
            TerminalRequest::KittyKeyboardQuery => {
                let flags = self.edit_screen.lock().terminal_state().kitty_keyboard.flags();
                Some(format!("\x1b[?{flags}u").into_bytes())
            }
            TerminalRequest::AudioChannelStateReport(channel) => {
                let status = audio_apc::status();
                let mut report = "\x1b[=7".to_string();
                match channel {
                    Some(channel) => {
                        let state = u8::from(status.is_active(*channel as u8));
                        let _ = write!(report, ";{channel};{state}");
                    }
                    None => {
                        for channel in 0..audio_apc::CHANNELS as u8 {
                            if status.is_active(channel) {
                                let _ = write!(report, ";{channel};1");
                            }
                        }
                    }
                }
                report.push('n');
                Some(report.into_bytes())
            }
            TerminalRequest::OscColorReport { foreground } => {
                let screen = self.edit_screen.lock();
                let attribute = screen.caret().attribute;
                let color = if *foreground {
                    attribute.foreground_color()
                } else {
                    attribute.background_color()
                };
                let (r, g, b) = color.as_rgb().unwrap_or_else(|| {
                    let index = if *foreground { attribute.foreground() } else { attribute.background() };
                    screen.palette().rgb(index)
                });
                let command = if *foreground { 10 } else { 11 };
                Some(
                    format!(
                        "\x1B]{};rgb:{:04x}/{:04x}/{:04x}\x1B\\",
                        command,
                        u16::from(r) * 257,
                        u16::from(g) * 257,
                        u16::from(b) * 257
                    )
                    .into_bytes(),
                )
            }
            TerminalRequest::OscPaletteColorReport { index } => {
                let screen = self.edit_screen.lock();
                let palette_index = icy_engine::ansi_to_internal_palette_index(u32::from(*index));
                let (r, g, b) = screen.palette().rgb(palette_index);
                Some(
                    format!(
                        "\x1B]4;{};rgb:{:04x}/{:04x}/{:04x}\x1B\\",
                        index,
                        u16::from(r) * 257,
                        u16::from(g) * 257,
                        u16::from(b) * 257
                    )
                    .into_bytes(),
                )
            }
            TerminalRequest::RequestTabStopReport => {
                let screen = self.edit_screen.lock();
                let mut response = b"\x1BP2$u".to_vec();
                let tab_count = screen.terminal_state().tab_count();
                for i in 0..tab_count {
                    let tab = screen.terminal_state().tabs()[i];
                    response.extend_from_slice((tab + 1).to_string().as_bytes());
                    if i < tab_count.saturating_sub(1) {
                        response.push(b'/');
                    }
                }
                response.extend_from_slice(b"\x1B\\");
                Some(response)
            }
            TerminalRequest::AnsiModeReport(mode) => {
                let screen = self.edit_screen.lock();
                Some(format!("\x1B[{};{}$y", mode, ansi_mode_report_status(&**screen, *mode)).into_bytes())
            }
            TerminalRequest::DecPrivateModeReport(mode) => {
                let screen = self.edit_screen.lock();
                Some(format!("\x1B[?{};{}$y", mode, dec_mode_report_status(&**screen, *mode)).into_bytes())
            }
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
                Some(format!("\x1BP{id}!~{checksum:04X}\x1B\\").into_bytes())
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
                let dim = screen.font_dimensions();
                Some(format!("\x1B[=3;{};{}n", dim.height, dim.width).into_bytes())
            }
            TerminalRequest::MacroSpaceReport => Some(b"\x1B[32767*{".to_vec()),
            TerminalRequest::MemoryChecksumReport(pid, checksum) => Some(format!("\x1BP{pid}!~{checksum:04X}\x1B\\").into_bytes()),
            TerminalRequest::RipRequestTerminalId => {
                let screen = self.edit_screen.lock();
                if screen.graphics_type() == GraphicsType::Rip {
                    Some(icy_engine::RIP_TERMINAL_ID.as_bytes().to_vec())
                } else {
                    None
                }
            }
            TerminalRequest::RipQueryFile(_)
            | TerminalRequest::RipQueryFileSize(_)
            | TerminalRequest::RipQueryFileDate(_)
            | TerminalRequest::RipReadFile(_) => {
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

    async fn flush_auto_login(&mut self, pending: &mut Vec<u8>) {
        if pending.is_empty() {
            return;
        }
        if let Some(connection) = &mut self.connection {
            if let Err(error) = connection.send(pending).await {
                log::error!("Auto-login send failed: {error}");
            }
        }
        pending.clear();
    }

    fn auto_login_control_code(terminal_type: TerminalEmulation, code: u8) -> Vec<u8> {
        if code == b'\r' {
            crate::scripting::parse_key_string(terminal_type, "enter").unwrap_or_else(|| vec![code])
        } else {
            vec![code]
        }
    }

    async fn wait_for_name_prompt(&mut self) {
        let timeout = tokio::time::Duration::from_secs(10);
        let start = tokio::time::Instant::now();
        let mut buffer = vec![0u8; 4096];
        let mut accumulated = String::new();

        while start.elapsed() < timeout {
            if let Some(data) = self.read_connection_raw(&mut buffer).await {
                accumulated.push_str(&String::from_utf8_lossy(&data).to_ascii_lowercase());
                self.process_data(&data).await;
                if ["name", "login", "user"].iter().any(|pattern| accumulated.contains(pattern)) {
                    return;
                }
                if accumulated.len() > 512 {
                    accumulated = accumulated.chars().rev().take(512).collect::<String>().chars().rev().collect();
                }
            } else {
                tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
            }
        }
        log::warn!("Auto-login timed out waiting for a name prompt");
    }

    async fn auto_login(&mut self, commands: &[AutoLoginCommand], user_name: Option<String>, password: Option<String>, terminal_type: TerminalEmulation) {
        // Extract user name parts
        let full_name = user_name.clone().unwrap_or_default();
        let parts: Vec<&str> = full_name.split_whitespace().collect();
        let first_name = parts.first().unwrap_or(&"").to_string();
        let last_name = parts.get(1..).map(|parts| parts.join(" ")).unwrap_or_default();
        let password = password.unwrap_or_default();
        let mut pending = Vec::new();

        for command in commands {
            match command {
                AutoLoginCommand::Delay(seconds) => {
                    self.flush_auto_login(&mut pending).await;
                    tokio::time::sleep(tokio::time::Duration::from_secs(*seconds as u64)).await;
                }

                AutoLoginCommand::EmulateMailerAccess => {
                    pending.extend(Self::auto_login_control_code(terminal_type, b'\r'));
                    pending.extend(Self::auto_login_control_code(terminal_type, b'\r'));
                    pending.push(0x1B);
                }

                AutoLoginCommand::WaitForNamePrompt => {
                    self.flush_auto_login(&mut pending).await;
                    self.wait_for_name_prompt().await;
                }

                AutoLoginCommand::SendFullName => pending.extend_from_slice(full_name.as_bytes()),
                AutoLoginCommand::SendFirstName => pending.extend_from_slice(first_name.as_bytes()),
                AutoLoginCommand::SendLastName => pending.extend_from_slice(last_name.as_bytes()),
                AutoLoginCommand::SendPassword => pending.extend_from_slice(password.as_bytes()),

                AutoLoginCommand::DisableIEMSI => {
                    self.iemsi_scanner = None;
                    self.iemsi_user_settings = None;
                }

                AutoLoginCommand::SendControlCode(code) => {
                    pending.extend(Self::auto_login_control_code(terminal_type, *code));
                }

                AutoLoginCommand::RunScript(filename) => {
                    log::warn!("Auto-login script files are not supported: {filename}");
                }

                AutoLoginCommand::SendText(text) => pending.extend_from_slice(text.as_bytes()),
            }
        }
        self.flush_auto_login(&mut pending).await;
    }
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

        if dest.exists() {
            let new_name = PathBuf::from(name);
            let stem = new_name.file_stem().map_or_else(|| "download".to_string(), |s| s.to_string_lossy().to_string());
            let ext = new_name.extension().map(|e| e.to_string_lossy().to_string());

            let mut i = 1;
            loop {
                let new_file_name = if let Some(ref e) = ext {
                    format!("{stem}.{i}.{e}")
                } else {
                    format!("{stem}.{i}")
                };
                dest = upload_location.join(&new_file_name);
                if !dest.exists() {
                    break;
                }
                i += 1;
            }
        }
        std::fs::copy(path, &dest)?;
        std::fs::remove_file(path)?;
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
                    crate::scripting::ScriptResult::Success | crate::scripting::ScriptResult::Stopped => TerminalEvent::ScriptFinished(Ok(())),
                    crate::scripting::ScriptResult::Error(e) => TerminalEvent::ScriptFinished(Err(e)),
                };
                self.send_event(event);
                self.script_runner = None;
            }
        }
    }

    /// Set terminal settings (terminal type, screen mode, ansi music) during session
    /// This reinitializes the screen and parser similar to `connect()`
    fn set_terminal_settings(&mut self, terminal_type: TerminalEmulation, screen_mode: ScreenMode, ansi_music: MusicOption) {
        self.use_utf8 = terminal_type == TerminalEmulation::Utf8Ansi;
        let screen_mode = normalize_screen_mode(terminal_type, screen_mode);
        let lf_expand = self.edit_screen.lock().terminal_state().lf_expand;

        // Create new screen and parser for the new terminal type
        let (mut new_screen, parser) = screen_mode.create_screen(terminal_type, Some(CreationOptions { ansi_music }));
        new_screen.terminal_state_mut().lf_expand = lf_expand;

        // Use a default scrollback buffer size
        new_screen.set_scrollback_buffer_size(10000);
        new_screen.mark_dirty();

        // Replace screen and parser
        {
            let mut screen = self.edit_screen.lock();
            *screen = new_screen;
        }
        self.parser = parser;

        // Request UI redraw
        self.send_event(TerminalEvent::RequestRedraw);

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

#[cfg(test)]
mod tests {
    use super::{
        ansi_mode_report_status, cache_listing, dec_mode_report_status, decrqss_status, parse_cached_media_command, read_cached_media, store_cached_media,
        valid_cache_filename, CachedMediaCommand, TerminalThread,
    };
    use async_trait::async_trait;
    use base64::{engine::general_purpose, Engine as _};
    use icy_engine::{EditableScreen, Position, Screen, Size, TextScreen};
    use icy_net::{telnet::TerminalEmulation, Connection, ConnectionType};
    use parking_lot::Mutex;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    #[derive(Clone, Default)]
    struct FakeConnection {
        sent: Arc<Mutex<Vec<u8>>>,
    }

    #[async_trait]
    impl Connection for FakeConnection {
        fn get_connection_type(&self) -> ConnectionType {
            ConnectionType::Raw
        }

        async fn read(&mut self, _buf: &mut [u8]) -> icy_net::Result<usize> {
            Ok(0)
        }

        async fn try_read(&mut self, _buf: &mut [u8]) -> icy_net::Result<usize> {
            Ok(0)
        }

        async fn send(&mut self, buf: &[u8]) -> icy_net::Result<()> {
            self.sent.lock().extend_from_slice(buf);
            Ok(())
        }
    }

    type TestTerminal = (TerminalThread, Arc<Mutex<Vec<u8>>>, Arc<Mutex<Box<dyn Screen>>>);

    fn test_terminal() -> TestTerminal {
        let screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(TextScreen::new(Size::new(80, 25)))));
        let sent = Arc::new(Mutex::new(Vec::new()));
        let connection = FakeConnection { sent: sent.clone() };
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, _event_rx) = mpsc::unbounded_channel();
        let mut terminal = TerminalThread::new(
            screen.clone(),
            Box::new(icy_parser_core::AnsiParser::new()),
            Arc::new(Mutex::new(crate::data::AddressBook::default())),
            command_tx,
            command_rx,
            event_tx,
        );
        terminal.connection = Some(Box::new(connection));
        (terminal, sent, screen)
    }

    async fn process_chunks(terminal: &mut TerminalThread, chunks: &[&[u8]]) {
        for chunk in chunks {
            terminal.process_data(chunk).await;
        }
    }

    #[derive(Clone, Copy)]
    struct SessionConfig {
        lf_expand: bool,
    }

    enum ChunkPlan {
        Whole,
        SplitAt(Vec<usize>),
        Bytewise,
    }

    struct CapturedSession<'a> {
        input: &'a [u8],
        config: SessionConfig,
        plans: Vec<ChunkPlan>,
        expected_replies: &'a [u8],
        expected_state: SessionStateDigest,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct SessionStateDigest {
        size: Size,
        kitty_flags: u8,
        mouse_mode: String,
        mouse_encoding: String,
        sixel_at_cursor: bool,
        sixel_shared_palette: bool,
        lf_expand: bool,
    }

    fn state_digest(screen: &dyn Screen) -> SessionStateDigest {
        let state = screen.terminal_state();
        SessionStateDigest {
            size: state.size(),
            kitty_flags: state.kitty_keyboard.flags(),
            mouse_mode: format!("{:?}", state.mouse_state.mouse_mode),
            mouse_encoding: format!("{:?}", state.mouse_state.extended_mode),
            sixel_at_cursor: state.sixel_at_cursor,
            sixel_shared_palette: state.sixel_shared_palette,
            lf_expand: state.lf_expand,
        }
    }

    async fn run_captured_session(session: &CapturedSession<'_>) {
        for plan in &session.plans {
            let (mut terminal, sent, screen) = test_terminal();
            if let Some(editable) = screen.lock().as_editable() {
                editable.terminal_state_mut().lf_expand = session.config.lf_expand;
            }

            match plan {
                ChunkPlan::Whole => terminal.process_data(session.input).await,
                ChunkPlan::Bytewise => {
                    for byte in session.input {
                        terminal.process_data(std::slice::from_ref(byte)).await;
                    }
                }
                ChunkPlan::SplitAt(boundaries) => {
                    let mut start = 0;
                    for end in boundaries.iter().copied().chain(std::iter::once(session.input.len())) {
                        terminal.process_data(&session.input[start..end]).await;
                        start = end;
                    }
                }
            }

            assert_eq!(&*sent.lock(), session.expected_replies);
            assert_eq!(state_digest(&**screen.lock()), session.expected_state);
        }
    }

    #[tokio::test]
    async fn captured_syncdoor_negotiation_is_chunk_invariant() {
        let input = b"\x1b[?1003;1016h\x1b[?80h\x1b[?1070l\x1b[>5u\x1b[?u\x1b[255n\x1b_SyncTERM:Q;JXL\x1b\\\x1b_SyncTERM:Q;libsndfile\x1b\\";
        let session = CapturedSession {
            input,
            config: SessionConfig { lf_expand: true },
            plans: vec![ChunkPlan::Whole, ChunkPlan::SplitAt(vec![1, 7, 22, 43, 67]), ChunkPlan::Bytewise],
            expected_replies: b"\x1b[?5u\x1B[25;80R\x1B[=1;1-n\x1b[=7;100;1n",
            expected_state: SessionStateDigest {
                size: Size::new(80, 25),
                kitty_flags: 5,
                mouse_mode: "AnyEvents".to_string(),
                mouse_encoding: "PixelPosition".to_string(),
                sixel_at_cursor: false,
                sixel_shared_palette: true,
                lf_expand: true,
            },
        };

        run_captured_session(&session).await;
    }

    #[tokio::test]
    async fn remote_queries_report_geometry_across_chunk_boundaries() {
        let (mut terminal, sent, _) = test_terminal();

        process_chunks(&mut terminal, &[b"\x1b[25", b"5n\x1b[14", b"t\x1b[16t"]).await;

        assert_eq!(&*sent.lock(), b"\x1B[25;80R\x1B[4;400;640t\x1B[6;16;8t");
    }

    #[tokio::test]
    async fn remote_mode_negotiation_updates_state_and_exact_replies() {
        let (mut terminal, sent, screen) = test_terminal();
        let input = b"\x1b[?1003;1006;1016h\x1b[?80h\x1b[?1070l\x1b[>5u\x1b[?u\x1b[?1003$p\x1b[?1006$p\x1b[?1016$p\x1b[?80$p\x1b[?1070$p";

        process_chunks(&mut terminal, &[&input[..17], &input[17..41], &input[41..]]).await;

        let state = screen.lock();
        assert_eq!(state.terminal_state().kitty_keyboard.flags(), 5);
        assert!(!state.terminal_state().sixel_at_cursor);
        assert!(state.terminal_state().sixel_shared_palette);
        assert_eq!(&*sent.lock(), b"\x1b[?5u\x1B[?1003;1$y\x1B[?1006;2$y\x1B[?1016;1$y\x1B[?80;1$y\x1B[?1070;2$y");
    }

    #[tokio::test]
    async fn rectangular_margin_rows_survive_startup_and_repaint_chunking() {
        let mut input = String::from("\x1b[5;18r\x1b[?69h\x1b[18;63s");
        for (row, marker) in (5..=18).zip('A'..='N') {
            input.push_str(&format!("\x1b[{row};18H\x1b[37;40m{}", marker.to_string().repeat(46)));
        }
        input.push_str("\x1b[5;18H\x1b[37;40m");
        input.push_str(&"A".repeat(46));
        input.push_str("\x1b[6;18H\x1b[30;47m");
        input.push_str(&"B".repeat(46));

        for bytewise in [false, true] {
            let (mut terminal, _, screen) = test_terminal();
            if bytewise {
                for byte in input.as_bytes() {
                    terminal.process_data(std::slice::from_ref(byte)).await;
                }
            } else {
                terminal.process_data(input.as_bytes()).await;
            }

            let screen = screen.lock();
            for (row, marker) in (4..=17).zip('A'..='N') {
                assert_eq!(screen.char_at(Position::new(17, row)).ch, marker, "screen row {row}, bytewise={bytewise}");
            }
        }
    }

    #[test]
    fn auto_login_enter_uses_active_terminal_mapping() {
        assert_eq!(TerminalThread::auto_login_control_code(TerminalEmulation::ATAscii, b'\r'), vec![0x9B]);
        assert_eq!(TerminalThread::auto_login_control_code(TerminalEmulation::AtariST, b'\r'), vec![b'\r']);
        assert_eq!(TerminalThread::auto_login_control_code(TerminalEmulation::Ansi, b'\r'), vec![b'\r']);
    }

    #[test]
    fn auto_login_non_enter_controls_remain_raw() {
        assert_eq!(TerminalThread::auto_login_control_code(TerminalEmulation::ATAscii, 0x1B), vec![0x1B]);
    }

    #[test]
    fn cache_names_allow_door_namespaces_but_not_traversal() {
        // Audio doors store effects under "sfx/<id>".
        assert!(valid_cache_filename("sfx/12"));
        assert!(valid_cache_filename("syncdoom/music/e1m1.ogg"));
        assert!(valid_cache_filename("syncdoom_1.jxl"));

        assert!(!valid_cache_filename("../escape"));
        assert!(!valid_cache_filename("sfx/../../etc/passwd"));
        assert!(!valid_cache_filename("/etc/passwd"));
        assert!(!valid_cache_filename("sfx//12"));
        assert!(!valid_cache_filename(""));
    }

    #[tokio::test]
    async fn cached_media_store_creates_namespaced_directories() {
        let root = std::env::temp_dir().join(format!("icy_term_cache_ns_{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&root).await;
        let encoded = general_purpose::STANDARD.encode(b"RIFFsound");

        store_cached_media(&root, "sfx/12", &encoded).await.unwrap();
        assert_eq!(read_cached_media(&root, "sfx/12").await.unwrap(), b"RIFFsound");

        tokio::fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn synchronized_output_round_trips_over_the_wire() {
        let (mut terminal, sent, screen) = test_terminal();

        terminal.process_data(b"\x1b[?2026h").await;
        assert!(screen.lock().terminal_state().synchronized_output());

        terminal.process_data(b"\x1b[?2026$p").await;
        assert_eq!(&*sent.lock(), b"\x1B[?2026;1$y");

        terminal.process_data(b"\x1b[?2026l").await;
        assert!(!screen.lock().terminal_state().synchronized_output());

        sent.lock().clear();
        terminal.process_data(b"\x1b[?2026$p").await;
        assert_eq!(&*sent.lock(), b"\x1B[?2026;2$y");
    }

    #[test]
    fn decrqm_reports_live_and_unknown_modes() {
        let mut screen = TextScreen::new(Size::new(80, 25));
        assert_eq!(ansi_mode_report_status(&screen, 4), 2);
        assert_eq!(ansi_mode_report_status(&screen, 20), 1);
        assert_eq!(dec_mode_report_status(&screen, 25), 1);
        assert_eq!(dec_mode_report_status(&screen, 80), 2);
        assert_eq!(dec_mode_report_status(&screen, 1070), 1);
        assert_eq!(dec_mode_report_status(&screen, 2004), 2);
        assert_eq!(dec_mode_report_status(&screen, 2026), 2);
        assert_eq!(dec_mode_report_status(&screen, 9999), 0);

        screen.caret_mut().insert_mode = true;
        screen.caret_mut().visible = false;
        screen.terminal_state_mut().bracketed_paste_mode = true;
        screen.terminal_state_mut().sixel_at_cursor = false;
        screen.terminal_state_mut().sixel_shared_palette = true;
        screen.terminal_state_mut().set_synchronized_output(true);
        screen.terminal_state_mut().lf_expand = false;
        assert_eq!(ansi_mode_report_status(&screen, 4), 1);
        assert_eq!(ansi_mode_report_status(&screen, 20), 2);
        assert_eq!(dec_mode_report_status(&screen, 25), 2);
        assert_eq!(dec_mode_report_status(&screen, 80), 1);
        assert_eq!(dec_mode_report_status(&screen, 1070), 2);
        assert_eq!(dec_mode_report_status(&screen, 2004), 1);
        assert_eq!(dec_mode_report_status(&screen, 2026), 1);
    }

    #[test]
    fn decrqss_reports_cursor_sgr_and_speed() {
        let mut screen = TextScreen::new(Size::new(80, 25));
        screen.caret_mut().shape = icy_parser_core::CaretShape::Bar;
        screen.caret_mut().blinking = false;
        screen.caret_mut().attribute.set_is_bold(true);
        screen.caret_mut().attribute.set_foreground_rgb(1, 2, 3);
        screen.terminal_state_mut().set_baud_rate(icy_parser_core::BaudEmulation::Rate(9600));

        assert_eq!(decrqss_status(&screen, b" q").as_deref(), Some("6 q"));
        assert_eq!(decrqss_status(&screen, b"m").as_deref(), Some("0;1;38;2;1;2;3;40m"));
        assert_eq!(decrqss_status(&screen, b"*r").as_deref(), Some("0;6*r"));
    }

    #[test]
    fn parses_syncdoom_cached_jxl_commands_safely() {
        assert!(matches!(
            parse_cached_media_command(b"SyncTERM:C;S;syncdoom_1.jxl;YWJj"),
            Some(CachedMediaCommand::Store {
                filename: "syncdoom_1.jxl",
                encoded: "YWJj"
            })
        ));
        assert!(matches!(
            parse_cached_media_command(b"SyncTERM:C;DrawJXL;DX=3;DY=4;ZX=2;syncdoom_1.jxl"),
            Some(CachedMediaCommand::DrawJxl {
                filename: "syncdoom_1.jxl",
                options: "DX=3;DY=4;ZX=2"
            })
        ));
        assert!(parse_cached_media_command(b"SyncTERM:C;S;../escape.jxl;YWJj").is_none());
        assert!(parse_cached_media_command(b"SyncTERM:C;DrawJXL;/tmp/escape.jxl").is_none());
    }

    #[test]
    fn parses_the_client_pixel_buffer_commands() {
        assert!(matches!(
            parse_cached_media_command(b"SyncTERM:C;LoadJXLBlob;B=1;YWJj"),
            Some(CachedMediaCommand::LoadBlob { buffer: 1, encoded: "YWJj" })
        ));
        assert!(matches!(
            parse_cached_media_command(b"SyncTERM:P;Paste;B=0;SX=2;SY=3;DX=20;DY=30"),
            Some(CachedMediaCommand::PasteBlob {
                buffer: 0,
                options: "B=0;SX=2;SY=3;DX=20;DY=30"
            })
        ));
        assert!(parse_cached_media_command(b"SyncTERM:C;LoadJXLBlob;B=2;YWJj").is_none());
        assert!(parse_cached_media_command(b"SyncTERM:P;Paste;B=9").is_none());
    }

    #[test]
    fn the_cache_listing_names_what_the_caller_holds() {
        let root = std::env::temp_dir().join(format!("icy_term_listing_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("snd")).unwrap();
        std::fs::create_dir_all(root.join("gfx")).unwrap();
        std::fs::write(root.join("snd/tone.wav"), b"tone").unwrap();
        std::fs::write(root.join("gfx/frame.jxl"), b"frame").unwrap();

        let everything = cache_listing(&root, "*");
        assert!(everything.contains(&format!("snd/tone.wav\t{:x}\n", md5::compute("tone"))), "{everything:?}");
        assert!(everything.contains("gfx/frame.jxl\t"), "{everything:?}");

        let sound_only = cache_listing(&root, "snd/*");
        assert!(sound_only.contains("snd/tone.wav\t"), "{sound_only:?}");
        assert!(!sound_only.contains("gfx/"), "{sound_only:?}");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn cached_jxl_store_and_draw_transport_roundtrip() {
        let root = std::env::temp_dir().join(format!("icy_term_cached_jxl_{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&root).await;
        let bytes = b"test-jxl-payload";
        let encoded = general_purpose::STANDARD.encode(bytes);

        store_cached_media(&root, "syncdoom.jxl", &encoded).await.unwrap();
        assert_eq!(tokio::fs::read(root.join("syncdoom.jxl")).await.unwrap(), bytes);

        let blob = read_cached_media(&root, "syncdoom.jxl").await.unwrap();
        assert_eq!(blob, bytes);
        assert!(read_cached_media(&root, "missing.jxl").await.is_none());

        tokio::fs::remove_dir_all(root).await.unwrap();
    }
}
