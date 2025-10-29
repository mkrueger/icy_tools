use std::fmt::Display;

use icy_engine::{
    ATARI, ATARI_DEFAULT_PALETTE, BitFont, C64_DEFAULT_PALETTE, C64_LOWER, CP437, Color, IBM_VGA50_SAUCE, Palette, SKYPIX_PALETTE, Size, VIEWDATA,
    VIEWDATA_PALETTE, editor::EditState,
};
use serde::{Deserialize, Serialize};

//use super::{BufferInputMode, BufferView};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenMode {
    Default,
    // Cga(i32, i32),
    // Ega(i32, i32),
    Vga(i32, i32),
    Unicode(i32, i32),
    Vic,
    Antic,
    Videotex,
    Mode7,
    Rip,
    SkyPix,
    AtariST(i32),
}

impl Default for ScreenMode {
    fn default() -> Self {
        ScreenMode::Vga(80, 25)
    }
}

impl ScreenMode {
    pub fn is_custom_vga(self) -> bool {
        match self {
            ScreenMode::Vga(w, h) => w == 40 && h == 25,
            _ => false,
        }
    }
}
lazy_static::lazy_static! {
    pub static ref VGA_MODES: Vec<ScreenMode> = vec![
        ScreenMode::Vga(80, 25),
        ScreenMode::Vga(80, 50),
        ScreenMode::Vga(132, 37),
        ScreenMode::Vga(132, 52),
        ScreenMode::Vga(40, 25), // Custom VGA
    ];

    pub static ref ATARI_MODES: Vec<ScreenMode> = vec![
        ScreenMode::AtariST(80),
        ScreenMode::AtariST(40),
    ];
}
impl Display for ScreenMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScreenMode::Vga(w, h) | ScreenMode::Unicode(w, h) => {
                if self.is_custom_vga() {
                    write!(f, "Custom VGA")
                } else {
                    write!(f, "VGA {w}x{h}")
                }
            }
            // ScreenMode::Ega(w, h) => write!(f, "EGA {w}x{h}"),
            // ScreenMode::Cga(w, h) => write!(f, "CGA {w}x{h}"),
            ScreenMode::Vic => write!(f, "VIC-II"),
            ScreenMode::Antic => write!(f, "ANTIC"),
            ScreenMode::Videotex => write!(f, "VIDEOTEX"),
            ScreenMode::Default => write!(f, "Default"),
            ScreenMode::Rip => write!(f, "RIPscrip"),
            ScreenMode::SkyPix => write!(f, "SkyPix"),
            ScreenMode::AtariST(x) => {
                if *x == 80 {
                    write!(f, "Atari ST 80 cols")
                } else {
                    write!(f, "Atari ST 40 cols")
                }
            }
            ScreenMode::Mode7 => write!(f, "Mode7"),
        }
    }
}

impl ScreenMode {
    pub fn get_window_size(&self) -> Size {
        match self {
            // ScreenMode::Cga(w, h) | ScreenMode::Ega(w, h) |
            ScreenMode::Vga(w, h) | ScreenMode::Unicode(w, h) => Size::new(*w, *h),
            ScreenMode::Vic | ScreenMode::Mode7 => Size::new(40, 25),
            ScreenMode::AtariST(cols) => Size::new(*cols, 25),
            ScreenMode::Antic | ScreenMode::Videotex => Size::new(40, 24),
            ScreenMode::Default => Size::new(80, 25),
            ScreenMode::Rip => Size::new(80, 44),
            ScreenMode::SkyPix => Size::new(80, 25),
        }
    }

    pub fn apply_to_edit_state(&self, edit_state: &mut EditState) {
        let _ = edit_state.clear_layer(0);
        let buffer = edit_state.get_buffer_mut();
        let window_size = self.get_window_size();

        // Set buffer sizes
        buffer.set_default_size(window_size);
        buffer.set_size(window_size);
        buffer.terminal_state.set_size(window_size);
        buffer.terminal_state.fixed_size = true;
        buffer.is_terminal_buffer = true;

        // Ensure we have at least one layer and set its size
        if buffer.layers.is_empty() {
            buffer.layers.push(Default::default());
        }
        buffer.layers[0].set_size(window_size);

        match self {
            ScreenMode::Vga(_x, y) => {
                buffer.clear_font_table();
                buffer.set_font(0, BitFont::from_bytes("", if *y >= 50 { IBM_VGA50_SAUCE } else { CP437 }).unwrap());
                buffer.palette = Palette::dos_default();
                buffer.buffer_type = icy_engine::BufferType::CP437;
            }
            ScreenMode::Unicode(_x, y) => {
                buffer.clear_font_table();
                buffer.set_font(0, BitFont::from_bytes("", if *y >= 50 { IBM_VGA50_SAUCE } else { CP437 }).unwrap());
                buffer.palette = Palette::dos_default();
                buffer.buffer_type = icy_engine::BufferType::Unicode;
            }
            ScreenMode::Default => {
                buffer.clear_font_table();
                buffer.set_font(0, BitFont::from_bytes("", CP437).unwrap());
                buffer.palette = Palette::dos_default();
                buffer.buffer_type = icy_engine::BufferType::CP437;
            }
            ScreenMode::Vic => {
                buffer.clear_font_table();
                buffer.set_font(0, BitFont::from_bytes("", C64_LOWER).unwrap());
                buffer.set_font(1, BitFont::from_bytes("", C64_LOWER).unwrap());
                buffer.palette = Palette::from_slice(&C64_DEFAULT_PALETTE);
                buffer.buffer_type = icy_engine::BufferType::Petscii;
            }
            ScreenMode::Antic => {
                buffer.clear_font_table();
                buffer.set_font(0, BitFont::from_bytes("", ATARI).unwrap());
                buffer.palette = Palette::from_slice(&ATARI_DEFAULT_PALETTE);
                buffer.buffer_type = icy_engine::BufferType::Atascii;
            }
            ScreenMode::Videotex | ScreenMode::Mode7 => {
                buffer.clear_font_table();
                buffer.set_font(0, BitFont::from_bytes("", VIEWDATA).unwrap());
                buffer.palette = Palette::from_slice(&VIEWDATA_PALETTE);
                buffer.buffer_type = icy_engine::BufferType::Viewdata;
            }
            ScreenMode::Rip => {
                buffer.clear_font_table();
                buffer.set_font(0, BitFont::from_sauce_name("IBM VGA50").unwrap());
                buffer.palette = Palette::dos_default();
                buffer.is_terminal_buffer = true;
                buffer.buffer_type = icy_engine::BufferType::CP437;
            }
            ScreenMode::SkyPix => {
                buffer.clear_font_table();
                buffer.set_font(0, BitFont::from_sauce_name("IBM VGA50").unwrap());
                buffer.palette = Palette::from_slice(&SKYPIX_PALETTE);
                buffer.is_terminal_buffer = true;
                buffer.terminal_state.fixed_size = true;
                buffer.buffer_type = icy_engine::BufferType::CP437;
            }
            ScreenMode::AtariST(_x) => {
                buffer.clear_font_table();
                buffer.set_font(0, BitFont::from_bytes("", ATARI).unwrap());
                buffer.palette = Palette::from_slice(&C64_DEFAULT_PALETTE);
                buffer.buffer_type = icy_engine::BufferType::Atascii;
            }
        }

        // Clear the first layer and stop any sixel threads
        buffer.layers[0].clear();
        buffer.stop_sixel_threads();
    }

    #[allow(clippy::match_same_arms)]
    pub(crate) fn get_selection_fg(&self) -> Color {
        match self {
            ScreenMode::Default | ScreenMode::Vga(_, _) | ScreenMode::Unicode(_, _) | ScreenMode::Rip | ScreenMode::SkyPix => Color::new(0xAA, 0x00, 0xAA),
            ScreenMode::Vic => Color::new(0x37, 0x39, 0xC4),
            ScreenMode::Antic => Color::new(0x09, 0x51, 0x83),
            ScreenMode::Videotex | ScreenMode::Mode7 => Color::new(0, 0, 0),
            ScreenMode::AtariST(_) => Color::new(0, 0, 0),
        }
    }

    #[allow(clippy::match_same_arms)]
    pub(crate) fn get_selection_bg(&self) -> Color {
        match self {
            ScreenMode::Default | ScreenMode::Vga(_, _) | ScreenMode::Unicode(_, _) | ScreenMode::Rip | ScreenMode::SkyPix => Color::new(0xAA, 0xAA, 0xAA),
            ScreenMode::Vic => Color::new(0xB0, 0x3F, 0xB6),
            ScreenMode::Antic => Color::new(0xFF, 0xFF, 0xFF),
            ScreenMode::Videotex | ScreenMode::Mode7 => Color::new(0xFF, 0xFF, 0xFF),
            ScreenMode::AtariST(_) => Color::new(0xFF, 0xFF, 0xFF),
        }
    }
}
