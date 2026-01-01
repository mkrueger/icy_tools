use std::collections::BTreeMap;

use clap::Parser;
use icy_ui::{
    advanced::graphics::core::keyboard,
    widget::{operation, space},
    window, Element, Event, Subscription, Task, Theme, Vector,
};

use icy_engine_gui::{handle_window_manager_keyboard_press, KeyboardAction};

use crate::{
    load_window_icon,
    ui::{MainWindow, MainWindowMode, Message},
    Args,
};

pub struct WindowManager {
    windows: BTreeMap<window::Id, MainWindow>,
}

#[derive(Debug, Clone)]
pub enum WindowManagerMessage {
    OpenWindow,
    CloseWindow(window::Id),
    WindowOpened(window::Id),
    FocusWindow(usize),
    /// Focus next widget (Tab)
    FocusNext,
    /// Focus previous widget (Shift+Tab)
    FocusPrevious,
    WindowClosed(window::Id),
    WindowMessage(window::Id, crate::ui::Message),
    Event(window::Id, icy_ui::Event),
    _UpdateBuffers,
}

impl WindowManager {
    pub fn new() -> (Self, Task<WindowManagerMessage>) {
        let window_icon = load_window_icon(include_bytes!("../../build/linux/256x256.png")).ok();
        let settings = window::Settings {
            icon: window_icon,
            ..window::Settings::default()
        };
        let (_, open) = window::open(settings);

        (Self { windows: BTreeMap::new() }, open.map(WindowManagerMessage::WindowOpened))
    }

    pub fn title(&self, _window: window::Id) -> String {
        format!("iCY MAIL {}", *crate::VERSION)
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
                            ..window::Settings::default()
                        };

                        let (_, open) = window::open(settings);

                        open
                    })
                    .map(WindowManagerMessage::WindowOpened)
            }
            WindowManagerMessage::CloseWindow(id) => window::close(id),
            WindowManagerMessage::WindowOpened(id) => {
                let window: MainWindow = MainWindow::new(id.clone(), MainWindowMode::ShowWelcomeScreen);
                self.windows.insert(id, window);

                let focus_input: Task<()> = operation::focus(format!("input-{id}"));

                // Check for command-line file argument only for the first window
                if self.windows.len() == 1 {
                    let args = Args::parse();
                    if let Some(file) = args.file {
                        // Create tasks to focus and then load the file
                        Task::batch([
                            focus_input.discard(),
                            Task::done(WindowManagerMessage::WindowMessage(id, Message::PackageSelected(file))),
                        ])
                    } else {
                        focus_input.discard()
                    }
                } else {
                    focus_input.discard()
                }
            }
            WindowManagerMessage::WindowClosed(id) => {
                self.windows.remove(&id);
                if self.windows.is_empty() {
                    icy_ui::exit()
                } else {
                    Task::none()
                }
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

            WindowManagerMessage::FocusWindow(_target_id) => {
                // icy_mail only supports single window, focus it if it exists
                if let Some(window_id) = self.windows.keys().next() {
                    return window::gain_focus(*window_id);
                }
                Task::none()
            }

            WindowManagerMessage::FocusNext => icy_ui::widget::operation::focus_next(),

            WindowManagerMessage::FocusPrevious => icy_ui::widget::operation::focus_previous(),

            WindowManagerMessage::_UpdateBuffers => {
                let mut tasks = vec![];
                for (id, _window) in self.windows.iter() {
                    let id = id.clone();
                    tasks.push(Task::done(WindowManagerMessage::WindowMessage(id, Message::BufferUpdated)));
                }
                Task::batch(tasks)
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
            icy_ui::event::listen_with(|event, _status, window_id| {
                match &event {
                    Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                        // Handle window manager keyboard shortcuts (Tab, Alt+Number, etc.)
                        if let Some(action) = handle_window_manager_keyboard_press(key, modifiers) {
                            return match action {
                                KeyboardAction::FocusWindow(target_id) => Some(WindowManagerMessage::FocusWindow(target_id)),
                                KeyboardAction::FocusNext => Some(WindowManagerMessage::FocusNext),
                                KeyboardAction::FocusPrevious => Some(WindowManagerMessage::FocusPrevious),
                            };
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
        ];
        icy_ui::Subscription::batch(subs)
    }
}
