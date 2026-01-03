//! Connect to server dialog
//!
//! Simple dialog to connect to Moebius-compatible collaboration servers.

use icy_engine_gui::ui::*;
use icy_engine_gui::{Dialog, DialogAction};
use icy_ui::{
    widget::{button, column, container, pick_list, row, svg, text, text_input},
    Element, Length,
};

use crate::fl;
use crate::ui::main_window::Message;
use crate::Settings;

const VISIBILITY_SVG: &[u8] = include_bytes!("../../../../data/icons/visibility.svg");
const VISIBILITY_OFF_SVG: &[u8] = include_bytes!("../../../../data/icons/visibility_off.svg");

/// Message type for the connect dialog
#[derive(Debug, Clone)]
pub enum ConnectDialogMessage {
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
    /// Toggle password visibility
    TogglePasswordVisibility,
    /// Connect button pressed
    Connect,
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

/// Connect to server dialog state
pub struct ConnectDialog {
    /// Server URL
    url: String,
    /// User nickname
    nick: String,
    /// User group (like Moebius)
    group: String,
    /// Session password
    password: String,
    /// Recent servers (for dropdown)
    recent_servers: Vec<String>,
    /// Whether to show password in plain text
    show_password: bool,
}

impl Default for ConnectDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectDialog {
    /// Create a new connect dialog
    pub fn new() -> Self {
        Self {
            url: String::new(),
            nick: "Anonymous".to_string(),
            group: String::new(),
            password: String::new(),
            recent_servers: Vec::new(),
            show_password: false,
        }
    }

    /// Create with settings (loads nick, group and servers from settings)
    pub fn with_settings(settings: &Settings) -> Self {
        let recent_servers = settings.collaboration_servers_list();
        let last_server = settings.last_collaboration_server().unwrap_or_default();
        let nick = settings.collaboration.nick.clone();
        let group = settings.collaboration.group.clone();

        Self {
            url: last_server,
            nick,
            group,
            password: String::new(),
            recent_servers,
            show_password: false,
        }
    }

    /// Check if the form is valid
    fn is_valid(&self) -> bool {
        !self.url.trim().is_empty() && !self.nick.trim().is_empty()
    }
}

impl Dialog<Message> for ConnectDialog {
    fn view(&self) -> Element<'_, Message> {
        let label_width = Length::Fixed(100.0);

        // Server URL input
        let url_label = container(text(fl!("collab-server-url")).size(TEXT_SIZE_NORMAL)).width(label_width);
        let url_input = text_input(&fl!("collab-server-url-placeholder"), &self.url)
            .on_input(|s| Message::ConnectDialog(ConnectDialogMessage::UrlChanged(s)))
            .padding(8)
            .width(Length::Fill);

        let url_row = if self.recent_servers.is_empty() {
            row![url_label, url_input].spacing(DIALOG_SPACING).align_y(icy_ui::Alignment::Center)
        } else {
            let servers: Vec<String> = self.recent_servers.iter().rev().cloned().collect();
            let selected = if self.url.is_empty() { None } else { Some(self.url.clone()) };
            let server_picker = pick_list(servers, selected, |s| Message::ConnectDialog(ConnectDialogMessage::ServerSelected(s)))
                .placeholder(fl!("collab-server-url-placeholder"))
                .width(Length::Fixed(150.0));
            row![url_label, url_input, server_picker]
                .spacing(DIALOG_SPACING)
                .align_y(icy_ui::Alignment::Center)
        };

        // Nickname input
        let nick_label = container(text(fl!("collab-nickname")).size(TEXT_SIZE_NORMAL)).width(label_width);
        let nick_input = text_input(&fl!("collab-nickname-placeholder"), &self.nick)
            .on_input(|s| Message::ConnectDialog(ConnectDialogMessage::NickChanged(s)))
            .padding(8)
            .width(Length::Fill);
        let nick_row = row![nick_label, nick_input].spacing(DIALOG_SPACING).align_y(icy_ui::Alignment::Center);

        // Group input (optional)
        let group_label = container(text(fl!("collab-group")).size(TEXT_SIZE_NORMAL)).width(label_width);
        let group_input = text_input(&fl!("collab-group-placeholder"), &self.group)
            .on_input(|s| Message::ConnectDialog(ConnectDialogMessage::GroupChanged(s)))
            .padding(8)
            .width(Length::Fill);
        let group_row = row![group_label, group_input].spacing(DIALOG_SPACING).align_y(icy_ui::Alignment::Center);

        // Password input with visibility toggle
        let password_label = container(text(fl!("collab-password")).size(TEXT_SIZE_NORMAL)).width(label_width);
        let password_input = text_input(&fl!("collab-password-placeholder"), &self.password)
            .on_input(|s| Message::ConnectDialog(ConnectDialogMessage::PasswordChanged(s)))
            .secure(!self.show_password)
            .padding(8)
            .width(Length::Fill);

        let visibility_icon = if self.show_password {
            svg(svg::Handle::from_memory(VISIBILITY_SVG))
        } else {
            svg(svg::Handle::from_memory(VISIBILITY_OFF_SVG))
        }
        .width(Length::Fixed(20.0))
        .height(Length::Fixed(20.0));

        let toggle_btn = button(visibility_icon)
            .on_press(Message::ConnectDialog(ConnectDialogMessage::TogglePasswordVisibility))
            .padding(4)
            .style(text_button_style);

        let password_row = row![password_label, password_input, toggle_btn]
            .spacing(DIALOG_SPACING)
            .align_y(icy_ui::Alignment::Center);

        // Form content
        let form = column![url_row, nick_row, group_row, password_row,].spacing(DIALOG_SPACING).width(Length::Fill);

        // Buttons
        let connect_msg = if self.is_valid() {
            Some(Message::ConnectDialog(ConnectDialogMessage::Connect))
        } else {
            None
        };

        let connect_button = primary_button(fl!("collab-connect-button"), connect_msg);
        let cancel_button: button::Button<'_, Message> = secondary_button(fl!("button-cancel"), Some(Message::ConnectDialog(ConnectDialogMessage::Cancel)));

        let button_row = button_row(vec![cancel_button.into(), connect_button.into()]);

        // Dialog layout
        let dialog_content = dialog_area(form.into());
        let button_area = dialog_area(button_row.into());

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area].into(),
            DIALOG_WIDTH_MEDIUM,
        )
        .into()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        let Message::ConnectDialog(msg) = message else {
            return None;
        };

        match msg {
            ConnectDialogMessage::UrlChanged(url) => {
                self.url = url.clone();
                Some(DialogAction::None)
            }
            ConnectDialogMessage::ServerSelected(url) => {
                self.url = url.clone();
                Some(DialogAction::None)
            }
            ConnectDialogMessage::NickChanged(nick) => {
                self.nick = nick.clone();
                Some(DialogAction::None)
            }
            ConnectDialogMessage::GroupChanged(group) => {
                self.group = group.clone();
                Some(DialogAction::None)
            }
            ConnectDialogMessage::PasswordChanged(password) => {
                self.password = password.clone();
                Some(DialogAction::None)
            }
            ConnectDialogMessage::TogglePasswordVisibility => {
                self.show_password = !self.show_password;
                Some(DialogAction::None)
            }
            ConnectDialogMessage::Connect => {
                if !self.is_valid() {
                    return Some(DialogAction::None);
                }

                let result = ConnectDialogResult {
                    url: self.url.trim().to_string(),
                    nick: self.nick.trim().to_string(),
                    group: self.group.trim().to_string(),
                    password: self.password.clone(),
                };
                Some(DialogAction::CloseWith(Message::ConnectToServer(result)))
            }
            ConnectDialogMessage::Cancel => Some(DialogAction::Close),
        }
    }

    fn handle_event(&mut self, event: &icy_ui::Event) -> Option<DialogAction<Message>> {
        // Handle Enter key to connect
        if let icy_ui::Event::Keyboard(icy_ui::keyboard::Event::KeyPressed {
            key: icy_ui::keyboard::Key::Named(icy_ui::keyboard::key::Named::Enter),
            ..
        }) = event
        {
            if self.is_valid() {
                let result = ConnectDialogResult {
                    url: self.url.trim().to_string(),
                    nick: self.nick.trim().to_string(),
                    group: self.group.trim().to_string(),
                    password: self.password.clone(),
                };
                return Some(DialogAction::CloseWith(Message::ConnectToServer(result)));
            }
        }

        // Handle Escape key to cancel
        if let icy_ui::Event::Keyboard(icy_ui::keyboard::Event::KeyPressed {
            key: icy_ui::keyboard::Key::Named(icy_ui::keyboard::key::Named::Escape),
            ..
        }) = event
        {
            return Some(DialogAction::Close);
        }

        None
    }
}
