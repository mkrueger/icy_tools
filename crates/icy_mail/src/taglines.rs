//! Taglines, the one-line sayings BBS mail readers put below a message as `... text`.
//!
//! They are kept in `taglines.txt` in the user data directory, a UTF-8 text file with one
//! tagline per line, so existing collections can be copied over.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::{drafts::atomic_write, editor};

/// Longest tagline supported by the reply format.
pub const TAGLINE_LENGTH: usize = 76;
const FILE_NAME: &str = "taglines.txt";
const PREFIX: &str = "... ";

#[derive(Clone, Debug)]
pub struct Taglines {
    path: PathBuf,
    pub lines: Vec<String>,
    /// The file as last read or written, to notice changes made by other programs.
    stamp: Option<Stamp>,
}

/// Modification time and size of the tagline file.
type Stamp = (SystemTime, u64);

/// What [`Taglines::import`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportResult {
    pub added: usize,
    pub duplicates: usize,
}

impl Taglines {
    pub fn open() -> crate::Res<Self> {
        Self::open_in(&crate::state::data_directory()?)
    }

    pub fn open_in(directory: &Path) -> crate::Res<Self> {
        let path = directory.join(FILE_NAME);
        let stamp = stamp(&path);
        let lines = read(&path)?;
        Ok(Self { path, lines, stamp })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Picks up changes made by another window.
    pub fn reload(&mut self) -> crate::Res<()> {
        self.stamp = stamp(&self.path);
        self.lines = read(&self.path)?;
        Ok(())
    }

    /// Whether the file was changed since it was last read or written here, e.g. in a text editor.
    pub fn changed_on_disk(&self) -> bool {
        stamp(&self.path) != self.stamp
    }

    pub fn save(&mut self) -> crate::Res<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = self.content();
        atomic_write(&self.path, |file| {
            file.write_all(content.as_bytes())?;
            Ok(())
        })?;
        self.stamp = stamp(&self.path);
        Ok(())
    }

    /// Adds the taglines of another UTF-8 tagline file, one per line, skipping those already known.
    /// The list is not saved.
    pub fn import(&mut self, path: &Path) -> crate::Res<ImportResult> {
        let mut result = ImportResult::default();
        for line in read(path)? {
            if self.lines.contains(&line) {
                result.duplicates += 1;
            } else {
                self.lines.push(line);
                result.added += 1;
            }
        }
        Ok(result)
    }

    /// Writes the taglines to `path` as a UTF-8 text file, one per line.
    pub fn export(&self, path: &Path) -> crate::Res<()> {
        fs::write(path, self.content())?;
        Ok(())
    }

    fn content(&self) -> String {
        self.lines.iter().map(|line| format!("{line}\n")).collect()
    }

    /// Adds `text` unless it is empty; returns the index of the new or already present tagline.
    pub fn add(&mut self, text: &str) -> Option<usize> {
        let text = clean(text);
        if text.is_empty() {
            return None;
        }
        if let Some(index) = self.lines.iter().position(|line| *line == text) {
            return Some(index);
        }
        self.lines.push(text);
        Some(self.lines.len() - 1)
    }

    /// Replaces the tagline at `index`; returns whether there was something to store.
    pub fn set(&mut self, index: usize, text: &str) -> bool {
        let text = clean(text);
        match self.lines.get_mut(index) {
            Some(line) if !text.is_empty() => {
                *line = text;
                true
            }
            _ => false,
        }
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.lines.len() {
            self.lines.remove(index);
        }
    }

    pub fn random(&self) -> Option<&str> {
        if self.lines.is_empty() {
            return None;
        }
        Some(&self.lines[fastrand::usize(..self.lines.len())])
    }
}

fn stamp(path: &Path) -> Option<Stamp> {
    let metadata = fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

fn read(path: &Path) -> crate::Res<Vec<String>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = clean(line);
        if !line.is_empty() && !lines.contains(&line) {
            lines.push(line);
        }
    }
    Ok(lines)
}

/// A tagline as it can be stored and sent: one line of at most [`TAGLINE_LENGTH`] characters
/// that can be written to a QWK message, without the `... ` it is shown with.
pub fn clean(text: &str) -> String {
    let text: String = text
        .chars()
        .map(|ch| if ch.is_whitespace() { ' ' } else { ch })
        .filter(|&ch| ch == ' ' || editor::is_message_char(ch))
        .collect();
    let mut text = text.trim();
    while let Some(rest) = text.strip_prefix("...") {
        text = rest.trim_start();
    }
    text.chars().take(TAGLINE_LENGTH).collect::<String>().trim_end().to_string()
}

/// The tagline of a message: its last `... text` line.
/// `text` is the plain message text without color codes.
pub fn find(text: &str) -> Option<String> {
    text.lines()
        .rev()
        .map(str::trim_end)
        .filter_map(|line| line.strip_prefix(PREFIX))
        .map(clean)
        .find(|tagline| !tagline.is_empty())
}

/// `body` with `tagline` below it, as the message is sent.
pub fn append(body: &str, tagline: &str) -> String {
    let tagline = tagline.trim();
    if tagline.is_empty() {
        return body.to_string();
    }
    let body = body.trim_end_matches(['\n', '\r']);
    // Reset colors so the tagline does not continue the text's last color.
    let reset = if body.contains('\x1b') { "\x1b[0m" } else { "" };
    if body.is_empty() {
        format!("{reset}{PREFIX}{tagline}")
    } else {
        format!("{body}\n\n{reset}{PREFIX}{tagline}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_round_trip_skips_blanks_and_duplicates() {
        let dir = crate::qwk::tests::TempDir::new();
        let mut taglines = Taglines::open_in(dir.path()).unwrap();
        assert!(taglines.lines.is_empty());
        assert!(taglines.random().is_none());
        assert_eq!(taglines.add("  ... Stay a while, stay forever!  "), Some(0));
        assert_eq!(taglines.add("Second"), Some(1));
        assert_eq!(taglines.add("Second"), Some(1));
        assert_eq!(taglines.add("   "), None);
        assert!(taglines.set(1, "Changed"));
        assert!(!taglines.set(1, ""));
        assert!(!taglines.set(9, "Missing"));
        taglines.save().unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join(FILE_NAME)).unwrap(),
            "Stay a while, stay forever!\nChanged\n"
        );
        fs::write(dir.path().join(FILE_NAME), "One\r\n\r\nTwo\tparts\nOne\n\u{df}eta\n").unwrap();
        taglines.reload().unwrap();
        assert_eq!(taglines.lines, ["One", "Two parts", "\u{df}eta"]);
        assert!(taglines.lines.contains(&taglines.random().unwrap().to_string()));
        taglines.remove(0);
        assert_eq!(taglines.lines, ["Two parts", "\u{df}eta"]);
    }

    #[test]
    fn imports_exports_and_notices_outside_changes() {
        let dir = crate::qwk::tests::TempDir::new();
        let mut taglines = Taglines::open_in(dir.path()).unwrap();
        assert!(!taglines.changed_on_disk());
        taglines.add("Known");
        taglines.save().unwrap();
        assert!(!taglines.changed_on_disk());

        let other = dir.path().join("other.tag");
        fs::write(&other, "... Known\r\nNew one\n\nAnother\u{df}\nNew one\n").unwrap();
        let result = taglines.import(&other).unwrap();
        assert_eq!(result, ImportResult { added: 2, duplicates: 1 });
        assert_eq!(taglines.lines, ["Known", "New one", "Another\u{df}"]);
        // Data files are UTF-8 only; a CP437 file is refused instead of guessed.
        fs::write(&other, b"Stra\xe1e\n").unwrap();
        assert!(taglines.import(&other).is_err());
        assert_eq!(taglines.lines.len(), 3);

        let exported = dir.path().join("export.txt");
        taglines.export(&exported).unwrap();
        assert_eq!(fs::read_to_string(&exported).unwrap(), "Known\nNew one\nAnother\u{df}\n");

        fs::write(taglines.path(), "Edited elsewhere\n").unwrap();
        assert!(taglines.changed_on_disk());
        taglines.reload().unwrap();
        assert!(!taglines.changed_on_disk());
        assert_eq!(taglines.lines, ["Edited elsewhere"]);
    }

    #[test]
    fn cleans_to_one_sendable_line() {
        assert_eq!(clean("a\u{1f30d}b\u{7}c"), "abc");
        assert_eq!(clean(&"x".repeat(100)).len(), TAGLINE_LENGTH);
        assert_eq!(clean("......"), "");
    }

    #[test]
    fn finds_and_appends_taglines() {
        assert_eq!(find("Hi\n\n... First\nbye\n... Last one  \n"), Some("Last one".into()));
        assert_eq!(find("No tagline\n...\n--- tearline"), None);
        assert_eq!(append("Text\n\n", "Bye"), "Text\n\n... Bye");
        assert_eq!(append("Text", " "), "Text");
        assert_eq!(append("", "Bye"), "... Bye");
        assert_eq!(append("\x1b[31mRed", "Bye"), "\x1b[31mRed\n\n\x1b[0m... Bye");
    }
}
