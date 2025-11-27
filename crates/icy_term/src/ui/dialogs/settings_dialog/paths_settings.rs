use i18n_embed_fl::fl;
use iced::{
    Alignment, Element, Length,
    widget::{Space, column, row, text_input},
};
use icy_engine_gui::{
    section_header,
    settings::{effect_box, left_label},
    ui::{DIALOG_SPACING, TEXT_SIZE_NORMAL, browse_button, secondary_button},
};

use crate::Options;
use crate::ui::dialogs::settings_dialog::SettingsMsg;

pub fn paths_settings_content(download_path: String, capture_path: String) -> Element<'static, crate::ui::Message> {
    let config_dir = directories::ProjectDirs::from("com", "GitHub", "icy_term")
        .map(|p| p.config_dir().display().to_string())
        .unwrap_or_else(|| "N/A".to_string());

    let config_file = directories::ProjectDirs::from("com", "GitHub", "icy_term")
        .map(|p| p.config_dir().join("options.toml").display().to_string())
        .unwrap_or_else(|| "N/A".to_string());

    let phonebook_file = directories::ProjectDirs::from("com", "GitHub", "icy_term")
        .map(|p| p.config_dir().join("phonebook.toml").display().to_string())
        .unwrap_or_else(|| "N/A".to_string());

    let log_file = Options::get_log_file().map(|p| p.display().to_string()).unwrap_or_else(|| "N/A".to_string());

    let content = column![
        section_header(fl!(crate::LANGUAGE_LOADER, "settings-paths-header")),
        effect_box(
            column![
                // Config directory (read-only with open button)
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "settings-paths-config-dir")),
                    text_input("", &config_dir).size(TEXT_SIZE_NORMAL).width(Length::Fill),
                    browse_button(crate::ui::Message::SettingsDialog(SettingsMsg::OpenSettingsFolder)),
                ]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center),
                // Config file (read-only)
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "settings-paths-config-file")),
                    text_input("", &config_file).size(TEXT_SIZE_NORMAL).width(Length::Fill),
                ]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center),
                // Phonebook file (read-only)
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "settings-paths-phonebook")),
                    text_input("", &phonebook_file).size(TEXT_SIZE_NORMAL).width(Length::Fill),
                ]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center),
                // Log file (read-only with open button)
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "settings-paths-log-file")),
                    text_input("", &log_file).size(TEXT_SIZE_NORMAL).width(Length::Fill),
                    secondary_button(
                        fl!(crate::LANGUAGE_LOADER, "settings-paths-open"),
                        Some(crate::ui::Message::SettingsDialog(SettingsMsg::OpenLogFile))
                    ),
                ]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center),
            ]
            .spacing(DIALOG_SPACING)
            .into()
        ),
        Space::new().height(Length::Fixed(12.0)),
        section_header(fl!(crate::LANGUAGE_LOADER, "settings-paths-editable-header")),
        effect_box(
            column![
                // Download directory (editable)
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "settings-paths-download-dir")),
                    text_input("", &download_path)
                        .on_input(|value| { crate::ui::Message::SettingsDialog(SettingsMsg::UpdateDownloadPath(value)) })
                        .size(TEXT_SIZE_NORMAL)
                        .width(Length::Fill),
                    browse_button(crate::ui::Message::SettingsDialog(SettingsMsg::BrowseDownloadPath)),
                ]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center),
                // Capture path (editable)
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "settings-paths-capture-path")),
                    text_input("", &capture_path)
                        .on_input(|value| { crate::ui::Message::SettingsDialog(SettingsMsg::UpdateCapturePath(value)) })
                        .size(TEXT_SIZE_NORMAL)
                        .width(Length::Fill),
                    browse_button(crate::ui::Message::SettingsDialog(SettingsMsg::BrowseCapturePath)),
                ]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center),
            ]
            .spacing(DIALOG_SPACING)
            .into()
        ),
    ]
    .spacing(0);

    content.into()
}
