//! Connection dialog for collaboration servers
//!
//! Allows users to connect to Moebius-compatible collaboration servers.

use iced::{
    Element, Length,
    widget::{column, container, row, text, text_input, Space},
};
use icy_engine_gui::settings::left_label;
use icy_engine_gui::ui::*;
use icy_engine_gui::{Dialog, DialogAction};

use crate::fl;
use crate::ui::main_window::Message;

/// Message type for the connect dialog
#[derive(Debug, Clone)]
pub enum ConnectDialogMessage {
    /// Server URL changed
    UrlChanged(String),
    /// Nickname changed
    NickChanged(String),
    /// Password changed
    PasswordChanged(String),
    /// Connect button pressed
    Connect,
    /// Cancel button pressed
    Cancel,
}

/// Result when dialog is confirmed
#[derive(Debug, Clone)]
pub struct ConnectDialogResult {
    pub url: String,
    pub nick: String,
    pub password: String,
}

/// Connection dialog state
pub struct ConnectDialog {
    /// Server URL
    url: String,
    /// User nickname
    nick: String,
    /// Session password
    password: String,
}

impl Default for ConnectDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectDialog {
    /// Create a new connection dialog
    pub fn new() -> Self {
        Self {
            url: String::new(),
            nick: "Anonymous".to_string(),
            password: String::new(),
        }
    }

    /// Create with pre-filled values
    pub fn with_values(url: impl Into<String>, nick: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            nick: nick.into(),
            password: password.into(),
        }
    }

    /// Check if the form is valid for connection
    fn is_valid(&self) -> bool {
        !self.url.trim().is_empty() && !self.nick.trim().is_empty()
    }
}

impl Dialog<Message> for ConnectDialog {
    fn view(&self) -> Element<'_, Message> {
        // Server URL input
        let url_label = left_label(fl!("connect-server-url"));
        let url_input = text_input(&fl!("connect-server-url-placeholder"), &self.url)
            .on_input(|s| Message::ConnectDialog(ConnectDialogMessage::UrlChanged(s)))
            .padding(8)
            .width(Length::Fill);

        let url_row = row![url_label, url_input].spacing(DIALOG_SPACING).align_y(iced::Alignment::Center);

        // Nickname input
        let nick_label = left_label(fl!("connect-nickname"));
        let nick_input = text_input(&fl!("connect-nickname-placeholder"), &self.nick)
            .on_input(|s| Message::ConnectDialog(ConnectDialogMessage::NickChanged(s)))
            .padding(8)
            .width(Length::Fill);

        let nick_row = row![nick_label, nick_input].spacing(DIALOG_SPACING).align_y(iced::Alignment::Center);

        // Password input (optional)
        let password_label = left_label(fl!("connect-password"));
        let password_input = text_input(&fl!("connect-password-placeholder"), &self.password)
            .on_input(|s| Message::ConnectDialog(ConnectDialogMessage::PasswordChanged(s)))
            .secure(true)
            .padding(8)
            .width(Length::Fill);

        let password_row = row![password_label, password_input]
            .spacing(DIALOG_SPACING)
            .align_y(iced::Alignment::Center);

        // Help text
        let help_text = text(fl!("connect-help-text"))
            .size(TEXT_SIZE_SMALL)
            .color(iced::Color::from_rgb(0.5, 0.5, 0.5));

        // Form content
        let form = column![url_row, nick_row, password_row, Space::new().height(8.0), help_text,]
            .spacing(DIALOG_SPACING)
            .width(Length::Fill);

        // Buttons
        let connect_msg = if self.is_valid() {
            Some(Message::ConnectDialog(ConnectDialogMessage::Connect))
        } else {
            None
        };

        let connect_button = primary_button(fl!("connect-button"), connect_msg);
        let cancel_button = secondary_button(fl!("button-cancel"), Some(Message::ConnectDialog(ConnectDialogMessage::Cancel)));

        let button_row = button_row(vec![cancel_button.into(), connect_button.into()]);

        // Dialog layout
        let dialog_content = dialog_area(
            column![
                section_header(fl!("connect-dialog-title")),
                Space::new().height(DIALOG_SPACING),
                form,
            ]
            .into(),
        );

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
            ConnectDialogMessage::NickChanged(nick) => {
                self.nick = nick.clone();
                Some(DialogAction::None)
            }
            ConnectDialogMessage::PasswordChanged(password) => {
                self.password = password.clone();
                Some(DialogAction::None)
            }
            ConnectDialogMessage::Connect => {
                if self.is_valid() {
                    let result = ConnectDialogResult {
                        url: self.url.trim().to_string(),
                        nick: self.nick.trim().to_string(),
                        password: self.password.clone(),
                    };
                    Some(DialogAction::CloseWith(Message::ConnectToServer(result)))
                } else {
                    Some(DialogAction::None)
                }
            }
            ConnectDialogMessage::Cancel => Some(DialogAction::Close),
        }
    }

    fn handle_event(&mut self, event: &iced::Event) -> Option<DialogAction<Message>> {
        // Handle Enter key to connect
        if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
            ..
        }) = event
        {
            if self.is_valid() {
                let result = ConnectDialogResult {
                    url: self.url.trim().to_string(),
                    nick: self.nick.trim().to_string(),
                    password: self.password.clone(),
                };
                return Some(DialogAction::CloseWith(Message::ConnectToServer(result)));
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
