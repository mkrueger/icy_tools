use i18n_embed_fl::fl;
use icy_engine_gui::{
    music::music::DialTone,
    section_header,
    settings::{effect_box, left_label},
    ui::DIALOG_SPACING,
    SECTION_PADDING, TEXT_SIZE_NORMAL,
};
use icy_parser_core::CaretShape;
use icy_ui::{
    widget::{checkbox, column, pick_list, row, slider, text, text_input},
    Alignment, Element, Length,
};

use crate::ui::settings_dialog::{SettingsDialogMessage, SettingsDialogState};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CursorShapeOption(CaretShape);

impl CursorShapeOption {
    const ALL: [Self; 3] = [Self(CaretShape::Block), Self(CaretShape::Underline), Self(CaretShape::Bar)];
}

impl std::fmt::Display for CursorShapeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            CaretShape::Block => "Block",
            CaretShape::Underline => "Underline",
            CaretShape::Bar => "Bar",
        })
    }
}

impl SettingsDialogState {
    pub fn terminal_settings_content_generic<'a, M: Clone + 'static>(
        &self,
        on_message: impl Fn(SettingsDialogMessage) -> M + Clone + 'static,
    ) -> Element<'a, M> {
        let temp_options = self.temp_options.lock();
        let console_beep = temp_options.console_beep;
        let dial_tone = temp_options.dial_tone;
        let max_scrollback_lines = temp_options.max_scrollback_lines;
        let audio_enabled = temp_options.audio_enabled;
        let master_volume = temp_options.master_volume;
        let selected_audio_device = temp_options.audio_device.clone().unwrap_or_else(|| "Default".to_string());
        let invert_mouse_wheel = temp_options.invert_mouse_wheel;
        let default_cursor_shape = temp_options.default_cursor_shape;
        let default_cursor_blinking = temp_options.default_cursor_blinking;
        drop(temp_options); // Release the lock early

        let mut audio_devices = icy_engine_gui::music::music::SoundThread::output_devices();
        audio_devices.insert(0, "Default".to_string());

        let temp_options = self.temp_options.clone();
        let temp_options2 = self.temp_options.clone();
        let temp_options3 = self.temp_options.clone();
        let temp_options4 = self.temp_options.clone();
        let temp_options5 = self.temp_options.clone();
        let temp_options6 = self.temp_options.clone();
        let temp_options7 = self.temp_options.clone();
        let temp_options8 = self.temp_options.clone();
        let temp_options9 = self.temp_options.clone();
        let temp_options10 = self.temp_options.clone();

        let on_msg = on_message.clone();
        let on_msg2 = on_message.clone();
        let on_msg3 = on_message.clone();
        let on_msg4 = on_message.clone();
        let on_msg5 = on_message.clone();
        let on_msg6 = on_message.clone();
        let on_msg7 = on_message.clone();
        let on_msg8 = on_message.clone();
        let on_msg9 = on_message.clone();
        let on_msg10 = on_message.clone();

        column![
            section_header(fl!(crate::LANGUAGE_LOADER, "settings-terminal-general-section")),
            effect_box(
                column![
                    // Console Beep
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-terminal-console-beep-checkbox")),
                        checkbox(console_beep)
                            .on_toggle(move |checked| {
                                let mut new_options = temp_options.lock().clone();
                                new_options.console_beep = checked;
                                on_msg(SettingsDialogMessage::UpdateOptions(new_options))
                            })
                            .size(18),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-terminal-audio-enabled")),
                        checkbox(audio_enabled)
                            .on_toggle(move |checked| {
                                let mut new_options = temp_options5.lock().clone();
                                new_options.audio_enabled = checked;
                                on_msg5(SettingsDialogMessage::UpdateOptions(new_options))
                            })
                            .size(18),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-terminal-master-volume")),
                        slider(0.0..=1.0, master_volume, move |value| {
                            let mut new_options = temp_options6.lock().clone();
                            new_options.master_volume = value;
                            on_msg6(SettingsDialogMessage::UpdateOptions(new_options))
                        })
                        .step(0.01)
                        .width(Length::Fill),
                        text(format!("{}%", (master_volume * 100.0).round() as u32)).width(Length::Fixed(48.0)),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-terminal-audio-device")),
                        pick_list(audio_devices, Some(selected_audio_device), move |device| {
                            let mut new_options = temp_options7.lock().clone();
                            new_options.audio_device = if device == "Default" { None } else { Some(device) };
                            on_msg7(SettingsDialogMessage::UpdateOptions(new_options))
                        })
                        .width(Length::Fill)
                        .text_size(TEXT_SIZE_NORMAL),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-terminal-invert-mouse-wheel")),
                        checkbox(invert_mouse_wheel)
                            .on_toggle(move |checked| {
                                let mut new_options = temp_options8.lock().clone();
                                new_options.invert_mouse_wheel = checked;
                                on_msg8(SettingsDialogMessage::UpdateOptions(new_options))
                            })
                            .size(18),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-terminal-cursor-shape")),
                        pick_list(&CursorShapeOption::ALL[..], Some(CursorShapeOption(default_cursor_shape)), move |value| {
                            let mut new_options = temp_options9.lock().clone();
                            new_options.default_cursor_shape = value.0;
                            on_msg9(SettingsDialogMessage::UpdateOptions(new_options))
                        })
                        .width(Length::Fixed(200.0))
                        .text_size(TEXT_SIZE_NORMAL),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-terminal-cursor-blinking")),
                        checkbox(default_cursor_blinking)
                            .on_toggle(move |checked| {
                                let mut new_options = temp_options10.lock().clone();
                                new_options.default_cursor_blinking = checked;
                                on_msg10(SettingsDialogMessage::UpdateOptions(new_options))
                            })
                            .size(18),
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                    // Dial Tone selection
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "settings-terminal-dial-tone")),
                        pick_list(&DialToneOption::ALL[..], Some(DialToneOption::from(dial_tone)), move |value: DialToneOption| {
                            let mut new_options = temp_options2.lock().clone();
                            new_options.dial_tone = value.into();
                            on_msg2(SettingsDialogMessage::UpdateOptions(new_options))
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
                            .on_input(move |value: String| {
                                if let Ok(lines) = value.parse::<usize>() {
                                    let lines = lines.clamp(100, 100_000);
                                    let mut new_options = temp_options3.lock().clone();
                                    new_options.max_scrollback_lines = lines;
                                    on_msg3(SettingsDialogMessage::UpdateOptions(new_options))
                                } else {
                                    on_msg4(SettingsDialogMessage::UpdateOptions(temp_options4.lock().clone()))
                                }
                            })
                            .width(Length::Fixed(100.0))
                            .size(TEXT_SIZE_NORMAL),
                        icy_ui::widget::text(fl!(crate::LANGUAGE_LOADER, "settings-terminal-scrollback-lines-unit"))
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
