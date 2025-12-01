use std::path::PathBuf;

use iced::{
    Element, Length,
    widget::{Space, button, container, row, text, text_input},
};

use super::options::ViewMode;
use crate::items::ProviderType;

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
    /// Filter text changed
    FilterChanged(String),
    /// Clear the filter
    ClearFilter,
    /// Refresh current directory
    Refresh,
    /// Toggle between list and tile view
    ToggleViewMode,
    /// Path input changed
    PathChanged(String),
    /// Path input submitted (Enter pressed)
    PathSubmitted,
    /// Open settings dialog
    OpenSettings,
}

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
    pub filter: String,
    pub view_mode: ViewMode,
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
            filter: String::new(),
            view_mode: ViewMode::default(),
            is_16colors_mode: false,
            path_input: String::new(),
            is_path_valid: true,
        }
    }

    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
    }

    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
    }

    pub fn toggle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::List => ViewMode::Tiles,
            ViewMode::Tiles => ViewMode::List,
        };
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
        // Back button
        let back_btn = button(text("◀").size(14)).padding([4, 8]).style(nav_button_style);
        let back_btn = if can_go_back {
            back_btn.on_press(NavigationBarMessage::Back)
        } else {
            back_btn
        };

        // Forward button
        let forward_btn = button(text("▶").size(14)).padding([4, 8]).style(nav_button_style);
        let forward_btn = if can_go_forward {
            forward_btn.on_press(NavigationBarMessage::Forward)
        } else {
            forward_btn
        };

        // Refresh button
        let refresh_btn = button(text("⟲").size(14))
            .padding([4, 8])
            .style(nav_button_style)
            .on_press(NavigationBarMessage::Refresh);

        // Path input (editable)
        let is_valid = self.is_path_valid;
        let path_input = text_input("Enter path or URL...", &self.path_input)
            .on_input(NavigationBarMessage::PathChanged)
            .on_submit(NavigationBarMessage::PathSubmitted)
            .padding([6, 10])
            .size(13)
            .width(Length::Fill)
            .style(move |theme, status| path_input_style(theme, status, is_valid));

        // Filter input
        let filter_input = text_input("🔍 Filter...", &self.filter)
            .on_input(NavigationBarMessage::FilterChanged)
            .padding([6, 10])
            .size(13)
            .width(Length::Fixed(150.0));

        // Clear filter button (only show if filter is not empty)
        let clear_btn = if !self.filter.is_empty() {
            button(text("✕").size(12))
                .padding([4, 6])
                .style(nav_button_style)
                .on_press(NavigationBarMessage::ClearFilter)
        } else {
            button(text("✕").size(12)).padding([4, 6]).style(nav_button_style)
        };

        // View mode toggle button (list/tiles)
        let view_icon = match self.view_mode {
            ViewMode::List => "☷",  // List icon
            ViewMode::Tiles => "⊞", // Grid icon
        };
        let view_btn = button(text(view_icon).size(14))
            .padding([4, 8])
            .style(nav_button_style)
            .on_press(NavigationBarMessage::ToggleViewMode);

        // 16colors.rs toggle button - use different style when active
        let web_btn = button(text("🌐").size(14))
            .padding([4, 8])
            .style(if self.is_16colors_mode { nav_button_active_style } else { nav_button_style })
            .on_press(NavigationBarMessage::Toggle16Colors);

        // Settings button
        let settings_btn = button(text("⚙").size(14))
            .padding([4, 8])
            .style(nav_button_style)
            .on_press(NavigationBarMessage::OpenSettings);

        let content = row![
            back_btn,
            forward_btn,
            refresh_btn,
            web_btn,
            Space::new().width(8),
            path_input,
            Space::new().width(8),
            view_btn,
            Space::new().width(8),
            filter_input,
            clear_btn,
            Space::new().width(8),
            settings_btn,
        ]
        .spacing(2)
        .padding(6)
        .align_y(iced::Alignment::Center);

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
        button::Status::Active => (palette.background.strong.color, palette.background.strong.text),
        button::Status::Hovered => (palette.primary.weak.color, palette.primary.weak.text),
        button::Status::Pressed => (palette.primary.base.color, palette.primary.base.text),
        button::Status::Disabled => (palette.background.weak.color, palette.background.weak.text.scale_alpha(0.5)),
    };
    button::Style {
        background: Some(iced::Background::Color(bg)),
        text_color,
        border: iced::Border {
            color: palette.background.strong.color,
            width: 1.0,
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
