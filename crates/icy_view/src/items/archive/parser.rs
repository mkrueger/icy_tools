use std::{collections::HashMap, collections::HashSet, io::Cursor};
use tokio_util::sync::CancellationToken;
use unarc_rs::unified::{ArchiveFormat, UnifiedArchive};

/// Parse an archive and extract all files/directories
pub fn parse_archive(data: Vec<u8>, format: ArchiveFormat, cancel_token: CancellationToken) -> Option<(HashMap<String, Vec<u8>>, HashSet<String>)> {
    let cursor = Cursor::new(data);
    let mut archive = UnifiedArchive::open_with_format(cursor, format).ok()?;

    let mut all_files: HashMap<String, Vec<u8>> = HashMap::new();
    let mut directories: HashSet<String> = HashSet::new();

    while let Ok(Some(entry)) = archive.next_entry() {
        if cancel_token.is_cancelled() {
            return None;
        }
        let name = entry.name().to_string();
        // Normalize path separators
        let name = name.replace('\\', "/");

        // Check if it's a directory (ends with /)
        if name.ends_with('/') {
            let dir_name = name.trim_end_matches('/').to_string();
            if !dir_name.is_empty() {
                directories.insert(dir_name);
            }
            continue;
        }

        // Read the file data
        if let Ok(data) = archive.read(&entry) {
            all_files.insert(name.clone(), data);

            // Register parent directories
            let mut path = name.as_str();
            while let Some(pos) = path.rfind('/') {
                let parent = &path[..pos];
                if !parent.is_empty() {
                    directories.insert(parent.to_string());
                }
                path = parent;
            }
        }
    }

    Some((all_files, directories))
}
