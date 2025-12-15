use async_trait::async_trait;
use icy_engine::formats::FileFormat;
use std::{collections::HashMap, collections::HashSet, path::PathBuf};
use tokio_util::sync::CancellationToken;

use super::{ArchiveContainer, ArchiveItem};
use crate::items::{FileIcon, Item, ItemError, sort_folder};
use crate::thumbnail::{RgbaData, scale_to_thumbnail_width};

/// A folder inside an archive
#[derive(Clone)]
pub struct ArchiveFolder {
    /// Name of this folder (just the folder name, not full path)
    name: String,
    /// Full path within the archive (e.g., "folder/subfolder")
    folder_path: String,
    /// Path to the archive file itself
    archive_path: String,
    /// All files in the archive: path -> data
    all_files: HashMap<String, Vec<u8>>,
    /// All directories in the archive
    all_directories: HashSet<String>,
}

impl ArchiveFolder {
    pub fn new(folder_path: String, archive_path: String, all_files: HashMap<String, Vec<u8>>, all_directories: HashSet<String>) -> Self {
        let name = folder_path.rsplit('/').next().unwrap_or(&folder_path).to_string();
        Self {
            name,
            folder_path,
            archive_path,
            all_files,
            all_directories,
        }
    }
}

#[async_trait]
impl Item for ArchiveFolder {
    fn get_label(&self) -> String {
        self.name.clone()
    }

    fn get_file_path(&self) -> String {
        // Return just the folder name for navigation
        self.name.clone()
    }

    fn is_container(&self) -> bool {
        true
    }

    fn get_sync_thumbnail(&self) -> Option<RgbaData> {
        Some(scale_to_thumbnail_width(crate::items::create_folder_placeholder()))
    }

    fn get_file_icon(&self) -> FileIcon {
        FileIcon::Folder
    }

    async fn get_thumbnail_preview(&self, _cancel_token: &CancellationToken) -> Option<RgbaData> {
        // Return None - use get_sync_thumbnail() instead
        None
    }

    async fn get_subitems(&self, _cancel_token: &CancellationToken) -> Result<Vec<Box<dyn Item>>, ItemError> {
        let mut items: Vec<Box<dyn Item>> = Vec::new();
        let prefix = format!("{}/", self.folder_path);

        // Collect direct children (files and folders)
        let mut child_dirs: HashSet<String> = HashSet::new();
        let mut child_files: HashSet<String> = HashSet::new();

        // Find child directories
        for dir in &self.all_directories {
            if dir.starts_with(&prefix) {
                let rest = &dir[prefix.len()..];
                // Only direct children (no more slashes)
                if !rest.is_empty() && !rest.contains('/') {
                    child_dirs.insert(rest.to_string());
                }
            }
        }

        // Find child files
        for file_path in self.all_files.keys() {
            if file_path.starts_with(&prefix) {
                let rest = &file_path[prefix.len()..];
                // Only direct children (no more slashes)
                if !rest.is_empty() && !rest.contains('/') {
                    child_files.insert(rest.to_string());
                }
            }
        }

        // Create folder items
        for dir_name in child_dirs {
            let child_path = format!("{}/{}", self.folder_path, dir_name);
            let folder = ArchiveFolder::new(child_path, self.archive_path.clone(), self.all_files.clone(), self.all_directories.clone());
            items.push(Box::new(folder));
        }

        // Create file items
        for file_name in child_files {
            let file_path = format!("{}/{}", self.folder_path, file_name);
            if let Some(data) = self.all_files.get(&file_path) {
                let entry = ArchiveItem::new(PathBuf::from(&file_name), data.clone());

                // Check if it's a nested archive - use from_path to properly extract extension
                let name_path = PathBuf::from(&file_name);
                if let Some(FileFormat::Archive(nested_format)) = FileFormat::from_path(&name_path) {
                    items.push(Box::new(ArchiveContainer::new(Box::new(entry), nested_format)));
                } else {
                    items.push(Box::new(entry));
                }
            }
        }

        sort_folder(&mut items);
        Ok(items)
    }

    fn clone_box(&self) -> Box<dyn Item> {
        Box::new(self.clone())
    }
}
