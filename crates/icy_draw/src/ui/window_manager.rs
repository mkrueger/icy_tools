//! Window Manager for icy_draw
//!
//! Manages multiple independent windows, each with its own MainWindow state.
//! Based on the icy_view/icy_term window manager pattern.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use parking_lot::Mutex;

use iced::{Element, Event, Size, Subscription, Task, Theme, Vector, keyboard, widget::space, window};

use icy_engine_gui::command_handler;
use icy_engine_gui::commands::cmd;

use super::{MainWindow, MostRecentlyUsedFiles, commands::create_draw_commands};
use crate::load_window_icon;

// Generate the WindowCommands struct with handle() method
command_handler!(WindowCommands, create_draw_commands(), _window_id: window::Id => WindowManagerMessage {
    cmd::WINDOW_NEW => WindowManagerMessage::OpenWindow,
    cmd::WINDOW_CLOSE => WindowManagerMessage::CloseWindow(_window_id),
    cmd::FILE_CLOSE => WindowManagerMessage::CloseWindow(_window_id),
});

/// Shared options between all windows
pub struct SharedOptions {
    /// Most recently used files
    pub recent_files: MostRecentlyUsedFiles,
}

impl SharedOptions {
    pub fn load() -> Self {
        Self {
            recent_files: MostRecentlyUsedFiles::load(),
        }
    }
}

pub struct WindowManager {
    windows: BTreeMap<window::Id, MainWindow>,
    options: Arc<Mutex<SharedOptions>>,
    initial_path: Option<PathBuf>,
    commands: WindowCommands,
}

#[derive(Clone, Debug)]
pub enum WindowManagerMessage {
    OpenWindow,
    CloseWindow(window::Id),
    WindowOpened(window::Id),
    FocusWindow(usize),
    WindowClosed(window::Id),
    WindowMessage(window::Id, super::main_window::Message),
    Event(window::Id, iced::Event),
    AnimationTick,
}

const DEFAULT_SIZE: Size = Size::new(1280.0, 800.0);

impl WindowManager {
    pub fn new() -> (Self, Task<WindowManagerMessage>) {
        let window_icon = load_window_icon(include_bytes!("../../build/linux/256x256.png")).ok();
        let settings = window::Settings {
            size: DEFAULT_SIZE,
            icon: window_icon,
            ..window::Settings::default()
        };
        let (_, open) = window::open(settings);

        let options = SharedOptions::load();
        let commands = WindowCommands::new();

        (
            Self {
                windows: BTreeMap::new(),
                options: Arc::new(Mutex::new(options)),
                initial_path: None,
                commands,
            },
            open.map(WindowManagerMessage::WindowOpened),
        )
    }

    pub fn with_path(path: PathBuf) -> (Self, Task<WindowManagerMessage>) {
        let (mut manager, task) = Self::new();
        manager.initial_path = Some(path);
        (manager, task)
    }

    pub fn title(&self, window: window::Id) -> String {
        if self.windows.iter().count() == 1 {
            return self.windows.get(&window).map(|w| w.title()).unwrap_or_default();
        }

        self.windows
            .get(&window)
            .map(|w| if w.id < 10 { format!("{} - ⌘{}", w.title(), w.id) } else { w.title() })
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
                let window = MainWindow::new(self.find_next_id(), self.initial_path.take(), self.options.clone());
                self.windows.insert(id, window);
                Task::none()
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
                // Handle keyboard commands at window manager level
                if let Some(msg) = self.commands.handle(&event, window_id) {
                    return Task::done(msg);
                }

                // Pass event to window for other handling
                if let Some(window) = self.windows.get_mut(&window_id) {
                    if let Some(msg) = window.handle_event(&event) {
                        return Task::done(WindowManagerMessage::WindowMessage(window_id, msg));
                    }
                }
                Task::none()
            }

            WindowManagerMessage::FocusWindow(target_id) => {
                for (window_id, window) in self.windows.iter() {
                    if window.id == target_id {
                        return window::gain_focus(*window_id);
                    }
                }
                Task::none()
            }

            WindowManagerMessage::AnimationTick => {
                // Send tick to all windows that need animation
                let tasks: Vec<_> = self
                    .windows
                    .iter_mut()
                    .filter(|(_, w)| w.needs_animation())
                    .map(|(id, w)| {
                        let id = *id;
                        w.update(super::main_window::Message::AnimationTick)
                            .map(move |msg| WindowManagerMessage::WindowMessage(id, msg))
                    })
                    .collect();
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
        let needs_animation = self.windows.values().any(|w| w.needs_animation());

        let mut subs = vec![
            window::close_events().map(WindowManagerMessage::WindowClosed),
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
                        // Alt+Number to focus window
                        if (modifiers.alt() || modifiers.command()) && !modifiers.shift() && !modifiers.control() {
                            if let keyboard::Key::Character(s) = &key {
                                if let Some(digit) = s.chars().next() {
                                    if digit.is_ascii_digit() {
                                        let target_id = digit.to_digit(10).unwrap() as usize;
                                        let target_id = if target_id == 0 { 10 } else { target_id };
                                        return Some(WindowManagerMessage::FocusWindow(target_id));
                                    }
                                }
                            }
                        }
                        Some(WindowManagerMessage::Event(window_id, event))
                    }
                    Event::Keyboard(_) => Some(WindowManagerMessage::Event(window_id, event)),
                    _ => None,
                }
            }),
        ];

        // Add animation tick subscription when any window needs animation
        if needs_animation {
            subs.push(iced::time::every(std::time::Duration::from_millis(16)).map(|_| WindowManagerMessage::AnimationTick));
        }

        Subscription::batch(subs)
    }

    fn find_next_id(&self) -> usize {
        let used_ids: std::collections::HashSet<usize> = self.windows.values().map(|w| w.id).collect();
        for id in 1.. {
            if !used_ids.contains(&id) {
                return id;
            }
        }
        1
    }
}
