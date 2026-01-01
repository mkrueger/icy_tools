//! SAUCE metadata UI helpers
//!
//! Provides consistent styling for SAUCE metadata fields (title, author, group)
//! across all icy_tools applications.

use icy_ui::{widget::text_input, Color, Theme};

/// SAUCE field color category for dialog styling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SauceFieldColor {
    /// Title field - warm yellow/gold color
    Title,
    /// Author field - green color
    Author,
    /// Group field - blue color
    Group,
    /// Normal field - uses default theme text color
    Normal,
}

/// Get the SAUCE field color for a given field type and theme.
///
/// Returns different colors for light and dark themes to ensure readability:
/// - Title: Yellow/gold tones
/// - Author: Green tones
/// - Group: Blue tones
/// - Normal: Theme's default text color
pub fn get_sauce_color(field: SauceFieldColor, theme: &Theme) -> Color {
    let is_dark = theme.is_dark;
    match field {
        SauceFieldColor::Title => {
            if is_dark {
                Color::from_rgb(0.9, 0.9, 0.6)
            } else {
                Color::from_rgb(0.6, 0.5, 0.0)
            }
        }
        SauceFieldColor::Author => {
            if is_dark {
                Color::from_rgb(0.6, 0.9, 0.6)
            } else {
                Color::from_rgb(0.0, 0.5, 0.0)
            }
        }
        SauceFieldColor::Group => {
            if is_dark {
                Color::from_rgb(0.6, 0.8, 0.9)
            } else {
                Color::from_rgb(0.0, 0.4, 0.6)
            }
        }
        SauceFieldColor::Normal => theme.background.on,
    }
}

/// Create a text input style function for SAUCE fields with appropriate coloring.
///
/// This returns a closure that can be used with `.style()` on text_input widgets.
///
/// # Example
/// ```ignore
/// text_input("", &self.title)
///     .style(sauce_input_style(SauceFieldColor::Title))
/// ```
pub fn sauce_input_style(field: SauceFieldColor) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |theme: &Theme, _status: text_input::Status| {
        let value_color = get_sauce_color(field, theme);
        text_input::Style {
            background: icy_ui::Background::Color(theme.primary.base),
            border: icy_ui::Border {
                color: theme.secondary.base,
                width: 1.0,
                radius: 4.0.into(),
            },
            icon: theme.secondary.on.scale_alpha(0.6),
            placeholder: theme.background.on.scale_alpha(0.5),
            value: value_color,
            selection: theme.accent.base.scale_alpha(0.5),
        }
    }
}

/// Create a text input style function for invalid/danger fields.
///
/// This returns a closure that can be used with `.style()` on text_input widgets
/// to indicate validation errors with a red border.
///
/// # Example
/// ```ignore
/// text_input("", &self.width)
///     .style(danger_input_style())
/// ```
pub fn danger_input_style() -> impl Fn(&Theme, text_input::Status) -> text_input::Style + Copy {
    |theme: &Theme, _status: text_input::Status| text_input::Style {
        background: icy_ui::Background::Color(theme.primary.base),
        border: icy_ui::Border {
            color: theme.destructive.base,
            width: 1.5,
            radius: 4.0.into(),
        },
        icon: theme.secondary.on.scale_alpha(0.6),
        placeholder: theme.background.on.scale_alpha(0.5),
        value: theme.destructive.base,
        selection: theme.accent.base.scale_alpha(0.5),
    }
}

/// Create a default text input style function.
///
/// This returns a closure that can be used with `.style()` on text_input widgets.
pub fn default_input_style() -> impl Fn(&Theme, text_input::Status) -> text_input::Style + Copy {
    |theme: &Theme, _status: text_input::Status| text_input::Style {
        background: icy_ui::Background::Color(theme.primary.base),
        border: icy_ui::Border {
            color: theme.secondary.base,
            width: 1.0,
            radius: 4.0.into(),
        },
        icon: theme.secondary.on.scale_alpha(0.6),
        placeholder: theme.background.on.scale_alpha(0.5),
        value: theme.background.on,
        selection: theme.accent.base.scale_alpha(0.5),
    }
}

/// Create a text input style function that is either default or danger based on validity.
///
/// # Example
/// ```ignore
/// text_input("", &self.width)
///     .style(validated_input_style(is_valid))
/// ```
pub fn validated_input_style(is_valid: bool) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |theme: &Theme, status: text_input::Status| {
        if is_valid {
            default_input_style()(theme, status)
        } else {
            danger_input_style()(theme, status)
        }
    }
}
