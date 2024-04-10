//use rs_sha256::Sha256Hasher;
use std::{
    fs,
    //  hash::Hasher,
    path::{self, Path, PathBuf},
};

use icy_engine::get_crc32;

use crate::Settings;

pub fn get_autosave_file(path: &Path) -> PathBuf {
    let auto_save_directory = Settings::get_auto_save_diretory().unwrap();
    //    let mut sha256hasher = Sha256Hasher::default();
    //    sha256hasher.write(path.as_os_str().to_str().unwrap().as_bytes());
    //    let u64result = sha256hasher.finish();

    // crc32 should be enough -if not the alternative rs_sha256 is available
    let key = get_crc32(path.as_os_str().to_string_lossy().as_bytes());
    auto_save_directory.join(path::Path::new(format!("{:x}.sav", key).as_str()))
}

pub fn remove_autosave(path: &Path) {
    let file = get_autosave_file(path);
    if file.exists() {
        if let Err(err) = fs::remove_file(file) {
            log::error!("Failed to remove autosave file: {}", err);
        }
    }
}

pub fn store_auto_save(path: &Path, data: &[u8]) {
    let auto_save = get_autosave_file(path);
    if let Err(err) = fs::write(auto_save, data) {
        log::error!("Failed to save autosave file: {}", err);
    }
}
