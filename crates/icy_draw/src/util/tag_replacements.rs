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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TaglistInfo {
    pub id: String,
    pub name: String,
}

impl std::fmt::Display for TaglistInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
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
                log::error!("Failed to parse taglist TOML {path:?}: {err}");
                None
            }
        },
        Err(err) => {
            log::error!("Failed to read taglist file {path:?}: {err}");
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
                    log::error!("Failed to parse built-in taglist '{id}': {err}");
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
                    log::error!("Failed to parse built-in taglist '{id}': {err}");
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
#[must_use]
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
            log::error!("Failed to read taglists directory {dir:?}: {err}");
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

    user_lists.sort_by_key(|a| a.name.to_lowercase());
    lists.extend(user_lists);

    lists
}

/// Load a tag replacement list by id.
///
/// If id is empty, loads the built-in `PCBoard` list.
#[must_use]
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
        let path: PathBuf = dir.join(format!("{id}.toml"));
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

/// Parses a taglist, reporting why it is not one.
pub fn parse_taglist(id: &str, text: &str) -> Result<TagReplacementList, String> {
    parse_taglist_toml(id, text).map_err(|error| error.message().to_string())
}

fn is_builtin(id: &str) -> bool {
    id.eq_ignore_ascii_case("pcboard") || id.eq_ignore_ascii_case("icyboard")
}

/// Copies the taglist at `source` into `dir` after checking it parses; returns the id it is
/// listed under. A file named like a built-in list gets a suffix, so it does not hide.
pub fn import_taglist(source: &Path, dir: &Path) -> Result<String, String> {
    let text = fs::read_to_string(source).map_err(|error| error.to_string())?;
    let stem = source.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = if stem.trim().is_empty() { "taglist".to_string() } else { stem };
    let id = if is_builtin(&stem) { format!("{stem}-custom") } else { stem };
    let list = parse_taglist(&id, &text)?;
    if list.entries.is_empty() {
        return Err("the list has no [[entries]]".to_string());
    }
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    fs::write(dir.join(format!("{id}.toml")), text).map_err(|error| error.to_string())?;
    Ok(id)
}

/// Template of a new user taglist, documenting the format with one entry.
pub const TAGLIST_TEMPLATE: &str = r#"# A tag replacement list for Icy Draw's tag tool.
name = "My Tags"
description = "Replacements of my BBS"
comments = """Shown below the list."""
version = "1.0.0"

# One block per replacement: the text the BBS replaces, an example shown as the tag's
# preview and a description.
[[entries]]
tag = "@USER@"
example = "Sysop"
description = "Name of the current user."
"#;

/// Creates a new taglist from [`TAGLIST_TEMPLATE`] in `dir` under a free name; returns its id
/// and path.
pub fn create_taglist(dir: &Path) -> Result<(String, PathBuf), String> {
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let id = (1..)
        .map(|number| if number == 1 { "my_tags".to_string() } else { format!("my_tags_{number}") })
        .find(|id| !dir.join(format!("{id}.toml")).exists())
        .expect("a free taglist name");
    let path = dir.join(format!("{id}.toml"));
    fs::write(&path, TAGLIST_TEMPLATE).map_err(|error| error.to_string())?;
    Ok((id, path))
}

/// The entries of `list` whose tag, description or example contain `filter`, ignoring case;
/// entries whose tag matches come first.
pub fn filter_taglist<'a>(list: &'a TagReplacementList, filter: &str) -> Vec<&'a TagReplacement> {
    let filter = filter.trim().to_lowercase();
    let (mut tags, other): (Vec<_>, Vec<_>) = list
        .entries
        .iter()
        .filter(|entry| {
            filter.is_empty()
                || entry.tag.to_lowercase().contains(&filter)
                || entry.description.to_lowercase().contains(&filter)
                || entry.example.to_lowercase().contains(&filter)
        })
        .partition(|entry| filter.is_empty() || entry.tag.to_lowercase().contains(&filter));
    tags.extend(other);
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_lists_parse_and_come_first() {
        let lists = get_available_taglists(None);
        assert_eq!(lists.iter().map(|list| list.id.as_str()).collect::<Vec<_>>(), ["pcboard", "icyboard"]);
        for list in lists {
            assert!(!load_taglist(&list.id, None).entries.is_empty(), "{} has entries", list.id);
        }
    }

    #[test]
    fn template_lists_are_created_imported_and_filtered() {
        let dir = tempfile::tempdir().unwrap();
        let (id, path) = create_taglist(dir.path()).unwrap();
        assert_eq!(id, "my_tags");
        assert_eq!(create_taglist(dir.path()).unwrap().0, "my_tags_2");
        let list = load_taglist(&id, Some(dir.path()));
        assert_eq!(list.name, "My Tags");
        assert_eq!(list.entries[0].tag, "@USER@");

        let imported = dir.path().join("import");
        fs::create_dir(&imported).unwrap();
        let source = dir.path().join("PCBoard.toml");
        fs::copy(&path, &source).unwrap();
        assert_eq!(import_taglist(&source, &imported).unwrap(), "PCBoard-custom", "built-in names stay visible");
        let ids: Vec<_> = get_available_taglists(Some(&imported)).into_iter().map(|list| list.id).collect();
        assert_eq!(ids, ["pcboard", "icyboard", "PCBoard-custom"]);

        fs::write(&source, "entries = 5").unwrap();
        assert!(import_taglist(&source, &imported).is_err(), "broken lists are not copied");

        let pcboard = load_taglist("pcboard", None);
        assert!(filter_taglist(&pcboard, "").len() == pcboard.entries.len());
        assert!(filter_taglist(&pcboard, "beep").iter().all(|entry| {
            format!("{}{}{}", entry.tag, entry.description, entry.example).to_lowercase().contains("beep")
        }));
        assert!(!filter_taglist(&pcboard, "BEEP").is_empty());
        let users = filter_taglist(&pcboard, "user");
        let first_description_match = users.iter().position(|entry| !entry.tag.to_lowercase().contains("user")).unwrap();
        assert!(users[..first_description_match].iter().any(|entry| entry.tag == "@USER@"), "tag matches come first");
        assert!(users[first_description_match..].iter().all(|entry| !entry.tag.to_lowercase().contains("user")));
    }
}
