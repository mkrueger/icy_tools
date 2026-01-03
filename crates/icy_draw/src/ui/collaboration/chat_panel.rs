//! Chat panel widget for collaboration
//!
//! A 3-part layout:
//! - Left: User list (Discord-style with avatars)
//! - Right: Chat messages (Discord-style grouped by user)
//! - Bottom: Input field

use icy_engine_edit::collaboration::ChatMessage;
use icy_ui::{
    widget::{column, container, row, scrollable, stack, text, text_input, Column, Row, Space},
    Border, Color, Element, Length, Padding, Theme,
};

use super::icons::{Avatar, UserStatus};
use crate::fl;
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

/// User list width
const USER_LIST_WIDTH: f32 = 180.0;
/// Avatar size in user list
const AVATAR_SIZE: f32 = 36.0;
/// Avatar size in chat messages
const CHAT_AVATAR_SIZE: f32 = 40.0;
/// Status badge size
const STATUS_BADGE_SIZE: f32 = 12.0;

/// Text colors
const BASE_COLOR: Color = Color::from_rgb(0.85, 0.85, 0.85);
const SECONDARY_COLOR: Color = Color::from_rgb(0.5, 0.5, 0.5);

// ============================================================================
// ViewModel: Message grouping for Discord-style display
// ============================================================================

/// A displayable chat item - either a user message group or a system message
enum ChatItem {
    /// A group of consecutive messages from the same user
    UserMessages(MessageGroup),
    /// A system message (user joined/left, etc.)
    SystemMessage(String),
}

/// A group of consecutive messages from the same user
struct MessageGroup {
    user_id: u32,
    nick: String,
    group: String,
    avatar: Avatar,
    first_time: u64,
    messages: Vec<String>,
}

/// Check if a message is a system message (id=0, empty nick)
fn is_system_message(msg: &ChatMessage) -> bool {
    msg.id == 0 && msg.nick.is_empty()
}

/// Group consecutive messages from the same user, keeping system messages separate
fn group_messages(messages: &[ChatMessage], state: &CollaborationState) -> Vec<ChatItem> {
    let mut items: Vec<ChatItem> = Vec::new();

    for msg in messages.iter() {
        // System messages are always standalone
        if is_system_message(msg) {
            items.push(ChatItem::SystemMessage(msg.text.clone()));
            continue;
        }

        // Check if we should add to existing group or start new one
        let should_start_new = match items.last() {
            Some(ChatItem::UserMessages(g)) => g.user_id != msg.id,
            _ => true,
        };

        if should_start_new {
            items.push(ChatItem::UserMessages(MessageGroup {
                user_id: msg.id,
                nick: if msg.nick.is_empty() { fl!("collab-guest") } else { msg.nick.clone() },
                group: msg.group.clone(),
                avatar: state.user_avatar(msg.id),
                first_time: msg.time,
                messages: vec![msg.text.clone()],
            }));
        } else if let Some(ChatItem::UserMessages(group)) = items.last_mut() {
            group.messages.push(msg.text.clone());
        }
    }

    items
}

/// Format timestamp for display
fn format_time(timestamp: u64) -> String {
    if timestamp == 0 {
        return String::new();
    }

    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let msg_time = UNIX_EPOCH + Duration::from_secs(timestamp);
    let now = SystemTime::now();

    // Extract hours and minutes from timestamp
    let secs_since_epoch = timestamp;
    let hours = (secs_since_epoch / 3600) % 24;
    let minutes = (secs_since_epoch / 60) % 60;

    if let Ok(elapsed) = now.duration_since(msg_time) {
        let hours_ago = elapsed.as_secs() / 3600;
        if hours_ago < 24 {
            format!("{:02}:{:02}", hours, minutes)
        } else if hours_ago < 48 {
            format!("{} {:02}:{:02}", fl!("collab-yesterday"), hours, minutes)
        } else {
            let days = hours_ago / 24;
            fl!("collab-days-ago", days = days.to_string())
        }
    } else {
        format!("{:02}:{:02}", hours, minutes)
    }
}

// ============================================================================
// Main View
// ============================================================================

/// View the chat panel
pub fn view_chat_panel<'a>(state: &'a CollaborationState, chat_input: &'a str) -> Element<'a, Message> {
    if !state.active {
        return Space::new().width(0.0).height(0.0).into();
    }

    let user_list = view_user_list(state);
    let chat_area = view_chat_area(state.chat_messages(), state);
    let input_area = view_input_area(chat_input);

    // Chat area with input at bottom
    let main_content = column![chat_area, input_area].spacing(4).width(Length::Fill);

    // Horizontal layout: user list | chat
    let content = row![user_list, main_content].spacing(0);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        })
        //        .style(chat_panel_style)
        .into()
}

// ============================================================================
// User List (Left side)
// ============================================================================

/// View the user list (Discord-style) with own user at bottom
fn view_user_list(state: &CollaborationState) -> Element<'_, Message> {
    // Remote users list (scrollable)
    let mut user_column = Column::new().spacing(4).width(Length::Fixed(USER_LIST_WIDTH));

    // Remote user entries
    let sorted_users = state.sorted_users();
    for remote_user in sorted_users {
        let user_entry = view_user_entry(remote_user, state);
        user_column = user_column.push(user_entry);
    }

    // Show hint if no other users
    if state.remote_users().is_empty() {
        user_column = user_column.push(container(text(fl!("collab-no-other-users")).size(14).color(SECONDARY_COLOR)).padding([8, 8]));
    }

    let scrollable_users = scrollable(user_column)
        .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default()))
        .width(Length::Fixed(USER_LIST_WIDTH))
        .height(Length::Fill);

    // Own user section at bottom (like Discord)
    let own_user_section = view_own_user(state);

    column![scrollable_users, own_user_section,]
        .width(Length::Fixed(USER_LIST_WIDTH))
        .height(Length::Fill)
        .into()
}

/// View own user at the bottom (Discord-style)
fn view_own_user(state: &CollaborationState) -> Element<'_, Message> {
    let (user_id, nick, group) = match (state.our_user_id(), state.our_nick()) {
        (Some(id), Some(nick)) => (id, nick.to_string(), state.our_group().unwrap_or("").to_string()),
        _ => return Space::new().height(0.0).into(),
    };

    let avatar = state.user_avatar(user_id);
    // Own user is always "Active"
    let status = UserStatus::Active;

    let avatar_with_badge = view_avatar_with_status(avatar, status, AVATAR_SIZE, STATUS_BADGE_SIZE);

    let display_name = if nick.is_empty() { fl!("collab-you") } else { nick };

    // Name column: Nick (14pt bold) + Group (12pt secondary)
    let mut name_column = Column::new().spacing(0);
    name_column = name_column.push(text(display_name).size(14).color(BASE_COLOR).font(icy_ui::Font {
        weight: icy_ui::font::Weight::Bold,
        ..Default::default()
    }));
    if !group.is_empty() {
        name_column = name_column.push(text(group).size(12).color(SECONDARY_COLOR));
    }

    let user_row = row![avatar_with_badge, name_column].spacing(8).align_y(icy_ui::Alignment::Center);

    let user_container = container(user_row).padding([6, 8]).width(Length::Fill);

    // Separator line + own user
    column![container(Space::new().height(1.0)).width(Length::Fill).style(separator_style), user_container,]
        .spacing(4)
        .into()
}

/// View a single user entry (Discord-style: avatar with status badge + name + group)
fn view_user_entry<'a>(remote_user: &'a RemoteUser, state: &'a CollaborationState) -> Element<'a, Message> {
    let user_id = remote_user.user.id;
    let avatar = state.user_avatar(user_id);
    let status = UserStatus::from_byte(remote_user.status);

    // Avatar with status badge overlay
    let avatar_with_badge = view_avatar_with_status(avatar, status, AVATAR_SIZE, STATUS_BADGE_SIZE);

    // Username + Group
    let nick = &remote_user.user.nick;
    let display_name = if nick.is_empty() { fl!("collab-guest") } else { nick.clone() };
    let user_group = &remote_user.user.group;

    // Name column: Nick (14pt bold) + Group (12pt secondary)
    let mut name_column = Column::new().spacing(0);
    name_column = name_column.push(text(display_name).size(14).color(BASE_COLOR).font(icy_ui::Font {
        weight: icy_ui::font::Weight::Bold,
        ..Default::default()
    }));
    if !user_group.is_empty() {
        name_column = name_column.push(text(user_group.clone()).size(12).color(SECONDARY_COLOR));
    }

    // Layout: [Avatar+Badge] [Name+Group]
    let user_row = row![avatar_with_badge, name_column].spacing(8).align_y(icy_ui::Alignment::Center);

    icy_ui::widget::button(user_row)
        .on_press(Message::ChatPanel(ChatPanelMessage::GotoUser(user_id)))
        .padding([4, 6])
        .style(user_entry_style)
        .into()
}

/// Create avatar with status badge overlay (Discord-style)
fn view_avatar_with_status<'a>(avatar: Avatar, status: UserStatus, avatar_size: f32, badge_size: f32) -> Element<'a, Message> {
    let avatar_svg = container(avatar.svg(avatar_size));

    let status_svg = container(status.svg(badge_size)).style(move |_theme: &Theme| container::Style {
        background: Some(icy_ui::Background::Color(status.color())),
        border: Border::default().rounded(badge_size / 2.0),
        ..Default::default()
    });

    // Stack: avatar at base, status badge at bottom-right
    let badge_row = row![Space::new().width(avatar_size - badge_size), status_svg];
    let badge_column = column![Space::new().height(avatar_size - badge_size), badge_row];

    stack![avatar_svg, badge_column]
        .width(Length::Fixed(avatar_size))
        .height(Length::Fixed(avatar_size))
        .into()
}

// ============================================================================
// Chat Area (Right side - Discord-style)
// ============================================================================

/// View the chat messages area (Discord-style grouped messages)
fn view_chat_area<'a>(messages: &'a [ChatMessage], state: &'a CollaborationState) -> Element<'a, Message> {
    let mut content_column = Column::new().spacing(12).width(Length::Fill).padding(Padding {
        top: 8.0,
        right: 12.0,
        bottom: 4.0,
        left: 12.0,
    });

    if messages.is_empty() {
        content_column = content_column.push(text(fl!("collab-no-messages")).size(11).color(Color::from_rgb(0.5, 0.5, 0.5)));
    } else {
        // Group messages by user
        let items = group_messages(messages, state);

        // Only show last N items to avoid performance issues
        let visible_items: Vec<_> = items.into_iter().rev().take(50).collect::<Vec<_>>().into_iter().rev().collect();

        for item in visible_items {
            let item_view = view_chat_item(item);
            content_column = content_column.push(item_view);
        }
    }

    container(
        scrollable(content_column)
            .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default()))
            .anchor_bottom(),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(chat_area_style)
    .into()
}

/// View a chat item (either user message group or system message)
fn view_chat_item(item: ChatItem) -> Element<'static, Message> {
    match item {
        ChatItem::UserMessages(group) => view_message_group(group),
        ChatItem::SystemMessage(msg) => view_system_message(msg),
    }
}

/// View a system message (user joined/left, etc.) - no avatar, left-aligned
fn view_system_message(msg: String) -> Element<'static, Message> {
    text(msg)
        .size(12)
        .color(SECONDARY_COLOR)
        .font(icy_ui::Font {
            style: icy_ui::font::Style::Italic,
            ..Default::default()
        })
        .into()
}

/// View a group of messages from the same user (Discord-style)
fn view_message_group(group: MessageGroup) -> Element<'static, Message> {
    // Left column: Avatar (no backdrop)
    let avatar_column = container(group.avatar.svg(CHAT_AVATAR_SIZE)).width(Length::Fixed(CHAT_AVATAR_SIZE + 8.0));

    // Right column: Header + Messages
    let mut message_column = Column::new().spacing(2);

    // Header: Nick <Group> · Time
    // Nick in 14pt bold base color, <Group> in secondary color, Time in secondary
    let time_str = format_time(group.first_time);
    let nick = group.nick.clone();
    let user_group = group.group.clone();

    let mut header_row = Row::new().spacing(6);

    // Nick in 14pt bold
    header_row = header_row.push(text(nick).size(14).color(BASE_COLOR).font(icy_ui::Font {
        weight: icy_ui::font::Weight::Bold,
        ..Default::default()
    }));

    // <Group> in secondary color (if not empty)
    if !user_group.is_empty() {
        header_row = header_row.push(text(format!("<{}>", user_group)).size(12).color(SECONDARY_COLOR));
    }

    // · Time in secondary color (always show if available)
    if !time_str.is_empty() {
        header_row = header_row.push(text("·").size(11).color(SECONDARY_COLOR));
        header_row = header_row.push(text(time_str).size(11).color(SECONDARY_COLOR));
    }

    message_column = message_column.push(header_row);

    // Messages
    for msg_text in group.messages {
        let msg = text(msg_text).size(13);
        message_column = message_column.push(msg);
    }

    // Combine avatar + messages
    row![avatar_column, message_column.width(Length::Fill),]
        .spacing(8)
        .align_y(icy_ui::Alignment::Start)
        .into()
}

// ============================================================================
// Input Area
// ============================================================================

/// View the input area
fn view_input_area(input_text: &str) -> Element<'_, Message> {
    let input = text_input(&fl!("collab-type-message"), input_text)
        .on_input(|s| Message::ChatPanel(ChatPanelMessage::InputChanged(s)))
        .on_submit(Message::ChatPanel(ChatPanelMessage::SendMessage))
        .padding(0.0)
        .width(Length::Fill);

    container(input)
        .width(Length::Fill)
        .padding(Padding {
            top: 0.0,
            right: 8.0,
            bottom: 4.0,
            left: 0.0,
        })
        .into()
}

// ============================================================================
// Styles
// ============================================================================

#[allow(dead_code)]
fn chat_panel_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(icy_ui::Background::Color(theme.secondary.base)),
        border: Border {
            color: theme.primary.divider,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

fn separator_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(icy_ui::Background::Color(theme.primary.divider)),
        ..Default::default()
    }
}

fn user_entry_style(theme: &Theme, status: icy_ui::widget::button::Status) -> icy_ui::widget::button::Style {
    let base = icy_ui::widget::button::Style {
        background: Some(icy_ui::Background::Color(Color::TRANSPARENT)),
        text_color: theme.background.on,
        border: Border::default().rounded(4.0),
        ..Default::default()
    };

    match status {
        icy_ui::widget::button::Status::Hovered => icy_ui::widget::button::Style {
            background: Some(icy_ui::Background::Color(theme.accent.base.scale_alpha(0.2))),
            ..base
        },
        icy_ui::widget::button::Status::Pressed => icy_ui::widget::button::Style {
            background: Some(icy_ui::Background::Color(theme.accent.base.scale_alpha(0.4))),
            ..base
        },
        _ => base,
    }
}

fn chat_area_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(icy_ui::Background::Color(theme.background.base)),
        border: Border {
            color: theme.primary.divider,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}
