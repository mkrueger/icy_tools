use crate::Size;

pub mod paint;

pub mod patterns;
pub use patterns::*;

pub mod sound;
pub mod vdi;

mod fonts;
pub use fonts::*;

pub const IGS_VERSION: &str = "2.19";

#[repr(u8)]
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TerminalResolution {
    /// 320x200
    #[default]
    Low = 0,
    /// 640x200
    Medium = 1,
    /// 640x400  
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

    pub fn get_resolution(&self) -> Size {
        match self {
            TerminalResolution::Low => Size { width: 320, height: 200 },
            TerminalResolution::Medium => Size { width: 640, height: 200 },
            TerminalResolution::High => Size { width: 640, height: 400 },
        }
    }

    pub fn get_text_resolution(&self) -> Size {
        match self {
            TerminalResolution::Low => Size { width: 40, height: 25 },
            TerminalResolution::Medium => Size { width: 80, height: 25 },
            TerminalResolution::High => Size { width: 80, height: 50 },
        }
    }

    pub fn get_max_colors(&self) -> u32 {
        match self {
            TerminalResolution::Low => 16,
            TerminalResolution::Medium => 4,
            TerminalResolution::High => 2,
        }
    }

    pub fn get_palette(&self) -> &crate::Palette {
        match self {
            TerminalResolution::Low => &crate::palette_handling::ATARI_ST_LOW_PALETTE,
            TerminalResolution::Medium => &crate::palette_handling::ATARI_ST_MEDIUM_PALETTE,
            TerminalResolution::High => &crate::palette_handling::ATARI_ST_HIGH_PALETTE,
        }
    }
}
