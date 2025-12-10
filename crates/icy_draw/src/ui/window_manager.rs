//! Window Manager for icy_draw
//!
//! Manages multiple independent windows, each with its own MainWindow state.
//! Based on the icy_view/icy_term window manager pattern.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use parking_lot::Mutex;

use iced::{Element, Event, Size, Subscription, Task, Theme, Vector, keyboard, widget::space, window};

use icy_engine_gui::command_handler;
use icy_engine_gui::commands::cmd;
use icy_engine_gui::{ANIMATION_TICK_MS, any_window_needs_animation, find_next_window_id, focus_window_by_id, handle_window_closed};

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

    pub fn title(&self, window_id: window::Id) -> String {
        let Some(w) = self.windows.get(&window_id) else {
            return String::new();
        };

        if self.windows.len() == 1 {
            w.title()
        } else if w.id <= 10 {
            let display_key = if w.id == 10 { 0 } else { w.id };
            format!("{} - ⌘{}", w.title(), display_key)
        } else {
            w.title()
        }
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
                let window = MainWindow::new(find_next_window_id(&self.windows), self.initial_path.take(), self.options.clone());
                self.windows.insert(id, window);
                Task::none()
            }

            WindowManagerMessage::WindowClosed(id) => handle_window_closed(&mut self.windows, id),

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
                        println!("Key pressed: {:?} with modifiers {:?}", key, modifiers);
                        if let Some(target_id) = icy_engine_gui::check_window_focus_key(key, modifiers) {
                            return Some(WindowManagerMessage::FocusWindow(target_id));
                        }
                        Some(WindowManagerMessage::Event(window_id, event))
                    }
                    Event::Keyboard(_) => Some(WindowManagerMessage::Event(window_id, event)),
                    _ => None,
                }
            }),
        ];

        // Add animation tick subscription when any window needs animation
        if any_window_needs_animation(&self.windows) {
            subs.push(iced::time::every(std::time::Duration::from_millis(ANIMATION_TICK_MS)).map(|_| WindowManagerMessage::AnimationTick));
        }

        Subscription::batch(subs)
    }
}
