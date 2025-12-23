//! File format registry for unified file handling.
//!
//! This module provides a central registry for all supported file formats,
//! enabling consistent file type detection, parser selection, and save/load operations.
//!
use std::{
    io::{Read, Seek},
    path::Path,
};

use icy_net::telnet::TerminalEmulation;
use icy_parser_core::{CommandParser, MusicOption};
use unarc_rs::unified::{ArchiveFormat, UnifiedArchive};

use crate::{BufferType, EngineError, Result, ScreenMode, TextBuffer, TextPane};

use super::{BitFontFormat, CharacterFontFormat, ImageFormat, LoadData, LoadedDocument, PaletteFormat, SaveOptions, io};

/// Map file extension to archive format (replacement for private ArchiveFormat::from_extension)
fn archive_format_from_extension(ext: &str) -> Option<ArchiveFormat> {
    match ext {
        "ace" => Some(ArchiveFormat::Ace),
        "arc" => Some(ArchiveFormat::Arc),
        "arj" => Some(ArchiveFormat::Arj),
        "zoo" => Some(ArchiveFormat::Zoo),
        "sq" | "sq2" | "qqq" => Some(ArchiveFormat::Sq),
        "sqz" => Some(ArchiveFormat::Sqz),
        "z" => Some(ArchiveFormat::Z),
        "gz" => Some(ArchiveFormat::Gz),
        "bz2" => Some(ArchiveFormat::Bz2),
        "ice" => Some(ArchiveFormat::Ice),
        "hyp" => Some(ArchiveFormat::Hyp),
        "ha" => Some(ArchiveFormat::Ha),
        "lha" | "lzh" => Some(ArchiveFormat::Lha),
        "zip" => Some(ArchiveFormat::Zip),
        "rar" => Some(ArchiveFormat::Rar),
        "7z" => Some(ArchiveFormat::SevenZ),
        "tar" => Some(ArchiveFormat::Tar),
        "tgz" | "tar.gz" => Some(ArchiveFormat::Tgz),
        "tbz" | "tbz2" | "tar.bz2" => Some(ArchiveFormat::Tbz),
        "tar.z" => Some(ArchiveFormat::TarZ),
        "uc2" => Some(ArchiveFormat::Uc2),
        _ => None,
    }
}

/// Represents all supported file formats for ANSI art and related files.
///
/// Each variant corresponds to a specific file format with its own
/// characteristics for loading, saving, and display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileFormat {
    // Parser-based formats (streaming support, baud emulation)
    /// ANSI art format (.ans, .diz, .nfo, .ice)
    Ansi,
    /// ANSI music format (.ams, .mus)
    AnsiMusic,
    /// Plain ASCII text (.asc, .txt)
    Ascii,
    /// Avatar terminal format (.avt)
    Avatar,
    /// PCBoard BBS format (.pcb)
    PCBoard,
    /// Synchronet/Wildcat CtrlA format (.msg)
    CtrlA,
    /// Renegade BBS format (.an1-.an9)
    Renegade,
    /// Commodore PETSCII format (.pet, .seq)
    Petscii,
    /// Atari ATASCII format (.ata, .xep)
    Atascii,
    /// British Videotex/Prestel format (.vtx)
    ViewData,
    /// BBC Micro Mode 7 teletext (.m7)
    Mode7,
    /// RIPscrip graphics format (.rip)
    Rip,
    /// SkyPix graphics format (.spx)
    SkyPix,
    /// VT52 terminal format (.vt52, .v52, .vt5)
    Vt52,
    /// Atari ST IGS graphics format (.ig)
    Igs,

    // Binary/direct load formats
    /// IcyDraw native format (.icy)
    IcyDraw,
    /// iCE Draw format (.idf)
    IceDraw,
    /// Raw binary format (.bin)
    Bin,
    /// XBin format with embedded font/palette (.xb)
    XBin,
    /// TundraDraw 24-bit color format (.tnd)
    TundraDraw,
    /// Artworx format (.adf)
    Artworx,

    // Animation format
    /// IcyDraw animation format (.icyanim)
    IcyAnim,

    // Palette formats
    /// Palette file formats (.pal, .gpl, .hex, ...)
    Palette(PaletteFormat),

    // Font formats
    /// Bitmap font format (.yaff, .psf, .fXX)
    BitFont(BitFontFormat),

    /// Character/ASCII art font format (.flf, .tdf)
    CharacterFont(CharacterFontFormat),

    // Image formats (for recognition, not native ANSI formats)
    /// Image format (PNG, GIF, JPEG, BMP)
    Image(ImageFormat),

    // Archive formats
    /// Archive format (ZIP, ARJ, LHA, etc.)
    Archive(ArchiveFormat),
}

impl FileFormat {
    /// All known file formats (excluding image variants)
    pub const ALL: &'static [FileFormat] = &[
        FileFormat::Ansi,
        FileFormat::AnsiMusic,
        FileFormat::Ascii,
        FileFormat::Avatar,
        FileFormat::PCBoard,
        FileFormat::CtrlA,
        FileFormat::Renegade,
        FileFormat::Petscii,
        FileFormat::Atascii,
        FileFormat::ViewData,
        FileFormat::Mode7,
        FileFormat::Rip,
        FileFormat::SkyPix,
        FileFormat::Vt52,
        FileFormat::Igs,
        FileFormat::IcyDraw,
        FileFormat::IceDraw,
        FileFormat::Bin,
        FileFormat::XBin,
        FileFormat::TundraDraw,
        FileFormat::Artworx,
        FileFormat::IcyAnim,
        FileFormat::Palette(PaletteFormat::Pal),
        FileFormat::Palette(PaletteFormat::Gpl),
        FileFormat::Palette(PaletteFormat::Hex),
        FileFormat::Palette(PaletteFormat::Txt),
        FileFormat::Palette(PaletteFormat::Ice),
        FileFormat::Palette(PaletteFormat::Ase),
        FileFormat::BitFont(BitFontFormat::Yaff),
        FileFormat::BitFont(BitFontFormat::Psf),
        FileFormat::BitFont(BitFontFormat::Raw(4)),
        FileFormat::BitFont(BitFontFormat::Raw(5)),
        FileFormat::BitFont(BitFontFormat::Raw(6)),
        FileFormat::BitFont(BitFontFormat::Raw(7)),
        FileFormat::BitFont(BitFontFormat::Raw(8)),
        FileFormat::BitFont(BitFontFormat::Raw(9)),
        FileFormat::BitFont(BitFontFormat::Raw(10)),
        FileFormat::BitFont(BitFontFormat::Raw(12)),
        FileFormat::BitFont(BitFontFormat::Raw(14)),
        FileFormat::BitFont(BitFontFormat::Raw(16)),
        FileFormat::BitFont(BitFontFormat::Raw(19)),
        FileFormat::BitFont(BitFontFormat::Raw(20)),
        FileFormat::BitFont(BitFontFormat::Raw(24)),
        FileFormat::BitFont(BitFontFormat::Raw(32)),
        FileFormat::CharacterFont(CharacterFontFormat::Figlet),
        FileFormat::CharacterFont(CharacterFontFormat::Tdf),
        FileFormat::Image(ImageFormat::Png),
        FileFormat::Image(ImageFormat::Gif),
        FileFormat::Image(ImageFormat::Jpeg),
        FileFormat::Image(ImageFormat::Bmp),
        FileFormat::Image(ImageFormat::Sixel),
        FileFormat::Archive(ArchiveFormat::Zip),
        FileFormat::Archive(ArchiveFormat::Arc),
        FileFormat::Archive(ArchiveFormat::Arj),
        FileFormat::Archive(ArchiveFormat::Zoo),
        FileFormat::Archive(ArchiveFormat::Lha),
        FileFormat::Archive(ArchiveFormat::Rar),
        FileFormat::Archive(ArchiveFormat::Sqz),
        FileFormat::Archive(ArchiveFormat::Hyp),
        FileFormat::Archive(ArchiveFormat::Uc2),
    ];

    /// Formats that support saving (text-based formats only, see SAVEABLE_WITH_IMAGES for full list)
    pub const SAVEABLE: &'static [FileFormat] = &[
        FileFormat::Ansi,
        FileFormat::Ascii,
        FileFormat::Avatar,
        FileFormat::PCBoard,
        FileFormat::CtrlA,
        FileFormat::Renegade,
        FileFormat::Atascii,
        FileFormat::ViewData,
        FileFormat::IcyDraw,
        FileFormat::IceDraw,
        FileFormat::Bin,
        FileFormat::XBin,
        FileFormat::TundraDraw,
        FileFormat::Artworx,
    ];

    /// All formats that support saving, including image formats
    pub const SAVEABLE_WITH_IMAGES: &'static [FileFormat] = &[
        FileFormat::Ansi,
        FileFormat::Ascii,
        FileFormat::Avatar,
        FileFormat::PCBoard,
        FileFormat::CtrlA,
        FileFormat::Renegade,
        FileFormat::Atascii,
        FileFormat::ViewData,
        FileFormat::IcyDraw,
        FileFormat::IceDraw,
        FileFormat::Bin,
        FileFormat::XBin,
        FileFormat::TundraDraw,
        FileFormat::Artworx,
        FileFormat::Image(ImageFormat::Png),
        FileFormat::Image(ImageFormat::Gif),
    ];

    /// Detect file format from a file extension (case-insensitive).
    ///
    /// # Arguments
    /// * `ext` - File extension without the leading dot (e.g., "ans", "xb")
    ///
    /// # Returns
    /// `Some(FileFormat)` if the extension is recognized, `None` otherwise.
    ///
    /// # Example
    /// ```
    /// use icy_engine::formats::FileFormat;
    ///
    /// assert_eq!(FileFormat::from_extension("ans"), Some(FileFormat::Ansi));
    /// assert_eq!(FileFormat::from_extension("ANS"), Some(FileFormat::Ansi));
    /// assert_eq!(FileFormat::from_extension("unknown"), None);
    /// ```
    pub fn from_extension(ext: &str) -> Option<FileFormat> {
        let ext_lower = ext.to_ascii_lowercase();
        match ext_lower.as_str() {
            // ANSI variants
            "ans" | "diz" | "nfo" | "ice" => Some(FileFormat::Ansi),

            // ANSI Music
            "ams" | "mus" => Some(FileFormat::AnsiMusic),

            // Plain ASCII
            "asc" | "txt" => Some(FileFormat::Ascii),

            // Avatar
            "avt" => Some(FileFormat::Avatar),

            // PCBoard
            "pcb" => Some(FileFormat::PCBoard),

            // CtrlA (Synchronet/Wildcat)
            "msg" => Some(FileFormat::CtrlA),

            // Renegade (numbered ANSI)
            "an1" | "an2" | "an3" | "an4" | "an5" | "an6" | "an7" | "an8" | "an9" => Some(FileFormat::Renegade),

            // PETSCII
            "pet" | "seq" => Some(FileFormat::Petscii),

            // ATASCII
            "ata" | "xep" => Some(FileFormat::Atascii),

            // Videotex/Prestel
            "vtx" => Some(FileFormat::ViewData),

            // Mode 7
            "m7" => Some(FileFormat::Mode7),

            // RIPscrip
            "rip" => Some(FileFormat::Rip),

            // SkyPix
            "spx" => Some(FileFormat::SkyPix),

            // VT52
            "vt52" | "v52" | "vt5" => Some(FileFormat::Vt52),

            // IGS
            "ig" => Some(FileFormat::Igs),

            // IcyDraw native
            "icy" => Some(FileFormat::IcyDraw),

            // iCE Draw
            "idf" => Some(FileFormat::IceDraw),

            // Raw binary
            "bin" => Some(FileFormat::Bin),

            // XBin
            "xb" => Some(FileFormat::XBin),

            // TundraDraw
            "tnd" => Some(FileFormat::TundraDraw),

            // Artworx
            "adf" => Some(FileFormat::Artworx),

            // IcyDraw animation
            "icyanim" => Some(FileFormat::IcyAnim),

            // Palette formats
            // Note: `.txt` remains mapped to Ascii (ambiguous). Use `FileFormat::load_palette()`.
            "pal" => Some(FileFormat::Palette(PaletteFormat::Pal)),
            "gpl" => Some(FileFormat::Palette(PaletteFormat::Gpl)),
            "hex" => Some(FileFormat::Palette(PaletteFormat::Hex)),
            "ase" => Some(FileFormat::Palette(PaletteFormat::Ase)),
            // Avoid collision with `.ice` ANSI. Provide an explicit extension for ICE palette files.
            "icepal" => Some(FileFormat::Palette(PaletteFormat::Ice)),

            // Image formats
            "png" => Some(FileFormat::Image(ImageFormat::Png)),
            "gif" => Some(FileFormat::Image(ImageFormat::Gif)),
            "jpg" | "jpeg" => Some(FileFormat::Image(ImageFormat::Jpeg)),
            "bmp" => Some(FileFormat::Image(ImageFormat::Bmp)),
            "six" | "sixel" => Some(FileFormat::Image(ImageFormat::Sixel)),

            // Try CharacterFont formats, then BitFont formats, then archive formats
            _ => {
                if let Some(char_font_fmt) = CharacterFontFormat::from_extension(&ext_lower) {
                    Some(FileFormat::CharacterFont(char_font_fmt))
                } else if let Some(font_fmt) = BitFontFormat::from_extension(&ext_lower) {
                    Some(FileFormat::BitFont(font_fmt))
                } else {
                    archive_format_from_extension(&ext_lower).map(FileFormat::Archive)
                }
            }
        }
    }

    /// Detect file format from a file path by extracting its extension.
    ///
    /// # Arguments
    /// * `path` - Path to the file
    ///
    /// # Returns
    /// `Some(FileFormat)` if the extension is recognized, `None` otherwise.
    ///
    /// # Example
    /// ```
    /// use icy_engine::formats::FileFormat;
    /// use std::path::Path;
    ///
    /// assert_eq!(FileFormat::from_path(Path::new("art.ans")), Some(FileFormat::Ansi));
    /// assert_eq!(FileFormat::from_path(Path::new("/path/to/file.xb")), Some(FileFormat::XBin));
    /// ```
    pub fn from_path(path: &Path) -> Option<FileFormat> {
        path.extension().and_then(|ext| ext.to_str()).and_then(FileFormat::from_extension)
    }

    /// Get the primary file extension for this format (used for saving).
    ///
    /// # Returns
    /// The canonical file extension without the leading dot.
    pub fn primary_extension(&self) -> &'static str {
        match self {
            FileFormat::Ansi => "ans",
            FileFormat::AnsiMusic => "ams",
            FileFormat::Ascii => "asc",
            FileFormat::Avatar => "avt",
            FileFormat::PCBoard => "pcb",
            FileFormat::CtrlA => "msg",
            FileFormat::Renegade => "an1",
            FileFormat::Petscii => "seq",
            FileFormat::Atascii => "ata",
            FileFormat::ViewData => "vtx",
            FileFormat::Mode7 => "m7",
            FileFormat::Rip => "rip",
            FileFormat::SkyPix => "spx",
            FileFormat::Vt52 => "vt52",
            FileFormat::Igs => "ig",
            FileFormat::IcyDraw => "icy",
            FileFormat::IceDraw => "idf",
            FileFormat::Bin => "bin",
            FileFormat::XBin => "xb",
            FileFormat::TundraDraw => "tnd",
            FileFormat::Artworx => "adf",
            FileFormat::IcyAnim => "icyanim",
            FileFormat::Palette(fmt) => fmt.extension(),
            FileFormat::BitFont(font_fmt) => match font_fmt {
                BitFontFormat::Yaff => "yaff",
                BitFontFormat::Psf => "psf",
                BitFontFormat::Raw(4) => "f04",
                BitFontFormat::Raw(5) => "f05",
                BitFontFormat::Raw(6) => "f06",
                BitFontFormat::Raw(7) => "f07",
                BitFontFormat::Raw(8) => "f08",
                BitFontFormat::Raw(9) => "f09",
                BitFontFormat::Raw(10) => "f10",
                BitFontFormat::Raw(12) => "f12",
                BitFontFormat::Raw(14) => "f14",
                BitFontFormat::Raw(16) => "f16",
                BitFontFormat::Raw(19) => "f19",
                BitFontFormat::Raw(20) => "f20",
                BitFontFormat::Raw(24) => "f24",
                BitFontFormat::Raw(32) => "f32",
                BitFontFormat::Raw(_) => "f16", // fallback to most common
            },
            FileFormat::CharacterFont(char_font_fmt) => char_font_fmt.extension(),
            FileFormat::Image(img) => img.extension(),
            FileFormat::Archive(arc) => match arc {
                ArchiveFormat::Ace => "ace",
                ArchiveFormat::Arc => "arc",
                ArchiveFormat::Arj => "arj",
                ArchiveFormat::Zoo => "zoo",
                ArchiveFormat::Sq => "sq",
                ArchiveFormat::Sqz => "sqz",
                ArchiveFormat::Z => "z",
                ArchiveFormat::Gz => "gz",
                ArchiveFormat::Bz2 => "bz2",
                ArchiveFormat::Ice => "ice",
                ArchiveFormat::Hyp => "hyp",
                ArchiveFormat::Ha => "ha",
                ArchiveFormat::Lha => "lha",
                ArchiveFormat::Zip => "zip",
                ArchiveFormat::Rar => "rar",
                ArchiveFormat::SevenZ => "7z",
                ArchiveFormat::Tar => "tar",
                ArchiveFormat::Tgz => "tgz",
                ArchiveFormat::Tbz => "tbz",
                ArchiveFormat::TarZ => "tar.z",
                ArchiveFormat::Uc2 => "uc2",
            },
        }
    }

    /// Get all file extensions recognized for this format.
    ///
    /// # Returns
    /// A slice of all extensions (without leading dots) that map to this format.
    pub fn all_extensions(&self) -> &'static [&'static str] {
        match self {
            FileFormat::Ansi => &["ans", "diz", "nfo", "ice"],
            FileFormat::AnsiMusic => &["ams", "mus"],
            FileFormat::Ascii => &["asc", "txt"],
            FileFormat::Avatar => &["avt"],
            FileFormat::PCBoard => &["pcb"],
            FileFormat::CtrlA => &["msg"],
            FileFormat::Renegade => &["an1", "an2", "an3", "an4", "an5", "an6", "an7", "an8", "an9"],
            FileFormat::Petscii => &["pet", "seq"],
            FileFormat::Atascii => &["ata", "xep"],
            FileFormat::ViewData => &["vtx"],
            FileFormat::Mode7 => &["m7"],
            FileFormat::Rip => &["rip"],
            FileFormat::SkyPix => &["spx"],
            FileFormat::Vt52 => &["vt52", "v52", "vt5"],
            FileFormat::Igs => &["ig"],
            FileFormat::IcyDraw => &["icy"],
            FileFormat::IceDraw => &["idf"],
            FileFormat::Bin => &["bin"],
            FileFormat::XBin => &["xb"],
            FileFormat::TundraDraw => &["tnd"],
            FileFormat::Artworx => &["adf"],
            FileFormat::IcyAnim => &["icyanim"],
            FileFormat::Palette(fmt) => fmt.all_extensions(),
            FileFormat::BitFont(BitFontFormat::Yaff) => &["yaff"],
            FileFormat::BitFont(BitFontFormat::Psf) => &["psf"],
            FileFormat::BitFont(BitFontFormat::Raw(_)) => &["f04", "f05", "f06", "f07", "f08", "f09", "f10", "f12", "f14", "f16", "f19", "f20", "f24", "f32"],
            FileFormat::CharacterFont(CharacterFontFormat::Figlet) => &["flf"],
            FileFormat::CharacterFont(CharacterFontFormat::Tdf) => &["tdf"],
            FileFormat::Image(ImageFormat::Png) => &["png"],
            FileFormat::Image(ImageFormat::Gif) => &["gif"],
            FileFormat::Image(ImageFormat::Jpeg) => &["jpg", "jpeg"],
            FileFormat::Image(ImageFormat::Bmp) => &["bmp"],
            FileFormat::Image(ImageFormat::Sixel) => &["six", "sixel"],
            FileFormat::Archive(ArchiveFormat::Zip) => &["zip"],
            FileFormat::Archive(ArchiveFormat::Arc) => &["arc"],
            FileFormat::Archive(ArchiveFormat::Ace) => &["ace"],
            FileFormat::Archive(ArchiveFormat::Arc) => &["arc"],
            FileFormat::Archive(ArchiveFormat::Arj) => &["arj"],
            FileFormat::Archive(ArchiveFormat::Zoo) => &["zoo"],
            FileFormat::Archive(ArchiveFormat::Sq) => &["sq", "sq2", "qqq"],
            FileFormat::Archive(ArchiveFormat::Sqz) => &["sqz"],
            FileFormat::Archive(ArchiveFormat::Z) => &["z"],
            FileFormat::Archive(ArchiveFormat::Gz) => &["gz"],
            FileFormat::Archive(ArchiveFormat::Bz2) => &["bz2"],
            FileFormat::Archive(ArchiveFormat::Ice) => &["ice"],
            FileFormat::Archive(ArchiveFormat::Hyp) => &["hyp"],
            FileFormat::Archive(ArchiveFormat::Ha) => &["ha"],
            FileFormat::Archive(ArchiveFormat::Lha) => &["lha", "lzh"],
            FileFormat::Archive(ArchiveFormat::Zip) => &["zip"],
            FileFormat::Archive(ArchiveFormat::Rar) => &["rar"],
            FileFormat::Archive(ArchiveFormat::SevenZ) => &["7z"],
            FileFormat::Archive(ArchiveFormat::Tar) => &["tar"],
            FileFormat::Archive(ArchiveFormat::Tgz) => &["tgz", "tar.gz"],
            FileFormat::Archive(ArchiveFormat::Tbz) => &["tbz", "tbz2", "tar.bz2"],
            FileFormat::Archive(ArchiveFormat::TarZ) => &["tar.z"],
            FileFormat::Archive(ArchiveFormat::Uc2) => &["uc2"],
        }
    }

    /// Get a human-readable name for this format.
    pub fn name(&self) -> &'static str {
        match self {
            FileFormat::Ansi => "ANSI",
            FileFormat::AnsiMusic => "ANSI Music",
            FileFormat::Ascii => "ASCII",
            FileFormat::Avatar => "Avatar",
            FileFormat::PCBoard => "PCBoard",
            FileFormat::CtrlA => "CtrlA",
            FileFormat::Renegade => "Renegade",
            FileFormat::Petscii => "PETSCII",
            FileFormat::Atascii => "ATASCII",
            FileFormat::ViewData => "Videotex",
            FileFormat::Mode7 => "Mode 7",
            FileFormat::Rip => "RIPscrip",
            FileFormat::SkyPix => "SkyPix",
            FileFormat::Vt52 => "VT52",
            FileFormat::Igs => "IGS",
            FileFormat::IcyDraw => "IcyDraw",
            FileFormat::IceDraw => "iCE Draw",
            FileFormat::Bin => "Binary",
            FileFormat::XBin => "XBin",
            FileFormat::TundraDraw => "TundraDraw",
            FileFormat::Artworx => "Artworx",
            FileFormat::IcyAnim => "IcyDraw Animation",
            FileFormat::Palette(fmt) => fmt.name(),
            FileFormat::BitFont(font_fmt) => font_fmt.name(),
            FileFormat::CharacterFont(char_font_fmt) => char_font_fmt.name(),
            FileFormat::Image(img) => img.name(),
            FileFormat::Archive(arc) => match arc {
                ArchiveFormat::Ace => "ACE Archive",
                ArchiveFormat::Arc => "ARC Archive",
                ArchiveFormat::Arj => "ARJ Archive",
                ArchiveFormat::Zoo => "ZOO Archive",
                ArchiveFormat::Sq => "Squeezed File",
                ArchiveFormat::Sqz => "SQZ Archive",
                ArchiveFormat::Z => "Unix Compress",
                ArchiveFormat::Gz => "Gzip",
                ArchiveFormat::Bz2 => "Bzip2",
                ArchiveFormat::Ice => "ICE Compressed",
                ArchiveFormat::Hyp => "Hyper Archive",
                ArchiveFormat::Ha => "HA Archive",
                ArchiveFormat::Lha => "LHA Archive",
                ArchiveFormat::Zip => "ZIP Archive",
                ArchiveFormat::Rar => "RAR Archive",
                ArchiveFormat::SevenZ => "7-Zip Archive",
                ArchiveFormat::Tar => "TAR Archive",
                ArchiveFormat::Tgz => "TGZ Archive",
                ArchiveFormat::Tbz => "TBZ Archive",
                ArchiveFormat::TarZ => "TAR.Z Archive",
                ArchiveFormat::Uc2 => "UC2 Archive",
            },
        }
    }

    /// Get the capabilities this format supports.
    pub fn capabilities(&self) -> super::FormatCapabilities {
        use super::FormatCapabilities as C;
        match self {
            // Modern ANSI terminal output - supports almost everything
            FileFormat::Ansi | FileFormat::AnsiMusic => C::UNICODE | C::TRUECOLOR | C::ICE_COLORS | C::SIXEL | C::CONTROL_CHARS,

            // ASCII - minimal features
            FileFormat::Ascii => C::UNICODE,

            // Avatar - basic ANSI-like
            FileFormat::Avatar => C::ICE_COLORS | C::CONTROL_CHARS,

            // PCBoard - no custom palette, no ice colors
            FileFormat::PCBoard => C::CONTROL_CHARS,

            // CtrlA - basic colors only
            FileFormat::CtrlA => C::CONTROL_CHARS,

            // Renegade - similar to PCBoard
            FileFormat::Renegade => C::CONTROL_CHARS,

            // PETSCII - native charset only
            FileFormat::Petscii => C::empty(),

            // ATASCII - native charset only
            FileFormat::Atascii => C::empty(),

            // Viewdata/Mode7
            FileFormat::ViewData | FileFormat::Mode7 => C::empty(),

            // Binary format - requires even width
            FileFormat::Bin => C::ICE_COLORS | C::REQUIRE_EVEN_WIDTH,

            // XBin - full DOS features plus extended attributes
            FileFormat::XBin => C::CUSTOM_PALETTE | C::ICE_COLORS | C::CUSTOM_FONT | C::XBIN_EXTENDED | C::REQUIRE_EVEN_WIDTH,

            // TundraDraw - 24-bit color support
            FileFormat::TundraDraw => C::TRUECOLOR | C::ICE_COLORS,

            // Artworx - basic DOS format
            FileFormat::Artworx => C::ICE_COLORS,

            // iCE Draw - DOS features with custom palette/font
            FileFormat::IceDraw => C::CUSTOM_PALETTE | C::ICE_COLORS | C::CUSTOM_FONT,

            // IcyDraw native - supports everything
            FileFormat::IcyDraw | FileFormat::IcyAnim => {
                C::UNICODE | C::TRUECOLOR | C::CUSTOM_PALETTE | C::ICE_COLORS | C::CUSTOM_FONT | C::UNLIMITED_FONTS | C::SIXEL | C::CONTROL_CHARS
            }

            // Graphics/special formats
            FileFormat::Rip | FileFormat::SkyPix | FileFormat::Vt52 | FileFormat::Igs => C::empty(),

            // Non-buffer formats
            FileFormat::Palette(_) | FileFormat::BitFont(_) | FileFormat::CharacterFont(_) | FileFormat::Image(_) | FileFormat::Archive(_) => C::empty(),
        }
    }

    /// Maximum width this format supports (None = unlimited).
    pub fn max_width(&self) -> Option<i32> {
        match self {
            FileFormat::Bin => Some(255),    // BIN uses 1 byte for width in SAUCE
            FileFormat::XBin => Some(65535), // 2 bytes
            _ => None,
        }
    }

    /// Maximum height this format supports (None = unlimited).
    pub fn max_height(&self) -> Option<i32> {
        match self {
            FileFormat::Bin => Some(65535),
            FileFormat::XBin => Some(65535),
            _ => None,
        }
    }

    /// Check compatibility between a buffer and this format.
    ///
    /// Returns a list of issues found. Empty list means fully compatible.
    pub fn check_compatibility(&self, buffer: &TextBuffer) -> Vec<super::CompatibilityIssue> {
        use super::{CompatibilityIssue, FormatCapabilities, IssueType};

        let requirements = buffer.analyze_capability_requirements();
        let caps = self.capabilities();
        let mut issues = Vec::new();

        // Check width constraints
        if caps.contains(FormatCapabilities::REQUIRE_EVEN_WIDTH) && requirements.width % 2 != 0 {
            issues.push(CompatibilityIssue::error(
                IssueType::OddWidthNotAllowed { width: requirements.width },
                format!("Format requires even width, buffer has {} columns", requirements.width),
            ));
        }

        // Check max dimensions for specific formats
        if let Some(max_width) = self.max_width() {
            if requirements.width > max_width {
                issues.push(CompatibilityIssue::error(
                    IssueType::WidthExceeded {
                        width: requirements.width,
                        max: max_width,
                    },
                    format!("Buffer width {} exceeds format maximum of {}", requirements.width, max_width),
                ));
            }
        }

        if let Some(max_height) = self.max_height() {
            if requirements.height > max_height {
                issues.push(CompatibilityIssue::error(
                    IssueType::HeightExceeded {
                        height: requirements.height,
                        max: max_height,
                    },
                    format!("Buffer height {} exceeds format maximum of {}", requirements.height, max_height),
                ));
            }
        }

        // Check truecolor
        if requirements.uses_truecolor && !caps.contains(FormatCapabilities::TRUECOLOR) {
            issues.push(CompatibilityIssue::warning(
                IssueType::TruecolorUnsupported,
                "Format doesn't support 24-bit colors, will be quantized to palette",
            ));
        }

        // Check custom palette
        if requirements.has_custom_palette && !caps.contains(FormatCapabilities::CUSTOM_PALETTE) {
            issues.push(CompatibilityIssue::warning(
                IssueType::CustomPaletteUnsupported,
                "Format doesn't support custom palettes, default palette will be used",
            ));
        }

        // Check ice colors
        if requirements.uses_ice_colors && !caps.contains(FormatCapabilities::ICE_COLORS) {
            issues.push(CompatibilityIssue::warning(
                IssueType::IceColorsUnsupported,
                "Format doesn't support iCE colors, high-intensity backgrounds will be lost",
            ));
        }

        // Check custom font
        if requirements.has_custom_font && !caps.contains(FormatCapabilities::CUSTOM_FONT) {
            issues.push(CompatibilityIssue::warning(
                IssueType::CustomFontUnsupported,
                "Format doesn't support custom fonts, default font will be used",
            ));
        }

        // Check multiple fonts
        if requirements.font_count > 1 && !caps.contains(FormatCapabilities::UNLIMITED_FONTS) {
            // XBin supports 2 fonts via XBIN_EXTENDED
            let max_fonts = if caps.contains(FormatCapabilities::XBIN_EXTENDED) { 2 } else { 1 };
            if requirements.font_count > max_fonts {
                issues.push(CompatibilityIssue::warning(
                    IssueType::MultipleFontsUnsupported {
                        font_count: requirements.font_count,
                    },
                    format!("Format supports max {} font(s), buffer uses {}", max_fonts, requirements.font_count),
                ));
            }
        }

        // Check sixels
        if requirements.has_sixels && !caps.contains(FormatCapabilities::SIXEL) {
            issues.push(CompatibilityIssue::error(
                IssueType::SixelUnsupported,
                "Format doesn't support SIXEL graphics, images will be lost",
            ));
        }

        // Check extended attributes
        if requirements.uses_extended_attributes && !caps.contains(FormatCapabilities::XBIN_EXTENDED) {
            issues.push(CompatibilityIssue::warning(
                IssueType::ExtendedAttributesUnsupported,
                "Format doesn't support extended attributes (underline, etc.)",
            ));
        }

        // Check control characters
        if requirements.has_control_chars && !caps.contains(FormatCapabilities::CONTROL_CHARS) {
            issues.push(CompatibilityIssue::warning(
                IssueType::ControlCharsUnsupported,
                "Format doesn't support control characters (0x00-0x1F), they will be replaced",
            ));
        }

        issues
    }

    /// Check if this format can save the buffer without errors (warnings are OK).
    pub fn can_save_lossless(&self, buffer: &TextBuffer) -> bool {
        self.check_compatibility(buffer)
            .iter()
            .all(|issue| issue.severity != super::IssueSeverity::Error)
    }

    /// Check if this is an image format.
    pub fn is_image(&self) -> bool {
        matches!(self, FileFormat::Image(_))
    }

    /// Check if this is an archive format.
    pub fn is_archive(&self) -> bool {
        matches!(self, FileFormat::Archive(_))
    }

    /// Get the ArchiveFormat if this is an archive, None otherwise.
    pub fn as_archive(&self) -> Option<ArchiveFormat> {
        match self {
            FileFormat::Archive(arc) => Some(*arc),
            _ => None,
        }
    }

    /// Get the ImageFormat if this is an image, None otherwise.
    pub fn as_image(&self) -> Option<ImageFormat> {
        match self {
            FileFormat::Image(img) => Some(*img),
            _ => None,
        }
    }

    /// Get the BitFontFormat if this is a bitmap font, None otherwise.
    pub fn as_bitfont(&self) -> Option<BitFontFormat> {
        match self {
            FileFormat::BitFont(font) => Some(*font),
            _ => None,
        }
    }

    /// Check if this is a character font format.
    pub fn is_character_font(&self) -> bool {
        matches!(self, FileFormat::CharacterFont(_))
    }

    /// Get the CharacterFontFormat if this is a character font, None otherwise.
    pub fn as_character_font(&self) -> Option<CharacterFontFormat> {
        match self {
            FileFormat::CharacterFont(font) => Some(*font),
            _ => None,
        }
    }

    /// Check if this format uses a streaming parser.
    ///
    /// Parser-based formats process data incrementally and support
    /// features like baud emulation for animated playback.
    pub fn uses_parser(&self) -> bool {
        matches!(
            self,
            FileFormat::Ansi
                | FileFormat::AnsiMusic
                | FileFormat::Ascii
                | FileFormat::Avatar
                | FileFormat::PCBoard
                | FileFormat::CtrlA
                | FileFormat::Renegade
                | FileFormat::Petscii
                | FileFormat::Atascii
                | FileFormat::ViewData
                | FileFormat::Mode7
                | FileFormat::Rip
                | FileFormat::SkyPix
                | FileFormat::Vt52
                | FileFormat::Igs
        )
    }

    /// Check if this format supports saving.
    pub fn supports_save(&self) -> bool {
        match self {
            FileFormat::Image(img) => img.supports_save(),
            FileFormat::Ansi
            | FileFormat::AnsiMusic
            | FileFormat::Ascii
            | FileFormat::Avatar
            | FileFormat::PCBoard
            | FileFormat::CtrlA
            | FileFormat::Renegade
            | FileFormat::Atascii
            | FileFormat::Petscii
            | FileFormat::Bin
            | FileFormat::XBin
            | FileFormat::IcyDraw
            | FileFormat::IceDraw
            | FileFormat::TundraDraw
            | FileFormat::Artworx => true,
            _ => false,
        }
    }

    /// Check if this format can contain animations.
    pub fn is_animated(&self) -> bool {
        matches!(self, FileFormat::IcyAnim)
    }

    /// Get the buffer types that this format supports.
    ///
    /// Returns a list of `BufferType` values that can be saved in this format.
    /// For example, ANSI supports CP437 and Unicode, while PETSCII only supports Petscii.
    pub fn supported_buffer_types(&self) -> &'static [BufferType] {
        match self {
            // CP437/ANSI-based formats support CP437 and Unicode
            FileFormat::Ansi
            | FileFormat::AnsiMusic
            | FileFormat::Ascii
            | FileFormat::Avatar
            | FileFormat::PCBoard
            | FileFormat::CtrlA
            | FileFormat::Renegade
            | FileFormat::Bin
            | FileFormat::IceDraw
            | FileFormat::Artworx => &[BufferType::CP437, BufferType::Unicode],

            // XBin and TundraDraw support extended features
            FileFormat::XBin | FileFormat::TundraDraw => &[BufferType::CP437, BufferType::Unicode],

            // IcyDraw native format supports all buffer types
            FileFormat::IcyDraw | FileFormat::IcyAnim => &[
                BufferType::CP437,
                BufferType::Unicode,
                BufferType::Petscii,
                BufferType::Atascii,
                BufferType::Viewdata,
            ],

            // PETSCII only
            FileFormat::Petscii => &[BufferType::Petscii],

            // ATASCII only
            FileFormat::Atascii => &[BufferType::Atascii],

            // Viewdata/Mode7 only
            FileFormat::ViewData | FileFormat::Mode7 => &[BufferType::Viewdata],

            // Graphics formats - treat as CP437 compatible
            FileFormat::Rip | FileFormat::SkyPix | FileFormat::Vt52 | FileFormat::Igs => &[BufferType::CP437, BufferType::Unicode],

            // Image formats - support all buffer types (they render pixels)
            FileFormat::Image(_) => &[
                BufferType::CP437,
                BufferType::Unicode,
                BufferType::Petscii,
                BufferType::Atascii,
                BufferType::Viewdata,
            ],

            // BitFont formats don't contain text buffer content
            FileFormat::BitFont(_) => &[],

            // CharacterFont formats don't contain text buffer content
            FileFormat::CharacterFont(_) => &[],

            // Archive formats don't contain displayable content directly
            FileFormat::Archive(_) => &[],

            // Palette formats don't contain text buffer content
            FileFormat::Palette(_) => &[],
        }
    }

    /// Check if this format is compatible with a specific buffer type.
    ///
    /// # Arguments
    /// * `buffer_type` - The buffer type to check compatibility with
    ///
    /// # Returns
    /// `true` if this format can save content with the given buffer type.
    ///
    /// # Example
    /// ```
    /// use icy_engine::{BufferType, formats::FileFormat};
    ///
    /// // ANSI can save CP437 content
    /// assert!(FileFormat::Ansi.is_compatible_with(BufferType::CP437));
    ///
    /// // ANSI cannot save Viewdata content
    /// assert!(!FileFormat::Ansi.is_compatible_with(BufferType::Viewdata));
    ///
    /// // PETSCII can only save PETSCII content
    /// assert!(FileFormat::Petscii.is_compatible_with(BufferType::Petscii));
    /// assert!(!FileFormat::Petscii.is_compatible_with(BufferType::CP437));
    /// ```
    pub fn is_compatible_with(&self, buffer_type: BufferType) -> bool {
        self.supported_buffer_types().contains(&buffer_type)
    }

    /// Get all file formats that can save content with the given buffer type.
    ///
    /// # Arguments
    /// * `buffer_type` - The buffer type to find compatible formats for
    ///
    /// # Returns
    /// A vector of `FileFormat` values that support saving the given buffer type.
    ///
    /// # Example
    /// ```
    /// use icy_engine::{BufferType, formats::FileFormat};
    ///
    /// let cp437_formats = FileFormat::save_formats_for_buffer_type(BufferType::CP437);
    /// assert!(cp437_formats.contains(&FileFormat::Ansi));
    /// assert!(cp437_formats.contains(&FileFormat::XBin));
    /// assert!(!cp437_formats.contains(&FileFormat::Petscii));
    ///
    /// let viewdata_formats = FileFormat::save_formats_for_buffer_type(BufferType::Viewdata);
    /// assert!(viewdata_formats.contains(&FileFormat::ViewData));
    /// assert!(viewdata_formats.contains(&FileFormat::IcyDraw)); // IcyDraw supports all
    /// assert!(!viewdata_formats.contains(&FileFormat::Ansi));
    /// ```
    pub fn save_formats_for_buffer_type(buffer_type: BufferType) -> Vec<FileFormat> {
        FileFormat::SAVEABLE.iter().copied().filter(|fmt| fmt.is_compatible_with(buffer_type)).collect()
    }

    /// Get all file formats that can save content with the given buffer type,
    /// including image formats (PNG, GIF).
    ///
    /// Image formats can save any buffer type since they render the content to pixels.
    pub fn save_formats_with_images_for_buffer_type(buffer_type: BufferType) -> Vec<FileFormat> {
        FileFormat::SAVEABLE_WITH_IMAGES
            .iter()
            .copied()
            .filter(|fmt| fmt.is_compatible_with(buffer_type))
            .collect()
    }

    /// Get the terminal emulation type for this format.
    ///
    /// # Returns
    /// The appropriate `TerminalEmulation` for parser-based formats,
    /// or `None` for binary formats that don't use terminal emulation.
    pub fn terminal_emulation(&self) -> Option<TerminalEmulation> {
        match self {
            FileFormat::Ansi => Some(TerminalEmulation::Ansi),
            FileFormat::AnsiMusic => Some(TerminalEmulation::Ansi),
            FileFormat::Ascii => Some(TerminalEmulation::Ascii),
            FileFormat::Avatar => Some(TerminalEmulation::Avatar),
            FileFormat::PCBoard => Some(TerminalEmulation::Ansi),  // PCBoard uses ANSI with extensions
            FileFormat::CtrlA => Some(TerminalEmulation::Ansi),    // CtrlA uses ANSI with extensions
            FileFormat::Renegade => Some(TerminalEmulation::Ansi), // Renegade uses ANSI with extensions
            FileFormat::Petscii => Some(TerminalEmulation::PETscii),
            FileFormat::Atascii => Some(TerminalEmulation::ATAscii),
            FileFormat::ViewData => Some(TerminalEmulation::ViewData),
            FileFormat::Mode7 => Some(TerminalEmulation::Mode7),
            FileFormat::Rip => Some(TerminalEmulation::Rip),
            FileFormat::SkyPix => Some(TerminalEmulation::Skypix),
            FileFormat::Vt52 => Some(TerminalEmulation::AtariST),
            FileFormat::Igs => Some(TerminalEmulation::AtariST),
            // Binary formats don't use terminal emulation
            FileFormat::IcyDraw
            | FileFormat::IceDraw
            | FileFormat::Bin
            | FileFormat::XBin
            | FileFormat::TundraDraw
            | FileFormat::Artworx
            | FileFormat::IcyAnim
            | FileFormat::BitFont(_)
            | FileFormat::CharacterFont(_)
            | FileFormat::Image(_)
            | FileFormat::Archive(_)
            | FileFormat::Palette(_) => None,
        }
    }

    /// Get the default screen mode for this format.
    ///
    /// # Returns
    /// The appropriate `ScreenMode` for displaying content in this format.
    pub fn screen_mode(&self) -> ScreenMode {
        match self {
            FileFormat::Ansi
            | FileFormat::AnsiMusic
            | FileFormat::Ascii
            | FileFormat::Avatar
            | FileFormat::PCBoard
            | FileFormat::CtrlA
            | FileFormat::Renegade
            | FileFormat::IcyDraw
            | FileFormat::IceDraw
            | FileFormat::Bin
            | FileFormat::XBin
            | FileFormat::TundraDraw
            | FileFormat::Artworx
            | FileFormat::IcyAnim => ScreenMode::Vga(80, 25),
            FileFormat::Petscii => ScreenMode::Vic,
            FileFormat::Atascii => ScreenMode::Atascii(40),
            FileFormat::ViewData | FileFormat::Mode7 => ScreenMode::Videotex,
            FileFormat::Rip => ScreenMode::Rip,
            FileFormat::SkyPix => ScreenMode::SkyPix,
            FileFormat::Vt52 => ScreenMode::AtariST(crate::TerminalResolution::Medium, false),
            FileFormat::Igs => ScreenMode::AtariST(crate::TerminalResolution::Medium, true),
            FileFormat::BitFont(_) => ScreenMode::Vga(80, 25),       // Default for fonts
            FileFormat::CharacterFont(_) => ScreenMode::Vga(80, 25), // Default for character fonts
            FileFormat::Image(_) => ScreenMode::Vga(80, 25),         // Default for images
            FileFormat::Archive(_) => ScreenMode::Vga(80, 25),       // Default for archives
            FileFormat::Palette(_) => ScreenMode::Vga(80, 25),       // Default for palettes
        }
    }

    /// Create a parser for this format.
    ///
    /// # Arguments
    /// * `music_option` - Optional music setting for ANSI-based parsers
    ///
    /// # Returns
    /// A boxed parser if this format uses streaming parsing, `None` otherwise.
    pub fn create_parser(&self, music_option: Option<MusicOption>) -> Option<Box<dyn CommandParser + Send>> {
        match self {
            FileFormat::Ansi => {
                let mut parser = icy_parser_core::AnsiParser::new();
                if let Some(opt) = music_option {
                    parser.music_option = opt;
                }
                Some(Box::new(parser))
            }
            FileFormat::AnsiMusic => {
                let mut parser = icy_parser_core::AnsiParser::new();
                parser.music_option = music_option.unwrap_or(MusicOption::Both);
                Some(Box::new(parser))
            }
            FileFormat::Ascii => Some(Box::new(icy_parser_core::AsciiParser::new())),
            FileFormat::Avatar => Some(Box::new(icy_parser_core::AvatarParser::new())),
            FileFormat::PCBoard => Some(Box::new(icy_parser_core::PcBoardParser::new())),
            FileFormat::CtrlA => Some(Box::new(icy_parser_core::CtrlAParser::new())),
            FileFormat::Renegade => Some(Box::new(icy_parser_core::RenegadeParser::new())),
            FileFormat::Petscii => Some(Box::new(icy_parser_core::PetsciiParser::new())),
            FileFormat::Atascii => Some(Box::new(icy_parser_core::AtasciiParser::new())),
            FileFormat::ViewData => Some(Box::new(icy_parser_core::ViewdataParser::new())),
            FileFormat::Mode7 => Some(Box::new(icy_parser_core::Mode7Parser::new())),
            FileFormat::Rip => Some(Box::new(icy_parser_core::RipParser::new())),
            FileFormat::SkyPix => Some(Box::new(icy_parser_core::SkypixParser::new())),
            FileFormat::Vt52 => Some(Box::new(icy_parser_core::Vt52Parser::new(icy_parser_core::VT52Mode::Mixed))),
            FileFormat::Igs => {
                let mut parser = icy_parser_core::IgsParser::new();
                parser.run_loop = true;
                Some(Box::new(parser))
            }
            // Binary formats don't use parsers
            FileFormat::IcyDraw
            | FileFormat::IceDraw
            | FileFormat::Bin
            | FileFormat::XBin
            | FileFormat::TundraDraw
            | FileFormat::Artworx
            | FileFormat::IcyAnim
            | FileFormat::BitFont(_)
            | FileFormat::CharacterFont(_)
            | FileFormat::Image(_)
            | FileFormat::Archive(_)
            | FileFormat::Palette(_) => None,
        }
    }

    /// Load content from bytes into a LoadedDocument.
    ///
    /// # Arguments
    /// * `data` - The raw file data
    /// * `load_data` - Optional loading options (music settings, terminal width, etc.)
    ///
    /// # Returns
    /// A `LoadedDocument` containing the loaded screen and any SAUCE metadata.
    /// SAUCE settings are automatically applied to the screen.
    ///
    /// # Errors
    /// Returns an error if the format doesn't support loading or if loading fails.
    ///
    /// # Example
    /// ```no_run
    /// use icy_engine::formats::{FileFormat, LoadData};
    /// use icy_engine::TextPane;
    ///
    /// let data = std::fs::read("artwork.ans").unwrap();
    /// let format = FileFormat::from_extension("ans").unwrap();
    /// let loaded = format.from_bytes(&data, None).unwrap();
    /// println!("Loaded {}x{}", loaded.screen.buffer.width(), loaded.screen.buffer.height());
    /// if let Some(sauce) = &loaded.sauce_opt {
    ///     println!("Title: {}", sauce.title());
    /// }
    /// ```
    pub fn from_bytes(&self, data: &[u8], load_data: Option<LoadData>) -> Result<LoadedDocument> {
        // IcyDraw has embedded SAUCE record in PNG chunks, handle separately
        if matches!(self, FileFormat::IcyDraw) {
            let (mut screen, sauce_opt) = io::load_icy_draw(data, load_data.as_ref())?;

            // Apply max height limit if specified
            if let Some(max_height) = load_data.as_ref().and_then(|ld| ld.max_height()) {
                if screen.height() > max_height {
                    screen.buffer.set_height(max_height);
                }
            }

            return Ok(LoadedDocument { screen, sauce_opt });
        }

        // Extract SAUCE record from the data (appended at end of file)
        let sauce_opt: Option<icy_sauce::SauceRecord> = icy_sauce::SauceRecord::from_bytes(data).ok().flatten();

        // Strip SAUCE data from file content for binary formats that don't handle it internally
        let stripped_data = icy_sauce::strip_sauce(data, icy_sauce::StripMode::All);

        let mut screen = match self {
            FileFormat::Ansi | FileFormat::AnsiMusic => io::load_ansi(stripped_data, load_data.as_ref(), sauce_opt.as_ref()),
            FileFormat::Ascii => io::load_ascii(stripped_data, load_data.as_ref(), sauce_opt.as_ref()),
            FileFormat::Avatar => io::load_avatar(stripped_data, load_data.as_ref(), sauce_opt.as_ref()),
            FileFormat::PCBoard => io::load_pcboard(stripped_data, load_data.as_ref(), sauce_opt.as_ref()),
            FileFormat::CtrlA => io::load_ctrla(stripped_data, load_data.as_ref(), sauce_opt.as_ref()),
            FileFormat::Renegade => io::load_renegade(stripped_data, load_data.as_ref(), sauce_opt.as_ref()),
            FileFormat::Atascii => io::load_atascii(stripped_data, load_data.as_ref(), sauce_opt.as_ref()),
            FileFormat::Petscii => io::load_seq(stripped_data, load_data.as_ref(), sauce_opt.as_ref()),
            FileFormat::Bin => io::load_bin(stripped_data, load_data.as_ref(), sauce_opt.as_ref()),
            FileFormat::XBin => io::load_xbin(stripped_data, load_data.as_ref(), sauce_opt.as_ref()),
            FileFormat::IcyDraw => unreachable!(), // Handled above
            FileFormat::IceDraw => io::load_ice_draw(stripped_data, load_data.as_ref(), sauce_opt.as_ref()),
            FileFormat::TundraDraw => io::load_tundra(stripped_data, load_data.as_ref(), sauce_opt.as_ref()),
            FileFormat::Artworx => io::load_artworx(stripped_data, load_data.as_ref(), sauce_opt.as_ref()),
            _ => Err(EngineError::FormatNotSupported {
                name: self.name().to_string(),
                operation: "loading".to_string(),
            }),
        }?;

        // Apply max height limit if specified
        if let Some(max_height) = load_data.as_ref().and_then(|ld| ld.max_height()) {
            if screen.height() > max_height {
                screen.buffer.set_height(max_height);
            }
        }

        Ok(LoadedDocument { screen, sauce_opt })
    }

    /// Load content from a file path.
    ///
    /// This is a convenience method that reads the file and calls `from_bytes`.
    /// SAUCE metadata is automatically extracted and applied.
    ///
    /// # Arguments
    /// * `file_path` - Path to the file to load
    /// * `load_data` - Optional loading options
    ///
    /// # Returns
    /// A `LoadedDocument` containing the loaded screen and any SAUCE metadata.
    ///
    /// # Errors
    /// Returns an error if file reading fails or if the format doesn't support loading.
    pub fn load(&self, file_path: &Path, load_data: Option<LoadData>) -> Result<LoadedDocument> {
        let data = std::fs::read(file_path)?;
        self.from_bytes(&data, load_data)
    }

    /// Save a buffer to bytes.
    ///
    /// # Arguments
    /// * `buffer` - The buffer to save
    /// * `options` - Save options controlling output format details
    ///
    /// # Returns
    /// The serialized file data as bytes.
    ///
    /// # Errors
    /// Returns an error if the format doesn't support saving or if saving fails.
    /// Note: Image formats require `save_to_file` instead as they need a file path.
    pub fn to_bytes(&self, buffer: &TextBuffer, options: &SaveOptions) -> Result<Vec<u8>> {
        if let FileFormat::Image(img) = self {
            return Err(EngineError::FormatNotSupported {
                name: img.name().to_string(),
                operation: "to_bytes() - use ImageFormat::save_buffer() with a file path".to_string(),
            });
        }

        // Apply color optimizer if not lossless output
        let buffer = if self == &FileFormat::IcyDraw {
            // IcyDraw native format
            buffer.clone()
        } else if options.is_lossless() {
            let mut buffer = buffer.clone();
            buffer.show_tags = false;
            buffer
        } else {
            let optimizer = crate::ColorOptimizer::new(buffer, options);
            let mut buffer = optimizer.optimize(buffer);
            buffer.show_tags = false;
            buffer
        };

        match self {
            FileFormat::Ansi | FileFormat::AnsiMusic => io::save_ansi(&buffer, options),
            FileFormat::Ascii => io::save_ascii(&buffer, options),
            FileFormat::Avatar => io::save_avatar(&buffer, options),
            FileFormat::PCBoard => io::save_pcboard(&buffer, options),
            FileFormat::CtrlA => io::save_ctrla(&buffer, options),
            FileFormat::Renegade => io::save_renegade(&buffer, options),
            FileFormat::Atascii => io::save_atascii(&buffer, options),
            FileFormat::Petscii => io::save_seq(&buffer, options),
            FileFormat::Bin => io::save_bin(&buffer, options),
            FileFormat::XBin => io::save_xbin(&buffer, options),
            FileFormat::IcyDraw => io::save_icy_draw(&buffer, options),
            FileFormat::IceDraw => io::save_ice_draw(&buffer, options),
            FileFormat::TundraDraw => io::save_tundra(&buffer, options),
            FileFormat::Artworx => io::save_artworx(&buffer, options),
            _ => Err(EngineError::FormatNotSupported {
                name: self.name().to_string(),
                operation: "saving".to_string(),
            }),
        }
    }

    /// Check if this format is supported for viewing/loading.
    ///
    /// A format is considered supported if it either:
    /// - Uses a streaming parser (can be played back with baud emulation)
    /// - Has a direct loader implementation
    /// - Is an animation format (.icyanim)
    /// - Is an image format (PNG, GIF, etc.)
    /// - Is a font format (.tdf, .flf, .psf, etc.)
    ///
    /// # Returns
    /// `true` if the format can be loaded/viewed, `false` otherwise.
    ///
    /// # Example
    /// ```
    /// use icy_engine::formats::FileFormat;
    ///
    /// // ANSI files are supported (parser-based)
    /// assert!(FileFormat::Ansi.is_supported());
    ///
    /// // XBin files are supported (direct load)
    /// assert!(FileFormat::XBin.is_supported());
    ///
    /// // Animation files are supported
    /// assert!(FileFormat::IcyAnim.is_supported());
    ///
    /// // Archives are not directly viewable
    /// assert!(!FileFormat::Archive(unarc_rs::unified::ArchiveFormat::Zip).is_supported());
    /// ```
    pub fn is_supported(&self) -> bool {
        self.uses_parser() || self.supports_save() || self.is_animated() || self.is_image() || self.is_character_font() || self.is_bitfont()
    }

    pub fn bitfont_format(&self) -> Option<BitFontFormat> {
        match self {
            FileFormat::BitFont(font_fmt) => Some(*font_fmt),
            _ => None,
        }
    }

    /// Check if this is a bitmap font format.
    pub fn is_bitfont(&self) -> bool {
        self.bitfont_format().is_some()
    }

    pub fn open_archive<T: Read + Seek>(&self, reader: T) -> Result<UnifiedArchive<T>> {
        match self {
            FileFormat::Archive(arc_fmt) => Ok(arc_fmt.open(reader)?),
            _ => Err(EngineError::FormatNotSupported {
                name: self.name().to_string(),
                operation: "opening as archive".to_string(),
            }),
        }
    }
}

impl std::fmt::Display for FileFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}
