use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use icy_engine_gui::command_handler;
use icy_engine_gui::commands::{cmd, create_common_commands};
use icy_engine_gui::error_dialog;
use icy_engine_gui::music::music::SoundThread;
use icy_engine_gui::{find_next_window_id, focus_window_by_id, format_window_title, handle_window_closed};
use parking_lot::Mutex;

use iced::{
    advanced::graphics::core::keyboard,
    widget::{operation, space},
    window, Element, Event, Size, Subscription, Task, Theme, Vector,
};

use crate::{
    load_window_icon,
    ui::{MainWindow, MainWindowMode, Message},
    AddressBook, McpHandler, Options,
};

// Generate the WindowCommands struct with handle() method
// Note: Zoom/Fullscreen commands are handled at MainWindow level, not here
command_handler!(WindowCommands, create_common_commands(), _window_id: window::Id => WindowManagerMessage {
    cmd::WINDOW_NEW => WindowManagerMessage::OpenWindow,
    cmd::WINDOW_CLOSE => WindowManagerMessage::CloseWindow(_window_id),
    cmd::FILE_CLOSE => WindowManagerMessage::CloseWindow(_window_id),
    cmd::FOCUS_NEXT => WindowManagerMessage::FocusNext,
    cmd::FOCUS_PREVIOUS => WindowManagerMessage::FocusPrevious,
});

pub struct WindowManager {
    windows: BTreeMap<window::Id, MainWindow>,

    mode: MainWindowMode,
    startup_error: Option<(String, String, String)>, // (title, secondary, message)
    addresses: Arc<Mutex<AddressBook>>,
    options: Arc<Mutex<Options>>,
    url: Option<String>,
    pub script_to_run: Option<PathBuf>,
    mcp_rx: McpHandler,

    // sound thread
    pub sound_thread: Arc<Mutex<SoundThread>>,
    commands: WindowCommands,
}

use crate::mcp::McpCommand;
use crate::terminal::terminal_thread::TerminalEvent;

#[derive(Debug, Clone)]
pub enum WindowManagerMessage {
    OpenWindow,
    CloseWindow(window::Id),
    WindowOpened(window::Id),
    FocusWindow(usize),
    FocusNext,
    FocusPrevious,
    WindowClosed(window::Id),
    WindowMessage(window::Id, Message),
    TitleChanged(window::Id, String),
    Event(window::Id, iced::Event),
    /// Terminal event from async subscription (window_id, event)
    TerminalEvent(usize, TerminalEvent),
    /// MCP command from async subscription
    McpCommand(Arc<McpCommand>),
    AnimationTick,
}

const DEFAULT_SIZE: Size = Size::new(853.0, 597.0);

impl WindowManager {
    pub fn new(mcp_rx: McpHandler) -> (Self, Task<WindowManagerMessage>) {
        let window_icon = load_window_icon(include_bytes!("../../build/linux/256x256.png")).ok();
        let settings = window::Settings {
            size: DEFAULT_SIZE,
            icon: window_icon,
            ..window::Settings::default()
        };
        let (_, open) = window::open(settings);

        let options = match Options::load_options() {
            Ok(options) => options,
            Err(e) => {
                log::error!("Error loading options file: {e}");
                Options::default()
            }
        };

        // Create a single sound thread to be shared by all windows
        let sound_thread = Arc::new(Mutex::new(SoundThread::new()));
        let mode = MainWindowMode::ShowTerminal;
        let mut startup_error = None;

        let addresses = match AddressBook::load_phone_book() {
            Ok(addresses) => addresses,
            Err(err) => {
                unsafe { crate::PHONE_LOCK = true };
                startup_error = Some((
                    i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "error-address-book-load-title"),
                    i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "error-address-book-load-secondary"),
                    format!("{}", err),
                ));
                AddressBook::default()
            }
        };
        let options = Arc::new(Mutex::new(options));
        (
            Self {
                windows: BTreeMap::new(),
                sound_thread,
                mode,
                startup_error,
                addresses: Arc::new(Mutex::new(addresses)),
                options,
                url: None,
                script_to_run: None,
                mcp_rx,
                commands: WindowCommands::new(),
            },
            open.map(WindowManagerMessage::WindowOpened),
        )
    }

    pub fn with_url(mcp_rx: McpHandler, url: String) -> (WindowManager, Task<WindowManagerMessage>) {
        let mut manager = Self::new(mcp_rx);
        manager.0.url = Some(url);
        manager
    }

    pub fn with_script(mcp_rx: McpHandler, script: PathBuf) -> (WindowManager, Task<WindowManagerMessage>) {
        let mut manager = Self::new(mcp_rx);
        manager.0.script_to_run = Some(script);
        manager
    }

    pub fn title(&self, window_id: window::Id) -> String {
        self.windows
            .get(&window_id)
            .map(|w| format_window_title(w, self.windows.len()))
            .unwrap_or_default()
    }

    pub fn update(&mut self, message: WindowManagerMessage) -> Task<WindowManagerMessage> {
        match message {
            WindowManagerMessage::OpenWindow => {
                let Some(last_window) = self.windows.keys().last() else {
                    return Task::none();
                };

                window::position(*last_window)
                    .then(|last_position| {
                        let position = last_position.map_or(window::Position::Default, |last_position| {
                            window::Position::Specific(last_position + Vector::new(20.0, 20.0))
                        });
                        let window_icon = load_window_icon(include_bytes!("../../build/linux/256x256.png")).ok();
                        let settings = window::Settings {
                            position,
                            icon: window_icon,
                            size: DEFAULT_SIZE,
                            ..window::Settings::default()
                        };

                        let (_, open) = window::open(settings);

                        open
                    })
                    .map(WindowManagerMessage::WindowOpened)
            }
            WindowManagerMessage::CloseWindow(id) => window::close(id),
            WindowManagerMessage::FocusNext => iced::widget::operation::focus_next(),
            WindowManagerMessage::FocusPrevious => iced::widget::operation::focus_previous(),
            WindowManagerMessage::WindowOpened(id) => {
                let mut window: MainWindow = MainWindow::new(
                    find_next_window_id(&self.windows),
                    self.mode.clone(),
                    self.sound_thread.clone(),
                    self.addresses.clone(),
                    self.options.clone(),
                );

                // Register MCP receiver for async subscription (only once, for first window)
                if let Some(mcp_rx) = self.mcp_rx.take() {
                    super::terminal_subscription::register_mcp_receiver(mcp_rx);
                }

                // Show startup error if any
                if let Some((title, secondary, message)) = self.startup_error.take() {
                    let mut dialog = error_dialog(title, message, |_| Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)));
                    dialog.dialog = dialog.dialog.secondary_message(secondary);
                    window.dialogs.push(dialog);
                }

                // reset mode to default after opening window
                self.mode = MainWindowMode::ShowTerminal;
                let focus_input: Task<()> = operation::focus(format!("input-{id}"));

                self.windows.insert(id, window);

                // Handle startup URL connection
                let url_task = if let Some(url) = self.url.take() {
                    if let Ok(connection_info) = crate::ConnectionInformation::parse(&url) {
                        Some(Task::done(WindowManagerMessage::WindowMessage(id, Message::Connect(connection_info.into()))))
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Handle startup script
                let script_task = if let Some(script) = self.script_to_run.take() {
                    Some(Task::done(WindowManagerMessage::WindowMessage(id, Message::RunScript(script))))
                } else {
                    None
                };

                // Combine tasks
                let focus_task = focus_input.map(move |_: ()| WindowManagerMessage::WindowOpened(id));
                match (url_task, script_task) {
                    (Some(url), Some(script)) => Task::batch([url, script, focus_task]),
                    (Some(url), None) => Task::batch([url, focus_task]),
                    (None, Some(script)) => Task::batch([script, focus_task]),
                    (None, None) => focus_task,
                }
            }
            WindowManagerMessage::WindowClosed(id) => handle_window_closed(&mut self.windows, id),

            WindowManagerMessage::WindowMessage(id, msg) => {
                if let Some(window) = self.windows.get_mut(&id) {
                    return window.update(msg).map(move |msg| WindowManagerMessage::WindowMessage(id, msg));
                }
                Task::none()
            }

            WindowManagerMessage::Event(window_id, event) => {
                // Handle keyboard commands at window manager level first
                if let Some(msg) = self.commands.handle(&event, window_id) {
                    return Task::done(msg);
                }

                // Pass event to window for other handling
                if let Some(window) = self.windows.get_mut(&window_id) {
                    let (msg_opt, task) = window.handle_event(&event);
                    let mut tasks = vec![task.map(move |m| WindowManagerMessage::WindowMessage(window_id, m))];
                    if let Some(msg) = msg_opt {
                        tasks.push(Task::done(WindowManagerMessage::WindowMessage(window_id, msg)));
                    }
                    return Task::batch(tasks);
                }
                Task::none()
            }

            WindowManagerMessage::TerminalEvent(window_id, event) => {
                // Find the window by its id (usize) and forward the terminal event
                for (id, window) in self.windows.iter() {
                    if window.id == window_id {
                        return Task::done(WindowManagerMessage::WindowMessage(id.clone(), Message::TerminalEvent(event)));
                    }
                }
                Task::none()
            }

            WindowManagerMessage::McpCommand(cmd) => {
                // Forward MCP command to first window
                if let Some((id, _)) = self.windows.iter().next() {
                    return Task::done(WindowManagerMessage::WindowMessage(id.clone(), Message::McpCommand(cmd)));
                }
                Task::none()
            }

            WindowManagerMessage::TitleChanged(id, title) => {
                if let Some(window) = self.windows.get_mut(&id) {
                    window.title = title.clone();
                }
                Task::none()
            }

            WindowManagerMessage::FocusWindow(target_id) => focus_window_by_id(&self.windows, target_id),

            WindowManagerMessage::AnimationTick => Task::none(),
        }
    }

    pub fn view(&self, window_id: window::Id) -> Element<'_, WindowManagerMessage> {
        let id = window_id.clone();
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
        let mut subs: Vec<Subscription<WindowManagerMessage>> = vec![
            window::close_events().map(WindowManagerMessage::WindowClosed),
            iced::event::listen_with(|event, _status, window_id| {
                // Only forward events that are actually needed - skip mouse move events
                // as they are handled directly by the shader's update() method
                match &event {
                    // Window focus events are needed
                    Event::Window(window::Event::Focused) | Event::Window(window::Event::Unfocused) => Some(WindowManagerMessage::Event(window_id, event)),
                    // Mouse events: only CursorLeft and WheelScrolled are needed
                    Event::Mouse(iced::mouse::Event::CursorLeft) | Event::Mouse(iced::mouse::Event::WheelScrolled { .. }) => {
                        Some(WindowManagerMessage::Event(window_id, event))
                    }
                    // Skip other mouse events (CursorMoved, ButtonPressed, etc.) - handled by shader
                    Event::Mouse(_) => None,

                    Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) => {
                        let ctrl = mods.control();
                        let alt = mods.alt();
                        let shift = mods.shift();
                        let command = mods.command(); // Cmd on macOS, Ctrl on Windows/Linux
                                                      // Also store globally for cross-widget access
                        icy_engine_gui::set_global_modifiers(ctrl, alt, shift, command);
                        None
                    }
                    // Keyboard events need special handling
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

                        // Forward all key events - command matching happens in update()
                        Some(WindowManagerMessage::Event(window_id, event))
                    }
                    // Forward other keyboard events (KeyReleased, ModifiersChanged)
                    Event::Keyboard(_) => Some(WindowManagerMessage::Event(window_id, event)),
                    // Skip touch events
                    Event::Touch(_) => None,
                    // Skip other events (InputMethod, etc.)
                    _ => None,
                }
            }),
        ];

        // Add async terminal subscriptions for each window
        for window in self.windows.values() {
            subs.push(super::terminal_subscription::terminal_events(window.id).map(|(window_id, event)| WindowManagerMessage::TerminalEvent(window_id, event)));
        }

        // Add MCP subscription if MCP is enabled (single global subscription)
        if super::terminal_subscription::has_mcp_receiver() || self.mcp_rx.is_some() {
            subs.push(super::terminal_subscription::mcp_events().map(WindowManagerMessage::McpCommand));
        }

        iced::Subscription::batch(subs)
    }
}
