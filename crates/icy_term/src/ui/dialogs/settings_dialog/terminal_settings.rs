use i18n_embed_fl::fl;
use iced::{
    Alignment, Element, Length,
    widget::{Space, button, checkbox, column, pick_list, row, text},
};
use iced_engine_gui::settings::{LABEL_WIDTH, SECTION_PADDING, effect_box, left_label};

use crate::{
    data::options::DialTone,
    ui::settings_dialog::{INPUT_SPACING, SettingsDialogState, SettingsMsg},
};

// Create a wrapper type for the pick list
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DialToneOption(DialTone);

impl DialToneOption {
    const ALL: [Self; 5] = [
        Self(DialTone::US),
        Self(DialTone::UK),
        Self(DialTone::Europe),
        Self(DialTone::France),
        Self(DialTone::Japan),
    ];
}

impl std::fmt::Display for DialToneOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            DialTone::US => write!(f, "US/Canada (350+440 Hz)"),
            DialTone::UK => write!(f, "UK (350+450 Hz)"),
            DialTone::Europe => write!(f, "Europe (425 Hz)"),
            DialTone::France => write!(f, "France (440 Hz)"),
            DialTone::Japan => write!(f, "Japan (400 Hz)"),
        }
    }
}

impl From<DialTone> for DialToneOption {
    fn from(tone: DialTone) -> Self {
        Self(tone)
    }
}

impl From<DialToneOption> for DialTone {
    fn from(option: DialToneOption) -> Self {
        option.0
    }
}

impl SettingsDialogState {
    pub fn terminal_settings_content<'a>(&self) -> Element<'a, crate::ui::Message> {
        let temp_options = self.temp_options.lock().unwrap();
        let console_beep = temp_options.console_beep;
        let dial_tone = temp_options.dial_tone;
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
                // Dial Tone selection
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "settings-terminal-dial-tone")),
                    pick_list(&DialToneOption::ALL[..], Some(DialToneOption::from(dial_tone)), {
                        let temp_options = self.temp_options.clone();
                        move |value: DialToneOption| {
                            let mut new_options = temp_options.lock().unwrap().clone();
                            new_options.dial_tone = value.into();
                            crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                        }
                    })
                    .width(Length::Fixed(200.0))
                    .text_size(14),
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
