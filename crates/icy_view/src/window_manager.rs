use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use parking_lot::Mutex;

use iced::{keyboard, widget::space, window, Element, Event, Size, Subscription, Task, Theme, Vector};

use crate::ui::{MainWindow, Message};
use crate::Options;
use icy_engine_gui::command_handler;
use icy_engine_gui::commands::{cmd, create_common_commands};
use icy_engine_gui::{find_next_window_id, focus_window_by_id, format_window_title, handle_window_closed};

fn load_window_icon(png_bytes: &[u8]) -> Result<iced::window::Icon, Box<dyn std::error::Error>> {
    let img = image::load_from_memory(png_bytes)?;
    let rgba = img.to_rgba8();
    let w = img.width();
    let h = img.height();
    Ok(iced::window::icon::from_rgba(rgba.into_raw(), w, h)?)
}

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
    options: Arc<Mutex<Options>>,
    initial_path: Option<PathBuf>,
    auto_scroll: bool,
    bps: Option<u32>,
    commands: WindowCommands,
}

#[derive(Clone)]
pub enum WindowManagerMessage {
    OpenWindow,
    CloseWindow(window::Id),
    WindowOpened(window::Id),
    FocusWindow(usize),
    FocusNext,
    FocusPrevious,
    WindowClosed(window::Id),
    WindowMessage(window::Id, Message),
    _TitleChanged(window::Id, String),
    Event(window::Id, iced::Event),
    AnimationTick,
}

impl std::fmt::Debug for WindowManagerMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenWindow => write!(f, "OpenWindow"),
            Self::CloseWindow(id) => f.debug_tuple("CloseWindow").field(id).finish(),
            Self::WindowOpened(id) => f.debug_tuple("WindowOpened").field(id).finish(),
            Self::FocusWindow(idx) => f.debug_tuple("FocusWindow").field(idx).finish(),
            Self::FocusNext => write!(f, "FocusNext"),
            Self::FocusPrevious => write!(f, "FocusPrevious"),
            Self::WindowClosed(id) => f.debug_tuple("WindowClosed").field(id).finish(),
            Self::WindowMessage(id, _) => f.debug_tuple("WindowMessage").field(id).field(&"...").finish(),
            Self::_TitleChanged(id, title) => f.debug_tuple("TitleChanged").field(id).field(title).finish(),
            Self::Event(id, _) => f.debug_tuple("Event").field(id).field(&"...").finish(),
            Self::AnimationTick => write!(f, "AnimationTick"),
        }
    }
}

const DEFAULT_SIZE: Size = Size::new(1337.0, 839.0);

impl WindowManager {
    pub fn new(auto_scroll: bool, bps: Option<u32>) -> (Self, Task<WindowManagerMessage>) {
        let window_icon = load_window_icon(include_bytes!("../build/linux/256x256.png")).ok();
        let settings = window::Settings {
            size: DEFAULT_SIZE,
            icon: window_icon,
            ..window::Settings::default()
        };
        let (_, open) = window::open(settings);

        let options = Options::load_options();

        (
            Self {
                windows: BTreeMap::new(),
                options: Arc::new(Mutex::new(options)),
                initial_path: None,
                auto_scroll,
                bps,
                commands: WindowCommands::new(),
            },
            open.map(WindowManagerMessage::WindowOpened),
        )
    }

    pub fn with_path(path: PathBuf, auto_scroll: bool, bps: Option<u32>) -> (Self, Task<WindowManagerMessage>) {
        let (mut manager, task) = Self::new(auto_scroll, bps);
        manager.initial_path = Some(path);
        (manager, task)
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
                        let window_icon = load_window_icon(include_bytes!("../build/linux/256x256.png")).ok();
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
                let (window, initial_message) = MainWindow::new(
                    find_next_window_id(&self.windows),
                    self.initial_path.take(),
                    self.options.clone(),
                    self.auto_scroll,
                    self.bps,
                );

                self.windows.insert(id, window);

                // If there's an initial message (e.g., to load a file preview), send it
                if let Some(msg) = initial_message {
                    Task::done(WindowManagerMessage::WindowMessage(id, msg))
                } else {
                    Task::none()
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
                // Handle keyboard commands
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

            WindowManagerMessage::_TitleChanged(id, title) => {
                if let Some(window) = self.windows.get_mut(&id) {
                    window.title = title;
                }
                Task::none()
            }

            WindowManagerMessage::FocusWindow(target_id) => focus_window_by_id(&self.windows, target_id),

            WindowManagerMessage::AnimationTick => {
                // Forward animation tick to all windows that need it
                let mut tasks = Vec::new();
                for (window_id, window) in self.windows.iter_mut() {
                    if window.needs_animation() {
                        let id = *window_id;
                        tasks.push(Task::done(WindowManagerMessage::WindowMessage(id, Message::AnimationTick)));
                    }
                }
                Task::batch(tasks)
            }
        }
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
        // Check if any window needs animation

        let subs = vec![
            window::close_events().map(WindowManagerMessage::WindowClosed),
            iced::event::listen_with(|event, _status, window_id| {
                match &event {
                    // Window focus events
                    Event::Window(window::Event::Focused) | Event::Window(window::Event::Unfocused) => Some(WindowManagerMessage::Event(window_id, event)),
                    // Mouse events - pass through for tile grid hover/click handling
                    Event::Mouse(iced::mouse::Event::WheelScrolled { .. }) => Some(WindowManagerMessage::Event(window_id, event)),
                    Event::Mouse(iced::mouse::Event::CursorMoved { .. }) => Some(WindowManagerMessage::Event(window_id, event)),
                    Event::Mouse(iced::mouse::Event::CursorLeft) => Some(WindowManagerMessage::Event(window_id, event)),
                    Event::Mouse(iced::mouse::Event::ButtonPressed { .. }) => Some(WindowManagerMessage::Event(window_id, event)),
                    // Skip other mouse events
                    Event::Mouse(_) => None,

                    Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                        // Track modifiers globally (replaces ModifiersChanged event)
                        icy_engine_gui::set_global_modifiers(modifiers.control(), modifiers.alt(), modifiers.shift(), modifiers.command());
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
                    Event::Keyboard(_) => Some(WindowManagerMessage::Event(window_id, event)),
                    Event::Touch(_) => None,

                    _ => None,
                }
            }),
            // important for updating slow blinking and smooth scroll animations
            iced::time::every(std::time::Duration::from_millis(icy_engine_gui::ANIMATION_TICK_MS)).map(|_| WindowManagerMessage::AnimationTick),
        ];

        Subscription::batch(subs)
    }
}
