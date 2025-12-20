//! Tag replacement list management.
//!
//! Loads replacement lists from CSV files in the taglists directory.
//! Each line has format: example,tag,description

use std::{fs, path::PathBuf};

use crate::Settings;

/// A single tag replacement entry.
#[derive(Debug, Clone)]
pub struct TagReplacement {
    /// Example value to show in preview field
    pub example: String,
    /// The tag/macro itself (e.g. @BEEP@)
    pub tag: String,
    /// Description of what the tag does
    pub description: String,
}

/// A loaded tag replacement list.
#[derive(Debug, Clone)]
pub struct TagReplacementList {
    /// Name of the list (filename without extension)
    pub name: String,
    /// The replacement entries
    pub entries: Vec<TagReplacement>,
}

impl TagReplacementList {
    /// Load a tag list from a CSV file.
    pub fn load_from_file(path: &PathBuf) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;
        let name = path.file_stem()?.to_string_lossy().to_string();

        let mut entries = Vec::new();
        for line in content.lines() {
            // Skip comments and empty lines
            if line.starts_with('#') || !line.contains(',') {
                continue;
            }

            let mut parts = line.split(',');
            let example = parts.next()?.trim().to_string();
            let tag = parts.next()?.trim().to_string();
            let description = parts.next().unwrap_or("").trim().to_string();

            entries.push(TagReplacement { example, tag, description });
        }

        Some(Self { name, entries })
    }

    /// Load the built-in PCBoard list.
    pub fn load_builtin_pcboard() -> Self {
        let content = include_str!("../../data/tags/pcboard.csv");
        let mut entries = Vec::new();

        for line in content.lines() {
            if line.starts_with('#') || !line.contains(',') {
                continue;
            }

            let mut parts = line.split(',');
            if let (Some(example), Some(tag)) = (parts.next(), parts.next()) {
                let description = parts.next().unwrap_or("").trim().to_string();
                entries.push(TagReplacement {
                    example: example.trim().to_string(),
                    tag: tag.trim().to_string(),
                    description,
                });
            }
        }

        Self {
            name: "PCBoard".to_string(),
            entries,
        }
    }
}

/// Get a list of available tag replacement lists.
/// Returns (name, path) pairs. The first entry is always "PCBoard" (built-in).
pub fn get_available_taglists() -> Vec<(String, Option<PathBuf>)> {
    let mut lists = vec![("PCBoard".to_string(), None)];

    if let Some(dir) = Settings::taglists_dir() {
        if dir.exists() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path: PathBuf = entry.path();
                    if path.extension().is_some_and(|e| e == "csv") {
                        if let Some(name) = path.file_stem() {
                            let name = name.to_string_lossy().to_string();
                            // Skip if it's the built-in PCBoard (case-insensitive)
                            if name.to_lowercase() != "pcboard" {
                                lists.push((name, Some(path)));
                            }
                        }
                    }
                }
            }
        }
    }

    lists
}

/// Load a tag replacement list by name.
/// If name is empty or "PCBoard", loads the built-in PCBoard list.
pub fn load_taglist(name: &str) -> TagReplacementList {
    if name.is_empty() || name.eq_ignore_ascii_case("pcboard") {
        return TagReplacementList::load_builtin_pcboard();
    }

    // Try to load from taglists directory
    if let Some(dir) = Settings::taglists_dir() {
        let path: PathBuf = dir.join(format!("{}.csv", name));
        if let Some(list) = TagReplacementList::load_from_file(&path) {
            return list;
        }
    }

    // Fallback to built-in
    TagReplacementList::load_builtin_pcboard()
}
