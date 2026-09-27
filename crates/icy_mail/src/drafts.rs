//! Packet-scoped draft persistence and QWK reply packet export.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use bstr::BString;
use i18n_embed_fl::fl;
use icy_engine::BufferType;
use jamjam::qwk::qwk_message::{QWKMessage, MSG_ACTIVE};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{qwk::QwkPackage, LANGUAGE_LOADER};

#[derive(Debug, Error)]
pub enum DraftError {
    #[error("invalid {field}: {reason}")]
    Invalid { field: &'static str, reason: String },
    #[error("draft {0} not found")]
    NotFound(u64),
    #[error("draft store belongs to a different BBS")]
    WrongPacket,
    #[error("no drafts to export")]
    Empty,
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    TomlDe(#[from] toml::de::Error),
    #[error(transparent)]
    TomlSer(#[from] toml::ser::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error("QWK message error: {0}")]
    Qwk(String),
    #[error("could not persist drafts: {0}")]
    Persistence(String),
}

pub type Result<T> = std::result::Result<T, DraftError>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DraftKind {
    #[default]
    New,
    Reply,
    Forward,
}

#[derive(Clone, Copy, Debug)]
pub enum Compose {
    New { conference: u16 },
    Reply { index: usize },
    Forward { index: usize },
}

/// Editable UTF-8 message fields. The UI can insert, edit, and remove drafts in `DraftStore::drafts`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub kind: DraftKind,
    pub to: String,
    pub from: String,
    pub subject: String,
    pub body: String,
    pub conference: u16,
    pub ref_number: u32,
    pub private: bool,
    #[serde(default)]
    pub date: String,
    /// Sent below the text as `... tagline`, see [`crate::taglines`].
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tagline: String,
}

impl Draft {
    /// The message text as it is sent, with the tagline.
    pub fn text(&self) -> String {
        crate::taglines::append(&self.body, &self.tagline)
    }
}

/// Maximum length of the QWK To, From and Subject header fields.
pub const HEADER_FIELD_LENGTH: usize = 25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraftField {
    Conference,
    From,
    To,
    Subject,
    Body,
    Tagline,
}

impl DraftField {
    pub fn label(self) -> String {
        match self {
            Self::Conference => fl!(LANGUAGE_LOADER, "draft-field-conference"),
            Self::From => fl!(LANGUAGE_LOADER, "draft-field-from"),
            Self::To => fl!(LANGUAGE_LOADER, "draft-field-to"),
            Self::Subject => fl!(LANGUAGE_LOADER, "draft-field-subject"),
            Self::Body => fl!(LANGUAGE_LOADER, "draft-field-body"),
            Self::Tagline => fl!(LANGUAGE_LOADER, "draft-field-tagline"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraftIssue {
    pub field: DraftField,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    bbs_id: String,
    date: String,
    #[serde(default)]
    next_id: u64,
    drafts: Vec<Draft>,
}

/// A single packet's drafts, stored under the application's data directory.
#[derive(Clone, Debug)]
pub struct DraftStore {
    path: PathBuf,
    source_path: PathBuf,
    bbs_id: String,
    date: String,
    next_id: u64,
    allowed_conferences: BTreeSet<u16>,
    pub drafts: Vec<Draft>,
}

impl DraftStore {
    /// Load drafts for any packet path (without reopening the packet).
    pub fn load(packet_path: &Path, package: &QwkPackage) -> crate::Res<Self> {
        Self::load_with_directory(packet_path, package, None)
    }

    /// Load drafts from a caller-specified directory (for portable storage or isolated tests).
    pub fn open_in(packet_path: &Path, package: &QwkPackage, directory: &Path) -> crate::Res<Self> {
        Self::load_with_directory(packet_path, package, Some(directory))
    }

    fn load_with_directory(packet_path: &Path, package: &QwkPackage, directory: Option<&Path>) -> crate::Res<Self> {
        let bbs_id = bbs_id(package)?;
        let allowed_conferences: BTreeSet<u16> = package
            .control_file
            .conferences
            .iter()
            .map(|conference| conference.number)
            .chain(package.descriptors.iter().map(|message| message.conference))
            .collect();
        let path = storage_path(packet_path, &bbs_id, directory, ".toml")?;
        let mut stored: Stored = if path.exists() {
            toml::from_str(&fs::read_to_string(&path)?)?
        } else {
            Stored {
                bbs_id: bbs_id.clone(),
                date: chrono::Local::now().format("%m-%d-%y%H:%M").to_string(),
                next_id: 1,
                drafts: Vec::new(),
            }
        };
        if stored.bbs_id != bbs_id {
            return Err(DraftError::WrongPacket.into());
        }
        validate_date(&stored.date)?;
        let mut max_id: u64 = 0;
        for draft in &mut stored.drafts {
            if draft.id == 0 {
                draft.id = max_id.checked_add(1).ok_or_else(|| invalid("draft ID", "exhausted"))?;
            }
            if draft.date.is_empty() {
                draft.date.clone_from(&stored.date);
            }
            validate_metadata(draft)?;
            validate_conference(&allowed_conferences, draft.conference)?;
            if draft.id <= max_id {
                return Err(invalid("draft IDs", "duplicate or unordered IDs").into());
            }
            max_id = draft.id;
        }
        let next_id = stored.next_id.max(max_id.saturating_add(1));
        if next_id <= max_id {
            return Err(invalid("draft ID", "exhausted").into());
        }
        Ok(Self {
            path,
            source_path: packet_path.canonicalize()?,
            bbs_id,
            date: stored.date,
            next_id,
            allowed_conferences,
            drafts: stored.drafts,
        })
    }

    pub fn open(packet_path: impl AsRef<Path>, package: &QwkPackage) -> crate::Res<Self> {
        Self::load(packet_path.as_ref(), package)
    }

    pub fn drafts(&self) -> &[Draft] {
        &self.drafts
    }

    /// Prepare a draft without writing it; cancelling the composer leaves no saved draft.
    pub fn prepare(&self, package: &QwkPackage, compose: Compose) -> Result<Draft> {
        if self.bbs_id != bbs_id(package)? {
            return Err(DraftError::WrongPacket);
        }
        let from = decode(&package.control_file.qmail_user_name);
        let (kind, to, subject, conference, ref_number, private) = match compose {
            Compose::New { conference } => (DraftKind::New, String::new(), String::new(), conference, 0, false),
            Compose::Reply { index } | Compose::Forward { index } => {
                let info = package.infos.get(index).ok_or_else(|| invalid("message index", "out of range"))?;
                let original = package.get_message(index).map_err(|e| DraftError::Qwk(e.to_string()))?;
                let subject = decode(&original.subj);
                if matches!(compose, Compose::Reply { .. }) {
                    (
                        DraftKind::Reply,
                        decode(&original.from),
                        if subject.to_ascii_lowercase().starts_with("re:") {
                            subject
                        } else {
                            format!("Re: {subject}")
                        },
                        info.conference,
                        info.number,
                        info.private,
                    )
                } else {
                    (DraftKind::Forward, String::new(), format!("Fwd: {subject}"), info.conference, 0, false)
                }
            }
        };
        validate_conference(&self.allowed_conferences, conference)?;
        Ok(Draft {
            id: self.next_id,
            kind,
            to,
            from,
            subject,
            body: String::new(),
            conference,
            ref_number,
            private,
            date: chrono::Local::now().format("%m-%d-%y%H:%M").to_string(),
            tagline: String::new(),
        })
    }

    /// Save a prepared draft, committing the in-memory change only after the write succeeds.
    pub fn insert(&mut self, draft: Draft) -> Result<()> {
        self.ensure_current()?;
        if draft.id != self.next_id {
            return Err(invalid("draft ID", "draft must be prepared for the current store"));
        }
        validate_metadata(&draft)?;
        validate_conference(&self.allowed_conferences, draft.conference)?;
        let mut next = self.clone();
        next.next_id = next.next_id.checked_add(1).ok_or_else(|| invalid("draft ID", "exhausted"))?;
        next.drafts.push(draft);
        next.save().map_err(|e| DraftError::Persistence(e.to_string()))?;
        *self = next;
        Ok(())
    }

    pub fn update(&mut self, draft: Draft) -> Result<()> {
        self.ensure_current()?;
        let index = self.drafts.iter().position(|d| d.id == draft.id).ok_or(DraftError::NotFound(draft.id))?;
        if draft.kind != self.drafts[index].kind || draft.date != self.drafts[index].date {
            return Err(invalid("draft metadata", "kind and date cannot be changed"));
        }
        validate_metadata(&draft)?;
        validate_conference(&self.allowed_conferences, draft.conference)?;
        let mut next = self.clone();
        next.drafts[index] = draft;
        next.save().map_err(|e| DraftError::Persistence(e.to_string()))?;
        *self = next;
        Ok(())
    }

    pub fn delete(&mut self, id: u64) -> Result<()> {
        self.ensure_current()?;
        let index = self.drafts.iter().position(|d| d.id == id).ok_or(DraftError::NotFound(id))?;
        let mut next = self.clone();
        next.drafts.remove(index);
        next.save().map_err(|e| DraftError::Persistence(e.to_string()))?;
        *self = next;
        Ok(())
    }

    fn ensure_current(&self) -> Result<()> {
        if !self.path.exists() {
            if self.next_id == 1 && self.drafts.is_empty() {
                return Ok(());
            }
            return Err(invalid("draft store", "changed on disk; reload the packet"));
        }
        let stored: Stored = toml::from_str(&fs::read_to_string(&self.path)?)?;
        if stored.bbs_id != self.bbs_id || stored.next_id != self.next_id || stored.drafts != self.drafts {
            return Err(invalid("draft store", "changed on disk; reload the packet"));
        }
        Ok(())
    }

    /// Atomically persist current drafts in the application's data directory.
    pub fn save(&self) -> crate::Res<()> {
        validate_date(&self.date)?;
        for draft in &self.drafts {
            validate_metadata(draft)?;
            validate_conference(&self.allowed_conferences, draft.conference)?;
        }
        let content = toml::to_string(&Stored {
            bbs_id: self.bbs_id.clone(),
            date: self.date.clone(),
            next_id: self.next_id,
            drafts: self.drafts.clone(),
        })?;
        atomic_write(&self.path, |file| {
            file.write_all(content.as_bytes())?;
            Ok(())
        })?;
        Ok(())
    }

    /// Write a deterministic ZIP with one uppercase `<BBSID>.MSG` member.
    pub fn export(&self, destination: &Path) -> crate::Res<()> {
        if destination.canonicalize().is_ok_and(|path| path == self.source_path) {
            return Err(invalid("export path", "cannot overwrite the source packet").into());
        }
        if self.drafts.is_empty() {
            return Err(DraftError::Empty.into());
        }
        let mut data = vec![b' '; 128];
        data[..self.bbs_id.len()].copy_from_slice(self.bbs_id.as_bytes());
        validate_date(&self.date)?;
        for draft in &self.drafts {
            validate_conference(&self.allowed_conferences, draft.conference)?;
            let (to, from, subj, body) = validate_draft(draft, true)?;
            let blocks = body.len().div_ceil(128).max(1) + 1;
            if blocks > 999_999 {
                return Err(invalid("body", "QWK block count exceeds six digits").into());
            }
            let message = QWKMessage {
                status: if draft.private { b'*' } else { b' ' },
                msg_number: u32::from(draft.conference), // REP uses conference in the number field.
                date_time: BString::from(draft.date.as_bytes()),
                to: to.into(),
                from: from.into(),
                subj: subj.into(),
                password: BString::from(""),
                ref_msg_number: draft.ref_number,
                active_flag: MSG_ACTIVE,
                conference_number: draft.conference,
                logical_message_number: 1,
                net_tag: b' ',
                text: body.into(),
            };
            message.write(&mut data, false).map_err(|e| DraftError::Qwk(e.to_string()))?;
        }
        atomic_write(destination, |file| {
            let mut zip = zip::ZipWriter::new(file);
            let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
                .last_modified_time(zip::DateTime::default());
            zip.start_file(format!("{}.MSG", self.bbs_id), options)?;
            zip.write_all(&data)?;
            zip.finish()?;
            Ok(())
        })?;
        Ok(())
    }

    pub fn export_rep(&self, destination: &Path) -> crate::Res<()> {
        self.export(destination)
    }

    /// Every problem that would stop `draft` from being exported, so an editor can show them while typing.
    pub fn issues(&self, draft: &Draft) -> Vec<DraftIssue> {
        let mut issues = Vec::new();
        if !self.allowed_conferences.contains(&draft.conference) {
            issues.push(DraftIssue {
                field: DraftField::Conference,
                message: fl!(LANGUAGE_LOADER, "draft-issue-conference-not-in-packet", conference = draft.conference),
            });
        }
        for (field, value) in [
            (DraftField::From, &draft.from),
            (DraftField::To, &draft.to),
            (DraftField::Subject, &draft.subject),
        ] {
            if value.trim().is_empty() {
                issues.push(DraftIssue {
                    field,
                    message: fl!(LANGUAGE_LOADER, "draft-issue-field-required", field = field.label()),
                });
                continue;
            }
            let length = value.chars().count();
            if length > HEADER_FIELD_LENGTH {
                issues.push(DraftIssue {
                    field,
                    message: fl!(
                        LANGUAGE_LOADER,
                        "draft-issue-field-too-long",
                        field = field.label(),
                        length = length,
                        limit = HEADER_FIELD_LENGTH
                    ),
                });
            }
            character_issues(field, value, false, &mut issues);
        }
        let body = draft.body.replace("\r\n", "\n").replace('\r', "\n");
        if crate::editor::strip_codes(&body).trim().is_empty() {
            issues.push(DraftIssue {
                field: DraftField::Body,
                message: fl!(LANGUAGE_LOADER, "draft-issue-message-text-required"),
            });
        } else {
            for line in body.split('\n') {
                character_issues(DraftField::Body, line, true, &mut issues);
            }
        }
        character_issues(DraftField::Tagline, &draft.tagline, false, &mut issues);
        issues
    }

    /// Suggested `.REP` filename beside the packet, using the BBS ID from CONTROL.DAT.
    pub fn default_export_path(&self, packet_path: &Path) -> PathBuf {
        packet_path.with_file_name(format!("{}.REP", self.bbs_id))
    }
}

fn invalid(field: &'static str, reason: impl Into<String>) -> DraftError {
    DraftError::Invalid { field, reason: reason.into() }
}

fn validate_conference(allowed: &BTreeSet<u16>, conference: u16) -> Result<()> {
    if !allowed.contains(&conference) {
        return Err(invalid(
            "conference",
            format!("{conference} is not listed in CONTROL.DAT or the packet messages"),
        ));
    }
    Ok(())
}

pub(crate) fn bbs_id(package: &QwkPackage) -> Result<String> {
    let raw = package.control_file.bbs_id.as_slice();
    if raw.is_empty() || raw.len() > 8 || !raw.iter().all(u8::is_ascii_alphanumeric) {
        return Err(invalid("BBS ID", "expected 1–8 ASCII letters or digits in CONTROL.DAT"));
    }
    Ok(String::from_utf8(raw.to_ascii_uppercase()).expect("ASCII ID"))
}

/// Per-packet file in the application's data directory (or `directory`), named after the BBS ID
/// and a stable hash of the packet's canonical path.
pub(crate) fn storage_path(packet_path: &Path, bbs_id: &str, directory: Option<&Path>, suffix: &str) -> Result<PathBuf> {
    let absolute = if packet_path.is_absolute() {
        packet_path.to_path_buf()
    } else {
        std::env::current_dir()?.join(packet_path)
    };
    let canonical = absolute.canonicalize().unwrap_or(absolute);
    let dir = if let Some(directory) = directory {
        directory.to_path_buf()
    } else {
        data_directory(packet_path)?
    };
    fs::create_dir_all(&dir)?;
    // FNV-1a keeps the name stable across Rust releases, unlike `DefaultHasher`.
    let hash = canonical
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3));
    let path = dir.join(format!("{bbs_id}-{hash:016x}{suffix}"));
    if suffix == ".toml" && !path.exists() {
        let mut legacy = std::collections::hash_map::DefaultHasher::new();
        canonical.hash(&mut legacy);
        let legacy = dir.join(format!("{bbs_id}-{:016x}.toml", legacy.finish()));
        if legacy.exists() {
            fs::rename(legacy, &path)?;
        }
    }
    Ok(path)
}

#[cfg(test)]
fn data_directory(packet_path: &Path) -> Result<PathBuf> {
    Ok(packet_path.parent().unwrap_or(Path::new(".")).join(".icy_mail_drafts"))
}

#[cfg(not(test))]
fn data_directory(_packet_path: &Path) -> Result<PathBuf> {
    Ok(directories::ProjectDirs::from("com", "GitHub", "icy_mail")
        .ok_or_else(|| invalid("data directory", fl!(LANGUAGE_LOADER, "packet-user-data-directory-unavailable")))?
        .data_local_dir()
        .join("drafts"))
}

/// Reports each distinct character of `value` that cannot be written to a QWK message once.
/// Message text may contain ESC for ANSI color sequences.
fn character_issues(field: DraftField, value: &str, allow_escape: bool, issues: &mut Vec<DraftIssue>) {
    for ch in value.chars() {
        if allow_escape && ch == '\x1b' {
            continue;
        }
        let field_label = field.label();
        let message = if ch.is_control() {
            fl!(
                LANGUAGE_LOADER,
                "draft-issue-control-character",
                field = field_label,
                code = format!("{:04X}", ch as u32)
            )
        } else {
            match BufferType::CP437.try_convert_from_unicode(ch) {
                None => fl!(
                    LANGUAGE_LOADER,
                    "draft-issue-no-cp437-equivalent",
                    field = field_label,
                    character = ch.to_string()
                ),
                Some(byte) if byte as u32 == 0xE3 || ch == '\u{e3}' => {
                    fl!(LANGUAGE_LOADER, "draft-issue-qwk-line-break", field = field_label, character = ch.to_string())
                }
                Some(_) => continue,
            }
        };
        if !issues.iter().any(|issue| issue.message == message) {
            issues.push(DraftIssue { field, message });
        }
    }
}

fn decode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&byte| BufferType::CP437.convert_to_unicode(char::from(byte)))
        .collect::<String>()
        .trim()
        .to_owned()
}

fn encode(field: &'static str, value: &str, max: Option<usize>) -> Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(value.len());
    for ch in value.chars() {
        if field == "body" && ch == '\x1b' {
            bytes.push(0x1b);
            continue;
        }
        if ch.is_control() || ch == '\u{e3}' {
            return Err(invalid(field, "control characters are not allowed"));
        }
        let byte = BufferType::CP437
            .try_convert_from_unicode(ch)
            .ok_or_else(|| invalid(field, format!("{ch:?} cannot be encoded as CP437")))? as u8;
        if byte == 0xE3 {
            return Err(invalid(field, "CP437 byte E3 is reserved for QWK newlines"));
        }
        bytes.push(byte);
    }
    if let Some(limit) = max {
        if bytes.len() > limit {
            return Err(invalid(field, format!("exceeds {limit} CP437 bytes")));
        }
    }
    Ok(bytes)
}

fn validate_date(date: &str) -> Result<()> {
    if date.len() != 13 || chrono::NaiveDateTime::parse_from_str(date, "%m-%d-%y%H:%M").is_err() {
        return Err(invalid("date", "expected MM-DD-YYHH:MM"));
    }
    Ok(())
}

fn validate_metadata(d: &Draft) -> Result<()> {
    if d.id == 0 {
        return Err(invalid("draft ID", "must be prepared before saving"));
    }
    validate_date(&d.date)?;
    if d.ref_number > 99_999_999 {
        return Err(invalid("reference number", "exceeds eight digits"));
    }
    Ok(())
}

fn validate_draft(d: &Draft, complete: bool) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)> {
    validate_metadata(d)?;
    let text = d.text();
    if text.len() > 128 * 999_998 {
        return Err(invalid("body", "QWK block count exceeds six digits"));
    }
    let to = encode("to", &d.to, Some(HEADER_FIELD_LENGTH))?;
    let from = encode("from", &d.from, Some(HEADER_FIELD_LENGTH))?;
    let subject = encode("subject", &d.subject, Some(HEADER_FIELD_LENGTH))?;
    let mut body = Vec::new();
    for (index, line) in text.replace("\r\n", "\n").replace('\r', "\n").split('\n').enumerate() {
        if index > 0 {
            body.push(b'\n');
        }
        body.extend(encode("body", line, None)?);
    }
    if complete && (to.is_empty() || from.is_empty() || subject.is_empty() || d.body.is_empty()) {
        return Err(invalid("draft", "to, from, subject and body are required for export"));
    }
    Ok((to, from, subject, body))
}

pub(crate) fn atomic_write(path: &Path, write: impl FnOnce(&mut File) -> Result<()>) -> Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let (temp, mut file) = loop {
        let mut name = path.as_os_str().to_os_string();
        name.push(format!(".{}.{}.tmp", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
        let temp = PathBuf::from(name);
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => break (temp, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    };
    let result = (|| {
        write(&mut file)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};

    #[test]
    fn prepare_is_not_persisted_until_insert_and_edits_are_atomic() {
        let (dir, package) = crate::qwk::tests::load();
        let path = dir.path().join("TEST.QWK");
        let mut store = DraftStore::open(&path, &package).unwrap();
        let mut prepared = store.prepare(&package, Compose::Reply { index: 1 }).unwrap();
        assert_eq!(prepared.ref_number, 11);
        assert!(store.drafts().is_empty());
        assert!(!store.path.exists());
        prepared.body = "First".into();
        store.insert(prepared.clone()).unwrap();
        assert_eq!(DraftStore::open(&path, &package).unwrap().drafts(), &[prepared.clone()]);
        assert!(store.insert(prepared.clone()).is_err());
        prepared.body = "Edited".into();
        store.update(prepared.clone()).unwrap();
        assert_eq!(DraftStore::open(&path, &package).unwrap().drafts(), &[prepared.clone()]);
        store.delete(prepared.id).unwrap();
        assert!(DraftStore::open(&path, &package).unwrap().drafts().is_empty());
    }

    #[test]
    fn issues_report_every_export_problem_while_editing() {
        let (dir, package) = crate::qwk::tests::load();
        let store = DraftStore::open(dir.path().join("TEST.QWK"), &package).unwrap();
        let mut draft = store.prepare(&package, Compose::New { conference: 1 }).unwrap();
        let fields: Vec<_> = store.issues(&draft).iter().map(|issue| issue.field).collect();
        assert_eq!(fields, [DraftField::To, DraftField::Subject, DraftField::Body]);
        draft.to = "All".into();
        draft.subject = "x".repeat(26);
        draft.body = "Tab\there \u{1F30D} and \u{3c0}\nok".into();
        draft.conference = 9;
        let issues = store.issues(&draft);
        assert_eq!(issues.len(), 5, "{issues:?}");
        assert!(issues[0].message.contains("Conference 9"));
        assert!(issues[1].message.contains("26 characters"));
        assert!(issues.iter().any(|issue| issue.message.contains("U+0009")));
        assert!(issues.iter().any(|issue| issue.message.contains("no CP437")));
        assert!(issues.iter().any(|issue| issue.message.contains("line break")));
        draft.subject = "Hello".into();
        draft.body = "Caf\u{e9}".into();
        draft.conference = 1;
        assert!(store.issues(&draft).is_empty());
    }

    #[test]
    fn stale_window_cannot_overwrite_another_windows_drafts() {
        let (dir, package) = crate::qwk::tests::load();
        let path = dir.path().join("TEST.QWK");
        let mut first = DraftStore::open(&path, &package).unwrap();
        let mut second = DraftStore::open(&path, &package).unwrap();
        let draft = first.prepare(&package, Compose::New { conference: 1 }).unwrap();
        first.insert(draft).unwrap();
        let stale = second.prepare(&package, Compose::New { conference: 2 }).unwrap();
        assert!(second.insert(stale).is_err());
        assert_eq!(DraftStore::open(&path, &package).unwrap().drafts(), first.drafts());
    }

    #[test]
    fn accepts_empty_control_conferences_and_packet_only_conferences() {
        let (dir, mut package) = crate::qwk::tests::load();
        let path = dir.path().join("TEST.QWK");
        package.control_file.conferences.push(jamjam::qwk::control::Conference {
            number: 3,
            name: BString::from("Empty"),
        });
        package.descriptors[0].conference = 4;
        let mut store = DraftStore::open(&path, &package).unwrap();
        assert!(store.prepare(&package, Compose::New { conference: 5 }).is_err());
        let draft = store.prepare(&package, Compose::New { conference: 3 }).unwrap();
        assert_eq!(draft.conference, 3);
        store.insert(draft).unwrap();
        let packet_only = store.prepare(&package, Compose::New { conference: 4 }).unwrap();
        store.insert(packet_only).unwrap();
        assert_eq!(DraftStore::open(&path, &package).unwrap().drafts().len(), 2);
        let mut invalid = store.drafts()[0].clone();
        invalid.conference = 5;
        assert!(store.update(invalid).is_err());
        store.drafts[0].conference = 5;
        assert!(store.save().is_err());
        assert!(store.export(&dir.path().join("TEST.REP")).is_err());
    }

    fn reply() -> Draft {
        Draft {
            id: 1,
            kind: DraftKind::Reply,
            to: "André".into(),
            from: "Reader".into(),
            subject: "Re: Coffee".into(),
            body: "Café\r\nNext".into(),
            conference: 1,
            ref_number: 11,
            private: true,
            date: "01-01-2600:00".into(),
            tagline: "Bye".into(),
        }
    }

    #[test]
    fn persists_and_exports_reply_packet() {
        let (dir, package) = crate::qwk::tests::load();
        let packet = dir.path().join("TEST.QWK");
        let mut store = DraftStore::load(&packet, &package).unwrap();
        store.drafts.push(reply());
        store.drafts.push(Draft {
            id: 2,
            kind: DraftKind::Forward,
            to: "Someone".into(),
            from: "Reader".into(),
            subject: "Fwd: Coffee".into(),
            body: "Forwarded".into(),
            conference: 1,
            ref_number: 0,
            private: false,
            date: "01-01-2600:00".into(),
            tagline: String::new(),
        });
        store.save().unwrap();
        drop(store);

        let mut store = DraftStore::load(&packet, &package).unwrap();
        assert_eq!(store.drafts[0], reply());
        assert_eq!(store.default_export_path(&packet), dir.path().join("TEST.REP"));
        store.drafts.remove(1);
        store.save().unwrap();
        store.drafts.push(Draft {
            id: 2,
            kind: DraftKind::Forward,
            to: "Someone".into(),
            from: "Reader".into(),
            subject: "Fwd: Coffee".into(),
            body: "Forwarded".into(),
            conference: 1,
            ref_number: 0,
            private: false,
            date: "01-01-2600:00".into(),
            tagline: String::new(),
        });
        let rep = dir.path().join("TEST.REP");
        let original_packet = fs::read(&packet).unwrap();
        assert!(store.export(&packet).is_err());
        assert_eq!(fs::read(&packet).unwrap(), original_packet);
        store.export(&rep).unwrap();
        let first_export = fs::read(&rep).unwrap();
        store.export(&rep).unwrap();
        assert_eq!(first_export, fs::read(&rep).unwrap());
        let mut archive = zip::ZipArchive::new(File::open(rep).unwrap()).unwrap();
        assert_eq!(archive.len(), 1);
        let mut entry = archive.by_name("TEST.MSG").unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes.len() % 128, 0);
        assert_eq!(&bytes[..4], b"TEST");
        let mut cursor = Cursor::new(&bytes[128..]);
        let msg = QWKMessage::read(&mut cursor, false).unwrap();
        assert_eq!(msg.msg_number, 1);
        assert_eq!(msg.conference_number, 1);
        assert_eq!(msg.ref_msg_number, 11);
        assert_eq!(msg.status, b'*');
        assert_eq!(msg.to.as_slice(), b"Andr\x82");
        assert!(msg.text.starts_with(b"Caf\x82\nNext"));
        assert!(bytes.windows(18).any(|w| w == b"Caf\x82\xE3Next\xE3\xE3... Bye"));
        let next = QWKMessage::read(&mut cursor, false).unwrap();
        assert_eq!(next.ref_msg_number, 0);
        assert_eq!(next.msg_number, 1);
        assert_eq!(next.conference_number, 1);
    }

    #[test]
    fn invalid_data_does_not_replace_export_or_drafts() {
        let (dir, package) = crate::qwk::tests::load();
        let path = dir.path().join("TEST.QWK");
        let mut store = DraftStore::load(&path, &package).unwrap();
        let mut wrong_package = package.clone();
        wrong_package.control_file.bbs_id = BString::from("OTHER");
        let prepared = store.prepare(&package, Compose::Reply { index: 1 }).unwrap();
        assert_eq!(prepared.ref_number, 11);
        assert!(store.drafts.is_empty());
        assert!(store.prepare(&wrong_package, Compose::New { conference: 1 }).is_err());
        store.drafts.push(reply());
        store.save().unwrap();
        assert!(DraftStore::load(&path, &wrong_package).unwrap().drafts.is_empty());
        let dest = dir.path().join("existing.REP");
        fs::write(&dest, b"untouched").unwrap();
        store.drafts[0].body = "🌍".into();
        store.save().unwrap();
        assert!(store.export(&dest).is_err());
        assert_eq!(fs::read(&dest).unwrap(), b"untouched");
        store.drafts[0].body = "ok".into();
        store.drafts[0].subject = "x".repeat(26);
        assert!(store.export(&dest).is_err());
        assert_eq!(fs::read(&dest).unwrap(), b"untouched");
        store.drafts[0].subject = "Valid".into();
        store.drafts[0].body = "π".into();
        assert!(store.export(&dest).is_err());
        assert_eq!(fs::read(&dest).unwrap(), b"untouched");
        store.drafts[0].ref_number = 100_000_000;
        let saved = fs::read(&store.path).unwrap();
        assert!(store.save().is_err());
        assert_eq!(fs::read(&store.path).unwrap(), saved);
        fs::write(&store.path, "not toml = [").unwrap();
        assert!(DraftStore::load(&path, &package).is_err());
    }

    #[test]
    fn preserves_leading_and_empty_body_lines() {
        let (dir, package) = crate::qwk::tests::load();
        let mut store = DraftStore::load(&dir.path().join("TEST.QWK"), &package).unwrap();
        store.drafts.push(Draft {
            id: 1,
            kind: DraftKind::New,
            to: "All".into(),
            from: "Reader".into(),
            subject: "Hello".into(),
            body: "\n\nText\n".into(),
            conference: 2,
            ref_number: 0,
            private: false,
            date: "01-01-2600:00".into(),
            tagline: String::new(),
        });
        let rep = dir.path().join("TEST.REP");
        store.export(&rep).unwrap();
        let mut zip = zip::ZipArchive::new(File::open(rep).unwrap()).unwrap();
        let mut data = Vec::new();
        zip.by_name("TEST.MSG").unwrap().read_to_end(&mut data).unwrap();
        let message = QWKMessage::read(Cursor::new(&data[128..]), false).unwrap();
        assert_eq!(message.msg_number, 2);
        assert!(message.text.starts_with(b"\n\nText\n"));
        assert!(data.windows(7).any(|w| w == b"\xE3\xE3Text\xE3"));
    }
}
