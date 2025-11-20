use crate::ui::MainWindowMode;
use i18n_embed_fl::fl;
use iced::{
    Alignment, Element, Length,
    widget::{Space, column, container, row, scrollable, text},
};
use iced_engine_gui::ui::*;

pub fn view<'a>(
    terminal_content: Element<'a, crate::ui::Message>,
    title: &'a str,
    secondary_msg: &'a str,
    error_msg: &'a str,
) -> Element<'a, crate::ui::Message> {
    // Header icon element (not a closure — avoids From<{closure}> error)
    let header_icon = container(text("⚠").size(22).style(|_| iced::widget::text::Style {
        // Use danger text color for contrast
        color: Some(iced::Color::WHITE),
    }))
    .width(36)
    .height(36)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(|t: &iced::Theme| container::Style {
        background: Some(t.extended_palette().danger.base.color.into()),
        border: iced::Border {
            color: t.extended_palette().danger.base.color,
            width: 0.0,
            radius: 18.0.into(),
        },
        ..Default::default()
    });

    // Title + secondary description
    let title_row = row![
        header_icon,
        Space::new().width(12),
        column![
            text(title)
                .size(20)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..iced::Font::default()
                })
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().background.strong.text)
                }),
        ]
        .spacing(4)
        .width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    // Technical details block (scrollable)
    let details_block = {
        let raw = text(error_msg).size(13).style(|theme: &iced::Theme| iced::widget::text::Style {
            color: Some(theme.extended_palette().background.strong.text),
        });

        let sc = scrollable(
            container(raw)
                .padding([6, 8])
                .style(|theme: &iced::Theme| container::Style {
                    background: Some(theme.extended_palette().background.weak.color.into()),
                    border: iced::Border {
                        color: theme.extended_palette().background.strong.color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                })
                .width(Length::Fill),
        )
        .height(Length::Fixed(140.0));

        column![
            text(secondary_msg).size(14).style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().secondary.strong.color)
            }),
            sc
        ]
        .spacing(6)
        .width(Length::Fill)
    };

    // Actions (removed Copy button because variant doesn't exist)
    let close_btn = primary_button(
        fl!(crate::LANGUAGE_LOADER, "dialog-close_button"),
        Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal))),
    );

    let button_area = button_row(vec![close_btn.into()]);

    // Dialog container
    let dialog_content = dialog_area(column![title_row, details_block, Space::new().height(DIALOG_SPACING)].into());

    let button_area_wrapped = dialog_area(button_area);

    let modal = modal_container(
        column![container(dialog_content).height(Length::Fill), separator(), button_area_wrapped,].into(),
        DIALOG_WIDTH_MEDIUM,
    );

    // Overlay
    let overlay = container(Space::new()).width(Length::Fill).height(Length::Fill).style(|_| container::Style {
        background: Some(iced::Color::from_rgba8(0, 0, 0, 0.55).into()),
        ..Default::default()
    });

    container(iced::widget::stack![
        terminal_content,
        overlay,
        container(modal)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
