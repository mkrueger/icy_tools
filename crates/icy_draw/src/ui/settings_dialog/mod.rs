use std::sync::Arc;

use iced::{
    Border, Color, Element, Event, Length,
    keyboard::{Key, key::Named},
    widget::{Space, button, column, container, row, scrollable, text, text_input},
};
use icy_engine::BitFont;
use icy_engine_gui::settings::{MonitorSettingsMessage, effect_box, left_label, show_monitor_settings, update_monitor_settings};
use icy_engine_gui::ui::*;
use icy_engine_gui::{Dialog, DialogAction, MonitorSettings};
use parking_lot::RwLock;

use crate::fl;
use crate::ui::{FKeySets, Options};

mod charset_picker;
mod outline_picker;

use charset_picker::view_charset;

const SETTINGS_CONTENT_HEIGHT: f32 = 410.0;

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsCategory {
    Monitor,
    FontOutline,
    Charset,
    Paths,
}

impl SettingsCategory {
    fn name(&self) -> String {
        match self {
            Self::Monitor => fl!("settings-monitor-category"),
            Self::FontOutline => fl!("settings-font-outline-category"),
            Self::Charset => fl!("settings-char-set-category"),
            Self::Paths => fl!("settings-paths-category"),
        }
    }

    fn all() -> Vec<Self> {
        vec![Self::Monitor, Self::FontOutline, Self::Charset, Self::Paths]
    }
}

#[derive(Debug, Clone)]
pub enum SettingsDialogMessage {
    SwitchCategory(SettingsCategory),

    MonitorSettings(MonitorSettingsMessage),
    ResetMonitorSettings,

    SelectOutlineStyle(usize),
    ResetOutlineStyle,

    PrevCharsetSet,
    NextCharsetSet,
    SelectCharsetSlot(usize),
    SelectCharFromGrid(u8),
    ResetCurrentCharset,

    OpenConfigDir,
    OpenLogFile,
    OpenFontDir,
    OpenPluginDir,

    Save,
    Cancel,
}

#[derive(Debug, Clone)]
pub struct SettingsResult;

pub struct SettingsDialog {
    options: Arc<RwLock<Options>>,

    current_category: SettingsCategory,
    temp_monitor_settings: MonitorSettings,
    temp_outline_style: usize,
    outline_cursor: usize,
    temp_fkeys: FKeySets,

    charset_font: BitFont,
    selected_charset_slot: Option<usize>,
    charset_cursor: u8,
}

impl SettingsDialog {
    pub fn new(options: Arc<RwLock<Options>>, preview_font: Option<BitFont>) -> Self {
        let (monitor_settings, outline_style, fkeys) = {
            let guard = options.read();
            (guard.monitor_settings.read().clone(), *guard.font_outline_style.read(), guard.fkeys.clone())
        };

        let mut temp_fkeys = fkeys;
        temp_fkeys.clamp_current_set();

        let charset_font = preview_font.unwrap_or_else(BitFont::default);

        Self {
            options,
            current_category: SettingsCategory::Monitor,
            temp_monitor_settings: monitor_settings,
            temp_outline_style: outline_style,
            outline_cursor: outline_style,
            temp_fkeys,
            charset_font,
            selected_charset_slot: None,
            charset_cursor: 0,
        }
    }

    fn view_category_tabs(&self) -> Element<'_, crate::ui::main_window::Message> {
        let mut category_row = row![].spacing(DIALOG_SPACING);
        for category in SettingsCategory::all() {
            let is_selected = self.current_category == category;
            let cat = category.clone();
            let cat_button = button(text(category.name()).size(TEXT_SIZE_NORMAL).wrapping(text::Wrapping::None))
                .on_press(crate::ui::main_window::Message::SettingsDialog(SettingsDialogMessage::SwitchCategory(cat)))
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
        category_row.into()
    }

    fn view_monitor(&self) -> Element<'_, crate::ui::main_window::Message> {
        let content = show_monitor_settings(self.temp_monitor_settings.clone())
            .map(|msg| crate::ui::main_window::Message::SettingsDialog(SettingsDialogMessage::MonitorSettings(msg)));
        content
    }

    fn view_outline(&self) -> Element<'_, crate::ui::main_window::Message> {
        let picker =
            outline_picker::outline_picker(self.temp_outline_style, self.outline_cursor).map(|msg| crate::ui::main_window::Message::SettingsDialog(msg));

        let help_text = text("Type letter A-S  |  ←↑↓→: Navigate  |  Enter: Confirm")
            .size(TEXT_SIZE_SMALL)
            .color(iced::Color::from_rgb(0.5, 0.5, 0.5));

        column![
            section_header(fl!("settings-font-outline-header")),
            container(effect_box(
                column![picker, Space::new().height(Length::Fixed(8.0)), help_text,].spacing(0).into()
            ))
            .width(Length::Fill)
            .center_x(Length::Fill),
        ]
        .spacing(0)
        .into()
    }

    fn view_charset(&self) -> Element<'_, crate::ui::main_window::Message> {
        view_charset(&self.charset_font, &self.temp_fkeys, self.selected_charset_slot, self.charset_cursor)
    }

    fn view_paths(&self) -> Element<'_, crate::ui::main_window::Message> {
        let config_dir = Options::config_dir().map(|p| p.display().to_string()).unwrap_or_else(|| "N/A".to_string());
        let config_file = Options::config_file().map(|p| p.display().to_string()).unwrap_or_else(|| "N/A".to_string());
        let log_file = Options::log_file().map(|p| p.display().to_string()).unwrap_or_else(|| "N/A".to_string());
        let font_dir = Options::font_dir().map(|p| p.display().to_string()).unwrap_or_else(|| "N/A".to_string());
        let plugin_dir = Options::plugin_dir().map(|p| p.display().to_string()).unwrap_or_else(|| "N/A".to_string());

        let inner: Element<'_, crate::ui::main_window::Message> = column![
            row![
                left_label(fl!("settings-paths-config-dir")),
                text_input("", &config_dir).size(TEXT_SIZE_NORMAL).width(Length::Fill),
                browse_button(crate::ui::main_window::Message::SettingsDialog(SettingsDialogMessage::OpenConfigDir)),
            ]
            .spacing(DIALOG_SPACING)
            .align_y(iced::Alignment::Center),
            row![
                left_label(fl!("settings-paths-config-file")),
                text_input("", &config_file).size(TEXT_SIZE_NORMAL).width(Length::Fill),
            ]
            .spacing(DIALOG_SPACING)
            .align_y(iced::Alignment::Center),
            row![
                left_label(fl!("settings-paths-log-file")),
                text_input("", &log_file).size(TEXT_SIZE_NORMAL).width(Length::Fill),
                secondary_button(
                    fl!("settings-paths-open"),
                    Some(crate::ui::main_window::Message::SettingsDialog(SettingsDialogMessage::OpenLogFile)),
                ),
            ]
            .spacing(DIALOG_SPACING)
            .align_y(iced::Alignment::Center),
            row![
                left_label(fl!("settings-paths-font-dir")),
                text_input("", &font_dir).size(TEXT_SIZE_NORMAL).width(Length::Fill),
                browse_button(crate::ui::main_window::Message::SettingsDialog(SettingsDialogMessage::OpenFontDir)),
            ]
            .spacing(DIALOG_SPACING)
            .align_y(iced::Alignment::Center),
            row![
                left_label(fl!("settings-paths-plugin-dir")),
                text_input("", &plugin_dir).size(TEXT_SIZE_NORMAL).width(Length::Fill),
                browse_button(crate::ui::main_window::Message::SettingsDialog(SettingsDialogMessage::OpenPluginDir)),
            ]
            .spacing(DIALOG_SPACING)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(DIALOG_SPACING)
        .into();

        column![section_header(fl!("settings-paths-header")), effect_box(inner),].spacing(0).into()
    }

    fn view_settings_content(&self) -> Element<'_, crate::ui::main_window::Message> {
        match self.current_category {
            SettingsCategory::Monitor => self.view_monitor(),
            SettingsCategory::FontOutline => self.view_outline(),
            SettingsCategory::Charset => self.view_charset(),
            SettingsCategory::Paths => self.view_paths(),
        }
    }
}

impl Dialog<crate::ui::main_window::Message> for SettingsDialog {
    fn view(&self) -> Element<'_, crate::ui::main_window::Message> {
        let category_tabs = self.view_category_tabs();

        let settings_content = self.view_settings_content();

        let content_container = container(scrollable(settings_content).direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default())))
            .height(Length::Fixed(SETTINGS_CONTENT_HEIGHT))
            .width(Length::Fill)
            .padding(0.0);

        let ok_button = primary_button(
            format!("{}", icy_engine_gui::ButtonType::Ok),
            Some(crate::ui::main_window::Message::SettingsDialog(SettingsDialogMessage::Save)),
        );

        let cancel_button = secondary_button(
            format!("{}", icy_engine_gui::ButtonType::Cancel),
            Some(crate::ui::main_window::Message::SettingsDialog(SettingsDialogMessage::Cancel)),
        );

        let reset_button: Option<Element<'_, crate::ui::main_window::Message>> = match self.current_category {
            SettingsCategory::Monitor => {
                let is_default = self.temp_monitor_settings == MonitorSettings::default();
                Some(
                    icy_engine_gui::ui::restore_defaults_button(
                        !is_default,
                        crate::ui::main_window::Message::SettingsDialog(SettingsDialogMessage::ResetMonitorSettings),
                    )
                    .into(),
                )
            }
            SettingsCategory::FontOutline => {
                let is_default = self.temp_outline_style == 0;
                Some(
                    icy_engine_gui::ui::restore_defaults_button(
                        !is_default,
                        crate::ui::main_window::Message::SettingsDialog(SettingsDialogMessage::ResetOutlineStyle),
                    )
                    .into(),
                )
            }
            SettingsCategory::Charset => {
                let set_idx = self.temp_fkeys.current_set();
                let is_default = self.temp_fkeys.is_set_default(set_idx);
                Some(
                    icy_engine_gui::ui::restore_defaults_button(
                        !is_default,
                        crate::ui::main_window::Message::SettingsDialog(SettingsDialogMessage::ResetCurrentCharset),
                    )
                    .into(),
                )
            }
            _ => None,
        };

        let buttons_left = reset_button.map(|b| vec![b]).unwrap_or_default();
        let buttons_right = vec![cancel_button.into(), ok_button.into()];
        let button_area_row = icy_engine_gui::ui::button_row_with_left(buttons_left, buttons_right);

        let dialog_content = dialog_area(column![category_tabs, Space::new().height(DIALOG_SPACING), content_container].into());
        let button_area = dialog_area(button_area_row.into());

        modal_container(
            column![container(dialog_content).height(Length::Fill), separator(), button_area].into(),
            DIALOG_WIDTH_XARGLE,
        )
        .into()
    }

    fn update(&mut self, message: &crate::ui::main_window::Message) -> Option<DialogAction<crate::ui::main_window::Message>> {
        let crate::ui::main_window::Message::SettingsDialog(msg) = message else {
            return None;
        };

        match msg {
            SettingsDialogMessage::SwitchCategory(cat) => {
                self.current_category = cat.clone();
                Some(DialogAction::None)
            }
            SettingsDialogMessage::MonitorSettings(m) => {
                update_monitor_settings(&mut self.temp_monitor_settings, m.clone());
                Some(DialogAction::None)
            }
            SettingsDialogMessage::ResetMonitorSettings => {
                self.temp_monitor_settings = MonitorSettings::default();
                Some(DialogAction::None)
            }
            SettingsDialogMessage::SelectOutlineStyle(style) => {
                self.temp_outline_style = *style;
                self.outline_cursor = *style;
                Some(DialogAction::None)
            }
            SettingsDialogMessage::ResetOutlineStyle => {
                self.temp_outline_style = 0;
                self.outline_cursor = 0;
                Some(DialogAction::None)
            }
            SettingsDialogMessage::PrevCharsetSet => {
                let count = self.temp_fkeys.set_count();
                if count > 0 {
                    self.temp_fkeys.current_set = self.temp_fkeys.current_set.saturating_sub(1);
                    self.temp_fkeys.clamp_current_set();
                    self.selected_charset_slot = None;
                }
                Some(DialogAction::None)
            }
            SettingsDialogMessage::NextCharsetSet => {
                let count = self.temp_fkeys.set_count();
                if count > 0 {
                    self.temp_fkeys.current_set = (self.temp_fkeys.current_set + 1).min(count - 1);
                    self.temp_fkeys.clamp_current_set();
                    self.selected_charset_slot = None;
                }
                Some(DialogAction::None)
            }
            SettingsDialogMessage::SelectCharsetSlot(slot) => {
                self.selected_charset_slot = Some(*slot);
                // Set cursor to current char of this slot
                let set_idx = self.temp_fkeys.current_set();
                self.charset_cursor = self.temp_fkeys.code_at(set_idx, *slot) as u8;
                Some(DialogAction::None)
            }
            SettingsDialogMessage::SelectCharFromGrid(char_code) => {
                if let Some(slot) = self.selected_charset_slot {
                    let set_idx = self.temp_fkeys.current_set();
                    self.temp_fkeys.set_code_at(set_idx, slot, *char_code as u16);
                }
                self.charset_cursor = *char_code;
                Some(DialogAction::None)
            }
            SettingsDialogMessage::ResetCurrentCharset => {
                let set_idx = self.temp_fkeys.current_set();
                self.temp_fkeys.reset_set(set_idx);
                self.selected_charset_slot = None;
                Some(DialogAction::None)
            }
            SettingsDialogMessage::OpenConfigDir => {
                if let Some(dir) = Options::config_dir() {
                    let _ = std::fs::create_dir_all(&dir);
                    if let Err(err) = open::that(&dir) {
                        log::error!("Failed to open config dir: {}", err);
                    }
                }
                Some(DialogAction::None)
            }
            SettingsDialogMessage::OpenLogFile => {
                if let Some(log_file) = Options::log_file() {
                    if log_file.exists() {
                        #[cfg(windows)]
                        {
                            let _ = std::process::Command::new("notepad").arg(&log_file).spawn();
                        }
                        #[cfg(not(windows))]
                        {
                            if let Err(err) = open::that(&log_file) {
                                log::error!("Failed to open log file: {}", err);
                            }
                        }
                    } else if let Some(parent) = log_file.parent() {
                        if let Err(err) = open::that(parent) {
                            log::error!("Failed to open log directory: {}", err);
                        }
                    }
                }
                Some(DialogAction::None)
            }
            SettingsDialogMessage::OpenFontDir => {
                if let Some(dir) = Options::font_dir() {
                    let _ = std::fs::create_dir_all(&dir);
                    if let Err(err) = open::that(&dir) {
                        log::error!("Failed to open font dir: {}", err);
                    }
                }
                Some(DialogAction::None)
            }
            SettingsDialogMessage::OpenPluginDir => {
                if let Some(dir) = Options::plugin_dir() {
                    let _ = std::fs::create_dir_all(&dir);
                    if let Err(err) = open::that(&dir) {
                        log::error!("Failed to open plugin dir: {}", err);
                    }
                }
                Some(DialogAction::None)
            }
            SettingsDialogMessage::Save => {
                // Apply to shared options + persist
                {
                    let mut guard = self.options.write();
                    *guard.monitor_settings.write() = self.temp_monitor_settings.clone();
                    *guard.font_outline_style.write() = self.temp_outline_style;
                    guard.fkeys = self.temp_fkeys.clone();
                    guard.store_persistent();
                    if let Err(err) = guard.fkeys.save() {
                        log::error!("Failed to save fkeys: {}", err);
                    }
                }

                Some(DialogAction::CloseWith(crate::ui::main_window::Message::SettingsSaved(SettingsResult)))
            }
            SettingsDialogMessage::Cancel => Some(DialogAction::Close),
        }
    }

    fn request_cancel(&mut self) -> DialogAction<crate::ui::main_window::Message> {
        DialogAction::Close
    }

    fn request_confirm(&mut self) -> DialogAction<crate::ui::main_window::Message> {
        {
            let mut guard = self.options.write();
            *guard.monitor_settings.write() = self.temp_monitor_settings.clone();
            *guard.font_outline_style.write() = self.temp_outline_style;
            guard.fkeys = self.temp_fkeys.clone();
            guard.store_persistent();
            if let Err(err) = guard.fkeys.save() {
                log::error!("Failed to save fkeys: {}", err);
            }
        }

        DialogAction::CloseWith(crate::ui::main_window::Message::SettingsSaved(SettingsResult))
    }

    fn theme(&self) -> Option<iced::Theme> {
        // Return the theme from temp_monitor_settings so changes are previewed live
        Some(self.temp_monitor_settings.get_theme())
    }

    fn handle_event(&mut self, event: &Event) -> Option<DialogAction<crate::ui::main_window::Message>> {
        let Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
            return None;
        };

        // Handle FontOutline category keyboard events
        if self.current_category == SettingsCategory::FontOutline {
            const COLS: usize = 7;
            const TOTAL_STYLES: usize = 19;

            match key {
                // TheDraw shortcuts: A-S (original TheDraw keys)
                Key::Character(c) => {
                    let ch = c.as_str().chars().next().unwrap_or(' ');
                    let style_idx = match ch.to_ascii_uppercase() {
                        'A' => Some(0),
                        'B' => Some(1),
                        'C' => Some(2),
                        'D' => Some(3),
                        'E' => Some(4),
                        'F' => Some(5),
                        'G' => Some(6),
                        'H' => Some(7),
                        'I' => Some(8),
                        'J' => Some(9),
                        'K' => Some(10),
                        'L' => Some(11),
                        'M' => Some(12),
                        'N' => Some(13),
                        'O' => Some(14),
                        'P' => Some(15),
                        'Q' => Some(16),
                        'R' => Some(17),
                        'S' => Some(18),
                        _ => None,
                    };
                    if let Some(idx) = style_idx {
                        self.outline_cursor = idx;
                        self.temp_outline_style = idx;
                        return Some(DialogAction::None);
                    }
                    return None;
                }
                Key::Named(Named::ArrowLeft) => {
                    if self.outline_cursor > 0 {
                        self.outline_cursor -= 1;
                    } else {
                        self.outline_cursor = TOTAL_STYLES - 1;
                    }
                    return Some(DialogAction::None);
                }
                Key::Named(Named::ArrowRight) => {
                    if self.outline_cursor < TOTAL_STYLES - 1 {
                        self.outline_cursor += 1;
                    } else {
                        self.outline_cursor = 0;
                    }
                    return Some(DialogAction::None);
                }
                Key::Named(Named::ArrowUp) => {
                    if self.outline_cursor >= COLS {
                        self.outline_cursor -= COLS;
                    } else {
                        // Wrap to last row
                        let col = self.outline_cursor;
                        let last_row_idx = (TOTAL_STYLES - 1) / COLS * COLS + col;
                        self.outline_cursor = last_row_idx.min(TOTAL_STYLES - 1);
                    }
                    return Some(DialogAction::None);
                }
                Key::Named(Named::ArrowDown) => {
                    if self.outline_cursor + COLS < TOTAL_STYLES {
                        self.outline_cursor += COLS;
                    } else {
                        // Wrap to first row
                        let col = self.outline_cursor % COLS;
                        self.outline_cursor = col;
                    }
                    return Some(DialogAction::None);
                }
                Key::Named(Named::Home) => {
                    if modifiers.control() {
                        self.outline_cursor = 0;
                    } else {
                        // Start of row
                        let row = self.outline_cursor / COLS;
                        self.outline_cursor = row * COLS;
                    }
                    return Some(DialogAction::None);
                }
                Key::Named(Named::End) => {
                    if modifiers.control() {
                        self.outline_cursor = TOTAL_STYLES - 1;
                    } else {
                        // End of row
                        let row = self.outline_cursor / COLS;
                        self.outline_cursor = ((row + 1) * COLS - 1).min(TOTAL_STYLES - 1);
                    }
                    return Some(DialogAction::None);
                }
                Key::Named(Named::Space) | Key::Named(Named::Enter) => {
                    self.temp_outline_style = self.outline_cursor;
                    return Some(DialogAction::None);
                }
                _ => return None,
            }
        }

        // Only handle keyboard events for charset category
        if self.current_category != SettingsCategory::Charset {
            return None;
        }

        match key {
            // F1-F12 to select slots
            Key::Named(Named::F1) => {
                self.selected_charset_slot = Some(0);
                let set_idx = self.temp_fkeys.current_set();
                self.charset_cursor = self.temp_fkeys.code_at(set_idx, 0) as u8;
                Some(DialogAction::None)
            }
            Key::Named(Named::F2) => {
                self.selected_charset_slot = Some(1);
                let set_idx = self.temp_fkeys.current_set();
                self.charset_cursor = self.temp_fkeys.code_at(set_idx, 1) as u8;
                Some(DialogAction::None)
            }
            Key::Named(Named::F3) => {
                self.selected_charset_slot = Some(2);
                let set_idx = self.temp_fkeys.current_set();
                self.charset_cursor = self.temp_fkeys.code_at(set_idx, 2) as u8;
                Some(DialogAction::None)
            }
            Key::Named(Named::F4) => {
                self.selected_charset_slot = Some(3);
                let set_idx = self.temp_fkeys.current_set();
                self.charset_cursor = self.temp_fkeys.code_at(set_idx, 3) as u8;
                Some(DialogAction::None)
            }
            Key::Named(Named::F5) => {
                self.selected_charset_slot = Some(4);
                let set_idx = self.temp_fkeys.current_set();
                self.charset_cursor = self.temp_fkeys.code_at(set_idx, 4) as u8;
                Some(DialogAction::None)
            }
            Key::Named(Named::F6) => {
                self.selected_charset_slot = Some(5);
                let set_idx = self.temp_fkeys.current_set();
                self.charset_cursor = self.temp_fkeys.code_at(set_idx, 5) as u8;
                Some(DialogAction::None)
            }
            Key::Named(Named::F7) => {
                self.selected_charset_slot = Some(6);
                let set_idx = self.temp_fkeys.current_set();
                self.charset_cursor = self.temp_fkeys.code_at(set_idx, 6) as u8;
                Some(DialogAction::None)
            }
            Key::Named(Named::F8) => {
                self.selected_charset_slot = Some(7);
                let set_idx = self.temp_fkeys.current_set();
                self.charset_cursor = self.temp_fkeys.code_at(set_idx, 7) as u8;
                Some(DialogAction::None)
            }
            Key::Named(Named::F9) => {
                self.selected_charset_slot = Some(8);
                let set_idx = self.temp_fkeys.current_set();
                self.charset_cursor = self.temp_fkeys.code_at(set_idx, 8) as u8;
                Some(DialogAction::None)
            }
            Key::Named(Named::F10) => {
                self.selected_charset_slot = Some(9);
                let set_idx = self.temp_fkeys.current_set();
                self.charset_cursor = self.temp_fkeys.code_at(set_idx, 9) as u8;
                Some(DialogAction::None)
            }
            Key::Named(Named::F11) => {
                self.selected_charset_slot = Some(10);
                let set_idx = self.temp_fkeys.current_set();
                self.charset_cursor = self.temp_fkeys.code_at(set_idx, 10) as u8;
                Some(DialogAction::None)
            }
            Key::Named(Named::F12) => {
                self.selected_charset_slot = Some(11);
                let set_idx = self.temp_fkeys.current_set();
                self.charset_cursor = self.temp_fkeys.code_at(set_idx, 11) as u8;
                Some(DialogAction::None)
            }
            // Arrow keys for cursor navigation (only when slot is selected)
            Key::Named(Named::ArrowLeft) if self.selected_charset_slot.is_some() => {
                let col = (self.charset_cursor % 32) as i32;
                let row = (self.charset_cursor / 32) as i32;
                let new_col = (col - 1).rem_euclid(32);
                self.charset_cursor = (row * 32 + new_col) as u8;
                Some(DialogAction::None)
            }
            Key::Named(Named::ArrowRight) if self.selected_charset_slot.is_some() => {
                let col = (self.charset_cursor % 32) as i32;
                let row = (self.charset_cursor / 32) as i32;
                let new_col = (col + 1).rem_euclid(32);
                self.charset_cursor = (row * 32 + new_col) as u8;
                Some(DialogAction::None)
            }
            Key::Named(Named::ArrowUp) if self.selected_charset_slot.is_some() => {
                let col = (self.charset_cursor % 32) as i32;
                let row = (self.charset_cursor / 32) as i32;
                let new_row = (row - 1).rem_euclid(8);
                self.charset_cursor = (new_row * 32 + col) as u8;
                Some(DialogAction::None)
            }
            Key::Named(Named::ArrowDown) if self.selected_charset_slot.is_some() => {
                let col = (self.charset_cursor % 32) as i32;
                let row = (self.charset_cursor / 32) as i32;
                let new_row = (row + 1).rem_euclid(8);
                self.charset_cursor = (new_row * 32 + col) as u8;
                Some(DialogAction::None)
            }
            // Home/End for row navigation
            Key::Named(Named::Home) if self.selected_charset_slot.is_some() => {
                if modifiers.control() {
                    // Ctrl+Home: first character
                    self.charset_cursor = 0;
                } else {
                    // Home: first char in row
                    let row = self.charset_cursor / 32;
                    self.charset_cursor = row * 32;
                }
                Some(DialogAction::None)
            }
            Key::Named(Named::End) if self.selected_charset_slot.is_some() => {
                if modifiers.control() {
                    // Ctrl+End: last character
                    self.charset_cursor = 255;
                } else {
                    // End: last char in row
                    let row = self.charset_cursor / 32;
                    self.charset_cursor = row * 32 + 31;
                }
                Some(DialogAction::None)
            }
            // Space/Enter to confirm selection
            Key::Named(Named::Space) | Key::Named(Named::Enter) if self.selected_charset_slot.is_some() => {
                if let Some(slot) = self.selected_charset_slot {
                    let set_idx = self.temp_fkeys.current_set();
                    self.temp_fkeys.set_code_at(set_idx, slot, self.charset_cursor as u16);
                }
                Some(DialogAction::None)
            }
            _ => None,
        }
    }
}
