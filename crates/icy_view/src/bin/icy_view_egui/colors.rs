//! Colors taken from the original viewer so list, status bar and shuffle overlay match.

use eframe::egui::Color32;
use icy_engine::formats::FileFormat;

pub struct Sauce {
    pub title: Color32,
    pub author: Color32,
    pub group: Color32,
    pub date: Color32,
    pub size: Color32,
    pub separator: Color32,
    pub empty: Color32,
}

impl Sauce {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                title: Color32::from_rgb(230, 230, 153),
                author: Color32::from_rgb(153, 230, 153),
                group: Color32::from_rgb(153, 204, 230),
                date: Color32::from_rgb(179, 179, 179),
                size: Color32::from_rgb(204, 153, 204),
                separator: Color32::from_rgb(102, 102, 102),
                empty: Color32::from_rgb(100, 100, 100),
            }
        } else {
            Self {
                title: Color32::from_rgb(153, 128, 0),
                author: Color32::from_rgb(0, 128, 0),
                group: Color32::from_rgb(0, 102, 153),
                date: Color32::from_rgb(102, 102, 102),
                size: Color32::from_rgb(128, 51, 128),
                separator: Color32::from_rgb(153, 153, 153),
                empty: Color32::from_rgb(140, 140, 140),
            }
        }
    }
}

pub fn highlight(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(255, 220, 100)
    } else {
        Color32::from_rgb(200, 150, 0)
    }
}

pub fn file_name(dark: bool, label: &str, container: bool, default: Color32) -> Color32 {
    if container {
        return Color32::from_rgb(0x55, 0x55, 255);
    }
    match FileFormat::from_path(std::path::Path::new(label)) {
        Some(format) if format.is_image() => {
            if dark {
                Color32::from_rgb(0xFF, 0x55, 0xFF)
            } else {
                Color32::from_rgb(0xAA, 0x00, 0xAA)
            }
        }
        Some(format) if format.is_supported() => {
            if dark {
                Color32::from_rgb(0x55, 0xFF, 0x55)
            } else {
                Color32::from_rgb(0x00, 0xAA, 0x00)
            }
        }
        _ => default,
    }
}

pub fn star(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(255, 196, 61)
    } else {
        Color32::from_rgb(214, 140, 0)
    }
}

/// Filled stars for the rating, empty for none.
pub fn stars(rating: u8) -> String {
    "★".repeat(rating.min(5) as usize)
}
