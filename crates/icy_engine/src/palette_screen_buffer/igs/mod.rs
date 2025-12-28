use crate::Size;

mod igs_runner;
pub mod vdi_paint;
pub use vdi_paint::*;

pub mod util;

mod fonts;
pub use fonts::*;

// Re-export TerminalResolution from icy_parser_core
pub use icy_parser_core::TerminalResolution;

pub const IGS_VERSION: &str = "2.19";

/// Extension trait for `TerminalResolution` with icy_engine-specific functionality
pub trait TerminalResolutionExt {
    fn resolution(&self) -> Size;
    fn text_resolution(&self) -> Size;
    fn palette(&self) -> &crate::Palette;
}

impl TerminalResolutionExt for TerminalResolution {
    fn resolution(&self) -> Size {
        match self {
            TerminalResolution::Low => Size { width: 320, height: 200 },
            TerminalResolution::Medium => Size { width: 640, height: 200 },
            TerminalResolution::High => Size { width: 640, height: 400 },
        }
    }

    fn text_resolution(&self) -> Size {
        match self {
            TerminalResolution::Low => Size { width: 40, height: 25 },
            TerminalResolution::Medium => Size { width: 80, height: 25 },
            TerminalResolution::High => Size { width: 80, height: 50 },
        }
    }

    fn palette(&self) -> &crate::Palette {
        match self {
            TerminalResolution::Low => &crate::palette_handling::ATARI_ST_LOW_PALETTE,
            TerminalResolution::Medium => &crate::palette_handling::ATARI_ST_MEDIUM_PALETTE,
            TerminalResolution::High => &crate::palette_handling::ATARI_ST_HIGH_PALETTE,
        }
    }
}
