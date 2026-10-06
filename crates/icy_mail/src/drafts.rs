//! Packet-scoped draft persistence and QWK reply packet export.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use bstr::BString;
use i18n_embed_fl::fl;
use icy_engine::BufferType;
use jamjam::qwk::qwk_message::{QWKMessage, MSG_ACTIVE};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use unarc_rs::unified::{ArchiveFormat, ArchiveOptions, UnifiedArchive};

use crate::{
    qwk::{Capabilities, QwkPackage, SubscriptionFormat},
    LANGUAGE_LOADER,
};

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
    /// The text as paragraphs: line breaks only where the author ended a paragraph, colors as ANSI
    /// SGR codes. [`Self::text`] wraps it into message lines.
    pub body: String,
    pub conference: u16,
    pub ref_number: u32,
    pub private: bool,
    #[serde(default)]
    pub date: String,
    /// Sent below the text as `... tagline`, see [`crate::taglines`].
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tagline: String,
    /// Appended once after the body, before the tagline.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature: String,
}

impl Draft {
    /// The message text as it is sent: the paragraphs wrapped into lines of at most
    /// [`crate::editor::WRAP_WIDTH`] columns, with the tagline.
    pub fn text(&self) -> String {
        let mut text = crate::editor::wrap_body(&self.body);
        let signature = crate::editor::wrap_body(&self.signature.replace("\r\n", "\n").replace('\r', "\n"));
        if !signature.trim().is_empty() && text != signature && !text.ends_with(&format!("\n{signature}")) {
            text.push_str("\n\n");
            text.push_str(&signature);
        }
        crate::taglines::append(&text, &self.tagline)
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
    Signature,
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
            Self::Signature => fl!(LANGUAGE_LOADER, "draft-field-signature"),
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
    #[serde(default)]
    subscriptions: BTreeMap<u16, bool>,
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
    capabilities: Capabilities,
    blue_wave: Option<std::sync::Arc<crate::blue_wave::Packet>>,
    sender: String,
    subscriptions: BTreeMap<u16, bool>,
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
                subscriptions: BTreeMap::new(),
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
            validate_metadata_for(draft, package.blue_wave.is_some())?;
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
        validate_subscriptions(&allowed_conferences, &package.capabilities, &stored.subscriptions)?;
        Ok(Self {
            path,
            source_path: packet_path.canonicalize()?,
            bbs_id,
            date: stored.date,
            next_id,
            allowed_conferences,
            capabilities: package.capabilities.clone(),
            blue_wave: package.blue_wave.clone(),
            sender: decode(&package.control_file.qmail_user_name),
            subscriptions: stored.subscriptions,
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
        let mut from = decode(&package.control_file.qmail_user_name);
        let (kind, to, subject, conference, ref_number, mut private) = match compose {
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
                        if package.blue_wave.is_some() { original.msg_number } else { info.number },
                        info.private,
                    )
                } else {
                    let conference = if self.can_post(info.conference) {
                        info.conference
                    } else {
                        self.allowed_conferences
                            .iter()
                            .copied()
                            .find(|&number| self.can_post(number))
                            .ok_or_else(|| invalid("conference", "the packet contains no writable conference"))?
                    };
                    (DraftKind::Forward, String::new(), format!("Fwd: {subject}"), conference, 0, false)
                }
            }
        };
        validate_conference(&self.allowed_conferences, conference)?;
        if let Some(packet) = &self.blue_wave {
            let (sender, required_private) =
                crate::blue_wave::posting_defaults(packet, conference).map_err(|error| invalid("conference", error.to_string()))?;
            from = decode(&sender);
            private |= required_private;
        }
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
            date: if self.blue_wave.is_some() {
                chrono::Utc::now().format("%m-%d-%y%H:%M").to_string()
            } else {
                chrono::Local::now().format("%m-%d-%y%H:%M").to_string()
            },
            tagline: String::new(),
            signature: String::new(),
        })
    }

    /// Save a prepared draft, committing the in-memory change only after the write succeeds.
    pub fn insert(&mut self, draft: Draft) -> Result<()> {
        self.ensure_current()?;
        if draft.id != self.next_id {
            return Err(invalid("draft ID", "draft must be prepared for the current store"));
        }
        validate_metadata_for(&draft, self.blue_wave.is_some())?;
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
        validate_metadata_for(&draft, self.blue_wave.is_some())?;
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
            if self.next_id == 1 && self.drafts.is_empty() && self.subscriptions.is_empty() {
                return Ok(());
            }
            return Err(invalid("draft store", "changed on disk; reload the packet"));
        }
        let mut stored: Stored = toml::from_str(&fs::read_to_string(&self.path)?)?;
        let mut max_id = 0u64;
        for draft in &mut stored.drafts {
            if draft.id == 0 {
                draft.id = max_id.checked_add(1).ok_or_else(|| invalid("draft ID", "exhausted"))?;
            }
            if draft.date.is_empty() {
                draft.date.clone_from(&stored.date);
            }
            max_id = draft.id;
        }
        stored.next_id = stored.next_id.max(max_id.saturating_add(1));
        if stored.bbs_id != self.bbs_id
            || stored.date != self.date
            || stored.next_id != self.next_id
            || stored.drafts != self.drafts
            || stored.subscriptions != self.subscriptions
        {
            return Err(invalid("draft store", "changed on disk; reload the packet"));
        }
        Ok(())
    }

    /// Atomically persist current drafts in the application's data directory.
    pub fn save(&self) -> crate::Res<()> {
        validate_date(&self.date)?;
        validate_subscriptions(&self.allowed_conferences, &self.capabilities, &self.subscriptions)?;
        for draft in &self.drafts {
            validate_metadata_for(draft, self.blue_wave.is_some())?;
            validate_conference(&self.allowed_conferences, draft.conference)?;
        }
        let content = toml::to_string(&Stored {
            bbs_id: self.bbs_id.clone(),
            date: self.date.clone(),
            next_id: self.next_id,
            drafts: self.drafts.clone(),
            subscriptions: self.subscriptions.clone(),
        })?;
        atomic_write(&self.path, |file| {
            file.write_all(content.as_bytes())?;
            Ok(())
        })?;
        Ok(())
    }

    /// Write a deterministic ZIP using the incoming packet's reply format.
    pub fn export(&self, destination: &Path) -> crate::Res<()> {
        if destination.canonicalize().is_ok_and(|path| path == self.source_path) {
            return Err(invalid("export path", "cannot overwrite the source packet").into());
        }
        if !self.has_exportable() {
            return Err(DraftError::Empty.into());
        }
        if let Some(packet) = &self.blue_wave {
            let replies = self.drafts.iter().map(|draft| self.blue_wave_reply(draft)).collect::<crate::Res<Vec<_>>>()?;
            let files = crate::blue_wave::reply_files(packet, &replies)?;
            atomic_write(destination, |file| {
                let mut zip = zip::ZipWriter::new(file);
                let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored)
                    .last_modified_time(zip::DateTime::default());
                for (name, data) in &files {
                    zip.start_file(name, options)?;
                    zip.write_all(data)?;
                }
                zip.finish()?;
                Ok(())
            })?;
            return Ok(());
        }
        let mut data = vec![b' '; 128];
        data[..self.bbs_id.len()].copy_from_slice(self.bbs_id.as_bytes());
        validate_date(&self.date)?;
        validate_subscriptions(&self.allowed_conferences, &self.capabilities, &self.subscriptions)?;
        for draft in &self.drafts {
            validate_conference(&self.allowed_conferences, draft.conference)?;
            let (to, from, subj, body) = validate_draft(draft, true, self.header_limit())?;
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
            write_reply(&message, &mut data, self.capabilities.qwke)?;
        }
        let mut todoor = String::new();
        for (&conference, &subscribe) in &self.subscriptions {
            let command = if subscribe { "ADD" } else { "DROP" };
            if self.capabilities.subscription_format == SubscriptionFormat::Qwke {
                todoor.push_str(&format!("AREA {conference} {}\r\n", if subscribe { "a" } else { "D" }));
                continue;
            }
            let config = self.capabilities.subscription_format == SubscriptionFormat::Config;
            let message = QWKMessage {
                status: b' ',
                msg_number: u32::from(conference),
                date_time: self.date.as_bytes().into(),
                to: self.capabilities.control_name.as_deref().unwrap_or_default().into(),
                from: encode("from", &self.sender, Some(25))?.into(),
                subj: if config { "CONFIG" } else { command }.into(),
                password: "".into(),
                ref_msg_number: 0,
                active_flag: MSG_ACTIVE,
                conference_number: conference,
                logical_message_number: 1,
                net_tag: b' ',
                text: if config { format!("{command} {conference}\n") } else { "\n".into() }.into(),
            };
            write_reply(&message, &mut data, false)?;
        }
        atomic_write(destination, |file| {
            let mut zip = zip::ZipWriter::new(file);
            let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
                .last_modified_time(zip::DateTime::default());
            zip.start_file(format!("{}.MSG", self.bbs_id), options)?;
            zip.write_all(&data)?;
            if !todoor.is_empty() {
                zip.start_file("TODOOR.EXT", options)?;
                zip.write_all(todoor.as_bytes())?;
            }
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
            let limit = self.field_limit(field);
            if length > limit {
                issues.push(DraftIssue {
                    field,
                    message: fl!(
                        LANGUAGE_LOADER,
                        "draft-issue-field-too-long",
                        field = field.label(),
                        length = length,
                        limit = limit
                    ),
                });
            }
            character_issues_for(field, value, false, self.blue_wave.is_some(), &mut issues);
        }
        let body = draft.body.replace("\r\n", "\n").replace('\r', "\n");
        if crate::editor::strip_codes(&body).trim().is_empty() {
            issues.push(DraftIssue {
                field: DraftField::Body,
                message: fl!(LANGUAGE_LOADER, "draft-issue-message-text-required"),
            });
        } else {
            for line in body.split('\n') {
                character_issues_for(DraftField::Body, line, true, self.blue_wave.is_some(), &mut issues);
            }
        }
        character_issues_for(DraftField::Tagline, &draft.tagline, false, self.blue_wave.is_some(), &mut issues);
        for line in draft.signature.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
            character_issues_for(DraftField::Signature, line, true, self.blue_wave.is_some(), &mut issues);
        }
        if issues.is_empty() && self.blue_wave.is_some() {
            if let Err(error) = self.blue_wave_reply(draft) {
                issues.push(DraftIssue {
                    field: DraftField::Conference,
                    message: error.to_string(),
                });
            }
        }
        issues
    }

    /// Suggested reply archive filename beside the packet, using its BBS ID.
    pub fn default_export_path(&self, packet_path: &Path) -> PathBuf {
        packet_path.with_file_name(format!("{}.{}", self.bbs_id, self.reply_extension().to_ascii_uppercase()))
    }

    pub fn header_limit(&self) -> usize {
        self.field_limit(DraftField::Subject)
    }

    pub fn field_limit(&self, field: DraftField) -> usize {
        if let Some(packet) = &self.blue_wave {
            if field == DraftField::Subject {
                usize::from(packet.info.subject_limit).min(71)
            } else {
                usize::from(packet.info.from_to_limit).min(35)
            }
        } else {
            self.capabilities.header_limit()
        }
    }

    pub fn reply_extension(&self) -> &'static str {
        if self.blue_wave.is_some() {
            "new"
        } else {
            "rep"
        }
    }

    pub fn is_blue_wave(&self) -> bool {
        self.blue_wave.is_some()
    }

    pub fn can_post(&self, conference: u16) -> bool {
        self.allowed_conferences.contains(&conference)
            && self
                .blue_wave
                .as_ref()
                .is_none_or(|packet| crate::blue_wave::posting_defaults(packet, conference).is_ok())
    }

    pub fn preserves_reply_references(&self) -> bool {
        self.blue_wave.as_ref().is_none_or(|packet| packet.info.uses_upl)
    }

    fn blue_wave_reply(&self, draft: &Draft) -> crate::Res<crate::blue_wave::Reply> {
        validate_conference(&self.allowed_conferences, draft.conference)?;
        let (to, from, subject, body) = validate_draft_for(
            draft,
            true,
            [
                self.field_limit(DraftField::To),
                self.field_limit(DraftField::From),
                self.field_limit(DraftField::Subject),
            ],
            true,
        )?;
        let unix_time = chrono::NaiveDateTime::parse_from_str(&draft.date, "%m-%d-%y%H:%M")?.and_utc().timestamp();
        let reply = crate::blue_wave::Reply {
            conference: draft.conference,
            reply_to: if self.preserves_reply_references() { draft.ref_number } else { 0 },
            unix_time,
            from,
            to,
            subject,
            body,
            private: draft.private,
        };
        let packet = self.blue_wave.as_ref().ok_or("Blue Wave reply requires a Blue Wave packet")?;
        crate::blue_wave::validate_reply(packet, &reply)?;
        Ok(reply)
    }

    /// ID assigned to the next new draft; imports may advance it while composing.
    pub fn next_id(&self) -> u64 {
        self.next_id
    }

    pub fn has_exportable(&self) -> bool {
        !self.drafts.is_empty() || !self.subscriptions.is_empty()
    }

    /// Queued changes as owned `(conference, subscribe)` pairs, sorted by
    /// conference number. The latest change wins.
    pub fn subscriptions(&self) -> Vec<(u16, bool)> {
        self.subscriptions.iter().map(|(&conference, &subscribe)| (conference, subscribe)).collect()
    }

    pub fn set_subscription(&mut self, conference: u16, subscribe: bool) -> crate::Res<()> {
        self.ensure_current()?;
        if !self.capabilities.supports_subscriptions() {
            return Err(invalid("subscriptions", "the packet does not advertise conference subscription support").into());
        }
        validate_conference(&self.allowed_conferences, conference)?;
        let mut next = self.clone();
        next.subscriptions.insert(conference, subscribe);
        next.save()?;
        *self = next;
        Ok(())
    }

    /// Remove a queued change without asserting the conference's current state.
    pub fn clear_subscription(&mut self, conference: u16) -> crate::Res<()> {
        self.ensure_current()?;
        validate_conference(&self.allowed_conferences, conference)?;
        let mut next = self.clone();
        if next.subscriptions.remove(&conference).is_some() {
            next.save()?;
            *self = next;
        }
        Ok(())
    }

    /// Merge a REP atomically using the same archive formats as QWK loading.
    pub fn import_rep(&mut self, path: &Path) -> crate::Res<usize> {
        let drafts = self.read_rep(path)?;
        self.import_drafts(drafts)
    }

    /// Parse and validate a reply archive without changing drafts or writing files.
    /// Door controls are excluded. Returned IDs are placeholders; `import_drafts`
    /// assigns fresh IDs when the main thread merges into its latest store.
    pub fn read_rep(&self, path: &Path) -> crate::Res<Vec<Draft>> {
        let mut reader = io::BufReader::new(File::open(path)?);
        let format = ArchiveFormat::detect(&mut reader, Some(path))?.ok_or_else(|| invalid("reply archive", "unknown archive format"))?;
        let options = ArchiveOptions::new()
            .with_max_entry_size(Some(128 * 999_999))
            .with_max_total_size(Some(256 * 999_999));
        let mut archive = UnifiedArchive::open_with_format_and_options(reader, format, options)?;
        if let Some(packet) = &self.blue_wave {
            let mut files = Vec::new();
            while let Some(entry) = archive.next_entry()? {
                if entry.original_size() > 128 * 999_999 {
                    return Err(invalid("Blue Wave replies", "member is too large").into());
                }
                files.push((entry.name().to_owned(), archive.read(&entry)?));
            }
            let mut drafts = Vec::new();
            for reply in crate::blue_wave::read_replies(packet, &files)? {
                let date = chrono::DateTime::from_timestamp(reply.unix_time, 0).ok_or_else(|| invalid("date", "invalid Blue Wave timestamp"))?;
                let draft = Draft {
                    id: 1,
                    kind: if reply.reply_to == 0 { DraftKind::New } else { DraftKind::Reply },
                    to: decode(&reply.to),
                    from: decode(&reply.from),
                    subject: decode(&reply.subject),
                    body: decode_text(&reply.body).replace("\r\n", "\n").replace('\r', "\n"),
                    conference: reply.conference,
                    ref_number: reply.reply_to,
                    private: reply.private,
                    date: date.format("%m-%d-%y%H:%M").to_string(),
                    tagline: String::new(),
                    signature: String::new(),
                };
                self.blue_wave_reply(&draft)?;
                drafts.push(draft);
            }
            return Ok(drafts);
        }
        let expected = format!("{}.MSG", self.bbs_id);
        let mut messages = None;
        let mut commands_seen = false;
        while let Some(entry) = archive.next_entry()? {
            let name = entry.name().to_ascii_uppercase();
            if name.contains(['/', '\\']) || !matches!(name.as_str(), "TODOOR.EXT") && name != expected {
                return Err(invalid("REP archive", "unexpected member or BBS ID").into());
            }
            if entry.original_size() > 128 * 999_999 {
                return Err(invalid("REP archive", "member is too large").into());
            }
            let bytes = archive.read(&entry)?;
            if name == expected {
                if messages.replace(bytes).is_some() {
                    return Err(invalid("REP archive", "duplicate message member").into());
                }
            } else {
                if commands_seen || !bytes.is_ascii() || bytes.iter().any(|b| b.is_ascii_control() && !matches!(b, b'\r' | b'\n' | b'\t')) {
                    return Err(invalid("REP archive", "duplicate or invalid door command file").into());
                }
                commands_seen = true;
            }
        }
        let data = messages.ok_or_else(|| invalid("REP archive", "BBS message member is missing"))?;
        if data.len() < 128 || data.len() % 128 != 0 || !trim_padding(&data[..128]).eq_ignore_ascii_case(self.bbs_id.as_bytes()) {
            return Err(invalid("REP archive", "invalid or mismatched packet header").into());
        }
        let mut drafts = Vec::new();
        let mut offset = 128;
        while offset < data.len() {
            let header = &data[offset..offset + 128];
            let block_count = strict_number(&header[116..122], false)?;
            let blocks = usize::try_from(block_count).map_err(|_| invalid("REP", "block count is too large"))?;
            let end = offset
                .checked_add(blocks.checked_mul(128).ok_or_else(|| invalid("REP", "invalid block count"))?)
                .filter(|&end| end <= data.len())
                .ok_or_else(|| invalid("REP", "truncated message"))?;
            let number = strict_number(&header[1..8], false)?;
            let conference = u16::try_from(number).map_err(|_| invalid("REP", "conference number is too large"))?;
            let reference = strict_number(&header[108..116], true)?;
            if blocks < 2 || header[122] != MSG_ACTIVE || !matches!(header[0], b' ' | b'-' | b'*' | b'+' | b'~' | b'`') {
                return Err(invalid("REP", "invalid message header").into());
            }
            // REP's ASCII number is authoritative; readers may leave the
            // binary conference unset. Normalize numeric padding for the codec.
            let mut normalized = header.to_vec();
            for (range, value) in [(1..8, number), (108..116, reference), (116..122, block_count)] {
                normalized[range.clone()].fill(b' ');
                let value = value.to_string();
                normalized[range.start..range.start + value.len()].copy_from_slice(value.as_bytes());
            }
            let input = io::Read::chain(Cursor::new(normalized), Cursor::new(&data[offset + 128..end]));
            let mut message = QWKMessage::read(input, false).map_err(|e| DraftError::Qwk(e.to_string()))?;
            let mut body = trim_padding(&data[offset + 128..end]).to_vec();
            for byte in &mut body {
                if *byte == 0xE3 {
                    *byte = b'\n';
                }
            }
            extract_kludges(&mut message, &mut body, self.capabilities.qwke)?;
            let recipient = decode(&message.to);
            let control_recipient = self
                .capabilities
                .control_name
                .as_deref()
                .is_some_and(|name| recipient.eq_ignore_ascii_case(name))
                || ["QMAIL", "MARKMAIL", "TOMCAT", "CONTROL"]
                    .iter()
                    .any(|name| recipient.eq_ignore_ascii_case(name));
            let subject = decode(&message.subj);
            let command = subject.split_ascii_whitespace().next().unwrap_or_default();
            let control = control_recipient
                && ["ADD", "DROP", "RESET", "CONFIG", "BLTS", "FILES", "WELCOME", "GOODBYE"]
                    .iter()
                    .any(|name| command.eq_ignore_ascii_case(name));
            let draft = Draft {
                id: 1,
                kind: if message.ref_msg_number == 0 { DraftKind::New } else { DraftKind::Reply },
                to: recipient,
                from: decode(&message.from),
                subject,
                body: decode_text(&body),
                conference,
                ref_number: message.ref_msg_number,
                private: matches!(message.status, b'*' | b'+' | b'~' | b'`'),
                date: String::from_utf8(message.date_time.to_vec()).map_err(|_| invalid("date", "non-ASCII date"))?,
                tagline: String::new(),
                signature: String::new(),
            };
            validate_metadata(&draft)?;
            if !control {
                validate_conference(&self.allowed_conferences, draft.conference)?;
                validate_draft(&draft, true, self.header_limit())?;
                drafts.push(draft);
            }
            offset = end;
        }
        Ok(drafts)
    }

    /// Validate every draft and atomically append to the latest store. Caller
    /// IDs are ignored; concurrent composer/subscription edits remain intact.
    pub fn import_drafts(&mut self, drafts: Vec<Draft>) -> crate::Res<usize> {
        self.ensure_current()?;
        let imported = drafts.len();
        let mut next = self.clone();
        for mut draft in drafts {
            draft.id = next.next_id;
            validate_conference(&self.allowed_conferences, draft.conference)?;
            if self.blue_wave.is_some() {
                self.blue_wave_reply(&draft)?;
            } else {
                validate_draft(&draft, true, self.header_limit())?;
            }
            next.next_id = next.next_id.checked_add(1).ok_or_else(|| invalid("draft ID", "exhausted"))?;
            next.drafts.push(draft);
        }
        self.ensure_current()?;
        if imported > 0 {
            next.save()?;
            *self = next;
        }
        Ok(imported)
    }
}

fn invalid(field: &'static str, reason: impl Into<String>) -> DraftError {
    DraftError::Invalid { field, reason: reason.into() }
}

fn validate_subscriptions(allowed: &BTreeSet<u16>, capabilities: &Capabilities, queued: &BTreeMap<u16, bool>) -> Result<()> {
    if !queued.is_empty() && !capabilities.supports_subscriptions() {
        return Err(invalid("subscriptions", "the packet does not advertise conference subscription support"));
    }
    for &conference in queued.keys() {
        validate_conference(allowed, conference)?;
    }
    Ok(())
}

/// The codec truncates fixed headers and emits QWKE kludges, but writes a
/// header-only record for an empty body. REP records need at least one body block.
fn write_reply(message: &QWKMessage, data: &mut Vec<u8>, extended: bool) -> Result<()> {
    let mut message = message.clone();
    if extended
        && [&message.to, &message.from, &message.subj].iter().all(|field| field.len() <= 25)
        && [b"To: ".as_slice(), b"From: ", b"Subject: "]
            .iter()
            .any(|prefix| message.text.starts_with(prefix))
    {
        // Disambiguate author text from protocol kludges, even when all real
        // headers fit QWK. A blank separator ends the kludge section.
        let mut text = Vec::new();
        for (prefix, value) in [(b"To: ".as_slice(), &message.to), (b"From: ", &message.from), (b"Subject: ", &message.subj)] {
            text.extend(prefix);
            text.extend(value.as_slice());
            text.push(b'\n');
        }
        text.push(b'\n');
        text.extend(message.text.as_slice());
        message.text = text.into();
    }
    if message.text.is_empty() {
        message.text = " ".into();
    }
    let start = data.len();
    message.write(&mut *data, extended).map_err(|e| DraftError::Qwk(e.to_string()))?;
    if (data.len() - start) / 128 > 999_999 {
        return Err(invalid("body", "QWK block count exceeds six digits"));
    }
    Ok(())
}

fn strict_number(bytes: &[u8], empty_allowed: bool) -> Result<u32> {
    let text = std::str::from_utf8(bytes).map_err(|_| invalid("REP", "non-ASCII numeric field"))?;
    let text = text.trim_matches(' ');
    if text.is_empty() && empty_allowed {
        return Ok(0);
    }
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid("REP", "malformed numeric field"));
    }
    text.parse().map_err(|_| invalid("REP", "numeric field overflow"))
}

fn trim_padding(bytes: &[u8]) -> &[u8] {
    let end = bytes.iter().rposition(|&b| b != b' ' && b != 0).map_or(0, |i| i + 1);
    &bytes[..end]
}

fn decode_text(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| BufferType::CP437.convert_to_unicode(char::from(b))).collect()
}

fn extract_kludges(message: &mut QWKMessage, body: &mut Vec<u8>, extended: bool) -> Result<()> {
    let mut consumed = 0;
    let mut seen = BTreeSet::new();
    while let Some(end) = body[consumed..].iter().position(|&b| matches!(b, b'\n' | b'\r')) {
        let line = &body[consumed..consumed + end];
        let (key, value) = if let Some(value) = line.strip_prefix(b"To: ") {
            ("to", value)
        } else if let Some(value) = line.strip_prefix(b"From: ") {
            ("from", value)
        } else if let Some(value) = line.strip_prefix(b"Subject: ") {
            ("subject", value)
        } else {
            break;
        };
        if !extended {
            let header = match key {
                "to" => &message.to,
                "from" => &message.from,
                _ => &message.subj,
            };
            // Without advertised extensions, ordinary body text must not be
            // mistaken for a kludge. Recognizable long fields are still
            // rejected by the plain-QWK limit rather than silently truncated.
            if value.len() <= 25 || header.len() != 25 || value[..25] != header.as_slice()[..25] {
                break;
            }
        }
        if value.len() > 255 || !seen.insert(key) {
            return Err(invalid("REP", "duplicate or oversized QWKE kludge"));
        }
        match key {
            "to" => message.to = value.into(),
            "from" => message.from = value.into(),
            _ => message.subj = value.into(),
        }
        consumed += end + 1;
        if body.get(consumed - 1) == Some(&b'\r') && body.get(consumed) == Some(&b'\n') {
            consumed += 1;
        }
    }
    if consumed > 0 {
        // Remove exactly the protocol separator, not the author's leading blank lines.
        if body.get(consumed) == Some(&b'\n') {
            consumed += 1;
        } else if body.get(consumed) == Some(&b'\r') {
            consumed += 1;
            if body.get(consumed) == Some(&b'\n') {
                consumed += 1;
            }
        }
        body.drain(..consumed);
    }
    Ok(())
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
    if raw.is_empty()
        || raw.len() > 8
        || !raw
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || package.blue_wave.is_some() && matches!(byte, b'_' | b'-'))
    {
        return Err(invalid("BBS ID", "expected 1–8 safe ASCII BBS identifier characters"));
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
fn character_issues_for(field: DraftField, value: &str, allow_escape: bool, blue_wave: bool, issues: &mut Vec<DraftIssue>) {
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
                Some(byte) if !blue_wave && (byte as u32 == 0xE3 || ch == '\u{e3}') => {
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
    encode_for(field, value, max, false)
}

fn encode_for(field: &'static str, value: &str, max: Option<usize>, blue_wave: bool) -> Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(value.len());
    for ch in value.chars() {
        if field == "body" && ch == '\x1b' {
            bytes.push(0x1b);
            continue;
        }
        if ch.is_control() || !blue_wave && ch == '\u{e3}' {
            return Err(invalid(field, "control characters are not allowed"));
        }
        let byte = BufferType::CP437
            .try_convert_from_unicode(ch)
            .ok_or_else(|| invalid(field, format!("{ch:?} cannot be encoded as CP437")))? as u8;
        if !blue_wave && byte == 0xE3 {
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
    validate_metadata_for(d, false)
}

fn validate_metadata_for(d: &Draft, blue_wave: bool) -> Result<()> {
    if d.id == 0 {
        return Err(invalid("draft ID", "must be prepared before saving"));
    }
    validate_date(&d.date)?;
    if !blue_wave && d.ref_number > 99_999_999 {
        return Err(invalid("reference number", "exceeds eight digits"));
    }
    Ok(())
}

fn validate_draft(d: &Draft, complete: bool, header_limit: usize) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)> {
    validate_draft_for(d, complete, [header_limit; 3], false)
}

fn validate_draft_for(d: &Draft, complete: bool, limits: [usize; 3], blue_wave: bool) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)> {
    validate_metadata_for(d, blue_wave)?;
    let text = d.text();
    if text.len() > if blue_wave { u32::MAX as usize - 1 } else { 128 * 999_998 } {
        return Err(invalid(
            "body",
            if blue_wave {
                "Blue Wave body length exceeds its 32-bit field"
            } else {
                "QWK block count exceeds six digits"
            },
        ));
    }
    let to = encode_for("to", &d.to, Some(limits[0]), blue_wave)?;
    let from = encode_for("from", &d.from, Some(limits[1]), blue_wave)?;
    let subject = encode_for("subject", &d.subject, Some(limits[2]), blue_wave)?;
    // Validate even when the signature is already present in the body.
    for line in d.signature.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        encode_for("body", line, None, blue_wave)?;
    }
    let mut body = Vec::new();
    for (index, line) in text.replace("\r\n", "\n").replace('\r', "\n").split('\n').enumerate() {
        if index > 0 {
            body.push(b'\n');
        }
        body.extend(encode_for("body", line, None, blue_wave)?);
    }
    if complete && (d.to.trim().is_empty() || d.from.trim().is_empty() || d.subject.trim().is_empty() || crate::editor::strip_codes(&d.body).trim().is_empty())
    {
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
            signature: String::new(),
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
            signature: String::new(),
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
            signature: String::new(),
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
            signature: String::new(),
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

    fn capable_package(package: &QwkPackage, advertisement: &[u8]) -> QwkPackage {
        let mut package = package.clone();
        package.capabilities = Capabilities::parse(&[("DOOR.ID".into(), advertisement.to_vec())]);
        package
    }

    fn rep_bytes(path: &Path) -> Vec<u8> {
        let mut archive = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
        let mut data = Vec::new();
        archive.by_name("TEST.MSG").unwrap().read_to_end(&mut data).unwrap();
        data
    }

    fn write_rep(path: &Path, name: &str, data: &[u8]) {
        let mut archive = zip::ZipWriter::new(File::create(path).unwrap());
        archive.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        archive.write_all(data).unwrap();
        archive.finish().unwrap();
    }

    #[test]
    fn qwke_roundtrips_extended_fields_privacy_date_reference_and_ansi() {
        let (dir, package) = crate::qwk::tests::load();
        let package = capable_package(&package, b"CONTROLTYPE=QWKE");
        let packet = dir.path().join("TEST.QWK");
        let mut store = DraftStore::open(&packet, &package).unwrap();
        assert_eq!(store.header_limit(), 255);
        let mut draft = reply();
        draft.to = "Long recipient name with accented é".into();
        draft.from = "Long sender name exceeding twenty five".into();
        draft.subject = "A subject that exceeds the standard header".into();
        draft.body = "\n\x1b[1;31mCafé\x1b[0m\nSecond\n".into();
        draft.tagline.clear();
        store.insert(draft.clone()).unwrap();
        let path = dir.path().join("TEST.REP");
        store.export(&path).unwrap();
        let data = rep_bytes(&path);
        let message = QWKMessage::read(Cursor::new(&data[128..]), true).unwrap();
        assert_eq!(decode(&message.to), draft.to);
        assert_eq!(decode(&message.from), draft.from);
        assert_eq!(decode(&message.subj), draft.subject);
        let mut imported = DraftStore::open_in(&packet, &package, &dir.path().join("imported")).unwrap();
        assert_eq!(imported.import_rep(&path).unwrap(), 1);
        let actual = &imported.drafts[0];
        assert_eq!(actual.to, draft.to);
        assert_eq!(actual.from, draft.from);
        assert_eq!(actual.subject, draft.subject);
        assert_eq!(actual.body, draft.text());
        assert_eq!(actual.date, draft.date);
        assert_eq!(actual.ref_number, draft.ref_number);
        assert_eq!(actual.private, draft.private);
        assert_eq!(
            DraftStore::open_in(&packet, &package, &dir.path().join("imported")).unwrap().drafts,
            imported.drafts
        );
        let plain = DraftStore::open_in(&packet, &crate::qwk::tests::load().1, &dir.path().join("plain")).unwrap();
        assert_eq!(plain.header_limit(), 25);
        assert!(plain.issues(&draft).iter().any(|issue| issue.field == DraftField::Subject));
        assert!(validate_draft(&draft, true, 25).is_err());
        draft.subject = "é".repeat(255);
        assert!(validate_draft(&draft, true, 255).is_ok());
        store.update(draft.clone()).unwrap();
        store.export(&path).unwrap();
        assert_eq!(imported.read_rep(&path).unwrap()[0].subject, draft.subject);
        draft.subject = "x".repeat(256);
        assert!(validate_draft(&draft, true, 255).is_err());
    }

    #[test]
    fn subscriptions_are_durable_door_specific_and_commands_are_not_imported() {
        let (dir, package) = crate::qwk::tests::load();
        let packet = dir.path().join("TEST.QWK");
        let mut unsupported = DraftStore::open_in(&packet, &package, &dir.path().join("unsupported")).unwrap();
        assert!(!unsupported.has_exportable());
        assert!(unsupported.set_subscription(1, true).is_err());
        assert!(!unsupported.path.exists());
        for (name, advertisement, expected_subject, expected_body) in [
            ("subject", &b"CONTROLNAME=CUSTOM\nCONTROLTYPE=ADD\nCONTROLTYPE=DROP"[..], "ADD", "\n"),
            (
                "config",
                &b"DOOR=Qmail\nCONTROLNAME=QMAIL\nCONTROLTYPE=ADD\nCONTROLTYPE=DROP"[..],
                "CONFIG",
                "ADD 1\n",
            ),
        ] {
            let package = capable_package(&package, advertisement);
            let directory = dir.path().join(name);
            let mut store = DraftStore::open_in(&packet, &package, &directory).unwrap();
            store.set_subscription(1, false).unwrap();
            store.set_subscription(1, true).unwrap();
            store.set_subscription(2, false).unwrap();
            assert_eq!(store.subscriptions(), vec![(1, true), (2, false)]);
            assert!(store.has_exportable());
            assert!(store.set_subscription(999, true).is_err());
            let mut loaded = DraftStore::open_in(&packet, &package, &directory).unwrap();
            assert_eq!(loaded.subscriptions(), store.subscriptions());
            let rep = directory.join("TEST.REP");
            loaded.export(&rep).unwrap();
            let data = rep_bytes(&rep);
            let message = QWKMessage::read(Cursor::new(&data[128..]), false).unwrap();
            assert_eq!(decode(&message.to), package.capabilities.control_name.clone().unwrap());
            assert_eq!(decode(&message.subj), expected_subject);
            assert!(message.text.starts_with(expected_body.as_bytes()));
            assert_eq!(loaded.import_rep(&rep).unwrap(), 0);
            assert!(loaded.drafts.is_empty());
            store.set_subscription(1, false).unwrap();
            assert!(loaded.set_subscription(2, true).is_err());
        }
        let package = capable_package(&package, b"CONTROLTYPE=QWKE");
        let mut store = DraftStore::open_in(&packet, &package, &dir.path().join("qwke")).unwrap();
        store.set_subscription(1, true).unwrap();
        store.set_subscription(2, false).unwrap();
        let rep = dir.path().join("QWKE.REP");
        store.export(&rep).unwrap();
        let mut archive = zip::ZipArchive::new(File::open(&rep).unwrap()).unwrap();
        let mut commands = String::new();
        archive.by_name("TODOOR.EXT").unwrap().read_to_string(&mut commands).unwrap();
        assert_eq!(commands, "AREA 1 a\r\nAREA 2 D\r\n");
        assert_eq!(store.import_rep(&rep).unwrap(), 0);
    }

    #[test]
    fn rep_validation_is_atomic_for_malformed_truncated_and_mismatched_packets() {
        let (dir, package) = crate::qwk::tests::load();
        let packet = dir.path().join("TEST.QWK");
        let mut store = DraftStore::open(&packet, &package).unwrap();
        store.insert(reply()).unwrap();
        let rep = dir.path().join("TEST.REP");
        store.export(&rep).unwrap();
        let valid = rep_bytes(&rep);
        let before = fs::read(&store.path).unwrap();
        let drafts = store.drafts.clone();
        let bad = dir.path().join("BAD.REP");
        for (offset, replacement) in [(0, b'X'), (129, b'x'), (128 + 116, b'0'), (128 + 121, b'9'), (128 + 122, 226)] {
            let mut data = valid.clone();
            data[offset] = replacement;
            write_rep(&bad, "TEST.MSG", &data);
            assert!(store.import_rep(&bad).is_err(), "offset {offset}");
            assert_eq!(fs::read(&store.path).unwrap(), before);
            assert_eq!(store.drafts, drafts);
        }
        for data in [&valid[..127], &valid[..valid.len() - 1], &valid[..valid.len() - 128]] {
            write_rep(&bad, "TEST.MSG", data);
            assert!(store.import_rep(&bad).is_err());
        }
        write_rep(&bad, "OTHER.MSG", &valid);
        assert!(store.import_rep(&bad).is_err());
        write_rep(&bad, "../TEST.MSG", &valid);
        assert!(store.import_rep(&bad).is_err());
        // A valid record followed by a bad one must not partially import.
        let mut data = valid.clone();
        data.extend_from_slice(&valid[128..]);
        let bad_header = valid.len();
        data[bad_header + 8] = b'X';
        write_rep(&bad, "TEST.MSG", &data);
        assert!(store.import_rep(&bad).is_err());
        assert_eq!(fs::read(&store.path).unwrap(), before);
        assert_eq!(store.drafts, drafts);
        fs::write(&bad, b"not an archive").unwrap();
        assert!(store.import_rep(&bad).is_err());
    }

    #[test]
    fn rep_import_accepts_numeric_padding_and_uses_ascii_conference() {
        let (dir, package) = crate::qwk::tests::load();
        let packet = dir.path().join("TEST.QWK");
        let mut store = DraftStore::open(&packet, &package).unwrap();
        let draft = reply();
        store.insert(draft.clone()).unwrap();
        let path = dir.path().join("TEST.REP");
        store.export(&path).unwrap();
        let original = rep_bytes(&path);
        for binary in [[0, 0], [b' ', b' '], [2, 0]] {
            let mut data = original.clone();
            data[128 + 123..128 + 125].copy_from_slice(&binary);
            for range in [129..136, 236..244, 244..250] {
                let value = std::str::from_utf8(&data[range.clone()]).unwrap().trim().to_owned();
                data[range.clone()].fill(b' ');
                data[range.end - value.len()..range.end].copy_from_slice(value.as_bytes());
            }
            write_rep(&path, "TEST.MSG", &data);
            let imported = store.read_rep(&path).unwrap();
            assert_eq!(imported.len(), 1);
            assert_eq!(imported[0].conference, draft.conference);
            assert_eq!(imported[0].ref_number, draft.ref_number);
            assert_eq!(imported[0].body, draft.text());
        }
    }

    #[test]
    fn rep_import_keeps_ordinary_mail_to_control_recipients() {
        let (dir, package) = crate::qwk::tests::load();
        let packet = dir.path().join("TEST.QWK");
        for recipient in ["QMAIL", "CUSTOM"] {
            let package = capable_package(&package, b"CONTROLNAME=CUSTOM\nCONTROLTYPE=ADD\nCONTROLTYPE=DROP");
            let mut store = DraftStore::open_in(&packet, &package, &dir.path().join(recipient)).unwrap();
            let mut draft = reply();
            draft.to = recipient.into();
            draft.subject = "A normal message".into();
            store.insert(draft.clone()).unwrap();
            let path = dir.path().join(format!("{recipient}.REP"));
            store.export(&path).unwrap();
            let imported = store.read_rep(&path).unwrap();
            assert_eq!(imported.len(), 1);
            assert_eq!(imported[0].to, recipient);
            assert_eq!(imported[0].subject, draft.subject);
            assert_eq!(imported[0].body, draft.text());
        }
    }

    #[test]
    fn signature_is_validated_and_appended_once_before_tagline() {
        let mut draft = reply();
        draft.body = "Text".into();
        draft.signature = "\x1b[32mSigned\x1b[0m\nReader".into();
        assert_eq!(crate::editor::strip_codes(&draft.text()), "Text\n\nSigned\nReader\n\n... Bye");
        draft.body = "Text\n\n\x1b[32mSigned\x1b[0m\nReader".into();
        assert_eq!(draft.text().matches("Signed").count(), 1);
        assert!(validate_draft(&draft, true, 25).is_ok());
        draft.signature = "🌍".into();
        assert!(validate_draft(&draft, true, 25).is_err());
        draft.signature = "\0".into();
        assert!(validate_draft(&draft, true, 25).is_err());
        let old = toml::to_string(&reply()).unwrap();
        assert!(!old.contains("signature"));
        assert_eq!(toml::from_str::<Draft>(&old).unwrap().signature, "");
    }

    #[test]
    fn legacy_store_defaults_remain_editable_after_loading() {
        let (dir, package) = crate::qwk::tests::load();
        let packet = dir.path().join("TEST.QWK");
        let mut store = DraftStore::open(&packet, &package).unwrap();
        store.insert(reply()).unwrap();
        let mut legacy: toml::Value = toml::from_str(&fs::read_to_string(&store.path).unwrap()).unwrap();
        let table = legacy.as_table_mut().unwrap();
        table.remove("next_id");
        table.remove("subscriptions");
        for draft in table.get_mut("drafts").unwrap().as_array_mut().unwrap() {
            let draft = draft.as_table_mut().unwrap();
            draft.remove("id");
            draft.remove("date");
            draft.remove("signature");
        }
        fs::write(&store.path, toml::to_string(&legacy).unwrap()).unwrap();
        let mut loaded = DraftStore::open(&packet, &package).unwrap();
        assert_eq!(loaded.drafts[0].signature, "");
        assert!(loaded.subscriptions().is_empty());
        let draft = loaded.prepare(&package, Compose::New { conference: 1 }).unwrap();
        loaded.insert(draft).unwrap();
        assert_eq!(DraftStore::open(&packet, &package).unwrap().drafts.len(), 2);
    }

    #[test]
    fn rep_import_rejects_duplicate_members_and_corrupt_archive_checksums() {
        let (dir, package) = crate::qwk::tests::load();
        let packet = dir.path().join("TEST.QWK");
        let mut store = DraftStore::open(&packet, &package).unwrap();
        store.insert(reply()).unwrap();
        let rep = dir.path().join("TEST.REP");
        store.export(&rep).unwrap();
        let bytes = rep_bytes(&rep);
        let bad = dir.path().join("BAD.REP");
        let mut archive = zip::ZipWriter::new(File::create(&bad).unwrap());
        for name in ["TEST.MSG", "test.msg"] {
            archive.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            archive.write_all(&bytes).unwrap();
        }
        archive.finish().unwrap();
        assert!(store.import_rep(&bad).is_err());
        let before = fs::read(&store.path).unwrap();
        let mut packed = fs::read(&rep).unwrap();
        let position = packed.windows(bytes.len()).position(|window| window == bytes).unwrap();
        packed[position + 128 + 128] ^= 1;
        fs::write(&bad, &packed).unwrap();
        assert!(store.import_rep(&bad).is_err());
        fs::write(&bad, &packed[..packed.len() - 20]).unwrap();
        assert!(store.import_rep(&bad).is_err());
        assert_eq!(fs::read(&store.path).unwrap(), before);
    }

    #[test]
    fn rep_import_accepts_non_zip_archives_by_content() {
        let (dir, package) = crate::qwk::tests::load();
        let packet = dir.path().join("TEST.QWK");
        let mut store = DraftStore::open(&packet, &package).unwrap();
        store.insert(reply()).unwrap();
        let rep = dir.path().join("TEST.REP");
        store.export(&rep).unwrap();
        let bytes = rep_bytes(&rep);
        // Minimal POSIX tar member: archive detection must not depend on .REP.
        let mut header = vec![0u8; 512];
        header[..8].copy_from_slice(b"TEST.MSG");
        header[100..108].copy_from_slice(b"0000644\0");
        header[108..116].copy_from_slice(b"0000000\0");
        header[116..124].copy_from_slice(b"0000000\0");
        header[124..136].copy_from_slice(format!("{:011o}\0", bytes.len()).as_bytes());
        header[136..148].copy_from_slice(b"00000000000\0");
        header[148..156].fill(b' ');
        header[156] = b'0';
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");
        let checksum: u32 = header.iter().map(|&byte| u32::from(byte)).sum();
        header[148..156].copy_from_slice(format!("{checksum:06o}\0 ").as_bytes());
        let mut tar = header;
        tar.extend(&bytes);
        tar.resize(tar.len().div_ceil(512) * 512 + 1024, 0);
        let path = dir.path().join("TAR.REP");
        fs::write(&path, tar).unwrap();
        assert_eq!(store.import_rep(&path).unwrap(), 1);
        assert_eq!(store.drafts.len(), 2);
    }

    #[test]
    fn kludge_looking_author_text_roundtrips_without_becoming_headers() {
        let (dir, package) = crate::qwk::tests::load();
        let packet = dir.path().join("TEST.QWK");
        for extended in [false, true] {
            let mut package = package.clone();
            package.capabilities.qwke = extended;
            let directory = dir.path().join(if extended { "extended" } else { "plain" });
            let mut store = DraftStore::open_in(&packet, &package, &directory).unwrap();
            let mut draft = reply();
            draft.tagline.clear();
            draft.body = "To: A long line of ordinary author text\nSubject: Not a header\n\nBody".into();
            store.insert(draft.clone()).unwrap();
            let rep = directory.join("TEST.REP");
            store.export(&rep).unwrap();
            assert_eq!(store.import_rep(&rep).unwrap(), 1);
            assert_eq!(store.drafts[1].to, draft.to);
            assert_eq!(store.drafts[1].subject, draft.subject);
            assert_eq!(store.drafts[1].body, draft.text());
        }
    }

    #[test]
    fn background_rep_parsing_is_read_only_and_merge_preserves_latest_edits() {
        let (dir, package) = crate::qwk::tests::load();
        let package = capable_package(&package, b"CONTROLNAME=CUSTOM\nCONTROLTYPE=ADD\nCONTROLTYPE=DROP");
        let packet = dir.path().join("TEST.QWK");
        let mut store = DraftStore::open(&packet, &package).unwrap();
        store.insert(reply()).unwrap();
        let rep = dir.path().join("TEST.REP");
        store.export(&rep).unwrap();
        let mut worker = store.clone();
        let before = fs::read(&store.path).unwrap();
        let parsed = worker.read_rep(&rep).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(fs::read(&store.path).unwrap(), before);
        assert_eq!(worker.drafts, store.drafts);

        let mut edited = store.drafts[0].clone();
        edited.body = "Composer edit made while parsing".into();
        store.update(edited.clone()).unwrap();
        store.set_subscription(2, false).unwrap();
        let mut composed = store.prepare(&package, Compose::New { conference: 1 }).unwrap();
        composed.to = "ALL".into();
        composed.subject = "New while parsing".into();
        composed.body = "New text".into();
        store.insert(composed.clone()).unwrap();
        // A worker snapshot can finish parsing after disk changes, but cannot
        // persist that stale snapshot over the current main-thread store.
        assert_eq!(worker.read_rep(&rep).unwrap(), parsed);
        assert!(worker.import_drafts(parsed.clone()).is_err());
        assert_eq!(store.import_drafts(parsed).unwrap(), 1);
        assert_eq!(store.drafts[0], edited);
        assert_eq!(store.drafts[1], composed);
        assert_eq!(store.drafts[2].id, 3);
        assert_eq!(store.subscriptions(), vec![(2, false)]);
        let loaded = DraftStore::open(&packet, &package).unwrap();
        assert_eq!(loaded.drafts, store.drafts);
        assert_eq!(loaded.subscriptions(), store.subscriptions());
    }

    #[test]
    fn imported_draft_batches_are_revalidated_before_atomic_append() {
        let (dir, package) = crate::qwk::tests::load();
        let packet = dir.path().join("TEST.QWK");
        let mut store = DraftStore::open(&packet, &package).unwrap();
        store.insert(reply()).unwrap();
        let before = fs::read(&store.path).unwrap();
        let mut malformed = reply();
        malformed.conference = 999;
        assert!(store.import_drafts(vec![reply(), malformed]).is_err());
        assert_eq!(store.drafts, vec![reply()]);
        assert_eq!(fs::read(&store.path).unwrap(), before);
        let mut draft = reply();
        draft.id = 0;
        assert_eq!(store.import_drafts(vec![draft]).unwrap(), 1);
        assert_eq!(store.drafts[1].id, 2);
    }

    #[test]
    fn clearing_subscription_is_durable_atomic_and_not_a_subscription_state() {
        let (dir, package) = crate::qwk::tests::load();
        let package = capable_package(&package, b"CONTROLNAME=CUSTOM\nCONTROLTYPE=ADD\nCONTROLTYPE=DROP");
        let packet = dir.path().join("TEST.QWK");
        let mut store = DraftStore::open(&packet, &package).unwrap();
        store.clear_subscription(1).unwrap();
        assert!(!store.path.exists());
        store.set_subscription(1, true).unwrap();
        store.set_subscription(2, false).unwrap();
        let mut stale = store.clone();
        store.clear_subscription(1).unwrap();
        assert_eq!(store.subscriptions(), vec![(2, false)]);
        assert!(stale.clear_subscription(2).is_err());
        assert_eq!(DraftStore::open(&packet, &package).unwrap().subscriptions(), store.subscriptions());
        assert!(store.clear_subscription(999).is_err());
        store.clear_subscription(2).unwrap();
        assert!(store.subscriptions().is_empty());
        assert!(!store.has_exportable());
        assert!(!DraftStore::open(&packet, &package).unwrap().has_exportable());
    }
}
