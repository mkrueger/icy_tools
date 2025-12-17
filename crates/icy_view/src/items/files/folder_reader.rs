use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

use icy_engine::formats::FileFormat;
use unarc_rs::unified::{ArchiveFormat, UnifiedArchive};

use super::{ItemFile, ItemFolder};
use crate::items::{
    Item,
    archive::{ArchiveContainer, ArchiveFolder, ArchiveItem},
    sort_folder,
};

#[cfg(windows)]
use super::get_drives;

#[cfg(windows)]
fn is_windows_drive_root(path: &Path) -> bool {
    // We normalize many paths to use '/' elsewhere; accept both separators here.
    let s = path.to_string_lossy();
    let b = s.as_bytes();
    b.len() == 3 && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/') && (b[0] as char).is_ascii_alphabetic()
}

/// Read contents of a filesystem folder
pub fn read_folder(path: &Path) -> Result<Vec<Box<dyn Item>>, std::io::Error> {
    fs::read_dir(path).map(|entries| {
        let mut directories: Vec<Box<dyn Item>> = Vec::new();
        let mut files: Vec<Box<dyn Item>> = Vec::new();
        for entry in entries.filter_map(|result| result.ok()) {
            let path = entry.path();

            // Get file type - skip sockets, pipes, devices etc.
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue, // Skip if we can't determine type
            };

            if file_type.is_dir() {
                directories.push(Box::new(ItemFolder::new(path.to_string_lossy().replace('\\', "/"))));
            } else if file_type.is_file() {
                // Only handle regular files (not sockets, devices, pipes, etc.)
                // Use from_path which extracts the extension properly
                if let Some(FileFormat::Archive(format)) = FileFormat::from_path(&path) {
                    files.push(Box::new(ArchiveContainer::new(
                        Box::new(ItemFile::new(path.to_string_lossy().replace('\\', "/"))),
                        format,
                    )));
                } else {
                    files.push(Box::new(ItemFile::new(path.to_string_lossy().replace('\\', "/"))));
                }
            }
            // Skip symlinks, sockets, pipes, devices, etc.
        }
        sort_folder(&mut directories);
        sort_folder(&mut files);

        #[cfg(windows)]
        {
            // Only show drives at the filesystem root (e.g. "C:/").
            if is_windows_drive_root(path) {
                let drives = get_drives();
                let mut infos: Vec<Box<dyn Item>> = Vec::with_capacity(drives.len() + directories.len());
                for drive in drives {
                    infos.push(Box::new(ItemFolder::new(drive.to_string_lossy().replace('\\', "/"))));
                }
                infos.append(&mut directories);
                directories = infos;
            }
        }
        directories.append(&mut files);
        directories
    })
}

/// Get items at a given path
///
/// This function resolves paths that may go through archive files.
/// For example: `/home/user/archive.zip/folder/file.ans`
///
/// Algorithm:
/// - Walk the path segments from root
/// - If segment is a directory, enter it
/// - If segment is an archive file, open it and continue inside
/// - If segment is a file at the end, return error (can't list a file)
pub fn get_items_at_path(path: &str) -> Option<Vec<Box<dyn Item>>> {
    let path = PathBuf::from(path);

    // First, find where the filesystem path ends and archive path begins
    let mut fs_path = PathBuf::new();
    let mut archive_path: Option<String> = None;
    let mut archive_format: Option<ArchiveFormat> = None;

    for component in path.components() {
        if archive_path.is_some() {
            // We're inside an archive, append to archive_path
            let ap = archive_path.as_mut().unwrap();
            if !ap.is_empty() {
                ap.push('/');
            }
            ap.push_str(&component.as_os_str().to_string_lossy());
        } else {
            // Still on filesystem
            fs_path.push(component);

            // Check if this is an archive file
            if fs_path.is_file() {
                if let Some(FileFormat::Archive(format)) = FileFormat::from_path(&fs_path) {
                    archive_path = Some(String::new());
                    archive_format = Some(format);
                }
            }
        }
    }

    if let (Some(archive_internal_path), Some(format)) = (archive_path, archive_format) {
        // We need to read inside an archive file
        read_archive_folder(&fs_path, &archive_internal_path, format)
    } else if fs_path.is_dir() {
        // Regular filesystem directory
        match read_folder(&fs_path) {
            Ok(items) => Some(items),
            Err(err) => {
                log::error!("Failed to read folder {:?}: {:?}", fs_path, err);
                None
            }
        }
    } else {
        // Path doesn't exist or is a file
        None
    }
}

/// Read contents of a folder inside an archive file
fn read_archive_folder(archive_path: &Path, internal_path: &str, format: ArchiveFormat) -> Option<Vec<Box<dyn Item>>> {
    let archive_data = fs::read(archive_path).ok()?;
    let cursor = Cursor::new(&archive_data);
    let mut archive = UnifiedArchive::open_with_format(cursor, format).ok()?;

    let mut all_files: HashMap<String, Vec<u8>> = HashMap::new();
    let mut directories: HashSet<String> = HashSet::new();

    // Collect all files and identify directories
    while let Ok(Some(entry)) = archive.next_entry() {
        let name = entry.name().to_string();
        let name = name.replace('\\', "/");

        if name.ends_with('/') {
            let dir_name = name.trim_end_matches('/').to_string();
            if !dir_name.is_empty() {
                directories.insert(dir_name);
            }
        } else {
            if let Ok(data) = archive.read(&entry) {
                all_files.insert(name.clone(), data);

                // Register parent directories
                let mut path_str = name.as_str();
                while let Some(pos) = path_str.rfind('/') {
                    let parent = &path_str[..pos];
                    if !parent.is_empty() {
                        directories.insert(parent.to_string());
                    }
                    path_str = parent;
                }
            }
        }
    }

    // Now find items at the requested internal path
    let normalized_path = internal_path.trim_start_matches('/').trim_end_matches('/');
    let prefix = if normalized_path.is_empty() {
        String::new()
    } else {
        format!("{}/", normalized_path)
    };

    let mut items: Vec<Box<dyn Item>> = Vec::new();
    let mut child_dirs: HashSet<String> = HashSet::new();
    let mut child_files: HashSet<String> = HashSet::new();

    if normalized_path.is_empty() {
        // Root level
        for dir in &directories {
            if !dir.contains('/') {
                child_dirs.insert(dir.clone());
            }
        }
        for file_path in all_files.keys() {
            if !file_path.contains('/') {
                child_files.insert(file_path.clone());
            }
        }
    } else {
        // Subdirectory
        for dir in &directories {
            if dir.starts_with(&prefix) {
                let rest = &dir[prefix.len()..];
                if !rest.is_empty() && !rest.contains('/') {
                    child_dirs.insert(rest.to_string());
                }
            }
        }
        for file_path in all_files.keys() {
            if file_path.starts_with(&prefix) {
                let rest = &file_path[prefix.len()..];
                if !rest.is_empty() && !rest.contains('/') {
                    child_files.insert(rest.to_string());
                }
            }
        }
    }

    // Create folder items
    for dir_name in child_dirs {
        let full_internal_path = if normalized_path.is_empty() {
            dir_name.clone()
        } else {
            format!("{}/{}", normalized_path, dir_name)
        };

        let folder = ArchiveFolder::new(
            full_internal_path,
            archive_path.to_string_lossy().replace('\\', "/"),
            all_files.clone(),
            directories.clone(),
        );
        items.push(Box::new(folder));
    }

    // Create file items
    for file_name in child_files {
        let full_internal_path = if normalized_path.is_empty() {
            file_name.clone()
        } else {
            format!("{}/{}", normalized_path, file_name)
        };

        if let Some(data) = all_files.get(&full_internal_path) {
            let entry = ArchiveItem::new(PathBuf::from(&file_name), data.clone());

            // Check if it's a nested archive - use from_path to properly extract extension
            let file_path = PathBuf::from(&file_name);
            if let Some(FileFormat::Archive(nested_format)) = FileFormat::from_path(&file_path) {
                items.push(Box::new(ArchiveContainer::new(Box::new(entry), nested_format)));
            } else {
                items.push(Box::new(entry));
            }
        }
    }

    sort_folder(&mut items);
    Some(items)
}
