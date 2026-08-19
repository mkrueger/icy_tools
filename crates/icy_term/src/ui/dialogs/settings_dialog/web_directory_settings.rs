use i18n_embed_fl::fl;
use icy_engine_gui::{
    section_header,
    settings::{effect_box, left_label},
    ui::*,
    SECTION_PADDING,
};
use icy_ui::{
    widget::{button, checkbox, column, container, row, scrollable, text, text_input, Space},
    Alignment, Element, Length,
};

use super::{SettingsDialogMessage, SettingsDialogState};

impl SettingsDialogState {
    pub fn web_directory_settings_content_generic<'a, M: Clone + 'static>(
        &self,
        on_message: impl Fn(SettingsDialogMessage) -> M + Clone + 'static,
    ) -> Element<'a, M> {
        let sources = self.temp_options.lock().web_directories.clone();
        let selected = self.selected_web_directory_index;
        let mut list = column![].spacing(2);
        for (index, source) in sources.iter().enumerate() {
            let on_msg = on_message.clone();
            list = list.push(
                button(text(source.name.clone()).size(TEXT_SIZE_NORMAL))
                    .on_press(on_msg(SettingsDialogMessage::SelectWebDirectory(index)))
                    .width(Length::Fill)
                    .style(if index == selected { button::primary } else { button::secondary }),
            );
        }

        let add = secondary_button(
            fl!(crate::LANGUAGE_LOADER, "settings-web-directory-add"),
            Some(on_message(SettingsDialogMessage::AddWebDirectory)),
        );
        let remove = secondary_button(
            fl!(crate::LANGUAGE_LOADER, "settings-web-directory-remove"),
            sources.get(selected).map(|_| on_message(SettingsDialogMessage::RemoveWebDirectory(selected))),
        );
        let list_panel = column![scrollable(list).height(Length::Fill), row![add, remove].spacing(DIALOG_SPACING)]
            .spacing(DIALOG_SPACING)
            .width(Length::Fixed(220.0));

        let editor: Element<'_, M> = if let Some(source) = sources.get(selected) {
            let on_name = on_message.clone();
            let on_url = on_message.clone();
            let on_enabled = on_message.clone();
            effect_box(
                column![
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-web-directory-name")),
                        text_input("Community", &source.name.clone())
                            .on_input(move |value| on_name(SettingsDialogMessage::UpdateWebDirectoryName(selected, value)))
                            .width(Length::Fill),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-web-directory-url")),
                        text_input("https://example.org/phonebook.toml", &source.url.clone())
                            .on_input(move |value| on_url(SettingsDialogMessage::UpdateWebDirectoryUrl(selected, value)))
                            .width(Length::Fill),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-web-directory-enabled")),
                        checkbox(source.enabled)
                            .on_toggle(move |enabled| on_enabled(SettingsDialogMessage::ToggleWebDirectory(selected, enabled)))
                            .size(18),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    text(fl!(crate::LANGUAGE_LOADER, "settings-web-directory-restart"))
                        .size(TEXT_SIZE_SMALL)
                        .style(text::secondary),
                ]
                .spacing(DIALOG_SPACING)
                .into(),
            )
            .into()
        } else {
            container(Space::new()).width(Length::Fill).height(Length::Fill).into()
        };

        column![
            section_header(fl!(crate::LANGUAGE_LOADER, "settings-web-directory-category")),
            row![list_panel, editor].spacing(DIALOG_SPACING).height(Length::Fill),
        ]
        .padding(SECTION_PADDING as u16)
        .height(Length::Fill)
        .into()
    }
}
