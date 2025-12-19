//! Chat panel widget for collaboration
//!
//! A 3-part layout:
//! - Left: User list
//! - Center: Chat messages
//! - Bottom: Input field

use iced::{
    Border, Color, Element, Length, Theme,
    widget::{Column, Space, column, container, row, scrollable, text, text_input},
};
use icy_engine_edit::collaboration::ChatMessage;

use crate::ui::collaboration::state::{CollaborationState, RemoteUser};
use crate::ui::main_window::Message;

/// Message type for chat panel interactions
#[derive(Debug, Clone)]
pub enum ChatPanelMessage {
    /// Chat input text changed
    InputChanged(String),
    /// Send chat message
    SendMessage,
    /// Click on user to go to their cursor
    GotoUser(u32),
}

/// Chat panel width
pub const CHAT_PANEL_WIDTH: f32 = 320.0;
const USER_LIST_WIDTH: f32 = 120.0;

/// View the chat panel
pub fn view_chat_panel<'a>(state: &'a CollaborationState, chat_input: &'a str) -> Element<'a, Message> {
    if !state.active {
        return Space::new().width(0.0).height(0.0).into();
    }

    let user_list = view_user_list(state);
    let chat_area = view_chat_area(&state.chat_messages);
    let input_area = view_input_area(chat_input);

    let main_content = column![chat_area, input_area].spacing(4);

    let content = row![user_list, main_content].spacing(4);

    container(content)
        .width(Length::Fixed(CHAT_PANEL_WIDTH))
        .height(Length::Fill)
        .padding(8)
        .style(chat_panel_style)
        .into()
}

/// View the user list
fn view_user_list(state: &CollaborationState) -> Element<'_, Message> {
    let mut user_column = Column::new().spacing(2).width(Length::Fixed(USER_LIST_WIDTH));

    // Header
    user_column = user_column.push(text("Users").size(12).color(Color::from_rgb(0.6, 0.6, 0.6)).width(Length::Fill));

    user_column = user_column.push(Space::new().height(4.0));

    // User entries
    let sorted_users = state.sorted_users();
    for remote_user in sorted_users {
        let user_entry = view_user_entry(remote_user, state);
        user_column = user_column.push(user_entry);
    }

    // Show "No users" if empty
    if state.remote_users.is_empty() {
        user_column = user_column.push(text("No other users").size(11).color(Color::from_rgb(0.5, 0.5, 0.5)));
    }

    container(scrollable(user_column).direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default())))
        .width(Length::Fixed(USER_LIST_WIDTH))
        .height(Length::Fill)
        .style(user_list_style)
        .into()
}

/// View a single user entry
fn view_user_entry<'a>(remote_user: &'a RemoteUser, state: &'a CollaborationState) -> Element<'a, Message> {
    let color = state.user_color(remote_user.user.id);
    let color_indicator = container(Space::new().width(8.0).height(8.0)).style(move |_theme: &Theme| container::Style {
        background: Some(iced::Background::Color(Color::from_rgb8(color.0, color.1, color.2))),
        border: Border::default().rounded(4.0),
        ..Default::default()
    });

    let nick = &remote_user.user.nick;
    let display_name = if nick.is_empty() { "Guest" } else { nick };

    let name_text = text(display_name).size(12);

    let user_row = row![color_indicator, name_text].spacing(6).align_y(iced::Alignment::Center);

    iced::widget::button(user_row)
        .on_press(Message::ChatPanel(ChatPanelMessage::GotoUser(remote_user.user.id)))
        .padding([2, 4])
        .style(user_entry_style)
        .into()
}

/// View the chat messages area
fn view_chat_area(messages: &[ChatMessage]) -> Element<'_, Message> {
    let mut message_column = Column::new().spacing(4).width(Length::Fill);

    if messages.is_empty() {
        message_column = message_column.push(text("No messages yet").size(11).color(Color::from_rgb(0.5, 0.5, 0.5)));
    } else {
        for msg in messages.iter().rev().take(100).rev() {
            let msg_view = view_chat_message(msg);
            message_column = message_column.push(msg_view);
        }
    }

    container(
        scrollable(message_column)
            .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default()))
            .anchor_bottom(),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(chat_area_style)
    .into()
}

/// View a single chat message
fn view_chat_message(msg: &ChatMessage) -> Element<'_, Message> {
    let nick = if msg.nick.is_empty() { "Guest" } else { &msg.nick };

    let nick_text = text(format!("{}: ", nick)).size(12).color(Color::from_rgb(0.3, 0.6, 1.0));

    let message_text = text(&msg.text).size(12);

    row![nick_text, message_text].spacing(0).into()
}

/// View the input area
fn view_input_area(input_text: &str) -> Element<'_, Message> {
    let input = text_input("Type a message...", input_text)
        .on_input(|s| Message::ChatPanel(ChatPanelMessage::InputChanged(s)))
        .on_submit(Message::ChatPanel(ChatPanelMessage::SendMessage))
        .padding(6)
        .size(12)
        .width(Length::Fill);

    container(input).width(Length::Fill).style(input_area_style).into()
}

// ============================================================================
// Styles
// ============================================================================

fn chat_panel_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(iced::Background::Color(palette.background.weak.color)),
        border: Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

fn user_list_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(iced::Background::Color(palette.background.base.color.scale_alpha(0.5))),
        border: Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

fn user_entry_style(theme: &Theme, status: iced::widget::button::Status) -> iced::widget::button::Style {
    let palette = theme.extended_palette();
    let base = iced::widget::button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        text_color: palette.background.base.text,
        border: Border::default().rounded(2.0),
        ..Default::default()
    };

    match status {
        iced::widget::button::Status::Hovered => iced::widget::button::Style {
            background: Some(iced::Background::Color(palette.primary.weak.color.scale_alpha(0.3))),
            ..base
        },
        iced::widget::button::Status::Pressed => iced::widget::button::Style {
            background: Some(iced::Background::Color(palette.primary.weak.color.scale_alpha(0.5))),
            ..base
        },
        _ => base,
    }
}

fn chat_area_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(iced::Background::Color(palette.background.base.color)),
        border: Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

fn input_area_style(_theme: &Theme) -> container::Style {
    container::Style::default()
}
