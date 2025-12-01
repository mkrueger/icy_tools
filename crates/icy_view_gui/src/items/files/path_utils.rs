use icy_engine::formats::FileFormat;
use std::path::{Path, PathBuf};

#[cfg(windows)]
unsafe extern "C" {
    pub fn GetLogicalDrives() -> u32;
}

#[cfg(windows)]
pub fn get_drives() -> Vec<PathBuf> {
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
pub fn is_drive_root(path: &Path) -> bool {
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

/// Check if a path exists (either on filesystem or inside an archive)
pub fn path_exists(path: &str) -> bool {
    let path_buf = PathBuf::from(path);

    // Check for archive path
    let mut fs_path = PathBuf::new();
    let mut in_archive = false;

    for component in path_buf.components() {
        if in_archive {
            // We're inside an archive - assume it exists for now
            // (full validation would require reading the archive)
            return true;
        }

        fs_path.push(component);

        if fs_path.is_file() {
            if let Some(FileFormat::Archive(_)) = FileFormat::from_path(&fs_path) {
                in_archive = true;
            }
        }
    }

    fs_path.exists()
}

/// Check if a path is a directory (either on filesystem or inside an archive)
pub fn is_directory(path: &str) -> bool {
    let path_buf = PathBuf::from(path);

    // Check for archive path
    let mut fs_path = PathBuf::new();
    let mut in_archive = false;

    for component in path_buf.components() {
        if in_archive {
            // We're inside an archive - it's a virtual directory path
            return true;
        }

        fs_path.push(component);

        if fs_path.is_file() {
            if let Some(FileFormat::Archive(_)) = FileFormat::from_path(&fs_path) {
                in_archive = true;
            }
        }
    }

    fs_path.is_dir()
}

/// Get the parent path
/// Works for both filesystem paths and paths inside archives
pub fn get_parent_path(path: &str) -> Option<String> {
    let path_buf = PathBuf::from(path);
    path_buf.parent().map(|p| p.to_string_lossy().to_string())
}
