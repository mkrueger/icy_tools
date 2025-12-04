use std::time::Instant;

use i18n_embed_fl::fl;
use iced::{
    Element, Length,
    widget::{button, container, row, text, tooltip},
};

use super::icons;
use super::options::{SortOrder, ViewMode};
use crate::LANGUAGE_LOADER;

/// Duration before toolbar auto-hides on first show (generous)
const INITIAL_HIDE_DELAY_SECS: f32 = 5.0;
/// Duration before toolbar auto-hides after subsequent interactions
const NORMAL_HIDE_DELAY_SECS: f32 = 1.5;
/// Width of the hover zone to show toolbar (in pixels)
pub const TOOLBAR_HOVER_ZONE_WIDTH: f32 = 40.0;
/// Toolbar width for calculating slide animation
pub const TOOLBAR_WIDTH: f32 = 80.0;

/// Messages for the file list toolbar
#[derive(Debug, Clone)]
pub enum FileListToolbarMessage {
    /// Go to parent directory
    Up,
    /// Toggle between list and tile view
    ToggleViewMode,
    /// Cycle sort order
    CycleSortOrder,
    /// Toggle SAUCE info mode
    ToggleSauceMode,
    /// Start shuffle mode
    StartShuffleMode,
    /// Mouse entered toolbar hover zone
    MouseEntered,
    /// Mouse left toolbar hover zone
    MouseLeft,
    /// Hide timer tick - check if toolbar should hide
    HideTick,
}

/// Toolbar for the file list area
/// Contains: Up button, View mode toggle, Sort button
pub struct FileListToolbar {
    pub view_mode: ViewMode,
    pub sort_order: SortOrder,
    pub sauce_mode: bool,
    /// Whether the up button is enabled (not at root)
    pub can_go_up: bool,
    /// Whether the toolbar is currently visible (for tiles mode slide animation)
    pub is_visible: bool,
    /// Whether mouse is hovering over toolbar or hover zone
    pub is_hovered: bool,
    /// When the toolbar was last shown (for auto-hide timing)
    last_shown: Option<Instant>,
    /// Whether this is the first time showing (use longer delay)
    first_show: bool,
}

impl FileListToolbar {
    pub fn new() -> Self {
        Self {
            view_mode: ViewMode::default(),
            sort_order: SortOrder::default(),
            sauce_mode: false,
            can_go_up: false,
            is_visible: true,
            is_hovered: false,
            last_shown: None,
            first_show: true,
        }
    }

    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
    }

    pub fn set_sort_order(&mut self, order: SortOrder) {
        self.sort_order = order;
    }

    pub fn set_sauce_mode(&mut self, sauce_mode: bool) {
        self.sauce_mode = sauce_mode;
    }

    pub fn set_can_go_up(&mut self, can_go_up: bool) {
        self.can_go_up = can_go_up;
    }

    /// Called when mouse enters the toolbar or hover zone
    pub fn on_mouse_enter(&mut self) {
        self.is_hovered = true;
        self.show();
    }

    /// Called when mouse leaves the toolbar area
    pub fn on_mouse_leave(&mut self) {
        self.is_hovered = false;
        // Start hide timer
        self.last_shown = Some(Instant::now());
    }

    /// Show the toolbar
    pub fn show(&mut self) {
        if !self.is_visible {
            self.is_visible = true;
            self.last_shown = Some(Instant::now());
        }
    }

    /// Check if toolbar should auto-hide (called on tick)
    /// Returns true if visibility changed
    pub fn check_auto_hide(&mut self) -> bool {
        if !self.is_visible || self.is_hovered {
            return false;
        }

        if let Some(shown_at) = self.last_shown {
            let delay = if self.first_show { INITIAL_HIDE_DELAY_SECS } else { NORMAL_HIDE_DELAY_SECS };

            if shown_at.elapsed().as_secs_f32() >= delay {
                self.is_visible = false;
                self.first_show = false;
                return true;
            }
        }
        false
    }

    /// Reset toolbar state when switching to tiles mode
    pub fn reset_for_tiles_mode(&mut self) {
        self.is_visible = true;
        self.is_hovered = false;
        self.last_shown = Some(Instant::now());
        self.first_show = true;
    }

    /// Get the current X offset for slide animation (0 = fully visible, -TOOLBAR_WIDTH = hidden)
    pub fn get_slide_offset(&self) -> f32 {
        if self.is_visible { 0.0 } else { -TOOLBAR_WIDTH }
    }

    /// View for list mode (solid background, fixed width 300px)
    pub fn view_for_list(&self) -> Element<'_, FileListToolbarMessage> {
        self.view_internal(false)
    }

    /// View for tiles mode (transparent overlay)
    pub fn view(&self) -> Element<'_, FileListToolbarMessage> {
        self.view_internal(true)
    }

    fn view_internal(&self, is_overlay: bool) -> Element<'_, FileListToolbarMessage> {
        // Up button
        let up_btn = button(icons::arrow_upward_icon(14.0)).padding([2, 6]).style(toolbar_button_style);
        let up_btn = if self.can_go_up {
            up_btn.on_press(FileListToolbarMessage::Up)
        } else {
            up_btn
        };
        let up_btn = tooltip(
            up_btn,
            container(text(fl!(LANGUAGE_LOADER, "tooltip-up")).size(12)).style(container::rounded_box),
            tooltip::Position::Bottom,
        );

        // Sort order button
        let sort_icon = self.sort_order.icon();
        let sort_tooltip = match self.sort_order {
            SortOrder::NameAsc => fl!(LANGUAGE_LOADER, "tooltip-sort-name-asc"),
            SortOrder::NameDesc => fl!(LANGUAGE_LOADER, "tooltip-sort-name-desc"),
            SortOrder::SizeAsc => fl!(LANGUAGE_LOADER, "tooltip-sort-size-asc"),
            SortOrder::SizeDesc => fl!(LANGUAGE_LOADER, "tooltip-sort-size-desc"),
            SortOrder::DateAsc => fl!(LANGUAGE_LOADER, "tooltip-sort-date-asc"),
            SortOrder::DateDesc => fl!(LANGUAGE_LOADER, "tooltip-sort-date-desc"),
        };
        let sort_btn = button(text(sort_icon).size(12))
            .padding([2, 6])
            .style(toolbar_button_style)
            .on_press(FileListToolbarMessage::CycleSortOrder);
        let sort_btn = tooltip(
            sort_btn,
            container(text(sort_tooltip).size(12)).style(container::rounded_box),
            tooltip::Position::Bottom,
        );

        // SAUCE mode toggle button
        let sauce_tooltip = if self.sauce_mode {
            fl!(LANGUAGE_LOADER, "tooltip-sauce-mode-off")
        } else {
            fl!(LANGUAGE_LOADER, "tooltip-sauce-mode-on")
        };
        let sauce_btn = button(text("S").size(12))
            .padding([2, 6])
            .style(if self.sauce_mode { sauce_button_active_style } else { toolbar_button_style })
            .on_press(FileListToolbarMessage::ToggleSauceMode);
        let sauce_btn = tooltip(
            sauce_btn,
            container(text(sauce_tooltip).size(12)).style(container::rounded_box),
            tooltip::Position::Bottom,
        );

        // View mode toggle button
        let (view_icon, view_tooltip) = match self.view_mode {
            ViewMode::List => (icons::grid_view_icon(14.0), fl!(LANGUAGE_LOADER, "tooltip-view-mode-tiles")), // Currently list -> switch to tiles
            ViewMode::Tiles => (icons::view_list_icon(14.0), fl!(LANGUAGE_LOADER, "tooltip-view-mode-list")), // Currently tiles -> switch to list
        };
        let view_btn = button(view_icon)
            .padding([2, 6])
            .style(toolbar_button_style)
            .on_press(FileListToolbarMessage::ToggleViewMode);
        let view_btn = tooltip(
            view_btn,
            container(text(view_tooltip).size(12)).style(container::rounded_box),
            tooltip::Position::Bottom,
        );

        // Shuffle mode button
        let shuffle_btn = button(icons::shuffle_icon(14.0))
            .padding([2, 6])
            .style(toolbar_button_style)
            .on_press(FileListToolbarMessage::StartShuffleMode);
        let shuffle_btn = tooltip(
            shuffle_btn,
            container(text(fl!(LANGUAGE_LOADER, "tooltip-shuffle-mode")).size(12)).style(container::rounded_box),
            tooltip::Position::Bottom,
        );

        // Build row: Up, Sort, ViewMode, Shuffle, then SAUCE (only in list mode)
        let content = if is_overlay {
            // Tiles mode: hide SAUCE button (has no effect in tile view)
            row![up_btn, sort_btn, view_btn, shuffle_btn,].spacing(2).padding([2, 4])
        } else {
            // List mode: show all buttons including SAUCE
            row![up_btn, sort_btn, view_btn, shuffle_btn, sauce_btn,].spacing(2).padding([2, 4])
        };

        if is_overlay {
            // Tiles mode: same background as list mode, no border, square corners
            container(content)
                .width(Length::Shrink)
                .style(|theme| {
                    let palette = theme.extended_palette();
                    container::Style {
                        background: Some(iced::Background::Color(palette.background.weak.color).scale_alpha(0.9)),
                        border: iced::Border {
                            color: iced::Color::TRANSPARENT,
                            width: 0.0,
                            radius: 0.0.into(),
                        },
                        ..Default::default()
                    }
                })
                .into()
        } else {
            // List mode: solid background, full width
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
}

impl Default for FileListToolbar {
    fn default() -> Self {
        Self::new()
    }
}

fn toolbar_button_style(theme: &iced::Theme, status: iced::widget::button::Status) -> iced::widget::button::Style {
    let palette = theme.extended_palette();
    let (bg, text_color) = match status {
        iced::widget::button::Status::Active => (iced::Color::TRANSPARENT, palette.background.strong.text),
        iced::widget::button::Status::Hovered => (palette.primary.weak.color, palette.primary.weak.text),
        iced::widget::button::Status::Pressed => (palette.primary.base.color, palette.primary.base.text),
        iced::widget::button::Status::Disabled => (iced::Color::TRANSPARENT, palette.background.weak.text.scale_alpha(0.3)),
    };
    iced::widget::button::Style {
        background: Some(iced::Background::Color(bg)),
        text_color,
        border: iced::Border {
            color: iced::Color::TRANSPARENT,
            width: 0.0,
            radius: 3.0.into(),
        },
        ..Default::default()
    }
}

fn sauce_button_active_style(theme: &iced::Theme, status: iced::widget::button::Status) -> iced::widget::button::Style {
    let palette = theme.extended_palette();
    let (bg, text_color) = match status {
        iced::widget::button::Status::Active => (palette.primary.base.color, palette.primary.base.text),
        iced::widget::button::Status::Hovered => (palette.primary.strong.color, palette.primary.strong.text),
        iced::widget::button::Status::Pressed => (palette.primary.weak.color, palette.primary.weak.text),
        iced::widget::button::Status::Disabled => (iced::Color::TRANSPARENT, palette.background.weak.text.scale_alpha(0.3)),
    };
    iced::widget::button::Style {
        background: Some(iced::Background::Color(bg)),
        text_color,
        border: iced::Border {
            color: iced::Color::TRANSPARENT,
            width: 0.0,
            radius: 3.0.into(),
        },
        ..Default::default()
    }
}
