use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length,
    widget::{Space, button, column, container, row, scrollable, text},
};
use icy_engine_gui::ui::*;

use crate::{
    TransferProtocol,
    ui::{MainWindowMode, Message},
};

// Text size constants
const PROTOCOL_NAME_SIZE: u32 = 16;
const PROTOCOL_DESCRIPTION_SIZE: u32 = 14;

pub struct ProtocolSelector {
    pub is_download: bool,
    pub protocols: Vec<TransferProtocol>,
}

impl ProtocolSelector {
    pub fn new(is_download: bool, protocols: Vec<TransferProtocol>) -> Self {
        Self { is_download, protocols }
    }

    pub fn view<'a>(&self, terminal_content: Element<'a, Message>) -> Element<'a, Message> {
        let overlay = create_modal_content(self.is_download, &self.protocols);
        crate::ui::modal(
            terminal_content,
            overlay,
            crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)),
        )
    }
}

fn get_protocol_description(protocol_id: &str) -> String {
    match protocol_id {
        "@zmodem" => fl!(crate::LANGUAGE_LOADER, "protocol-zmodem-description"),
        "@zmodem8k" => fl!(crate::LANGUAGE_LOADER, "protocol-zmodem8k-description"),
        "@xmodem" => fl!(crate::LANGUAGE_LOADER, "protocol-xmodem-description"),
        "@xmodem1k" => fl!(crate::LANGUAGE_LOADER, "protocol-xmodem1k-description"),
        "@xmodem1kg" => fl!(crate::LANGUAGE_LOADER, "protocol-xmodem1kG-description"),
        "@ymodem" => fl!(crate::LANGUAGE_LOADER, "protocol-ymodem-description"),
        "@ymodemg" => fl!(crate::LANGUAGE_LOADER, "protocol-ymodemg-description"),
        "@text" => fl!(crate::LANGUAGE_LOADER, "protocol-text-description"),
        _ => String::new(),
    }
}

fn create_modal_content(is_download: bool, protocols: &[TransferProtocol]) -> Element<'static, Message> {
    let title = dialog_title(if is_download {
        fl!(crate::LANGUAGE_LOADER, "protocol-select-download")
    } else {
        fl!(crate::LANGUAGE_LOADER, "protocol-select-upload")
    });

    // Filter to only enabled protocols
    let enabled_protocols: Vec<_> = protocols
        .iter()
        .filter(|p| p.enabled)
        .filter(|p| !(is_download && p.id == "@text")) // Skip Text protocol for downloads
        .collect();

    // Create protocol list
    let mut protocol_rows = column![].spacing(0);

    for protocol in enabled_protocols {
        let description = get_protocol_description(&protocol.id);

        let protocol_button = button(
            container(
                row![
                    container(
                        text(protocol.get_name())
                            .size(PROTOCOL_NAME_SIZE)
                            .style(|theme: &iced::Theme| iced::widget::text::Style {
                                color: Some(theme.extended_palette().primary.strong.color),
                                ..Default::default()
                            })
                    )
                    .width(Length::Fixed(120.0)),
                    text(description)
                        .size(PROTOCOL_DESCRIPTION_SIZE)
                        .style(|theme: &iced::Theme| iced::widget::text::Style {
                            color: Some(theme.extended_palette().secondary.base.color),
                            ..Default::default()
                        })
                ]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill),
        )
        .on_press(Message::InitiateFileTransfer {
            protocol: protocol.clone(),
            is_download,
        })
        .width(Length::Fill)
        .style(protocol_button_style);

        protocol_rows = protocol_rows.push(protocol_button);
    }

    let cancel_button = secondary_button(
        format!("{}", icy_engine_gui::ButtonType::Cancel),
        Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal))),
    );

    let protocol_list = icy_engine_gui::settings::effect_box(
        scrollable(protocol_rows)
            .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default()))
            .into(),
    );

    let dialog_content = dialog_area(
        column![
            title,
            Space::new().height(DIALOG_SPACING),
            container(protocol_list).height(Length::Fill).width(Length::Fill),
        ]
        .into(),
    );

    let button_area = dialog_area(button_row(vec![cancel_button.into()]));

    let modal = modal_container(
        column![container(dialog_content).height(Length::Fill), separator(), button_area,].into(),
        DIALOG_WIDTH_MEDIUM,
    );

    container(modal)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

fn protocol_button_style(theme: &iced::Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let base = button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        text_color: palette.background.base.text,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 4.0.into(),
        },
        shadow: Default::default(),
        snap: false,
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(iced::Background::Color(Color::from_rgba(
                palette.primary.weak.color.r,
                palette.primary.weak.color.g,
                palette.primary.weak.color.b,
                0.2,
            ))),
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(iced::Background::Color(palette.primary.weak.color)),
            ..base
        },
        button::Status::Disabled => button::Style {
            text_color: Color::from_rgba(
                palette.background.base.text.r,
                palette.background.base.text.g,
                palette.background.base.text.b,
                0.5,
            ),
            ..base
        },
    }
}

// Helper function to create the selector and wrap terminal content
pub fn view_selector(is_download: bool, protocols: Vec<TransferProtocol>, terminal_content: Element<'_, Message>) -> Element<'_, Message> {
    let selector = ProtocolSelector::new(is_download, protocols);
    selector.view(terminal_content)
}
