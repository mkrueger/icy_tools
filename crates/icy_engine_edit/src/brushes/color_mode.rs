//! Color mode for brush operations

/// The color mode determines which color attributes are affected
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorMode {
    /// Don't change any colors
    None,
    /// Only change foreground color
    Foreground,
    /// Only change background color
    Background,
    /// Change both foreground and background
    #[default]
    Both,
}

impl ColorMode {
    /// Returns true if foreground color should be applied
    pub fn affects_foreground(&self) -> bool {
        matches!(self, ColorMode::Foreground | ColorMode::Both)
    }

    /// Returns true if background color should be applied
    pub fn affects_background(&self) -> bool {
        matches!(self, ColorMode::Background | ColorMode::Both)
    }

    /// Returns true if any color should be applied
    pub fn affects_any(&self) -> bool {
        !matches!(self, ColorMode::None)
    }
}
