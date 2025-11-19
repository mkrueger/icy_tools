use std::sync::{Arc, Mutex};

use i18n_embed_fl::fl;
use iced::{
    Border, Color, Element, Length,
    widget::{Space, button, column, container, row, scrollable, text},
};
use iced_engine_gui::settings::{MonitorSettingsMessage, show_monitor_settings, update_monitor_settings};
use icy_net::serial::{CharSize, Parity, StopBits};

use crate::{Options, ui::MainWindowMode};

mod iemsi_settings;
mod modem_settings;
mod paths_settings;
mod terminal_settings;

// Constants for sizing
const MODAL_WIDTH: f32 = 680.0;
const MODAL_HEIGHT: f32 = 470.0;
const INPUT_SPACING: f32 = 8.0;

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
                *self.temp_options.lock().unwrap() = options;
                None
            }
            SettingsMsg::ResetCategory(category) => {
                match category {
                    SettingsCategory::Monitor => {
                        self.temp_options.lock().unwrap().reset_monitor_settings();
                    }
                    SettingsCategory::Paths => {
                        let mut options = self.temp_options.lock().unwrap();
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
            SettingsMsg::Save => {
                // Save the options and close dialog
                let tmp = self.temp_options.lock().unwrap().clone();
                *self.original_options.lock().unwrap() = tmp;
                if let Err(e) = self.original_options.lock().unwrap().store_options() {
                    log::error!("Failed to save options: {}", e);
                }
                Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)))
            }
            SettingsMsg::Cancel => {
                // Reset to original options and close
                let tmp = self.original_options.lock().unwrap().clone();
                *self.temp_options.lock().unwrap() = tmp;
                Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)))
            }
            SettingsMsg::SelectModem(index) => {
                self.selected_modem_index = index;
                None
            }
            SettingsMsg::AddModem => {
                let len = self.temp_options.lock().unwrap().modems.len();
                let new_modem = crate::data::modem::Modem {
                    name: format!("Modem {}", len + 1),
                    ..Default::default()
                };
                self.temp_options.lock().unwrap().modems.push(new_modem);
                self.selected_modem_index = len;
                None
            }
            SettingsMsg::RemoveModem(index) => {
                let mut temp_options = self.temp_options.lock().unwrap();
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
                update_monitor_settings(&mut self.temp_options.lock().unwrap().monitor_settings, settings);
                None
            }
            SettingsMsg::UpdateDownloadPath(path) => {
                self.temp_options.lock().unwrap().download_path = path;
                None
            }
            SettingsMsg::UpdateCapturePath(path) => {
                self.temp_options.lock().unwrap().capture_path = path;
                None
            }
            SettingsMsg::BrowseDownloadPath => {
                let current_path = self.temp_options.lock().unwrap().download_path();
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
                        self.temp_options.lock().unwrap().download_path = path_str.to_string();
                    }
                }
                None
            }
            SettingsMsg::BrowseCapturePath => {
                let current_path = self.temp_options.lock().unwrap().capture_path();
                let initial_path = std::path::Path::new(&current_path);
                let initial_dir = if initial_path.exists() {
                    initial_path.parent().map(|p| p.to_path_buf())
                } else {
                    std::env::current_dir().ok()
                };

                let mut dialog = rfd::FileDialog::new();
                if let Some(dir) = initial_dir {
                    dialog = dialog.set_directory(dir);
                }
                if let Some(file_name) = initial_path.file_name() {
                    dialog = dialog.set_directory(file_name.to_string_lossy().as_ref());
                }

                if let Some(path) = dialog.save_file() {
                    if let Some(path_str) = path.to_str() {
                        self.temp_options.lock().unwrap().capture_path = path_str.to_string();
                    }
                }
                None
            }
            SettingsMsg::ResetPaths => {
                let mut options = self.temp_options.lock().unwrap();
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
        let mut category_row = row![].spacing(8);
        for category in SettingsCategory::all() {
            let is_selected = self.current_category == category;
            let cat = category.clone();
            let cat_button = button(text(category.name()).size(14))
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
                let monitor_settings = self.temp_options.lock().unwrap().monitor_settings.clone();
                show_monitor_settings(monitor_settings).map(|msg| crate::ui::Message::SettingsDialog(SettingsMsg::MonitorSettings(msg)))
            }
            SettingsCategory::IEMSI => self.iemsi_settings_content(),
            SettingsCategory::Terminal => self.terminal_settings_content(),
            SettingsCategory::Keybinds => self.keybinds_settings_content(),
            SettingsCategory::Modem => self.modem_settings_content(),
            SettingsCategory::Paths => {
                let options = self.temp_options.lock().unwrap();
                let download_path = options.download_path();
                let capture_path = options.capture_path();
                drop(options);
                paths_settings::paths_settings_content(download_path, capture_path)
            }
        };

        // Buttons
        let ok_button = button(text(fl!(crate::LANGUAGE_LOADER, "dialog-ok_button")))
            .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::Save))
            .padding([8, 16])
            .style(button::primary);

        let cancel_button = button(text(fl!(crate::LANGUAGE_LOADER, "dialog-cancel_button")))
            .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::Cancel))
            .padding([8, 16])
            .style(button::secondary);

        let reset_button = match self.current_category {
            SettingsCategory::Monitor | SettingsCategory::Keybinds => Some(
                button(text(fl!(crate::LANGUAGE_LOADER, "settings-reset-button")))
                    .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::ResetCategory(self.current_category.clone())))
                    .padding([8, 16])
                    .style(button::danger),
            ),
            SettingsCategory::Paths => {
                let options = self.temp_options.lock().unwrap();
                let is_default = options.download_path.is_empty() && options.capture_path.is_empty();
                drop(options);
                let mut btn = button(text(fl!(crate::LANGUAGE_LOADER, "settings-reset-button")))
                    .padding([8, 16])
                    .style(button::danger);
                if !is_default {
                    btn = btn.on_press(crate::ui::Message::SettingsDialog(SettingsMsg::ResetCategory(self.current_category.clone())));
                }
                Some(btn)
            }
            _ => None,
        };

        let mut button_row = row![Space::new().width(Length::Fill),];

        if let Some(reset_btn) = reset_button {
            button_row = button_row.push(reset_btn).push(Space::new().width(8.0));
        }

        button_row = button_row.push(cancel_button).push(Space::new().width(8.0)).push(ok_button);

        // For modem settings, don't wrap in scrollable since it has its own layout
        let content_container = if matches!(self.current_category, SettingsCategory::Modem) {
            container(settings_content).height(Length::Fixed(330.0)).width(Length::Fill)
        } else {
            container(scrollable(settings_content).direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default())))
                .height(Length::Fixed(330.0))
                .width(Length::Fill)
                .padding(0.0)
        };

        let modal_content = container(
            column![
                category_row,
                container(Space::new())
                    .height(Length::Fixed(1.0))
                    .width(Length::Fill)
                    .style(|theme: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(theme.extended_palette().background.strong.color)),
                        ..Default::default()
                    }),
                content_container,
                Space::new().height(Length::Fixed(12.0)),
                button_row,
            ]
            .padding(10)
            .spacing(8),
        )
        .width(Length::Fixed(MODAL_WIDTH))
        .height(Length::Fixed(MODAL_HEIGHT))
        .style(|theme: &iced::Theme| {
            let palette = theme.palette();
            container::Style {
                background: Some(iced::Background::Color(palette.background)),
                border: Border {
                    color: palette.text,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                text_color: Some(palette.text),
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
                snap: false,
            }
        });

        container(modal_content)
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
