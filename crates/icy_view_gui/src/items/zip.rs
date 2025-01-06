use super::{Item, ItemType};
use std::{io::Read, path::PathBuf};

pub struct ZipFile {
    pub item: Box<dyn Item>,
}

impl ZipFile {
    pub fn new(item: Box<dyn Item>) -> Self {
        Self { item }
    }
}

impl Item for ZipFile {
    fn get_label(&self) -> String {
        self.item.get_label()
    }

    fn get_file_path(&self) -> PathBuf {
        self.item.get_file_path()
    }
    fn item_type(&self) -> ItemType {
        ItemType::Folder
    }

    fn get_icon(&self) -> Option<char> {
        Some('📦')
    }

    fn get_subitems(&self) -> Option<Vec<Box<dyn Item>>> {
        let mut files: Vec<Box<dyn Item>> = Vec::new();
        match self.item.read_data() {
            Some(file) => match zip::ZipArchive::new(std::io::Cursor::new(file)) {
                Ok(mut archive) => {
                    for i in 0..archive.len() {
                        match archive.by_index(i) {
                            Ok(mut file) => {
                                let mut data = Vec::new();
                                file.read_to_end(&mut data).unwrap_or_default();
                                let entry = ZipItem::new(file.enclosed_name().unwrap_or(PathBuf::from("unknown")).to_path_buf(), data);
                                if entry.path.extension().unwrap_or_default().to_ascii_lowercase() == "zip" {
                                    files.push(Box::new(ZipFile::new(Box::new(entry))));
                                } else {
                                    files.push(Box::new(entry));
                                }
                            }
                            Err(err) => {
                                log::error!("Error reading zip file: {}", err);
                            }
                        }
                    }
                }
                Err(err) => {
                    log::error!("Failed to open zip file: {}", err);
                }
            },
            None => {
                log::error!("Error reading zip file: {:?}", self.get_file_path());
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
