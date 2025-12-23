use i18n_embed_fl::fl;
use iced::{
    widget::{checkbox, column, row, text_input},
    Alignment, Element, Length,
};
use icy_engine_gui::{
    section_header,
    settings::{effect_box_toggleable, left_label},
    ui::{DIALOG_SPACING, TEXT_SIZE_NORMAL},
    SECTION_PADDING,
};

use crate::ui::settings_dialog::{SettingsDialogMessage, SettingsDialogState};

impl SettingsDialogState {
    pub fn iemsi_settings_content_generic<'a, M: Clone + 'static>(&self, on_message: impl Fn(SettingsDialogMessage) -> M + Clone + 'static) -> Element<'a, M> {
        let iemsi = &self.temp_options.lock().iemsi;
        let iemsi_autologin = iemsi.autologin;
        let iemsi_alias = iemsi.alias.clone();
        let iemsi_location = iemsi.location.clone();
        let iemsi_data_phone = iemsi.data_phone.clone();
        let iemsi_voice_phone = iemsi.voice_phone.clone();
        let iemsi_birth_date = iemsi.birth_date.clone();

        let temp_options = self.temp_options.clone();
        let temp_options2 = self.temp_options.clone();
        let temp_options3 = self.temp_options.clone();
        let temp_options4 = self.temp_options.clone();
        let temp_options5 = self.temp_options.clone();
        let temp_options6 = self.temp_options.clone();

        let on_msg = on_message.clone();
        let on_msg2 = on_message.clone();
        let on_msg3 = on_message.clone();
        let on_msg4 = on_message.clone();
        let on_msg5 = on_message.clone();
        let on_msg6 = on_message.clone();

        column![
            // IEMSI Autologin section
            section_header(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-autologin-section")),
            effect_box_toggleable(
                column![
                    // Enabled checkbox
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-enabled-checkbox")),
                        checkbox(iemsi_autologin)
                            .on_toggle(move |checked| {
                                let mut new_options = (*temp_options.lock()).clone();
                                new_options.iemsi.autologin = checked;
                                on_msg(SettingsDialogMessage::UpdateOptions(new_options))
                            })
                            .size(18),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    // Alias
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-alias")),
                        text_input("", &iemsi_alias)
                            .on_input(move |value| {
                                let mut new_options = (*temp_options2.lock()).clone();
                                new_options.iemsi.alias = value;
                                on_msg2(SettingsDialogMessage::UpdateOptions(new_options))
                            })
                            .width(Length::Fill)
                            .size(TEXT_SIZE_NORMAL),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    // Location
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-location")),
                        text_input("", &iemsi_location)
                            .on_input(move |value| {
                                let mut new_options = (*temp_options3.lock()).clone();
                                new_options.iemsi.location = value;
                                on_msg3(SettingsDialogMessage::UpdateOptions(new_options))
                            })
                            .width(Length::Fill)
                            .size(TEXT_SIZE_NORMAL),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    // Data Phone
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-data-phone")),
                        text_input("", &iemsi_data_phone)
                            .on_input(move |value| {
                                let mut new_options = (*temp_options4.lock()).clone();
                                new_options.iemsi.data_phone = value;
                                on_msg4(SettingsDialogMessage::UpdateOptions(new_options))
                            })
                            .width(Length::Fill)
                            .size(TEXT_SIZE_NORMAL),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    // Voice Phone
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-voice-phone")),
                        text_input("", &iemsi_voice_phone)
                            .on_input(move |value| {
                                let mut new_options = (*temp_options5.lock()).clone();
                                new_options.iemsi.voice_phone = value;
                                on_msg5(SettingsDialogMessage::UpdateOptions(new_options))
                            })
                            .width(Length::Fill)
                            .size(TEXT_SIZE_NORMAL),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    // Birth Date
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-birth-date")),
                        text_input("", &iemsi_birth_date)
                            .on_input(move |value| {
                                let mut new_options = (*temp_options6.lock()).clone();
                                new_options.iemsi.birth_date = value;
                                on_msg6(SettingsDialogMessage::UpdateOptions(new_options))
                            })
                            .width(Length::Fill)
                            .size(TEXT_SIZE_NORMAL),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                ]
                .spacing(DIALOG_SPACING)
                .into(), // Convert column to Element
                !iemsi_autologin // Disable if not enabled
            ),
        ]
        .padding(SECTION_PADDING as u16)
        .width(Length::Fill)
        .into()
    }
}
