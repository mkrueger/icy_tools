use crate::Size;

pub mod paint;

pub mod patterns;
pub use patterns::*;

mod sound;
pub mod vdi;

pub const IGS_VERSION: &str = "2.19";

#[repr(u8)]
#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub enum TerminalResolution {
    /// 320x200
    #[default]
    Low,
    /// 640x200
    Medium,
    /// 640x400  
    High,
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
}
