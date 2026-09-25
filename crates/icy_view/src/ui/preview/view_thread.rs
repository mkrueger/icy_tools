use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use icy_engine::{formats::FileFormat, limits, EditableScreen, LoadData, Screen, ScreenMode, ScreenSink, Size, TextBuffer, TextPane, TextScreen};
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
    LoadDataTagged(u64, PathBuf, Vec<u8>, bool),
    /// Stop current loading/playback
    Stop,
    /// Set baud emulation rate
    SetBaudEmulation(BaudEmulation),
    /// Pause or resume streaming playback
    SetPaused(bool),
    /// Jump to a byte position of the streamed file; earlier positions replay from the start
    Seek(usize),
    /// Shutdown the thread
    Shutdown,
}

/// Events sent from the view thread to the UI
#[derive(Clone)]
pub enum ViewEvent {
    ForRequest(u64, Box<ViewEvent>),
    /// File loading started
    LoadingStarted(PathBuf),
    /// File loading completed
    LoadingCompleted,
    LoadFailed(String),
    /// Sauce information extracted from file, with content size (file size without SAUCE)
    SauceInfo(Option<SauceRecord>, usize),
    /// Set scroll mode (determined by background thread)
    SetScrollMode(ScrollMode),
    /// Streaming playback progress (position, length) in bytes and typing cursor in source pixels
    Progress(usize, usize, i32),
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
    /// Streaming is suspended by the user
    paused: bool,
    /// Repaired SAUCE applied to the screen before parsing
    sauce: Option<SauceRecord>,
    /// Keep the screen height when applying SAUCE
    preserve_height: bool,
    /// Data had a UTF-8 BOM
    is_unicode: bool,
    /// Fixed document height from the complete stream, in text rows
    document_height: Option<i32>,
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
            paused: false,
            sauce: None,
            preserve_height: false,
            is_unicode: false,
            document_height: None,
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

/// Size a text stream from the complete input, without painting into the live screen.
/// Parser commands (rather than newline counts) account for wrapping and cursor moves.
fn text_document_height(mode: ScreenMode, emulation: TerminalEmulation, sauce: Option<&SauceRecord>, unicode: bool, data: &[u8]) -> Option<i32> {
    if !matches!(mode, ScreenMode::Vga(..) | ScreenMode::Unicode(..)) {
        return None;
    }
    let (mut screen, mut parser) = mode.create_screen(emulation, None);
    screen.terminal_state_mut().is_terminal_buffer = false;
    if let Some(sauce) = sauce {
        let height = screen.height();
        screen.apply_sauce(sauce);
        screen.set_height(height);
    }
    if unicode {
        *screen.buffer_type_mut() = icy_engine::BufferType::Unicode;
    }
    let mut commands = VecDeque::new();
    for chunk in data.chunks(4096) {
        parser.parse(chunk, &mut QueueingSink::new(&mut commands));
        let mut sink = ScreenSink::new(screen.as_mut());
        while let Some(command) = commands.pop_front() {
            if !command.needs_async_processing() {
                command.process_screen_command(&mut sink);
            }
        }
        if screen.height() >= limits::MAX_BUFFER_HEIGHT || screen.width() >= limits::MAX_BUFFER_WIDTH {
            break;
        }
    }
    Some(screen.height().max(screen.caret_position().y + 1).min(limits::MAX_BUFFER_HEIGHT))
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
    event_tx: ViewEvents,
    /// Sound thread for audio playback
    sound_thread: SoundThread,
    /// Load generation counter - incremented for each new load
    /// Background threads compare their generation to this to know if their result is still wanted
    load_generation: Arc<AtomicU64>,
    /// Pending format load task (will be ignored if load_generation changed)
    pending_format_load: Option<tokio::task::JoinHandle<Result<FormatLoadResult, String>>>,
    /// Auto-scroll enabled setting from UI (preserved across loads)
    auto_scroll_enabled: bool,
    /// Current load operation - None if no file is loaded
    /// Replacing this automatically cancels the previous operation via Drop
    current_load: Option<LoadOperation>,
    /// Last time a progress event was sent
    last_progress: Instant,
}

struct ViewEvents {
    sender: mpsc::UnboundedSender<ViewEvent>,
    request: Option<u64>,
}

impl ViewEvents {
    fn send(&self, event: ViewEvent) -> Result<(), mpsc::error::SendError<ViewEvent>> {
        self.sender.send(match self.request {
            Some(request) => ViewEvent::ForRequest(request, Box::new(event)),
            None => event,
        })
    }
}

impl ViewThread {
    pub fn spawn(screen: Arc<Mutex<Box<dyn Screen>>>) -> (mpsc::UnboundedSender<ViewCommand>, mpsc::UnboundedReceiver<ViewEvent>) {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let mut thread = Self {
            screen,
            baud_emulator: BaudEmulator::new(),
            event_tx: ViewEvents {
                sender: event_tx,
                request: None,
            },
            sound_thread: SoundThread::new(),
            load_generation: Arc::new(AtomicU64::new(0)),
            pending_format_load: None,
            auto_scroll_enabled: false,
            current_load: None,
            last_progress: Instant::now(),
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

    /// Replace the shared screen with a clean text screen while a new file is loading.
    /// This prevents pixel-oriented modes such as RIP from remaining visible behind the
    /// next text/ANSI/XBin document while background loading is still in progress.
    fn clear_screen_for_new_load(&mut self) {
        let mut new_screen = TextScreen::new(Size::new(80, 25));
        new_screen.terminal_state_mut().is_terminal_buffer = false;

        let mut screen = self.screen.lock();
        *screen = Box::new(new_screen);
    }

    /// Check if current load is playing
    fn is_playing(&self) -> bool {
        self.current_load.as_ref().is_some_and(|l| l.is_playing && !l.paused)
    }

    /// Report the streaming position, at most ~30 times per second unless forced.
    fn send_progress(&mut self, force: bool) {
        let Some(load) = &self.current_load else {
            return;
        };
        if load.parser.is_none() || (!force && self.last_progress.elapsed().as_millis() < 33) {
            return;
        }
        self.last_progress = Instant::now();
        let cursor_px = {
            let screen = self.screen.lock();
            (screen.caret_position().y + 1).max(0) * screen.font_dimensions().height
        };
        let _ = self.event_tx.send(ViewEvent::Progress(load.file_position, load.file_data.len(), cursor_px));
    }

    fn finish_playback(&mut self) {
        let Some(load) = &mut self.current_load else {
            return;
        };
        load.is_playing = false;
        let auto_scroll = load.auto_scroll_enabled;
        let _ = self.event_tx.send(ViewEvent::LoadingCompleted);
        let scroll_mode = if auto_scroll { ScrollMode::AutoScroll } else { ScrollMode::Off };
        let _ = self.event_tx.send(ViewEvent::SetScrollMode(scroll_mode));
    }

    /// Jump to `position`, parsing everything before it at once (without sounds or delays).
    async fn seek(&mut self, position: usize) {
        let Some(load) = &self.current_load else {
            return;
        };
        if load.parser.is_none() {
            return;
        }
        let position = position.min(load.file_data.len());
        if position < load.file_position {
            let (mode, emulation, sauce, preserve_height, unicode, file_data, mut document_height) = (
                load.screen_mode,
                load.terminal_emulation,
                load.sauce.clone(),
                load.preserve_height,
                load.is_unicode,
                load.file_data.clone(),
                load.document_height,
            );
            if document_height.is_none() && !matches!(self.baud_emulator.baud_emulation, BaudEmulation::Off) {
                document_height = text_document_height(mode, emulation, sauce.as_ref(), unicode, &file_data);
            }
            self.sound_thread.clear();
            let parser = self.init_parser_screen(mode, emulation, sauce.as_ref(), preserve_height, unicode, document_height);
            let load = self.current_load.as_mut().unwrap();
            load.parser = Some(parser);
            load.document_height = document_height;
            load.command_queue.clear();
            load.file_position = 0;
        }
        let load = self.current_load.as_mut().unwrap();
        let was_finished = !load.is_playing;
        load.is_playing = true;
        if position > load.file_position {
            let LoadOperation {
                parser,
                command_queue,
                file_data,
                file_position,
                ..
            } = load;
            if let Some(parser) = parser {
                let mut sink = QueueingSink::new(command_queue);
                parser.parse(&file_data[*file_position..position], &mut sink);
            }
            *file_position = position;
            self.process_command_queue(true).await;
        }
        let Some(load) = &self.current_load else {
            return;
        };
        if load.file_position >= load.file_data.len() || !load.is_playing {
            self.finish_playback();
        } else if was_finished {
            let _ = self
                .event_tx
                .send(ViewEvent::SetScrollMode(if matches!(self.baud_emulator.baud_emulation, BaudEmulation::Off) {
                    ScrollMode::Off
                } else {
                    ScrollMode::ClampToBottom
                }));
        }
        self.baud_emulator.reset();
        self.send_progress(true);
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
                        match result {
                            Ok(Ok(load_result)) => {
                                let current_gen = self.load_generation.load(Ordering::SeqCst);
                                if load_result.generation == current_gen { self.apply_format_load_result(load_result); }
                            }
                            Ok(Err(error)) => { let _ = self.event_tx.send(ViewEvent::LoadFailed(error)); }
                            Err(error) => { let _ = self.event_tx.send(ViewEvent::LoadFailed(error.to_string())); }
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
                self.event_tx.request = None;
                self.auto_scroll_enabled = auto_scroll;
                self.load_data(path, data).await;
            }
            ViewCommand::LoadDataTagged(request, path, data, auto_scroll) => {
                self.event_tx.request = Some(request);
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
            ViewCommand::SetPaused(paused) => {
                if let Some(load) = &mut self.current_load {
                    load.paused = paused;
                }
                // Don't let the paused time count as transmitted bytes.
                self.baud_emulator.reset();
                self.send_progress(true);
            }
            ViewCommand::Seek(position) => self.seek(position).await,
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

    /// Create the screen and parser for streaming, with SAUCE and the Unicode buffer type applied.
    fn init_parser_screen(
        &mut self,
        mode: ScreenMode,
        emulation: TerminalEmulation,
        sauce: Option<&SauceRecord>,
        preserve_height: bool,
        unicode: bool,
        document_height: Option<i32>,
    ) -> Box<dyn CommandParser + Send> {
        let parser = self.init_screen_for_mode(mode, emulation);
        let mut screen = self.screen.lock();
        if let Some(editable) = screen.as_editable() {
            if let Some(sauce) = sauce {
                let height = editable.height();
                editable.apply_sauce(sauce);
                if preserve_height {
                    // preserve height otherwise the "baud rate" emulation may not work correctly
                    editable.set_height(height);
                }
            }
            if unicode {
                *editable.buffer_type_mut() = icy_engine::BufferType::Unicode;
            }
            if let Some(height) = document_height {
                let window_height = editable.terminal_state().height();
                editable.set_height(height);
                editable.terminal_state_mut().set_height(window_height);
            }
        }
        parser
    }

    /// Replace the current screen with a text screen built from a loaded buffer.
    ///
    /// Format loaders can run after a RIP/graphics preview. Reusing the existing
    /// `PaletteScreenBuffer` would leave its pixel layer intact behind the text buffer,
    /// so replace the whole screen instead of copying into the current one.
    fn replace_screen_with_buffer(&mut self, buffer: TextBuffer) {
        let mut new_screen = TextScreen::from_buffer(buffer);
        new_screen.terminal_state_mut().is_terminal_buffer = false;

        let mut screen = self.screen.lock();
        *screen = Box::new(new_screen);
    }

    /// Apply the result of a background format load
    fn apply_format_load_result(&mut self, result: FormatLoadResult) {
        // Replace the screen with the loaded buffer.
        self.replace_screen_with_buffer(result.buffer);

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

        // Clear the visible screen immediately. In particular, RIP graphics are pixel-based
        // and can otherwise remain visible while the next file is loaded asynchronously.
        self.clear_screen_for_new_load();

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

                // Prepare data: strip BOM and crop at EOF marker
                let (file_data, is_unicode) = prepare_parser_data(stripped_data, &ext);
                let document_height = if matches!(self.baud_emulator.baud_emulation, BaudEmulation::Off) {
                    None
                } else {
                    text_document_height(mode, emulation, repaired_sauce_opt.as_ref(), is_unicode, &file_data)
                };
                // Initialize screen for this mode - replaces the entire screen
                let parser = self.init_parser_screen(mode, emulation, repaired_sauce_opt.as_ref(), true, is_unicode, document_height);
                self.start_streaming(parser, mode, emulation, file_data, repaired_sauce_opt, true, is_unicode, document_height);

                // Send scroll mode: ClampToBottom if baud emulation active, otherwise Off (will switch to AutoScroll on complete)
                if !matches!(self.baud_emulator.baud_emulation, BaudEmulation::Off) {
                    let _ = self.event_tx.send(ViewEvent::SetScrollMode(ScrollMode::ClampToBottom));
                } else {
                    let _ = self.event_tx.send(ViewEvent::SetScrollMode(ScrollMode::Off));
                }
                return;
            }
        }

        if let Some(format) = file_format.filter(|format| crate::format_preview::is_previewable(*format)) {
            if let Some(load) = &mut self.current_load {
                load.load_mode = LoadMode::Format(format);
            }
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            let path_clone = path.clone();
            self.pending_format_load = Some(tokio::task::spawn_blocking(move || {
                crate::format_preview::render(format, &name, &stripped_data)
                    .map(|buffer| FormatLoadResult {
                        buffer,
                        path: path_clone,
                        stripped_data,
                        generation,
                    })
                    .map_err(|error| error.to_string())
            }));
            return;
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
                let load_data = LoadData::new(None, None);
                match format.from_bytes(&stripped_data_clone, Some(load_data)) {
                    Ok(loaded_doc) => {
                        let buffer = loaded_doc.screen.buffer;
                        // Validate buffer before returning
                        if buffer.width() == 0 || buffer.height() == 0 {
                            Err(format!("Invalid buffer dimensions: {}x{}", buffer.width(), buffer.height()))
                        } else {
                            Ok(FormatLoadResult {
                                buffer,
                                path: path_clone,
                                stripped_data: stripped_data_clone,
                                generation,
                            })
                        }
                    }
                    Err(e) => {
                        log::error!("Format loading failed: {}", e);
                        Err(e.to_string())
                    }
                }
            });

            self.pending_format_load = Some(handle);
            return;
        }

        // Unknown format - try parser anyway with default ANSI emulation
        let mode = ScreenMode::Vga(80, 25);
        let emulation = TerminalEmulation::Ansi;
        let (file_data, is_unicode) = prepare_parser_data(stripped_data, &ext);
        let document_height = if matches!(self.baud_emulator.baud_emulation, BaudEmulation::Off) {
            None
        } else {
            text_document_height(mode, emulation, repaired_sauce_opt.as_ref(), is_unicode, &file_data)
        };
        let parser = self.init_parser_screen(mode, emulation, repaired_sauce_opt.as_ref(), false, is_unicode, document_height);
        self.start_streaming(parser, mode, emulation, file_data, repaired_sauce_opt, false, is_unicode, document_height);
    }

    #[allow(clippy::too_many_arguments)]
    fn start_streaming(
        &mut self,
        parser: Box<dyn CommandParser + Send>,
        mode: ScreenMode,
        emulation: TerminalEmulation,
        file_data: Vec<u8>,
        sauce: Option<SauceRecord>,
        preserve_height: bool,
        is_unicode: bool,
        document_height: Option<i32>,
    ) {
        if let Some(load) = &mut self.current_load {
            load.parser = Some(parser);
            load.screen_mode = mode;
            load.terminal_emulation = emulation;
            load.file_data = file_data;
            load.load_mode = LoadMode::Parser;
            load.file_position = 0;
            load.is_playing = true;
            load.sauce = sauce;
            load.preserve_height = preserve_height;
            load.is_unicode = is_unicode;
            load.document_height = document_height;
        }
        self.send_progress(true);
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
                tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
                return; // Not enough time has passed, wait for next tick
            }

            let chunk = load.file_data[load.file_position..load.file_position + bytes_to_send].to_vec();
            load.file_position += bytes_to_send;
            chunk
        } else {
            self.finish_playback();
            return;
        };
        let at_end = load.file_position >= load.file_data.len();

        if matches!(self.baud_emulator.baud_emulation, BaudEmulation::Off) {
            self.process_data(&chunk).await;
        } else {
            for byte in chunk {
                self.process_data(std::slice::from_ref(&byte)).await;
            }
        }
        self.send_progress(at_end);
    }

    async fn process_data(&mut self, data: &[u8]) {
        let Some(load) = &mut self.current_load else {
            return;
        };

        if let Some(parser) = &mut load.parser {
            let mut sink = QueueingSink::new(&mut load.command_queue);
            parser.parse(data, &mut sink);
        }

        self.process_command_queue(false).await;
    }

    /// Run the queued commands; `fast_forward` skips sounds and delays (used when seeking).
    async fn process_command_queue(&mut self, fast_forward: bool) {
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

            if fast_forward && (cmd.is_sound() || cmd.is_delay()) {
                continue;
            }

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
