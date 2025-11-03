use std::sync::Arc;

use crate::ui::NavigateDirection;
use crate::{qwk::QwkPackage, ui::Message};
use iced::widget::{Space, button, column, container, pane_grid, progress_bar, text, text_editor};
use iced::{Alignment, Element, Length, Task, Theme, window};

#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub enum MainWindowMode {
    #[default]
    ShowWelcomeScreen,
    LoadingPackage,
    ShowMailReader,
}

pub struct MainWindow {
    _id: window::Id,
    mode: MainWindowMode,
    pub package: Option<Arc<QwkPackage>>,
    loading_progress: f32,
    loading_message: String,

    // Mail reader state
    pub selected_conference: u16,
    pub selected_message: Option<usize>,
    pub message_list_scroll: iced::widget::Id,
    pub message_content: text_editor::Content,
    pub panes: pane_grid::State<PaneContent>,
    pub show_threads: bool,
    pub conference_list_focused: bool,
    pub message_list_focused: bool,
}

#[derive(Debug, Clone)]
pub enum PaneContent {
    ConferenceList,
    MessageList,
    MessageContent,
    ThreadView,
}

impl MainWindow {
    pub fn new(id: window::Id, mode: MainWindowMode) -> Self {
        // Create pane layout - simpler approach without needing active()
        let (mut panes, first_pane) = pane_grid::State::new(PaneContent::ConferenceList);
        let (message_list_pane, _) = panes
            .split(
                pane_grid::Axis::Vertical,
                first_pane, // Use the pane returned from new()
                PaneContent::MessageList,
            )
            .unwrap();
        let (_, _) = panes
            .split(pane_grid::Axis::Horizontal, message_list_pane, PaneContent::MessageContent)
            .unwrap();

        Self {
            _id: id,
            mode,
            package: None,
            loading_progress: 0.0,
            loading_message: String::new(),
            selected_conference: 0,
            selected_message: None,
            message_list_scroll: iced::widget::Id::unique(),
            message_content: text_editor::Content::new(),
            panes,
            show_threads: false,
            conference_list_focused: false,
            message_list_focused: false,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::QuitIcyMail => iced::exit(),

            Message::OpenPackage => Task::perform(
                async {
                    let file_dialog = rfd::AsyncFileDialog::new()
                        .set_title("Open Mail Package")
                        .add_filter("Mail Packages", &["zip", "qwk", "rep"])
                        .add_filter("All Files", &["*"]);

                    file_dialog.pick_file().await
                },
                |file| {
                    if let Some(file) = file {
                        Message::PackageSelected(file.path().to_path_buf())
                    } else {
                        Message::Noop
                    }
                },
            ),

            Message::PackageSelected(path) => {
                println!("Loading package: {:?}", path);
                self.mode = MainWindowMode::LoadingPackage;
                self.loading_progress = 0.0;
                self.loading_message = format!("Loading {}", path.file_name().unwrap_or_default().to_string_lossy());

                Task::perform(
                    async move { tokio::task::spawn_blocking(move || QwkPackage::load_from_file(path).map(Arc::new)).await },
                    |result| match result {
                        Ok(Ok(package)) => {
                            println!("Package load task completed");
                            Message::PackageLoaded(package)
                        }
                        Ok(Err(e)) => Message::PackageLoadError(format!("Failed to load package: {}", e)),
                        Err(e) => Message::PackageLoadError(format!("Thread error: {}", e)),
                    },
                )
            }

            Message::PackageLoaded(package) => {
                self.package = Some(package);
                self.mode = MainWindowMode::ShowMailReader;
                self.loading_progress = 1.0;
                Task::none()
            }

            Message::SelectConference(conf_idx) => {
                self.selected_conference = conf_idx;
                self.selected_message = None;
                self.message_content = text_editor::Content::new();
                self.conference_list_focused = true;
                self.message_list_focused = false;
                Task::none()
            }

            Message::SelectMessage(msg_idx) => {
                self.selected_message = Some(msg_idx);
                self.conference_list_focused = false;
                self.message_list_focused = true;

                // Load message content
                if let Some(package) = &self.package {
                    if let Ok(message) = package.get_message(msg_idx) {
                        // Convert message content to string for display
                        let content = String::from_utf8_lossy(&message.text).to_string();
                        self.message_content = text_editor::Content::with_text(&content);
                    }
                }
                Task::none()
            }

            Message::ToggleThreadView => {
                self.show_threads = !self.show_threads;
                Task::none()
            }

            Message::PackageLoadError(error) => {
                println!("Error loading package: {}", error);
                self.mode = MainWindowMode::ShowWelcomeScreen;
                self.loading_progress = 0.0;
                self.loading_message.clear();
                Task::none()
            }

            Message::NavigateConference(direction) => {
                if let Some(package) = &self.package {
                    // Get list of available conferences (including "All")
                    let mut available_conferences = vec![0]; // "All" is always 0

                    for conference in package.control_file.conferences.iter() {
                        let conf_num = conference.number;
                        let conf_name = String::from_utf8_lossy(&conference.name);
                        if !conf_name.trim().is_empty() {
                            // Check if conference has messages
                            let has_messages = package.descriptors.iter().any(|desc| desc.conference == conf_num as u16);
                            if has_messages {
                                available_conferences.push(conf_num);
                            }
                        }
                    }

                    // Find current position
                    let current_idx = available_conferences.iter().position(|&c| c == self.selected_conference).unwrap_or(0);

                    let new_idx = match direction {
                        NavigateDirection::Up => {
                            if current_idx > 0 {
                                current_idx - 1
                            } else {
                                current_idx
                            }
                        }
                        NavigateDirection::Down => {
                            if current_idx < available_conferences.len() - 1 {
                                current_idx + 1
                            } else {
                                current_idx
                            }
                        }
                        NavigateDirection::First => 0,
                        NavigateDirection::Last => available_conferences.len() - 1,
                        NavigateDirection::PageUp => {
                            current_idx.saturating_sub(5) // Move 5 items up
                        }
                        NavigateDirection::PageDown => {
                            (current_idx + 5).min(available_conferences.len() - 1) // Move 5 items down
                        }
                    };

                    self.selected_conference = available_conferences[new_idx];

                    // Optionally scroll the conference list to make selection visible
                    // This would require keeping track of a scrollable::Id for the conference list
                }
                Task::none()
            }

            Message::FocusConferenceList => {
                self.conference_list_focused = true;
                self.message_list_focused = false; // Assuming you have this field
                Task::none()
            }

            Message::NavigateMessage(direction) => {
                if let Some(package) = &self.package {
                    // Get filtered messages based on selected conference
                    let messages: Vec<usize> = if self.selected_conference == 0 {
                        (0..package.descriptors.len()).collect()
                    } else {
                        package
                            .descriptors
                            .iter()
                            .enumerate()
                            .filter(|(_, h)| h.conference == self.selected_conference as u16)
                            .map(|(idx, _)| idx)
                            .collect()
                    };

                    if messages.is_empty() {
                        return Task::none();
                    }

                    // Find current position in filtered list
                    let current_position = if let Some(selected) = self.selected_message {
                        messages.iter().position(|&idx| idx == selected)
                    } else {
                        None
                    };

                    let new_position = match direction {
                        NavigateDirection::Up => {
                            if let Some(pos) = current_position {
                                if pos > 0 { pos - 1 } else { pos }
                            } else {
                                0
                            }
                        }
                        NavigateDirection::Down => {
                            if let Some(pos) = current_position {
                                if pos < messages.len() - 1 { pos + 1 } else { pos }
                            } else {
                                0
                            }
                        }
                        NavigateDirection::First => 0,
                        NavigateDirection::Last => messages.len() - 1,
                        NavigateDirection::PageUp => current_position.unwrap_or(0).saturating_sub(10),
                        NavigateDirection::PageDown => {
                            let current = current_position.unwrap_or(0);
                            (current + 10).min(messages.len() - 1)
                        }
                    };

                    self.selected_message = Some(messages[new_position]);

                    // Load the message content
                    if let Ok(msg) = package.get_message(messages[new_position]) {
                        let content = String::from_utf8_lossy(&msg.text).to_string();
                        self.message_content = text_editor::Content::with_text(&content);
                    }
                }
                Task::none()
            }

            Message::FocusMessageList => {
                self.conference_list_focused = false;
                self.message_list_focused = true;
                // If no message is selected, select the first one
                if self.selected_message.is_none() {
                    if let Some(package) = &self.package {
                        let messages: Vec<usize> = if self.selected_conference == 0 {
                            (0..package.descriptors.len()).collect()
                        } else {
                            package
                                .descriptors
                                .iter()
                                .enumerate()
                                .filter(|(_, h)| h.conference == self.selected_conference as u16)
                                .map(|(idx, _)| idx)
                                .collect()
                        };
                        if !messages.is_empty() {
                            self.selected_message = Some(messages[0]);
                        }
                    }
                }
                Task::none()
            }

            Message::FocusMessageContent => {
                self.conference_list_focused = false;
                self.message_list_focused = false;
                // You might want to add a message_content_focused field
                Task::none()
            }

            _ => Task::none(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        match &self.mode {
            MainWindowMode::ShowWelcomeScreen => {
                let content = column![
                    Space::new().height(Length::Fill),
                    text("Welcome to IcyMail").size(32),
                    Space::new().height(20),
                    text("Open a mail package to get started").size(16),
                    Space::new().height(30),
                    button(text("Open Package").size(16)).on_press(Message::OpenPackage).padding([12, 24]),
                    Space::new().height(Length::Fill),
                ]
                .align_x(Alignment::Center)
                .width(Length::Fill);

                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into()
            }

            MainWindowMode::LoadingPackage => {
                let content = column![
                    Space::new().height(Length::Fill),
                    text("Loading Package").size(24),
                    Space::new().height(20),
                    text(&self.loading_message).size(14),
                    Space::new().height(20),
                    progress_bar(0.0..=1.0, self.loading_progress),
                    Space::new().height(Length::Fill),
                ]
                .align_x(Alignment::Center)
                .width(Length::Fill);

                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into()
            }

            MainWindowMode::ShowMailReader => self.mail_reader_view(),
        }
    }

    pub fn handle_event(&self, event: &iced::Event) -> Option<Message> {
        use iced::keyboard::{Event as KeyEvent, Key};

        match event {
            iced::Event::Keyboard(KeyEvent::KeyPressed { key, modifiers, .. }) => {
                // Only handle if conference list is focused
                if self.conference_list_focused {
                    match key {
                        Key::Named(iced::keyboard::key::Named::ArrowUp) => {
                            return Some(Message::NavigateConference(NavigateDirection::Up));
                        }
                        Key::Named(iced::keyboard::key::Named::ArrowDown) => {
                            return Some(Message::NavigateConference(NavigateDirection::Down));
                        }
                        Key::Named(iced::keyboard::key::Named::Home) => {
                            return Some(Message::NavigateConference(NavigateDirection::First));
                        }
                        Key::Named(iced::keyboard::key::Named::End) => {
                            return Some(Message::NavigateConference(NavigateDirection::Last));
                        }
                        Key::Named(iced::keyboard::key::Named::PageUp) => {
                            return Some(Message::NavigateConference(NavigateDirection::PageUp));
                        }
                        Key::Named(iced::keyboard::key::Named::PageDown) => {
                            return Some(Message::NavigateConference(NavigateDirection::PageDown));
                        }
                        Key::Named(iced::keyboard::key::Named::Enter) | Key::Named(iced::keyboard::key::Named::Space) => {
                            // Enter/Space confirms selection (already selected, but could trigger a message list update)
                            return Some(Message::SelectConference(self.selected_conference));
                        }
                        Key::Named(iced::keyboard::key::Named::Tab) if !modifiers.shift() => {
                            // Tab to move focus to message list
                            return Some(Message::FocusMessageList);
                        }
                        _ => {}
                    }
                } else if self.message_list_focused {
                    match key {
                        Key::Named(iced::keyboard::key::Named::ArrowUp) => {
                            return Some(Message::NavigateMessage(NavigateDirection::Up));
                        }
                        Key::Named(iced::keyboard::key::Named::ArrowDown) => {
                            return Some(Message::NavigateMessage(NavigateDirection::Down));
                        }
                        Key::Named(iced::keyboard::key::Named::Home) => {
                            return Some(Message::NavigateMessage(NavigateDirection::First));
                        }
                        Key::Named(iced::keyboard::key::Named::End) => {
                            return Some(Message::NavigateMessage(NavigateDirection::Last));
                        }
                        Key::Named(iced::keyboard::key::Named::PageUp) => {
                            return Some(Message::NavigateMessage(NavigateDirection::PageUp));
                        }
                        Key::Named(iced::keyboard::key::Named::PageDown) => {
                            return Some(Message::NavigateMessage(NavigateDirection::PageDown));
                        }
                        Key::Named(iced::keyboard::key::Named::Enter) | Key::Named(iced::keyboard::key::Named::Space) => {
                            if let Some(idx) = self.selected_message {
                                return Some(Message::SelectMessage(idx));
                            }
                        }
                        Key::Named(iced::keyboard::key::Named::Tab) if modifiers.shift() => {
                            // Shift+Tab to go back to conference list
                            return Some(Message::FocusConferenceList);
                        }
                        Key::Named(iced::keyboard::key::Named::Tab) if !modifiers.shift() => {
                            // Tab to move focus to message content (if you want to add that)
                            return Some(Message::FocusMessageContent);
                        }
                        _ => {}
                    }
                }
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                // We'll need to check if the click was in the conference area
                // This is handled by the button presses, but we need to track focus
                // You might want to return a FocusConferenceList message here based on position
            }
            _ => {}
        }
        None
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }
}
