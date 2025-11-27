use i18n_embed_fl::fl;
use iced::{
    Alignment, Element, Length,
    widget::{checkbox, column, pick_list, row, text_input},
};
use icy_engine_gui::{
    SECTION_PADDING, TEXT_SIZE_NORMAL, section_header,
    settings::{effect_box, left_label},
    ui::DIALOG_SPACING,
};

use crate::{
    data::options::DialTone,
    ui::settings_dialog::{SettingsDialogState, SettingsMsg},
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
        let temp_options = self.temp_options.lock();
        let console_beep = temp_options.console_beep;
        let dial_tone = temp_options.dial_tone;
        let max_scrollback_lines = temp_options.max_scrollback_lines;
        drop(temp_options); // Release the lock early

        column![
            section_header(fl!(crate::LANGUAGE_LOADER, "settings-terminal-general-section")),
            effect_box(
                column![
                    // Console Beep
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-terminal-console-beep-checkbox")),
                        checkbox(console_beep)
                            .on_toggle({
                                let temp_options = self.temp_options.clone();
                                move |checked| {
                                    let mut new_options = temp_options.lock().clone();
                                    new_options.console_beep = checked;
                                    crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                }
                            })
                            .size(18),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    // Dial Tone selection
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-terminal-dial-tone")),
                        pick_list(&DialToneOption::ALL[..], Some(DialToneOption::from(dial_tone)), {
                            let temp_options = self.temp_options.clone();
                            move |value: DialToneOption| {
                                let mut new_options = temp_options.lock().clone();
                                new_options.dial_tone = value.into();
                                crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                            }
                        })
                        .width(Length::Fixed(200.0))
                        .text_size(TEXT_SIZE_NORMAL),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    // Scrollback Buffer Size
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-terminal-scrollback-lines")),
                        text_input("2000", &max_scrollback_lines.to_string())
                            .on_input({
                                let temp_options = self.temp_options.clone();
                                move |value: String| {
                                    if let Ok(lines) = value.parse::<usize>() {
                                        let lines = lines.clamp(100, 100000);
                                        let mut new_options = temp_options.lock().clone();
                                        new_options.max_scrollback_lines = lines;
                                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                    } else {
                                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(temp_options.lock().clone()))
                                    }
                                }
                            })
                            .width(Length::Fixed(100.0))
                            .size(TEXT_SIZE_NORMAL),
                        iced::widget::text(fl!(crate::LANGUAGE_LOADER, "settings-terminal-scrollback-lines-unit"))
                            .size(TEXT_SIZE_NORMAL)
                            .width(Length::Shrink),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                ]
                .spacing(DIALOG_SPACING)
                .into()
            ),
        ]
        .padding(SECTION_PADDING as u16)
        .width(Length::Fill)
        .into()
    }
}
