use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length, Theme,
    widget::{Space, column, container, row, text},
};
use std::fmt;

use crate::{
    DIALOG_SPACING, DIALOG_WIDTH_MEDIUM, LANGUAGE_LOADER, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL, button_row, danger_button, dialog_area, modal_container,
    modal_overlay, primary_button, secondary_button,
    ui::icons::{error_icon, warning_icon},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSet {
    Ok,
    Close,
    OkCancel,
    YesNo,
    YesNoCancel,
    DeleteCancel,
}

impl ButtonSet {
    /// Returns buttons in correct order with appropriate styles
    pub fn to_buttons(&self) -> Vec<(ButtonType, ButtonStyle)> {
        match self {
            Self::Ok => vec![(ButtonType::Ok, ButtonStyle::Primary)],
            Self::Close => vec![(ButtonType::Close, ButtonStyle::Secondary)],
            Self::OkCancel => vec![(ButtonType::Cancel, ButtonStyle::Secondary), (ButtonType::Ok, ButtonStyle::Primary)],
            Self::YesNo => vec![(ButtonType::No, ButtonStyle::Secondary), (ButtonType::Yes, ButtonStyle::Primary)],
            Self::YesNoCancel => vec![
                (ButtonType::Cancel, ButtonStyle::Secondary),
                (ButtonType::No, ButtonStyle::Secondary),
                (ButtonType::Yes, ButtonStyle::Primary),
            ],
            Self::DeleteCancel => vec![(ButtonType::Cancel, ButtonStyle::Secondary), (ButtonType::Delete, ButtonStyle::Danger)],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonType {
    Ok,
    Cancel,
    Yes,
    No,
    Close,
    Delete,
}

impl fmt::Display for ButtonType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::Ok => fl!(LANGUAGE_LOADER, "dialog-ok-button"),
            Self::Cancel => fl!(LANGUAGE_LOADER, "dialog-cancel-button"),
            Self::Yes => fl!(LANGUAGE_LOADER, "dialog-yes-button"),
            Self::No => fl!(LANGUAGE_LOADER, "dialog-no-button"),
            Self::Close => fl!(LANGUAGE_LOADER, "dialog-close-button"),
            Self::Delete => fl!(LANGUAGE_LOADER, "dialog-delete-button"),
        };
        write!(f, "{}", text)
    }
}

impl ButtonType {
    pub fn to_result(&self) -> DialogResult {
        match self {
            Self::Ok => DialogResult::Ok,
            Self::Cancel => DialogResult::Cancel,
            Self::Yes => DialogResult::Yes,
            Self::No => DialogResult::No,
            Self::Close => DialogResult::Close,
            Self::Delete => DialogResult::Delete,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogType {
    Plain,
    Error,
    Warning,
    Info,
    Question,
}

impl DialogType {
    pub fn icon<'a>(&self) -> Option<Element<'a, Theme>> {
        let c = *self;
        match self {
            DialogType::Plain => None,
            DialogType::Error => Some(
                error_icon(48.0)
                    .style(move |theme: &Theme, _status| {
                        let color = c.icon_color(theme);
                        iced::widget::svg::Style { color: Some(color) }
                    })
                    .into(),
            ),
            DialogType::Warning => Some(
                warning_icon(48.0)
                    .style(move |theme: &Theme, _status| {
                        let color = c.icon_color(theme);
                        iced::widget::svg::Style { color: Some(color) }
                    })
                    .into(),
            ),
            DialogType::Info => None,
            DialogType::Question => None,
        }
    }

    pub fn icon_color(&self, theme: &Theme) -> Color {
        let palette = theme.extended_palette();
        match self {
            DialogType::Plain => Color::TRANSPARENT,
            DialogType::Error => palette.danger.base.color,
            DialogType::Warning => palette.warning.base.color,
            DialogType::Info => palette.primary.base.color,
            DialogType::Question => palette.primary.base.color,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogResult {
    Ok,
    Cancel,
    Yes,
    No,
    Close,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    Primary,
    Secondary,
    Danger,
}

#[derive(Debug, Clone)]
pub struct ConfirmationDialog {
    pub dialog_type: DialogType,
    pub title: String,
    pub secondary_message: Option<String>,
    pub primary_message: String,
    pub buttons: ButtonSet,
}

impl Default for ConfirmationDialog {
    fn default() -> Self {
        Self {
            dialog_type: DialogType::Plain,
            title: String::new(),
            primary_message: String::new(),
            secondary_message: None,
            buttons: ButtonSet::Ok,
        }
    }
}

impl ConfirmationDialog {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            dialog_type: DialogType::Plain,
            title: title.into(),
            primary_message: message.into(),
            secondary_message: None,
            buttons: ButtonSet::Ok,
        }
    }

    pub fn dialog_type(mut self, dialog_type: DialogType) -> Self {
        self.dialog_type = dialog_type;
        self
    }

    pub fn secondary_message(mut self, msg: impl Into<String>) -> Self {
        self.secondary_message = Some(msg.into());
        self
    }

    pub fn buttons(mut self, buttons: ButtonSet) -> Self {
        self.buttons = buttons;
        self
    }
    pub fn view<'a, Message: 'a + Clone>(self, background: Element<'a, Message>, on_result: impl Fn(DialogResult) -> Message + 'a) -> Element<'a, Message> {
        let mut text_column = column![text(self.title.clone()).size(22.0).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..iced::Font::default()
        })];

        if let Some(secondary) = self.secondary_message.clone() {
            text_column = text_column.push(text(secondary).size(TEXT_SIZE_SMALL).style(|theme: &Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().secondary.base.color),
            }));
        }

        // Build header with icon and title/secondary message side by side
        let header = if let Some(icon_elem) = self.dialog_type.icon() {
            let icon_container = container(icon_elem).style(|_theme: &Theme| container::Style {
                background: None,
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });
            row![icon_container, Space::new().width(12.0), text_column.width(Length::Fill)].align_y(Alignment::Center)
        } else {
            row![text_column.width(Length::Fill)]
        };

        // Main content area with primary message
        let content = column![
            header,
            Space::new().height(DIALOG_SPACING),
            text(self.primary_message.clone()).size(TEXT_SIZE_NORMAL)
        ]
        .spacing(DIALOG_SPACING);

        // Assembly - convert Theme elements to Message elements
        let dialog_content: Element<'a, Message> = dialog_area(content.into()).map(|_| unreachable!());
        let buttons_row: Element<'a, Message> = button_row(
            self.buttons
                .to_buttons()
                .into_iter()
                .map(|(button_type, style)| {
                    let msg = on_result(button_type.to_result());
                    let label = button_type.to_string();
                    match style {
                        ButtonStyle::Primary => primary_button(&label, Some(msg)),
                        ButtonStyle::Secondary => secondary_button(&label, Some(msg)),
                        ButtonStyle::Danger => danger_button(&label, Some(msg)),
                    }
                    .into()
                })
                .collect(),
        );

        let button_area: Element<'a, Message> = dialog_area(buttons_row);
        let modal = modal_container(column![dialog_content, button_area].into(), DIALOG_WIDTH_MEDIUM);

        // Overlay + centering
        modal_overlay::<Message>(background, modal.into())
    }
}
