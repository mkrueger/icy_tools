use iced::{Color, Theme};

pub struct IcyTheme {
    pub margin: f32,
    pub padding: f32,
    pub spacing: f32,
    pub button_padding: [f32; 2],

    pub title_font_size: f32,
    pub label_font_size: f32,
    pub button_font_size: f32,
}

impl IcyTheme {
    pub fn default() -> Self {
        Self {
            margin: 20.0,
            spacing: 8.0,
            padding: 6.0,
            button_padding: [12.0, 6.0],
            button_font_size: 14.0,
            label_font_size: 14.0,
            title_font_size: 20.0,
        }
    }
}

/// Get the main area background color for content areas (preview, editor canvas, etc.)
/// This provides visual separation from sidebars and toolbars
pub fn main_area_background(theme: &Theme) -> Color {
    theme.primary.base
}
