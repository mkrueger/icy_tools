use i18n_embed_fl::fl;
use icy_ui::widget::{button, column, container, row, space, text, tooltip, Space};
use icy_ui::{Background, Border, Color, Element, Length, Padding, Shadow, Theme};

mod icons;
pub use icons::*;

mod dialog;
pub use dialog::*;

mod sauce_helpers;
pub use sauce_helpers::*;

mod confirmation_dialog;
pub use confirmation_dialog::*;

mod about_dialog;
pub use about_dialog::*;

mod toast;
pub use toast::*;

mod export_dialog;
pub use export_dialog::*;

mod help_dialog;
pub use help_dialog::*;

pub mod menu;
pub use menu::{Menu, MenuBar, MenuItem};

pub mod window_manager;
pub use window_manager::*;

pub mod version_helper;

// Button styling
pub const BUTTON_FONT_SIZE: f32 = 14.0;
pub const BUTTON_BORDER_WIDTH: f32 = 1.0;
pub const BUTTON_BORDER_RADIUS: f32 = 5.0;

pub const LABEL_SMALL_WIDTH: f32 = 120.0;
pub const LABEL_WIDTH: f32 = 180.0;
pub const SECTION_PADDING: f32 = 20.0;
pub const SECTION_SPACING: f32 = 24.0;
pub const EFFECT_BOX_PADDING: u16 = 16;
pub const EFFECT_BOX_RADIUS: f32 = 6.0;
pub const SLIDER_SPACING: f32 = 8.0;
pub const TOGGLE_SPACING: f32 = 10.0;
pub const TEXT_SIZE_NORMAL: f32 = 14.0;
pub const TEXT_SIZE_SMALL: f32 = 12.0;
pub const HEADER_TEXT_SIZE: f32 = 16.0;

// Modal dialog sizing
pub const DIALOG_WIDTH_SMALL: f32 = 340.0;
pub const DIALOG_WIDTH_MEDIUM: f32 = 500.0;
pub const DIALOG_WIDTH_LARGE: f32 = 550.0;
pub const DIALOG_WIDTH_XARGLE: f32 = 680.0;

// Dialog component heights
const SEPARATOR_HEIGHT: f32 = 1.0;
const BUTTON_ROW_HEIGHT: f32 = 24.0; // button height with padding [6, 12]

// Dialog-specific spacing
pub const DIALOG_SPACING: f32 = 8.0;
// Generic spacing scale
pub const SPACE_4: f32 = 4.0;
pub const SPACE_8: f32 = 8.0;
pub const SPACE_16: f32 = 16.0;
pub const DIALOG_PADDING: u16 = 16;
pub const MODAL_PADDING: u16 = 0;

// Button sizing
pub const BUTTON_PADDING_NORMAL: [u16; 2] = [6, 12];

// Border/Shadow constants
pub const DIALOG_BORDER_WIDTH: f32 = 1.0;
pub const DIALOG_BORDER_RADIUS: f32 = 8.0;
pub const SEPARATOR_ALPHA: f32 = 0.06;

pub fn dialog_title<T: 'static>(title: String) -> Element<'static, T> {
    text(title)
        .size(HEADER_TEXT_SIZE)
        .font(icy_ui::Font {
            weight: icy_ui::font::Weight::Bold,
            ..icy_ui::Font::default()
        })
        .into()
}

pub fn separator<'a, T: 'a>() -> Element<'a, T> {
    container(Space::new())
        .width(Length::Fill)
        .height(1.0)
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(theme.background.on.scale_alpha(SEPARATOR_ALPHA))),
            ..Default::default()
        })
        .into()
}

/// Calculate the total dialog height from the content area height
/// Formula: content_area_height + (2 * DIALOG_PADDING) + separator_height + button_row_height + (2 * DIALOG_PADDING)
pub const fn calculate_dialog_height(content_area_height: f32) -> f32 {
    content_area_height
        + (DIALOG_PADDING as f32 * 2.0)  // top dialog_area padding
        + SEPARATOR_HEIGHT
        + BUTTON_ROW_HEIGHT
        + (DIALOG_PADDING as f32 * 2.0) // bottom dialog_area padding
}

pub fn dialog_area<'a, T: 'a>(content: Element<'a, T>) -> Element<'a, T> {
    container(content).padding(DIALOG_PADDING).width(Length::Fill).into()
}

pub fn modal_container<'a, T: 'a>(content: Element<'a, T>, width: f32) -> container::Container<'a, T> {
    container(content)
        .width(Length::Fixed(width))
        .height(Length::Shrink)
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(theme.background.base)),
            border: Border {
                color: theme.background.on,
                width: DIALOG_BORDER_WIDTH,
                radius: DIALOG_BORDER_RADIUS.into(),
            },
            text_color: Some(theme.background.on),
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: icy_ui::Vector::new(0.0, 4.0),
                blur_radius: 8.0,
            },
            snap: false,
        })
}

pub fn modal_overlay<'a, Message: 'a + Clone>(background: Element<'a, Message>, modal: Element<'a, Message>) -> Element<'a, Message> {
    // Overlay that blocks clicks to the background
    let overlay = icy_ui::widget::opaque(container(Space::new()).width(Length::Fill).height(Length::Fill).style(|_| container::Style {
        background: Some(Color::from_rgba8(0, 0, 0, 0.55).into()),
        ..Default::default()
    }));

    container(icy_ui::widget::stack![
        background,
        overlay,
        container(modal)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(icy_ui::alignment::Horizontal::Center)
            .align_y(icy_ui::alignment::Vertical::Center),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// Create a modal dialog overlay with a semi-transparent background that closes on click outside
pub fn modal<'a, Message>(base: impl Into<Element<'a, Message>>, content: impl Into<Element<'a, Message>>, on_blur: Message) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    use icy_ui::widget::{center, mouse_area, opaque, stack};

    stack![
        base.into(),
        opaque(
            mouse_area(center(opaque(content)).style(|_theme| {
                container::Style {
                    background: Some(Color { a: 0.8, ..Color::BLACK }.into()),
                    ..container::Style::default()
                }
            }))
            .on_press(on_blur)
        )
    ]
    .into()
}

pub fn warning_tooltip<'a, Message: 'a>(error_text: String) -> Element<'a, Message> {
    use crate::ui::icons::warning_icon;

    tooltip(
        warning_icon(18.0).style(|theme: &Theme, _status| icy_ui::widget::svg::Style {
            color: Some(theme.warning.base),
        }),
        container(text(error_text)).style(container::rounded_box),
        tooltip::Position::Top,
    )
    .into()
}

pub fn error_tooltip<'a, Message: 'a>(error_text: String) -> Element<'a, Message> {
    use crate::ui::icons::error_icon;

    tooltip(
        error_icon(18.0).style(|theme: &Theme, _status| icy_ui::widget::svg::Style {
            color: Some(theme.destructive.base),
        }),
        container(text(error_text)).style(container::rounded_box),
        tooltip::Position::Top,
    )
    .into()
}

// Button style functions
pub fn primary_button_style(theme: &Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Active | button::Status::Selected => button::Style {
            background: Some(theme.accent.base.into()),
            text_color: theme.accent.on,
            border: Border {
                color: theme.accent.hover,
                width: BUTTON_BORDER_WIDTH,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
        button::Status::Hovered => button::Style {
            background: Some(theme.accent.hover.into()),
            text_color: theme.accent.on,
            border: Border {
                color: theme.accent.hover,
                width: BUTTON_BORDER_WIDTH,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
        button::Status::Pressed => button::Style {
            background: Some(theme.accent.pressed.into()),
            text_color: theme.accent.on,
            border: Border {
                color: theme.accent.hover,
                width: BUTTON_BORDER_WIDTH,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
        button::Status::Disabled => button::Style {
            background: Some(theme.accent.disabled.into()),
            text_color: theme.accent.on_disabled,
            border: Border {
                color: theme.secondary.base,
                width: BUTTON_BORDER_WIDTH,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
    }
}

pub fn secondary_button_style(theme: &Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Active | button::Status::Selected => button::Style {
            background: Some(theme.button.base.into()),
            text_color: theme.button.on,
            border: Border {
                color: theme.button.hover,
                width: BUTTON_BORDER_WIDTH,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
        button::Status::Hovered => button::Style {
            background: Some(theme.button.hover.into()),
            text_color: theme.button.on,
            border: Border {
                color: theme.button.hover,
                width: BUTTON_BORDER_WIDTH,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
        button::Status::Pressed => button::Style {
            background: Some(theme.button.pressed.into()),
            text_color: theme.button.on,
            border: Border {
                color: theme.button.hover,
                width: BUTTON_BORDER_WIDTH,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
        button::Status::Disabled => button::Style {
            background: Some(theme.button.disabled.into()),
            text_color: theme.button.on_disabled,
            border: Border {
                color: theme.secondary.base,
                width: BUTTON_BORDER_WIDTH,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
    }
}

pub fn danger_button_style(theme: &Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Active | button::Status::Selected => button::Style {
            background: Some(theme.destructive.base.into()),
            text_color: theme.destructive.on,
            border: Border {
                color: theme.destructive.hover,
                width: BUTTON_BORDER_WIDTH,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
        button::Status::Hovered => button::Style {
            background: Some(theme.destructive.hover.into()),
            text_color: theme.destructive.on,
            border: Border {
                color: theme.destructive.hover,
                width: BUTTON_BORDER_WIDTH,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
        button::Status::Pressed => button::Style {
            background: Some(theme.destructive.pressed.into()),
            text_color: theme.destructive.on,
            border: Border {
                color: theme.destructive.hover,
                width: BUTTON_BORDER_WIDTH,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
        button::Status::Disabled => button::Style {
            background: Some(theme.destructive.disabled.into()),
            text_color: theme.destructive.on_disabled,
            border: Border {
                color: theme.secondary.base,
                width: BUTTON_BORDER_WIDTH,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
    }
}

pub fn success_button_style(theme: &Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Active | button::Status::Selected => button::Style {
            background: Some(theme.success.base.into()),
            text_color: theme.success.on,
            border: Border {
                color: theme.success.hover,
                width: BUTTON_BORDER_WIDTH,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
        button::Status::Hovered => button::Style {
            background: Some(theme.success.hover.into()),
            text_color: theme.success.on,
            border: Border {
                color: theme.success.hover,
                width: BUTTON_BORDER_WIDTH,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
        button::Status::Pressed => button::Style {
            background: Some(theme.success.pressed.into()),
            text_color: theme.success.on,
            border: Border {
                color: theme.success.hover,
                width: BUTTON_BORDER_WIDTH,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
        button::Status::Disabled => button::Style {
            background: Some(theme.success.disabled.into()),
            text_color: theme.success.on_disabled,
            border: Border {
                color: theme.secondary.base,
                width: BUTTON_BORDER_WIDTH,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
    }
}

pub fn text_button_style(theme: &Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Active | button::Status::Selected => button::Style {
            background: None,
            text_color: theme.accent.hover,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
        button::Status::Hovered => button::Style {
            background: Some(theme.accent.base.scale_alpha(0.1).into()),
            text_color: theme.accent.hover,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
        button::Status::Pressed => button::Style {
            background: Some(theme.accent.base.scale_alpha(0.2).into()),
            text_color: theme.accent.base,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
        button::Status::Disabled => button::Style {
            background: None,
            text_color: theme.primary.on.scale_alpha(0.5),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            ..Default::default()
        },
    }
}

pub fn primary_button<'a, Message: Clone + 'a>(label: impl Into<String>, on_press: Option<Message>) -> button::Button<'a, Message> {
    let mut btn = button(text(label.into()).size(BUTTON_FONT_SIZE).wrapping(text::Wrapping::None))
        .padding(BUTTON_PADDING_NORMAL)
        .style(primary_button_style);

    if let Some(msg) = on_press {
        btn = btn.on_press(msg);
    }
    btn
}

pub fn secondary_button<'a, Message: Clone + 'a>(label: impl Into<String>, on_press: Option<Message>) -> button::Button<'a, Message> {
    let mut btn = button(text(label.into()).size(BUTTON_FONT_SIZE).wrapping(text::Wrapping::None))
        .padding(BUTTON_PADDING_NORMAL)
        .style(secondary_button_style);

    if let Some(msg) = on_press {
        btn = btn.on_press(msg);
    }
    btn
}

pub fn restore_defaults_button<'a, Message: Clone + 'a>(is_sensitive: bool, on_press: Message) -> button::Button<'a, Message> {
    let label = fl!(crate::LANGUAGE_LOADER, "settings-restore-defaults-button");
    let mut btn = button(text(label).size(BUTTON_FONT_SIZE).wrapping(text::Wrapping::None))
        .padding(BUTTON_PADDING_NORMAL)
        .style(secondary_button_style);

    if is_sensitive {
        btn = btn.on_press(on_press);
    }
    btn
}

pub fn danger_button<'a, Message: Clone + 'a>(label: impl Into<String>, on_press: Option<Message>) -> button::Button<'a, Message> {
    let mut btn = button(text(label.into()).size(BUTTON_FONT_SIZE).wrapping(text::Wrapping::None))
        .padding(BUTTON_PADDING_NORMAL)
        .style(danger_button_style);

    if let Some(msg) = on_press {
        btn = btn.on_press(msg);
    }
    btn
}

pub fn success_button<'a, Message: Clone + 'a>(label: impl Into<String>, on_press: Option<Message>) -> button::Button<'a, Message> {
    let mut btn = button(text(label.into()).size(BUTTON_FONT_SIZE).wrapping(text::Wrapping::None))
        .padding(BUTTON_PADDING_NORMAL)
        .style(success_button_style);

    if let Some(msg) = on_press {
        btn = btn.on_press(msg);
    }
    btn
}

pub fn text_button<'a, Message: Clone + 'a>(label: impl Into<String>, on_press: Option<Message>) -> button::Button<'a, Message> {
    let mut btn = button(text(label.into()).size(BUTTON_FONT_SIZE).wrapping(text::Wrapping::None))
        .padding(BUTTON_PADDING_NORMAL)
        .style(text_button_style);

    if let Some(msg) = on_press {
        btn = btn.on_press(msg);
    }
    btn
}

pub fn browse_button<'a, Message: Clone + 'a>(on_press: Message) -> button::Button<'a, Message> {
    button(text("…").size(BUTTON_FONT_SIZE))
        .on_press(on_press)
        .padding(BUTTON_PADDING_NORMAL)
        .style(secondary_button_style)
}

pub fn button_row<'a, Message: 'a>(buttons: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    use icy_ui::widget::row;

    let mut row_widget = row![Space::new().width(Length::Fill)].padding(Padding {
        left: SPACE_8,
        right: SPACE_8,
        top: 0.0,
        bottom: 0.0,
    });

    for (i, button) in buttons.into_iter().enumerate() {
        if i > 0 {
            row_widget = row_widget.push(Space::new().width(DIALOG_SPACING));
        }
        row_widget = row_widget.push(button);
    }

    row_widget.into()
}

pub fn button_row_with_left<'a, Message: 'a>(left_buttons: Vec<Element<'a, Message>>, right_buttons: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    use icy_ui::widget::row;

    let mut row_widget = row![].padding(Padding {
        left: SPACE_8,
        right: SPACE_8,
        top: 0.0,
        bottom: 0.0,
    });

    // Add left buttons
    for (i, button) in left_buttons.into_iter().enumerate() {
        if i > 0 {
            row_widget = row_widget.push(Space::new().width(DIALOG_SPACING));
        }
        row_widget = row_widget.push(button);
    }

    // Add fill space
    row_widget = row_widget.push(Space::new().width(Length::Fill));

    // Add right buttons
    for (i, button) in right_buttons.into_iter().enumerate() {
        if i > 0 {
            row_widget = row_widget.push(Space::new().width(DIALOG_SPACING));
        }
        row_widget = row_widget.push(button);
    }

    row_widget.into()
}

// Section header with styling
pub fn section_header<T: 'static>(title: String) -> Element<'static, T> {
    column![
        row![
            space().width(8.0),
            text(title)
                .size(TEXT_SIZE_NORMAL)
                .font(icy_ui::Font {
                    weight: icy_ui::font::Weight::Bold,
                    ..icy_ui::Font::default()
                })
                .style(|theme: &Theme| {
                    text::Style {
                        color: Some(theme.background.on),
                    }
                }),
        ],
        space().height(4),
    ]
    .spacing(4)
    .into()
}

pub fn left_label_small<T: 'static>(txt: String) -> Element<'static, T> {
    container(text(txt).size(TEXT_SIZE_NORMAL))
        .width(Length::Fixed(LABEL_SMALL_WIDTH))
        .align_x(icy_ui::alignment::Horizontal::Left)
        .into()
}
