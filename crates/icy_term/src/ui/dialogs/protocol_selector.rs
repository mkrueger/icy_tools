use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length,
    widget::{Space, button, column, container, row, scrollable, text},
};
use iced_engine_gui::ui::*;
use icy_net::protocol::TransferProtocolType;

use crate::ui::{MainWindowMode, Message};

use once_cell::sync::Lazy;

// Text size constants
const PROTOCOL_NAME_SIZE: u32 = 16;
const PROTOCOL_DESCRIPTION_SIZE: u32 = 14;

static PROTOCOL_TABLE: Lazy<[(TransferProtocolType, String, String); 8]> = Lazy::new(|| {
    [
        (
            TransferProtocolType::ZModem,
            "Zmodem".to_string(),
            fl!(crate::LANGUAGE_LOADER, "protocol-zmodem-description"),
        ),
        (
            TransferProtocolType::ZModem8k,
            "ZedZap".to_string(),
            fl!(crate::LANGUAGE_LOADER, "protocol-zmodem8k-description"),
        ),
        (
            TransferProtocolType::XModem,
            "Xmodem".to_string(),
            fl!(crate::LANGUAGE_LOADER, "protocol-xmodem-description"),
        ),
        (
            TransferProtocolType::XModem1k,
            "Xmodem 1k".to_string(),
            fl!(crate::LANGUAGE_LOADER, "protocol-xmodem1k-description"),
        ),
        (
            TransferProtocolType::XModem1kG,
            "Xmodem 1k-G".to_string(),
            fl!(crate::LANGUAGE_LOADER, "protocol-xmodem1kG-description"),
        ),
        (
            TransferProtocolType::YModem,
            "Ymodem".to_string(),
            fl!(crate::LANGUAGE_LOADER, "protocol-ymodem-description"),
        ),
        (
            TransferProtocolType::YModemG,
            "Ymodem-G".to_string(),
            fl!(crate::LANGUAGE_LOADER, "protocol-ymodemg-description"),
        ),
        (
            TransferProtocolType::ASCII,
            "Text".to_string(),
            fl!(crate::LANGUAGE_LOADER, "protocol-text-description"),
        ),
    ]
});

pub struct ProtocolSelector {
    pub is_download: bool,
}

impl ProtocolSelector {
    pub fn new(is_download: bool) -> Self {
        Self { is_download }
    }

    pub fn view<'a>(&self, terminal_content: Element<'a, Message>) -> Element<'a, Message> {
        let overlay = create_modal_content(self.is_download);
        crate::ui::modal(
            terminal_content,
            overlay,
            crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)),
        )
    }
}

fn create_modal_content(is_download: bool) -> Element<'static, Message> {
    let title = dialog_title(if is_download {
        fl!(crate::LANGUAGE_LOADER, "protocol-select-download")
    } else {
        fl!(crate::LANGUAGE_LOADER, "protocol-select-upload")
    });

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
        format!("{}", iced_engine_gui::ButtonType::Cancel),
        Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal))),
    );

    let protocol_list = iced_engine_gui::settings::effect_box(
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
pub fn view_selector(is_download: bool, terminal_content: Element<'_, Message>) -> Element<'_, Message> {
    let selector = ProtocolSelector::new(is_download);
    selector.view(terminal_content)
}
