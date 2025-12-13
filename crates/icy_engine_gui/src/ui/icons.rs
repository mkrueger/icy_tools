use iced::{Length, Theme, widget::svg};
use icy_engine::formats::FileFormat;

// Status icons
const WARNING_SVG: &[u8] = include_bytes!("icons/warning.svg");
const ERROR_SVG: &[u8] = include_bytes!("icons/error.svg");
const INFO_SVG: &[u8] = include_bytes!("icons/info.svg");

// UI icons
const SETTINGS_SVG: &[u8] = include_bytes!("icons/settings.svg");

// File type icons
const FILE_TEXT_SVG: &[u8] = include_bytes!("icons/files/file_text.svg");
const FILE_IMAGE_SVG: &[u8] = include_bytes!("icons/files/file_image.svg");
const FILE_MUSIC_SVG: &[u8] = include_bytes!("icons/files/file_music.svg");
const FILE_MOVIE_SVG: &[u8] = include_bytes!("icons/files/file_movie.svg");
const FILE_FOLDER_SVG: &[u8] = include_bytes!("icons/files/file_folder.svg");
const FILE_GENERIC_SVG: &[u8] = include_bytes!("icons/files/file_generic.svg");
const FILE_ANSI_SVG: &[u8] = include_bytes!("icons/files/file_ansi.svg");
const FILE_BINARY_SVG: &[u8] = include_bytes!("icons/files/file_binary.svg");
const FILE_TERMINAL_SVG: &[u8] = include_bytes!("icons/files/file_terminal.svg");
const FILE_GRAPHICS_SVG: &[u8] = include_bytes!("icons/files/file_graphics.svg");
const FILE_GAME_SVG: &[u8] = include_bytes!("icons/files/file_game.svg");
const FILE_NATIVE_SVG: &[u8] = include_bytes!("icons/files/file_native.svg");

// Folder icons
const FOLDER_OPEN_SVG: &[u8] = include_bytes!("icons/files/folder_open.svg");
const FOLDER_PARENT_SVG: &[u8] = include_bytes!("icons/files/folder_parent.svg");
const FOLDER_ZIP_SVG: &[u8] = include_bytes!("icons/files/folder_zip.svg");
const FOLDER_DATA_SVG: &[u8] = include_bytes!("icons/files/folder_data.svg");

pub fn warning_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(WARNING_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

pub fn error_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(ERROR_SVG)).width(Length::Fixed(size)).height(Length::Fixed(size))
}

pub fn info_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(INFO_SVG)).width(Length::Fixed(size)).height(Length::Fixed(size))
}

pub fn settings_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(SETTINGS_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

// File type icons
pub fn file_text_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(FILE_TEXT_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

pub fn file_image_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(FILE_IMAGE_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

pub fn file_music_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(FILE_MUSIC_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

pub fn file_movie_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(FILE_MOVIE_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

pub fn file_folder_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(FILE_FOLDER_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

pub fn file_generic_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(FILE_GENERIC_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

pub fn file_ansi_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(FILE_ANSI_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

pub fn file_binary_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(FILE_BINARY_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

pub fn file_terminal_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(FILE_TERMINAL_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

pub fn file_graphics_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(FILE_GRAPHICS_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

pub fn file_game_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(FILE_GAME_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

pub fn file_native_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(FILE_NATIVE_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

// Folder icons
pub fn folder_open_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(FOLDER_OPEN_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

pub fn folder_parent_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(FOLDER_PARENT_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

pub fn folder_zip_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(FOLDER_ZIP_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

pub fn folder_data_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(FOLDER_DATA_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

/// Icon type for file format representation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileIcon {
    /// ANSI art file (.ans, .diz, .nfo, .ice)
    Ansi,
    /// Plain text file (.txt, .asc)
    Text,
    /// Binary format file (Bin, XBin, IceDraw, TundraDraw, Artworx)
    Binary,
    /// Terminal format file (Avatar, PCBoard, CtrlA, Renegade)
    Terminal,
    /// Retro/legacy format (Petscii, Atascii, ViewData, Mode7, Vt52)
    Retro,
    /// Graphics format (RIP, SkyPix)
    Graphics,
    /// Game-related format (IGS - Atari ST)
    Game,
    /// Native IcyDraw format (.icy, .icyanim)
    Native,
    /// Image file (PNG, GIF, etc.)
    Image,
    /// Music file (AMS, MUS)
    Music,
    /// Animation/movie file (IcyAnim)
    Movie,
    /// Archive file (ZIP, RAR, ARJ, etc.)
    Archive,
    /// Generic folder
    Folder,
    /// Open folder
    FolderOpen,
    /// Parent folder (..)
    FolderParent,
    /// Data folder
    FolderData,
    /// Unknown/generic file
    Unknown,
}

impl FileIcon {
    /// Get the icon for a FileFormat
    pub fn from_format(format: &FileFormat) -> Self {
        match format {
            // Archive formats
            FileFormat::Archive(_) => FileIcon::Archive,

            // Image formats
            FileFormat::Image(_) => FileIcon::Image,

            // Animation format
            FileFormat::IcyAnim => FileIcon::Movie,

            // Music formats
            FileFormat::AnsiMusic => FileIcon::Music,

            // ANSI art (most common)
            FileFormat::Ansi => FileIcon::Ansi,

            // Plain ASCII text
            FileFormat::Ascii => FileIcon::Text,

            // Terminal/BBS formats
            FileFormat::Avatar | FileFormat::PCBoard | FileFormat::CtrlA | FileFormat::Renegade => FileIcon::Terminal,

            // Retro/legacy formats
            FileFormat::Petscii | FileFormat::Atascii | FileFormat::ViewData | FileFormat::Mode7 | FileFormat::Vt52 => FileIcon::Retro,

            // Graphics formats
            FileFormat::Rip | FileFormat::SkyPix => FileIcon::Graphics,

            // Game-related (Atari ST IGS)
            FileFormat::Igs => FileIcon::Game,

            // Native IcyDraw format
            FileFormat::IcyDraw => FileIcon::Native,

            // Binary formats
            FileFormat::IceDraw | FileFormat::Bin | FileFormat::XBin | FileFormat::TundraDraw | FileFormat::Artworx => FileIcon::Binary,

            // Font formats
            FileFormat::BitFont(_) | FileFormat::CharacterFont(_) => FileIcon::Binary,

            // Palette formats (mostly text-based)
            FileFormat::Palette(_) => FileIcon::Text,
        }
    }

    /// Create an SVG icon widget for this file icon
    pub fn to_svg<'a>(self, size: f32) -> svg::Svg<'a, Theme> {
        match self {
            FileIcon::Ansi => file_ansi_icon(size),
            FileIcon::Text => file_text_icon(size),
            FileIcon::Binary => file_binary_icon(size),
            FileIcon::Terminal => file_terminal_icon(size),
            FileIcon::Retro => file_terminal_icon(size), // Reuse terminal icon for retro
            FileIcon::Graphics => file_graphics_icon(size),
            FileIcon::Game => file_game_icon(size),
            FileIcon::Native => file_native_icon(size),
            FileIcon::Image => file_image_icon(size),
            FileIcon::Music => file_music_icon(size),
            FileIcon::Movie => file_movie_icon(size),
            FileIcon::Archive => folder_zip_icon(size),
            FileIcon::Folder => file_folder_icon(size),
            FileIcon::FolderOpen => folder_open_icon(size),
            FileIcon::FolderParent => folder_parent_icon(size),
            FileIcon::FolderData => folder_data_icon(size),
            FileIcon::Unknown => file_generic_icon(size),
        }
    }
}

/// Get an SVG icon for a FileFormat
pub fn get_format_icon<'a>(format: &FileFormat, size: f32) -> svg::Svg<'a, Theme> {
    FileIcon::from_format(format).to_svg(size)
}
