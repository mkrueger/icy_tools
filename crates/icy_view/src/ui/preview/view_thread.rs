use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use icy_engine::{LoadData, Screen, ScreenMode, ScreenSink, Size, TextBuffer, TextPane, formats::FileFormat, limits};
use icy_engine_gui::music::SoundThread;
use icy_engine_gui::util::{BaudEmulator, QueuedCommand, QueueingSink};
use icy_net::telnet::TerminalEmulation;
use icy_parser_core::*;
use icy_sauce::{Capabilities, SauceRecord};
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// How to load the file
#[derive(Debug, Clone, PartialEq)]
enum LoadMode {
    /// Stream through a parser (supports baud emulation)
    Parser,
    /// Load via format (instant load, no streaming)
    Format(FileFormat),
}

/// Scroll mode determined by background thread
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollMode {
    /// No automatic scrolling
    Off,
    /// Animated auto-scroll (for viewing completed files)
    AutoScroll,
    /// Clamp to bottom during loading (terminal-like for baud emulation)
    ClampToBottom,
}

/// Commands sent to the view thread
#[derive(Debug, Clone)]
pub enum ViewCommand {
    /// Load data for viewing (path, data, auto_scroll_enabled)
    LoadData(PathBuf, Vec<u8>, bool),
    /// Stop current loading/playback
    Stop,
    /// Set baud emulation rate
    SetBaudEmulation(BaudEmulation),
    /// Shutdown the thread
    Shutdown,
}

/// Events sent from the view thread to the UI
#[derive(Clone)]
pub enum ViewEvent {
    /// File loading started
    LoadingStarted(PathBuf),
    /// File loading completed
    LoadingCompleted,
    /// Sauce information extracted from file, with content size (file size without SAUCE)
    SauceInfo(Option<SauceRecord>, usize),
    /// Set scroll mode (determined by background thread)
    SetScrollMode(ScrollMode),
}

/// Result from background format loading
struct FormatLoadResult {
    /// The loaded buffer
    buffer: TextBuffer,
    /// Path of the loaded file
    path: PathBuf,
    /// Stripped file data (without SAUCE)
    stripped_data: Vec<u8>,
    /// The generation this load was started with
    generation: u64,
}

/// Active load operation - contains all state for a single file load
/// When a new file is loaded, this entire struct is replaced, automatically
/// cancelling any ongoing operations via the CancellationToken's Drop impl
struct LoadOperation {
    /// Cancellation token - cancelled when this operation is dropped
    cancel_token: CancellationToken,
    /// Current file path
    path: PathBuf,
    /// Current file data being processed
    file_data: Vec<u8>,
    /// Current position in file data
    file_position: usize,
    /// Whether we're currently playing back a file
    is_playing: bool,
    /// Current load mode
    load_mode: LoadMode,
    /// Current screen mode
    screen_mode: ScreenMode,
    /// Current terminal emulation
    terminal_emulation: TerminalEmulation,
    /// Parser for ANSI/etc content (None for format-based loading)
    parser: Option<Box<dyn CommandParser + Send>>,
    /// Command queue for granular locking
    command_queue: VecDeque<QueuedCommand>,
    /// Auto-scroll enabled setting
    auto_scroll_enabled: bool,
}

impl LoadOperation {
    fn new(path: PathBuf, auto_scroll_enabled: bool) -> Self {
        Self {
            cancel_token: CancellationToken::new(),
            path,
            file_data: Vec::new(),
            file_position: 0,
            is_playing: false,
            load_mode: LoadMode::Parser,
            screen_mode: ScreenMode::Vga(80, 25),
            terminal_emulation: TerminalEmulation::Ansi,
            parser: None,
            command_queue: VecDeque::new(),
            auto_scroll_enabled,
        }
    }

    /// Check if this operation has been cancelled
    fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }

    /// Get a clone of the cancel token for async operations
    /// This ensures async operations hold a reference to THIS operation's token
    fn get_cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }
}

impl Drop for LoadOperation {
    fn drop(&mut self) {
        // Cancel all pending async operations when this load operation is dropped
        self.cancel_token.cancel();
    }
}

/// Prepare file data for parser-based loading
/// - Strips UTF-8 BOM if present and returns whether the file is Unicode
/// - Crops data at 0x1A (SUB/EOF) for non-PETSCII formats
pub fn prepare_parser_data(data: Vec<u8>, ext: &str) -> (Vec<u8>, bool) {
    // Check for UTF-8 BOM (0xEF, 0xBB, 0xBF)
    let (data, is_unicode) = if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        (data[3..].to_vec(), true)
    } else {
        (data, false)
    };

    // Crop at 0x1A (SUB/EOF) for non-PETSCII formats
    // This is a legacy DOS EOF marker used in ANSI files
    let data = if !matches!(ext, "pet" | "seq") {
        if let Some(eof_pos) = data.iter().position(|&b| b == 0x1A) {
            data[..eof_pos].to_vec()
        } else {
            data
        }
    } else {
        data
    };

    (data, is_unicode)
}

/// Find a matching format for the file extension
fn find_format_for_extension(ext: &str) -> Option<FileFormat> {
    FileFormat::from_extension(ext)
}

/// Repair invalid SAUCE data
/// Some files have 0 width/height which causes issues during loading
/// This creates a repaired copy for internal use
fn repair_sauce_data(sauce: &SauceRecord) -> SauceRecord {
    if let Some(Capabilities::Character(mut char_caps)) = sauce.capabilities() {
        if char_caps.columns > 0 && char_caps.lines > 0 {
            return sauce.clone();
        }
        char_caps.columns = 80;
        char_caps.lines = char_caps.lines.min(25);
        let mut builder = sauce.to_builder();
        builder = builder.capabilities(Capabilities::Character(char_caps)).unwrap();
        return builder.build();
    }
    if let Some(Capabilities::Binary(mut bin_caps)) = sauce.capabilities() {
        if bin_caps.columns > 0 {
            return sauce.clone();
        }
        bin_caps.columns = 80;
        let mut builder = sauce.to_builder();
        builder = builder.capabilities(Capabilities::Binary(bin_caps)).unwrap();
        return builder.build();
    }

    sauce.clone()
}

/// Cancellable sleep - returns true if cancelled, false if completed normally
async fn cancellable_sleep(duration: tokio::time::Duration, cancel_token: &CancellationToken) -> bool {
    tokio::select! {

        _ = cancel_token.cancelled() => true,
        _ = tokio::time::sleep(duration) => false,
    }
}

/// View thread for file loading and parsing
pub struct ViewThread {
    /// Shared screen state with UI
    screen: Arc<Mutex<Box<dyn Screen>>>,
    /// Baud rate emulator
    baud_emulator: BaudEmulator,
    /// Event sender
    event_tx: mpsc::UnboundedSender<ViewEvent>,
    /// Sound thread for audio playback
    sound_thread: SoundThread,
    /// Load generation counter - incremented for each new load
    /// Background threads compare their generation to this to know if their result is still wanted
    load_generation: Arc<AtomicU64>,
    /// Pending format load task (will be ignored if load_generation changed)
    pending_format_load: Option<tokio::task::JoinHandle<Option<FormatLoadResult>>>,
    /// Auto-scroll enabled setting from UI (preserved across loads)
    auto_scroll_enabled: bool,
    /// Current load operation - None if no file is loaded
    /// Replacing this automatically cancels the previous operation via Drop
    current_load: Option<LoadOperation>,
}

impl ViewThread {
    pub fn spawn(screen: Arc<Mutex<Box<dyn Screen>>>) -> (mpsc::UnboundedSender<ViewCommand>, mpsc::UnboundedReceiver<ViewEvent>) {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let mut thread = Self {
            screen,
            baud_emulator: BaudEmulator::new(),
            event_tx,
            sound_thread: SoundThread::new(),
            load_generation: Arc::new(AtomicU64::new(0)),
            pending_format_load: None,
            auto_scroll_enabled: false,
            current_load: None,
        };

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime");

            runtime.block_on(async move {
                thread.run(command_rx).await;
            });
        });

        (command_tx, event_rx)
    }

    /// Start a new load operation, cancelling any existing one
    /// The old LoadOperation is dropped, which cancels its token
    fn start_new_load(&mut self, path: PathBuf) {
        // Stop sounds
        self.sound_thread.clear();
        // Reset baud emulator
        self.baud_emulator.reset();
        // Drop pending format load
        self.pending_format_load = None;
        // Replace current load - the old one's Drop will cancel its token
        self.current_load = Some(LoadOperation::new(path, self.auto_scroll_enabled));
    }

    /// Check if current load is playing
    fn is_playing(&self) -> bool {
        self.current_load.as_ref().map_or(false, |l| l.is_playing)
    }

    async fn run(&mut self, mut command_rx: mpsc::UnboundedReceiver<ViewCommand>) {
        loop {
            // Update sound thread state
            let _ = self.sound_thread.update_state();

            if self.pending_format_load.is_some() {
                // Polling pending format load while listening for commands
                tokio::select! {
                    biased;

                    Some(cmd) = command_rx.recv() => {
                        if !self.handle_command(cmd).await {
                            break;
                        }
                    }

                    result = async { self.pending_format_load.as_mut().unwrap().await } => {
                        self.pending_format_load = None;
                        if let Ok(Some(load_result)) = result {
                            let current_gen = self.load_generation.load(Ordering::SeqCst);
                            if load_result.generation == current_gen {
                                self.apply_format_load_result(load_result);
                            }
                        }
                    }
                }
            } else if self.is_playing() {
                // Playing: process chunks while listening for commands
                tokio::select! {
                    biased;

                    Some(cmd) = command_rx.recv() => {
                        if !self.handle_command(cmd).await {
                            break;
                        }
                    }

                    _ = self.process_next_chunk() => {}
                }
            } else {
                // Idle: just wait for commands
                if let Some(cmd) = command_rx.recv().await {
                    if !self.handle_command(cmd).await {
                        break;
                    }
                } else {
                    break; // Channel closed
                }
            }
        }
    }

    async fn handle_command(&mut self, command: ViewCommand) -> bool {
        match command {
            ViewCommand::LoadData(path, data, auto_scroll) => {
                self.auto_scroll_enabled = auto_scroll;
                self.load_data(path, data).await;
            }
            ViewCommand::Stop => {
                self.stop();
            }
            ViewCommand::SetBaudEmulation(baud) => {
                self.baud_emulator.set_baud_rate(baud);
                // If we're playing with baud emulation, switch to clamp mode
                if self.is_playing() && !matches!(baud, BaudEmulation::Off) {
                    let _ = self.event_tx.send(ViewEvent::SetScrollMode(ScrollMode::ClampToBottom));
                }
            }
            ViewCommand::Shutdown => {
                return false;
            }
        }
        true
    }

    /// Initialize the screen for the given screen mode using create_screen
    /// This replaces the entire screen like icy_term does
    /// Returns the parser for storing in LoadOperation
    fn init_screen_for_mode(&mut self, mode: ScreenMode, emulation: TerminalEmulation) -> Box<dyn CommandParser + Send> {
        let (mut new_screen, parser) = mode.create_screen(emulation, None);
        {
            new_screen.terminal_state_mut().is_terminal_buffer = false;
            let mut screen = self.screen.lock();
            *screen = new_screen;
        }
        parser
    }

    /// Copy a TextBuffer to the screen
    fn copy_buffer_to_screen(&mut self, buffer: &TextBuffer) {
        let mut screen = self.screen.lock();
        if let Some(editable) = screen.as_editable() {
            // Validate buffer dimensions to prevent overflow
            let width = buffer.width();
            let height = buffer.height();

            if width == 0 || height == 0 {
                log::warn!("Invalid buffer dimensions: {}x{}, skipping copy", width, height);
                return;
            }

            // Clamp to limits to prevent huge allocations
            let width = width.min(limits::MAX_BUFFER_WIDTH);
            let height = height.min(limits::MAX_BUFFER_HEIGHT);

            // Set the size to match the buffer (clamped)
            let size = Size::new(width, height);
            editable.set_size(size);
            editable.terminal_state_mut().set_width(width);

            // Also update layer sizes to match
            for layer_idx in 0..editable.layer_count() {
                if let Some(layer) = editable.get_layer_mut(layer_idx) {
                    layer.set_size(size);
                }
            }

            // Copy fonts
            editable.clear_font_table();
            for (i, font) in buffer.font_iter() {
                editable.set_font(*i as usize, font.clone());
            }

            // Copy palette
            *editable.palette_mut() = buffer.palette.clone();

            // Copy buffer type
            *editable.buffer_type_mut() = buffer.buffer_type;

            // Copy all characters from layer 0
            if !buffer.layers.is_empty() {
                let layer = &buffer.layers[0];
                for y in 0..buffer.height() {
                    for x in 0..buffer.width() {
                        let ch: icy_engine::AttributedChar = layer.char_at((x, y).into());
                        editable.set_char((x, y).into(), ch);
                    }
                }
            }

            editable.caret_default_colors();
        }
    }

    /// Apply the result of a background format load
    fn apply_format_load_result(&mut self, result: FormatLoadResult) {
        // Copy the loaded buffer to the screen
        self.copy_buffer_to_screen(&result.buffer);

        // Update the current load with the result data
        if let Some(load) = &mut self.current_load {
            load.path = result.path;
            load.file_data = result.stripped_data;
        }

        let _ = self.event_tx.send(ViewEvent::LoadingCompleted);
        // Send scroll mode: AutoScroll if enabled, otherwise Off
        let scroll_mode = if self.auto_scroll_enabled { ScrollMode::AutoScroll } else { ScrollMode::Off };
        let _ = self.event_tx.send(ViewEvent::SetScrollMode(scroll_mode));
    }

    async fn load_data(&mut self, path: PathBuf, data: Vec<u8>) {
        // Start a new load operation (cancels any existing one via Drop)
        self.start_new_load(path.clone());

        // Increment load generation to invalidate any pending background loads
        let generation = self.load_generation.fetch_add(1, Ordering::SeqCst) + 1;

        // Send loading started event
        let _ = self.event_tx.send(ViewEvent::LoadingStarted(path.clone()));

        // Extract sauce information and strip it from data
        let sauce_opt = SauceRecord::from_bytes(&data).ok().flatten();
        let stripped_data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All).to_vec();
        let content_size = stripped_data.len();

        // Send original sauce info to main thread (for display) with content size
        let _ = self.event_tx.send(ViewEvent::SauceInfo(sauce_opt.clone(), content_size));

        // Create repaired sauce for internal use (fixes 0 width/height issues)
        let repaired_sauce_opt = sauce_opt.as_ref().map(repair_sauce_data);

        // Get file extension
        let ext = path.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()).unwrap_or_default();

        // Try to detect file format
        let file_format = FileFormat::from_extension(&ext);

        // Try parser-based loading first (streaming with baud emulation support)
        // Parser has priority for formats it supports natively
        if let Some(format) = file_format {
            if format.uses_parser() {
                let mode = format.screen_mode();
                let emulation = format.terminal_emulation().unwrap_or(TerminalEmulation::Ansi);

                // Initialize screen for this mode - replaces the entire screen
                let parser = self.init_screen_for_mode(mode, emulation);

                // Apply SAUCE width if available (using repaired sauce data)
                // This must be done after init_screen_for_mode but before parsing
                if let Some(sauce) = &repaired_sauce_opt {
                    let mut screen = self.screen.lock();
                    if let Some(editable) = screen.as_editable() {
                        let height = editable.height();
                        editable.apply_sauce(sauce);
                        // preserve height otherwise the "baud rate"
                        // emulation may not work correctly
                        editable.set_height(height);
                    }
                }

                // Prepare data: strip BOM and crop at EOF marker
                let (file_data, is_unicode) = prepare_parser_data(stripped_data, &ext);

                if is_unicode {
                    let mut screen = self.screen.lock();
                    if let Some(editable) = screen.as_editable() {
                        *editable.buffer_type_mut() = icy_engine::BufferType::Unicode;
                    }
                }

                // Update load operation
                if let Some(load) = &mut self.current_load {
                    load.parser = Some(parser);
                    load.screen_mode = mode;
                    load.terminal_emulation = emulation;
                    load.file_data = file_data;
                    load.load_mode = LoadMode::Parser;
                    load.file_position = 0;
                    load.is_playing = true;
                }

                // Send scroll mode: ClampToBottom if baud emulation active, otherwise Off (will switch to AutoScroll on complete)
                if !matches!(self.baud_emulator.baud_emulation, BaudEmulation::Off) {
                    let _ = self.event_tx.send(ViewEvent::SetScrollMode(ScrollMode::ClampToBottom));
                } else {
                    let _ = self.event_tx.send(ViewEvent::SetScrollMode(ScrollMode::Off));
                }
                return;
            }
        }

        // Fallback: format-based loading for formats without parser support
        // This runs in a background task so the UI stays responsive
        if let Some(format) = find_format_for_extension(&ext) {
            if let Some(load) = &mut self.current_load {
                load.load_mode = LoadMode::Format(format.clone());
            }

            // Spawn background task for format loading
            let path_clone = path.clone();
            let stripped_data_clone = stripped_data.clone();
            let handle = tokio::task::spawn_blocking(move || {
                let load_data = LoadData::new(repaired_sauce_opt, None, None);
                match format.from_bytes(&stripped_data_clone, Some(load_data)) {
                    Ok(screen) => {
                        let buffer = screen.buffer;
                        // Validate buffer before returning
                        if buffer.width() == 0 || buffer.height() == 0 {
                            log::error!("Format produced invalid buffer dimensions: {}x{}", buffer.width(), buffer.height());
                            None
                        } else {
                            Some(FormatLoadResult {
                                buffer,
                                path: path_clone,
                                stripped_data: stripped_data_clone,
                                generation,
                            })
                        }
                    }
                    Err(e) => {
                        log::error!("Format loading failed: {}", e);
                        None
                    }
                }
            });

            self.pending_format_load = Some(handle);
            return;
        }

        // Unknown format - try parser anyway with default ANSI emulation
        let mode = ScreenMode::Vga(80, 25);
        let emulation = TerminalEmulation::Ansi;
        let parser = self.init_screen_for_mode(mode, emulation);

        // Apply SAUCE width if available (using repaired sauce data)
        if let Some(sauce) = &repaired_sauce_opt {
            let mut screen = self.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.apply_sauce(sauce);
            }
        }

        if let Some(sauce) = &repaired_sauce_opt {
            let mut screen = self.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.apply_sauce(sauce);
            }
        }

        // Prepare data: strip BOM and crop at EOF marker
        let (file_data, is_unicode) = prepare_parser_data(stripped_data, &ext);

        if is_unicode {
            let mut screen = self.screen.lock();
            if let Some(editable) = screen.as_editable() {
                *editable.buffer_type_mut() = icy_engine::BufferType::Unicode;
            }
        }

        // Update load operation
        if let Some(load) = &mut self.current_load {
            load.parser = Some(parser);
            load.screen_mode = mode;
            load.terminal_emulation = emulation;
            load.file_data = file_data;
            load.load_mode = LoadMode::Parser;
            load.file_position = 0;
            load.is_playing = true;
        }
    }

    fn stop(&mut self) {
        // Drop the current load - this cancels any ongoing operations via Drop
        self.current_load = None;
        self.pending_format_load = None;
        self.sound_thread.clear();
        self.baud_emulator.reset();
    }

    async fn process_next_chunk(&mut self) {
        let Some(load) = &mut self.current_load else {
            return;
        };

        if !load.is_playing {
            return;
        }

        // Check if cancelled
        if load.is_cancelled() {
            load.is_playing = false;
            return;
        }

        // Only process for parser-based loading
        if load.parser.is_none() {
            load.is_playing = false;
            return;
        }

        let chunk_size = if self.baud_emulator.baud_emulation == BaudEmulation::Off {
            64 * 1024
        } else {
            1024
        };

        let chunk = if load.file_position < load.file_data.len() {
            let remaining = load.file_data.len() - load.file_position;
            let actual_chunk_size = remaining.min(chunk_size);

            // Apply baud emulation to determine how many bytes we can actually send
            let bytes_to_send = if self.baud_emulator.baud_emulation == BaudEmulation::Off {
                actual_chunk_size
            } else {
                self.baud_emulator.calculate_bytes_to_send(actual_chunk_size)
            };

            if bytes_to_send == 0 {
                return; // Not enough time has passed, wait for next tick
            }

            let chunk = load.file_data[load.file_position..load.file_position + bytes_to_send].to_vec();
            load.file_position += bytes_to_send;
            chunk
        } else {
            load.is_playing = false;
            let auto_scroll = load.auto_scroll_enabled;
            let _ = self.event_tx.send(ViewEvent::LoadingCompleted);
            // Send scroll mode: AutoScroll if enabled, otherwise Off
            let scroll_mode = if auto_scroll { ScrollMode::AutoScroll } else { ScrollMode::Off };
            let _ = self.event_tx.send(ViewEvent::SetScrollMode(scroll_mode));
            return;
        };

        if !chunk.is_empty() {
            self.process_data(&chunk).await;
        }
    }

    async fn process_data(&mut self, data: &[u8]) {
        let Some(load) = &mut self.current_load else {
            return;
        };

        if let Some(parser) = &mut load.parser {
            let mut sink = QueueingSink::new(&mut load.command_queue);
            parser.parse(data, &mut sink);
        }

        self.process_command_queue().await;
    }

    async fn process_command_queue(&mut self) {
        const MAX_LOCK_DURATION_MS: u64 = 10;

        loop {
            let Some(load) = &mut self.current_load else {
                return;
            };

            // Check if cancelled
            if load.is_cancelled() {
                load.command_queue.clear();
                return;
            }

            let Some(cmd) = load.command_queue.pop_front() else {
                break;
            };

            // Get cancel token BEFORE any async operations
            let cancel_token = load.get_cancel_token();

            // Handle async commands first (these yield to allow command handling)
            if self.try_handle_async_command(&cmd, &cancel_token).await {
                // After async command, yield to allow other tasks to run
                tokio::task::yield_now().await;
                continue;
            }

            // Handle screen commands with granular locking
            {
                let lock_start = Instant::now();
                let mut screen = self.screen.lock();

                if let Some(editable) = screen.as_editable() {
                    let mut screen_sink = ScreenSink::new(editable);

                    cmd.process_screen_command(&mut screen_sink);

                    while lock_start.elapsed().as_millis() < MAX_LOCK_DURATION_MS as u128 {
                        let Some(load) = &mut self.current_load else {
                            return;
                        };

                        if load.is_cancelled() {
                            load.command_queue.clear();
                            return;
                        }

                        if screen_sink.screen().height() >= limits::MAX_BUFFER_HEIGHT as i32 || screen_sink.screen().width() >= limits::MAX_BUFFER_WIDTH as i32
                        {
                            load.is_playing = false;
                            return;
                        }

                        match load.command_queue.front() {
                            None => break,
                            Some(c) if c.needs_async_processing() => break,
                            _ => {}
                        }

                        let next_cmd = load.command_queue.pop_front().unwrap();
                        next_cmd.process_screen_command(&mut screen_sink);
                    }
                }
            }
        }
    }

    /// Try to handle async commands (sound, delays, etc.)
    /// Returns true if the command was handled
    /// Takes a cloned cancel_token to ensure we're checking the right token
    async fn try_handle_async_command(&mut self, cmd: &QueuedCommand, cancel_token: &CancellationToken) -> bool {
        match cmd {
            QueuedCommand::Music(music) => {
                let _ = self.sound_thread.play_music(music.clone());
                true
            }
            QueuedCommand::Bell => {
                let _ = self.sound_thread.beep();
                true
            }
            // IGS sound effects
            QueuedCommand::Igs(IgsCommand::BellsAndWhistles { sound_effect }) => {
                let snd_num = (*sound_effect as usize).min(19);
                if let Some(sound) = icy_engine_gui::music::sound_effects::sound_data(snd_num) {
                    let _ = self.sound_thread.play_gist(sound.to_vec());
                }
                true
            }
            QueuedCommand::Igs(IgsCommand::StopAllSound) => {
                let _ = self.sound_thread.stop_snd_all();
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
                    if let Some(sound) = icy_engine_gui::music::sound_effects::sound_data(snd_num) {
                        let _ = self.sound_thread.play_chip_music(sound.to_vec(), *voice, *volume, *pitch);
                    }
                }
                if *timing > 0 {
                    let wait_ms = (*timing as u64 * 1000) / 200;
                    // Use cancellable sleep with the token from THIS load operation
                    if cancellable_sleep(tokio::time::Duration::from_millis(wait_ms), cancel_token).await {
                        // Cancelled - stop processing
                        return true;
                    }
                }
                match *stop_type {
                    StopType::SndOff => {
                        let _ = self.sound_thread.snd_off(*voice);
                    }
                    StopType::StopSnd => {
                        let _ = self.sound_thread.stop_snd(*voice);
                    }
                    StopType::SndOffAll => {
                        let _ = self.sound_thread.snd_off_all();
                    }
                    StopType::StopSndAll => {
                        let _ = self.sound_thread.stop_snd_all();
                    }
                    StopType::NoEffect => {}
                }
                true
            }
            // Delays - use cancellable sleep with the token from THIS load operation
            QueuedCommand::Igs(IgsCommand::Pause { pause_type }) => {
                if !pause_type.is_double_step_config() {
                    let delay_ms = pause_type.ms().min(10_000);
                    cancellable_sleep(tokio::time::Duration::from_millis(delay_ms), cancel_token).await;
                }
                true
            }
            QueuedCommand::Skypix(SkypixCommand::Delay { jiffies }) => {
                let delay_ms = 1000 * (*jiffies) as u64 / 60;
                cancellable_sleep(tokio::time::Duration::from_millis(delay_ms), cancel_token).await;
                true
            }
            // Ignored async commands (not applicable to viewer)
            QueuedCommand::Igs(IgsCommand::Noise { .. })
            | QueuedCommand::Igs(IgsCommand::LoadMidiBuffer { .. })
            | QueuedCommand::Igs(IgsCommand::SetEffectLoops { .. })
            | QueuedCommand::Igs(IgsCommand::AlterSoundEffect { .. })
            | QueuedCommand::Igs(IgsCommand::RestoreSoundEffect { .. })
            | QueuedCommand::Igs(IgsCommand::AskIG { .. })
            | QueuedCommand::Skypix(SkypixCommand::CrcTransfer { .. })
            | QueuedCommand::TerminalRequest(_)
            | QueuedCommand::ResizeTerminal(_, _) => true,
            // Not an async command
            _ => false,
        }
    }
}

/// Helper function to create a view thread
pub fn create_view_thread(screen: Arc<Mutex<Box<dyn Screen>>>) -> (mpsc::UnboundedSender<ViewCommand>, mpsc::UnboundedReceiver<ViewEvent>) {
    ViewThread::spawn(screen)
}
