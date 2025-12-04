use async_trait::async_trait;
use icy_engine::formats::FileFormat;
use std::{collections::HashSet, io::Cursor, path::PathBuf, sync::Arc};
use tokio_util::sync::CancellationToken;
use unarc_rs::unified::{ArchiveFormat, UnifiedArchive};

use super::{ArchiveFolder, ArchiveItem, parse_archive, render_diz_to_thumbnail};
use crate::items::{FileIcon, Item, sort_folder};
use crate::ui::thumbnail_view::{DIZ_NOT_FOUND_PLACEHOLDER, RgbaData};

/// An archive file (ZIP, RAR, ARJ, etc.)
pub struct ArchiveContainer {
    pub item: Arc<dyn Item>,
    pub format: ArchiveFormat,
}

impl ArchiveContainer {
    pub fn new(item: Box<dyn Item>, format: ArchiveFormat) -> Self {
        Self { item: Arc::from(item), format }
    }

    /// Extract FILE_ID.DIZ from the archive and render it as a thumbnail
    async fn extract_and_render_file_id_diz(&self, cancel_token: &CancellationToken) -> Option<RgbaData> {
        let file_data = self.item.read_data().await?;
        let format = self.format;
        let cancel_token = cancel_token.clone();

        // Archive extraction is CPU-bound, run in blocking thread
        tokio::task::spawn_blocking(move || {
            let cursor = Cursor::new(file_data);
            let mut archive = UnifiedArchive::open_with_format(cursor, format).ok()?;

            // Search for FILE_ID.DIZ (case-insensitive)
            while let Ok(Some(entry)) = archive.next_entry() {
                // Check for cancellation
                if cancel_token.is_cancelled() {
                    return None;
                }

                let name = entry.name();
                // Check if it's FILE_ID.DIZ at root level
                let file_name = name.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(name);
                if file_name.eq_ignore_ascii_case("file_id.ans") {
                    if let Ok(data) = archive.read(&entry) {
                        return render_diz_to_thumbnail(&data);
                    }
                }
                if file_name.eq_ignore_ascii_case("file_id.diz") {
                    if let Ok(data) = archive.read(&entry) {
                        return render_diz_to_thumbnail(&data);
                    }
                }
            }

            None
        })
        .await
        .ok()?
    }
}

#[async_trait]
impl Item for ArchiveContainer {
    fn get_label(&self) -> String {
        self.item.get_label()
    }

    fn get_file_path(&self) -> PathBuf {
        self.item.get_file_path()
    }

    fn is_container(&self) -> bool {
        true
    }

    fn get_file_icon(&self) -> FileIcon {
        FileIcon::Archive
    }

    async fn get_thumbnail_preview(&self, cancel_token: &CancellationToken) -> Option<RgbaData> {
        // Try to extract and render FILE_ID.DIZ
        if let Some(rgba) = self.extract_and_render_file_id_diz(cancel_token).await {
            return Some(rgba);
        }

        // No FILE_ID.DIZ found
        Some(DIZ_NOT_FOUND_PLACEHOLDER.clone())
    }

    async fn get_subitems(&self, cancel_token: &CancellationToken) -> Option<Vec<Box<dyn Item>>> {
        let file = self.item.read_data().await?;
        let archive_path = self.item.get_file_path();
        let format: ArchiveFormat = self.format;
        let cancel_token = cancel_token.clone();

        // Archive extraction is CPU-bound, run in blocking thread
        tokio::task::spawn_blocking(move || {
            // Check cancellation early
            if cancel_token.is_cancelled() {
                return None;
            }
            let mut files: Vec<Box<dyn Item>> = Vec::new();

            match parse_archive(file, format, cancel_token) {
                Some((all_files, directories)) => {
                    // Collect root level items
                    let mut root_items: HashSet<String> = HashSet::new();

                    // Add root directories
                    for dir in &directories {
                        if !dir.contains('/') {
                            root_items.insert(dir.clone());
                        }
                    }

                    // Add root files
                    for name in all_files.keys() {
                        if !name.contains('/') {
                            root_items.insert(name.clone());
                        }
                    }

                    // Create items
                    for name in root_items {
                        if directories.contains(&name) {
                            // It's a directory
                            let folder = ArchiveFolder::new(name.clone(), archive_path.clone(), all_files.clone(), directories.clone());
                            files.push(Box::new(folder));
                        } else if let Some(data) = all_files.get(&name) {
                            // It's a file
                            let entry = ArchiveItem::new(PathBuf::from(&name), data.clone());

                            // Check if it's a nested archive - use from_path to properly extract extension
                            let file_path = PathBuf::from(&name);
                            if let Some(FileFormat::Archive(nested_format)) = FileFormat::from_path(&file_path) {
                                files.push(Box::new(ArchiveContainer::new(Box::new(entry), nested_format)));
                            } else {
                                files.push(Box::new(entry));
                            }
                        }
                    }
                }
                None => {
                    log::error!("Failed to open archive file");
                }
            }
            sort_folder(&mut files);
            Some(files)
        })
        .await
        .ok()?
    }

    async fn read_data(&self) -> Option<Vec<u8>> {
        // Return the archive file data from the underlying item
        self.item.read_data().await
    }

    fn clone_box(&self) -> Box<dyn Item> {
        Box::new(ArchiveContainer {
            item: self.item.clone(),
            format: self.format,
        })
    }
}
