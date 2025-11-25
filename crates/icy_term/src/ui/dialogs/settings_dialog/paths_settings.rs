use i18n_embed_fl::fl;
use iced::{
    Alignment, Element, Length,
    widget::{Space, button, column, row, text, text_input},
};
use icy_engine_gui::{
    section_header,
    settings::{effect_box, left_label},
};

use crate::ui::dialogs::settings_dialog::SettingsMsg;

pub fn paths_settings_content(download_path: String, capture_path: String) -> Element<'static, crate::ui::Message> {
    let config_dir = directories::ProjectDirs::from("com", "GitHub", "icy_term")
        .map(|p| p.config_dir().display().to_string())
        .unwrap_or_else(|| "N/A".to_string());

    let config_file = directories::ProjectDirs::from("com", "GitHub", "icy_term")
        .map(|p| p.config_dir().join("options.toml").display().to_string())
        .unwrap_or_else(|| "N/A".to_string());

    let phonebook_file = directories::ProjectDirs::from("com", "GitHub", "icy_term")
        .map(|p| p.config_dir().join("addresses.json").display().to_string())
        .unwrap_or_else(|| "N/A".to_string());

    let content = column![
        section_header(fl!(crate::LANGUAGE_LOADER, "settings-paths-header")),
        effect_box(
            column![
                // Config directory (read-only with open button)
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "settings-paths-config-dir")),
                    text_input("", &config_dir).size(14).width(Length::Fill),
                    button(text("…".to_string()).size(14).wrapping(text::Wrapping::None))
                        .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::OpenSettingsFolder))
                        .padding([4, 8]),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
                // Config file (read-only)
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "settings-paths-config-file")),
                    text_input("", &config_file).size(14).width(Length::Fill),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
                // Phonebook file (read-only)
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "settings-paths-phonebook")),
                    text_input("", &phonebook_file).size(14).width(Length::Fill),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            ]
            .spacing(8)
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
                        .size(14)
                        .width(Length::Fill),
                    button(text("…").size(14).wrapping(text::Wrapping::None))
                        .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::BrowseDownloadPath))
                        .padding([4, 8]),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                // Capture path (editable)
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "settings-paths-capture-path")),
                    text_input("", &capture_path)
                        .on_input(|value| { crate::ui::Message::SettingsDialog(SettingsMsg::UpdateCapturePath(value)) })
                        .size(14)
                        .width(Length::Fill),
                    button(text("…").size(14).wrapping(text::Wrapping::None))
                        .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::BrowseCapturePath))
                        .padding([4, 8]),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ]
            .spacing(8)
            .into()
        ),
    ]
    .spacing(0);

    content.into()
}
