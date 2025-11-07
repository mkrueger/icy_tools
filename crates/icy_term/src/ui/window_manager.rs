use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use iced::{
    Element, Event, Size, Subscription, Task, Theme, Vector,
    advanced::graphics::core::keyboard,
    widget::{operation, space},
    window,
};

use crate::{
    AddressBook, McpHandler, Options, load_window_icon,
    terminal_thread::TerminalEvent,
    ui::{MainWindow, MainWindowMode, Message},
    util::SoundThread,
};

pub struct WindowManager {
    windows: BTreeMap<window::Id, MainWindow>,

    mode: MainWindowMode,
    addresses: Arc<Mutex<AddressBook>>,
    options: Arc<Mutex<Options>>,
    temp_options: Arc<Mutex<Options>>,
    url: Option<String>,
    mcp_rx: McpHandler,

    // sound thread
    pub sound_thread: Arc<Mutex<SoundThread>>,
}

#[derive(Debug, Clone)]
pub enum WindowManagerMessage {
    OpenWindow,
    CloseWindow(window::Id),
    WindowOpened(window::Id),
    FocusWindow(usize),
    WindowClosed(window::Id),
    WindowMessage(window::Id, Message),
    TitleChanged(window::Id, String),
    Event(window::Id, iced::Event),
    UpdateBuffers,
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
                mcp_rx,
            },
            open.map(WindowManagerMessage::WindowOpened),
        )
    }

    pub fn with_url(mcp_rx: McpHandler, url: String) -> (WindowManager, Task<WindowManagerMessage>) {
        let mut manager = Self::new(mcp_rx);
        manager.0.url = Some(url);
        manager
    }

    pub fn title(&self, window: window::Id) -> String {
        if self.windows.iter().count() == 1 {
            return self.windows.get(&window).map(|window| window.title.clone()).unwrap_or_default();
        }

        self.windows
            .get(&window)
            .map(|window| {
                if window.id < 10 {
                    format!("{} - ⌘{}", window.title, window.id)
                } else {
                    window.title.clone()
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

                if let Some(url) = self.url.take() {
                    if let Ok(connection_info) = crate::ConnectionInformation::parse(&url) {
                        return Task::done(WindowManagerMessage::WindowMessage(id, Message::Connect(connection_info.into())));
                    }
                }

                focus_input.map(move |_: ()| WindowManagerMessage::WindowOpened(id))
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
                // Handle the event for the specific window
                if let Some(window) = self.windows.get(&window_id) {
                    if let Some(msg) = window.handle_event(&event) {
                        return Task::done(WindowManagerMessage::WindowMessage(window_id, msg));
                    }
                }
                Task::none()
            }

            WindowManagerMessage::UpdateBuffers => {
                let mut tasks = vec![];
                for (id, _window) in self.windows.iter() {
                    let id = id.clone();
                    tasks.push(Task::done(WindowManagerMessage::WindowMessage(
                        id,
                        Message::TerminalEvent(TerminalEvent::BufferUpdated),
                    )));
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
        let subs = vec![
            window::close_events().map(WindowManagerMessage::WindowClosed),
            iced::event::listen_with(|event, _status, window_id| {
                match &event {
                    Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                        if (modifiers.alt() || modifiers.command()) && !modifiers.shift() && !modifiers.control() {
                            match &key {
                                keyboard::Key::Character(s) => {
                                    if let Some(digit) = s.chars().next() {
                                        if digit.is_ascii_digit() {
                                            let target_id = digit.to_digit(10).unwrap() as usize;
                                            // Special case: Alt+0 focuses window 10
                                            let target_id = if target_id == 0 { 10 } else { target_id };

                                            return Some(WindowManagerMessage::FocusWindow(target_id));
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }

                        if modifiers.shift() {
                            if modifiers.command() {
                                match &key {
                                    keyboard::Key::Character(s) => match s.to_lowercase().as_str() {
                                        "n" => return Some(WindowManagerMessage::OpenWindow),
                                        _ => {}
                                    },
                                    _ => {}
                                }
                            }
                        } else {
                            if modifiers.command() {
                                match &key {
                                    keyboard::Key::Character(s) => match s.to_lowercase().as_str() {
                                        "w" => return Some(WindowManagerMessage::CloseWindow(window_id)),
                                        _ => {}
                                    },
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => { /* Handle other events if necessary */ }
                }

                Some(WindowManagerMessage::Event(window_id, event))
            }),
            iced::time::every(std::time::Duration::from_millis(60)).map(|_| WindowManagerMessage::UpdateBuffers),
        ];
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
