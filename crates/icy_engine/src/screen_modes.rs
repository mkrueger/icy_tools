use std::fmt::{self, Display};

use crate::{
    amiga_screen_buffer, fonts::ansi::font_height_for_lines, seq_prepare, AutoWrapMode, BitFont, BufferType, EditableScreen, GraphicsType, Palette,
    PaletteScreenBuffer, Size, TerminalResolution, TextScreen, ATARI, ATARI_DEFAULT_PALETTE, ATARI_XEP80, ATARI_XEP80_INT, ATARI_XEP80_PALETTE,
    C64_DEFAULT_PALETTE, C64_SHIFTED, C64_UNSHIFTED, CP437, SKYPIX_PALETTE, VIEWDATA, VIEWDATA_PALETTE,
};
use icy_net::telnet::TerminalEmulation;
use icy_parser_core::{CaretShape, CommandParser, MusicOption};
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ScreenMode {
    Default,
    // Cga(i32, i32),
    // Ega(i32, i32),
    Vga(i32, i32),
    Unicode(i32, i32),
    Vic,
    Atascii(i32),
    Videotex,
    Mode7,
    Rip,
    SkyPix,
    AtariST(TerminalResolution, bool),
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
            ScreenMode::Vga(w, h) => format!("Vga({w}, {h})"),
            ScreenMode::Unicode(w, h) => format!("Unicode({w}, {h})"),
            ScreenMode::Vic => "Vic".to_string(),
            ScreenMode::Atascii(i) => {
                if *i == 80 {
                    "XEP80".to_string()
                } else {
                    "Antic".to_string()
                }
            }
            ScreenMode::Videotex => "Videotex".to_string(),
            ScreenMode::Mode7 => "Mode7".to_string(),
            ScreenMode::Rip => "Rip".to_string(),
            ScreenMode::SkyPix => "SkyPix".to_string(),
            ScreenMode::AtariST(n, igs) => {
                let term_res = match n {
                    TerminalResolution::High => "High",
                    TerminalResolution::Medium => "Medium",
                    TerminalResolution::Low => "Low",
                };
                format!("AtariST({}, {})", term_res, *igs)
            }
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

        impl Visitor<'_> for ScreenModeVisitor {
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
                    let params: Vec<&str> = params_str.split(',').map(str::trim).collect();
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
                            if params.len() != 2 {
                                return Err(E::custom("AtariST expects two parameters"));
                            }
                            let term_res = match params[0] {
                                "High" => TerminalResolution::High,
                                "Low" => TerminalResolution::Low,
                                // Default to Medium if unrecognized
                                _ => TerminalResolution::Medium,
                            };
                            let has_igs = params[1] == "true";
                            Ok(ScreenMode::AtariST(term_res, has_igs))
                        }
                        _ => Err(E::unknown_variant(name, &["Vga", "Unicode", "AtariST"])),
                    }
                } else {
                    match value {
                        "Default" => Ok(ScreenMode::Default),
                        "Vic" => Ok(ScreenMode::Vic),
                        "Antic" => Ok(ScreenMode::Atascii(40)),
                        "XEP80" => Ok(ScreenMode::Atascii(80)),
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
        ScreenMode::AtariST(TerminalResolution::Low, false),
        ScreenMode::AtariST(TerminalResolution::Medium, false),
        ScreenMode::AtariST(TerminalResolution::High, false),
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
            ScreenMode::Atascii(x) => {
                if *x == 80 {
                    write!(f, "XEP80")
                } else {
                    write!(f, "ANTIC")
                }
            }
            ScreenMode::Videotex => write!(f, "VIDEOTEX"),
            ScreenMode::Default => write!(f, "Default"),
            ScreenMode::Rip => write!(f, "RIPscrip"),
            ScreenMode::SkyPix => write!(f, "SkyPix"),
            ScreenMode::AtariST(resolution, igs) => {
                let igs = if *igs { "enabled " } else { "disabled" };
                match resolution {
                    TerminalResolution::Low => write!(f, "Atari ST low, igs {igs}"),
                    TerminalResolution::Medium => write!(f, "Atari ST medium, igs {igs}"),
                    TerminalResolution::High => write!(f, "Atari ST high, igs {igs}"),
                }
            }
            ScreenMode::Mode7 => write!(f, "Mode7"),
        }
    }
}

pub const ATASCII_SCREEN_SIZE: Size = Size { width: 40, height: 24 };
pub const ATASCII_PAL_SCREEN_SIZE: Size = Size { width: 40, height: 25 };
pub const ATASCII_XEP80_SCREEN_SIZE: Size = Size { width: 80, height: 25 };

/// Options for creating a screen and parser combination.
pub struct CreationOptions {
    /// Music option for ANSI parsers
    pub ansi_music: MusicOption,
}

impl ScreenMode {
    pub fn window_size(&self) -> Size {
        match self {
            // ScreenMode::Cga(w, h) | ScreenMode::Ega(w, h) |
            ScreenMode::Vga(w, h) | ScreenMode::Unicode(w, h) => Size::new(*w, *h),
            ScreenMode::Vic | ScreenMode::Mode7 => Size::new(40, 25),
            ScreenMode::AtariST(res, _igs) => {
                let (w, h) = res.text_resolution();
                Size::new(w, h)
            }
            ScreenMode::Atascii(i) => {
                if *i == 80 {
                    ATASCII_XEP80_SCREEN_SIZE
                } else {
                    ATASCII_SCREEN_SIZE
                }
            }
            ScreenMode::Videotex => Size::new(40, 24),
            ScreenMode::Default | ScreenMode::SkyPix => Size::new(80, 25),
            ScreenMode::Rip => Size::new(80, 44),
        }
    }

    fn apply_to_edit_screen(&self, screen: &mut dyn EditableScreen) {
        screen.terminal_state_mut().auto_wrap_mode = AutoWrapMode::AutoWrap;
        // Ensure we have at least one layer and set its size
        match self {
            ScreenMode::Vga(_x, y) => {
                screen.clear_font_table();
                screen.set_font(0, BitFont::from_ansi_font_page(0, font_height_for_lines(*y as usize)).unwrap().clone());
                *screen.buffer_type_mut() = BufferType::CP437;
            }
            ScreenMode::Unicode(_x, _y) => {
                *screen.buffer_type_mut() = BufferType::Unicode;
            }
            ScreenMode::Default => {
                screen.clear_font_table();
                screen.set_font(0, BitFont::from_bytes("", CP437).unwrap());
                *screen.buffer_type_mut() = BufferType::CP437;
            }
            ScreenMode::Vic => {
                screen.clear_font_table();
                screen.set_font(0, C64_UNSHIFTED.clone());
                screen.set_font(1, C64_SHIFTED.clone());
                screen.set_font_dimensions(Size::new(8, 8)); // C64 uses 8x8 fonts
                *screen.palette_mut() = Palette::from_slice(&C64_DEFAULT_PALETTE);
                *screen.buffer_type_mut() = BufferType::Petscii;

                seq_prepare(screen);
            }
            ScreenMode::Atascii(i) => {
                screen.clear_font_table();
                if *i == 40 {
                    screen.set_font(0, ATARI.clone());
                    screen.set_font_dimensions(Size::new(8, 8)); // Atari uses 8x8 fonts
                    *screen.palette_mut() = Palette::from_slice(&ATARI_DEFAULT_PALETTE);
                } else {
                    screen.set_font(0, ATARI_XEP80.clone());
                    screen.set_font(1, ATARI_XEP80_INT.clone());
                    screen.set_font_dimensions(Size::new(8, 8)); // XEP80 also uses 8x8 fonts
                    *screen.palette_mut() = Palette::from_slice(&ATARI_XEP80_PALETTE);
                }
                *screen.buffer_type_mut() = BufferType::Atascii;
            }
            ScreenMode::Videotex | ScreenMode::Mode7 => {
                screen.clear_font_table();
                screen.set_font(0, VIEWDATA.clone());
                screen.set_font_dimensions(Size::new(6, 10)); // Viewdata SAA5050 uses 6x10 fonts
                *screen.palette_mut() = Palette::from_slice(&VIEWDATA_PALETTE);
                *screen.buffer_type_mut() = BufferType::Viewdata;
            }
            ScreenMode::Rip => {
                // Done by creation
            }
            ScreenMode::SkyPix => {
                *screen.palette_mut() = Palette::from_slice(&SKYPIX_PALETTE);
                screen.caret_mut().shape = CaretShape::Underline;
            }
            ScreenMode::AtariST(_x, _igs) => {
                *screen.buffer_type_mut() = BufferType::Atascii;
                screen.set_font_dimensions(Size::new(8, 8));
            }
        }
        screen.caret_default_colors();
    }

    /// Creates a screen and parser combination for the given terminal emulation.
    ///
    /// # Arguments
    /// * `emulation` - The terminal emulation type
    /// * `option` - Optional creation options (e.g., ANSI music settings)
    ///
    /// # Returns
    /// A tuple of (`EditableScreen`, `CommandParser`) properly configured for the emulation
    ///
    /// # Example
    /// ```no_run
    /// use icy_engine::{ScreenMode, CreationOptions, MusicOption};
    /// use icy_net::telnet::TerminalEmulation;
    ///
    /// let mode = ScreenMode::Vga(80, 25);
    /// let options = Some(CreationOptions { ansi_music: MusicOption::Both });
    /// let (screen, parser) = mode.create_screen(TerminalEmulation::Ansi, options);
    /// ```
    pub fn create_screen(&self, emulation: TerminalEmulation, option: Option<CreationOptions>) -> (Box<dyn EditableScreen>, Box<dyn CommandParser + Send>) {
        let mut screen: Box<dyn EditableScreen> = match emulation {
            TerminalEmulation::Rip => {
                let buf = PaletteScreenBuffer::new(GraphicsType::Rip);
                Box::new(buf)
            }
            TerminalEmulation::Skypix => {
                let buf = amiga_screen_buffer::AmigaScreenBuffer::new(GraphicsType::Skypix);
                Box::new(buf)
            }
            TerminalEmulation::AtariST => {
                let (res, _igs) = if let ScreenMode::AtariST(res, igs) = self {
                    (*res, *igs)
                } else {
                    (TerminalResolution::Low, false)
                };
                let buf = PaletteScreenBuffer::new(GraphicsType::IGS(res));
                Box::new(buf)
            }
            _ => Box::new(TextScreen::new(self.window_size())),
        };

        self.apply_to_edit_screen(screen.as_mut());

        // Override buffer_type for UTF8Ansi emulation - needs Unicode rendering
        if emulation == TerminalEmulation::Utf8Ansi {
            *screen.buffer_type_mut() = BufferType::Unicode;
        }

        let music_option = option.as_ref().map(|o: &CreationOptions| o.ansi_music);
        let parser = get_parser(&emulation, music_option, self);
        (screen, parser)
    }
}

/// Creates a parser for the given terminal emulation and screen mode.
///
/// # Arguments
/// * `emulator` - The terminal emulation type
/// * `use_ansi_music` - Optional music option for ANSI parsers
/// * `screen_mode` - The screen mode (used for `AtariST` IGS detection)
///
/// # Returns
/// A boxed command parser configured for the emulation type
#[must_use]
pub fn get_parser(emulator: &TerminalEmulation, use_ansi_music: Option<MusicOption>, screen_mode: &ScreenMode) -> Box<dyn CommandParser + Send> {
    match emulator {
        TerminalEmulation::Ansi | TerminalEmulation::Utf8Ansi => {
            let mut parser = icy_parser_core::AnsiParser::new();
            if let Some(music_opt) = use_ansi_music {
                parser.music_option = music_opt;
            }
            Box::new(parser)
        }
        TerminalEmulation::Avatar => Box::new(icy_parser_core::AvatarParser::new()),
        TerminalEmulation::Ascii => Box::new(icy_parser_core::AsciiParser::new()),
        TerminalEmulation::PETscii => Box::new(icy_parser_core::PetsciiParser::new()),
        TerminalEmulation::ATAscii => Box::new(icy_parser_core::AtasciiParser::new()),
        TerminalEmulation::ViewData => Box::new(icy_parser_core::ViewdataParser::new()),
        TerminalEmulation::Mode7 => Box::new(icy_parser_core::Mode7Parser::new()),
        TerminalEmulation::Rip => Box::new(icy_parser_core::RipParser::new()),
        TerminalEmulation::Skypix => Box::new(icy_parser_core::SkypixParser::new()),
        TerminalEmulation::AtariST => {
            if let ScreenMode::AtariST(_, igs) = screen_mode {
                return if *igs {
                    let mut p = icy_parser_core::IgsParser::new();
                    p.run_loop = true;
                    Box::new(p)
                } else {
                    Box::new(icy_parser_core::Vt52Parser::new(icy_parser_core::VT52Mode::Mixed))
                };
            }
            log::warn!("ScreenMode is wrong for AtariST {screen_mode:?}, fall back to IGS.");
            Box::new(icy_parser_core::IgsParser::new())
        }
    }
}
