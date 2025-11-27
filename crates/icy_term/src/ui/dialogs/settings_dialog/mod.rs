use parking_lot::Mutex;
use std::sync::Arc;

use i18n_embed_fl::fl;
use iced::{
    Border, Color, Element, Length,
    widget::{Space, button, column, container, row, scrollable, text},
};
use icy_engine_gui::settings::{MonitorSettingsMessage, show_monitor_settings, update_monitor_settings};
use icy_engine_gui::ui::*;
use icy_net::{
    modem::ModemConfiguration,
    serial::{CharSize, Parity, StopBits},
};

use crate::{Options, ui::MainWindowMode};

mod iemsi_settings;
mod modem_settings;
mod paths_settings;
mod terminal_settings;

// Settings-specific constants
const SETTINGS_CONTENT_HEIGHT: f32 = 410.0;

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsCategory {
    Monitor,
    IEMSI,
    Terminal,
    Keybinds,
    Modem,
    Paths,
}

impl SettingsCategory {
    fn name(&self) -> String {
        match self {
            Self::Monitor => fl!(crate::LANGUAGE_LOADER, "settings-monitor-category"),
            Self::IEMSI => fl!(crate::LANGUAGE_LOADER, "settings-iemsi-category"),
            Self::Terminal => fl!(crate::LANGUAGE_LOADER, "settings-terminal-category"),
            Self::Keybinds => fl!(crate::LANGUAGE_LOADER, "settings-keybinds-category"),
            Self::Modem => fl!(crate::LANGUAGE_LOADER, "settings-modem-category"),
            Self::Paths => fl!(crate::LANGUAGE_LOADER, "settings-paths-category"),
        }
    }

    fn all() -> Vec<Self> {
        vec![Self::Monitor, Self::IEMSI, Self::Terminal, /*Self::Keybinds,*/ Self::Modem, Self::Paths]
    }
}

#[derive(Debug, Clone)]
pub enum SettingsMsg {
    SwitchCategory(SettingsCategory),
    UpdateOptions(Options),
    ResetCategory(SettingsCategory),
    MonitorSettings(MonitorSettingsMessage),
    OpenSettingsFolder,
    OpenLogFile,
    SelectModem(usize),
    AddModem,
    RemoveModem(usize),
    UpdateDownloadPath(String),
    UpdateCapturePath(String),
    BrowseDownloadPath,
    BrowseCapturePath,
    ResetPaths,
    Save,
    Cancel,
    Noop,
}

pub struct SettingsDialogState {
    pub current_category: SettingsCategory,
    pub temp_options: Arc<Mutex<Options>>,
    pub original_options: Arc<Mutex<Options>>,
    pub selected_modem_index: usize,
}

impl SettingsDialogState {
    pub fn new(original_options: Arc<Mutex<Options>>, temp_options: Arc<Mutex<Options>>) -> Self {
        Self {
            current_category: SettingsCategory::Monitor,
            temp_options,
            original_options,
            selected_modem_index: 0,
        }
    }

    pub fn update(&mut self, message: SettingsMsg) -> Option<crate::ui::Message> {
        match message {
            SettingsMsg::SwitchCategory(category) => {
                self.current_category = category;
                None
            }
            SettingsMsg::UpdateOptions(options) => {
                *self.temp_options.lock() = options;
                None
            }
            SettingsMsg::ResetCategory(category) => {
                match category {
                    SettingsCategory::Monitor => {
                        self.temp_options.lock().reset_monitor_settings();
                    }
                    SettingsCategory::Terminal => {
                        let mut options = self.temp_options.lock();
                        let default_options = crate::data::Options::default();
                        options.console_beep = default_options.console_beep;
                        options.dial_tone = default_options.dial_tone;
                    }
                    SettingsCategory::IEMSI => {
                        let mut options = self.temp_options.lock();
                        options.iemsi = crate::data::IEMSISettings::default();
                    }
                    SettingsCategory::Paths => {
                        let mut options = self.temp_options.lock();
                        options.download_path = String::new();
                        options.capture_path = String::new();
                    }
                    SettingsCategory::Keybinds => {
                        // self.temp_options.reset_keybindings();
                    }
                    _ => {}
                }
                None
            }
            SettingsMsg::OpenSettingsFolder => {
                if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_term") {
                    let _ = open::that(proj_dirs.config_dir());
                }
                None
            }
            SettingsMsg::OpenLogFile => {
                if let Some(log_file) = Options::get_log_file() {
                    if log_file.exists() {
                        #[cfg(windows)]
                        {
                            // open::that doesn't work for me for unknown reason - their opening string should work.
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
                        // If log file doesn't exist yet, open the log directory
                        let _ = open::that(parent);
                    }
                }
                None
            }
            SettingsMsg::Save => {
                // Save the options and close dialog
                let old_buffer_size = self.original_options.lock().max_scrollback_lines;
                let tmp = self.temp_options.lock().clone();
                let new_buffer_size = tmp.max_scrollback_lines;
                *self.original_options.lock() = tmp;
                if let Err(e) = self.original_options.lock().store_options() {
                    log::error!("Failed to save options: {}", e);
                }

                // If buffer size changed, notify the terminal thread
                if old_buffer_size != new_buffer_size {
                    return Some(crate::ui::Message::SetScrollbackBufferSize(new_buffer_size));
                }

                Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)))
            }
            SettingsMsg::Cancel => {
                // Reset to original options and close
                let tmp = self.original_options.lock().clone();
                *self.temp_options.lock() = tmp;
                Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)))
            }
            SettingsMsg::SelectModem(index) => {
                self.selected_modem_index = index;
                None
            }
            SettingsMsg::AddModem => {
                let len = self.temp_options.lock().modems.len();
                let new_modem = ModemConfiguration {
                    name: format!("Modem {}", len + 1),
                    ..Default::default()
                };
                self.temp_options.lock().modems.push(new_modem);
                self.selected_modem_index = len;
                None
            }
            SettingsMsg::RemoveModem(index) => {
                let mut temp_options = self.temp_options.lock();
                if index < temp_options.modems.len() {
                    temp_options.modems.remove(index);
                    // After removal, get the new length
                    let new_len = temp_options.modems.len();
                    if new_len > 0 {
                        // Select the previous item if possible, otherwise stay at current position
                        self.selected_modem_index = index.min(new_len - 1);
                    } else {
                        self.selected_modem_index = 0;
                    }
                }
                None
            }
            SettingsMsg::MonitorSettings(settings) => {
                update_monitor_settings(&mut self.temp_options.lock().monitor_settings, settings);
                None
            }
            SettingsMsg::UpdateDownloadPath(path) => {
                self.temp_options.lock().download_path = path;
                None
            }
            SettingsMsg::UpdateCapturePath(path) => {
                self.temp_options.lock().capture_path = path;
                None
            }
            SettingsMsg::BrowseDownloadPath => {
                let current_path = self.temp_options.lock().download_path();
                let initial_dir = if std::path::Path::new(&current_path).exists() {
                    Some(std::path::PathBuf::from(&current_path))
                } else {
                    std::env::current_dir().ok()
                };

                let mut dialog = rfd::FileDialog::new();
                if let Some(dir) = initial_dir {
                    dialog = dialog.set_directory(dir);
                }

                if let Some(path) = dialog.pick_folder() {
                    if let Some(path_str) = path.to_str() {
                        self.temp_options.lock().download_path = path_str.to_string();
                    }
                }
                None
            }
            SettingsMsg::BrowseCapturePath => {
                let current_path = self.temp_options.lock().capture_path();
                let initial_dir = if std::path::Path::new(&current_path).exists() {
                    Some(std::path::PathBuf::from(&current_path))
                } else {
                    std::env::current_dir().ok()
                };

                let mut dialog = rfd::FileDialog::new();
                if let Some(dir) = initial_dir {
                    dialog = dialog.set_directory(dir);
                }

                if let Some(path) = dialog.pick_folder() {
                    if let Some(path_str) = path.to_str() {
                        self.temp_options.lock().capture_path = path_str.to_string();
                    }
                }
                None
            }
            SettingsMsg::ResetPaths => {
                let mut options = self.temp_options.lock();
                options.download_path = String::new();
                options.capture_path = String::new();
                None
            }
            SettingsMsg::Noop => None,
        }
    }

    pub fn view<'a>(&'a self, terminal_content: Element<'a, crate::ui::Message>) -> Element<'a, crate::ui::Message> {
        let overlay = self.create_modal_content();
        crate::ui::modal(terminal_content, overlay, crate::ui::Message::SettingsDialog(SettingsMsg::Cancel))
    }

    fn create_modal_content(&self) -> Element<'_, crate::ui::Message> {
        // Category tabs
        let mut category_row = row![].spacing(DIALOG_SPACING);
        for category in SettingsCategory::all() {
            let is_selected = self.current_category == category;
            let cat = category.clone();
            let cat_button = button(text(category.name()).size(14).wrapping(text::Wrapping::None))
                .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::SwitchCategory(cat)))
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
                show_monitor_settings(monitor_settings).map(|msg| crate::ui::Message::SettingsDialog(SettingsMsg::MonitorSettings(msg)))
            }
            SettingsCategory::IEMSI => self.iemsi_settings_content(),
            SettingsCategory::Terminal => self.terminal_settings_content(),
            SettingsCategory::Keybinds => self.keybinds_settings_content(),
            SettingsCategory::Modem => self.modem_settings_content(),
            SettingsCategory::Paths => {
                let options = self.temp_options.lock();
                let download_path = options.download_path();
                let capture_path = options.capture_path();
                drop(options);
                paths_settings::paths_settings_content(download_path, capture_path)
            }
        };

        // Buttons
        let ok_button = primary_button(
            format!("{}", icy_engine_gui::ButtonType::Ok),
            Some(crate::ui::Message::SettingsDialog(SettingsMsg::Save)),
        );

        let cancel_button = secondary_button(
            format!("{}", icy_engine_gui::ButtonType::Cancel),
            Some(crate::ui::Message::SettingsDialog(SettingsMsg::Cancel)),
        );

        let reset_button = match self.current_category {
            SettingsCategory::Monitor => {
                let current_settings = self.temp_options.lock().monitor_settings.clone();
                let default_settings = icy_engine_gui::MonitorSettings::default();
                let is_default = current_settings == default_settings;
                let msg = if !is_default {
                    Some(crate::ui::Message::SettingsDialog(SettingsMsg::ResetCategory(self.current_category.clone())))
                } else {
                    None
                };
                Some(secondary_button(fl!(crate::LANGUAGE_LOADER, "settings-restore-defaults-button"), msg))
            }
            SettingsCategory::Terminal => {
                let options = self.temp_options.lock();
                // let default_options = crate::data::Options::default();
                // let is_default = options.console_beep == default_options.console_beep && options.dial_tone == default_options.dial_tone;
                drop(options);
                /*let msg = if !is_default {
                    Some(crate::ui::Message::SettingsDialog(SettingsMsg::ResetCategory(self.current_category.clone())))
                } else {
                    None
                };
                Some(secondary_button(fl!(crate::LANGUAGE_LOADER, "settings-restore-defaults-button"), msg))*/
                None
            }
            SettingsCategory::IEMSI => {
                let options = self.temp_options.lock();
                // let current_iemsi = options.iemsi.clone();
                // let default_iemsi = crate::data::IEMSISettings::default();
                // let is_default = current_iemsi == default_iemsi;
                drop(options);
                /*
                let msg = if !is_default {
                    Some(crate::ui::Message::SettingsDialog(SettingsMsg::ResetCategory(self.current_category.clone())))
                } else {
                    None
                };
                Some(secondary_button(fl!(crate::LANGUAGE_LOADER, "settings-restore-defaults-button"), msg))*/
                None
            }
            SettingsCategory::Keybinds => {
                // TODO: Add default keybindings check when implemented
                Some(secondary_button(
                    fl!(crate::LANGUAGE_LOADER, "settings-restore-defaults-button"),
                    Some(crate::ui::Message::SettingsDialog(SettingsMsg::ResetCategory(self.current_category.clone()))),
                ))
            }
            SettingsCategory::Paths => {
                let options = self.temp_options.lock();
                let is_default = options.download_path.is_empty() && options.capture_path.is_empty();
                drop(options);
                let msg = if !is_default {
                    Some(crate::ui::Message::SettingsDialog(SettingsMsg::ResetCategory(self.current_category.clone())))
                } else {
                    None
                };
                Some(secondary_button(fl!(crate::LANGUAGE_LOADER, "settings-restore-defaults-button"), msg))
            }
            _ => None,
        };

        let mut buttons_left = vec![];
        if let Some(reset_btn) = reset_button {
            buttons_left.push(reset_btn.into());
        }

        let buttons_right = vec![cancel_button.into(), ok_button.into()];

        let button_area_row = button_row_with_left(buttons_left, buttons_right);

        // For modem settings, don't wrap in scrollable since it has its own layout
        let content_container = if matches!(self.current_category, SettingsCategory::Modem) {
            container(settings_content).height(Length::Fixed(SETTINGS_CONTENT_HEIGHT)).width(Length::Fill)
        } else {
            container(scrollable(settings_content).direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default())))
                .height(Length::Fixed(SETTINGS_CONTENT_HEIGHT))
                .width(Length::Fill)
                .padding(0.0)
        };

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

    fn keybinds_settings_content(&self) -> Element<'_, crate::ui::Message> {
        // TODO: Implement keybindings editor
        column![text("Keybindings editor - TODO: Implement keybinding controls").size(14),].into()
    }
}

// Wrapper types for external enums to implement Display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CharSizeOption(CharSize);

impl CharSizeOption {
    const ALL: [CharSizeOption; 4] = [
        CharSizeOption(CharSize::Bits5),
        CharSizeOption(CharSize::Bits6),
        CharSizeOption(CharSize::Bits7),
        CharSizeOption(CharSize::Bits8),
    ];
}

impl From<CharSize> for CharSizeOption {
    fn from(value: CharSize) -> Self {
        CharSizeOption(value)
    }
}

impl From<CharSizeOption> for CharSize {
    fn from(value: CharSizeOption) -> Self {
        value.0
    }
}

impl std::fmt::Display for CharSizeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            CharSize::Bits5 => write!(f, "5"),
            CharSize::Bits6 => write!(f, "6"),
            CharSize::Bits7 => write!(f, "7"),
            CharSize::Bits8 => write!(f, "8"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StopBitsOption(StopBits);

impl StopBitsOption {
    const ALL: [StopBitsOption; 2] = [StopBitsOption(StopBits::One), StopBitsOption(StopBits::Two)];
}

impl From<StopBits> for StopBitsOption {
    fn from(value: StopBits) -> Self {
        StopBitsOption(value)
    }
}

impl From<StopBitsOption> for StopBits {
    fn from(value: StopBitsOption) -> Self {
        value.0
    }
}

impl std::fmt::Display for StopBitsOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            StopBits::One => write!(f, "1"),
            StopBits::Two => write!(f, "2"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParityOption(Parity);

impl ParityOption {
    const ALL: [ParityOption; 3] = [ParityOption(Parity::None), ParityOption(Parity::Odd), ParityOption(Parity::Even)];
}

impl From<Parity> for ParityOption {
    fn from(value: Parity) -> Self {
        ParityOption(value)
    }
}

impl From<ParityOption> for Parity {
    fn from(value: ParityOption) -> Self {
        value.0
    }
}

impl std::fmt::Display for ParityOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Parity::None => write!(f, "None"),
            Parity::Odd => write!(f, "Odd"),
            Parity::Even => write!(f, "Even"),
        }
    }
}

// Add FlowControl wrapper
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FlowControlOption(icy_net::serial::FlowControl);

impl FlowControlOption {
    const ALL: [FlowControlOption; 3] = [
        FlowControlOption(icy_net::serial::FlowControl::None),
        FlowControlOption(icy_net::serial::FlowControl::XonXoff),
        FlowControlOption(icy_net::serial::FlowControl::RtsCts),
    ];
}

impl From<icy_net::serial::FlowControl> for FlowControlOption {
    fn from(value: icy_net::serial::FlowControl) -> Self {
        FlowControlOption(value)
    }
}

impl From<FlowControlOption> for icy_net::serial::FlowControl {
    fn from(value: FlowControlOption) -> Self {
        value.0
    }
}

impl std::fmt::Display for FlowControlOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            icy_net::serial::FlowControl::None => write!(f, "None"),
            icy_net::serial::FlowControl::XonXoff => write!(f, "Software"),
            icy_net::serial::FlowControl::RtsCts => write!(f, "Hardware"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ThemeOption(iced::Theme);

impl ThemeOption {
    const _ALL: [ThemeOption; 22] = [
        ThemeOption(iced::Theme::Light),
        ThemeOption(iced::Theme::Dark),
        ThemeOption(iced::Theme::Dracula),
        ThemeOption(iced::Theme::Nord),
        ThemeOption(iced::Theme::SolarizedLight),
        ThemeOption(iced::Theme::SolarizedDark),
        ThemeOption(iced::Theme::GruvboxLight),
        ThemeOption(iced::Theme::GruvboxDark),
        ThemeOption(iced::Theme::CatppuccinLatte),
        ThemeOption(iced::Theme::CatppuccinFrappe),
        ThemeOption(iced::Theme::CatppuccinMacchiato),
        ThemeOption(iced::Theme::CatppuccinMocha),
        ThemeOption(iced::Theme::TokyoNight),
        ThemeOption(iced::Theme::TokyoNightStorm),
        ThemeOption(iced::Theme::TokyoNightLight),
        ThemeOption(iced::Theme::KanagawaWave),
        ThemeOption(iced::Theme::KanagawaDragon),
        ThemeOption(iced::Theme::KanagawaLotus),
        ThemeOption(iced::Theme::Moonfly),
        ThemeOption(iced::Theme::Nightfly),
        ThemeOption(iced::Theme::Oxocarbon),
        ThemeOption(iced::Theme::Ferra),
    ];
}

impl From<iced::Theme> for ThemeOption {
    fn from(value: iced::Theme) -> Self {
        ThemeOption(value)
    }
}

impl From<ThemeOption> for iced::Theme {
    fn from(value: ThemeOption) -> Self {
        value.0
    }
}

impl std::fmt::Display for ThemeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            iced::Theme::Light => write!(f, "Light"),
            iced::Theme::Dark => write!(f, "Dark"),
            iced::Theme::Dracula => write!(f, "Dracula"),
            iced::Theme::Nord => write!(f, "Nord"),
            iced::Theme::SolarizedLight => write!(f, "Solarized Light"),
            iced::Theme::SolarizedDark => write!(f, "Solarized Dark"),
            iced::Theme::GruvboxLight => write!(f, "Gruvbox Light"),
            iced::Theme::GruvboxDark => write!(f, "Gruvbox Dark"),
            iced::Theme::CatppuccinLatte => write!(f, "Catppuccin Latte"),
            iced::Theme::CatppuccinFrappe => write!(f, "Catppuccin Frappe"),
            iced::Theme::CatppuccinMacchiato => write!(f, "Catppuccin Macchiato"),
            iced::Theme::CatppuccinMocha => write!(f, "Catppuccin Mocha"),
            iced::Theme::TokyoNight => write!(f, "Tokyo Night"),
            iced::Theme::TokyoNightStorm => write!(f, "Tokyo Night Storm"),
            iced::Theme::TokyoNightLight => write!(f, "Tokyo Night Light"),
            iced::Theme::KanagawaWave => write!(f, "Kanagawa Wave"),
            iced::Theme::KanagawaDragon => write!(f, "Kanagawa Dragon"),
            iced::Theme::KanagawaLotus => write!(f, "Kanagawa Lotus"),
            iced::Theme::Moonfly => write!(f, "Moonfly"),
            iced::Theme::Nightfly => write!(f, "Nightfly"),
            iced::Theme::Oxocarbon => write!(f, "Oxocarbon"),
            iced::Theme::Ferra => write!(f, "Ferra"),
            iced::Theme::Custom(_) => write!(f, "Custom"),
        }
    }
}
