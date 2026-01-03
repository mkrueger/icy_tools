use parking_lot::Mutex;
use std::sync::Arc;

use i18n_embed_fl::fl;
use icy_engine_gui::settings::{show_monitor_settings, update_monitor_settings, MonitorSettingsMessage};
use icy_engine_gui::ui::*;
use icy_engine_gui::{dialog_wrapper, Dialog, DialogAction};
use icy_net::{
    modem::ModemConfiguration,
    serial::{CharSize, Parity, StopBits},
};
use icy_ui::{
    widget::{button, column, container, row, scrollable, text, Space},
    Border, Color, Element, Event, Length,
};

use crate::Options;

mod iemsi_settings;
mod modem_command_input;
mod modem_settings;
mod paths_settings;
mod protocol_settings;
mod terminal_settings;

pub use modem_command_input::*;

// Settings-specific constants
const SETTINGS_CONTENT_HEIGHT: f32 = 410.0;

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsCategory {
    Monitor,
    IEMSI,
    Terminal,
    Keybinds,
    Modem,
    Protocols,
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
            Self::Protocols => fl!(crate::LANGUAGE_LOADER, "settings-protocol-category"),
            Self::Paths => fl!(crate::LANGUAGE_LOADER, "settings-paths-category"),
        }
    }

    fn all() -> Vec<Self> {
        vec![
            Self::Monitor,
            Self::IEMSI,
            Self::Terminal,
            /*Self::Keybinds,*/ Self::Modem,
            Self::Protocols,
            Self::Paths,
        ]
    }
}

/// Result type for settings dialog - contains optional scrollback buffer size change
#[derive(Debug, Clone)]
pub struct SettingsResult {
    pub new_scrollback_size: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum SettingsDialogMessage {
    SwitchCategory(SettingsCategory),
    UpdateOptions(Options),
    ResetCategory(SettingsCategory),
    MonitorSettings(MonitorSettingsMessage),
    OpenSettingsFolder,
    OpenLogFile,
    SelectModem(usize),
    AddModem,
    RemoveModem(usize),
    SelectProtocol(usize),
    AddProtocol,
    RemoveProtocol(usize),
    MoveProtocolUp(usize),
    MoveProtocolDown(usize),
    ToggleProtocolEnabled(usize),
    UpdateDownloadPath(String),
    UpdateCapturePath(String),
    BrowseDownloadPath,
    BrowseCapturePath,
    ResetPaths,
    Save,
    Cancel,
    Noop,
}

#[dialog_wrapper(close_on_blur = false, result_type = SettingsResult)]
pub struct SettingsDialogState {
    pub current_category: SettingsCategory,
    pub temp_options: Arc<Mutex<Options>>,
    pub original_options: Arc<Mutex<Options>>,
    pub selected_modem_index: usize,
    pub selected_protocol_index: usize,
}

impl SettingsDialogState {
    pub fn new(original_options: Arc<Mutex<Options>>) -> Self {
        // Create temp_options as a clone of original_options for editing
        let temp_options = Arc::new(Mutex::new(original_options.lock().clone()));
        Self {
            current_category: SettingsCategory::Monitor,
            temp_options,
            original_options,
            selected_modem_index: 0,
            selected_protocol_index: 0,
        }
    }

    pub fn handle_message(&mut self, message: SettingsDialogMessage) -> StateResult<SettingsResult> {
        match message {
            SettingsDialogMessage::SwitchCategory(category) => {
                self.current_category = category;
                StateResult::None
            }
            SettingsDialogMessage::UpdateOptions(options) => {
                *self.temp_options.lock() = options;
                StateResult::None
            }
            SettingsDialogMessage::ResetCategory(category) => {
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
                    SettingsCategory::Protocols => {
                        let mut options = self.temp_options.lock();
                        options.transfer_protocols = crate::data::default_protocols();
                        drop(options);
                        self.selected_protocol_index = 0;
                    }
                    _ => {}
                }
                StateResult::None
            }
            SettingsDialogMessage::OpenSettingsFolder => {
                if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_term") {
                    if let Err(err) = open::that(proj_dirs.config_dir()) {
                        log::error!("Failed to open settings folder: {}", err);
                    }
                }
                StateResult::None
            }
            SettingsDialogMessage::OpenLogFile => {
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
                        if let Err(err) = open::that(parent) {
                            log::error!("Failed to open log file directory: {}", err);
                        }
                    }
                }
                StateResult::None
            }
            SettingsDialogMessage::Save => {
                // Save the options and close dialog
                let old_buffer_size = self.original_options.lock().max_scrollback_lines;
                let tmp = self.temp_options.lock().clone();
                let new_buffer_size = tmp.max_scrollback_lines;
                *self.original_options.lock() = tmp;
                if let Err(e) = self.original_options.lock().store_options() {
                    log::error!("Failed to save options: {}", e);
                }

                // Return result with scrollback size change if needed
                let new_scrollback_size = if old_buffer_size != new_buffer_size { Some(new_buffer_size) } else { None };

                StateResult::Success(SettingsResult { new_scrollback_size })
            }
            SettingsDialogMessage::Cancel => {
                // Reset to original options and close
                let tmp = self.original_options.lock().clone();
                *self.temp_options.lock() = tmp;
                StateResult::Close
            }
            SettingsDialogMessage::SelectModem(index) => {
                self.selected_modem_index = index;
                StateResult::None
            }
            SettingsDialogMessage::AddModem => {
                let len = self.temp_options.lock().modems.len();
                let new_modem = ModemConfiguration {
                    name: format!("Modem {}", len + 1),
                    ..Default::default()
                };
                self.temp_options.lock().modems.push(new_modem);
                self.selected_modem_index = len;
                StateResult::None
            }
            SettingsDialogMessage::RemoveModem(index) => {
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
                StateResult::None
            }
            SettingsDialogMessage::MonitorSettings(settings) => {
                update_monitor_settings(&mut self.temp_options.lock().monitor_settings, settings);
                StateResult::None
            }
            SettingsDialogMessage::UpdateDownloadPath(path) => {
                self.temp_options.lock().download_path = path;
                StateResult::None
            }
            SettingsDialogMessage::UpdateCapturePath(path) => {
                self.temp_options.lock().capture_path = path;
                StateResult::None
            }
            SettingsDialogMessage::BrowseDownloadPath => {
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
                StateResult::None
            }
            SettingsDialogMessage::BrowseCapturePath => {
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
                StateResult::None
            }
            SettingsDialogMessage::ResetPaths => {
                let mut options = self.temp_options.lock();
                options.download_path = String::new();
                options.capture_path = String::new();
                StateResult::None
            }
            SettingsDialogMessage::SelectProtocol(index) => {
                self.selected_protocol_index = index;
                StateResult::None
            }
            SettingsDialogMessage::AddProtocol => {
                use crate::data::TransferProtocol;
                let len = self.temp_options.lock().transfer_protocols.len();
                let new_protocol = TransferProtocol {
                    enabled: true,
                    id: format!("protocol_{}", len + 1),
                    name: format!("Protocol {}", len + 1),
                    ..Default::default()
                };
                self.temp_options.lock().transfer_protocols.push(new_protocol);
                self.selected_protocol_index = len;
                StateResult::None
            }
            SettingsDialogMessage::RemoveProtocol(index) => {
                let mut temp_options = self.temp_options.lock();
                if index < temp_options.transfer_protocols.len() {
                    // Don't allow removing internal protocols
                    if !temp_options.transfer_protocols[index].is_internal() {
                        temp_options.transfer_protocols.remove(index);
                        let new_len = temp_options.transfer_protocols.len();
                        if new_len > 0 {
                            self.selected_protocol_index = index.min(new_len - 1);
                        } else {
                            self.selected_protocol_index = 0;
                        }
                    }
                }
                StateResult::None
            }
            SettingsDialogMessage::MoveProtocolUp(index) => {
                let mut temp_options = self.temp_options.lock();
                if index > 0 && index < temp_options.transfer_protocols.len() {
                    temp_options.transfer_protocols.swap(index, index - 1);
                    self.selected_protocol_index = index - 1;
                }
                StateResult::None
            }
            SettingsDialogMessage::MoveProtocolDown(index) => {
                let mut temp_options = self.temp_options.lock();
                if index + 1 < temp_options.transfer_protocols.len() {
                    temp_options.transfer_protocols.swap(index, index + 1);
                    self.selected_protocol_index = index + 1;
                }
                StateResult::None
            }
            SettingsDialogMessage::ToggleProtocolEnabled(index) => {
                let mut temp_options = self.temp_options.lock();
                if index < temp_options.transfer_protocols.len() {
                    let protocol = &mut temp_options.transfer_protocols[index];
                    protocol.enabled = !protocol.enabled;
                }
                StateResult::None
            }
            SettingsDialogMessage::Noop => StateResult::None,
        }
    }

    /// Build just the dialog content (for use with Dialog trait)
    pub fn view<'a, M: Clone + 'static>(&'a self, on_message: impl Fn(SettingsDialogMessage) -> M + Clone + 'static) -> Element<'a, M> {
        // Category tabs
        let mut category_row = row![].spacing(DIALOG_SPACING);
        for category in SettingsCategory::all() {
            let is_selected = self.current_category == category;
            let cat = category.clone();
            let on_msg = on_message.clone();
            let cat_button = button(text(category.name()).size(TEXT_SIZE_NORMAL).wrapping(text::Wrapping::None))
                .on_press(on_msg(SettingsDialogMessage::SwitchCategory(cat)))
                .style(move |theme: &icy_ui::Theme, status| {
                    use icy_ui::widget::button::{Status, Style};

                    let base = if is_selected {
                        Style {
                            background: Some(icy_ui::Background::Color(theme.accent.selected)),
                            text_color: theme.accent.on,
                            border: Border::default().rounded(4.0),
                            shadow: Default::default(),
                            snap: false,
                            ..Default::default()
                        }
                    } else {
                        Style {
                            background: Some(icy_ui::Background::Color(Color::TRANSPARENT)),
                            text_color: theme.background.on,
                            border: Border::default().rounded(4.0),
                            shadow: Default::default(),
                            snap: false,
                            ..Default::default()
                        }
                    };

                    match status {
                        Status::Active | Status::Selected => base,
                        Status::Hovered if !is_selected => Style {
                            background: Some(icy_ui::Background::Color(Color::from_rgba(
                                theme.accent.selected.r,
                                theme.accent.selected.g,
                                theme.accent.selected.b,
                                0.2,
                            ))),
                            ..base
                        },
                        Status::Pressed => Style {
                            background: Some(icy_ui::Background::Color(theme.accent.hover)),
                            ..base
                        },
                        _ => base,
                    }
                })
                .padding([6, 12]);
            category_row = category_row.push(cat_button);
        }

        // Settings content for current category
        let settings_content: Element<'_, M> = match self.current_category {
            SettingsCategory::Monitor => {
                let monitor_settings = self.temp_options.lock().monitor_settings.clone();
                let on_msg = on_message.clone();
                show_monitor_settings(monitor_settings).map(move |msg| on_msg(SettingsDialogMessage::MonitorSettings(msg)))
            }
            SettingsCategory::IEMSI => self.iemsi_settings_content_generic(on_message.clone()),
            SettingsCategory::Terminal => self.terminal_settings_content_generic(on_message.clone()),
            SettingsCategory::Keybinds => self.keybinds_settings_content_generic(),
            SettingsCategory::Modem => self.modem_settings_content_generic(on_message.clone()),
            SettingsCategory::Protocols => self.protocol_settings_content_generic(on_message.clone()),
            SettingsCategory::Paths => {
                let options = self.temp_options.lock();
                let download_path = options.download_path();
                let capture_path = options.capture_path();
                drop(options);
                paths_settings::paths_settings_content_generic(download_path, capture_path, on_message.clone())
            }
        };

        // Buttons
        let on_msg = on_message.clone();
        let ok_button = primary_button(format!("{}", icy_engine_gui::ButtonType::Ok), Some(on_msg(SettingsDialogMessage::Save)));

        let on_msg = on_message.clone();
        let cancel_button = secondary_button(format!("{}", icy_engine_gui::ButtonType::Cancel), Some(on_msg(SettingsDialogMessage::Cancel)));

        let reset_button: Option<Element<'_, M>> = match self.current_category {
            SettingsCategory::Monitor => {
                let current_settings = self.temp_options.lock().monitor_settings.clone();
                let default_settings = icy_engine_gui::MonitorSettings::default();
                let is_default = current_settings == default_settings;
                Some(
                    icy_engine_gui::ui::restore_defaults_button(!is_default, on_message(SettingsDialogMessage::ResetCategory(self.current_category.clone())))
                        .into(),
                )
            }
            SettingsCategory::Terminal => None,
            SettingsCategory::IEMSI => None,
            SettingsCategory::Keybinds => {
                // TODO: Add default keybindings check when implemented
                Some(icy_engine_gui::ui::restore_defaults_button(true, on_message(SettingsDialogMessage::ResetCategory(self.current_category.clone()))).into())
            }
            SettingsCategory::Paths => {
                let options = self.temp_options.lock();
                let is_default = options.download_path.is_empty() && options.capture_path.is_empty();
                drop(options);
                Some(
                    icy_engine_gui::ui::restore_defaults_button(!is_default, on_message(SettingsDialogMessage::ResetCategory(self.current_category.clone())))
                        .into(),
                )
            }
            SettingsCategory::Protocols => {
                let options = self.temp_options.lock();
                let current_protocols = options.transfer_protocols.clone();
                drop(options);
                let default_protocols = crate::data::default_protocols();
                let is_default = current_protocols == default_protocols;
                Some(
                    icy_engine_gui::ui::restore_defaults_button(!is_default, on_message(SettingsDialogMessage::ResetCategory(self.current_category.clone())))
                        .into(),
                )
            }
            _ => None,
        };

        let mut buttons_left: Vec<Element<'_, M>> = vec![];
        if let Some(reset_btn) = reset_button {
            buttons_left.push(reset_btn);
        }

        let buttons_right: Vec<Element<'_, M>> = vec![cancel_button.into(), ok_button.into()];

        let button_area_row = button_row_with_left(buttons_left, buttons_right);

        // For modem and protocol settings, don't wrap in scrollable since they have their own layout
        let content_container = if matches!(self.current_category, SettingsCategory::Modem | SettingsCategory::Protocols) {
            container(settings_content).height(Length::Fixed(SETTINGS_CONTENT_HEIGHT)).width(Length::Fill)
        } else {
            container(scrollable(settings_content).direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default())))
                .height(Length::Fixed(SETTINGS_CONTENT_HEIGHT))
                .width(Length::Fill)
                .padding(0.0)
        };

        let dialog_content = dialog_area(column![category_row, Space::new().height(DIALOG_SPACING), content_container,].into());

        let button_area_wrapped = dialog_area(button_area_row.into());

        modal_container(
            column![container(dialog_content).height(Length::Fill), separator(), button_area_wrapped,].into(),
            DIALOG_WIDTH_XARGLE,
        )
        .into()
    }

    fn keybinds_settings_content_generic<'a, M: Clone + 'static>(&self) -> Element<'a, M> {
        // TODO: Implement keybindings editor
        column![text("Keybindings editor - TODO: Implement keybinding controls").size(TEXT_SIZE_NORMAL),].into()
    }

    /// Get the current theme for live preview
    pub fn get_theme(&self) -> icy_ui::Theme {
        self.temp_options.lock().monitor_settings.get_theme()
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
struct ThemeOption(icy_ui::Theme);

impl ThemeOption {
    fn all() -> Vec<ThemeOption> {
        icy_ui::Theme::all().into_iter().map(ThemeOption).collect()
    }
}

impl From<icy_ui::Theme> for ThemeOption {
    fn from(value: icy_ui::Theme) -> Self {
        ThemeOption(value)
    }
}

impl From<ThemeOption> for icy_ui::Theme {
    fn from(value: ThemeOption) -> Self {
        value.0
    }
}

impl std::fmt::Display for ThemeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.name)
    }
}

// ============================================================================
// Builder functions for settings dialog
// ============================================================================

/// Create a settings dialog for use with DialogStack using an existing state
pub fn settings_dialog_with_state<M, F, E>(state: SettingsDialogState, on_message: F, extract_message: E) -> SettingsDialogWrapperWithTheme<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(SettingsDialogMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&SettingsDialogMessage> + Clone + 'static,
{
    SettingsDialogWrapperWithTheme {
        inner: SettingsDialogWrapper::new(state, on_message, extract_message),
    }
}

/// Creates a settings dialog wrapper using a tuple of (on_message, extract_message).
///
/// This is a convenience function to use with the `dialog_msg!` macro:
/// ```ignore
/// use icy_engine_gui::dialog_msg;
/// dialog_stack.push(settings_dialog_from_msg(
///     state,
///     dialog_msg!(Message::SettingsDialog),
/// ));
/// ```
pub fn settings_dialog_from_msg<M, F, E>(state: SettingsDialogState, msg_tuple: (F, E)) -> SettingsDialogWrapperWithTheme<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(SettingsDialogMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&SettingsDialogMessage> + Clone + 'static,
{
    settings_dialog_with_state(state, msg_tuple.0, msg_tuple.1)
}

// ============================================================================
// Custom wrapper that adds theme() support
// ============================================================================

/// A wrapper around SettingsDialogWrapper that adds theme() support for live preview.
/// This is needed because the dialog_wrapper macro doesn't support custom theme methods.
pub struct SettingsDialogWrapperWithTheme<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(SettingsDialogMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&SettingsDialogMessage> + Clone + 'static,
{
    inner: SettingsDialogWrapper<M, F, E>,
}

impl<M, F, E> SettingsDialogWrapperWithTheme<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(SettingsDialogMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&SettingsDialogMessage> + Clone + 'static,
{
    /// Set callback for successful save (confirm).
    pub fn on_save<G>(mut self, callback: G) -> Self
    where
        G: Fn(SettingsResult) -> M + Send + 'static,
    {
        self.inner = self.inner.on_confirm(callback);
        self
    }

    /// Set callback for cancel/close.
    pub fn on_cancel<G>(mut self, callback: G) -> Self
    where
        G: Fn() -> M + Send + 'static,
    {
        self.inner = self.inner.on_cancel(callback);
        self
    }
}

impl<M, F, E> Dialog<M> for SettingsDialogWrapperWithTheme<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(SettingsDialogMessage) -> M + Clone + Send + 'static,
    E: Fn(&M) -> Option<&SettingsDialogMessage> + Clone + Send + 'static,
{
    fn view(&self) -> Element<'_, M> {
        self.inner.view()
    }

    fn update(&mut self, message: &M) -> Option<DialogAction<M>> {
        self.inner.update(message)
    }

    fn request_cancel(&mut self) -> DialogAction<M> {
        self.inner.request_cancel()
    }

    fn request_confirm(&mut self) -> DialogAction<M> {
        self.inner.request_confirm()
    }

    fn handle_event(&mut self, event: &Event) -> Option<DialogAction<M>> {
        self.inner.handle_event(event)
    }

    fn close_on_blur(&self) -> bool {
        false // Settings dialog should not close on blur
    }

    fn theme(&self) -> Option<icy_ui::Theme> {
        // Return the theme from temp_options so changes are previewed live
        Some(self.inner.state.get_theme())
    }
}
