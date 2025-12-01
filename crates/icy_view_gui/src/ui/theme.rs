use iced::{Color, Theme};

/// Get the main area background color for preview/thumbnail areas
/// This provides visual separation from the file list
pub fn main_area_background(theme: &Theme) -> Color {
    theme.extended_palette().background.weaker.color
}
