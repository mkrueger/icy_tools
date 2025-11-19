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

/// Palette mode for SetResolution command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteMode {
    /// No palette change
    NoChange = 0,
    /// Desktop colors
    Desktop = 1,
    /// IG default palette
    IgDefault = 2,
    /// VDI default palette
    VdiDefault = 3,
}

impl Default for PaletteMode {
    fn default() -> Self {
        Self::NoChange
    }
}

impl TryFrom<i32> for PaletteMode {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::NoChange),
            1 => Ok(Self::Desktop),
            2 => Ok(Self::IgDefault),
            3 => Ok(Self::VdiDefault),
            _ => Err(format!("Invalid PaletteMode value: {}", value)),
        }
    }
}

/// Terminal resolution for SetResolution command
#[repr(u8)]
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TerminalResolution {
    /// 320x200, 16 colors
    #[default]
    Low = 0,
    /// 640x200, 4 colors
    Medium = 1,
    /// 640x400, 2 colors
    High = 2,
}

impl TerminalResolution {
    pub fn resolution_id(&self) -> String {
        match self {
            TerminalResolution::Low => "0".to_string(),
            TerminalResolution::Medium => "1".to_string(),
            TerminalResolution::High => "2".to_string(),
        }
    }

    pub fn get_max_colors(&self) -> u32 {
        match self {
            TerminalResolution::Low => 16,
            TerminalResolution::Medium => 4,
            TerminalResolution::High => 2,
        }
    }

    pub fn default_fg_color(&self) -> u8 {
        match self {
            TerminalResolution::Low => 15,
            TerminalResolution::Medium => 3,
            TerminalResolution::High => 1,
        }
    }

    /// Returns text resolution in characters (width, height)
    pub fn get_text_resolution(&self) -> (i32, i32) {
        match self {
            TerminalResolution::Low => (40, 25),
            TerminalResolution::Medium => (80, 25),
            TerminalResolution::High => (80, 50),
        }
    }

    /// Returns true if scan lines should be used (only for Medium resolution)
    pub fn use_scanlines(&self) -> bool {
        matches!(self, TerminalResolution::Medium)
    }
}

impl TryFrom<i32> for TerminalResolution {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Low),
            1 => Ok(Self::Medium),
            2 => Ok(Self::High),
            _ => Err(format!("Invalid TerminalResolution value: {}", value)),
        }
    }
}
