use crate::ui::Message;
use crate::ui::dialing_directory_dialog::DialingDirectoryMsg;
use i18n_embed_fl::fl;
use iced::{
    Alignment, Element, Length,
    widget::{Space, button, column, container, row, rule, text},
};

impl super::DialingDirectoryState {
    pub fn delete_confirmation_modal(&self, idx: usize) -> Element<'_, Message> {
        let system_name = if idx < self.addresses.addresses.len() {
            &self.addresses.addresses[idx].system_name
        } else {
            "Unknown"
        };

        let title = text(fl!(crate::LANGUAGE_LOADER, "delete-bbs-title")).size(22);

        let question = text(fl!(crate::LANGUAGE_LOADER, "delete-bbs-question", system = system_name))
            .wrapping(text::Wrapping::WordOrGlyph)
            .size(16);

        let delete_btn = button(
            text(fl!(crate::LANGUAGE_LOADER, "delete-bbs-delete-button")).style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().danger.base.color),
                ..Default::default()
            }),
        )
        .on_press(Message::from(DialingDirectoryMsg::ConfirmDelete(idx)))
        .style(|theme: &iced::Theme, status| {
            let palette = theme.extended_palette();
            let base = button::Style {
                background: Some(iced::Background::Color(palette.background.base.color)),
                border: iced::Border {
                    color: palette.danger.base.color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                text_color: palette.danger.base.color,
                shadow: Default::default(),
                snap: false,
            };

            match status {
                button::Status::Hovered => button::Style {
                    background: Some(iced::Background::Color(palette.danger.weak.color)),
                    text_color: palette.background.base.color,
                    ..base
                },
                button::Status::Pressed => button::Style {
                    background: Some(iced::Background::Color(palette.danger.strong.color)),
                    text_color: palette.background.base.color,
                    ..base
                },
                _ => base,
            }
        });

        let cancel_btn = button(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-cancel-button")))
            .on_press(Message::from(DialingDirectoryMsg::Cancel))
            .style(button::secondary);

        let modal_content = container(
            column![
                title.width(Length::Fill).align_x(Alignment::Center),
                rule::horizontal(1),
                question.width(Length::Fixed(440.0)),
                Space::new().height(Length::Fixed(24.0)),
                row![Space::new().width(Length::Fill), cancel_btn, delete_btn].spacing(12)
            ]
            .padding(20)
            .spacing(8),
        )
        .width(Length::Fixed(480.0))
        .style(|theme: &iced::Theme| {
            let palette = theme.palette();
            container::Style {
                background: Some(iced::Background::Color(palette.background)),
                border: iced::Border {
                    color: palette.text,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                text_color: Some(palette.text),
                shadow: iced::Shadow {
                    color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.5),
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
}
