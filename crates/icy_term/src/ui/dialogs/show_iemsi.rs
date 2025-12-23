use i18n_embed_fl::fl;
use iced::{
    widget::{column, container, row, scrollable, text, text_input, Space},
    Alignment, Element, Length, Theme,
};
use icy_engine_gui::{
    dialog_wrapper,
    ui::{
        button_row, dialog_area, modal_container, primary_button, section_header, separator, StateResult, DIALOG_SPACING, DIALOG_WIDTH_MEDIUM,
        TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL,
    },
};
use icy_net::iemsi::EmsiISI;

const LABEL_WIDTH: f32 = 140.0;

#[derive(Debug, Clone)]
pub enum ShowIemsiMessage {
    Close,
}

#[dialog_wrapper(close_on_blur = true)]
pub struct ShowIemsiState {
    pub iemsi_info: EmsiISI,
}

impl ShowIemsiState {
    pub fn new(iemsi_info: EmsiISI) -> Self {
        Self { iemsi_info }
    }

    pub fn handle_message(&mut self, message: ShowIemsiMessage) -> StateResult<()> {
        match message {
            ShowIemsiMessage::Close => StateResult::Close,
        }
    }

    pub fn view<'a, M: Clone + 'static>(&'a self, on_message: impl Fn(ShowIemsiMessage) -> M + Clone + 'static) -> Element<'a, M> {
        let on_msg = on_message.clone();

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
        let ok_btn = primary_button(format!("{}", icy_engine_gui::ButtonType::Ok), Some(on_msg(ShowIemsiMessage::Close)));

        let buttons = button_row(vec![ok_btn.into()]);

        let dialog_content = dialog_area(content.into());
        let button_area = dialog_area(buttons.into());

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area,].into(),
            DIALOG_WIDTH_MEDIUM,
        )
        .into()
    }

    fn create_field<'a, M: Clone + 'static>(label: String, value: &str) -> Element<'a, M> {
        row![
            text(label).size(TEXT_SIZE_NORMAL).width(Length::Fixed(LABEL_WIDTH)),
            text_input("", value).size(TEXT_SIZE_NORMAL).width(Length::Fill),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center)
        .into()
    }
}

// ============================================================================
// Builder functions
// ============================================================================

/// Create an IEMSI dialog for use with DialogStack
pub fn show_iemsi_dialog<M, F, E>(iemsi_info: EmsiISI, on_message: F, extract_message: E) -> ShowIemsiWrapper<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(ShowIemsiMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&ShowIemsiMessage> + Clone + 'static,
{
    ShowIemsiWrapper::new(ShowIemsiState::new(iemsi_info), on_message, extract_message)
}

/// Creates an IEMSI dialog wrapper using a tuple of (on_message, extract_message).
pub fn show_iemsi_dialog_from_msg<M, F, E>(iemsi_info: EmsiISI, msg_tuple: (F, E)) -> ShowIemsiWrapper<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(ShowIemsiMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&ShowIemsiMessage> + Clone + 'static,
{
    show_iemsi_dialog(iemsi_info, msg_tuple.0, msg_tuple.1)
}
