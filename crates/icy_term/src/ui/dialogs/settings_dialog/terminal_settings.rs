use i18n_embed_fl::fl;
use iced::{
    Alignment, Element, Length,
    widget::{Space, button, checkbox, column, row, text},
};
use iced_engine_gui::settings::{LABEL_WIDTH, SECTION_PADDING, effect_box, left_label};

use crate::ui::settings_dialog::{INPUT_SPACING, SettingsDialogState, SettingsMsg};

impl SettingsDialogState {
    pub fn terminal_settings_content<'a>(&self) -> Element<'a, crate::ui::Message> {
        let temp_options = self.temp_options.lock().unwrap();
        let console_beep = temp_options.console_beep;
        drop(temp_options); // Release the lock early

        column![effect_box(
            column![
                // Console Beep
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "settings-terminal-console-beep-checkbox")),
                    checkbox("", console_beep)
                        .on_toggle({
                            let temp_options = self.temp_options.clone();
                            move |checked| {
                                let mut new_options = temp_options.lock().unwrap().clone();
                                new_options.console_beep = checked;
                                crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                            }
                        })
                        .size(18),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
                Space::new().height(12.0),
                // Open Settings Directory button
                row![
                    Space::new().width(Length::Fixed(LABEL_WIDTH)),
                    button(text(fl!(crate::LANGUAGE_LOADER, "settings-terminal-open-settings-dir-button")).size(14))
                        .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::OpenSettingsFolder))
                        .padding([6, 12]),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            ]
            .spacing(INPUT_SPACING)
            .into()
        ),]
        .padding(SECTION_PADDING as u16)
        .width(Length::Fill)
        .into()
    }
}
