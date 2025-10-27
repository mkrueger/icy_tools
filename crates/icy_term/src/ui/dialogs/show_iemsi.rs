use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length,
    widget::{Space, button, column, container, row, text, text_input},
};
use icy_net::iemsi::EmsiISI;

const MODAL_WIDTH: f32 = 450.0;
const MODAL_HEIGHT: f32 = 380.0;
const LABEL_WIDTH: f32 = 100.0;

#[derive(Debug, Clone)]
pub enum IemsiMsg {
    Close,
}

pub struct ShowIemsiDialog {
    pub iemsi_info: Option<EmsiISI>,
}

impl ShowIemsiDialog {
    pub fn new(iemsi_info: Option<EmsiISI>) -> Self {
        Self { iemsi_info }
    }

    pub fn update(&mut self, message: IemsiMsg) -> Option<crate::ui::Message> {
        match message {
            IemsiMsg::Close => Some(crate::ui::Message::CloseDialog),
        }
    }

    pub fn view<'a>(&'a self, terminal_content: Element<'a, crate::ui::Message>) -> Element<'a, crate::ui::Message> {
        let overlay = self.create_modal_content();
        crate::ui::modal(terminal_content, overlay, crate::ui::Message::ShowIemsi(IemsiMsg::Close))
    }

    fn create_modal_content(&self) -> Element<'_, crate::ui::Message> {
        let title = text(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-heading"))
            .size(20)
            .align_x(Alignment::Center)
            .width(Length::Fill);

        let content = if let Some(ref iemsi) = self.iemsi_info {
            // Create rows for each IEMSI field
            column![
                // Name
                row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-name")).size(14))
                        .width(Length::Fixed(LABEL_WIDTH))
                        .align_x(Alignment::End),
                    Space::new().width(8.0),
                    text_input("", &iemsi.name).width(Length::Fill).padding(6),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                // Location
                row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-location")).size(14))
                        .width(Length::Fixed(LABEL_WIDTH))
                        .align_x(Alignment::End),
                    Space::new().width(8.0),
                    text_input("", &iemsi.location).width(Length::Fill).padding(6),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                // Operator
                row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-operator")).size(14))
                        .width(Length::Fixed(LABEL_WIDTH))
                        .align_x(Alignment::End),
                    Space::new().width(8.0),
                    text_input("", &iemsi.operator).width(Length::Fill).padding(6),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                // Notice
                row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-notice")).size(14))
                        .width(Length::Fixed(LABEL_WIDTH))
                        .align_x(Alignment::End),
                    Space::new().width(8.0),
                    text_input("", &iemsi.notice).width(Length::Fill).padding(6),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                // Capabilities
                row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-capabilities")).size(14))
                        .width(Length::Fixed(LABEL_WIDTH))
                        .align_x(Alignment::End),
                    Space::new().width(8.0),
                    text_input("", &iemsi.capabilities).width(Length::Fill).padding(6),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                // ID
                row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-id")).size(14))
                        .width(Length::Fixed(LABEL_WIDTH))
                        .align_x(Alignment::End),
                    Space::new().width(8.0),
                    text_input("", &iemsi.id).width(Length::Fill).padding(6),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ]
            .spacing(12)
        } else {
            column![
                container(text("No IEMSI information available").size(14))
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .padding(40),
            ]
        };

        // OK button
        let ok_button = button(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-ok-button")))
            .on_press(crate::ui::Message::ShowIemsi(IemsiMsg::Close))
            .padding([8, 16])
            .style(button::primary);

        let button_row = row![Space::new().width(Length::Fill), ok_button,];

        // Main modal content
        let modal_content = container(
            column![
                title,
                Space::new().height(12.0),
                container(content).width(Length::Fill).padding([0, 10]),
                Space::new().height(Length::Fill),
                button_row,
            ]
            .padding(10),
        )
        .width(Length::Fixed(MODAL_WIDTH))
        .height(Length::Fixed(MODAL_HEIGHT))
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
}
