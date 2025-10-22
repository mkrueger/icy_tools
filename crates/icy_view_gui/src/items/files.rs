use std::{
    fs,
    path::{Path, PathBuf},
};

use super::{Item, ItemType, SixteenFolder, sort_folder, zip::ZipFile};

pub struct ItemFolder {
    pub path: PathBuf,
    pub include_16colors: bool,
}

impl ItemFolder {
    pub fn new(path: PathBuf) -> Self {
        Self { path, include_16colors: false }
    }
}

impl Item for ItemFolder {
    fn get_label(&self) -> String {
        get_file_name(&self.path).to_string()
    }

    fn get_file_path(&self) -> PathBuf {
        self.path.clone()
    }
    fn item_type(&self) -> ItemType {
        ItemType::Folder
    }

    fn get_subitems(&mut self) -> Option<Vec<Box<dyn Item>>> {
        Some(match read_folder(&self.path) {
            Ok(mut items) => {
                if self.include_16colors {
                    items.insert(0, Box::new(SixteenFolder::new()));
                }
                items
            }
            Err(err) => {
                log::error!("Failed to read folder: {:?}", err);
                Vec::new()
            }
        })
    }
}
pub struct ItemFile {
    item_type: ItemType,
    path: PathBuf,
    data: Option<Vec<u8>>,
}

impl ItemFile {
    pub fn new(path: PathBuf) -> Self {
        Self {
            item_type: ItemType::get_type(&path),
            path,
            data: None,
        }
    }
}

impl Item for ItemFile {
    fn item_type(&self) -> ItemType {
        self.item_type
    }
    fn get_label(&self) -> String {
        get_file_name(&self.path).to_string()
    }

    fn get_file_path(&self) -> PathBuf {
        self.path.clone()
    }

    fn read_data(&mut self) -> Option<Vec<u8>> {
        if let Some(data) = &self.data {
            return Some(data.clone());
        }
        if let Ok(file) = fs::read(&self.path) {
            self.data = Some(file.clone());
            return Some(file);
        }
        None
    }
}

#[cfg(windows)]
unsafe extern "C" {
    pub fn GetLogicalDrives() -> u32;
}

#[cfg(windows)]
fn get_drives() -> Vec<PathBuf> {
    let mut drive_names = Vec::new();
    let mut drives = unsafe { GetLogicalDrives() };
    let mut letter = b'A';
    while drives > 0 {
        if drives & 1 != 0 {
            drive_names.push(format!("{}:\\", letter as char).into());
        }
        drives >>= 1;
        letter += 1;
    }
    drive_names
}

#[cfg(windows)]
fn is_drive_root(path: &Path) -> bool {
    path.to_str()
        .filter(|path| &path[1..] == ":\\")
        .and_then(|path| path.chars().next())
        .map_or(false, |ch| ch.is_ascii_uppercase())
}

pub fn get_file_name(path: &Path) -> &str {
    #[cfg(windows)]
    if path.is_dir() && is_drive_root(path) {
        return path.to_str().unwrap_or_default();
    }
    path.file_name().and_then(|name| name.to_str()).unwrap_or_default()
}

fn read_folder(path: &Path) -> Result<Vec<Box<dyn Item>>, std::io::Error> {
    fs::read_dir(path).map(|entries| {
        let mut directories: Vec<Box<dyn Item>> = Vec::new();
        let mut files: Vec<Box<dyn Item>> = Vec::new();
        for entry in entries.filter_map(|result| result.ok()) {
            let path = entry.path();
            if path.is_dir() {
                directories.push(Box::new(ItemFolder::new(path)));
            } else {
                if path.extension().unwrap_or_default().to_ascii_lowercase() == "zip" {
                    files.push(Box::new(ZipFile::new(Box::new(ItemFile::new(path)))));
                } else {
                    files.push(Box::new(ItemFile::new(path)));
                }
            }
        }
        sort_folder(&mut directories);
        sort_folder(&mut files);

        #[cfg(windows)]
        {
            let drives = get_drives();
            let mut infos: Vec<Box<dyn Item>> = Vec::with_capacity(drives.len() + directories.len());
            for drive in drives {
                infos.push(Box::new(ItemFolder::new(drive)));
            }
            infos.append(&mut directories);
            directories = infos;
        }
        directories.append(&mut files);
        directories
    })
}
