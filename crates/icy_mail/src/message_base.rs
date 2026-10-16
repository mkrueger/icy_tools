//! JAM message bases, opened on their own (`.jhr`) or as the message areas of an IcyBoard
//! installation (`icboard.toml`).
//!
//! Unlike packets, a message base is read in place: headers are indexed once when it is opened,
//! bodies are read on demand under JAM's shared lock, and posted messages are appended under its
//! exclusive lock, so IcyBoard and tossers can keep working on the same files.
//!
//! IcyBoard keeps all text as UTF-8: it converts tossed mail from CP437 (keeping the packet's
//! `CHRS` line) and converts back when it sends mail out. Its areas are read and written as UTF-8.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

use bstr::BString;
use jamjam::jam::attributes;
use jamjam::jam::last_read_storage::JamLastReadStorage;
use jamjam::jam::msg_header::{JamMessageHeader, MessageSubfield, SubfieldType};
use jamjam::jam::pack::PackOptions;
use jamjam::jam::{JamMessage, JamMessageBase};
use jamjam::qwk::qwk_message::{QwkMessage, MSG_ACTIVE};
use jamjam::util::echomail::EchomailAddress;

use crate::Res;

const HEADER_EXTENSION: &str = "jhr";
const ICYBOARD_CONFIG: &str = "icboard.toml";
const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";
/// JAM's "no CRC", the seed of its CRC-32.
const NO_CRC: u32 = 0xFFFF_FFFF;
/// Attribute bits that say what kind of area a message belongs to.
const TYPE_BITS: u32 = attributes::MSG_TYPELOCAL | attributes::MSG_TYPEECHO | attributes::MSG_TYPENET;

/// Longest sender and recipient names FidoNet tools accept (36 bytes including the terminator).
pub const NAME_LIMIT: usize = 35;
/// Longest subject FidoNet tools accept (72 bytes including the terminator).
pub const SUBJECT_LIMIT: usize = 71;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseKind {
    /// A single `.jhr` base.
    Single,
    /// Every message area of an IcyBoard installation.
    IcyBoard,
}

/// A message area: one JAM base.
pub struct Area {
    /// Conference number the rest of icy_mail knows the area by.
    pub number: u16,
    pub name: String,
    /// Base path; jamjam replaces its extension with the one of each file. IcyBoard configures
    /// it without one; a single base keeps `.jhr`, so dots in names like `r50.sysop` survive.
    pub path: PathBuf,
    pub read_only: bool,
    /// Echomail area; posts are flagged for the tosser to send out.
    pub echo: bool,
    /// Netmail area: messages travel to one FidoNet address, given in an `AddressD` subfield.
    pub netmail: bool,
    /// Written like IcyBoard does (UTF-8 text, LF line ends, UTC date with an RFC 3339 subfield)
    /// instead of the JAM specification's CR line ends and local time.
    pub icyboard: bool,
    base: Mutex<Option<JamMessageBase>>,
    /// The user's lastread pointer, `0` before anything was read.
    last_read: AtomicU32,
}

/// Header fields of one message, read when the base is opened.
#[derive(Clone, Debug)]
pub struct Message {
    pub area: u16,
    pub number: u32,
    pub reply_to: u32,
    pub from: Vec<u8>,
    pub to: Vec<u8>,
    pub subject: Vec<u8>,
    pub date: Option<chrono::NaiveDateTime>,
    pub private: bool,
    /// Written on this system rather than received from a network.
    pub local: bool,
    /// Always `0`: bodies are only read when a message is shown.
    pub lines: u32,
    /// Charset the message's `CHRS` kludge declares, UTF-8 in IcyBoard areas.
    pub charset: Option<crate::charset::Charset>,
    /// Text and header fields are UTF-8 whatever a kludge says, as in IcyBoard areas.
    pub utf8: bool,
    /// FidoNet address the message was written at (`Address0`).
    pub origin: Option<String>,
    /// FidoNet address a netmail is sent to (`AddressD`).
    pub destination: Option<String>,
}

/// A message to append to an area. Fields hold the bytes as they are stored.
#[derive(Clone, Debug)]
pub struct Post {
    pub area: u16,
    pub from: Vec<u8>,
    pub to: Vec<u8>,
    pub subject: Vec<u8>,
    /// Lines separated by LF.
    pub body: Vec<u8>,
    pub private: bool,
    /// Number of the message this one answers in the same area, `0` for none.
    pub reply_to: u32,
    pub utf8: bool,
    /// Destination of a netmail, `zone:net/node[.point]`.
    pub address: Option<String>,
}

/// What [`MessageBase::pack`] removes besides the messages marked deleted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PackChoice {
    /// Messages written before this time.
    pub purge_before: Option<chrono::DateTime<chrono::Utc>>,
    /// Private messages their recipient has read.
    pub purge_received_private: bool,
}

/// One area's result of [`MessageBase::verify`] or [`MessageBase::repair`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AreaCheck {
    pub area: String,
    /// What is wrong with the files, as jamjam describes it; empty when nothing is.
    pub problems: Vec<String>,
    /// Bytes of text no message refers to any more, as edits leave them; packing frees them.
    pub reclaimable: u64,
}

impl AreaCheck {
    fn new(area: &Area, report: &jamjam::jam::verify::VerifyReport) -> Self {
        let mut reclaimable = 0;
        let mut problems = Vec::new();
        for problem in &report.problems {
            match problem {
                jamjam::jam::verify::Problem::UnreferencedText { bytes, .. } => reclaimable += bytes,
                problem => problems.push(problem.to_string()),
            }
        }
        Self {
            area: area.name.clone(),
            problems,
            reclaimable,
        }
    }

    #[must_use]
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

/// The stored header of a message, for showing and changing what the list does not show.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MessageDetails {
    pub area: String,
    pub number: u32,
    pub attributes: u32,
    pub times_read: u32,
    pub written: Option<chrono::NaiveDateTime>,
    pub received: Option<chrono::NaiveDateTime>,
    pub processed: Option<chrono::NaiveDateTime>,
    pub reply_to: u32,
    pub reply_first: u32,
    pub reply_next: u32,
    pub text_len: u32,
    /// Kludges and other subfields as `(name, value)`, in stored order.
    pub fields: Vec<(String, String)>,
}

/// Attribute bits the message info dialog shows, with names that match the JAM specification.
pub const EDITABLE_ATTRIBUTES: &[(u32, &str)] = &[
    (attributes::MSG_LOCAL, "Local"),
    (attributes::MSG_PRIVATE, "Private"),
    (attributes::MSG_READ, "Read"),
    (attributes::MSG_SENT, "Sent"),
    (attributes::MSG_KILLSENT, "Kill/Sent"),
    (attributes::MSG_ARCHIVESENT, "Archive/Sent"),
    (attributes::MSG_HOLD, "Hold"),
    (attributes::MSG_CRASH, "Crash"),
    (attributes::MSG_IMMEDIATE, "Immediate"),
    (attributes::MSG_DIRECT, "Direct"),
    (attributes::MSG_FILEREQUEST, "File request"),
    (attributes::MSG_FILEATTACH, "File attached"),
    (attributes::MSG_RECEIPTREQ, "Return receipt requested"),
    (attributes::MSG_CONFIRMREQ, "Confirmation requested"),
    (attributes::MSG_ORPHAN, "Orphan"),
    (attributes::MSG_TYPELOCAL, "Local mail"),
    (attributes::MSG_TYPEECHO, "Echomail"),
    (attributes::MSG_TYPENET, "Netmail"),
    (attributes::MSG_NODISP, "Hidden"),
    (attributes::MSG_LOCKED, "Locked"),
];

pub struct MessageBase {
    pub kind: BaseKind,
    /// The opened `.jhr` or `icboard.toml`.
    pub path: PathBuf,
    pub name: String,
    /// One to eight uppercase ASCII letters and digits, keys drafts and read marks.
    pub bbs_id: String,
    /// Default sender: the sysop of the IcyBoard installation; for a single base outside of one,
    /// the author of its newest locally written message.
    pub user_name: String,
    pub areas: Vec<Area>,
    /// Live and deleted messages; deleted ones stay until the base is packed.
    pub messages: Vec<Message>,
    /// Delete marks, parallel to `messages`; they change while the base is open.
    deleted: Vec<AtomicBool>,
    /// Name CRC and user id of the user's lastread records, known only for the sysop of an
    /// IcyBoard installation whose user list names them. Without it the records are left alone:
    /// a guessed user would have the BBS skip mail they never read.
    lastread_user: Option<(u32, u32)>,
}

/// What [`MessageBase::pack`] removed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PackSummary {
    pub removed: u32,
    /// Areas whose base was packed.
    pub areas: usize,
}

impl std::fmt::Debug for MessageBase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageBase")
            .field("kind", &self.kind)
            .field("path", &self.path)
            .field("name", &self.name)
            .field("areas", &self.areas.len())
            .field("messages", &self.messages.len())
            .finish()
    }
}

/// Whether `path` names a JAM base or an IcyBoard configuration rather than a mail packet.
#[must_use]
pub fn is_message_base(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(HEADER_EXTENSION) || extension.eq_ignore_ascii_case("toml"))
}

/// Extensions of the files [`MessageBase::open`] accepts, for open dialogs.
pub fn extensions() -> Vec<String> {
    ["jhr", "toml"]
        .into_iter()
        .flat_map(|extension| [extension.to_owned(), extension.to_ascii_uppercase()])
        .collect()
}

impl MessageBase {
    pub fn open(path: &Path) -> Res<Self> {
        let _timer = crate::perf::Timer::new("message_base::open");
        let is_header = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case(HEADER_EXTENSION));
        let mut base = if is_header { Self::single(path)? } else { Self::icyboard(path)? };
        base.index()?;
        if base.user_name.is_empty() {
            base.user_name = base
                .messages
                .iter()
                .rev()
                .find(|message| message.local && !message.from.is_empty())
                .map(|message| String::from_utf8_lossy(&message.from).trim().to_owned())
                .unwrap_or_default();
        }
        base.load_last_read();
        Ok(base)
    }

    /// Sysop of the IcyBoard installation a base lies in, with their user id, if any.
    fn enclosing_sysop(path: &Path) -> Option<(String, Option<u32>)> {
        path.ancestors().skip(1).take(6).find_map(|directory| {
            let config: toml::Table = toml::from_str(&fs::read_to_string(directory.join(ICYBOARD_CONFIG)).ok()?).ok()?;
            Some(icyboard_sysop(directory, &config))
        })
    }

    /// Whether the message at `index` is UTF-8 regardless of its kludges.
    #[must_use]
    pub fn declares_utf8(&self, index: usize) -> bool {
        self.messages.get(index).is_some_and(|message| message.utf8)
    }

    fn single(path: &Path) -> Res<Self> {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        // A base inside an IcyBoard installation is one of its areas.
        let sysop = Self::enclosing_sysop(path);
        Ok(Self {
            kind: BaseKind::Single,
            path: path.to_path_buf(),
            bbs_id: bbs_id(&stem),
            user_name: sysop.as_ref().map(|(name, _)| name.clone()).unwrap_or_default(),
            areas: vec![Area::new(0, stem.clone(), path.to_path_buf(), false, false, sysop.is_some())],
            lastread_user: sysop.and_then(|(name, id)| lastread_user(&name, id)),
            name: stem,
            messages: Vec::new(),
            deleted: Vec::new(),
        })
    }

    fn icyboard(path: &Path) -> Res<Self> {
        let root = path.parent().unwrap_or(Path::new("."));
        let config: toml::Table = toml::from_str(&fs::read_to_string(path)?)?;
        let conferences = text(&config, &["paths", "conferences"]).ok_or("Not an IcyBoard configuration: [paths] conferences is missing")?;
        let name = text(&config, &["board", "name"]).unwrap_or_default();
        let qwk_id = text(&config, &["qwk_settings", "bbs_id"]).unwrap_or_default();
        let mut areas = Vec::new();
        let conferences: toml::Table = toml::from_str(&fs::read_to_string(resolve(root, &conferences))?)?;
        for conference in conferences.get("conference").and_then(toml::Value::as_array).into_iter().flatten() {
            let conference_name = conference.get("name").and_then(toml::Value::as_str).unwrap_or_default().trim();
            let echo = flag(conference, "echo_mail_in_conference");
            let conference_read_only = flag(conference, "is_read_only");
            let Some(area_file) = conference.get("area_file").and_then(toml::Value::as_str).filter(|file| !file.trim().is_empty()) else {
                continue;
            };
            let area_path = resolve(root, area_file);
            let area_list: toml::Table = match fs::read_to_string(&area_path)
                .map_err(|error| error.to_string())
                .and_then(|text| toml::from_str(&text).map_err(|error| error.to_string()))
            {
                Ok(list) => list,
                Err(error) => {
                    log::warn!("skipping the areas of conference {conference_name:?}: {}: {error}", area_path.display());
                    continue;
                }
            };
            for area in area_list.get("area").and_then(toml::Value::as_array).into_iter().flatten() {
                let Some(base) = area.get("path").and_then(toml::Value::as_str).filter(|path| !path.trim().is_empty()) else {
                    continue;
                };
                let area_name = area.get("name").and_then(toml::Value::as_str).unwrap_or_default().trim();
                let name = match (conference_name.is_empty(), area_name.is_empty()) {
                    (_, true) => conference_name.to_owned(),
                    (true, false) => area_name.to_owned(),
                    (false, false) if conference_name.eq_ignore_ascii_case(area_name) => area_name.to_owned(),
                    (false, false) => format!("{conference_name} / {area_name}"),
                };
                let number = u16::try_from(areas.len()).map_err(|_| "IcyBoard installation has too many message areas")?;
                areas.push(Area::new(
                    number,
                    name,
                    resolve(root, base),
                    conference_read_only || flag(area, "is_read_only"),
                    echo,
                    true,
                ));
            }
        }
        if let Some(email) = text(&config, &["paths", "email_msgbase"]).filter(|path| !path.trim().is_empty()) {
            let mut email = resolve(root, &email);
            if email.is_dir() {
                email = email.join("email");
            }
            if email.with_extension(HEADER_EXTENSION).exists() {
                let number = u16::try_from(areas.len()).map_err(|_| "IcyBoard installation has too many message areas")?;
                areas.push(Area::new(number, "Email".to_owned(), email, false, false, true));
            }
        }
        // IcyBoard's tosser sends what is written to its netmail base.
        if let Some(ftn) = text(&config, &["paths", "ftn_file"]).filter(|path| !path.trim().is_empty()) {
            let ftn = resolve(root, &ftn);
            if let Ok(ftn) = fs::read_to_string(&ftn)
                .map_err(|error| error.to_string())
                .and_then(|text| toml::from_str::<toml::Table>(&text).map_err(|error| error.to_string()))
            {
                let netmail = text(&ftn, &["netmail"]).unwrap_or_else(|| "ftn/netmail".to_owned());
                let number = u16::try_from(areas.len()).map_err(|_| "IcyBoard installation has too many message areas")?;
                let mut area = Area::new(number, "Netmail".to_owned(), resolve(root, &netmail), false, false, true);
                area.netmail = true;
                areas.push(area);
            }
        }
        let (sysop, user_id) = icyboard_sysop(root, &config);
        let lastread_user = lastread_user(&sysop, user_id);
        let name = if name.trim().is_empty() {
            root.file_name().unwrap_or_default().to_string_lossy().to_string()
        } else {
            name.trim().to_owned()
        };
        Ok(Self {
            kind: BaseKind::IcyBoard,
            path: path.to_path_buf(),
            bbs_id: bbs_id(if qwk_id.trim().is_empty() { &name } else { &qwk_id }),
            user_name: sysop,
            areas,
            name,
            messages: Vec::new(),
            deleted: Vec::new(),
            lastread_user,
        })
    }

    /// Reads every area's headers. An IcyBoard area that cannot be read is left empty, so one
    /// damaged base does not hide the others; a single base reports the error.
    fn index(&mut self) -> Res<()> {
        let mut messages = Vec::new();
        let mut deleted = Vec::new();
        // IcyBoard's configuration says what an area is for; a single base has only its messages.
        let infer = self.kind == BaseKind::Single;
        for area in &mut self.areas {
            match area.index(infer) {
                Ok(area_messages) => {
                    for (message, is_deleted) in area_messages {
                        messages.push(message);
                        deleted.push(AtomicBool::new(is_deleted));
                    }
                }
                Err(error) if self.kind == BaseKind::IcyBoard => {
                    log::warn!("skipping message area {:?} ({}): {error}", area.name, area.path.display());
                }
                Err(error) => return Err(error),
            }
        }
        self.messages = messages;
        self.deleted = deleted;
        Ok(())
    }

    /// Whether the message at `index` is marked deleted.
    #[must_use]
    pub fn is_deleted(&self, index: usize) -> bool {
        self.deleted.get(index).is_some_and(|deleted| deleted.load(Ordering::Relaxed))
    }

    /// Messages marked deleted, which [`Self::pack`] removes.
    #[must_use]
    pub fn deleted_count(&self) -> usize {
        self.deleted.iter().filter(|deleted| deleted.load(Ordering::Relaxed)).count()
    }

    /// Whether the message at `index` may be changed: read-only IcyBoard areas are left alone.
    #[must_use]
    pub fn can_change(&self, index: usize) -> bool {
        self.messages
            .get(index)
            .and_then(|message| self.area(message.area))
            .is_some_and(|area| !area.read_only)
    }

    /// Sets or clears the delete mark of the message at `index`. Like the BBS does, a deleted
    /// message stays in the base, and can be restored, until the base is packed.
    pub fn set_deleted(&self, index: usize, deleted: bool) -> Res<()> {
        let message = self.messages.get(index).ok_or("Message index out of range")?;
        let area = self.area(message.area).ok_or("Message area does not exist")?;
        if area.read_only {
            return Err(format!("Message area {:?} is read-only", area.name).into());
        }
        area.with_base(false, |base| {
            if deleted {
                base.delete_message(message.number)?;
            } else {
                base.restore_message(message.number)?;
            }
            Ok(())
        })?;
        self.deleted[index].store(deleted, Ordering::Relaxed);
        Ok(())
    }

    /// Removes the deleted messages of every area for good, and what `choice` asks for. Message
    /// numbers do not change, so replies and lastread pointers stay valid.
    pub fn pack(&self, choice: PackChoice) -> Res<PackSummary> {
        let mut options = PackOptions::default().with_purge_received_private(choice.purge_received_private);
        if let Some(before) = choice.purge_before {
            options = options.with_purge_before(before);
        }
        let mut summary = PackSummary::default();
        for area in self.existing_areas() {
            let report = area.with_base(false, |base| Ok(base.pack(&options)?))?;
            summary.removed += report.removed;
            summary.areas += 1;
        }
        Ok(summary)
    }

    fn existing_areas(&self) -> impl Iterator<Item = &Area> {
        self.areas.iter().filter(|area| area.path.with_extension(HEADER_EXTENSION).exists())
    }

    /// Checks the files of every area against each other.
    pub fn verify(&self) -> Res<Vec<AreaCheck>> {
        self.existing_areas()
            .map(|area| {
                let report = area.with_base(false, |base| Ok(base.verify()?))?;
                Ok(AreaCheck::new(area, &report))
            })
            .collect()
    }

    /// Repairs what [`Self::verify`] finds, area by area.
    pub fn repair(&self) -> Res<Vec<AreaCheck>> {
        self.existing_areas()
            .map(|area| {
                let report = area.with_base(false, |base| Ok(base.repair()?))?;
                Ok(AreaCheck::new(area, &report.after))
            })
            .collect()
    }

    /// Changes sender, recipient, subject, privacy, netmail address and text of the message at
    /// `index`; `post.area` must be its area.
    pub fn update(&self, index: usize, post: &Post) -> Res<()> {
        let message = self.messages.get(index).ok_or("Message index out of range")?;
        if post.area != message.area {
            return Err("an edited message stays in its area".into());
        }
        let area = self.writable(message.area)?;
        area.replace(message.number, post)
    }

    fn writable(&self, number: u16) -> Res<&Area> {
        let area = self.area(number).ok_or("Message area does not exist")?;
        if area.read_only {
            return Err(format!("Message area {:?} is read-only", area.name).into());
        }
        Ok(area)
    }

    /// Copies the message at `index` into another area, or moves it there with `keep` off, which
    /// marks the original deleted. The copy is filed as the target area's kind of mail; a copy
    /// gets an identity of its own, a moved message keeps its `MSGID`. Answers are linked by
    /// number only within one area, so those links are dropped. Copies are local and unsent.
    /// Netmail targets require an existing valid destination address. Returns the new number.
    pub fn transfer(&self, index: usize, target: u16, keep: bool) -> Res<u32> {
        let message = self.messages.get(index).ok_or("Message index out of range")?;
        if message.area == target {
            return Err("the message is in that area already".into());
        }
        let source = if keep {
            self.area(message.area).ok_or("Message area does not exist")?
        } else {
            self.writable(message.area)?
        };
        let target_area = self.writable(target)?;
        let stored = source.with_base(false, |base| Ok(base.read_transaction(|base| base.read_message(message.number))?))?;
        let mut header = stored.header().clone();
        if target_area.netmail {
            let address = subfield(&header, SubfieldType::AddressD)
                .and_then(|address| parse_address(&address.to_string()))
                .ok_or("transfer to netmail requires a valid destination address")?;
            set_subfield(&mut header, SubfieldType::AddressD, Some(address.into_bytes()));
            header.attributes |= attributes::MSG_PRIVATE;
        }
        header.reply_to = 0;
        header.reply_first = 0;
        header.reply_next = 0;
        header.attributes = (header.attributes & !TYPE_BITS & !attributes::MSG_DELETED) | target_area.type_bits();
        if keep {
            header.attributes = (header.attributes & !attributes::MSG_SENT) | attributes::MSG_LOCAL;
            header
                .sub_fields
                .retain(|field| !matches!(field.field_type(), SubfieldType::MsgID | SubfieldType::ReplyID));
            header.msgid_crc = NO_CRC;
            header.reply_crc = NO_CRC;
        }
        let copy = JamMessage::from_stored(header, stored.text().clone());
        let number = target_area.with_base(true, |base| Ok(base.write_message(&copy)?))?;
        if !keep {
            self.set_deleted(index, true)?;
        }
        Ok(number)
    }

    /// The stored header of the message at `index`.
    pub fn details(&self, index: usize) -> Res<MessageDetails> {
        let message = self.messages.get(index).ok_or("Message index out of range")?;
        let area = self.area(message.area).ok_or("Message area does not exist")?;
        let stored = area.with_base(false, |base| {
            base.read_transaction(|base| match base.read_message(message.number) {
                Ok(message) => Ok(message),
                Err(error) => deleted_message(base, message.number)?.ok_or(error),
            })
            .map_err(Into::into)
        })?;
        let header = stored.header();
        let date = |seconds: u32| {
            (seconds != 0)
                .then(|| chrono::DateTime::from_timestamp(i64::from(seconds), 0))
                .flatten()
                .map(|date| date.naive_utc())
        };
        Ok(MessageDetails {
            area: area.name.clone(),
            number: header.message_number,
            attributes: header.attributes,
            times_read: header.times_read,
            written: message_date(header),
            received: date(header.date_received),
            processed: date(header.date_processed),
            reply_to: header.reply_to,
            reply_first: header.reply_first,
            reply_next: header.reply_next,
            text_len: header.txt_len,
            fields: header
                .sub_fields
                .iter()
                .filter(|field| !matches!(field.field_type(), SubfieldType::SenderName | SubfieldType::RecvName | SubfieldType::Subject))
                .map(|field| (subfield_name(field.field_type()), String::from_utf8_lossy(field.content()).into_owned()))
                .collect(),
        })
    }

    /// Turns attribute bits of the message at `index` on and off. The delete flag goes through
    /// [`Self::set_deleted`].
    pub fn set_attributes(&self, index: usize, set: u32, clear: u32) -> Res<()> {
        let message = self.messages.get(index).ok_or("Message index out of range")?;
        let area = self.writable(message.area)?;
        let keep = !attributes::MSG_DELETED;
        area.with_base(false, |base| {
            jamjam::jam::raw::set_attributes(base, message.number, set & keep, clear & keep)?;
            Ok(())
        })
    }

    /// The user's lastread pointer in an area, `0` when nothing was read there.
    #[must_use]
    pub fn last_read(&self, area: u16) -> u32 {
        self.area(area).map_or(0, |area| area.last_read.load(Ordering::Relaxed))
    }

    /// Reads the user's lastread record of every area, found by name CRC and user id as the BBS
    /// finds it.
    fn load_last_read(&self) {
        let Some((crc, user_id)) = self.lastread_user else {
            return;
        };
        for area in self.existing_areas() {
            let records = area.with_base(false, |base| Ok(base.read_transaction(|base| base.read_last_read_file())?));
            match records {
                Ok(records) => {
                    if let Some(record) = records.iter().find(|record| record.user_crc == crc && record.user_id == user_id) {
                        area.last_read.store(record.last_read_msg, Ordering::Relaxed);
                    }
                }
                Err(error) => log::warn!("could not read the lastread records of {}: {error}", area.name),
            }
        }
    }

    /// Whether the user's lastread records are known, and so kept in step with the read marks.
    #[must_use]
    pub fn syncs_last_read(&self) -> bool {
        self.lastread_user.is_some()
    }

    /// Moves the user's lastread pointer of an area forward to `number`; it never moves back.
    /// Callers pass the highest number up to which everything has been read, so the BBS does not
    /// take unread mail for read. Returns whether the record changed.
    pub fn advance_last_read(&self, area: u16, number: u32) -> Res<bool> {
        let Some((crc, user_id)) = self.lastread_user else {
            return Ok(false);
        };
        let area = self.area(area).ok_or("Message area does not exist")?;
        if number <= area.last_read.load(Ordering::Relaxed) || !area.path.with_extension(HEADER_EXTENSION).exists() {
            return Ok(false);
        }
        let stored = area.with_base(false, |base| {
            Ok(base.transaction(|base| {
                let mut record = match base.find_last_read(crc, user_id)? {
                    Some(record) => record,
                    None => JamLastReadStorage {
                        user_crc: crc,
                        user_id,
                        ..Default::default()
                    },
                };
                record.last_read_msg = record.last_read_msg.max(number);
                record.high_read_msg = record.high_read_msg.max(number);
                base.write_last_read(&record)?;
                Ok(record.last_read_msg)
            })?)
        })?;
        area.last_read.store(stored, Ordering::Relaxed);
        Ok(true)
    }

    /// The netmail areas, which need a destination address.
    #[must_use]
    pub fn is_netmail(&self, area: u16) -> bool {
        self.area(area).is_some_and(|area| area.netmail)
    }

    #[must_use]
    pub fn area(&self, number: u16) -> Option<&Area> {
        self.areas.get(usize::from(number)).filter(|area| area.number == number)
    }

    /// The message at `index` with its text, in the shape the reader and composer use. JAM keeps
    /// kludges in header subfields; they are put back in front of the text as `^A` lines.
    pub fn read(&self, index: usize) -> Res<QwkMessage> {
        let message = self.messages.get(index).ok_or("Message index out of range")?;
        let area = self.area(message.area).ok_or("Message area does not exist")?;
        let stored = area.with_base(false, |base| {
            base.read_transaction(|base| match base.read_message(message.number) {
                Ok(message) => Ok(message),
                Err(error) => deleted_message(base, message.number)?.ok_or(error),
            })
            .map_err(Into::into)
        })?;
        Ok(QwkMessage {
            status: if message.private { b'*' } else { b' ' },
            msg_number: message.number,
            date_time: message.date.map_or_else(String::new, |date| date.format("%m-%d-%y%H:%M").to_string()).into(),
            to: message.to.clone().into(),
            from: message.from.clone().into(),
            subj: message.subject.clone().into(),
            password: "".into(),
            ref_msg_number: message.reply_to,
            active_flag: MSG_ACTIVE,
            conference_number: message.area,
            logical_message_number: 1,
            net_tag: b' ',
            text: display_text(stored.header(), stored.text()).into(),
        })
    }

    /// Appends `post` to its area and returns the new message number. The area's base is created
    /// when it does not exist yet, as IcyBoard does on the first message.
    pub fn post(&self, post: &Post) -> Res<u32> {
        let area = self.area(post.area).ok_or("Message area does not exist")?;
        if area.read_only {
            return Err(format!("Message area {:?} is read-only", area.name).into());
        }
        area.with_base(true, |base| {
            let parent_id = if post.reply_to == 0 {
                None
            } else {
                base.read_header(post.reply_to)
                    .ok()
                    .and_then(|header| subfield(&header, SubfieldType::MsgID).cloned())
            };
            let message = area.message(post, parent_id);
            Ok(base.write_message(&message)?)
        })
    }
}

impl Area {
    fn new(number: u16, name: String, path: PathBuf, read_only: bool, echo: bool, icyboard: bool) -> Self {
        Self {
            number,
            name,
            path,
            read_only,
            echo,
            icyboard,
            netmail: false,
            base: Mutex::new(None),
            last_read: AtomicU32::new(0),
        }
    }

    /// Runs `f` on the area's base, opening it the first time and with `create` creating it.
    fn with_base<T>(&self, create: bool, f: impl FnOnce(&mut JamMessageBase) -> Res<T>) -> Res<T> {
        let mut slot = self.base.lock().map_err(|_| "Message base lock poisoned")?;
        if slot.is_none() {
            let base = if self.path.with_extension(HEADER_EXTENSION).exists() {
                JamMessageBase::open(&self.path)?
            } else if create {
                if let Some(parent) = self.path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
                    fs::create_dir_all(parent)?;
                }
                JamMessageBase::create(&self.path)?
            } else {
                return Err(format!("Message base {} does not exist", self.path.display()).into());
            };
            *slot = Some(base);
        }
        f(slot.as_mut().expect("opened above"))
    }

    /// The area's messages in number order, each with whether it is marked deleted.
    fn index(&mut self, infer: bool) -> Res<Vec<(Message, bool)>> {
        if !self.path.with_extension(HEADER_EXTENSION).exists() {
            // IcyBoard creates a base with its first message.
            return Ok(Vec::new());
        }
        let number = self.number;
        let mut echo = false;
        let mut netmail = false;
        let mut icyboard = false;
        // Headers only, so writers wait no longer than the header file takes to read.
        let mut messages = self.with_base(false, |base| {
            Ok(base.read_transaction(|base| {
                let mut messages = Vec::new();
                for header in base.messages() {
                    let header = match header {
                        Ok(header) => header,
                        Err(error) => {
                            log::warn!("skipping unreadable JAM message in {}: {error}", base.path().display());
                            continue;
                        }
                    };
                    netmail |= header.attributes & attributes::MSG_TYPENET != 0;
                    echo |= header.attributes & attributes::MSG_TYPEECHO != 0
                        || header
                            .sub_fields
                            .iter()
                            .any(|field| matches!(field.field_type(), SubfieldType::SeenBy2D | SubfieldType::Path2D));
                    // Only jamjam, which IcyBoard writes with, adds this subfield.
                    icyboard |= subfield(&header, SubfieldType::DateWritten).is_some();
                    messages.push((index_message(number, &header), false));
                }
                if messages.len() < base.index_records() as usize {
                    // Deleted messages are skipped by the index walk; they stay until a pack.
                    let live: std::collections::HashSet<u32> = messages.iter().map(|(message, _)| message.number).collect();
                    for header in deleted_headers(base, &live)? {
                        messages.push((index_message(number, &header), true));
                    }
                    messages.sort_by_key(|(message, _)| message.number);
                }
                Ok(messages)
            })?)
        })?;
        if infer {
            self.netmail |= netmail;
            self.echo |= echo && !self.netmail;
            self.icyboard |= icyboard;
        }
        if self.icyboard {
            for (message, _) in &mut messages {
                message.charset = Some(crate::charset::Charset::Utf8);
                message.utf8 = true;
            }
        }
        Ok(messages)
    }

    /// `post` as this area stores it: IcyBoard reads every message as UTF-8 and converts to CP437
    /// itself when it sends mail out; other bases end lines with CR as the JAM specification asks.
    /// Returns the converted fields and the text.
    fn encode(&self, post: &Post) -> (Post, Vec<u8>) {
        let mut post = post.clone();
        if self.icyboard && !post.utf8 {
            post.from = cp437_to_utf8(&post.from);
            post.to = cp437_to_utf8(&post.to);
            post.subject = cp437_to_utf8(&post.subject);
            post.body = cp437_to_utf8(&post.body);
            post.utf8 = true;
        }
        let mut text = post.body.clone();
        if !self.icyboard {
            for byte in &mut text {
                if *byte == b'\n' {
                    *byte = b'\r';
                }
            }
            if text.last() != Some(&b'\r') {
                text.push(b'\r');
            }
        }
        (post, text)
    }

    /// The bits that file a message under this kind of area.
    fn type_bits(&self) -> u32 {
        if self.netmail {
            attributes::MSG_TYPENET
        } else if self.echo {
            attributes::MSG_TYPEECHO
        } else {
            attributes::MSG_TYPELOCAL
        }
    }

    fn message(&self, post: &Post, parent_id: Option<BString>) -> JamMessage {
        let (post, text) = self.encode(post);
        let mut header = JamMessageHeader::default();
        header.sub_fields.push(MessageSubfield::new(SubfieldType::SenderName, post.from.clone().into()));
        header.sub_fields.push(MessageSubfield::new(SubfieldType::RecvName, post.to.clone().into()));
        header.sub_fields.push(MessageSubfield::new(SubfieldType::Subject, post.subject.clone().into()));
        if let Some(address) = post.address.as_ref().filter(|_| self.netmail) {
            header.sub_fields.push(MessageSubfield::new(SubfieldType::AddressD, address.as_str().into()));
        }
        if let Some(id) = parent_id {
            header.reply_crc = JamMessageBase::crc(&id);
            header.sub_fields.push(MessageSubfield::new(SubfieldType::ReplyID, id));
        }
        header.sub_fields.push(MessageSubfield::new(
            SubfieldType::PID,
            format!("{} {}", crate::blue_wave::READER_NAME, *crate::VERSION).into(),
        ));
        let charset = if post.utf8 { "CHRS: UTF-8 4" } else { "CHRS: CP437 2" };
        header.sub_fields.push(MessageSubfield::new(SubfieldType::FTSKludge, charset.into()));
        header.reply_to = post.reply_to;
        // Netmail is always private.
        header.attributes = attributes::MSG_LOCAL | if post.private || self.netmail { attributes::MSG_PRIVATE } else { 0 } | self.type_bits();
        let now = chrono::Utc::now();
        if self.icyboard {
            header.date_written = u32::try_from(now.timestamp()).unwrap_or(u32::MAX);
            header.sub_fields.push(MessageSubfield::new(SubfieldType::DateWritten, now.to_rfc3339().into()));
        } else {
            let local = now.with_timezone(&chrono::Local).naive_local().and_utc();
            header.date_written = u32::try_from(local.timestamp()).unwrap_or(u32::MAX);
        }
        JamMessage::from_stored(header, text.into())
    }

    /// Replaces sender, recipient, subject, privacy, netmail address and text of a stored message,
    /// keeping its number, date, identity and links. JAM never rewrites text in place: the new
    /// text is appended and the header moved to the end; a pack reclaims the old ones.
    fn replace(&self, number: u32, post: &Post) -> Res<()> {
        let (post, text) = self.encode(post);
        let netmail = self.netmail;
        self.with_base(false, |base| {
            Ok(base.transaction(|base| {
                let mut header = base.read_header(number)?;
                set_subfield(&mut header, SubfieldType::SenderName, Some(post.from.clone()));
                set_subfield(&mut header, SubfieldType::RecvName, Some(post.to.clone()));
                set_subfield(&mut header, SubfieldType::Subject, Some(post.subject.clone()));
                if netmail {
                    set_subfield(&mut header, SubfieldType::AddressD, post.address.clone().map(String::into_bytes));
                }
                header
                    .sub_fields
                    .retain(|field| !(field.field_type() == SubfieldType::FTSKludge && is_charset_kludge(field.content())));
                let charset = if post.utf8 { "CHRS: UTF-8 4" } else { "CHRS: CP437 2" };
                header.sub_fields.push(MessageSubfield::new(SubfieldType::FTSKludge, charset.into()));
                if post.private || netmail {
                    header.attributes |= attributes::MSG_PRIVATE;
                } else {
                    header.attributes &= !attributes::MSG_PRIVATE;
                }
                let text_path = base.path().with_extension("jdt");
                let mut file = fs::OpenOptions::new().append(true).open(&text_path)?;
                let offset = file.metadata()?.len();
                header.offset = u32::try_from(offset).map_err(|_| std::io::Error::other("the JAM text file has reached 4 GiB"))?;
                header.txt_len = u32::try_from(text.len()).map_err(|_| std::io::Error::other("message text exceeds 4 GiB"))?;
                file.write_all(&text)?;
                file.flush()?;
                jamjam::jam::raw::update_header(base, number, &header)
            })?)
        })
    }
}

fn subfield_name(kind: SubfieldType) -> String {
    match kind {
        SubfieldType::Address0 => "Origin address".to_owned(),
        SubfieldType::AddressD => "Destination address".to_owned(),
        SubfieldType::MsgID => "MSGID".to_owned(),
        SubfieldType::ReplyID => "REPLY".to_owned(),
        SubfieldType::PID => "PID".to_owned(),
        SubfieldType::FTSKludge => "Kludge".to_owned(),
        SubfieldType::SeenBy2D => "SEEN-BY".to_owned(),
        SubfieldType::Path2D => "PATH".to_owned(),
        SubfieldType::Flags => "FLAGS".to_owned(),
        SubfieldType::TZUTCInfo => "TZUTC".to_owned(),
        SubfieldType::DateWritten => "Date written".to_owned(),
        SubfieldType::DateProcessed => "Date processed".to_owned(),
        SubfieldType::PackoutDate => "Pack out date".to_owned(),
        other => format!("{other:?}"),
    }
}

/// The sysop an IcyBoard configuration names and their position in its user list, which is the
/// user id IcyBoard keys their lastread records with.
fn icyboard_sysop(root: &Path, config: &toml::Table) -> (String, Option<u32>) {
    let sysop = text(config, &["sysop", "name"]).unwrap_or_default().trim().to_owned();
    let id = text(config, &["paths", "user_file"])
        .and_then(|users| fs::read_to_string(resolve(root, &users)).ok())
        .and_then(|users| toml::from_str::<toml::Table>(&users).ok())
        .and_then(|users| {
            users.get("users")?.as_array()?.iter().position(|user| {
                user.get("name")
                    .and_then(toml::Value::as_str)
                    .is_some_and(|name| name.trim().eq_ignore_ascii_case(&sysop))
            })
        })
        .and_then(|position| u32::try_from(position).ok());
    (sysop, id)
}

fn lastread_user(name: &str, id: Option<u32>) -> Option<(u32, u32)> {
    let name = name.trim();
    (!name.is_empty()).then(|| JamMessageBase::crc(&BString::from(name))).zip(id)
}

/// Validates a FidoNet address as netmail needs it, returning its canonical form.
pub fn parse_address(address: &str) -> Option<String> {
    EchomailAddress::parse(address.trim()).map(|address| address.to_string())
}

/// Replaces every subfield of a kind by one with `value`, or removes them for `None`.
fn set_subfield(header: &mut JamMessageHeader, kind: SubfieldType, value: Option<Vec<u8>>) {
    let position = header.sub_fields.iter().position(|field| field.field_type() == kind);
    header.sub_fields.retain(|field| field.field_type() != kind);
    if let Some(value) = value {
        let field = MessageSubfield::new(kind, value.into());
        match position {
            Some(position) => header.sub_fields.insert(position.min(header.sub_fields.len()), field),
            None => header.sub_fields.push(field),
        }
    }
}

fn is_charset_kludge(content: &[u8]) -> bool {
    let name = content.split(|&byte| byte == b':').next().unwrap_or_default();
    ["CHRS", "CHARSET", "CODEPAGE"]
        .iter()
        .any(|key| name.trim_ascii().eq_ignore_ascii_case(key.as_bytes()))
}

/// The current headers of messages marked deleted. Physical records also hold the headers that a
/// changed message superseded; the last record of a number is its current one.
fn deleted_headers(base: &JamMessageBase, live: &std::collections::HashSet<u32>) -> jamjam::Result<Vec<JamMessageHeader>> {
    let range = base.lowest_message_number()..=base.highest_message_number();
    let mut latest = std::collections::BTreeMap::new();
    for header in jamjam::jam::raw::physical_headers(base)? {
        let header = header?;
        if range.contains(&header.message_number) && !live.contains(&header.message_number) {
            latest.insert(header.message_number, header);
        }
    }
    Ok(latest.into_values().filter(JamMessageHeader::is_deleted).collect())
}

/// A message marked deleted, with its text, which `read_message` refuses.
fn deleted_message(base: &JamMessageBase, number: u32) -> jamjam::Result<Option<JamMessage>> {
    let mut found = None;
    for header in jamjam::jam::raw::physical_headers(base)? {
        let header = header?;
        if header.message_number == number {
            found = Some(header);
        }
    }
    let Some(header) = found.filter(JamMessageHeader::is_deleted) else {
        return Ok(None);
    };
    let text = base.read_message_text(&header)?;
    Ok(Some(JamMessage::from_stored(header, text)))
}

fn index_message(area: u16, header: &JamMessageHeader) -> Message {
    let charset = crate::charset::declared(&display_text(header, &[]));
    let field = |kind| subfield(header, kind).map(|value| value.to_vec()).unwrap_or_default();
    Message {
        area,
        number: header.message_number,
        reply_to: header.reply_to,
        from: field(SubfieldType::SenderName),
        to: field(SubfieldType::RecvName),
        subject: field(SubfieldType::Subject),
        date: message_date(header),
        private: header.attributes & attributes::MSG_PRIVATE != 0,
        local: header.attributes & attributes::MSG_LOCAL != 0,
        lines: 0,
        charset,
        utf8: false,
        origin: subfield(header, SubfieldType::Address0).map(ToString::to_string),
        destination: subfield(header, SubfieldType::AddressD).map(ToString::to_string),
    }
}

/// IcyBoard stores the UTC time and repeats it as RFC 3339; the JAM specification has local time.
fn message_date(header: &JamMessageHeader) -> Option<chrono::NaiveDateTime> {
    if let Some(date) = subfield(header, SubfieldType::DateWritten)
        .and_then(|value| std::str::from_utf8(value).ok())
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value.trim()).ok())
    {
        return Some(date.with_timezone(&chrono::Local).naive_local());
    }
    (header.date_written != 0)
        .then(|| chrono::DateTime::from_timestamp(i64::from(header.date_written), 0))
        .flatten()
        .map(|date| date.naive_utc())
}

fn subfield(header: &JamMessageHeader, kind: SubfieldType) -> Option<&BString> {
    header.sub_fields.iter().find(|field| field.field_type() == kind).map(MessageSubfield::content)
}

/// The message text with LF line ends and the kludge subfields as `^A` lines: identification
/// before the text, the echomail trail after it.
fn display_text(header: &JamMessageHeader, text: &[u8]) -> Vec<u8> {
    let kludge = |field: &MessageSubfield| -> Option<&'static str> {
        Some(match field.field_type() {
            SubfieldType::MsgID => "MSGID: ",
            SubfieldType::ReplyID => "REPLY: ",
            SubfieldType::PID => "PID: ",
            SubfieldType::FTSKludge => "",
            SubfieldType::TZUTCInfo => "TZUTC: ",
            SubfieldType::Flags => "FLAGS ",
            _ => return None,
        })
    };
    let mut out = Vec::with_capacity(text.len() + 64);
    for field in &header.sub_fields {
        if let Some(prefix) = kludge(field) {
            out.push(0x01);
            out.extend_from_slice(prefix.as_bytes());
            out.extend_from_slice(field.content());
            out.push(b'\n');
        }
    }
    let mut body = normalize_newlines(text);
    // IcyBoard marks UTF-8 text with a byte order mark instead of a CHRS kludge.
    if body.starts_with(UTF8_BOM) {
        body.drain(..UTF8_BOM.len());
        if crate::charset::declared(&out).is_none() && crate::charset::declared(&body).is_none() {
            out.extend_from_slice(b"\x01CHRS: UTF-8 4\n");
        }
    }
    out.extend(body);
    let mut trail = header
        .sub_fields
        .iter()
        .filter_map(|field| match field.field_type() {
            SubfieldType::SeenBy2D => Some(("SEEN-BY: ", field.content())),
            SubfieldType::Path2D => Some(("PATH: ", field.content())),
            _ => None,
        })
        .peekable();
    if trail.peek().is_some() && !out.is_empty() && out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    for (prefix, content) in trail {
        out.push(0x01);
        out.extend_from_slice(prefix.as_bytes());
        out.extend_from_slice(content);
        out.push(b'\n');
    }
    out
}

/// Control bytes stay as they are, so ANSI sequences survive.
fn cp437_to_utf8(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .map(|&byte| {
            if byte < 0x80 {
                char::from(byte)
            } else {
                icy_engine::BufferType::CP437.convert_to_unicode(char::from(byte))
            }
        })
        .collect::<String>()
        .into_bytes()
}

fn normalize_newlines(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.iter().copied().peekable();
    while let Some(byte) = bytes.next() {
        if byte == b'\r' {
            if bytes.peek() == Some(&b'\n') {
                bytes.next();
            }
            out.push(b'\n');
        } else {
            out.push(byte);
        }
    }
    out
}

fn text(table: &toml::Table, keys: &[&str]) -> Option<String> {
    let (last, path) = keys.split_last()?;
    let mut table = table;
    for key in path {
        table = table.get(*key)?.as_table()?;
    }
    table.get(*last)?.as_str().map(str::to_owned)
}

fn flag(value: &toml::Value, key: &str) -> bool {
    value.get(key).and_then(toml::Value::as_bool).unwrap_or(false)
}

/// IcyBoard paths are relative to the directory of `icboard.toml`.
fn resolve(root: &Path, path: &str) -> PathBuf {
    let path = Path::new(path.trim());
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn bbs_id(name: &str) -> String {
    let id: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(8)
        .collect::<String>()
        .to_ascii_uppercase();
    if id.is_empty() {
        "JAM".to_owned()
    } else {
        id
    }
}

#[cfg(test)]
mod tests;
