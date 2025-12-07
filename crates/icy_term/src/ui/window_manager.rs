use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use icy_engine_gui::command_handler;
use icy_engine_gui::commands::{cmd, create_common_commands};
use icy_engine_gui::music::music::SoundThread;
use parking_lot::Mutex;

use iced::{
    Element, Event, Size, Subscription, Task, Theme, Vector,
    advanced::graphics::core::keyboard,
    widget::{operation, space},
    window,
};

use crate::{
    AddressBook, McpHandler, Options, load_window_icon,
    ui::{MainWindow, MainWindowMode, Message},
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
    addresses: Arc<Mutex<AddressBook>>,
    options: Arc<Mutex<Options>>,
    temp_options: Arc<Mutex<Options>>,
    url: Option<String>,
    pub script_to_run: Option<PathBuf>,
    mcp_rx: McpHandler,

    // sound thread
    pub sound_thread: Arc<Mutex<SoundThread>>,
    commands: WindowCommands,
}

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
    UpdateBuffers,
    ViewportTick,
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
        let mut mode = MainWindowMode::ShowTerminal;

        let addresses = match AddressBook::load_phone_book() {
            Ok(addresses) => addresses,
            Err(err) => {
                unsafe { crate::PHONE_LOCK = true };
                mode = MainWindowMode::ShowErrorDialog(
                    i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "error-address-book-load-title"),
                    i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "error-address-book-load-secondary"),
                    format!("{}", err),
                    Box::new(MainWindowMode::ShowTerminal),
                );
                AddressBook::default()
            }
        };
        let options = Arc::new(Mutex::new(options));
        let temp_options = options.clone();
        (
            Self {
                windows: BTreeMap::new(),
                sound_thread,
                mode,
                addresses: Arc::new(Mutex::new(addresses)),
                options,
                temp_options,
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

    pub fn title(&self, window: window::Id) -> String {
        let zoom_info = self.windows.get(&window).map(|w| w.get_zoom_info_string()).unwrap_or_default();

        if self.windows.iter().count() == 1 {
            return self
                .windows
                .get(&window)
                .map(|window| format!("{} {}", window.title, zoom_info))
                .unwrap_or_default();
        }

        self.windows
            .get(&window)
            .map(|window| {
                if window.id < 10 {
                    format!("{} {} - ⌘{}", window.title, zoom_info, window.id)
                } else {
                    format!("{} {}", window.title, zoom_info)
                }
            })
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
                    self.find_next_id(),
                    self.mode.clone(),
                    self.sound_thread.clone(),
                    self.addresses.clone(),
                    self.options.clone(),
                    self.temp_options.clone(),
                );

                if let Some(mcp_rx) = self.mcp_rx.take() {
                    window.mcp_rx = Some(mcp_rx);
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
            WindowManagerMessage::WindowClosed(id) => {
                self.windows.remove(&id);
                if self.windows.is_empty() { iced::exit() } else { Task::none() }
            }

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
                if let Some(window) = self.windows.get(&window_id) {
                    if let Some(msg) = window.handle_event(&event) {
                        return Task::done(WindowManagerMessage::WindowMessage(window_id, msg));
                    }
                }
                Task::none()
            }

            WindowManagerMessage::UpdateBuffers => {
                let mut tasks = vec![];
                for (id, window) in self.windows.iter_mut() {
                    let mcp_commands = window.get_mcp_commands();
                    for cmd in mcp_commands {
                        tasks.push(Task::done(WindowManagerMessage::WindowMessage(id.clone(), Message::McpCommand(Arc::new(cmd)))));
                    }

                    let terminal_events = window.get_terminal_commands();
                    for cmd in terminal_events {
                        tasks.push(Task::done(WindowManagerMessage::WindowMessage(id.clone(), Message::TerminalEvent(cmd))));
                    }
                }
                if tasks.is_empty() {
                    return Task::none();
                }
                Task::batch(tasks)
            }

            WindowManagerMessage::TitleChanged(id, title) => {
                if let Some(window) = self.windows.get_mut(&id) {
                    window.title = title.clone();
                }
                Task::none()
            }

            WindowManagerMessage::FocusWindow(target_id) => {
                // Find the window with the target ID number
                for (window_id, window) in self.windows.iter() {
                    if window.id == target_id {
                        return window::gain_focus(*window_id);
                    }
                }
                Task::none()
            }

            WindowManagerMessage::ViewportTick => {
                // Update viewport and scrollbar animations for all windows
                for window in self.windows.values_mut() {
                    window.terminal_window.terminal.update_animations();
                }
                Task::none()
            }
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
        let mut subs = vec![
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
                    // Keyboard events need special handling
                    Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                        // Alt+Number to focus window (keep this special case)
                        if (modifiers.alt() || modifiers.command()) && !modifiers.shift() && !modifiers.control() {
                            if let keyboard::Key::Character(s) = &key {
                                if let Some(digit) = s.chars().next() {
                                    if digit.is_ascii_digit() {
                                        let target_id = digit.to_digit(10).unwrap() as usize;
                                        // Special case: Alt+0 focuses window 10
                                        let target_id = if target_id == 0 { 10 } else { target_id };
                                        return Some(WindowManagerMessage::FocusWindow(target_id));
                                    }
                                }
                            }
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
            iced::time::every(std::time::Duration::from_millis(160)).map(|_| WindowManagerMessage::UpdateBuffers),
        ];

        // Only subscribe to ViewportTick if any window needs animation (viewport or scrollbar)
        let any_animating = self.windows.values().any(|w| w.terminal_window.terminal.needs_animation());
        if any_animating {
            subs.push(iced::time::every(std::time::Duration::from_millis(16)).map(|_| WindowManagerMessage::ViewportTick));
        }

        iced::Subscription::batch(subs)
    }

    fn find_next_id(&self) -> usize {
        let used_ids: std::collections::HashSet<usize> = self.windows.values().map(|w| w.id).collect();

        // Start from 1 (or 0 if you prefer 0-based)
        for id in 1.. {
            if !used_ids.contains(&id) {
                return id;
            }
        }

        // This should never be reached, but as a fallback
        1
    }
}
