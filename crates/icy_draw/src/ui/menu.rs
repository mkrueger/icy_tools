//! Menu system for icy_draw
//!
//! Uses iced_aw's menu widget for dropdown menus.

use iced::{
    Border, Color, Element, Length, Theme, alignment,
    border::Radius,
    widget::{button, row, text},
};
use iced_aw::menu::{self, Menu};
use iced_aw::style::{Status, menu_bar::primary};
use iced_aw::{menu_bar, menu_items, quad, widget::InnerBounds};

use super::main_window::Message;
use crate::fl;
use icy_engine_gui::commands::cmd;

/// Menu bar state
#[derive(Default)]
pub struct MenuBarState;

impl MenuBarState {
    pub fn new() -> Self {
        Self
    }

    /// Build the menu bar view
    pub fn view(&self) -> Element<'_, Message> {
        let menu_template = |items| Menu::new(items).width(250.0).offset(5.0);

        let mb = menu_bar!(
            // File menu
            (
                menu_button(fl!("menu-file")),
                menu_template(menu_items!(
                    (menu_item(&cmd::FILE_NEW, Message::NewFile)),
                    (menu_item(&cmd::FILE_OPEN, Message::OpenFile)),
                    (separator()),
                    (menu_item(&cmd::FILE_SAVE, Message::SaveFile)),
                    (menu_item(&cmd::FILE_SAVE_AS, Message::SaveFileAs)),
                    (separator()),
                    (menu_item(&cmd::FILE_CLOSE, Message::CloseFile))
                ))
            ),
            // Edit menu
            (
                menu_button(fl!("menu-edit")),
                menu_template(menu_items!(
                    (menu_item(&cmd::EDIT_UNDO, Message::Undo)),
                    (menu_item(&cmd::EDIT_REDO, Message::Redo)),
                    (separator()),
                    (menu_item(&cmd::EDIT_CUT, Message::Cut)),
                    (menu_item(&cmd::EDIT_COPY, Message::Copy)),
                    (menu_item(&cmd::EDIT_PASTE, Message::Paste)),
                    (separator()),
                    (menu_item(&cmd::EDIT_SELECT_ALL, Message::SelectAll))
                ))
            ),
            // View menu
            (
                menu_button(fl!("menu-view")),
                menu_template(menu_items!(
                    (menu_item(&cmd::VIEW_ZOOM_IN, Message::ZoomIn)),
                    (menu_item(&cmd::VIEW_ZOOM_OUT, Message::ZoomOut)),
                    (menu_item(&cmd::VIEW_ZOOM_RESET, Message::ZoomReset))
                ))
            )
        )
        .spacing(4.0)
        .padding([4, 8])
        .draw_path(menu::DrawPath::Backdrop)
        .style(|theme: &Theme, status: Status| {
            let palette = theme.extended_palette();
            menu::Style {
                path_border: Border {
                    radius: Radius::new(6.0),
                    ..Default::default()
                },
                path: Color::from_rgb(
                    palette.primary.weak.color.r * 1.2,
                    palette.primary.weak.color.g * 1.2,
                    palette.primary.weak.color.b * 1.2,
                )
                .into(),
                ..primary(theme, status)
            }
        });

        mb.into()
    }
}

/// Create a menu bar button
fn menu_button(label: String) -> Element<'static, Message> {
    button(text(label).size(14))
        .padding([4, 8])
        .style(menu_button_style)
        .on_press(Message::Tick) // Dummy message - menu handles the interaction
        .into()
}

/// Create a menu item from a command definition
fn menu_item(cmd: &icy_engine_gui::commands::CommandDef, msg: Message) -> Element<'static, Message> {
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

/// Create a separator line
/*fn separator() -> Element<'static, Message> {
    container(rule::horizontal(1))
        .padding([4, 0])
        .width(Length::Fill)
        .into()
}*/
fn separator() -> quad::Quad {
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

// ============================================================================
// Styles
// ============================================================================

fn menu_button_style(theme: &Theme, status: button::Status) -> button::Style {
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

fn menu_item_style(theme: &Theme, status: button::Status) -> button::Style {
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
