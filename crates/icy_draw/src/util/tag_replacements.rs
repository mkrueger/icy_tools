//! Tag replacement list management.
//!
//! Loads replacement lists from TOML files.
//!
//! Built-in lists live in `crates/icy_draw/data/tags/*.toml` and are embedded at compile time.
//! User lists are loaded from the configured taglist directory (see Settings).

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

/// A single tag replacement entry.
#[derive(Debug, Clone, Deserialize)]
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
    /// Stable identifier of the list (usually filename without extension)
    pub id: String,
    /// Display name of the list
    pub name: String,
    /// Short description shown above the entry list
    pub description: String,
    /// Optional longer comments shown below the entry list
    pub comments: String,
    /// Taglist format/content version (free-form)
    pub version: String,
    /// The replacement entries
    pub entries: Vec<TagReplacement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaglistInfo {
    pub id: String,
    pub name: String,
}

impl std::fmt::Display for TaglistInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Default for TaglistInfo {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct TaglistToml {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub comments: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub entries: Vec<TagReplacement>,
}

impl TagReplacementList {
    fn from_toml(id: String, toml: TaglistToml) -> Self {
        Self {
            id,
            name: toml.name,
            description: toml.description,
            comments: toml.comments,
            version: toml.version,
            entries: toml.entries,
        }
    }
}

fn parse_taglist_toml(id: &str, text: &str) -> Result<TagReplacementList, toml::de::Error> {
    let mut parsed: TaglistToml = toml::from_str(text)?;
    if parsed.name.trim().is_empty() {
        parsed.name = id.to_string();
    }
    Ok(TagReplacementList::from_toml(id.to_string(), parsed))
}

fn load_taglist_toml_from_path(id: &str, path: &Path) -> Option<TagReplacementList> {
    match fs::read_to_string(path) {
        Ok(text) => match parse_taglist_toml(id, &text) {
            Ok(list) => Some(list),
            Err(err) => {
                log::error!("Failed to parse taglist TOML {:?}: {}", path, err);
                None
            }
        },
        Err(err) => {
            log::error!("Failed to read taglist file {:?}: {}", path, err);
            None
        }
    }
}

fn load_builtin_taglist(id: &str) -> Option<TagReplacementList> {
    match id.to_ascii_lowercase().as_str() {
        "pcboard" => {
            let content = include_str!("../../data/tags/pcboard.toml");
            match parse_taglist_toml("pcboard", content) {
                Ok(mut list) => {
                    // Keep legacy display name for compatibility.
                    if list.name.trim().is_empty() {
                        list.name = "PCBoard".to_string();
                    }
                    if list.name == "pcboard" {
                        list.name = "PCBoard".to_string();
                    }
                    Some(list)
                }
                Err(err) => {
                    log::error!("Failed to parse built-in taglist '{}': {}", id, err);
                    None
                }
            }
        }
        "icyboard" => {
            let content = include_str!("../../data/tags/icyboard.toml");
            match parse_taglist_toml("icyboard", content) {
                Ok(mut list) => {
                    if list.name.trim().is_empty() {
                        list.name = "IcyBoard".to_string();
                    }
                    Some(list)
                }
                Err(err) => {
                    log::error!("Failed to parse built-in taglist '{}': {}", id, err);
                    None
                }
            }
        }
        _ => None,
    }
}

fn builtin_taglists() -> Vec<TaglistInfo> {
    // Keep built-ins explicit; they are compiled into the binary.
    let mut lists = Vec::new();
    if let Some(list) = load_builtin_taglist("pcboard") {
        lists.push(TaglistInfo { id: list.id, name: list.name });
    }
    if let Some(list) = load_builtin_taglist("icyboard") {
        lists.push(TaglistInfo { id: list.id, name: list.name });
    }
    lists
}

/// Get a list of available tag replacement lists.
///
/// Built-in lists are always included first.
/// User lists are loaded from the provided directory (if any).
pub fn get_available_taglists(taglists_dir: Option<&Path>) -> Vec<TaglistInfo> {
    let mut lists = builtin_taglists();

    let Some(dir) = taglists_dir else {
        return lists;
    };

    if !dir.exists() {
        return lists;
    }

    let mut user_lists: Vec<TaglistInfo> = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(err) => {
            log::error!("Failed to read taglists directory {:?}: {}", dir, err);
            return lists;
        }
    };

    for entry in entries.flatten() {
        let path: PathBuf = entry.path();
        if !path.is_file() {
            continue;
        }
        if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("toml")) {
            continue;
        }
        let Some(stem) = path.file_stem() else {
            continue;
        };
        let id = stem.to_string_lossy().to_string();
        if id.eq_ignore_ascii_case("pcboard") || id.eq_ignore_ascii_case("icyboard") {
            continue;
        }
        if let Some(list) = load_taglist_toml_from_path(&id, &path) {
            user_lists.push(TaglistInfo { id: list.id, name: list.name });
        }
    }

    user_lists.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    lists.extend(user_lists);

    lists
}

/// Load a tag replacement list by id.
///
/// If id is empty, loads the built-in PCBoard list.
pub fn load_taglist(id: &str, taglists_dir: Option<&Path>) -> TagReplacementList {
    if id.is_empty() {
        return load_builtin_taglist("pcboard").unwrap_or(TagReplacementList {
            id: "pcboard".to_string(),
            name: "PCBoard".to_string(),
            description: String::new(),
            comments: String::new(),
            version: String::new(),
            entries: Vec::new(),
        });
    }

    let id_lower = id.to_ascii_lowercase();
    if id_lower == "pcboard" || id_lower == "icyboard" {
        return load_builtin_taglist(&id_lower).unwrap_or_else(|| TagReplacementList {
            id: id_lower,
            name: String::new(),
            description: String::new(),
            comments: String::new(),
            version: String::new(),
            entries: Vec::new(),
        });
    }

    if let Some(dir) = taglists_dir {
        let path: PathBuf = dir.join(format!("{}.toml", id));
        if let Some(list) = load_taglist_toml_from_path(id, &path) {
            return list;
        }
    }

    // Fallback to built-in
    load_builtin_taglist("pcboard").unwrap_or(TagReplacementList {
        id: "pcboard".to_string(),
        name: "PCBoard".to_string(),
        description: String::new(),
        comments: String::new(),
        version: String::new(),
        entries: Vec::new(),
    })
}
