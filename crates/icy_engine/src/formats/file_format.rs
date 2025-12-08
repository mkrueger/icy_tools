//! File format registry for unified file handling.
//!
//! This module provides a central registry for all supported file formats,
//! enabling consistent file type detection, parser selection, and save/load operations.
//!
//! # Example
//!
//! ```no_run
//! use icy_engine::formats::FileFormat;
//! use std::path::Path;
//!
//! // Detect format from file extension
//! let format = FileFormat::from_path(Path::new("artwork.ans")).unwrap();
//! assert!(format.uses_parser());
//! assert!(format.supports_save());
//!
//! // Get parser for streaming formats
//! if let Some(parser) = format.create_parser(None) {
//!     // Use parser for streaming playback
//! }
//! ```

use std::path::Path;

use icy_net::telnet::TerminalEmulation;
use icy_parser_core::{CommandParser, MusicOption};
use unarc_rs::unified::ArchiveFormat;

use crate::{BufferType, EngineError, Result, ScreenMode, TextBuffer};

use super::{FORMATS, ImageFormat, LoadData, OutputFormat, SaveOptions};

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

            // Image formats
            "png" => Some(FileFormat::Image(ImageFormat::Png)),
            "gif" => Some(FileFormat::Image(ImageFormat::Gif)),
            "jpg" | "jpeg" => Some(FileFormat::Image(ImageFormat::Jpeg)),
            "bmp" => Some(FileFormat::Image(ImageFormat::Bmp)),
            "six" | "sixel" => Some(FileFormat::Image(ImageFormat::Sixel)),

            // Try archive formats
            _ => ArchiveFormat::from_extension(&ext_lower).map(FileFormat::Archive),
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
            FileFormat::Image(img) => img.extension(),
            FileFormat::Archive(arc) => match arc {
                ArchiveFormat::Zip => "zip",
                ArchiveFormat::Arc => "arc",
                ArchiveFormat::Arj => "arj",
                ArchiveFormat::Zoo => "zoo",
                ArchiveFormat::Lha => "lha",
                ArchiveFormat::Rar => "rar",
                ArchiveFormat::Sq => "sq",
                ArchiveFormat::Sqz => "sqz",
                ArchiveFormat::Z => "z",
                ArchiveFormat::Hyp => "hyp",
                ArchiveFormat::Uc2 => "uc2",
                ArchiveFormat::SevenZ => "7z",
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
            FileFormat::Image(ImageFormat::Png) => &["png"],
            FileFormat::Image(ImageFormat::Gif) => &["gif"],
            FileFormat::Image(ImageFormat::Jpeg) => &["jpg", "jpeg"],
            FileFormat::Image(ImageFormat::Bmp) => &["bmp"],
            FileFormat::Image(ImageFormat::Sixel) => &["six", "sixel"],
            FileFormat::Archive(ArchiveFormat::Zip) => &["zip"],
            FileFormat::Archive(ArchiveFormat::Arc) => &["arc"],
            FileFormat::Archive(ArchiveFormat::Arj) => &["arj"],
            FileFormat::Archive(ArchiveFormat::Zoo) => &["zoo"],
            FileFormat::Archive(ArchiveFormat::Lha) => &["lha", "lzh"],
            FileFormat::Archive(ArchiveFormat::Rar) => &["rar"],
            FileFormat::Archive(ArchiveFormat::Sq) => &["sq", "sq2", "qqq"],
            FileFormat::Archive(ArchiveFormat::Sqz) => &["sqz"],
            FileFormat::Archive(ArchiveFormat::Z) => &["z"],
            FileFormat::Archive(ArchiveFormat::Hyp) => &["hyp"],
            FileFormat::Archive(ArchiveFormat::Uc2) => &["uc2"],
            FileFormat::Archive(ArchiveFormat::SevenZ) => &["7z"],
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
            FileFormat::Image(img) => img.name(),
            FileFormat::Archive(arc) => match arc {
                ArchiveFormat::Zip => "ZIP Archive",
                ArchiveFormat::Arc => "ARC Archive",
                ArchiveFormat::Arj => "ARJ Archive",
                ArchiveFormat::Zoo => "ZOO Archive",
                ArchiveFormat::Lha => "LHA Archive",
                ArchiveFormat::Rar => "RAR Archive",
                ArchiveFormat::Sq => "Squeezed File",
                ArchiveFormat::Sqz => "SQZ Archive",
                ArchiveFormat::Z => "Unix Compress",
                ArchiveFormat::Hyp => "Hyper Archive",
                ArchiveFormat::Uc2 => "UC2 Archive",
                ArchiveFormat::SevenZ => "7-Zip Archive",
            },
        }
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
            _ => self.output_format().is_some(),
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

            // Archive formats don't contain displayable content directly
            FileFormat::Archive(_) => &[],
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
            | FileFormat::Image(_)
            | FileFormat::Archive(_) => None,
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
            FileFormat::Image(_) => ScreenMode::Vga(80, 25),   // Default for images
            FileFormat::Archive(_) => ScreenMode::Vga(80, 25), // Default for archives
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
            | FileFormat::Image(_)
            | FileFormat::Archive(_) => None,
        }
    }

    /// Get the OutputFormat implementation for this format.
    ///
    /// # Returns
    /// A reference to the `OutputFormat` trait object if this format supports
    /// saving, `None` otherwise.
    pub fn output_format(&self) -> Option<&'static dyn OutputFormat> {
        let ext = self.primary_extension();
        for format in FORMATS.iter() {
            if format.get_file_extension() == ext {
                return Some(format.as_ref());
            }
            if format.get_alt_extensions().iter().any(|e| e == ext) {
                return Some(format.as_ref());
            }
        }
        None
    }

    /// Load a buffer from file data.
    ///
    /// # Arguments
    /// * `file_name` - Path to the file (used for format-specific handling)
    /// * `data` - The raw file data
    /// * `load_data` - Optional loading options (SAUCE, music settings, etc.)
    ///
    /// # Returns
    /// A `TextBuffer` containing the loaded content.
    ///
    /// # Errors
    /// Returns an error if the format doesn't support loading or if loading fails.
    pub fn load_buffer(&self, file_name: &Path, data: &[u8], load_data: Option<LoadData>) -> Result<TextBuffer> {
        if let Some(output_format) = self.output_format() {
            output_format.load_buffer(file_name, data, load_data)
        } else {
            Err(EngineError::FormatNotSupported {
                name: self.name().to_string(),
                operation: "loading via OutputFormat".to_string(),
            })
        }
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
    pub fn to_bytes(&self, buffer: &mut TextBuffer, options: &SaveOptions) -> Result<Vec<u8>> {
        if let FileFormat::Image(img) = self {
            return Err(EngineError::FormatNotSupported {
                name: img.name().to_string(),
                operation: "to_bytes() - use ImageFormat::save_buffer() with a file path".to_string(),
            });
        }
        if let Some(output_format) = self.output_format() {
            output_format.to_bytes(buffer, options)
        } else {
            Err(EngineError::FormatNotSupported {
                name: self.name().to_string(),
                operation: "saving".to_string(),
            })
        }
    }

    /// Check if this format is supported for viewing/loading.
    ///
    /// A format is considered supported if it either:
    /// - Uses a streaming parser (can be played back with baud emulation)
    /// - Has a loader via `output_format()` (can be loaded directly)
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
    /// // Archives are not directly viewable
    /// assert!(!FileFormat::Archive(unarc_rs::unified::ArchiveFormat::Zip).is_supported());
    /// ```
    pub fn is_supported(&self) -> bool {
        self.uses_parser() || self.output_format().is_some()
    }
}

impl std::fmt::Display for FileFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_extension() {
        assert_eq!(FileFormat::from_extension("ans"), Some(FileFormat::Ansi));
        assert_eq!(FileFormat::from_extension("ANS"), Some(FileFormat::Ansi));
        assert_eq!(FileFormat::from_extension("diz"), Some(FileFormat::Ansi));
        assert_eq!(FileFormat::from_extension("xb"), Some(FileFormat::XBin));
        assert_eq!(FileFormat::from_extension("unknown"), None);
    }

    #[test]
    fn test_from_path() {
        assert_eq!(FileFormat::from_path(Path::new("test.ans")), Some(FileFormat::Ansi));
        assert_eq!(FileFormat::from_path(Path::new("/path/to/file.xb")), Some(FileFormat::XBin));
        assert_eq!(FileFormat::from_path(Path::new("noext")), None);
    }

    #[test]
    fn test_uses_parser() {
        assert!(FileFormat::Ansi.uses_parser());
        assert!(FileFormat::Avatar.uses_parser());
        assert!(!FileFormat::XBin.uses_parser());
        assert!(!FileFormat::IcyDraw.uses_parser());
    }

    #[test]
    fn test_supports_save() {
        assert!(FileFormat::Ansi.supports_save());
        assert!(FileFormat::XBin.supports_save());
        // Animation format might not support save
    }

    #[test]
    fn test_is_animated() {
        assert!(FileFormat::IcyAnim.is_animated());
        assert!(!FileFormat::Ansi.is_animated());
    }

    #[test]
    fn test_all_extensions_contain_primary() {
        for format in FileFormat::ALL {
            let exts = format.all_extensions();
            let primary = format.primary_extension();
            assert!(
                exts.contains(&primary),
                "Format {:?} primary extension '{}' not in all_extensions {:?}",
                format,
                primary,
                exts
            );
        }
    }

    #[test]
    fn test_buffer_type_compatibility() {
        // CP437 formats
        assert!(FileFormat::Ansi.is_compatible_with(BufferType::CP437));
        assert!(FileFormat::XBin.is_compatible_with(BufferType::CP437));
        assert!(!FileFormat::Petscii.is_compatible_with(BufferType::CP437));
        assert!(!FileFormat::ViewData.is_compatible_with(BufferType::CP437));

        // PETSCII format
        assert!(FileFormat::Petscii.is_compatible_with(BufferType::Petscii));
        assert!(!FileFormat::Ansi.is_compatible_with(BufferType::Petscii));

        // Viewdata format
        assert!(FileFormat::ViewData.is_compatible_with(BufferType::Viewdata));
        assert!(FileFormat::Mode7.is_compatible_with(BufferType::Viewdata));
        assert!(!FileFormat::Ansi.is_compatible_with(BufferType::Viewdata));

        // IcyDraw supports everything
        assert!(FileFormat::IcyDraw.is_compatible_with(BufferType::CP437));
        assert!(FileFormat::IcyDraw.is_compatible_with(BufferType::Petscii));
        assert!(FileFormat::IcyDraw.is_compatible_with(BufferType::Viewdata));
        assert!(FileFormat::IcyDraw.is_compatible_with(BufferType::Atascii));
    }

    #[test]
    fn test_save_formats_for_buffer_type() {
        let cp437_formats = FileFormat::save_formats_for_buffer_type(BufferType::CP437);
        assert!(cp437_formats.contains(&FileFormat::Ansi));
        assert!(cp437_formats.contains(&FileFormat::XBin));
        assert!(!cp437_formats.contains(&FileFormat::Petscii));

        let viewdata_formats = FileFormat::save_formats_for_buffer_type(BufferType::Viewdata);
        assert!(viewdata_formats.contains(&FileFormat::IcyDraw));
        assert!(!viewdata_formats.contains(&FileFormat::Ansi));
    }
}
