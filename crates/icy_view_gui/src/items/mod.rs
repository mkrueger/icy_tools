use std::path::{Path, PathBuf};

use crate::EXT_MUSIC_LIST;
use icy_sauce::SauceInformation;

use super::{EXT_IMAGE_LIST, EXT_WHITE_LIST};
mod files;
mod zip;
pub use files::*;
mod sixteencolors;
pub use sixteencolors::*;

pub trait Item {
    fn item_type(&self) -> ItemType;
    fn get_label(&self) -> String;
    fn get_file_path(&self) -> PathBuf;

    fn is_virtual_file(&self) -> bool {
        false
    }

    fn get_icon(&self) -> Option<char> {
        self.item_type().get_icon()
    }

    fn get_subitems(&mut self) -> Option<Vec<Box<dyn Item>>> {
        None
    }

    fn get_sauce(&mut self) -> Option<SauceInformation> {
        if let Some(file) = self.read_data() {
            if let Ok(info) = SauceInformation::read(&file) {
                return info;
            }
        }
        None
    }

    fn read_data(&mut self) -> Option<Vec<u8>> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ItemType {
    Unknown,
    Folder,
    Ansi,
    AnsiMusic,
    IcyAnimation,
    Rip,
    Picture,
    IGS,
}

impl ItemType {
    pub fn get_type(path: &Path) -> Self {
        if path.is_dir() {
            ItemType::Folder
        } else {
            let ext = if let Some(ext) = path.extension() {
                let ext2 = ext.to_ascii_lowercase();
                ext2.to_str().unwrap_or_default().to_string()
            } else {
                String::new()
            };

            if EXT_MUSIC_LIST.contains(&ext.as_str()) {
                ItemType::AnsiMusic
            } else if EXT_WHITE_LIST.contains(&ext.as_str())
                || icy_engine::FORMATS.iter().any(|f| {
                    let e = ext.as_str().to_ascii_lowercase();
                    f.get_file_extension() == e || f.get_alt_extensions().contains(&e)
                })
            {
                ItemType::Ansi
            } else if EXT_IMAGE_LIST.contains(&ext.as_str()) {
                ItemType::Picture
            } else if ext == "icyanim" {
                ItemType::IcyAnimation
            } else if ext == "rip" {
                ItemType::Rip
            } else if ext == "ig" {
                ItemType::IGS
            } else {
                ItemType::Unknown
            }
        }
    }
    fn get_icon(&self) -> Option<char> {
        match self {
            ItemType::Folder => Some('🗁'),
            ItemType::IcyAnimation => Some('🎥'),
            ItemType::Ansi | ItemType::Rip => Some('🖹'),
            ItemType::AnsiMusic => Some('🎵'),
            ItemType::Picture => Some('🖻'),
            ItemType::IGS => Some('🕹'),
            _ => Some('🗋'),
        }
    }
}

impl dyn Item {
    pub fn is_folder(&self) -> bool {
        self.item_type() == ItemType::Folder
    }

    pub fn is_binary(&mut self) -> bool {
        if let Some(data) = self.read_data() {
            for i in data.iter().take(500) {
                if i == &0 || i == &255 {
                    return true;
                }
            }
            false
        } else {
            true
        }
    }
}

pub fn sort_folder(directories: &mut Vec<Box<dyn Item>>) {
    directories.sort_by(|a, b| a.get_label().to_lowercase().cmp(&b.get_label().to_lowercase()));
}
