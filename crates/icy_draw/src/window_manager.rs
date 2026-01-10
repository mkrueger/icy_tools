//! Window Manager for icy_draw
//!
//! Manages multiple independent windows, each with its own MainWindow state.
//! Implements VS Code-like "Hot Exit" for session persistence and crash recovery.

use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use parking_lot::RwLock;
use tokio::sync::mpsc as tokio_mpsc;

use icy_ui::{keyboard, menu, widget::space, window, Element, Event, Point, Size, Subscription, Task, Theme, Vector};

use crate::mcp::McpCommand;
use crate::session::{edit_mode_to_string, SessionManager, SessionState, WindowRestoreInfo, WindowState};
use crate::ui::{main_window::commands::create_draw_commands, MainWindow};
use crate::{load_window_icon, Settings, SharedFontLibrary};
use icy_engine_gui::command_handler;
use icy_engine_gui::commands::cmd;
use icy_engine_gui::{find_next_window_id, focus_window_by_id};

// Generate the WindowCommands struct with handle() method
command_handler!(WindowCommands, create_draw_commands(), _window_id: window::Id => WindowManagerMessage {
    cmd::WINDOW_NEW => WindowManagerMessage::OpenWindow,
    cmd::WINDOW_CLOSE => WindowManagerMessage::CloseWindow(_window_id),
    cmd::FILE_CLOSE => WindowManagerMessage::CloseWindow(_window_id),
    cmd::APP_QUIT => WindowManagerMessage::WindowMessage(_window_id, crate::ui::Message::QuitApp),
});

const DEFAULT_SIZE: Size = Size::new(1280.0, 800.0);

/// How often to save session (in seconds)
const SESSION_SAVE_INTERVAL_SECS: u64 = 10;

/// Debounce delay for session saves triggered by resize/move, etc.
const SESSION_SAVE_DEBOUNCE_MS: u64 = 300;

enum SaveRequest {
    Save(SessionState),
    Flush(SessionState, mpsc::Sender<()>),
}

/// Cached window geometry for session saving
#[derive(Clone, Debug)]
struct WindowGeometry {
    position: Option<(f32, f32)>,
    size: (f32, f32),
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self {
            position: None,
            size: (DEFAULT_SIZE.width, DEFAULT_SIZE.height),
        }
    }
}

pub struct WindowManager {
    windows: BTreeMap<window::Id, MainWindow>,
    /// Cached window geometry (position, size) for session saving
    window_geometry: BTreeMap<window::Id, WindowGeometry>,
    options: Arc<RwLock<Settings>>,
    /// Shared font library for TDF/Figlet fonts
    font_library: SharedFontLibrary,
    /// Pending windows to restore (for session restore)
    pending_restores: Vec<WindowRestoreInfo>,
    /// Session manager for hot exit
    session_manager: SessionManager,
    /// Background worker channel for session file writes
    session_save_tx: mpsc::Sender<SaveRequest>,
    /// Debounce deadline for the next session save
    session_save_deadline: Option<Instant>,
    /// Whether we're restoring a session (to avoid saving during restore)
    restoring_session: bool,
    /// Tick counter for periodic session save
    session_save_counter: u64,
    commands: WindowCommands,
    /// MCP command receiver (optional)
    _mcp_rx: Option<tokio_mpsc::UnboundedReceiver<McpCommand>>,
    /// Current keyboard modifiers state (for DnD with modifiers)
    current_modifiers: keyboard::Modifiers,
}

fn percent_decode(input: &str) -> String {
    fn hex_value(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h1), Some(h2)) = (hex_value(bytes[i + 1]), hex_value(bytes[i + 2])) {
                out.push(h1 * 16 + h2);
                i += 3;
                continue;
            }
        }

        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8_lossy(&out).into_owned()
}

fn file_uri_to_pathbuf(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;

    // Handle `file:///path` and `file://localhost/path`.
    let path_part = if rest.starts_with('/') {
        rest
    } else if let Some(without_host) = rest.strip_prefix("localhost") {
        without_host
    } else {
        // `file://<host>/...` (remote host) is ignored.
        return None;
    };

    Some(PathBuf::from(percent_decode(path_part)))
}

fn extract_paths_from_drag_drop(format: &str, data: &[u8]) -> Vec<PathBuf> {
    let Ok(text) = std::str::from_utf8(data) else {
        return Vec::new();
    };

    let mut lines = text.lines();

    // `x-special/gnome-copied-files` is typically:
    //   copy\nfile:///...\nfile:///...
    if format == "x-special/gnome-copied-files" {
        let _ = lines.next();
    }

    let mut paths = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(path) = file_uri_to_pathbuf(line) {
            paths.push(path);
        } else if line.starts_with('/') {
            // Some backends may provide a direct path.
            paths.push(PathBuf::from(percent_decode(line)));
        }
    }

    paths
}

#[derive(Clone, Debug)]
pub enum WindowManagerMessage {
    OpenWindow,
    /// Open a new window with a pre-created buffer (e.g., from paste as new image)
    OpenWindowWithBuffer,
    CloseWindow(window::Id),
    /// Window close button (X) was clicked - check for unsaved changes
    WindowCloseRequested(window::Id),
    WindowOpened(window::Id),
    /// Window opened with a pending buffer
    WindowOpenedWithBuffer(window::Id),
    FocusWindow(usize),
    /// Focus next widget (Tab)
    FocusNext,
    /// Focus previous widget (Shift+Tab)
    FocusPrevious,
    WindowClosed(window::Id),
    /// Window was moved - save session with new position
    WindowMoved(window::Id, Point),
    /// Window was resized - save session with new size  
    WindowResized(window::Id, Size),
    WindowMessage(window::Id, crate::ui::Message),
    Event(window::Id, icy_ui::Event),
    /// Autosave tick (periodic check)
    AutosaveTick,
    /// Debounced session-save tick
    SessionSaveTick,
    /// Animation tick for animation playback
    AnimationTick,
    /// MCP command received from automation server
    McpCommand(Arc<McpCommand>),
    /// File was dropped onto the window (with Ctrl state for new window)
    FileDropped(window::Id, PathBuf, bool),
}

fn start_session_save_worker() -> mpsc::Sender<SaveRequest> {
    let (tx, rx) = mpsc::channel::<SaveRequest>();

    std::thread::Builder::new()
        .name("icy_draw-session-save".to_string())
        .spawn(move || {
            let manager = SessionManager::new();

            loop {
                let request = match rx.recv() {
                    Ok(req) => req,
                    Err(_) => return,
                };

                // Coalesce bursts: always keep only the last pending state.
                let mut last_request = request;
                while let Ok(next) = rx.try_recv() {
                    last_request = next;
                }

                match last_request {
                    SaveRequest::Save(state) => {
                        if let Err(e) = manager.save_session(&state) {
                            log::error!("Failed to save session: {}", e);
                        }
                    }
                    SaveRequest::Flush(state, ack) => {
                        if let Err(e) = manager.save_session(&state) {
                            log::error!("Failed to save session: {}", e);
                        }
                        let _ = ack.send(());
                    }
                }
            }
        })
        .expect("failed to spawn session save worker");

    tx
}

impl WindowManager {
    /// Create a new WindowManager, restoring session if available

    pub fn new(font_library: SharedFontLibrary, mcp_rx: Option<tokio_mpsc::UnboundedReceiver<McpCommand>>) -> (Self, Task<WindowManagerMessage>) {
        let session_manager = SessionManager::new();
        let session_save_tx = start_session_save_worker();
        let options = Settings::load();
        let commands = WindowCommands::new();

        // Try to restore session
        if let Some(session) = session_manager.load_session() {
            log::info!("Restoring session with {} windows", session.windows.len());
            return Self::restore_session(font_library, session, session_manager, options, commands, mcp_rx);
        }

        // No session - open default window
        Self::open_initial_window(font_library, session_manager, session_save_tx, options, commands, None, mcp_rx)
    }

    /// Create WindowManager with a specific file (CLI argument - starts fresh session)
    pub fn with_path(
        font_library: SharedFontLibrary,
        path: PathBuf,
        mcp_rx: Option<tokio_mpsc::UnboundedReceiver<McpCommand>>,
    ) -> (Self, Task<WindowManagerMessage>) {
        let session_manager = SessionManager::new();
        let session_save_tx = start_session_save_worker();
        // Clear any existing session when opening via CLI
        session_manager.clear_session();

        let options = Settings::load();
        let commands = WindowCommands::new();

        Self::open_initial_window(font_library, session_manager, session_save_tx, options, commands, Some(path), mcp_rx)
    }

    /// Restore a session from saved state
    fn restore_session(
        font_library: SharedFontLibrary,
        session: SessionState,
        session_manager: SessionManager,
        options: Settings,
        commands: WindowCommands,
        mcp_rx: Option<tokio_mpsc::UnboundedReceiver<McpCommand>>,
    ) -> (Self, Task<WindowManagerMessage>) {
        // Convert all window states to restore info
        let mut pending_restores: Vec<WindowRestoreInfo> = session.windows.into_iter().map(|ws| ws.to_restore_info()).collect();

        // Pop first window to open immediately
        let first_restore = pending_restores.pop();

        let settings = if let Some(ref restore) = first_restore {
            let position = restore
                .position
                .map_or(window::Position::Default, |(x, y)| window::Position::Specific(Point::new(x, y)));
            window::Settings {
                size: Size::new(restore.size.0, restore.size.1),
                position,
                icon: load_window_icon(include_bytes!("../build/linux/256x256.png")).ok(),
                exit_on_close_request: false,
                ..window::Settings::default()
            }
        } else {
            window::Settings {
                size: DEFAULT_SIZE,
                icon: load_window_icon(include_bytes!("../build/linux/256x256.png")).ok(),
                exit_on_close_request: false,
                ..window::Settings::default()
            }
        };

        let (_, open) = window::open(settings);

        let session_save_tx = start_session_save_worker();

        let mut manager = Self {
            windows: BTreeMap::new(),
            window_geometry: BTreeMap::new(),
            options: Arc::new(RwLock::new(options)),
            font_library,
            pending_restores,
            session_manager,
            session_save_tx,
            session_save_deadline: None,
            restoring_session: true,
            session_save_counter: 0,
            commands,
            _mcp_rx: mcp_rx,
            current_modifiers: keyboard::Modifiers::default(),
        };

        // Store first window info for later
        if let Some(restore) = first_restore {
            manager.pending_restores.insert(0, restore);
        }

        (manager, open.map(WindowManagerMessage::WindowOpened))
    }

    /// Open initial window (no session restore)
    fn open_initial_window(
        font_library: SharedFontLibrary,
        session_manager: SessionManager,
        session_save_tx: mpsc::Sender<SaveRequest>,
        options: Settings,
        commands: WindowCommands,
        path: Option<PathBuf>,
        mcp_rx: Option<tokio_mpsc::UnboundedReceiver<McpCommand>>,
    ) -> (Self, Task<WindowManagerMessage>) {
        let window_icon = load_window_icon(include_bytes!("../build/linux/256x256.png")).ok();
        let settings = window::Settings {
            size: DEFAULT_SIZE,
            icon: window_icon,
            exit_on_close_request: false,
            ..window::Settings::default()
        };
        let (_, open) = window::open(settings);

        let pending = if let Some(p) = path {
            vec![WindowRestoreInfo {
                original_path: Some(p),
                load_path: None, // Will use original_path
                mark_dirty: false,
                position: None,
                size: (DEFAULT_SIZE.width, DEFAULT_SIZE.height),
                session_data_path: None,
            }]
        } else {
            vec![]
        };

        let options_arc = Arc::new(RwLock::new(options));

        let manager = Self {
            windows: BTreeMap::new(),
            window_geometry: BTreeMap::new(),
            options: options_arc,
            font_library,
            pending_restores: pending,
            session_manager,
            session_save_tx,
            session_save_deadline: None,
            restoring_session: false,
            session_save_counter: 0,
            commands,
            _mcp_rx: mcp_rx,
            current_modifiers: keyboard::Modifiers::default(),
        };

        let task: Task<WindowManagerMessage> = open.map(WindowManagerMessage::WindowOpened);

        (manager, task)
    }

    fn request_session_save_debounced(&mut self) {
        if self.restoring_session {
            return;
        }
        self.session_save_deadline = Some(Instant::now() + Duration::from_millis(SESSION_SAVE_DEBOUNCE_MS));
    }

    fn enqueue_session_save_now(&self) {
        let state = self.build_session_state();
        let _ = self.session_save_tx.send(SaveRequest::Save(state));
    }

    fn flush_session_save_and_wait(&self, timeout: Duration) {
        let state = self.build_session_state();
        let (ack_tx, ack_rx) = mpsc::channel::<()>();
        if self.session_save_tx.send(SaveRequest::Flush(state, ack_tx)).is_ok() {
            let _ = ack_rx.recv_timeout(timeout);
        }
    }

    fn build_session_state(&self) -> SessionState {
        let mut window_states = Vec::new();

        for (window_id, window) in self.windows.iter() {
            let has_unsaved = window.is_modified();
            let file_path = window.file_path().cloned();

            // Do NOT write autosaves here. Autosave is handled by `AutosaveTick`.
            // We only store the path so restore can pick it up.
            let autosave_path = if has_unsaved {
                if let Some(ref path) = file_path {
                    Some(self.session_manager.get_autosave_path(path))
                } else {
                    Some(self.session_manager.get_untitled_autosave_path(window.id))
                }
            } else {
                None
            };

            // Get session data path for this window and save it
            let session_data_path = if let Some(ref path) = file_path {
                Some(self.session_manager.get_session_data_path(path))
            } else {
                Some(self.session_manager.get_untitled_session_data_path(window.id))
            };

            // Save session data to disk (bitcode serialization)
            if let Some(ref session_path) = session_data_path {
                if let Some(session_data) = window.get_session_data() {
                    if let Err(e) = self.session_manager.save_session_data(session_path, &session_data) {
                        log::warn!("Failed to save session data for window {}: {}", window.id, e);
                    }
                }
            }

            let geom = self.window_geometry.get(window_id).cloned().unwrap_or_default();

            window_states.push(WindowState {
                position: geom.position,
                size: geom.size,
                file_path,
                edit_mode: edit_mode_to_string(&window.mode()),
                has_unsaved_changes: has_unsaved,
                autosave_path,
                session_data_path,
            });
        }

        SessionState {
            version: 1,
            windows: window_states,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn title(&self, window_id: window::Id) -> String {
        let Some(w) = self.windows.get(&window_id) else {
            return String::new();
        };

        // Compute title dynamically to ensure dirty state is always current
        let base_title = w.compute_title();
        let zoom_info = w.get_zoom_info_string();
        let title_with_zoom = if zoom_info.is_empty() {
            base_title
        } else {
            format!("{} {}", base_title, zoom_info)
        };

        if self.windows.len() == 1 {
            title_with_zoom
        } else if w.id <= 10 {
            let display_key = if w.id == 10 { 0 } else { w.id };
            format!("{} - ⌘{}", title_with_zoom, display_key)
        } else {
            title_with_zoom
        }
    }

    pub fn update(&mut self, message: WindowManagerMessage) -> Task<WindowManagerMessage> {
        // Poll for MCP commands on every update cycle
        let mcp_task: Task<WindowManagerMessage> = self.poll_mcp_commands();

        let main_task = match message {
            WindowManagerMessage::OpenWindow => {
                let Some(last_window) = self.windows.keys().last() else {
                    return Task::batch([mcp_task]);
                };

                window::position(*last_window)
                    .then(|last_position| {
                        let position = last_position.map_or(window::Position::Default, |last_position| {
                            window::Position::Specific(last_position + Vector::new(20.0, 20.0))
                        });
                        let window_icon = load_window_icon(include_bytes!("../build/linux/256x256.png")).ok();
                        let settings = window::Settings {
                            position,
                            icon: window_icon,
                            size: DEFAULT_SIZE,
                            exit_on_close_request: false,
                            ..window::Settings::default()
                        };

                        let (_, open) = window::open(settings);
                        open
                    })
                    .map(WindowManagerMessage::WindowOpened)
            }

            WindowManagerMessage::OpenWindowWithBuffer => {
                // Open a new window with a pending buffer (from paste as new image)
                let Some(last_window) = self.windows.keys().last() else {
                    return Task::batch([mcp_task]);
                };

                window::position(*last_window)
                    .then(|last_position| {
                        let position = last_position.map_or(window::Position::Default, |last_position| {
                            window::Position::Specific(last_position + Vector::new(20.0, 20.0))
                        });
                        let window_icon = load_window_icon(include_bytes!("../build/linux/256x256.png")).ok();
                        let settings = window::Settings {
                            position,
                            icon: window_icon,
                            size: DEFAULT_SIZE,
                            exit_on_close_request: false,
                            ..window::Settings::default()
                        };

                        let (_, open) = window::open(settings);
                        open
                    })
                    .map(WindowManagerMessage::WindowOpenedWithBuffer)
            }

            WindowManagerMessage::CloseWindow(id) => {
                // Check if window has unsaved changes
                if let Some(window) = self.windows.get(&id) {
                    if window.is_modified() {
                        // Send CloseFile message to window - it will show the save dialog
                        return Task::done(WindowManagerMessage::WindowMessage(id, crate::ui::Message::CloseFile));
                    }
                }

                // No unsaved changes or window not found - close directly
                // If this is the last window, save session first
                if self.windows.len() == 1 {
                    return self.save_session_and_close(id);
                }
                window::close(id)
            }

            WindowManagerMessage::WindowOpened(id) => {
                // Get pending restore info if any
                let pending = self.pending_restores.pop();

                let window = if let Some(ref restore) = pending {
                    // Initialize geometry from restore info
                    self.window_geometry.insert(
                        id,
                        WindowGeometry {
                            position: restore.position,
                            size: restore.size,
                        },
                    );

                    // Create window with restore info (handles autosave + original path)
                    let mut w = MainWindow::new_restored(
                        find_next_window_id(&self.windows),
                        restore.original_path.clone(),
                        restore.load_path.clone(),
                        restore.mark_dirty,
                        self.options.clone(),
                        self.font_library.clone(),
                    );

                    // Restore session data if available
                    if let Some(ref session_path) = restore.session_data_path {
                        if let Some(session_data) = self.session_manager.load_session_data(session_path) {
                            w.set_session_data(session_data);
                            // After restoring session data (including undo stack), update last_save
                            // to match the new undo stack length - unless mark_dirty is set,
                            // which means we want the document to remain dirty (e.g., from autosave)
                            if !restore.mark_dirty {
                                w.mark_saved();
                            }
                            log::debug!("Restored session data from {:?}", session_path);
                        }
                    }

                    w
                } else {
                    // Initialize with default geometry
                    self.window_geometry.insert(
                        id,
                        WindowGeometry {
                            position: None,
                            size: (DEFAULT_SIZE.width, DEFAULT_SIZE.height),
                        },
                    );

                    // No pending - create empty window
                    MainWindow::new(find_next_window_id(&self.windows), None, self.options.clone(), self.font_library.clone())
                };

                // Initialize autosave status
                self.session_manager.get_autosave_status(id, window.undo_stack_len());

                self.windows.insert(id, window);

                // If more windows pending, open next
                if !self.pending_restores.is_empty() {
                    let next = self.pending_restores.last().unwrap();
                    let position = next
                        .position
                        .map_or(window::Position::Default, |(x, y)| window::Position::Specific(Point::new(x, y)));
                    let settings = window::Settings {
                        size: Size::new(next.size.0, next.size.1),
                        position,
                        icon: load_window_icon(include_bytes!("../build/linux/256x256.png")).ok(),
                        exit_on_close_request: false,
                        ..window::Settings::default()
                    };
                    let (_, open) = window::open(settings);
                    return open.map(WindowManagerMessage::WindowOpened);
                }

                // Session restore complete
                if self.restoring_session {
                    self.restoring_session = false;
                    // Clear session file now that restore is complete
                    self.session_manager.clear_session();
                    log::info!("Session restore complete");
                } else {
                    // Save session when a new window opens (not during restore)
                    self.request_session_save_debounced();
                }

                Task::none()
            }

            WindowManagerMessage::WindowOpenedWithBuffer(id) => {
                // Get buffer from global static (set by PasteAsNewImage handler)
                let buffer = crate::PENDING_NEW_WINDOW_BUFFERS.lock().ok().and_then(|mut pending| pending.pop());

                // Initialize with default geometry
                self.window_geometry.insert(
                    id,
                    WindowGeometry {
                        position: None,
                        size: (DEFAULT_SIZE.width, DEFAULT_SIZE.height),
                    },
                );

                let window = if let Some(buf) = buffer {
                    MainWindow::with_buffer(find_next_window_id(&self.windows), buf, self.options.clone(), self.font_library.clone())
                } else {
                    // Fallback to empty window if no buffer pending
                    MainWindow::new(find_next_window_id(&self.windows), None, self.options.clone(), self.font_library.clone())
                };

                // Initialize autosave status
                self.session_manager.get_autosave_status(id, window.undo_stack_len());

                self.windows.insert(id, window);
                self.request_session_save_debounced();

                Task::none()
            }

            WindowManagerMessage::WindowMoved(id, position) => {
                // Update cached geometry
                let geom = self.window_geometry.entry(id).or_insert_with(|| WindowGeometry {
                    position: None,
                    size: (DEFAULT_SIZE.width, DEFAULT_SIZE.height),
                });
                geom.position = Some((position.x, position.y));

                // Save session when window position changes
                if !self.restoring_session {
                    self.request_session_save_debounced();
                }
                Task::none()
            }

            WindowManagerMessage::WindowResized(id, size) => {
                // Update cached geometry
                let geom = self.window_geometry.entry(id).or_insert_with(|| WindowGeometry {
                    position: None,
                    size: (DEFAULT_SIZE.width, DEFAULT_SIZE.height),
                });
                geom.size = (size.width, size.height);

                // Save session when window size changes
                if !self.restoring_session {
                    self.request_session_save_debounced();
                }
                Task::none()
            }

            WindowManagerMessage::WindowCloseRequested(id) => {
                // X button was clicked - check for unsaved changes before closing
                // This is handled the same as CloseWindow
                self.update(WindowManagerMessage::CloseWindow(id))
            }

            WindowManagerMessage::WindowClosed(id) => {
                // Always enqueue session save when a window closes (async)
                if !self.restoring_session {
                    self.request_session_save_debounced();
                }

                // Remove autosave status
                self.session_manager.remove_autosave_status(id);

                // Remove window and remove its autosave if clean
                if let Some(window) = self.windows.get(&id) {
                    if !window.is_modified() {
                        // Clean close - remove autosave
                        if let Some(path) = window.file_path() {
                            let autosave_path = self.session_manager.get_autosave_path(path);
                            self.session_manager.remove_autosave(&autosave_path);
                        }
                    }
                }

                self.windows.remove(&id);
                self.window_geometry.remove(&id);

                if self.windows.is_empty() {
                    icy_ui::exit()
                } else {
                    Task::none()
                }
            }

            WindowManagerMessage::WindowMessage(id, msg) => {
                // Close the current editor window (menu click)
                if matches!(msg, crate::ui::Message::CloseEditor) {
                    return Task::done(WindowManagerMessage::CloseWindow(id));
                }

                // Quit app (menu click): attempt to close all windows.
                // Modified windows will prompt via the existing CloseWindow flow.
                if matches!(msg, crate::ui::Message::QuitApp) {
                    let window_ids: Vec<_> = self.windows.keys().cloned().collect();
                    let tasks: Vec<_> = window_ids.into_iter().map(|wid| Task::done(WindowManagerMessage::CloseWindow(wid))).collect();
                    return Task::batch(tasks);
                }

                // Handle ForceCloseFile by closing the window directly (bypass is_modified check)
                if matches!(msg, crate::ui::Message::ForceCloseFile) {
                    // If this is the last window, save session first
                    if self.windows.len() == 1 {
                        return self.save_session_and_close(id);
                    }
                    return window::close(id);
                }

                // Handle OpenNewWindowWithBuffer - open a new window with pending buffer from global static
                if matches!(msg, crate::ui::Message::OpenNewWindowWithBuffer) {
                    return Task::done(WindowManagerMessage::OpenWindowWithBuffer);
                }

                // Check if this is a confirmed save success message - clear autosave only then
                let is_save_success = matches!(msg, crate::ui::Message::SaveSucceeded(_));
                // Check if this is a file open message - save session after opening
                let is_file_open = matches!(msg, crate::ui::Message::FileOpened(_) | crate::ui::Message::OpenRecentFile(_));

                if let Some(window) = self.windows.get_mut(&id) {
                    let task = window.update(msg).map(move |msg| WindowManagerMessage::WindowMessage(id, msg));

                    // After a successful save, remove autosave
                    if is_save_success {
                        if let Some(path) = window.file_path() {
                            let autosave_path = self.session_manager.get_autosave_path(path);
                            self.session_manager.remove_autosave(&autosave_path);
                        }
                        // Update autosave status
                        let status = self.session_manager.get_autosave_status(id, 0);
                        status.mark_saved(window.undo_stack_len());
                    }

                    // After file open, save session
                    if is_file_open {
                        self.request_session_save_debounced();
                    }

                    return task;
                }
                Task::none()
            }

            WindowManagerMessage::Event(window_id, ref event) => {
                // Track keyboard modifiers from keyboard events
                if let Event::Keyboard(keyboard::Event::KeyPressed { modifiers, .. } | keyboard::Event::KeyReleased { modifiers, .. }) = event {
                    self.current_modifiers = *modifiers;
                }

                // Track modifiers while dragging (new DnD API)
                if let Event::Window(window::Event::DragMoved { modifiers, .. }) = event {
                    self.current_modifiers = *modifiers;
                }

                // Handle file drop events - use current modifier state
                if let Event::Window(window::Event::DragDropped { data, format, .. }) = event {
                    let paths = extract_paths_from_drag_drop(format, data);
                    let Some(path) = paths.last() else {
                        return Task::none();
                    };

                    let open_in_new_window = self.current_modifiers.control();
                    return Task::done(WindowManagerMessage::FileDropped(window_id, path.clone(), open_in_new_window));
                }

                // Handle keyboard commands at window manager level
                if let Some(msg) = self.commands.handle(event, window_id) {
                    return Task::done(msg);
                }

                // Pass event to window for other handling
                if let Some(window) = self.windows.get_mut(&window_id) {
                    let (msg_opt, task) = window.handle_event(event);
                    let msg_task = if let Some(msg) = msg_opt {
                        Task::done(WindowManagerMessage::WindowMessage(window_id, msg))
                    } else {
                        Task::none()
                    };
                    let dialog_task: Task<WindowManagerMessage> = task.map(move |msg| WindowManagerMessage::WindowMessage(window_id, msg));
                    return Task::batch([msg_task, dialog_task]);
                }
                Task::none()
            }

            WindowManagerMessage::FocusWindow(target_id) => focus_window_by_id(&self.windows, target_id),

            WindowManagerMessage::FocusNext => icy_ui::widget::operation::focus_next(),

            WindowManagerMessage::FocusPrevious => icy_ui::widget::operation::focus_previous(),

            WindowManagerMessage::AutosaveTick => {
                // Increment session save counter
                self.session_save_counter += 1;

                // Save session periodically (every SESSION_SAVE_INTERVAL_SECS seconds)
                if self.session_save_counter >= SESSION_SAVE_INTERVAL_SECS && !self.restoring_session {
                    self.session_save_counter = 0;
                    self.enqueue_session_save_now();
                }

                // Check each window for autosave
                for (window_id, window) in self.windows.iter() {
                    let undo_len = window.undo_stack_len();

                    if self.session_manager.should_autosave(*window_id, undo_len) {
                        // Perform autosave
                        let autosave_path = if let Some(path) = window.file_path() {
                            self.session_manager.get_autosave_path(path)
                        } else {
                            // Untitled document - use window's id (1-based display id)
                            self.session_manager.get_untitled_autosave_path(window.id)
                        };

                        match window.get_autosave_bytes() {
                            Ok(bytes) => {
                                if let Err(e) = self.session_manager.save_autosave(&autosave_path, &bytes) {
                                    log::error!("Autosave failed for {:?}: {}", autosave_path, e);
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to get autosave bytes for {:?}: {}", autosave_path, e);
                            }
                        }
                    }
                }
                Task::none()
            }

            WindowManagerMessage::SessionSaveTick => {
                if let Some(deadline) = self.session_save_deadline {
                    if Instant::now() >= deadline {
                        self.session_save_deadline = None;
                        self.enqueue_session_save_now();
                    }
                }
                Task::none()
            }

            WindowManagerMessage::AnimationTick => {
                // Send tick to all windows that need animation updates
                let tasks: Vec<_> = self
                    .windows
                    .iter_mut()
                    .filter(|(_, w)| w.needs_animation_tick())
                    .map(|(&wid, w)| {
                        w.update(crate::ui::Message::AnimationEditor(crate::ui::editor::animation::AnimationEditorMessage::Tick))
                            .map(move |msg| WindowManagerMessage::WindowMessage(wid, msg))
                    })
                    .collect();
                Task::batch(tasks)
            }

            WindowManagerMessage::McpCommand(cmd) => {
                // Route MCP commands to the active window
                if let Some((_window_id, window)) = self.windows.iter_mut().next() {
                    window.handle_mcp_command(&cmd);
                }
                Task::none()
            }

            WindowManagerMessage::FileDropped(window_id, path, open_in_new_window) => {
                if open_in_new_window {
                    // Ctrl+Drop: Open in new window
                    // Store the path in pending_restores and open a new window
                    self.pending_restores.push(WindowRestoreInfo {
                        original_path: Some(path),
                        load_path: None,
                        mark_dirty: false,
                        position: None,
                        size: (DEFAULT_SIZE.width, DEFAULT_SIZE.height),
                        session_data_path: None,
                    });
                    return Task::done(WindowManagerMessage::OpenWindow);
                } else {
                    // Normal drop: Open in current window (like File Open)
                    if let Some(window) = self.windows.get(&window_id) {
                        // Check if current window has unsaved changes
                        if window.is_modified() {
                            // Route through the dirty check flow
                            return Task::done(WindowManagerMessage::WindowMessage(
                                window_id,
                                crate::ui::Message::OpenRecentFile(path),
                            ));
                        }
                    }
                    // No unsaved changes - open directly
                    return Task::done(WindowManagerMessage::WindowMessage(
                        window_id,
                        crate::ui::Message::FileOpened(path),
                    ));
                }
            }
        };

        Task::batch([main_task, mcp_task])
    }

    /// Save session and close the last window
    fn save_session_and_close(&mut self, window_id: window::Id) -> Task<WindowManagerMessage> {
        // Flush session state (worker thread) before closing the last window
        self.flush_session_save_and_wait(Duration::from_millis(750));

        // Close the window
        window::close(window_id)
    }

    pub fn view(&self, window_id: window::Id) -> Element<'_, WindowManagerMessage> {
        let id = window_id;
        if let Some(window) = self.windows.get(&window_id) {
            window.view().map(move |msg| WindowManagerMessage::WindowMessage(id, msg))
        } else {
            space().into()
        }
    }

    pub fn theme(&self, window: window::Id) -> Option<Theme> {
        Some(self.windows.get(&window)?.theme())
    }

    pub fn subscription(&self) -> Subscription<WindowManagerMessage> {
        let mut subs = vec![
            // Intercept window close requests (X button) to check for unsaved changes
            window::close_requests().map(WindowManagerMessage::WindowCloseRequested),
            window::resize_events().map(|(id, size)| WindowManagerMessage::WindowResized(id, size)),
            window::events().filter_map(|(id, event)| {
                match event {
                    window::Event::Moved(position) => Some(WindowManagerMessage::WindowMoved(id, position)),
                    window::Event::Opened { size, .. } => Some(WindowManagerMessage::WindowResized(id, size)),
                    window::Event::Closed => Some(WindowManagerMessage::WindowClosed(id)),
                    // CloseRequested is handled by close_requests() above - ignore here to prevent duplicate
                    window::Event::CloseRequested => None,
                    // Animation tick is now handled via dedicated subscription
                    window::Event::RedrawRequested(_) => None,
                    _ => Some(WindowManagerMessage::Event(id, Event::Window(event))),
                }
            }),
            icy_ui::event::listen_with(|event, _status, window_id| {
                match &event {
                    // Window focus events
                    Event::Window(window::Event::Focused) | Event::Window(window::Event::Unfocused) => Some(WindowManagerMessage::Event(window_id, event)),
                    // External drag-and-drop events (new DnD API)
                    Event::Window(window::Event::DragEntered { .. }) => Some(WindowManagerMessage::Event(window_id, event)),
                    Event::Window(window::Event::DragMoved { .. }) => Some(WindowManagerMessage::Event(window_id, event)),
                    Event::Window(window::Event::DragDropped { .. }) => Some(WindowManagerMessage::Event(window_id, event.clone())),
                    Event::Window(window::Event::DragLeft) => Some(WindowManagerMessage::Event(window_id, event)),
                    Event::Window(window::Event::DragSource(_)) => Some(WindowManagerMessage::Event(window_id, event)),
                    // Mouse events
                    Event::Mouse(icy_ui::mouse::Event::WheelScrolled { .. }) => Some(WindowManagerMessage::Event(window_id, event)),
                    Event::Mouse(icy_ui::mouse::Event::CursorMoved { .. }) => Some(WindowManagerMessage::Event(window_id, event)),
                    Event::Mouse(icy_ui::mouse::Event::ButtonPressed { .. }) => Some(WindowManagerMessage::Event(window_id, event)),
                    Event::Mouse(_) => None,
                    // Keyboard events are handled below
                    Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                        // Handle window manager keyboard shortcuts (Tab, Alt+Number, etc.)
                        if let Some(action) = icy_engine_gui::handle_window_manager_keyboard_press(key, modifiers) {
                            use icy_engine_gui::KeyboardAction;
                            return match action {
                                KeyboardAction::FocusWindow(target_id) => Some(WindowManagerMessage::FocusWindow(target_id)),
                                KeyboardAction::FocusNext => Some(WindowManagerMessage::FocusNext),
                                KeyboardAction::FocusPrevious => Some(WindowManagerMessage::FocusPrevious),
                            };
                        }
                        Some(WindowManagerMessage::Event(window_id, event))
                    }
                    Event::Keyboard(_) => Some(WindowManagerMessage::Event(window_id, event)),
                    _ => None,
                }
            }),
            // Autosave tick - check every second
            icy_ui::time::every(std::time::Duration::from_secs(1)).map(|_| WindowManagerMessage::AutosaveTick),
        ];

        // Debounced session-save tick (only when something scheduled)
        // Keep session-save ticking even if nothing is scheduled.
        // This avoids edge cases where a deadline is set but the subscription
        // doesn't get activated soon enough due to update timing.
        subs.push(icy_ui::time::every(Duration::from_millis(200)).map(|_| WindowManagerMessage::SessionSaveTick));

        // Animation tick - only active when an animation editor is playing
        let needs_animation = self.windows.values().any(|w| w.needs_animation_tick());
        if needs_animation {
            subs.push(icy_ui::time::every(Duration::from_millis(16)).map(|_| WindowManagerMessage::AnimationTick));
        }

        // Add collaboration subscriptions from all windows
        for (&window_id, window) in &self.windows {
            let collab_sub = window
                .subscription()
                .with(window_id)
                .map(|(wid, msg)| WindowManagerMessage::WindowMessage(wid, msg));
            subs.push(collab_sub);
        }

        Subscription::batch(subs)
    }

    /// Poll MCP commands from the receiver (called from update)
    pub fn poll_mcp_commands(&mut self) -> Task<WindowManagerMessage> {
        /*if let Some(ref mut rx) = self.mcp_rx {
            // Try to receive without blocking
            match rx.try_recv() {
                Ok(cmd) => {
                    return Task::done(WindowManagerMessage::McpCommand(Arc::new(cmd)));
                }
                Err(tokio_mpsc::error::TryRecvError::Empty) => {}
                Err(tokio_mpsc::error::TryRecvError::Disconnected) => {
                    log::warn!("MCP channel disconnected");
                    self.mcp_rx = None;
                }
            }
        }*/
        Task::none()
    }

    /// Build recent files submenu items with stable IDs
    fn build_recent_files_submenu<F>(recent_files: &[std::path::PathBuf], wrap: F) -> Vec<menu::MenuNode<WindowManagerMessage>>
    where
        F: Fn(crate::ui::main_window::Message) -> WindowManagerMessage + Copy,
    {
        use crate::fl;
        use crate::ui::main_window::Message;

        let mut items: Vec<menu::MenuNode<WindowManagerMessage>> = Vec::new();

        if recent_files.is_empty() {
            items.push(menu::MenuNode::item_with_id(menu::MenuId::from_str("recent.empty"), fl!("menu-no_recent_files"), wrap(Message::Noop)).enabled(false));
        } else {
            for (i, file) in recent_files.iter().rev().take(10).enumerate() {
                let file_name = file
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| file.display().to_string());
                items.push(menu::MenuNode::item_with_id(
                    menu::MenuId::from_str(&format!("recent.{}", i)),
                    file_name,
                    wrap(Message::OpenRecentFile(file.clone())),
                ));
            }
            items.push(menu::separator!());
            items.push(menu::item!(fl!("menu-clear_recent_files"), wrap(Message::ClearRecentFiles)));
        }

        items
    }

    fn menu_id_escape(input: &str) -> String {
        input.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
    }

    fn build_plugins_submenu<F>(plugins: &[crate::Plugin], wrap: F) -> Vec<menu::MenuNode<WindowManagerMessage>>
    where
        F: Fn(crate::ui::Message) -> WindowManagerMessage + Copy,
    {
        use crate::fl;
        use crate::ui::editor::ansi::AnsiEditorMessage;
        use crate::ui::Message;

        if plugins.is_empty() {
            return vec![menu::MenuNode::item_with_id(menu::MenuId::from_str("plugins.empty"), fl!("menu-no_plugins"), wrap(Message::Noop)).enabled(false)];
        }

        let mut out: Vec<menu::MenuNode<WindowManagerMessage>> = Vec::new();

        for (group, items) in crate::Plugin::group_by_path(plugins) {
            if group.is_empty() {
                for (i, p) in items {
                    out.push(menu::MenuNode::item_with_id(
                        menu::MenuId::from_str(&format!("plugins.item.{i}")),
                        p.title.clone(),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::RunPlugin(i))),
                    ));
                }
                continue;
            }

            let group_id = Self::menu_id_escape(&group);
            let mut group_nodes: Vec<menu::MenuNode<WindowManagerMessage>> = Vec::new();
            for (i, p) in items {
                group_nodes.push(menu::MenuNode::item_with_id(
                    menu::MenuId::from_str(&format!("plugins.{group_id}.{i}")),
                    p.title.clone(),
                    wrap(Message::AnsiEditor(AnsiEditorMessage::RunPlugin(i))),
                ));
            }

            out.push(menu::MenuNode::submenu_with_id(
                menu::MenuId::from_str(&format!("plugins.group.{group_id}")),
                group,
                group_nodes,
            ));
        }

        out
    }

    /// Application menu (native on macOS, widget-based on other platforms)
    pub fn application_menu(state: &WindowManager, context: &menu::MenuContext) -> Option<menu::AppMenu<WindowManagerMessage>> {
        use crate::fl;
        use crate::ui::Message;
        use icy_ui::keyboard::key::Named;
        use icy_ui::keyboard::Key;

        use crate::ui::editor::animation::AnimationEditorMessage;
        use crate::ui::editor::ansi::{AnsiEditorCoreMessage, AnsiEditorMessage, AnsiViewMenuState, ColorSwitcherMessage};
        use crate::ui::editor::bitfont::BitFontEditorMessage;
        use crate::ui::editor::charfont::CharFontEditorMessage;

        // Get the focused window from context, or fall back to first window
        let (focused_window_id, focused_window) = match context.current_window.and_then(|id| state.windows.get(&id).map(|w| (id, w))) {
            Some((id, window)) => (id, window),
            None => match state.windows.iter().next() {
                Some((id, window)) => (*id, window),
                None => {
                    // No windows yet - return basic menu without window-specific items
                    let dummy_id = window::Id::unique();
                    let wrap = move |msg: Message| WindowManagerMessage::WindowMessage(dummy_id, msg);

                    let file_menu = menu::submenu!(
                        fl!("menu-file"),
                        [
                            menu::item!(
                                fl!("menu-new"),
                                WindowManagerMessage::OpenWindow,
                                menu::MenuShortcut::cmd(Key::Character("n".into()))
                            ),
                            menu::separator!(),
                            menu::quit!(wrap(Message::QuitApp)),
                        ]
                    );
                    let help_menu = menu::submenu!(fl!("menu-help"), [menu::about!(fl!("menu-about"), wrap(Message::ShowAbout)),]);
                    return Some(menu::AppMenu::new(vec![file_menu, help_menu]));
                }
            },
        };
        let window_id = focused_window_id;
        let edit_mode = focused_window.mode();

        // Get window state for menu generation
        let undo_info = focused_window.get_undo_info();
        let is_connected = focused_window.is_connected();
        let ansi_view_state: AnsiViewMenuState = focused_window.ansi_view_menu_state().unwrap_or_default();
        let ansi_mirror_mode: bool = focused_window.ansi_mirror_mode().unwrap_or(false);
        let plugins = focused_window.plugins().clone();

        // Helper to wrap Message in WindowManagerMessage
        let wrap = move |msg: Message| WindowManagerMessage::WindowMessage(window_id, msg);

        // Build Recent Files submenu using helper
        let recent_files = state.options.read().recent_files.files().clone();
        let recent_items = Self::build_recent_files_submenu(&recent_files, wrap);

        let file_export_item: menu::MenuNode<WindowManagerMessage> = match edit_mode {
            crate::ui::EditMode::BitFont => menu::item!(
                fl!("menu-export-font"),
                wrap(Message::BitFontEditor(BitFontEditorMessage::ShowExportFontDialog))
            ),
            crate::ui::EditMode::CharFont => {
                menu::item!(fl!("menu-export-font"), wrap(Message::CharFontEditor(CharFontEditorMessage::ExportFont)))
            }
            crate::ui::EditMode::Animation => {
                menu::item!(fl!("menu-export"), wrap(Message::AnimationEditor(AnimationEditorMessage::ShowExportDialog)))
            }
            crate::ui::EditMode::Ansi => menu::item!(fl!("menu-export"), wrap(Message::ExportFile)),
        };

        // File menu with Recent Files submenu
        let mut file_nodes: Vec<menu::MenuNode<WindowManagerMessage>> = vec![
            menu::item!(fl!("menu-new"), wrap(Message::NewFile), menu::MenuShortcut::cmd(Key::Character("n".into()))),
            menu::item!(fl!("menu-open"), wrap(Message::OpenFile), menu::MenuShortcut::cmd(Key::Character("o".into()))),
            menu::MenuNode::submenu_with_id(menu::MenuId::from_str("menu.recent"), fl!("menu-open_recent"), recent_items),
            menu::separator!(),
            menu::item!(fl!("menu-save"), wrap(Message::SaveFile), menu::MenuShortcut::cmd(Key::Character("s".into()))).enabled(!is_connected),
            menu::item!(
                fl!("menu-save-as"),
                wrap(Message::SaveFileAs),
                menu::MenuShortcut::cmd_shift(Key::Character("s".into()))
            )
            .enabled(!is_connected),
            menu::separator!(),
            file_export_item,
        ];

        if edit_mode == crate::ui::EditMode::BitFont {
            file_nodes.push(menu::item!(fl!("menu-import-font"), wrap(Message::ShowImportFontDialog)));
        }

        if edit_mode == crate::ui::EditMode::CharFont {
            file_nodes.push(menu::item!(
                fl!("menu-import-fonts"),
                wrap(Message::CharFontEditor(CharFontEditorMessage::ImportFonts))
            ));
        }

        if edit_mode == crate::ui::EditMode::Ansi {
            file_nodes.push(menu::separator!());
            file_nodes.push(menu::item!(fl!("menu-import-font"), wrap(Message::ShowImportFontDialog)));
        }

        file_nodes.extend([
            menu::separator!(),
            menu::item!(fl!("menu-connect-to-server"), wrap(Message::ShowConnectDialog)),
            menu::separator!(),
            menu::preferences!(fl!("menu-show_settings"), wrap(Message::ShowSettings)),
            menu::separator!(),
            menu::item!(
                fl!("menu-close-editor"),
                wrap(Message::CloseEditor),
                menu::MenuShortcut::cmd(Key::Character("w".into()))
            ),
            menu::quit!(wrap(Message::QuitApp)),
        ]);

        let file_menu = menu::MenuNode::submenu_with_id(menu::MenuId::from_str("menu.file"), fl!("menu-file"), file_nodes);

        // Edit menu - with dynamic undo/redo labels
        let undo_label = match &undo_info.undo_description {
            Some(desc) => format!("&Undo {}", desc),
            None => "&Undo".to_string(),
        };
        let redo_label = match &undo_info.redo_description {
            Some(desc) => format!("&Redo {}", desc),
            None => "&Redo".to_string(),
        };

        let mut edit_nodes: Vec<menu::MenuNode<WindowManagerMessage>> = vec![
            menu::item!(undo_label, wrap(Message::Undo), menu::MenuShortcut::cmd(Key::Character("z".into()))).enabled(undo_info.undo_description.is_some()),
            menu::item!(redo_label, wrap(Message::Redo), menu::MenuShortcut::cmd_shift(Key::Character("z".into())))
                .enabled(undo_info.redo_description.is_some()),
            menu::separator!(),
            menu::item!(fl!("menu-cut"), wrap(Message::Cut), menu::MenuShortcut::cmd(Key::Character("x".into()))),
            menu::item!(fl!("menu-copy"), wrap(Message::Copy), menu::MenuShortcut::cmd(Key::Character("c".into()))),
            menu::item!(
                fl!("menu-paste"),
                wrap(Message::Paste),
                menu::MenuShortcut::cmd(Key::Character("v".into()))
            ),
        ];

        if edit_mode == crate::ui::EditMode::Ansi {
            edit_nodes.push(menu::MenuNode::submenu_with_id(
                menu::MenuId::from_str("menu.paste_as"),
                fl!("menu-paste-as"),
                vec![menu::item!(fl!("menu-paste-as-new-image"), wrap(Message::PasteAsNewImage(None)))],
            ));
            edit_nodes.push(menu::item!(fl!("menu-insert-sixel-from-file"), wrap(Message::InsertSixelFromFile)));
            edit_nodes.push(menu::separator!());

            let area_ops = menu::MenuNode::submenu_with_id(
                menu::MenuId::from_str("menu.area_ops"),
                fl!("menu-area_operations"),
                vec![
                    menu::item!(
                        fl!("menu-justify_line_left"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLineLeft)))
                    ),
                    menu::item!(
                        fl!("menu-justify_line_center"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLineCenter)))
                    ),
                    menu::item!(
                        fl!("menu-justify_line_right"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLineRight)))
                    ),
                    menu::separator!(),
                    menu::item!(
                        fl!("menu-insert_row"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::InsertRow)))
                    ),
                    menu::item!(
                        fl!("menu-delete_row"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::DeleteRow)))
                    ),
                    menu::item!(
                        fl!("menu-insert_colum"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::InsertColumn)))
                    ),
                    menu::item!(
                        fl!("menu-delete_colum"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::DeleteColumn)))
                    ),
                    menu::separator!(),
                    menu::item!(
                        fl!("menu-erase_row"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseRow)))
                    ),
                    menu::item!(
                        fl!("menu-erase_row_to_start"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseRowToStart)))
                    ),
                    menu::item!(
                        fl!("menu-erase_row_to_end"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseRowToEnd)))
                    ),
                    menu::separator!(),
                    menu::item!(
                        fl!("menu-erase_column"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseColumn)))
                    ),
                    menu::item!(
                        fl!("menu-erase_column_to_start"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseColumnToStart)))
                    ),
                    menu::item!(
                        fl!("menu-erase_column_to_end"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseColumnToEnd)))
                    ),
                    menu::separator!(),
                    menu::item!(
                        fl!("menu-scroll_area_up"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ScrollAreaUp)))
                    ),
                    menu::item!(
                        fl!("menu-scroll_area_down"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ScrollAreaDown)))
                    ),
                    menu::item!(
                        fl!("menu-scroll_area_left"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ScrollAreaLeft)))
                    ),
                    menu::item!(
                        fl!("menu-scroll_area_right"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ScrollAreaRight)))
                    ),
                ],
            );
            edit_nodes.push(area_ops);
            edit_nodes.push(menu::separator!());
            edit_nodes.push(menu::item!(
                fl!("menu-open_font_selector"),
                wrap(Message::AnsiEditor(AnsiEditorMessage::OpenFontSelector))
            ));
            edit_nodes.push(menu::separator!());
            edit_nodes.push(menu::check_item!(
                fl!("menu-mirror_mode"),
                Some(ansi_mirror_mode),
                wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleMirrorMode))),
            ));
            edit_nodes.push(menu::separator!());
            edit_nodes.push(menu::item!(fl!("menu-file-settings"), wrap(Message::ShowFileSettingsDialog)));
        }

        if edit_mode == crate::ui::EditMode::BitFont {
            edit_nodes.push(menu::separator!());
            edit_nodes.push(menu::item!(
                fl!("cmd-bitfont-swap_chars-menu"),
                wrap(Message::BitFontEditor(BitFontEditorMessage::SwapChars))
            ));
            edit_nodes.push(menu::item!(
                fl!("cmd-bitfont-duplicate_line-menu"),
                wrap(Message::BitFontEditor(BitFontEditorMessage::DuplicateLine))
            ));
            edit_nodes.push(menu::separator!());
            edit_nodes.push(menu::item!(
                fl!("menu-set-font-size"),
                wrap(Message::BitFontEditor(BitFontEditorMessage::ShowFontSizeDialog))
            ));
        }

        if edit_mode == crate::ui::EditMode::CharFont {
            edit_nodes.push(menu::separator!());
            edit_nodes.push(menu::item!(
                fl!("menu-add_fonts"),
                wrap(Message::CharFontEditor(CharFontEditorMessage::OpenAddFontDialog))
            ));
            edit_nodes.push(menu::item!(
                fl!("tdf-dialog-edit-settings-title"),
                wrap(Message::CharFontEditor(CharFontEditorMessage::OpenEditSettingsDialog))
            ));
            edit_nodes.push(menu::separator!());
            edit_nodes.push(menu::item!(
                fl!("tdf-editor-clone_button"),
                wrap(Message::CharFontEditor(CharFontEditorMessage::CloneFont))
            ));
            edit_nodes.push(menu::item!(
                fl!("menu-delete"),
                wrap(Message::CharFontEditor(CharFontEditorMessage::DeleteFont))
            ));
            edit_nodes.push(menu::separator!());
            edit_nodes.push(menu::item!("Move Up", wrap(Message::CharFontEditor(CharFontEditorMessage::MoveFontUp))));
            edit_nodes.push(menu::item!("Move Down", wrap(Message::CharFontEditor(CharFontEditorMessage::MoveFontDown))));
            edit_nodes.push(menu::separator!());
            edit_nodes.push(menu::item!(
                fl!("tdf-editor-clear_char_button"),
                wrap(Message::CharFontEditor(CharFontEditorMessage::ClearChar))
            ));
        }

        if edit_mode == crate::ui::EditMode::Animation {
            edit_nodes.push(menu::separator!());
            edit_nodes.push(menu::item!(
                "Play/Pause",
                wrap(Message::AnimationEditor(AnimationEditorMessage::TogglePlayback))
            ));
            edit_nodes.push(menu::item!("Stop", wrap(Message::AnimationEditor(AnimationEditorMessage::Stop))));
            edit_nodes.push(menu::item!("Restart", wrap(Message::AnimationEditor(AnimationEditorMessage::Restart))));
            edit_nodes.push(menu::separator!());
            edit_nodes.push(menu::item!(
                "Previous Frame",
                wrap(Message::AnimationEditor(AnimationEditorMessage::PreviousFrame))
            ));
            edit_nodes.push(menu::item!("Next Frame", wrap(Message::AnimationEditor(AnimationEditorMessage::NextFrame))));
            edit_nodes.push(menu::item!("First Frame", wrap(Message::AnimationEditor(AnimationEditorMessage::FirstFrame))));
            edit_nodes.push(menu::item!("Last Frame", wrap(Message::AnimationEditor(AnimationEditorMessage::LastFrame))));
            edit_nodes.push(menu::separator!());
            edit_nodes.push(menu::item!("Loop", wrap(Message::AnimationEditor(AnimationEditorMessage::ToggleLoop))));
            edit_nodes.push(menu::item!("Toggle Scale", wrap(Message::AnimationEditor(AnimationEditorMessage::ToggleScale))));
            edit_nodes.push(menu::item!(
                "Toggle Log Panel",
                wrap(Message::AnimationEditor(AnimationEditorMessage::ToggleLogPanel))
            ));
            edit_nodes.push(menu::separator!());
            edit_nodes.push(menu::item!("Recompile", wrap(Message::AnimationEditor(AnimationEditorMessage::Recompile))));
        }

        let edit_menu = menu::MenuNode::submenu_with_id(menu::MenuId::from_str("menu.edit"), fl!("menu-edit"), edit_nodes);

        let mut extra_top_level: Vec<menu::MenuNode<WindowManagerMessage>> = Vec::new();

        if edit_mode == crate::ui::EditMode::Ansi {
            let selection_menu = menu::MenuNode::submenu_with_id(
                menu::MenuId::from_str("menu.selection"),
                fl!("menu-selection"),
                vec![
                    menu::item!(
                        fl!("menu-select-all"),
                        wrap(Message::SelectAll),
                        menu::MenuShortcut::cmd(Key::Character("a".into()))
                    ),
                    menu::item!(fl!("menu-select_nothing"), wrap(Message::Deselect)),
                    menu::item!(fl!("menu-inverse_selection"), wrap(Message::AnsiEditor(AnsiEditorMessage::InverseSelection))),
                    menu::separator!(),
                    menu::item!(
                        fl!("menu-flipx"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::FlipX)))
                    ),
                    menu::item!(
                        fl!("menu-flipy"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::FlipY)))
                    ),
                    menu::item!(
                        fl!("menu-crop"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::Crop)))
                    ),
                    menu::separator!(),
                    menu::item!(
                        fl!("menu-justifyleft"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLeft)))
                    ),
                    menu::item!(
                        fl!("menu-justifyright"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyRight)))
                    ),
                    menu::item!(
                        fl!("menu-justifycenter"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyCenter)))
                    ),
                ],
            );
            extra_top_level.push(selection_menu);

            let colors_menu = menu::MenuNode::submenu_with_id(
                menu::MenuId::from_str("menu.colors"),
                fl!("menu-colors"),
                vec![
                    menu::item!(fl!("menu-edit_palette"), wrap(Message::AnsiEditor(AnsiEditorMessage::EditPalette))),
                    menu::separator!(),
                    menu::item!(
                        fl!("menu-next_fg_color"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::NextFgColor)))
                    ),
                    menu::item!(
                        fl!("menu-prev_fg_color"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PrevFgColor)))
                    ),
                    menu::separator!(),
                    menu::item!(
                        fl!("menu-next_bg_color"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::NextBgColor)))
                    ),
                    menu::item!(
                        fl!("menu-prev_bg_color"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PrevBgColor)))
                    ),
                    menu::separator!(),
                    menu::item!(
                        fl!("menu-pick_attribute_under_caret"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PickAttributeUnderCaret)))
                    ),
                    menu::item!(
                        fl!("menu-toggle_color"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::ColorSwitcher(ColorSwitcherMessage::SwapColors)))
                    ),
                    menu::item!(
                        fl!("menu-default_color"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SwitchToDefaultColor)))
                    ),
                ],
            );
            extra_top_level.push(colors_menu);

            let zoom_sub = menu::MenuNode::submenu_with_id(
                menu::MenuId::from_str("view.zoom"),
                fl!("menu-zoom"),
                vec![
                    menu::item!(
                        fl!("menu-zoom_reset"),
                        wrap(Message::ZoomReset),
                        menu::MenuShortcut::cmd(Key::Character("0".into()))
                    ),
                    menu::item!(fl!("menu-zoom_in"), wrap(Message::ZoomIn), menu::MenuShortcut::cmd(Key::Character("=".into()))),
                    menu::item!(
                        fl!("menu-zoom_out"),
                        wrap(Message::ZoomOut),
                        menu::MenuShortcut::cmd(Key::Character("-".into()))
                    ),
                    menu::separator!(),
                    menu::item!("4:1 400%", wrap(Message::SetZoom(4.0))),
                    menu::item!("2:1 200%", wrap(Message::SetZoom(2.0))),
                    menu::item!("1:1 100%", wrap(Message::SetZoom(1.0))),
                    menu::item!("1:2 50%", wrap(Message::SetZoom(0.5))),
                    menu::item!("1:4 25%", wrap(Message::SetZoom(0.25))),
                ],
            );

            // Helper to check if guide matches a specific size
            let guide_is = |w: i32, h: i32| -> Option<bool> {
                ansi_view_state
                    .guide
                    .and_then(|(gw, gh)| if gw as i32 == w && gh as i32 == h { Some(true) } else { None })
            };

            let guide_sub = menu::MenuNode::submenu_with_id(
                menu::MenuId::from_str("view.guides"),
                fl!("menu-guides"),
                vec![
                    menu::check_item!(
                        "Off",
                        if ansi_view_state.guide.is_none() { Some(true) } else { None },
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ClearGuide)))
                    ),
                    menu::separator!(),
                    menu::check_item!(
                        "Smallscale 80x25",
                        guide_is(80, 25),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SetGuide(80, 25))))
                    ),
                    menu::check_item!(
                        "Square 80x40",
                        guide_is(80, 40),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SetGuide(80, 40))))
                    ),
                    menu::check_item!(
                        "Instagram 80x50",
                        guide_is(80, 50),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SetGuide(80, 50))))
                    ),
                    menu::check_item!(
                        "File_ID.DIZ 44x22",
                        guide_is(44, 22),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SetGuide(44, 22))))
                    ),
                    menu::separator!(),
                    menu::check_item!(
                        fl!("menu-toggle_guide"),
                        Some(ansi_view_state.show_guide),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleGuide))),
                    ),
                ],
            );

            // Helper to check if raster matches a specific size
            let raster_is = |w: i32, h: i32| -> Option<bool> {
                ansi_view_state
                    .raster
                    .and_then(|(rw, rh)| if rw as i32 == w && rh as i32 == h { Some(true) } else { None })
            };

            let raster_sub = menu::MenuNode::submenu_with_id(
                menu::MenuId::from_str("view.raster"),
                fl!("menu-raster"),
                vec![
                    menu::check_item!(
                        "Off",
                        if ansi_view_state.raster.is_none() { Some(true) } else { None },
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ClearRaster)))
                    ),
                    menu::separator!(),
                    menu::check_item!(
                        "1x1",
                        raster_is(1, 1),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SetRaster(1, 1))))
                    ),
                    menu::check_item!(
                        "2x2",
                        raster_is(2, 2),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SetRaster(2, 2))))
                    ),
                    menu::check_item!(
                        "4x2",
                        raster_is(4, 2),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SetRaster(4, 2))))
                    ),
                    menu::check_item!(
                        "4x4",
                        raster_is(4, 4),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SetRaster(4, 4))))
                    ),
                    menu::check_item!(
                        "8x2",
                        raster_is(8, 2),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SetRaster(8, 2))))
                    ),
                    menu::check_item!(
                        "8x4",
                        raster_is(8, 4),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SetRaster(8, 4))))
                    ),
                    menu::check_item!(
                        "8x8",
                        raster_is(8, 8),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SetRaster(8, 8))))
                    ),
                    menu::check_item!(
                        "16x4",
                        raster_is(16, 4),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SetRaster(16, 4))))
                    ),
                    menu::check_item!(
                        "16x8",
                        raster_is(16, 8),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SetRaster(16, 8))))
                    ),
                    menu::check_item!(
                        "16x16",
                        raster_is(16, 16),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SetRaster(16, 16))))
                    ),
                    menu::separator!(),
                    menu::check_item!(
                        fl!("menu-toggle_raster"),
                        Some(ansi_view_state.show_raster),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleRaster))),
                    ),
                ],
            );

            let mut view_nodes = vec![
                zoom_sub,
                menu::separator!(),
                guide_sub,
                raster_sub,
                menu::check_item!(
                    fl!("menu-show_layer_borders"),
                    Some(ansi_view_state.show_layer_borders),
                    wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleLayerBorders))),
                ),
                menu::check_item!(
                    fl!("menu-show_line_numbers"),
                    Some(ansi_view_state.show_line_numbers),
                    wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleLineNumbers))),
                ),
                menu::separator!(),
                menu::item!(
                    fl!("menu-toggle_fullscreen"),
                    wrap(Message::ToggleFullscreen),
                    menu::MenuShortcut::new(icy_ui::keyboard::Modifiers::empty(), Key::Named(Named::F11))
                ),
            ];

            if is_connected {
                view_nodes.push(menu::separator!());
                view_nodes.push(menu::item!(fl!("menu-toggle-chat"), wrap(Message::ToggleChatPanel)));
            }

            view_nodes.push(menu::separator!());
            view_nodes.push(menu::item!(
                fl!("menu-reference-image"),
                wrap(Message::AnsiEditor(AnsiEditorMessage::ShowReferenceImageDialog))
            ));
            view_nodes.push(menu::item!(
                fl!("menu-toggle-reference-image"),
                wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleReferenceImage)))
            ));

            let view_menu = menu::MenuNode::submenu_with_id(menu::MenuId::from_str("menu.view"), fl!("menu-view"), view_nodes);
            extra_top_level.push(view_menu);

            let plugins_menu = menu::MenuNode::submenu_with_id(
                menu::MenuId::from_str("menu.plugins"),
                fl!("menu-plugins"),
                Self::build_plugins_submenu(&plugins, wrap),
            );
            extra_top_level.push(plugins_menu);
        }

        if edit_mode == crate::ui::EditMode::BitFont {
            let selection_menu = menu::MenuNode::submenu_with_id(
                menu::MenuId::from_str("menu.selection"),
                fl!("menu-selection"),
                vec![
                    menu::item!(fl!("menu-select-all"), wrap(Message::BitFontEditor(BitFontEditorMessage::SelectAll))),
                    menu::item!(fl!("menu-select_nothing"), wrap(Message::BitFontEditor(BitFontEditorMessage::ClearSelection))),
                    menu::separator!(),
                    menu::item!(fl!("cmd-bitfont-clear-menu"), wrap(Message::BitFontEditor(BitFontEditorMessage::Clear))),
                    menu::item!(fl!("cmd-bitfont-fill-menu"), wrap(Message::BitFontEditor(BitFontEditorMessage::FillSelection))),
                    menu::item!(fl!("cmd-bitfont-inverse-menu"), wrap(Message::BitFontEditor(BitFontEditorMessage::Inverse))),
                    menu::separator!(),
                    menu::item!(fl!("cmd-bitfont-flip_x-menu"), wrap(Message::BitFontEditor(BitFontEditorMessage::FlipX))),
                    menu::item!(fl!("cmd-bitfont-flip_y-menu"), wrap(Message::BitFontEditor(BitFontEditorMessage::FlipY))),
                ],
            );
            extra_top_level.push(selection_menu);

            let view_menu = menu::MenuNode::submenu_with_id(
                menu::MenuId::from_str("menu.view"),
                fl!("menu-view"),
                vec![
                    menu::item!(
                        fl!("cmd-bitfont-toggle_letter_spacing-menu"),
                        wrap(Message::BitFontEditor(BitFontEditorMessage::ToggleLetterSpacing))
                    ),
                    menu::item!(
                        fl!("cmd-bitfont-show_preview-menu"),
                        wrap(Message::BitFontEditor(BitFontEditorMessage::ShowPreview))
                    ),
                    menu::separator!(),
                    menu::item!(
                        fl!("menu-toggle_fullscreen"),
                        wrap(Message::ToggleFullscreen),
                        menu::MenuShortcut::new(icy_ui::keyboard::Modifiers::empty(), Key::Named(Named::F11))
                    ),
                ],
            );
            extra_top_level.push(view_menu);
        }

        if edit_mode == crate::ui::EditMode::CharFont {
            let selection_menu = menu::MenuNode::submenu_with_id(
                menu::MenuId::from_str("menu.selection"),
                fl!("menu-selection"),
                vec![
                    menu::item!(
                        "Select Char at Cursor",
                        wrap(Message::CharFontEditor(CharFontEditorMessage::SelectCharAtCursor))
                    ),
                    menu::item!(
                        fl!("menu-select_nothing"),
                        wrap(Message::CharFontEditor(CharFontEditorMessage::ClearCharsetSelection))
                    ),
                ],
            );
            extra_top_level.push(selection_menu);

            let colors_menu = menu::MenuNode::submenu_with_id(
                menu::MenuId::from_str("menu.colors"),
                fl!("menu-colors"),
                vec![
                    menu::item!(
                        fl!("menu-next_fg_color"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::NextFgColor)))
                    ),
                    menu::item!(
                        fl!("menu-prev_fg_color"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PrevFgColor)))
                    ),
                    menu::separator!(),
                    menu::item!(
                        fl!("menu-next_bg_color"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::NextBgColor)))
                    ),
                    menu::item!(
                        fl!("menu-prev_bg_color"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PrevBgColor)))
                    ),
                    menu::separator!(),
                    menu::item!(
                        fl!("menu-default_color"),
                        wrap(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SwitchToDefaultColor)))
                    ),
                ],
            );
            extra_top_level.push(colors_menu);

            let fonts_menu = menu::MenuNode::submenu_with_id(
                menu::MenuId::from_str("menu.fonts"),
                fl!("menu-fonts"),
                vec![
                    menu::item!(fl!("menu-add_fonts"), wrap(Message::CharFontEditor(CharFontEditorMessage::OpenAddFontDialog))),
                    menu::item!(
                        fl!("tdf-dialog-edit-settings-title"),
                        wrap(Message::CharFontEditor(CharFontEditorMessage::OpenEditSettingsDialog))
                    ),
                    menu::separator!(),
                    menu::item!(fl!("tdf-editor-clone_button"), wrap(Message::CharFontEditor(CharFontEditorMessage::CloneFont))),
                    menu::item!(fl!("menu-delete"), wrap(Message::CharFontEditor(CharFontEditorMessage::DeleteFont))),
                    menu::separator!(),
                    menu::item!("Move Up", wrap(Message::CharFontEditor(CharFontEditorMessage::MoveFontUp))),
                    menu::item!("Move Down", wrap(Message::CharFontEditor(CharFontEditorMessage::MoveFontDown))),
                ],
            );
            extra_top_level.push(fonts_menu);

            let view_menu = menu::MenuNode::submenu_with_id(
                menu::MenuId::from_str("menu.view"),
                fl!("menu-view"),
                vec![menu::item!(
                    fl!("menu-toggle_fullscreen"),
                    wrap(Message::ToggleFullscreen),
                    menu::MenuShortcut::new(icy_ui::keyboard::Modifiers::empty(), Key::Named(Named::F11))
                )],
            );
            extra_top_level.push(view_menu);
        }

        if edit_mode == crate::ui::EditMode::Animation {
            let view_menu = menu::MenuNode::submenu_with_id(
                menu::MenuId::from_str("menu.view"),
                fl!("menu-view"),
                vec![menu::item!(
                    fl!("menu-toggle_fullscreen"),
                    wrap(Message::ToggleFullscreen),
                    menu::MenuShortcut::new(icy_ui::keyboard::Modifiers::empty(), Key::Named(Named::F11))
                )],
            );
            extra_top_level.push(view_menu);
        }

        // View menu
        // Help menu
        let help_menu = menu::MenuNode::submenu_with_id(
            menu::MenuId::from_str("menu.help"),
            fl!("menu-help"),
            vec![
                menu::item!(fl!("menu-discuss"), wrap(Message::OpenDiscussions)),
                menu::item!(fl!("menu-open_log_file"), wrap(Message::OpenLogFile)),
                menu::item!(fl!("menu-report-bug"), wrap(Message::ReportBug)),
                menu::separator!(),
                menu::about!(fl!("menu-about"), wrap(Message::ShowAbout)),
            ],
        );

        let mut top_level = vec![file_menu, edit_menu];
        top_level.extend(extra_top_level);
        top_level.push(help_menu);

        Some(menu::AppMenu::new(top_level))
    }
}
