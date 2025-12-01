use crate::ui::options::EXTERNAL_COMMAND_COUNT;
use i18n_embed_fl::fl;
use iced::{
    Alignment, Element, Length, Theme,
    widget::{column, row, text, text_input},
};
use icy_engine_gui::{
    SECTION_PADDING, section_header,
    settings::effect_box,
    ui::{DIALOG_SPACING, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL, left_label_small},
};

use super::SettingsMessage;

pub fn commands_settings_content(commands: [crate::ui::options::ExternalCommand; EXTERNAL_COMMAND_COUNT]) -> Element<'static, crate::ui::Message> {
    let mut command_rows = column![].spacing(DIALOG_SPACING);

    for (i, cmd) in commands.into_iter().enumerate() {
        let key_label = format!("F{}", i + 5);

        let command_input = text_input(&fl!(crate::LANGUAGE_LOADER, "settings-commands-placeholder"), &cmd.command)
            .on_input(move |s| crate::ui::Message::SettingsDialog(SettingsMessage::ExternalCommandChanged(i, s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        command_rows = command_rows.push(
            row![left_label_small(key_label), command_input,]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center),
        );
    }

    let description = text(fl!(crate::LANGUAGE_LOADER, "settings-commands-description"))
        .style(|theme: &Theme| text::Style {
            color: Some(theme.extended_palette().secondary.base.color),
        })
        .size(TEXT_SIZE_SMALL);

    column![
        section_header(fl!(crate::LANGUAGE_LOADER, "settings-commands-section")),
        effect_box(
            column![command_rows, row![left_label_small(String::new()), description].spacing(DIALOG_SPACING),]
                .spacing(DIALOG_SPACING)
                .into()
        ),
    ]
    .padding(SECTION_PADDING as u16)
    .width(Length::Fill)
    .into()
}
