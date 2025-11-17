/// Initialization type for Initialize command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationType {
    /// Set desktop palette and attributes
    DesktopPaletteAndAttributes = 0,
    /// Set desktop palette only
    DesktopPaletteOnly = 1,
    /// Set desktop attributes only
    DesktopAttributesOnly = 2,
    /// Set IG default palette
    IgDefaultPalette = 3,
    /// Set VDI default palette
    VdiDefaultPalette = 4,
    /// Set desktop resolution and VDI clipping (should be used FIRST)
    DesktopResolutionAndClipping = 5,
}

impl From<i32> for InitializationType {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::DesktopPaletteAndAttributes,
            1 => Self::DesktopPaletteOnly,
            2 => Self::DesktopAttributesOnly,
            3 => Self::IgDefaultPalette,
            4 => Self::VdiDefaultPalette,
            5 => Self::DesktopResolutionAndClipping,
            _ => Self::DesktopPaletteAndAttributes,
        }
    }
}
