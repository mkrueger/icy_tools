use std::{path::PathBuf, str::FromStr};

use i18n_embed_fl::fl;

use super::{sort_folder, zip::ZipFile, Item, ItemType};

pub struct SixteenFolder {}

impl SixteenFolder {
    pub fn new() -> Self {
        Self {}
    }
}

const MAIN_PATH: &str = "https://16colo.rs";
const API_PATH: &str = "http://api.16colo.rs/v0";

impl Item for SixteenFolder {
    fn get_label(&self) -> String {
        "https://16colo.rs".to_string()
    }

    fn get_file_path(&self) -> PathBuf {
        PathBuf::from_str(MAIN_PATH).unwrap()
    }

    fn item_type(&self) -> ItemType {
        ItemType::Folder
    }

    fn get_icon(&self) -> Option<char> {
        Some('🌐')
    }

    fn get_subitems(&mut self) -> Option<Vec<Box<dyn Item>>> {
        let mut result: Vec<Box<dyn Item>> = Vec::new();
        let url = format!("{}/year?rows=0", API_PATH);
        match reqwest::blocking::get(url) {
            Ok(response) => match response.json::<serde_json::Value>() {
                Ok(json) => {
                    let packs = json.as_array().unwrap();
                    for pack in packs {
                        let year = pack["year"].as_u64().unwrap();
                        let packs = pack["packs"].as_u64().unwrap();
                        result.push(Box::new(SixteenPack::new(year, packs)));
                    }
                    result.reverse();
                }
                Err(err) => {
                    log::error!("Error parsing json: {}", err);
                }
            },
            Err(err) => {
                log::error!("Failed to fetch 16colors data: {}", err);
            }
        }
        Some(result)
    }
}

struct SixteenPack {
    pub year: u64,
    pub packs: u64,
}

impl SixteenPack {
    pub fn new(year: u64, packs: u64) -> Self {
        Self { year, packs }
    }
}

impl Item for SixteenPack {
    fn get_label(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "label-sixteencolors_pack", year = self.year, packs = self.packs)
    }

    fn get_file_path(&self) -> PathBuf {
        format!("{}/year/{}?rows=0", API_PATH, self.year).into()
    }

    fn item_type(&self) -> ItemType {
        ItemType::Folder
    }

    fn get_icon(&self) -> Option<char> {
        Some('📦')
    }

    fn get_subitems(&mut self) -> Option<Vec<Box<dyn Item>>> {
        let mut result: Vec<Box<dyn Item>> = Vec::new();
        let url = format!("{}/year/{}?rows=0", API_PATH, self.year);
        match reqwest::blocking::get(url) {
            Ok(response) => match response.json::<serde_json::Value>() {
                Ok(json) => {
                    let packs = json.as_array().unwrap();
                    for pack in packs {
                        let filename = pack["filename"].as_str().unwrap_or_default().to_string();
                        let month = pack["month"].as_u64().unwrap();
                        let year = pack["year"].as_u64().unwrap();
                        let name = pack["name"].as_str().unwrap_or_default().to_string();
                        result.push(Box::new(SixteenFiles::new(filename, month, year, name)));
                    }
                    result.reverse();
                }
                Err(err) => {
                    log::error!("Error parsing json: {}", err);
                }
            },
            Err(err) => {
                log::error!("Failed to fetch 16colors data: {}", err);
            }
        }
        Some(result)
    }
}

pub struct SixteenFiles {
    pub filename: String,
    pub month: u64,
    pub year: u64,
    pub name: String,
}

impl SixteenFiles {
    pub fn new(filename: String, month: u64, year: u64, name: String) -> Self {
        Self { filename, month, year, name }
    }
}

impl Item for SixteenFiles {
    fn get_label(&self) -> String {
        self.name.clone()
    }

    fn get_file_path(&self) -> PathBuf {
        format!("{}/pack/{}?rows=0", API_PATH, self.name).into()
    }

    fn item_type(&self) -> ItemType {
        ItemType::Folder
    }

    fn get_icon(&self) -> Option<char> {
        Some('📁')
    }

    fn get_subitems(&mut self) -> Option<Vec<Box<dyn Item>>> {
        let mut result: Vec<Box<dyn Item>> = Vec::new();
        let url = format!("{}/pack/{}?rows=0", API_PATH, self.name);
        match reqwest::blocking::get(url) {
            Ok(response) => match response.json::<serde_json::Value>() {
                Ok(json) => {
                    let packs = json["files"].as_array().unwrap();
                    for pack in packs {
                        let filename = pack["filename"].as_str().unwrap().to_string();
                        let location = pack["file_location"].as_str().unwrap().to_string();
                        let uri = pack["uri"].as_str().unwrap().to_string();

                        if filename.to_ascii_lowercase().ends_with(".zip") {
                            result.push(Box::new(ZipFile::new(Box::new(SixteenFile::new(filename, location, uri)))));
                        } else {
                            result.push(Box::new(SixteenFile::new(filename, location, uri)));
                        }
                    }
                    sort_folder(&mut result);
                }
                Err(err) => {
                    log::error!("Error parsing json: {}", err);
                }
            },
            Err(err) => {
                log::error!("Failed to fetch 16colors data: {}", err);
            }
        }
        Some(result)
    }
}

struct SixteenFile {
    pub filename: String,
    pub location: String,
    pub uri: String,
    data: Option<Vec<u8>>,
}

impl SixteenFile {
    pub fn new(filename: String, location: String, uri: String) -> Self {
        Self {
            filename,
            location,
            uri,
            data: None,
        }
    }
}

impl Item for SixteenFile {
    fn get_label(&self) -> String {
        self.filename.clone()
    }

    fn get_file_path(&self) -> PathBuf {
        self.location.clone().into()
    }

    fn item_type(&self) -> ItemType {
        ItemType::get_type(&PathBuf::from_str(&self.filename).unwrap())
    }
    fn is_virtual_file(&self) -> bool {
        true
    }
    fn read_data(&mut self) -> Option<Vec<u8>> {
        if let Some(data) = &self.data {
            return Some(data.clone());
        }

        let url = format!("{}{}", MAIN_PATH, self.uri);
        match reqwest::blocking::get(url) {
            Ok(response) => {
                if let Ok(bytes) = response.bytes() {
                    self.data = Some(bytes.to_vec());
                    return Some(bytes.to_vec());
                }
            }
            Err(err) => {
                log::error!("Failed to fetch 16colors data: {}", err);
            }
        }

        None
    }
}
