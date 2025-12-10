//! Menu system for icy_draw
//!
//! Uses iced_aw's menu widget for dropdown menus.
//! Ported from the egui version in src_egui/ui/top_bar.rs

use iced::{
    Border, Color, Element, Length, Theme, alignment,
    border::Radius,
    widget::{button, row, text},
};
use iced_aw::menu::Menu;
use iced_aw::{quad, widget::InnerBounds};

use super::MostRecentlyUsedFiles;
use super::main_window::{EditMode, Message};
use crate::{
    fl,
    ui::{animation_editor, ansi_editor, bitfont_editor},
};

/// Information about current undo/redo state for menu display
#[derive(Default, Clone)]
pub struct UndoInfo {
    /// Description of the next undo operation (None if nothing to undo)
    pub undo_description: Option<String>,
    /// Description of the next redo operation (None if nothing to redo)
    pub redo_description: Option<String>,
}

impl UndoInfo {
    pub fn new(undo_description: Option<String>, redo_description: Option<String>) -> Self {
        Self {
            undo_description,
            redo_description,
        }
    }
}

/// Menu bar state
#[derive(Default)]
pub struct MenuBarState;

impl MenuBarState {
    pub fn new() -> Self {
        Self
    }

    /// Build the menu bar view based on the current edit mode
    pub fn view(&self, mode: &EditMode, recent_files: &MostRecentlyUsedFiles, undo_info: &UndoInfo) -> Element<'_, Message> {
        match mode {
            EditMode::Ansi => ansi_editor::menu_bar::view_ansi(recent_files, undo_info),
            EditMode::BitFont => {
                bitfont_editor::menu_bar::view_bitfont(recent_files, undo_info.undo_description.as_deref(), undo_info.redo_description.as_deref())
            }
            EditMode::CharFont => ansi_editor::menu_bar::view_ansi(recent_files, undo_info), // TODO: Create separate charfont menu
            EditMode::Animation => animation_editor::menu_bar::view_animation_menu(recent_files, undo_info),
        }
    }
}

// ============================================================================
// Public helper functions for editor menu modules
// ============================================================================

/// Create a menu bar button
pub fn menu_button(label: String) -> Element<'static, Message> {
    button(text(label).size(14))
        .padding([4, 8])
        .style(menu_button_style)
        .on_press(Message::Tick) // Dummy message - menu handles the interaction
        .into()
}

/// Create a menu item from a command definition
pub fn menu_item(cmd: &icy_engine_gui::commands::CommandDef, msg: Message) -> Element<'static, Message> {
    let label = if cmd.label_menu.is_empty() { cmd.id.clone() } else { cmd.label_menu.clone() };

    let hotkey = cmd.primary_hotkey_display().unwrap_or_default();

    button(
        row![
            text(label).size(14).width(Length::Fill),
            text(hotkey).size(12).style(|theme: &Theme| {
                iced::widget::text::Style {
                    color: Some(theme.palette().text.scale_alpha(0.6)),
                }
            }),
        ]
        .spacing(16)
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .padding([4, 8])
    .style(menu_item_style)
    .on_press(msg)
    .into()
}

/// Create an Undo menu item with optional operation description
pub fn menu_item_undo(cmd: &icy_engine_gui::commands::CommandDef, msg: Message, description: Option<&str>) -> Element<'static, Message> {
    let base_label = if cmd.label_menu.is_empty() { cmd.id.clone() } else { cmd.label_menu.clone() };
    let label = match description {
        Some(desc) => format!("{} {}", base_label, desc),
        None => base_label,
    };
    let hotkey = cmd.primary_hotkey_display().unwrap_or_default();
    let is_enabled = description.is_some();

    let btn = button(
        row![
            text(label).size(14).width(Length::Fill),
            text(hotkey).size(12).style(|theme: &Theme| {
                iced::widget::text::Style {
                    color: Some(theme.palette().text.scale_alpha(0.6)),
                }
            }),
        ]
        .spacing(16)
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .padding([4, 8])
    .style(if is_enabled { menu_item_style } else { menu_item_disabled_style });

    if is_enabled { btn.on_press(msg).into() } else { btn.into() }
}

/// Create a Redo menu item with optional operation description
pub fn menu_item_redo(cmd: &icy_engine_gui::commands::CommandDef, msg: Message, description: Option<&str>) -> Element<'static, Message> {
    let base_label = if cmd.label_menu.is_empty() { cmd.id.clone() } else { cmd.label_menu.clone() };
    let label = match description {
        Some(desc) => format!("{} {}", base_label, desc),
        None => base_label,
    };
    let hotkey = cmd.primary_hotkey_display().unwrap_or_default();
    let is_enabled = description.is_some();

    let btn = button(
        row![
            text(label).size(14).width(Length::Fill),
            text(hotkey).size(12).style(|theme: &Theme| {
                iced::widget::text::Style {
                    color: Some(theme.palette().text.scale_alpha(0.6)),
                }
            }),
        ]
        .spacing(16)
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .padding([4, 8])
    .style(if is_enabled { menu_item_style } else { menu_item_disabled_style });

    if is_enabled { btn.on_press(msg).into() } else { btn.into() }
}

/// Create a menu item with direct label and hotkey (without CommandDef)
pub fn menu_item_simple(label: String, hotkey: &str, msg: Message) -> Element<'static, Message> {
    let hotkey_text = hotkey.to_string();

    button(
        row![
            text(label).size(14).width(Length::Fill),
            text(hotkey_text).size(12).style(|theme: &Theme| {
                iced::widget::text::Style {
                    color: Some(theme.palette().text.scale_alpha(0.6)),
                }
            }),
        ]
        .spacing(16)
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .padding([4, 8])
    .style(menu_item_style)
    .on_press(msg)
    .into()
}

/// Create a separator line
pub fn separator() -> quad::Quad {
    quad::Quad {
        quad_color: Color::from([0.5; 3]).into(),
        quad_border: Border {
            radius: Radius::new(5.0),
            ..Default::default()
        },
        inner_bounds: InnerBounds::Ratio(0.98, 0.2),
        height: Length::Fixed(4.0),
        ..Default::default()
    }
}

/// Create a submenu item (with arrow indicator)
pub fn menu_item_submenu(label: String) -> Element<'static, Message> {
    button(
        row![
            text(label).size(14).width(Length::Fill),
            text("▶").size(12).style(|theme: &Theme| {
                iced::widget::text::Style {
                    color: Some(theme.palette().text.scale_alpha(0.6)),
                }
            }),
        ]
        .spacing(16)
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .padding([4, 8])
    .style(menu_item_style)
    .on_press(Message::Tick) // Dummy - submenu handles interaction
    .into()
}

/// Build the recent files submenu
pub fn build_recent_files_menu(recent_files: &MostRecentlyUsedFiles) -> Menu<'static, Message, Theme, iced::Renderer> {
    let files = recent_files.files();

    let mut items: Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>> = Vec::new();

    if files.is_empty() {
        // Show "No recent files" when empty
        items.push(iced_aw::menu::Item::new(
            button(text(fl!("menu-no_recent_files")).size(14))
                .width(Length::Fill)
                .padding([4, 8])
                .style(menu_item_disabled_style),
        ));
    } else {
        // Show files in reverse order (most recent first)
        for file in files.iter().rev() {
            let file_name = file
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| file.display().to_string());
            let path = file.clone();

            items.push(iced_aw::menu::Item::new(
                button(text(file_name).size(14))
                    .width(Length::Fill)
                    .padding([4, 8])
                    .style(menu_item_style)
                    .on_press(Message::OpenRecentFile(path)),
            ));
        }

        // Add separator and clear option
        items.push(iced_aw::menu::Item::new(separator()));
        items.push(iced_aw::menu::Item::new(
            button(text(fl!("menu-clear_recent_files")).size(14))
                .width(Length::Fill)
                .padding([4, 8])
                .style(menu_item_style)
                .on_press(Message::ClearRecentFiles),
        ));
    }

    Menu::new(items).width(350.0).offset(0.0)
}

// ============================================================================
// Styles
// ============================================================================

pub fn menu_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        text_color: palette.background.base.text,
        border: Border::default().rounded(6.0),
        ..Default::default()
    };
    match status {
        button::Status::Active => base.with_background(Color::TRANSPARENT),
        button::Status::Hovered => base.with_background(Color::from_rgb(
            palette.primary.weak.color.r * 1.2,
            palette.primary.weak.color.g * 1.2,
            palette.primary.weak.color.b * 1.2,
        )),
        button::Status::Pressed => base.with_background(palette.primary.weak.color),
        button::Status::Disabled => base.with_background(Color::from_rgb(0.5, 0.5, 0.5)),
    }
}

pub fn menu_item_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        text_color: palette.background.base.text,
        border: Border::default().rounded(6.0),
        ..Default::default()
    };

    match status {
        button::Status::Active => base.with_background(Color::TRANSPARENT),
        button::Status::Hovered => base.with_background(Color::from_rgb(
            palette.primary.weak.color.r * 1.2,
            palette.primary.weak.color.g * 1.2,
            palette.primary.weak.color.b * 1.2,
        )),
        button::Status::Pressed => base.with_background(palette.primary.weak.color),
        button::Status::Disabled => base.with_background(Color::from_rgb(0.5, 0.5, 0.5)),
    }
}

pub fn menu_item_disabled_style(theme: &Theme, _status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    button::Style {
        text_color: palette.background.base.text.scale_alpha(0.5),
        border: Border::default().rounded(6.0),
        background: Some(Color::TRANSPARENT.into()),
        ..Default::default()
    }
}
