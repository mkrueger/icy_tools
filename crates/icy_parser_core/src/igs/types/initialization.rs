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

impl Default for InitializationType {
    fn default() -> Self {
        Self::DesktopPaletteAndAttributes
    }
}

impl TryFrom<i32> for InitializationType {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::DesktopPaletteAndAttributes),
            1 => Ok(Self::DesktopPaletteOnly),
            2 => Ok(Self::DesktopAttributesOnly),
            3 => Ok(Self::IgDefaultPalette),
            4 => Ok(Self::VdiDefaultPalette),
            5 => Ok(Self::DesktopResolutionAndClipping),
            _ => Err(format!("Invalid InitializationType value: {}", value)),
        }
    }
}
