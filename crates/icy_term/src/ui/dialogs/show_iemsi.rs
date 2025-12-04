use i18n_embed_fl::fl;
use iced::{
    Alignment, Element, Length, Theme,
    widget::{Space, column, container, row, scrollable, text, text_input},
};
use icy_engine_gui::ui::{
    DIALOG_SPACING, DIALOG_WIDTH_MEDIUM, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL, button_row, dialog_area, modal_container, primary_button, section_header, separator,
};
use icy_net::iemsi::EmsiISI;

use crate::ui::MainWindowMode;

const LABEL_WIDTH: f32 = 140.0;

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

    fn create_field(label: String, value: &str) -> Element<'_, crate::ui::Message> {
        row![
            text(label).size(TEXT_SIZE_NORMAL).width(Length::Fixed(LABEL_WIDTH)),
            text_input("", value).size(TEXT_SIZE_NORMAL).width(Length::Fill),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center)
        .into()
    }

    fn create_modal_content(&self) -> Element<'_, crate::ui::Message> {
        // System info section
        let system_section = column![
            section_header(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-heading")),
            Space::new().height(DIALOG_SPACING),
            Self::create_field(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-name"), &self.iemsi_info.name),
            Self::create_field(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-location"), &self.iemsi_info.location),
            Self::create_field(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-operator"), &self.iemsi_info.operator),
            Self::create_field(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-id"), &self.iemsi_info.id),
            Self::create_field(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-capabilities"), &self.iemsi_info.capabilities),
        ]
        .spacing(4.0);

        // Notice section
        let notice_section = column![
            text(fl!(crate::LANGUAGE_LOADER, "show-iemsi-dialog-notice"))
                .size(TEXT_SIZE_SMALL)
                .style(|theme: &Theme| text::Style {
                    color: Some(theme.palette().text.scale_alpha(0.7)),
                }),
            Space::new().height(4.0),
            container(
                scrollable(container(text(&self.iemsi_info.notice).size(TEXT_SIZE_NORMAL)).width(Length::Fill).padding(8))
                    .height(Length::Fixed(80.0))
                    .width(Length::Fill)
            )
            .style(container::rounded_box),
        ];

        let content = column![system_section, Space::new().height(DIALOG_SPACING), notice_section,];

        // OK button
        let ok_btn = primary_button(
            format!("{}", icy_engine_gui::ButtonType::Ok),
            Some(crate::ui::Message::ShowIemsi(IemsiMsg::Close)),
        );

        let buttons = button_row(vec![ok_btn.into()]);

        let dialog_content = dialog_area(content.into());
        let button_area = dialog_area(buttons.into());

        let modal = modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area,].into(),
            DIALOG_WIDTH_MEDIUM,
        );

        container(modal)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}
