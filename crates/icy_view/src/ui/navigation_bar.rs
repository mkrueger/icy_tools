use std::path::PathBuf;

use i18n_embed_fl::fl;
use iced::{
    Element, Event, Length,
    widget::{Space, button, container, row, text, text_input, tooltip},
};
use icy_engine_gui::command_handler;

use super::icons::{arrow_back_icon, arrow_forward_icon, language_icon, refresh_icon, search_icon, settings_icon};
use super::options::ViewMode;
use crate::LANGUAGE_LOADER;
use crate::commands::{cmd, create_icy_view_commands};
use crate::items::ProviderType;
use crate::{LATEST_VERSION, VERSION};

/// A point in navigation history
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryPoint {
    /// Provider type (File or Web)
    pub provider: ProviderType,
    /// View mode (List or Tiles)
    pub view_mode: ViewMode,
    /// Path within the provider
    pub path: String,
    /// Selected item name (if any)
    pub selected_item: Option<String>,
}

impl HistoryPoint {
    pub fn new(provider: ProviderType, view_mode: ViewMode, path: String, selected_item: Option<String>) -> Self {
        Self {
            provider,
            view_mode,
            path,
            selected_item,
        }
    }
}

/// Messages for the navigation bar
#[derive(Debug, Clone)]
pub enum NavigationBarMessage {
    /// Go back in history
    Back,
    /// Go forward in history
    Forward,
    /// Go to parent directory
    Up,
    /// Toggle 16colors.rs browsing mode
    Toggle16Colors,
    /// Open filter popup
    OpenFilter,
    /// Refresh current directory
    Refresh,
    /// Path input changed
    PathChanged(String),
    /// Path input submitted (Enter pressed)
    PathSubmitted,
    /// Open settings dialog
    OpenSettings,
    /// Open the releases page for update
    OpenReleasesPage,
}

// Command handler for NavigationBar
command_handler!(NavigationCommands, create_icy_view_commands(), => NavigationBarMessage {
    cmd::NAV_BACK => NavigationBarMessage::Back,
    cmd::NAV_FORWARD => NavigationBarMessage::Forward,
    cmd::NAV_UP => NavigationBarMessage::Up,
});

/// Navigation history
pub struct NavigationHistory {
    back_stack: Vec<HistoryPoint>,
    forward_stack: Vec<HistoryPoint>,
    current: Option<HistoryPoint>,
}

impl NavigationHistory {
    pub fn new() -> Self {
        Self {
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            current: None,
        }
    }

    /// Initialize with a starting point
    pub fn init(&mut self, point: HistoryPoint) {
        self.current = Some(point);
        self.back_stack.clear();
        self.forward_stack.clear();
    }

    /// Navigate to a new history point
    pub fn navigate_to(&mut self, point: HistoryPoint) {
        if let Some(current) = self.current.take() {
            // Don't add duplicate entries
            if current != point {
                self.back_stack.push(current);
            }
        }
        self.current = Some(point);
        self.forward_stack.clear();
    }

    /// Update the current point's selected item without creating a new history entry
    pub fn update_selection(&mut self, selected_item: Option<String>) {
        if let Some(ref mut current) = self.current {
            current.selected_item = selected_item;
        }
    }

    /// Go back in history
    pub fn go_back(&mut self) -> Option<HistoryPoint> {
        if let Some(prev) = self.back_stack.pop() {
            if let Some(current) = self.current.take() {
                self.forward_stack.push(current);
            }
            self.current = Some(prev.clone());
            Some(prev)
        } else {
            None
        }
    }

    /// Go forward in history
    pub fn go_forward(&mut self) -> Option<HistoryPoint> {
        if let Some(next) = self.forward_stack.pop() {
            if let Some(current) = self.current.take() {
                self.back_stack.push(current);
            }
            self.current = Some(next.clone());
            Some(next)
        } else {
            None
        }
    }

    pub fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }

    /// Get the current history point
    pub fn current_point(&self) -> Option<&HistoryPoint> {
        self.current.as_ref()
    }
}

impl Default for NavigationHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Navigation bar widget
pub struct NavigationBar {
    /// Command handler for navigation shortcuts
    commands: NavigationCommands,
    /// Whether we're currently browsing 16colors.rs
    pub is_16colors_mode: bool,
    /// Editable path input
    pub path_input: String,
    /// Whether the current path input is valid
    pub is_path_valid: bool,
}

impl NavigationBar {
    pub fn new() -> Self {
        Self {
            commands: NavigationCommands::new(),
            is_16colors_mode: false,
            path_input: String::new(),
            is_path_valid: true,
        }
    }

    /// Handle an event and return the corresponding message if it matches a command
    pub fn handle_event(&self, event: &Event) -> Option<NavigationBarMessage> {
        self.commands.handle(event)
    }

    pub fn set_16colors_mode(&mut self, enabled: bool) {
        self.is_16colors_mode = enabled;
    }

    /// Update the path input to match the current path
    pub fn set_path_input(&mut self, path: String) {
        self.path_input = path;
        self.is_path_valid = true; // Valid when set programmatically
    }

    /// Set whether the current path is valid
    pub fn set_path_valid(&mut self, valid: bool) {
        self.is_path_valid = valid;
    }

    pub fn view<'a>(&'a self, _current_path: Option<&PathBuf>, can_go_back: bool, can_go_forward: bool) -> Element<'a, NavigationBarMessage> {
        let icon_size = 16.0;

        // Back button
        let back_btn = button(arrow_back_icon(icon_size)).padding([4, 6]).style(nav_button_style);
        let back_btn = if can_go_back {
            back_btn.on_press(NavigationBarMessage::Back)
        } else {
            back_btn
        };
        let back_btn = tooltip(
            back_btn,
            container(text(fl!(LANGUAGE_LOADER, "tooltip-back")).size(12)).style(container::rounded_box),
            tooltip::Position::Bottom,
        );

        // Forward button
        let forward_btn = button(arrow_forward_icon(icon_size)).padding([4, 6]).style(nav_button_style);
        let forward_btn = if can_go_forward {
            forward_btn.on_press(NavigationBarMessage::Forward)
        } else {
            forward_btn
        };
        let forward_btn = tooltip(
            forward_btn,
            container(text(fl!(LANGUAGE_LOADER, "tooltip-forward")).size(12)).style(container::rounded_box),
            tooltip::Position::Bottom,
        );

        // Refresh button
        let refresh_btn = button(refresh_icon(icon_size))
            .padding([4, 6])
            .style(nav_button_style)
            .on_press(NavigationBarMessage::Refresh);
        let refresh_btn = tooltip(
            refresh_btn,
            container(text(fl!(LANGUAGE_LOADER, "tooltip-refresh")).size(12)).style(container::rounded_box),
            tooltip::Position::Bottom,
        );

        // Path input (editable)
        let is_valid = self.is_path_valid;
        let path_input = text_input("Enter path…", &self.path_input)
            .on_input(NavigationBarMessage::PathChanged)
            .on_submit(NavigationBarMessage::PathSubmitted)
            .padding([6, 10])
            .size(13)
            .width(Length::Fill)
            .style(move |theme, status| path_input_style(theme, status, is_valid));

        // Filter button (opens filter popup)
        let filter_btn = button(search_icon(icon_size))
            .padding([4, 6])
            .style(nav_button_style)
            .on_press(NavigationBarMessage::OpenFilter);
        let filter_btn = tooltip(
            filter_btn,
            container(text(fl!(LANGUAGE_LOADER, "tooltip-filter")).size(12)).style(container::rounded_box),
            tooltip::Position::Bottom,
        );

        // 16colors.rs toggle button - use different style when active
        let web_btn = button(language_icon(icon_size))
            .padding([4, 6])
            .style(if self.is_16colors_mode { nav_button_active_style } else { nav_button_style })
            .on_press(NavigationBarMessage::Toggle16Colors);
        let web_btn = tooltip(
            web_btn,
            container(text(fl!(LANGUAGE_LOADER, "tooltip-browse-16colors")).size(12)).style(container::rounded_box),
            tooltip::Position::Bottom,
        );

        // Settings button
        let settings_btn = button(settings_icon(icon_size))
            .padding([4, 6])
            .style(nav_button_style)
            .on_press(NavigationBarMessage::OpenSettings);
        let settings_btn = tooltip(
            settings_btn,
            container(text(fl!(LANGUAGE_LOADER, "tooltip-settings")).size(12)).style(container::rounded_box),
            tooltip::Position::Bottom,
        );

        // Update available link (only shown if newer version exists)
        let update_available = *VERSION < *LATEST_VERSION;

        let mut content = row![
            back_btn,
            forward_btn,
            refresh_btn,
            web_btn,
            Space::new().width(8),
            path_input,
            Space::new().width(8),
        ]
        .spacing(2)
        .padding(6)
        .align_y(iced::Alignment::Center);

        // Add update link if available
        if update_available {
            let update_text = fl!(LANGUAGE_LOADER, "update-available", version = LATEST_VERSION.to_string());
            let update_btn = button(text(update_text).size(12))
                .padding([4, 8])
                .style(update_link_style)
                .on_press(NavigationBarMessage::OpenReleasesPage);
            content = content.push(update_btn);
            content = content.push(Space::new().width(8));
        }

        content = content.push(filter_btn);
        content = content.push(settings_btn);

        container(content)
            .width(Length::Fill)
            .style(|theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(iced::Background::Color(palette.background.weak.color)),
                    border: iced::Border {
                        color: palette.background.strong.color,
                        width: 0.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                }
            })
            .into()
    }
}

impl Default for NavigationBar {
    fn default() -> Self {
        Self::new()
    }
}

fn nav_button_style(theme: &iced::Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let (bg, text_color) = match status {
        button::Status::Active => (iced::Color::TRANSPARENT, palette.background.strong.text),
        button::Status::Hovered => (palette.primary.weak.color, palette.primary.weak.text),
        button::Status::Pressed => (palette.primary.base.color, palette.primary.base.text),
        button::Status::Disabled => (iced::Color::TRANSPARENT, palette.background.weak.text.scale_alpha(0.3)),
    };
    button::Style {
        background: Some(iced::Background::Color(bg)),
        text_color,
        border: iced::Border {
            color: iced::Color::TRANSPARENT,
            width: 0.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

/// Style for active toggle buttons (like 16colors when enabled)
fn nav_button_active_style(theme: &iced::Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let (bg, text_color) = match status {
        button::Status::Active => (palette.primary.base.color, palette.primary.base.text),
        button::Status::Hovered => (palette.primary.strong.color, palette.primary.strong.text),
        button::Status::Pressed => (palette.primary.weak.color, palette.primary.weak.text),
        button::Status::Disabled => (palette.background.weak.color, palette.background.weak.text.scale_alpha(0.5)),
    };
    button::Style {
        background: Some(iced::Background::Color(bg)),
        text_color,
        border: iced::Border {
            color: palette.primary.strong.color,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

/// Style for the path input field
fn path_input_style(theme: &iced::Theme, status: text_input::Status, is_valid: bool) -> text_input::Style {
    let palette = theme.extended_palette();
    let danger_color = palette.danger.base.color;

    let (bg, border_color) = match status {
        text_input::Status::Active => {
            let border = if is_valid { palette.background.strong.color } else { danger_color };
            (palette.background.base.color, border)
        }
        text_input::Status::Hovered => {
            let border = if is_valid { palette.background.strong.color } else { danger_color };
            (palette.background.weak.color, border)
        }
        text_input::Status::Focused { .. } => {
            let border = if is_valid { palette.primary.base.color } else { danger_color };
            (palette.background.base.color, border)
        }
        text_input::Status::Disabled => (palette.background.weak.color, palette.background.strong.color),
    };
    text_input::Style {
        background: iced::Background::Color(bg),
        border: iced::Border {
            color: border_color,
            width: if is_valid { 1.0 } else { 2.0 },
            radius: 4.0.into(),
        },
        icon: palette.background.strong.text.scale_alpha(0.6),
        placeholder: palette.background.base.text.scale_alpha(0.5),
        value: palette.background.base.text,
        selection: palette.primary.weak.color.scale_alpha(0.5),
    }
}

/// Style for the update available link button
fn update_link_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    // Use the same info blue color as icy_term: Color::from_rgb(0.2, 0.6, 1.0)
    let info_color = iced::Color::from_rgb(0.2, 0.6, 1.0);

    let (bg, text_color) = match status {
        button::Status::Active => (iced::Color::TRANSPARENT, info_color),
        button::Status::Hovered => (iced::Color::from_rgba(info_color.r, info_color.g, info_color.b, 0.1), info_color),
        button::Status::Pressed => (iced::Color::from_rgba(info_color.r, info_color.g, info_color.b, 0.15), info_color),
        button::Status::Disabled => (iced::Color::TRANSPARENT, info_color.scale_alpha(0.3)),
    };
    button::Style {
        background: Some(iced::Background::Color(bg)),
        text_color,
        border: iced::Border {
            color: iced::Color::TRANSPARENT,
            width: 0.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}
