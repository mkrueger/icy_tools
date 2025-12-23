//! Window Manager for icy_draw
//!
//! Manages multiple independent windows, each with its own MainWindow state.
//! Implements VS Code-like "Hot Exit" for session persistence and crash recovery.

use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use parking_lot::RwLock;
use tokio::sync::mpsc as tokio_mpsc;

use iced::{keyboard, widget::space, window, Element, Event, Point, Size, Subscription, Task, Theme, Vector};

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
    Event(window::Id, iced::Event),
    /// Autosave tick (periodic check)
    AutosaveTick,
    /// Debounced session-save tick
    SessionSaveTick,
    /// Animation tick for animation playback
    AnimationTick,
    /// MCP command received from automation server
    McpCommand(Arc<McpCommand>),
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
                    iced::exit()
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

            WindowManagerMessage::Event(window_id, event) => {
                // Handle keyboard commands at window manager level
                if let Some(msg) = self.commands.handle(&event, window_id) {
                    return Task::done(msg);
                }

                // Pass event to window for other handling
                if let Some(window) = self.windows.get_mut(&window_id) {
                    let (msg_opt, task) = window.handle_event(&event);
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

            WindowManagerMessage::FocusNext => iced::widget::operation::focus_next(),

            WindowManagerMessage::FocusPrevious => iced::widget::operation::focus_previous(),

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
            iced::event::listen_with(|event, _status, window_id| {
                match &event {
                    // Window focus events
                    Event::Window(window::Event::Focused) | Event::Window(window::Event::Unfocused) => Some(WindowManagerMessage::Event(window_id, event)),
                    // Mouse events
                    Event::Mouse(iced::mouse::Event::WheelScrolled { .. }) => Some(WindowManagerMessage::Event(window_id, event)),
                    Event::Mouse(iced::mouse::Event::CursorMoved { .. }) => Some(WindowManagerMessage::Event(window_id, event)),
                    Event::Mouse(iced::mouse::Event::ButtonPressed(_)) => Some(WindowManagerMessage::Event(window_id, event)),
                    Event::Mouse(_) => None,
                    // Keyboard events
                    Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) => {
                        icy_engine_gui::set_global_modifiers(mods.control(), mods.alt(), mods.shift(), mods.command());
                        None
                    }
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
            iced::time::every(std::time::Duration::from_secs(1)).map(|_| WindowManagerMessage::AutosaveTick),
        ];

        // Debounced session-save tick (only when something scheduled)
        // Keep session-save ticking even if nothing is scheduled.
        // This avoids edge cases where a deadline is set but the subscription
        // doesn't get activated soon enough due to update timing.
        subs.push(iced::time::every(Duration::from_millis(200)).map(|_| WindowManagerMessage::SessionSaveTick));

        // Animation tick - only active when an animation editor is playing
        let needs_animation = self.windows.values().any(|w| w.needs_animation_tick());
        if needs_animation {
            subs.push(iced::time::every(Duration::from_millis(16)).map(|_| WindowManagerMessage::AnimationTick));
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
}
