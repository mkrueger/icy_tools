use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length,
    widget::{Space, column, container, row, scrollable, text, text_input},
};
use iced_engine_gui::ui::primary_button;
use icy_net::iemsi::EmsiISI;

use crate::ui::MainWindowMode;

const MODAL_WIDTH: f32 = 500.0;
const MODAL_HEIGHT: f32 = 520.0;
const LABEL_WIDTH: f32 = 90.0;
const ICON_WIDTH: f32 = 28.0;

#[derive(Debug, Clone)]
pub enum IemsiMsg {
    Close,
}

pub struct ShowIemsiDialog {
    pub iemsi_info: EmsiISI,
}

impl ShowIemsiDialog {
    pub fn new(iemsi_info: EmsiISI) -> Self {
        Self { iemsi_info }
    }

    pub fn update(&mut self, message: IemsiMsg) -> Option<crate::ui::Message> {
        match message {
            IemsiMsg::Close => Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal))),
        }
    }

    pub fn view<'a>(&'a self, terminal_content: Element<'a, crate::ui::Message>) -> Element<'a, crate::ui::Message> {
        let overlay = self.create_modal_content();
        crate::ui::modal(terminal_content, overlay, crate::ui::Message::ShowIemsi(IemsiMsg::Close))
    }

    fn create_field<'a>(label: String, value: &str, icon: &'a str) -> Element<'a, crate::ui::Message> {
        // Icon column (fixed width)
        let icon_col = container(text(icon).size(16).style(|theme: &iced::Theme| {
            let palette = theme.extended_palette();
            text::Style {
                color: Some(palette.primary.weak.color),
            }
        }))
        .width(Length::Fixed(ICON_WIDTH))
        .align_x(Alignment::Center);

        // Label column (fixed width)
        let label_col = container(text(label).size(13).style(|theme: &iced::Theme| text::Style {
            color: Some(theme.palette().text.scale_alpha(0.6)),
        }))
        .width(Length::Fixed(LABEL_WIDTH));

        // Value/input column (fills remaining space)
        let value_col = container(
            text_input("", value)
                .width(Length::Fill)
                .padding(8)
                .size(14)
                .style(|theme: &iced::Theme, status| {
                    let palette = theme.extended_palette();
                    let focused = matches!(status, text_input::Status::Focused { .. });
                    text_input::Style {
                        background: iced::Background::Color(if focused {
                            Color::from_rgba(0.1, 0.1, 0.2, 0.15)
                        } else {
                            Color::from_rgba(0.0, 0.0, 0.0, 0.08)
                        }),
                        border: Border {
                            color: if focused {
                                palette.primary.base.color
                            } else {
                                Color::from_rgba(0.0, 0.0, 0.0, 0.0)
                            },
                            width: 1.0, // keep constant to avoid subtle width shifts
                            radius: 4.0.into(),
                        },
                        icon: theme.palette().text,
                        placeholder: Color::from_rgba(0.5, 0.5, 0.5, 0.5),
                        value: theme.palette().text,
                        selection: palette.primary.strong.color,
                    }
                }),
        )
        .width(Length::Fill);

        // Assemble row with consistent spacing; no ad‑hoc Space elements
        let row_line = row![icon_col, label_col, value_col].spacing(8).align_y(Alignment::Center);

        container(row_line).padding([6.0, 12.0]).into()
    }

    fn create_modal_content(&self) -> Element<'_, crate::ui::Message> {
        // Header with icon
        let header = container(
            column![
                row![
                    text("🖥️").size(24),
                    Space::new().width(8.0),
                    text(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-heading"))
                        .size(20)
                        .style(|theme: &iced::Theme| {
                            let palette = theme.extended_palette();
                            text::Style {
                                color: Some(palette.primary.base.color),
                            }
                        }),
                ]
                .align_y(Alignment::Center),
            ]
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .padding(16.0);

        // Main system info
        let system_info = container(
            column![
                Self::create_field(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-name"), &self.iemsi_info.name, "📟"),
                Self::create_field(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-location"), &self.iemsi_info.location, "📍"),
                Self::create_field(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-operator"), &self.iemsi_info.operator, "👤"),
                Self::create_field(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-id"), &self.iemsi_info.id, "🔑"),
                Self::create_field(
                    fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-capabilities"),
                    &self.iemsi_info.capabilities,
                    "⚙️"
                ),
            ]
            .spacing(2),
        )
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(Color::from_rgba(0.0, 0.0, 0.1, 0.03))),
            border: Border {
                color: Color::from_rgba(0.5, 0.5, 0.6, 0.15),
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
        .padding(4);

        // Notice section - expandable for longer text
        let notice_section = column![
            // Separator line using a thin container
            container(Space::new().height(1.0))
                .width(Length::Fill)
                .height(Length::Fixed(1.0))
                .style(|_theme: &iced::Theme| {
                    container::Style {
                        background: Some(iced::Background::Color(Color::from_rgba(0.5, 0.5, 0.5, 0.15))),
                        ..Default::default()
                    }
                }),
            Space::new().height(12.0),
            row![
                text("📋").size(16).style(|theme: &iced::Theme| {
                    let palette = theme.extended_palette();
                    text::Style {
                        color: Some(palette.primary.weak.color),
                    }
                }),
                Space::new().width(8.0),
                text(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-notice"))
                    .size(13)
                    .style(|theme: &iced::Theme| {
                        text::Style {
                            color: Some(theme.palette().text.scale_alpha(0.6)),
                        }
                    }),
            ]
            .padding([0, 12])
            .align_y(Alignment::Center),
            Space::new().height(6.0),
            container(
                scrollable(
                    container(text(&self.iemsi_info.notice).size(13).style(|theme: &iced::Theme| {
                        text::Style {
                            color: Some(theme.palette().text.scale_alpha(0.9)),
                        }
                    }))
                    .width(Length::Fill)
                    .padding(12)
                )
                .height(Length::Fixed(70.0))
                .width(Length::Fill)
            )
            .style(|_theme: &iced::Theme| {
                container::Style {
                    background: Some(iced::Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.06))),
                    border: Border {
                        color: Color::from_rgba(0.5, 0.5, 0.6, 0.1),
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                }
            })
            .padding([0, 12]),
        ];

        // Styled OK button
        let ok_button = primary_button(
            format!("{}", iced_engine_gui::ButtonType::Ok),
            Some(crate::ui::Message::ShowIemsi(IemsiMsg::Close)),
        )
        .padding(10.0);

        let button_row = row![Space::new().width(Length::Fill), ok_button,].padding([12, 0]);

        // Main modal content
        let modal_content = container(column![header, system_info, notice_section, Space::new().height(Length::Fill), button_row,].padding([20, 24]))
            .width(Length::Fixed(MODAL_WIDTH))
            .height(Length::Fixed(MODAL_HEIGHT))
            .style(|theme: &iced::Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(iced::Background::Color(theme.palette().background)),
                    border: Border {
                        color: palette.primary.weak.color.scale_alpha(0.3),
                        width: 1.0,
                        radius: 12.0.into(),
                    },
                    text_color: Some(theme.palette().text),
                    shadow: iced::Shadow {
                        color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                        offset: iced::Vector::new(0.0, 8.0),
                        blur_radius: 20.0,
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
