use std::fmt::{self, Display};

use icy_engine::{
    ATARI, ATARI_DEFAULT_PALETTE, BitFont, C64_DEFAULT_PALETTE, C64_SHIFTED, C64_UNSHIFTED, CP437, EditableScreen, IBM_VGA50_SAUCE, Palette, SKYPIX_PALETTE,
    Size, VIEWDATA, VIEWDATA_PALETTE,
};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, Visitor},
};

//use super::{BufferInputMode, BufferView};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
            ScreenMode::Vga(w, h) => {
                // Treat any VGA size not in the predefined set as custom.
                let predefined = [(80, 25), (80, 50), (132, 37), (132, 52)];
                !predefined.contains(&(w, h))
            }
            _ => false,
        }
    }
}

impl Serialize for ScreenMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match self {
            ScreenMode::Default => "Default".to_string(),
            ScreenMode::Vga(w, h) => format!("Vga({}, {})", w, h),
            ScreenMode::Unicode(w, h) => format!("Unicode({}, {})", w, h),
            ScreenMode::Vic => "Vic".to_string(),
            ScreenMode::Antic => "Antic".to_string(),
            ScreenMode::Videotex => "Videotex".to_string(),
            ScreenMode::Mode7 => "Mode7".to_string(),
            ScreenMode::Rip => "Rip".to_string(),
            ScreenMode::SkyPix => "SkyPix".to_string(),
            ScreenMode::AtariST(n) => format!("AtariST({})", n),
        };
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for ScreenMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ScreenModeVisitor;

        impl<'de> Visitor<'de> for ScreenModeVisitor {
            type Value = ScreenMode;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a ScreenMode string like 'Vga(80, 25)' or 'Default'")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if let Some(open_paren) = value.find('(') {
                    if !value.ends_with(')') {
                        return Err(E::custom("expected closing parenthesis"));
                    }
                    let name = &value[..open_paren];
                    let params_str = &value[open_paren + 1..value.len() - 1];
                    let params: Vec<&str> = params_str.split(',').map(|p| p.trim()).collect();
                    match name {
                        "Vga" => {
                            if params.len() != 2 {
                                return Err(E::custom("Vga expects two integer parameters"));
                            }
                            let w = params[0].parse::<i32>().map_err(E::custom)?;
                            let h = params[1].parse::<i32>().map_err(E::custom)?;
                            Ok(ScreenMode::Vga(w, h))
                        }
                        "Unicode" => {
                            if params.len() != 2 {
                                return Err(E::custom("Unicode expects two integer parameters"));
                            }
                            let w = params[0].parse::<i32>().map_err(E::custom)?;
                            let h = params[1].parse::<i32>().map_err(E::custom)?;
                            Ok(ScreenMode::Unicode(w, h))
                        }
                        "AtariST" => {
                            if params.len() != 1 {
                                return Err(E::custom("AtariST expects one integer parameter"));
                            }
                            let n = params[0].parse::<i32>().map_err(E::custom)?;
                            Ok(ScreenMode::AtariST(n))
                        }
                        _ => Err(E::unknown_variant(name, &["Vga", "Unicode", "AtariST"])),
                    }
                } else {
                    match value {
                        "Default" => Ok(ScreenMode::Default),
                        "Vic" => Ok(ScreenMode::Vic),
                        "Antic" => Ok(ScreenMode::Antic),
                        "Videotex" => Ok(ScreenMode::Videotex),
                        "Mode7" => Ok(ScreenMode::Mode7),
                        "Rip" => Ok(ScreenMode::Rip),
                        "SkyPix" => Ok(ScreenMode::SkyPix),
                        _ => Err(E::unknown_variant(value, &["Default", "Vic", "Antic", "Videotex", "Mode7", "Rip", "SkyPix"])),
                    }
                }
            }
        }

        deserializer.deserialize_str(ScreenModeVisitor)
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

    pub fn apply_to_edit_screen(&self, screen: &mut dyn EditableScreen) {
        // Ensure we have at least one layer and set its size
        match self {
            ScreenMode::Vga(_x, y) => {
                screen.clear_font_table();
                screen.set_font(0, BitFont::from_bytes("", if *y >= 50 { IBM_VGA50_SAUCE } else { CP437 }).unwrap());
                *screen.buffer_type_mut() = icy_engine::BufferType::CP437;
            }
            ScreenMode::Unicode(_x, _y) => {
                screen.clear_font_table();
                let mut font = BitFont::default();
                font.size = Size::new(32, 64);
                screen.set_font(0, font);
                *screen.buffer_type_mut() = icy_engine::BufferType::Unicode;
            }
            ScreenMode::Default => {
                screen.clear_font_table();
                screen.set_font(0, BitFont::from_bytes("", CP437).unwrap());
                *screen.buffer_type_mut() = icy_engine::BufferType::CP437;
            }
            ScreenMode::Vic => {
                screen.clear_font_table();
                screen.set_font(0, BitFont::from_bytes("", C64_UNSHIFTED).unwrap());
                screen.set_font(1, BitFont::from_bytes("", C64_SHIFTED).unwrap());
                *screen.palette_mut() = Palette::from_slice(&C64_DEFAULT_PALETTE);
                *screen.buffer_type_mut() = icy_engine::BufferType::Petscii;
            }
            ScreenMode::Antic => {
                screen.clear_font_table();
                screen.set_font(0, BitFont::from_bytes("", ATARI).unwrap());
                *screen.palette_mut() = Palette::from_slice(&ATARI_DEFAULT_PALETTE);
                *screen.buffer_type_mut() = icy_engine::BufferType::Atascii;
            }
            ScreenMode::Videotex | ScreenMode::Mode7 => {
                screen.clear_font_table();
                screen.set_font(0, BitFont::from_bytes("", VIEWDATA).unwrap());
                *screen.palette_mut() = Palette::from_slice(&VIEWDATA_PALETTE);
                *screen.buffer_type_mut() = icy_engine::BufferType::Viewdata;
            }
            ScreenMode::Rip => {
                // Done by creation
            }
            ScreenMode::SkyPix => {
                *screen.palette_mut() = Palette::from_slice(&SKYPIX_PALETTE);
            }
            ScreenMode::AtariST(_x) => {
                *screen.palette_mut() = Palette::from_slice(&C64_DEFAULT_PALETTE);
                *screen.buffer_type_mut() = icy_engine::BufferType::Atascii;
            }
        }
    }
}
