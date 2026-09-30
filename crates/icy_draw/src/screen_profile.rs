//! What a document's screen can hold, so editing only offers what its format can store.
//!
//! ANSI screens color every cell and may switch fonts per character. Other home computers
//! work differently: an Atari's ATASCII screen has one font and two colors for the whole
//! screen, and inverse video is the upper half of the character codes.

use icy_engine::{BufferType, FileFormat, Size, TerminalResolution, TextBuffer, TextPane, ATASCII_SCREEN_SIZE, ATASCII_XEP80_SCREEN_SIZE};

/// The Atari text screens: the built-in 40 columns or the XEP80's 80 columns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AtasciiMode {
    #[default]
    Antic,
    Xep80,
}

impl AtasciiMode {
    pub const ALL: [Self; 2] = [Self::Antic, Self::Xep80];

    pub fn screen_size(self) -> Size {
        match self {
            Self::Antic => ATASCII_SCREEN_SIZE,
            Self::Xep80 => ATASCII_XEP80_SCREEN_SIZE,
        }
    }

    pub fn columns(self) -> i32 {
        self.screen_size().width
    }

    /// The mode of a screen `columns` wide; like the Atari, anything wider than 40 is the XEP80.
    pub fn for_columns(columns: i32) -> Self {
        if columns > ATASCII_SCREEN_SIZE.width {
            Self::Xep80
        } else {
            Self::Antic
        }
    }

    /// The file extension ATASCII of this mode is exported with.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Antic => "ata",
            Self::Xep80 => "xep",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ScreenProfile {
    /// DOS and Unicode text: colors and fonts per character.
    #[default]
    Ansi,
    Atascii(AtasciiMode),
    /// The Atari ST's VT52 text screen in one of its resolutions.
    AtariSt(TerminalResolution),
    /// Screens without their own editing support yet (PETSCII, Viewdata), edited like ANSI.
    Other(BufferType),
}

impl ScreenProfile {
    pub fn of(buffer: &TextBuffer) -> Self {
        match buffer.buffer_type {
            BufferType::CP437 | BufferType::Unicode => Self::Ansi,
            BufferType::Atascii => Self::Atascii(AtasciiMode::for_columns(buffer.width())),
            BufferType::AtariSt => Self::AtariSt(icy_engine::atari_st_resolution(buffer)),
            other => Self::Other(other),
        }
    }

    /// Whether each character has colors of its own; ATASCII colors the whole screen.
    pub fn per_character_colors(self) -> bool {
        !matches!(self, Self::Atascii(_))
    }

    /// Whether inverse video is part of the character code (bit 7) rather than its colors.
    pub fn inverse_in_character(self) -> bool {
        matches!(self, Self::Atascii(_))
    }

    /// How many fonts a document may use at once.
    pub fn font_slots(self) -> Option<usize> {
        match self {
            Self::Atascii(_) | Self::AtariSt(_) => Some(1),
            _ => None,
        }
    }

    /// The width the screen is fixed to, if any.
    pub fn fixed_width(self) -> Option<i32> {
        match self {
            Self::Atascii(mode) => Some(mode.columns()),
            Self::AtariSt(resolution) => Some(icy_engine::atari_st_columns(resolution)),
            _ => None,
        }
    }

    /// The file format documents of this screen export to by default, with its extension.
    pub fn native_format(self) -> Option<(FileFormat, &'static str)> {
        match self {
            Self::Atascii(mode) => Some((FileFormat::Atascii, mode.extension())),
            Self::AtariSt(_) => Some((FileFormat::Vt52, "vt52")),
            _ => None,
        }
    }
}

/// An empty ATASCII document screen: the mode's width and height with the Atari font and colors.
pub fn atascii_buffer(mode: AtasciiMode) -> TextBuffer {
    let size = mode.screen_size();
    icy_engine::atascii_buffer(size.width, size.height)
}

/// An empty VT52 document: the ST's text screen in `resolution` with its font and colors.
pub fn atari_st_buffer(resolution: TerminalResolution) -> TextBuffer {
    icy_engine::atari_st_buffer(resolution, 25)
}

/// The columns an ATASCII file at `path` is loaded with: 80 for XEP80 text (.xep).
pub fn atascii_columns(path: &std::path::Path) -> Option<usize> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    (extension == AtasciiMode::Xep80.extension()).then_some(AtasciiMode::Xep80.columns() as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atascii_documents_have_the_screen_of_their_mode() {
        for mode in AtasciiMode::ALL {
            let buffer = atascii_buffer(mode);
            assert_eq!(buffer.size(), mode.screen_size());
            assert_eq!(buffer.buffer_type, BufferType::Atascii);
            let profile = ScreenProfile::of(&buffer);
            assert_eq!(profile, ScreenProfile::Atascii(mode));
            assert!(!profile.per_character_colors() && profile.inverse_in_character());
            assert_eq!(profile.font_slots(), Some(1));
            assert_eq!(profile.fixed_width(), Some(mode.columns()));
        }
        assert_eq!(ScreenProfile::of(&TextBuffer::new((80, 25))), ScreenProfile::Ansi);
    }

    #[test]
    fn vt52_documents_have_the_screen_of_their_resolution() {
        for (resolution, columns) in [(TerminalResolution::Low, 40), (TerminalResolution::Medium, 80), (TerminalResolution::High, 80)] {
            let buffer = atari_st_buffer(resolution);
            let profile = ScreenProfile::of(&buffer);
            assert_eq!(profile, ScreenProfile::AtariSt(resolution));
            assert!(profile.per_character_colors() && !profile.inverse_in_character());
            assert_eq!(profile.fixed_width(), Some(columns));
            assert_eq!(profile.native_format(), Some((FileFormat::Vt52, "vt52")));
        }
    }

    #[test]
    fn xep_files_load_with_80_columns() {
        assert_eq!(atascii_columns(std::path::Path::new("art.XEP")), Some(80));
        assert_eq!(atascii_columns(std::path::Path::new("art.ata")), None);
    }
}
