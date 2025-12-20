//! Collaboration dialog for connecting or hosting sessions
//!
//! Allows users to connect to Moebius-compatible collaboration servers
//! or host their own session.

use iced::{
    Element, Length,
    widget::{Space, column, container, pick_list, radio, row, text, text_input},
};
use icy_engine_gui::ui::*;
use icy_engine_gui::{Dialog, DialogAction};

use crate::Settings;
use crate::fl;
use crate::ui::main_window::Message;

/// The collaboration mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollaborationMode {
    /// Connect to an existing server
    #[default]
    Connect,
    /// Host a new session
    Host,
}

/// Message type for the collaboration dialog
#[derive(Debug, Clone)]
pub enum CollaborationDialogMessage {
    /// Mode changed (connect vs host)
    ModeChanged(CollaborationMode),
    /// Server URL changed
    UrlChanged(String),
    /// Server selected from dropdown
    ServerSelected(String),
    /// Nickname changed
    NickChanged(String),
    /// Group changed
    GroupChanged(String),
    /// Password changed
    PasswordChanged(String),
    /// Port changed (for hosting)
    PortChanged(String),
    /// Start button pressed
    Start,
    /// Cancel button pressed
    Cancel,
}

/// Result when connecting to a server
#[derive(Debug, Clone)]
pub struct ConnectDialogResult {
    pub url: String,
    pub nick: String,
    pub group: String,
    pub password: String,
}

/// Result when hosting a session
#[derive(Debug, Clone)]
pub struct HostSessionResult {
    pub port: u16,
    pub nick: String,
    pub group: String,
    pub password: String,
}

/// Collaboration dialog state
pub struct CollaborationDialog {
    /// Current mode
    mode: CollaborationMode,
    /// Server URL (for connect mode)
    url: String,
    /// User nickname
    nick: String,
    /// User group (like Moebius)
    group: String,
    /// Session password
    password: String,
    /// Port for hosting (default 8000)
    host_port: String,
    /// Recent servers (for dropdown)
    recent_servers: Vec<String>,
}

impl Default for CollaborationDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl CollaborationDialog {
    /// Create a new collaboration dialog
    pub fn new() -> Self {
        Self {
            mode: CollaborationMode::Connect,
            url: String::new(),
            nick: "Anonymous".to_string(),
            group: String::new(),
            password: String::new(),
            host_port: crate::DEFAULT_COLLAB_PORT_STR.to_string(),
            recent_servers: Vec::new(),
        }
    }

    /// Create with settings (loads nick, group and servers from settings)
    pub fn with_settings(settings: &Settings) -> Self {
        let recent_servers = settings.collaboration_servers_list();
        let last_server = settings.last_collaboration_server().unwrap_or_default();
        let nick = settings.get_collaboration_nick();
        let group = settings.get_collaboration_group();

        Self {
            mode: CollaborationMode::Connect,
            url: last_server,
            nick,
            group,
            password: String::new(),
            host_port: crate::DEFAULT_COLLAB_PORT_STR.to_string(),
            recent_servers,
        }
    }

    /// Check if the form is valid
    fn is_valid(&self) -> bool {
        match self.mode {
            CollaborationMode::Connect => !self.url.trim().is_empty() && !self.nick.trim().is_empty(),
            CollaborationMode::Host => !self.nick.trim().is_empty() && self.host_port.parse::<u16>().is_ok(),
        }
    }
}

impl Dialog<Message> for CollaborationDialog {
    fn view(&self) -> Element<'_, Message> {
        let label_width = Length::Fixed(100.0);
        let dimmed_color = iced::Color::from_rgb(0.4, 0.4, 0.4);
        let normal_color = iced::Color::from_rgb(0.85, 0.85, 0.85);

        // Nickname input (shared)
        let nick_label = container(text(fl!("collab-nickname")).size(TEXT_SIZE_NORMAL)).width(label_width);
        let nick_input = text_input(&fl!("collab-nickname-placeholder"), &self.nick)
            .on_input(|s| Message::CollaborationDialog(CollaborationDialogMessage::NickChanged(s)))
            .padding(8)
            .width(Length::Fill);
        let nick_row = row![nick_label, nick_input].spacing(DIALOG_SPACING).align_y(iced::Alignment::Center);

        // Group input (shared, optional)
        let group_label = container(text(fl!("collab-group")).size(TEXT_SIZE_NORMAL)).width(label_width);
        let group_input = text_input(&fl!("collab-group-placeholder"), &self.group)
            .on_input(|s| Message::CollaborationDialog(CollaborationDialogMessage::GroupChanged(s)))
            .padding(8)
            .width(Length::Fill);
        let group_row = row![group_label, group_input].spacing(DIALOG_SPACING).align_y(iced::Alignment::Center);

        // Password input (shared, optional)
        let password_label = container(text(fl!("collab-password")).size(TEXT_SIZE_NORMAL)).width(label_width);
        let password_input = text_input(&fl!("collab-password-placeholder"), &self.password)
            .on_input(|s| Message::CollaborationDialog(CollaborationDialogMessage::PasswordChanged(s)))
            .secure(true)
            .padding(8)
            .width(Length::Fill);
        let password_row = row![password_label, password_input].spacing(DIALOG_SPACING).align_y(iced::Alignment::Center);

        // Radio button: Connect to existing server
        let connect_radio = radio(fl!("collab-mode-connect"), CollaborationMode::Connect, Some(self.mode), |mode| {
            Message::CollaborationDialog(CollaborationDialogMessage::ModeChanged(mode))
        })
        .size(18)
        .spacing(8);

        // URL input (only active in Connect mode)
        let url_dimmed = self.mode != CollaborationMode::Connect;
        let url_color = if url_dimmed { dimmed_color } else { normal_color };

        let url_label = container(text(fl!("collab-server-url")).size(TEXT_SIZE_NORMAL).color(url_color)).width(label_width);

        let url_input = if url_dimmed {
            text_input(&fl!("collab-server-url-placeholder"), &self.url).padding(8).width(Length::Fill)
        } else {
            text_input(&fl!("collab-server-url-placeholder"), &self.url)
                .on_input(|s| Message::CollaborationDialog(CollaborationDialogMessage::UrlChanged(s)))
                .padding(8)
                .width(Length::Fill)
        };

        let url_row = if self.recent_servers.is_empty() || url_dimmed {
            row![Space::new().width(24), url_label, url_input]
                .spacing(DIALOG_SPACING)
                .align_y(iced::Alignment::Center)
        } else {
            let servers: Vec<String> = self.recent_servers.iter().rev().cloned().collect();
            let selected = if self.url.is_empty() { None } else { Some(self.url.clone()) };
            let server_picker = pick_list(servers, selected, |s| {
                Message::CollaborationDialog(CollaborationDialogMessage::ServerSelected(s))
            })
            .placeholder(fl!("collab-server-url-placeholder"))
            .width(Length::Fixed(150.0));
            row![Space::new().width(24), url_label, url_input, server_picker]
                .spacing(DIALOG_SPACING)
                .align_y(iced::Alignment::Center)
        };

        // Radio button: Host new session
        let host_radio = radio(fl!("collab-mode-host"), CollaborationMode::Host, Some(self.mode), |mode| {
            Message::CollaborationDialog(CollaborationDialogMessage::ModeChanged(mode))
        })
        .size(18)
        .spacing(8);

        // Port input (only active in Host mode)
        let port_dimmed = self.mode != CollaborationMode::Host;
        let port_color = if port_dimmed { dimmed_color } else { normal_color };

        let port_label = container(text(fl!("collab-port")).size(TEXT_SIZE_NORMAL).color(port_color)).width(label_width);

        let port_input = if port_dimmed {
            text_input(crate::DEFAULT_COLLAB_PORT_STR, &self.host_port)
                .padding(8)
                .width(Length::Fixed(80.0))
        } else {
            text_input(crate::DEFAULT_COLLAB_PORT_STR, &self.host_port)
                .on_input(|s| Message::CollaborationDialog(CollaborationDialogMessage::PortChanged(s)))
                .padding(8)
                .width(Length::Fixed(80.0))
        };

        let port_row = row![Space::new().width(24), port_label, port_input]
            .spacing(DIALOG_SPACING)
            .align_y(iced::Alignment::Center);

        // Form content
        let form = column![
            nick_row,
            group_row,
            password_row,
            Space::new().height(16.0),
            connect_radio,
            url_row,
            Space::new().height(12.0),
            host_radio,
            port_row,
        ]
        .spacing(DIALOG_SPACING)
        .width(Length::Fill);

        // Buttons
        let start_msg = if self.is_valid() {
            Some(Message::CollaborationDialog(CollaborationDialogMessage::Start))
        } else {
            None
        };

        let start_button = primary_button(fl!("collab-start-button"), start_msg);
        let cancel_button = secondary_button(fl!("button-cancel"), Some(Message::CollaborationDialog(CollaborationDialogMessage::Cancel)));

        let button_row = button_row(vec![cancel_button.into(), start_button.into()]);

        // Dialog layout (no title)
        let dialog_content = dialog_area(form.into());
        let button_area = dialog_area(button_row.into());

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area].into(),
            DIALOG_WIDTH_MEDIUM,
        )
        .into()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        let Message::CollaborationDialog(msg) = message else {
            return None;
        };

        match msg {
            CollaborationDialogMessage::ModeChanged(mode) => {
                self.mode = *mode;
                Some(DialogAction::None)
            }
            CollaborationDialogMessage::UrlChanged(url) => {
                self.url = url.clone();
                Some(DialogAction::None)
            }
            CollaborationDialogMessage::ServerSelected(url) => {
                self.url = url.clone();
                Some(DialogAction::None)
            }
            CollaborationDialogMessage::NickChanged(nick) => {
                self.nick = nick.clone();
                Some(DialogAction::None)
            }
            CollaborationDialogMessage::GroupChanged(group) => {
                self.group = group.clone();
                Some(DialogAction::None)
            }
            CollaborationDialogMessage::PasswordChanged(password) => {
                self.password = password.clone();
                Some(DialogAction::None)
            }
            CollaborationDialogMessage::PortChanged(port) => {
                // Only allow numeric input
                if port.is_empty() || port.chars().all(|c| c.is_ascii_digit()) {
                    self.host_port = port.clone();
                }
                Some(DialogAction::None)
            }
            CollaborationDialogMessage::Start => {
                if !self.is_valid() {
                    return Some(DialogAction::None);
                }

                match self.mode {
                    CollaborationMode::Connect => {
                        let result = ConnectDialogResult {
                            url: self.url.trim().to_string(),
                            nick: self.nick.trim().to_string(),
                            group: self.group.trim().to_string(),
                            password: self.password.clone(),
                        };
                        Some(DialogAction::CloseWith(Message::ConnectToServer(result)))
                    }
                    CollaborationMode::Host => {
                        let result = HostSessionResult {
                            port: self.host_port.parse().unwrap_or(crate::DEFAULT_COLLAB_PORT),
                            nick: self.nick.trim().to_string(),
                            group: self.group.trim().to_string(),
                            password: self.password.clone(),
                        };
                        Some(DialogAction::CloseWith(Message::HostSession(result)))
                    }
                }
            }
            CollaborationDialogMessage::Cancel => Some(DialogAction::Close),
        }
    }

    fn handle_event(&mut self, event: &iced::Event) -> Option<DialogAction<Message>> {
        // Handle Enter key to start
        if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
            ..
        }) = event
        {
            if self.is_valid() {
                match self.mode {
                    CollaborationMode::Connect => {
                        let result = ConnectDialogResult {
                            url: self.url.trim().to_string(),
                            nick: self.nick.trim().to_string(),
                            group: self.group.trim().to_string(),
                            password: self.password.clone(),
                        };
                        return Some(DialogAction::CloseWith(Message::ConnectToServer(result)));
                    }
                    CollaborationMode::Host => {
                        let result = HostSessionResult {
                            port: self.host_port.parse().unwrap_or(crate::DEFAULT_COLLAB_PORT),
                            nick: self.nick.trim().to_string(),
                            group: self.group.trim().to_string(),
                            password: self.password.clone(),
                        };
                        return Some(DialogAction::CloseWith(Message::HostSession(result)));
                    }
                }
            }
        }

        // Handle Escape key to cancel
        if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            ..
        }) = event
        {
            return Some(DialogAction::Close);
        }

        None
    }
}
