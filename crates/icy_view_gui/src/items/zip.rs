use std::{fs, io::Read, path::PathBuf};

use super::{get_file_name, Item, ItemType};


pub struct ZipFile {
    pub path: PathBuf,
}

impl ZipFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Item for ZipFile {
    fn get_label(&self) -> String {
        get_file_name(&self.path).to_string()
    }

    fn get_file_path(&self) -> PathBuf {
        self.path.clone()
    }
    fn item_type(&self) -> ItemType {
        ItemType::Folder
    }

    fn get_icon(&self) -> Option<char> {
        Some('🗃')
    }

    fn get_subitems(&self) -> Option<Vec<Box<dyn Item>>> {
        let mut files: Vec<Box<dyn Item>> = Vec::new();
        match fs::File::open(&self.path) {
            Ok(file) => match zip::ZipArchive::new(file) {
                Ok(mut archive) => {
                    for i in 0..archive.len() {
                        match archive.by_index(i) {
                            Ok(mut file) => {
                                let mut data = Vec::new();
                                file.read_to_end(&mut data).unwrap_or_default();

                                let entry = ZipItem::new(file.enclosed_name().unwrap_or(PathBuf::from("unknown")).to_path_buf(), data);
                                files.push(Box::new(entry));
                            }
                            Err(err) => {
                                log::error!("Error reading zip file: {}", err);
                            }
                        }
                    }
                }
                Err(err) => {
                    log::error!("Error reading zip archive: {}", err);
                }
            },
            Err(err) => {
                log::error!("Failed to open zip file: {}", err);
            }
        }
        Some(files)
    }
}

#[derive(Clone)]
pub struct ZipItem {
    item_type: ItemType,
    path: PathBuf,
    data: Vec<u8>,
}

impl ZipItem {
    pub fn new(path: PathBuf, data: Vec<u8>) -> Self {
        Self {
            item_type: ItemType::get_type(&path),
            path,
            data,
        }
    }
}
impl Item for ZipItem {
    fn get_label(&self) -> String {
        self.path.to_str().unwrap_or_default().to_string()
    }

    fn get_file_path(&self) -> PathBuf {
        self.path.clone()
    }
    fn item_type(&self) -> ItemType {
        self.item_type
    }
    fn read_data(&self) -> Option<Vec<u8>> {
        Some(self.data.clone())
    }
}