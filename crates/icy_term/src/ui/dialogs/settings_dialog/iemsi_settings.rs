use i18n_embed_fl::fl;
use iced::{
    Alignment, Element, Length,
    widget::{checkbox, column, row, text_input},
};
use iced_engine_gui::{
    SECTION_PADDING, section_header,
    settings::{effect_box_toggleable, left_label},
    ui::DIALOG_SPACING as INPUT_SPACING,
};

use crate::ui::settings_dialog::{SettingsDialogState, SettingsMsg};

impl SettingsDialogState {
    pub fn iemsi_settings_content<'a>(&self) -> Element<'a, crate::ui::Message> {
        let iemsi = &self.temp_options.lock().unwrap().iemsi;
        column![
            // IEMSI Autologin section
            section_header(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-autologin-section")),
            effect_box_toggleable(
                column![
                    // Enabled checkbox
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-enabled-checkbox")),
                        checkbox(iemsi.autologin)
                            .on_toggle({
                                let temp_options = self.temp_options.clone();
                                move |checked| {
                                    let mut new_options = (*temp_options.lock().unwrap()).clone();
                                    new_options.iemsi.autologin = checked;
                                    crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                }
                            })
                            .size(18),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                    // Alias
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-alias")),
                        text_input("", &iemsi.alias)
                            .on_input({
                                let temp_options = self.temp_options.clone();
                                move |value| {
                                    let mut new_options = (*temp_options.lock().unwrap()).clone();
                                    new_options.iemsi.alias = value;
                                    crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                }
                            })
                            .width(Length::Fill)
                            .size(14),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                    // Location
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-location")),
                        text_input("", &iemsi.location)
                            .on_input({
                                let temp_options = self.temp_options.clone();
                                move |value| {
                                    let mut new_options = (*temp_options.lock().unwrap()).clone();
                                    new_options.iemsi.location = value;
                                    crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                }
                            })
                            .width(Length::Fill)
                            .size(14),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                    // Data Phone
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-data-phone")),
                        text_input("", &iemsi.data_phone)
                            .on_input({
                                let temp_options = self.temp_options.clone();
                                move |value| {
                                    let mut new_options = (*temp_options.lock().unwrap()).clone();
                                    new_options.iemsi.data_phone = value;
                                    crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                }
                            })
                            .width(Length::Fill)
                            .size(14),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                    // Voice Phone
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-voice-phone")),
                        text_input("", &iemsi.voice_phone)
                            .on_input({
                                let temp_options = self.temp_options.clone();
                                move |value| {
                                    let mut new_options = (*temp_options.lock().unwrap()).clone();
                                    new_options.iemsi.voice_phone = value;
                                    crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                }
                            })
                            .width(Length::Fill)
                            .size(14),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                    // Birth Date
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-birth-date")),
                        text_input("", &iemsi.birth_date)
                            .on_input({
                                let temp_options = self.temp_options.clone();
                                move |value| {
                                    let mut new_options = (*temp_options.lock().unwrap()).clone();
                                    new_options.iemsi.birth_date = value;
                                    crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                }
                            })
                            .width(Length::Fill)
                            .size(14),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                ]
                .spacing(INPUT_SPACING)
                .into(), // Convert column to Element
                !iemsi.autologin // Disable if not enabled
            ),
        ]
        .padding(SECTION_PADDING as u16)
        .width(Length::Fill)
        .into()
    }
}
