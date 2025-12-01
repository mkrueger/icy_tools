use parking_lot::Mutex;
use std::sync::Arc;

use i18n_embed_fl::fl;
use iced::{
    Border, Color, Element, Length,
    widget::{Space, button, column, container, row, scrollable, text},
};
use icy_engine_gui::settings::{MonitorSettingsMessage, show_monitor_settings, update_monitor_settings};
use icy_engine_gui::ui::*;

use crate::ui::Options;

mod command_settings;
mod paths_settings;

// Settings-specific constants
const SETTINGS_CONTENT_HEIGHT: f32 = 410.0;

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsCategory {
    Monitor,
    Commands,
    Paths,
}

impl SettingsCategory {
    fn name(&self) -> String {
        match self {
            Self::Monitor => fl!(crate::LANGUAGE_LOADER, "settings-monitor-category"),
            Self::Commands => fl!(crate::LANGUAGE_LOADER, "settings-commands-category"),
            Self::Paths => fl!(crate::LANGUAGE_LOADER, "settings-paths-category"),
        }
    }

    fn all() -> Vec<Self> {
        vec![Self::Monitor, Self::Commands, Self::Paths]
    }
}

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    SwitchCategory(SettingsCategory),
    MonitorSettings(MonitorSettingsMessage),
    ExternalCommandChanged(usize, String),
    OpenSettingsFolder,
    OpenLogFile,
    Save,
    Cancel,
}

pub struct SettingsDialogState {
    pub current_category: SettingsCategory,
    pub temp_options: Arc<Mutex<Options>>,
    pub original_options: Arc<Mutex<Options>>,
}

impl SettingsDialogState {
    pub fn new(original_options: Arc<Mutex<Options>>, temp_options: Arc<Mutex<Options>>) -> Self {
        Self {
            current_category: SettingsCategory::Monitor,
            temp_options,
            original_options,
        }
    }

    pub fn update(&mut self, message: SettingsMessage) -> Option<crate::ui::Message> {
        match message {
            SettingsMessage::SwitchCategory(category) => {
                self.current_category = category;
                None
            }
            SettingsMessage::OpenSettingsFolder => {
                if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_view") {
                    if let Err(err) = open::that(proj_dirs.config_dir()) {
                        log::error!("Failed to open settings folder: {}", err);
                    }
                }
                None
            }
            SettingsMessage::OpenLogFile => {
                if let Some(log_file) = Options::get_log_file() {
                    if log_file.exists() {
                        #[cfg(windows)]
                        {
                            if let Err(err) = std::process::Command::new("notepad").arg(&log_file).spawn() {
                                log::error!("Failed to open log file: {}", err);
                            }
                        }
                        #[cfg(not(windows))]
                        {
                            if let Err(err) = open::that(&log_file) {
                                log::error!("Failed to open log file: {}", err);
                            }
                        }
                    } else if let Some(parent) = log_file.parent() {
                        if let Err(err) = open::that(parent) {
                            log::error!("Failed to open log file directory: {}", err);
                        }
                    }
                }
                None
            }
            SettingsMessage::Save => {
                let tmp = self.temp_options.lock().clone();
                *self.original_options.lock() = tmp;
                self.original_options.lock().store_options();
                Some(crate::ui::Message::CloseSettingsDialog)
            }
            SettingsMessage::Cancel => {
                let tmp = self.original_options.lock().clone();
                *self.temp_options.lock() = tmp;
                Some(crate::ui::Message::CloseSettingsDialog)
            }
            SettingsMessage::MonitorSettings(settings) => {
                update_monitor_settings(&mut self.temp_options.lock().monitor_settings, settings);
                None
            }
            SettingsMessage::ExternalCommandChanged(idx, cmd) => {
                self.temp_options.lock().external_commands[idx].command = cmd;
                None
            }
        }
    }

    pub fn view<'a>(&'a self, background_content: Element<'a, crate::ui::Message>) -> Element<'a, crate::ui::Message> {
        let overlay = self.create_modal_content();
        modal_overlay(background_content, overlay)
    }

    fn create_modal_content(&self) -> Element<'_, crate::ui::Message> {
        // Category tabs
        let mut category_row = row![].spacing(DIALOG_SPACING);
        for category in SettingsCategory::all() {
            let is_selected = self.current_category == category;
            let cat = category.clone();
            let cat_button = button(text(category.name()).size(TEXT_SIZE_NORMAL).wrapping(text::Wrapping::None))
                .on_press(crate::ui::Message::SettingsDialog(SettingsMessage::SwitchCategory(cat)))
                .style(move |theme: &iced::Theme, status| {
                    use iced::widget::button::{Status, Style};

                    let palette = theme.extended_palette();
                    let base = if is_selected {
                        Style {
                            background: Some(iced::Background::Color(palette.primary.weak.color)),
                            text_color: palette.primary.weak.text,
                            border: Border::default().rounded(4.0),
                            shadow: Default::default(),
                            snap: false,
                        }
                    } else {
                        Style {
                            background: Some(iced::Background::Color(Color::TRANSPARENT)),
                            text_color: palette.background.base.text,
                            border: Border::default().rounded(4.0),
                            shadow: Default::default(),
                            snap: false,
                        }
                    };

                    match status {
                        Status::Active => base,
                        Status::Hovered if !is_selected => Style {
                            background: Some(iced::Background::Color(Color::from_rgba(
                                palette.primary.weak.color.r,
                                palette.primary.weak.color.g,
                                palette.primary.weak.color.b,
                                0.2,
                            ))),
                            ..base
                        },
                        Status::Pressed => Style {
                            background: Some(iced::Background::Color(palette.primary.strong.color)),
                            ..base
                        },
                        _ => base,
                    }
                })
                .padding([6, 12]);
            category_row = category_row.push(cat_button);
        }

        // Settings content for current category
        let settings_content = match self.current_category {
            SettingsCategory::Monitor => {
                let monitor_settings = self.temp_options.lock().monitor_settings.clone();
                show_monitor_settings(monitor_settings).map(|msg| crate::ui::Message::SettingsDialog(SettingsMessage::MonitorSettings(msg)))
            }
            SettingsCategory::Commands => {
                let commands = self.temp_options.lock().external_commands.clone();
                command_settings::commands_settings_content(commands)
            }
            SettingsCategory::Paths => paths_settings::paths_settings_content(),
        };

        // Buttons
        let ok_button = primary_button(
            format!("{}", icy_engine_gui::ButtonType::Ok),
            Some(crate::ui::Message::SettingsDialog(SettingsMessage::Save)),
        );

        let cancel_button = secondary_button(
            format!("{}", icy_engine_gui::ButtonType::Cancel),
            Some(crate::ui::Message::SettingsDialog(SettingsMessage::Cancel)),
        );

        let buttons_right = vec![cancel_button.into(), ok_button.into()];

        let button_area_row = button_row(buttons_right);

        let content_container = container(scrollable(settings_content).direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default())))
            .height(Length::Fixed(SETTINGS_CONTENT_HEIGHT))
            .width(Length::Fill)
            .padding(0.0);

        let dialog_content = dialog_area(column![category_row, Space::new().height(DIALOG_SPACING), content_container,].into());

        let button_area_wrapped = dialog_area(button_area_row.into());

        let modal = modal_container(
            column![container(dialog_content).height(Length::Fill), separator(), button_area_wrapped,].into(),
            DIALOG_WIDTH_XARGLE,
        );

        container(modal)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}
