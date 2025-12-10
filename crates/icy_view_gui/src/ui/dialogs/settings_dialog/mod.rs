use parking_lot::Mutex;
use std::sync::Arc;

use i18n_embed_fl::fl;
use iced::{
    Border, Color, Element, Event, Length,
    widget::{Space, button, column, container, row, scrollable, text},
};
use icy_engine_gui::settings::{MonitorSettingsMessage, show_monitor_settings, update_monitor_settings};
use icy_engine_gui::ui::*;
use icy_engine_gui::{Dialog, DialogAction, dialog_wrapper};

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
pub enum SettingsDialogMessage {
    SwitchCategory(SettingsCategory),
    MonitorSettings(MonitorSettingsMessage),
    ResetMonitorSettings,
    ExternalCommandChanged(usize, String),
    UpdateExportPath(String),
    BrowseExportPath,
    OpenSettingsFolder,
    OpenLogFile,
    Save,
    Cancel,
}

#[dialog_wrapper]
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

    pub fn handle_message(&mut self, message: SettingsDialogMessage) -> StateResult<()> {
        match message {
            SettingsDialogMessage::SwitchCategory(category) => {
                self.current_category = category;
                StateResult::None
            }
            SettingsDialogMessage::OpenSettingsFolder => {
                if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_view") {
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
                StateResult::None
            }
            SettingsDialogMessage::Save => {
                // Apply the changes
                let tmp = self.temp_options.lock().clone();
                *self.original_options.lock() = tmp;
                self.original_options.lock().store_options();
                StateResult::Success(())
            }
            SettingsDialogMessage::Cancel => {
                // Revert to original
                let tmp = self.original_options.lock().clone();
                *self.temp_options.lock() = tmp;
                StateResult::Close
            }
            SettingsDialogMessage::MonitorSettings(settings) => {
                update_monitor_settings(&mut self.temp_options.lock().monitor_settings, settings);
                StateResult::None
            }
            SettingsDialogMessage::ResetMonitorSettings => {
                self.temp_options.lock().monitor_settings = icy_engine_gui::MonitorSettings::default();
                StateResult::None
            }
            SettingsDialogMessage::ExternalCommandChanged(idx, cmd) => {
                self.temp_options.lock().external_commands[idx].command = cmd;
                StateResult::None
            }
            SettingsDialogMessage::UpdateExportPath(path) => {
                self.temp_options.lock().export_path = path;
                StateResult::None
            }
            SettingsDialogMessage::BrowseExportPath => {
                let mut opt = self.temp_options.lock();
                if let Some(folder) = rfd::FileDialog::new().set_directory(opt.export_path()).pick_folder() {
                    opt.export_path = folder.to_string_lossy().to_string();
                }
                StateResult::None
            }
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
        let on_msg = on_message.clone();
        let settings_content = match self.current_category {
            SettingsCategory::Monitor => {
                let monitor_settings = self.temp_options.lock().monitor_settings.clone();
                show_monitor_settings(monitor_settings).map(move |msg| on_msg(SettingsDialogMessage::MonitorSettings(msg)))
            }
            SettingsCategory::Commands => {
                let commands = self.temp_options.lock().external_commands.clone();
                command_settings::commands_settings_content_generic(commands, on_message.clone())
            }
            SettingsCategory::Paths => {
                let export_path = self.temp_options.lock().export_path.clone();
                paths_settings::paths_settings_content_generic(export_path, on_message.clone())
            }
        };

        // Buttons
        let on_msg = on_message.clone();
        let ok_button = primary_button(format!("{}", icy_engine_gui::ButtonType::Ok), Some(on_msg(SettingsDialogMessage::Save)));

        let on_msg = on_message.clone();
        let cancel_button = secondary_button(format!("{}", icy_engine_gui::ButtonType::Cancel), Some(on_msg(SettingsDialogMessage::Cancel)));

        // Reset button for Monitor category
        let reset_button = match self.current_category {
            SettingsCategory::Monitor => {
                let current_settings = self.temp_options.lock().monitor_settings.clone();
                let default_settings = icy_engine_gui::MonitorSettings::default();
                let is_default = current_settings == default_settings;
                Some(icy_engine_gui::ui::restore_defaults_button(
                    !is_default,
                    on_message(SettingsDialogMessage::ResetMonitorSettings),
                ))
            }
            _ => None,
        };

        let mut buttons_left = vec![];
        if let Some(reset_btn) = reset_button {
            buttons_left.push(reset_btn.into());
        }

        let buttons_right = vec![cancel_button.into(), ok_button.into()];

        let button_area_row = icy_engine_gui::ui::button_row_with_left(buttons_left, buttons_right);

        let content_container = container(scrollable(settings_content).direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default())))
            .height(Length::Fixed(SETTINGS_CONTENT_HEIGHT))
            .width(Length::Fill)
            .padding(0.0);

        let dialog_content = dialog_area(column![category_row, Space::new().height(DIALOG_SPACING), content_container,].into());

        let button_area_wrapped = dialog_area(button_area_row.into());

        modal_container(
            column![container(dialog_content).height(Length::Fill), separator(), button_area_wrapped,].into(),
            DIALOG_WIDTH_XARGLE,
        )
        .into()
    }

    /// Get the current theme for live preview
    pub fn get_theme(&self) -> iced::Theme {
        self.temp_options.lock().monitor_settings.get_theme()
    }
}

// ============================================================================
// Builder functions for settings dialog
// ============================================================================

/// Create a settings dialog for use with DialogStack
///
/// # Example
/// ```ignore
/// dialog_stack.push(settings_dialog(
///     options.clone(),
///     temp_options.clone(),
///     Message::SettingsDialog,
///     |msg| match msg { Message::SettingsDialog(m) => Some(m), _ => None },
/// ));
/// ```
pub fn settings_dialog<M, F, E>(
    original_options: Arc<Mutex<Options>>,
    temp_options: Arc<Mutex<Options>>,
    on_message: F,
    extract_message: E,
) -> SettingsDialogWrapperWithTheme<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(SettingsDialogMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&SettingsDialogMessage> + Clone + 'static,
{
    SettingsDialogWrapperWithTheme {
        inner: SettingsDialogWrapper::new(
            SettingsDialogState::new(original_options, temp_options),
            on_message,
            extract_message,
        ),
    }
}

/// Creates a settings dialog wrapper using a tuple of (on_message, extract_message).
///
/// This is a convenience function to use with the `dialog_msg!` macro:
/// ```ignore
/// use icy_engine_gui::dialog_msg;
/// dialog_stack.push(settings_dialog_from_msg(
///     options.clone(),
///     temp_options.clone(),
///     dialog_msg!(Message::SettingsDialog),
/// ));
/// ```
pub fn settings_dialog_from_msg<M, F, E>(
    original_options: Arc<Mutex<Options>>,
    temp_options: Arc<Mutex<Options>>,
    msg_tuple: (F, E),
) -> SettingsDialogWrapperWithTheme<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(SettingsDialogMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&SettingsDialogMessage> + Clone + 'static,
{
    settings_dialog(original_options, temp_options, msg_tuple.0, msg_tuple.1)
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
        G: Fn() -> M + Send + 'static,
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

    fn theme(&self) -> Option<iced::Theme> {
        // Return the theme from temp_options so changes are previewed live
        Some(self.inner.state.get_theme())
    }
}

