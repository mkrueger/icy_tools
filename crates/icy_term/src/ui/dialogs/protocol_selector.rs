use iced::{
    Alignment, Border, Color, Element, Length, widget::{Space, button, column, container, row, rule, scrollable, text}
};
use i18n_embed_fl::fl;
use icy_net::protocol::TransferProtocolType;

use crate::ui::Message;

use once_cell::sync::Lazy;

// Text size constants
const TITLE_SIZE: u32 = 20;
const PROTOCOL_NAME_SIZE: u32 = 16;
const PROTOCOL_DESCRIPTION_SIZE: u32 = 14;

static PROTOCOL_TABLE: Lazy<[(TransferProtocolType, String, String); 8]> = Lazy::new(|| [
    (
        TransferProtocolType::ZModem,
        "Zmodem".to_string(),
        fl!(crate::LANGUAGE_LOADER, "protocol-zmodem-description")
    ),
    (
        TransferProtocolType::ZModem8k,
        "ZedZap".to_string(),
        fl!(crate::LANGUAGE_LOADER, "protocol-zmodem8k-description"),
    ),
    (
        TransferProtocolType::XModem,
        "Xmodem".to_string(),
        fl!(crate::LANGUAGE_LOADER, "protocol-xmodem-description")
    ),
    (
        TransferProtocolType::XModem1k,
        "Xmodem 1k".to_string(),
        fl!(crate::LANGUAGE_LOADER, "protocol-xmodem1k-description")
    ),
    (
        TransferProtocolType::XModem1kG,
        "Xmodem 1k-G".to_string(),
        fl!(crate::LANGUAGE_LOADER, "protocol-xmodem1kG-description")
    ),
    (
        TransferProtocolType::YModem,
        "Ymodem".to_string(),
        fl!(crate::LANGUAGE_LOADER, "protocol-ymodem-description")
    ),
    (
        TransferProtocolType::YModemG,
        "Ymodem-G".to_string(),
        fl!(crate::LANGUAGE_LOADER, "protocol-ymodemg-description")
    ),
    (
        TransferProtocolType::ASCII,
        "Text".to_string(),
        fl!(crate::LANGUAGE_LOADER, "protocol-text-description")
    )
]);

pub struct ProtocolSelector {
    pub is_download: bool,
}

impl ProtocolSelector {
    pub fn new(is_download: bool) -> Self {
        Self {
            is_download,
        }
    }

    pub fn view<'a>(&self, terminal_content: Element<'a, Message>) -> Element<'a, Message> {
        let overlay = create_modal_content(self.is_download);
        crate::ui::modal(terminal_content, overlay, Message::CloseDialog)
    }
}

fn create_modal_content(is_download: bool) -> Element<'static, Message> {
    let title = text(if is_download {
        fl!(crate::LANGUAGE_LOADER, "protocol-select-download")
    } else {
        fl!(crate::LANGUAGE_LOADER, "protocol-select-upload")
    })
    .size(TITLE_SIZE);

    // Create protocol list
    let mut protocol_rows = column![].spacing(0);
    
    for (protocol, title, descr) in PROTOCOL_TABLE.iter() {
        // Skip ASCII protocol for downloads
        if is_download && matches!(protocol, TransferProtocolType::ASCII) {
            continue;
        }

        let protocol_button = button(
            container(
                row![
                    container(
                        text(title.clone())
                            .size(PROTOCOL_NAME_SIZE)
                            .style(|theme: &iced::Theme| iced::widget::text::Style {
                                color: Some(theme.extended_palette().primary.strong.color),
                                ..Default::default()
                            })
                    )
                    .width(Length::Fixed(120.0)),
                    text(descr.clone())
                        .size(PROTOCOL_DESCRIPTION_SIZE)
                        .style(|theme: &iced::Theme| iced::widget::text::Style {
                            color: Some(theme.extended_palette().secondary.base.color),
                            ..Default::default()
                        })
                ]
                .spacing(12)
                .align_y(Alignment::Center)
            )
            .width(Length::Fill)
        )
        .on_press(Message::InitiateFileTransfer {
            protocol: protocol.clone(),
            is_download,
        })
        .width(Length::Fill)
        .style(protocol_button_style);

        protocol_rows = protocol_rows.push(protocol_button);
    }

    let cancel_button = button(
        text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-cancel-button")).size(14.0)
    )
    .on_press(Message::CloseDialog)
    .style(button::secondary);

    let modal_content = container(
        column![
            container(title).width(Length::Fill).align_x(Alignment::Center),
            container(
                scrollable(protocol_rows)
                    .direction(scrollable::Direction::Vertical(
                        scrollable::Scrollbar::default()
                    ))
            )
            .height(Length::Fixed(250.0))
            .width(Length::Fill),
            rule::horizontal(1),
            container(
                row![
                    Space::new().width(Length::Fill),
                    cancel_button
                ]
            )
        ]
        .padding(10)
        .spacing(8)
    )
    .width(Length::Fixed(400.0))
    .style(|theme: &iced::Theme| {
        let palette = theme.palette();
        container::Style {
            background: Some(iced::Background::Color(palette.background)),
            border: Border {
                color: palette.text,
                width: 1.0,
                radius: 8.0.into(),
            },
            text_color: Some(palette.text),
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 16.0,
            },
            snap: false,
        }
    });

    container(modal_content)
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
            background: Some(iced::Background::Color(
                Color::from_rgba(
                    palette.primary.weak.color.r,
                    palette.primary.weak.color.g,
                    palette.primary.weak.color.b,
                    0.2,
                )
            )),
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
pub fn view_selector(is_download: bool, terminal_content: Element<'_, Message>) -> Element<'_, Message> {
    let selector = ProtocolSelector::new(is_download);
    selector.view(terminal_content)
}