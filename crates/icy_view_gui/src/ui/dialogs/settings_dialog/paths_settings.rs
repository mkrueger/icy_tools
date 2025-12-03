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

use super::SettingsMessage;
use crate::ui::Options;

pub fn paths_settings_content(export_path: String) -> Element<'static, crate::ui::Message> {
    let config_dir = directories::ProjectDirs::from("com", "GitHub", "icy_view")
        .map(|p| p.config_dir().display().to_string())
        .unwrap_or_else(|| "N/A".to_string());

    let config_file = directories::ProjectDirs::from("com", "GitHub", "icy_view")
        .map(|p| p.config_dir().join("options.toml").display().to_string())
        .unwrap_or_else(|| "N/A".to_string());

    let log_file = Options::get_log_file().map(|p| p.display().to_string()).unwrap_or_else(|| "N/A".to_string());

    let content = column![
        // System Paths (read-only)
        section_header(fl!(crate::LANGUAGE_LOADER, "settings-paths-header")),
        effect_box(
            column![
                // Config directory (read-only with open button)
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "settings-paths-config-dir")),
                    text_input("", &config_dir).size(TEXT_SIZE_NORMAL).width(Length::Fill),
                    browse_button(crate::ui::Message::SettingsDialog(SettingsMessage::OpenSettingsFolder)),
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
                // Log file (read-only with open button)
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "settings-paths-log-file")),
                    text_input("", &log_file).size(TEXT_SIZE_NORMAL).width(Length::Fill),
                    secondary_button(
                        fl!(crate::LANGUAGE_LOADER, "settings-paths-open"),
                        Some(crate::ui::Message::SettingsDialog(SettingsMessage::OpenLogFile))
                    ),
                ]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center),
            ]
            .spacing(DIALOG_SPACING)
            .into()
        ),
        Space::new().height(Length::Fixed(12.0)),
        // User Paths (editable)
        section_header(fl!(crate::LANGUAGE_LOADER, "settings-paths-user-header")),
        effect_box(
            column![
                // Export path (editable with browse button)
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "settings-paths-export-path")),
                    text_input(&Options::default_export_directory().to_string_lossy(), &export_path)
                        .size(TEXT_SIZE_NORMAL)
                        .width(Length::Fill)
                        .on_input(|s| crate::ui::Message::SettingsDialog(SettingsMessage::UpdateExportPath(s))),
                    browse_button(crate::ui::Message::SettingsDialog(SettingsMessage::BrowseExportPath)),
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
