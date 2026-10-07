use bstr::ByteSlice;
use i18n_embed_fl::fl;
use jamjam::qwk::control::{Conference, ControlDat};
use jamjam::qwk::qwk_message::{QWKMessage, MSG_ACTIVE};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::{Cursor, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Arc, Mutex};
use unarc_rs::unified::{ArchiveFormat, ArchiveOptions, UnifiedArchive};

use crate::{text::HeaderText, Res, LANGUAGE_LOADER};

mod bodies;
mod cache;
mod protocol;
use bodies::MessageData;
pub use cache::ExtractionCache;
pub use protocol::Capabilities;
pub(crate) use protocol::SubscriptionFormat;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PacketFormat {
    #[default]
    Qwk,
    BlueWave,
}

pub fn packet_extensions() -> Vec<String> {
    let mut extensions: Vec<_> = ["qwk", "bw", "zip", "arj", "lzh", "lha", "rar", "7z", "arc", "zoo", "rep"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    for day in ["su", "mo", "tu", "we", "th", "fr", "sa"] {
        for sequence in 0..=9 {
            extensions.push(format!("{day}{sequence}"));
        }
    }
    // Linux portal filters use case-sensitive globs.
    extensions.extend(extensions.clone().into_iter().map(|extension| extension.to_ascii_uppercase()));
    extensions
}

#[cfg(test)]
pub mod tests;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageDescriptor {
    pub number: u32,
    pub conference: u16,
    pub offset: u64,
    pub block_count: u32,
    /// Native Blue Wave byte length; QWK messages use `block_count` instead.
    pub body_len: Option<u64>,
}

/// Header fields of a message, extracted once at load time.
///
/// The list view sorts, filters and threads over thousands of rows on every frame, so it must
/// never touch the (lazily parsed) message bodies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageInfo {
    pub index: usize,
    pub number: u32,
    /// Message this one replies to, `0` when it starts a thread.
    pub ref_number: u32,
    pub conference: u16,
    pub from: HeaderText,
    pub to: HeaderText,
    pub subject: HeaderText,
    /// Subject with all `Re:` prefixes stripped, lowercased - the thread key.
    pub subject_key: String,
    #[serde(with = "index_date")]
    pub date: chrono::NaiveDateTime,
    pub date_str: String,
    pub lines: u32,
    pub private: bool,
}

/// Strips any number of leading `Re:` / `Re[2]:` / `Fwd:` prefixes and lowercases the rest.
#[must_use]
pub fn normalize_subject(subject: &str) -> String {
    subject[reply_prefix_len(subject)..].trim_end().to_ascii_lowercase()
}

/// Byte length of the leading `Re:` / `Re[2]:` / `Fwd:` prefixes of `subject`, including surrounding spaces.
#[must_use]
pub fn reply_prefix_len(subject: &str) -> usize {
    let mut rest = subject.trim_start();
    loop {
        let lower = rest.to_ascii_lowercase();
        let stripped = ["re:", "fwd:", "fw:", "aw:"]
            .iter()
            .find_map(|p| lower.starts_with(p).then(|| &rest[p.len()..]))
            .or_else(|| {
                // `Re[2]:` / `Re(2):` styles
                let bytes = lower.as_bytes();
                if !bytes.starts_with(b"re") || bytes.len() < 4 {
                    return None;
                }
                let close = match bytes[2] {
                    b'[' => b']',
                    b'(' => b')',
                    _ => return None,
                };
                let end = bytes.iter().position(|b| *b == close)?;
                (bytes.get(end + 1) == Some(&b':')).then(|| &rest[end + 2..])
            });

        match stripped {
            Some(next) => rest = next.trim_start(),
            None => break,
        }
    }
    subject.len() - rest.len()
}

pub struct QwkPackage {
    pub bbs_name: String,
    pub descriptors: Vec<MessageDescriptor>,
    /// Header index, parallel to `descriptors`.
    pub infos: Vec<MessageInfo>,
    pub control_file: ControlDat,
    pub capabilities: Capabilities,
    /// Welcome, news and goodbye screens, bulletins and new files lists, in display order.
    pub files: Arc<Vec<PacketFile>>,
    pub blue_wave: Option<Arc<crate::blue_wave::Packet>>,
    messages_data: Arc<MessageData>,
    message_cache: Arc<Mutex<HashMap<usize, QWKMessage>>>, // Thread-safe cache
    thread_rows: Option<Arc<Vec<crate::threading::Row>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PacketFileKind {
    Welcome,
    News,
    Bulletin,
    NewFiles,
    Goodbye,
}

/// A text screen shipped next to the messages.
#[derive(Clone, Debug)]
pub struct PacketFile {
    /// File name inside the packet, as stored.
    pub name: String,
    pub kind: PacketFileKind,
    pub data: Vec<u8>,
    line_count: usize,
    page_count: usize,
}

impl PacketFile {
    pub fn lines(&self) -> usize {
        self.line_count
    }

    pub fn pages(&self) -> usize {
        self.page_count
    }

    fn new(name: String, kind: PacketFileKind, data: Vec<u8>) -> Self {
        let text = data.split(|byte| *byte == 0x1A).next().unwrap_or_default();
        let line_count = text.split(|byte| *byte == b'\n').filter(|line| !line.trim_ascii().is_empty()).count();
        let page_count = text
            .split_inclusive(|byte| *byte == b'\n')
            .count()
            .max(1)
            .div_ceil(crate::reader::FILE_PAGE_LINES);
        Self {
            name,
            kind,
            data,
            line_count,
            page_count,
        }
    }
}

/// Larger files are not bulletins; skipping them keeps odd packets from exhausting memory.
const MAX_PACKET_FILE_SIZE: u64 = 16 * 1024 * 1024;

const MAX_ARCHIVE_ENTRY_SIZE: u64 = 10 * 1024 * 1024 * 1024;

struct ExtractedPacket {
    format: PacketFormat,
    control: Vec<u8>,
    messages: Vec<u8>,
    message_source: Option<Arc<MessageData>>,
    others: Vec<(String, Vec<u8>)>,
    bbs_name: String,
    metadata: Option<cache::MetadataIndex>,
    cache_identity: Option<cache::CacheIdentity>,
}

/// Picks the screens named in CONTROL.DAT and the `BLT*`, `NEWFILES*` and `NFILE*` files.
/// `files` holds every other file of the packet.
pub fn packet_files(control: &ControlDat, mut files: Vec<(String, Vec<u8>)>) -> Vec<PacketFile> {
    files.sort_by_cached_key(|(name, _)| natural_key(name));
    let mut picked = Vec::new();
    for (kind, wanted) in [
        (PacketFileKind::Welcome, &control.welcome_screen),
        (PacketFileKind::News, &control.news_screen),
        (PacketFileKind::Goodbye, &control.logoff_screen),
    ] {
        let wanted = wanted.to_str_lossy().trim().to_uppercase();
        if wanted.is_empty() {
            continue;
        }
        // Exact names first; otherwise the first file starting with it.
        let position = files
            .iter()
            .position(|(name, _)| name.to_uppercase() == wanted)
            .or_else(|| files.iter().position(|(name, _)| name.to_uppercase().starts_with(&wanted)));
        if let Some(position) = position {
            let (name, data) = files.remove(position);
            picked.push(PacketFile::new(name, kind, data));
        }
    }
    for (name, data) in files {
        let upper = name.to_uppercase();
        let kind = if upper.starts_with("BLT") {
            PacketFileKind::Bulletin
        } else if upper.starts_with("NEWFILES") || upper.starts_with("NFILE") {
            PacketFileKind::NewFiles
        } else {
            continue;
        };
        picked.push(PacketFile::new(name, kind, data));
    }
    // Stable, so files of one kind keep their natural name order.
    picked.sort_by_key(|file| file.kind);
    picked
}

/// Orders `BLT-0.2` before `BLT-0.10`.
fn natural_key(name: &str) -> Vec<(String, u64)> {
    let upper = name.to_uppercase();
    let mut key = Vec::new();
    let mut rest = upper.as_str();
    while !rest.is_empty() {
        let text_len = rest.find(|c: char| c.is_ascii_digit()).unwrap_or(rest.len());
        let (text, tail) = rest.split_at(text_len);
        let digits_len = tail.find(|c: char| !c.is_ascii_digit()).unwrap_or(tail.len());
        let (digits, tail) = tail.split_at(digits_len);
        key.push((text.to_string(), digits.parse().unwrap_or(0)));
        rest = tail;
    }
    key
}

impl Clone for QwkPackage {
    fn clone(&self) -> Self {
        Self {
            bbs_name: self.bbs_name.clone(),
            descriptors: self.descriptors.clone(),
            infos: self.infos.clone(),
            control_file: self.control_file.clone(),
            capabilities: self.capabilities.clone(),
            files: self.files.clone(),
            blue_wave: self.blue_wave.clone(),
            messages_data: self.messages_data.clone(),
            message_cache: self.message_cache.clone(), // Share the cache across clones
            thread_rows: self.thread_rows.clone(),
        }
    }
}

impl QwkPackage {
    pub fn load_from_file(path: impl AsRef<Path>) -> Res<Self> {
        let _timer = crate::perf::Timer::new("qwk::load_from_file");
        let path = path.as_ref();
        Self::extract_packet(path)
            .and_then(|extracted| Self::from_extracted(path, extracted))
            .inspect_err(|error| log::error!("unable to load mail packet {}: {error}", path.display()))
    }

    pub fn load_from_file_cached(path: impl AsRef<Path>, cache: &ExtractionCache) -> Res<Self> {
        let _timer = crate::perf::Timer::new("qwk::load_from_file_cached");
        let path = path.as_ref();
        let mut extracted = cache
            .load(path, || Self::extract_packet(path))
            .inspect_err(|error| log::error!("unable to extract mail packet {}: {error}", path.display()))?;
        let identity = extracted.cache_identity.take();
        let indexed = extracted.metadata.is_some();
        let mut package = Self::from_extracted(path, extracted).inspect_err(|error| log::error!("unable to load mail packet {}: {error}", path.display()))?;
        if !indexed && identity.is_some() {
            let _timer = crate::perf::Timer::new("qwk::build_cached_threads");
            package.thread_rows = Some(Arc::new(crate::threading::build_threads(&package.infos.iter().collect::<Vec<_>>())));
        }
        if !indexed {
            if let Some(identity) = identity {
                if let Err(error) = cache.store_index(&identity, &package) {
                    log::warn!("unable to store packet metadata cache for {}: {error}", path.display());
                }
            }
        }
        Ok(package)
    }

    /// Full all-message thread order; filtered views must build their own thread rows.
    pub fn cached_threads(&self) -> Option<&[crate::threading::Row]> {
        self.thread_rows.as_deref().map(Vec::as_slice)
    }

    fn extract_packet(path: &Path) -> Res<ExtractedPacket> {
        let mut reader = std::io::BufReader::new(fs::File::open(path)?);
        // Packets are usually ZIP files, but BBSes also pack them with ARJ, LHA, RAR, ARC, ZOO and
        // others; the content decides, since the extension is always `.QWK`.
        let format = ArchiveFormat::detect(&mut reader, Some(path))?.ok_or_else(|| fl!(LANGUAGE_LOADER, "packet-error-unknown-archive-format"))?;
        let options = ArchiveOptions::new().with_max_entry_size(Some(MAX_ARCHIVE_ENTRY_SIZE));
        let mut archive = UnifiedArchive::open_with_format_and_options(reader, format, options)?;

        let mut messages_dat: Option<Vec<u8>> = None;
        let mut control_dat: Option<Vec<u8>> = None;
        let mut others: Vec<(String, Vec<u8>)> = Vec::new();
        let mut bbs_id = String::new();

        // Extract relevant files from the archive
        while let Some(entry) = archive.next_entry()? {
            let file_name = entry.name().replace('\\', "/").to_uppercase();
            let base_name = file_name.rsplit('/').next().unwrap_or_default();

            if base_name == "MESSAGES.DAT" {
                messages_dat = Some(archive.read(&entry)?);

                if let Some(dot_pos) = file_name.find('.') {
                    if dot_pos > 0 {
                        bbs_id = file_name[..dot_pos].to_string();
                    }
                }
            } else if base_name == "CONTROL.DAT" {
                control_dat = Some(archive.read(&entry)?);
            } else if !base_name.is_empty() && !base_name.ends_with(".NDX") && (entry.original_size() <= MAX_PACKET_FILE_SIZE || blue_wave_member(base_name)) {
                // The screen names are only known once CONTROL.DAT is parsed, which may come later.
                let name = entry.name().replace('\\', "/").rsplit('/').next().unwrap_or_default().to_string();
                let data = archive.read(&entry)?;
                if data.len() as u64 <= MAX_PACKET_FILE_SIZE || blue_wave_member(base_name) {
                    others.push((name, data));
                }
            } else {
                archive.skip(&entry)?;
            }
        }

        let mut stems = Vec::new();
        for (name, _) in &others {
            let upper = name.to_ascii_uppercase();
            let Some(stem) = upper.strip_suffix(".INF") else { continue };
            if ["INF", "MIX", "FTI", "DAT"]
                .iter()
                .all(|extension| others.iter().any(|(name, _)| name.eq_ignore_ascii_case(&format!("{stem}.{extension}"))))
            {
                stems.push(stem.to_owned());
            }
        }
        if stems.len() > 1 || !stems.is_empty() && (control_dat.is_some() || messages_dat.is_some()) {
            return Err("Archive contains ambiguous or mixed mail packet formats".into());
        }
        if let Some(stem) = stems.pop() {
            for extension in ["INF", "MIX", "FTI", "DAT"] {
                let name = format!("{stem}.{extension}");
                if others.iter().filter(|(member, _)| member.eq_ignore_ascii_case(&name)).count() != 1 {
                    return Err(format!("Blue Wave packet has duplicate {extension} members").into());
                }
            }
            let inf = others.iter().position(|(name, _)| name.eq_ignore_ascii_case(&format!("{stem}.INF"))).unwrap();
            let control = others.remove(inf).1;
            let dat = others.iter().position(|(name, _)| name.eq_ignore_ascii_case(&format!("{stem}.DAT"))).unwrap();
            let messages = others.remove(dat).1;
            others.retain(|(name, data)| {
                data.len() as u64 <= MAX_PACKET_FILE_SIZE || ["MIX", "FTI"].iter().any(|extension| name.eq_ignore_ascii_case(&format!("{stem}.{extension}")))
            });
            return Ok(ExtractedPacket {
                format: PacketFormat::BlueWave,
                control,
                messages,
                message_source: None,
                others,
                bbs_name: stem,
                metadata: None,
                cache_identity: None,
            });
        }
        if others.iter().any(|(name, _)| name.to_ascii_uppercase().ends_with(".INF")) && control_dat.is_none() && messages_dat.is_none() {
            return Err("Incomplete Blue Wave packet: matching INF, MIX, FTI and DAT files are required".into());
        }
        others.retain(|(_, data)| data.len() as u64 <= MAX_PACKET_FILE_SIZE);
        Ok(ExtractedPacket {
            format: PacketFormat::Qwk,
            control: control_dat.ok_or_else(|| fl!(LANGUAGE_LOADER, "packet-error-control-dat-not-found"))?,
            messages: messages_dat.ok_or_else(|| fl!(LANGUAGE_LOADER, "packet-error-messages-dat-not-found"))?,
            message_source: None,
            others,
            bbs_name: bbs_id,
            metadata: None,
            cache_identity: None,
        })
    }

    fn from_extracted(path: &Path, extracted: ExtractedPacket) -> Res<Self> {
        if extracted.format == PacketFormat::BlueWave {
            return Self::from_blue_wave(extracted);
        }
        let ExtractedPacket {
            control,
            messages,
            message_source,
            others,
            bbs_name: mut bbs_id,
            metadata,
            cache_identity: _,
            format: _,
        } = extracted;
        // Parse CONTROL.DAT
        let control_file =
            ControlDat::read(&control).map_err(|error| fl!(LANGUAGE_LOADER, "packet-error-control-dat-parse-failed", error = format!("{error:?}")))?;

        // Use BBS name from control file if we don't have one yet
        if !control_file.bbs_name.is_empty() && bbs_id.is_empty() {
            bbs_id = control_file.bbs_name.to_string();
        }

        // Parse just the headers, not full messages
        let (headers, infos, thread_rows) = match metadata {
            Some(index) => (index.descriptors, index.infos, Some(Arc::new(index.threads))),
            None => {
                let headers = Self::parse_headers(&messages);
                let infos = Self::build_index(&messages, &headers);
                (headers, infos, None)
            }
        };
        let messages_data = message_source.unwrap_or_else(|| Arc::new(MessageData::Memory(messages)));

        // Use filename as fallback for BBS name
        if bbs_id.is_empty() {
            bbs_id = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        }

        let mut capabilities = Capabilities::parse(&others);
        // Actual extended headers are evidence for QWKE posting, but not for
        // offline subscription commands (those still need an advertisement).
        capabilities.qwke |= infos
            .iter()
            .any(|info| [&info.from, &info.to, &info.subject].iter().any(|field| field.chars().count() > 25));
        Ok(QwkPackage {
            bbs_name: bbs_id,
            infos,
            descriptors: headers,
            capabilities,
            files: Arc::new(packet_files(&control_file, others)),
            blue_wave: None,
            control_file,
            messages_data,
            message_cache: Arc::new(Mutex::new(HashMap::new())),
            thread_rows,
        })
    }

    fn from_blue_wave(extracted: ExtractedPacket) -> Res<Self> {
        let ExtractedPacket {
            control,
            messages,
            message_source,
            mut others,
            bbs_name: stem,
            metadata,
            ..
        } = extracted;
        let mix = others
            .iter()
            .position(|(name, _)| name.eq_ignore_ascii_case(&format!("{stem}.MIX")))
            .ok_or("Blue Wave MIX file is missing")?;
        let mix = others.remove(mix).1;
        let fti = others
            .iter()
            .position(|(name, _)| name.eq_ignore_ascii_case(&format!("{stem}.FTI")))
            .ok_or("Blue Wave FTI file is missing")?;
        let fti = others.remove(fti).1;
        let messages_data = message_source.unwrap_or_else(|| Arc::new(MessageData::Memory(messages)));
        let packet = crate::blue_wave::parse(&control, &mix, &fti, messages_data.len())?;
        let mut descriptors = Vec::with_capacity(packet.messages.len());
        let mut infos = Vec::with_capacity(packet.messages.len());
        for (index, message) in packet.messages.iter().enumerate() {
            let number = if message.number == 0 {
                u32::try_from(index)?.checked_add(0x1_0000).ok_or("Blue Wave local message identity overflow")?
            } else {
                message.number
            };
            descriptors.push(MessageDescriptor {
                number,
                conference: message.area,
                offset: message.body_offset,
                block_count: 0,
                body_len: Some(message.body_len),
            });
            if metadata.is_some() {
                continue;
            }
            let date = chrono::DateTime::from_timestamp(message.unix_time, 0)
                .ok_or("Invalid Blue Wave message date")?
                .naive_utc();
            let subject = HeaderText::new(&message.subject);
            infos.push(MessageInfo {
                index,
                number,
                ref_number: message.reply_to,
                conference: message.area,
                from: HeaderText::new(&message.from),
                to: HeaderText::new(&message.to),
                subject_key: normalize_subject(&subject),
                subject,
                date,
                date_str: if message.date_known {
                    date.format("%Y-%m-%d %H:%M").to_string()
                } else if !message.date_raw.is_empty() {
                    HeaderText::new(&message.date_raw).to_string()
                } else {
                    fl!(LANGUAGE_LOADER, "packet-date-unknown")
                },
                lines: u32::try_from(blue_wave_body(&messages_data, message)?.iter().filter(|&&byte| byte == b'\n').count())?,
                private: message.private,
            });
        }
        let (infos, thread_rows) = if let Some(metadata) = metadata {
            if metadata.descriptors != descriptors {
                return Err("Blue Wave metadata cache does not match the packet index".into());
            }
            (metadata.infos, Some(Arc::new(metadata.threads)))
        } else {
            (infos, None)
        };
        let control_file = ControlDat {
            bbs_name: packet
                .info
                .bbs_name
                .chars()
                .map(crate::editor::cp437_byte)
                .collect::<Option<Vec<_>>>()
                .ok_or("Blue Wave BBS name cannot be encoded as CP437")?
                .into(),
            bbs_city_and_state: "".into(),
            bbs_phone_number: "".into(),
            bbs_sysop_name: "".into(),
            bbs_id: packet.info.bbs_id.as_bytes().into(),
            serial_number: 0,
            creation_time: "".into(),
            qmail_user_name: packet.info.user_name.clone().into(),
            qmail_menu_name: "".into(),
            zero_line: "".into(),
            message_count: u32::try_from(packet.messages.len())?,
            conferences: packet
                .areas
                .iter()
                .map(|area| Conference {
                    number: area.number,
                    name: area.title.clone().into(),
                })
                .collect(),
            welcome_screen: "WELCOME".into(),
            news_screen: "NEWS".into(),
            logoff_screen: "GOODBYE".into(),
        };
        Ok(Self {
            bbs_name: packet.info.bbs_name.clone(),
            descriptors,
            infos,
            files: Arc::new(packet_files(&control_file, others)),
            control_file,
            capabilities: Capabilities::default(),
            blue_wave: Some(Arc::new(packet)),
            messages_data,
            message_cache: Arc::new(Mutex::new(HashMap::new())),
            thread_rows,
        })
    }

    pub fn format(&self) -> PacketFormat {
        if self.blue_wave.is_some() {
            PacketFormat::BlueWave
        } else {
            PacketFormat::Qwk
        }
    }

    pub fn matches_personal(&self, recipient: &str, user: &str) -> bool {
        if user.is_empty() {
            return false;
        }
        if recipient.trim().eq_ignore_ascii_case(user.trim()) {
            return true;
        }
        self.blue_wave.as_ref().is_some_and(|packet| {
            let name = HeaderText::new(&packet.info.user_name);
            let alias = HeaderText::new(&packet.info.alias);
            (user.trim().eq_ignore_ascii_case(name.trim()) || !alias.is_empty() && user.trim().eq_ignore_ascii_case(alias.trim()))
                && (recipient.trim().eq_ignore_ascii_case(name.trim()) || !alias.is_empty() && recipient.trim().eq_ignore_ascii_case(alias.trim()))
        })
    }

    /// Reads message metadata, including QWKE fields, so the list can sort/filter/thread without I/O.
    fn build_index(data: &[u8], descriptors: &[MessageDescriptor]) -> Vec<MessageInfo> {
        let _timer = crate::perf::Timer::with("qwk::build_index", format!("{} messages", descriptors.len()));
        descriptors
            .par_iter()
            .with_min_len(256)
            .enumerate()
            .map(|(index, descriptor)| {
                let mut cursor = Cursor::new(data);
                let msg = cursor
                    .seek(SeekFrom::Start(descriptor.offset))
                    .ok()
                    .and_then(|_| QWKMessage::read(&mut cursor, true).ok());

                let Some(msg) = msg else {
                    return MessageInfo {
                        index,
                        number: descriptor.number,
                        ref_number: 0,
                        conference: descriptor.conference,
                        from: HeaderText::default(),
                        to: HeaderText::default(),
                        subject: fl!(LANGUAGE_LOADER, "packet-unreadable-message-subject", number = descriptor.number).into(),
                        subject_key: String::new(),
                        date: chrono::NaiveDateTime::default(),
                        date_str: String::new(),
                        lines: 0,
                        private: false,
                    };
                };

                let from = HeaderText::new(msg.from.trim());
                let to = HeaderText::new(msg.to.trim());
                let subject = HeaderText::new(msg.subj.trim());
                if log::log_enabled!(log::Level::Debug)
                    && [&msg.from[..], &msg.to[..], &msg.subj[..]]
                        .iter()
                        .any(|field| field.iter().any(u8::is_ascii_control))
                {
                    log::debug!(
                        "parsed ANSI QWK header conference={} message={}: from={:?}, to={:?}, subject={:?}",
                        msg.conference_number,
                        msg.msg_number,
                        msg.from.as_bstr(),
                        msg.to.as_bstr(),
                        msg.subj.as_bstr()
                    );
                }
                let raw_date = trim_field(&msg.date_time);
                let (date, date_str) = match parse_qwk_date(&raw_date) {
                    Ok(date) => (date, date.format("%Y-%m-%d %H:%M").to_string()),
                    Err(error) => {
                        log::warn!("invalid QWK date for message {}: {raw_date:?}: {error}", msg.msg_number);
                        (chrono::NaiveDateTime::default(), raw_date)
                    }
                };
                MessageInfo {
                    index,
                    number: msg.msg_number,
                    ref_number: msg.ref_msg_number,
                    conference: msg.conference_number,
                    from,
                    to,
                    subject_key: normalize_subject(&subject),
                    subject,
                    date,
                    date_str,
                    lines: msg.text.iter().filter(|b| **b == b'\n').count() as u32,
                    private: matches!(msg.status, b'*' | b'+' | b'~' | b'`'),
                }
            })
            .collect()
    }

    fn parse_headers(data: &[u8]) -> Vec<MessageDescriptor> {
        const HEADER_SIZE: usize = 128;
        let _timer = crate::perf::Timer::new("qwk::parse_headers");
        let mut headers = Vec::with_capacity(data.len() / 256); // Pre-allocate estimated capacity

        let mut pos = HEADER_SIZE; // Skip packet header

        while pos + HEADER_SIZE <= data.len() {
            let header_data = &data[pos..pos + HEADER_SIZE];

            let status = header_data[0];
            if status != 225 && status != b' ' && status != b'+' && status != b'-' && status != b'*' {
                pos += HEADER_SIZE;
                continue; // Skip deleted/invalid messages
            }

            let msg_number: u32 = parse_qwk_number(&header_data[1..8]).unwrap_or(0);

            let block_count = parse_qwk_number(&header_data[116..122]).unwrap_or(1);

            let conference = u16::from_le_bytes([header_data[123], header_data[124]]);

            headers.push(MessageDescriptor {
                number: msg_number,
                conference,
                offset: pos as u64,
                block_count,
                body_len: None,
            });

            // Skip to next message (header + content blocks)
            pos += HEADER_SIZE * block_count as usize;
        }

        headers
    }

    /// Load a specific message on demand with caching
    pub fn get_message(&self, index: usize) -> Res<QWKMessage> {
        if index >= self.descriptors.len() {
            return Err(fl!(LANGUAGE_LOADER, "packet-error-message-index-out-of-range").into());
        }

        // Check cache first
        {
            let cache: std::sync::MutexGuard<'_, HashMap<usize, QWKMessage>> = self.message_cache.lock().unwrap();
            if let Some(message) = cache.get(&index) {
                return Ok(message.clone());
            }
        }

        let msg = self.read_message(index)?;

        // Store in cache
        {
            // Optional: Limit cache size to prevent excessive memory usage
            const MAX_CACHE_SIZE: usize = 1000;
            let mut cache = self.message_cache.lock().unwrap();

            if cache.len() >= MAX_CACHE_SIZE {
                // Remove oldest entries (simple FIFO for now)
                // In production, you might want LRU eviction
                let keys_to_remove: Vec<usize> = cache.keys().take(cache.len() - MAX_CACHE_SIZE / 2).copied().collect();
                for key in keys_to_remove {
                    cache.remove(&key);
                }
            }

            cache.insert(index, msg.clone());
        }

        Ok(msg)
    }

    /// Read without locking or populating the interactive message cache, for bulk scans.
    pub fn read_message(&self, index: usize) -> Res<QWKMessage> {
        if let Some(packet) = &self.blue_wave {
            let message = packet
                .messages
                .get(index)
                .ok_or_else(|| fl!(LANGUAGE_LOADER, "packet-error-message-index-out-of-range"))?;
            let text = blue_wave_body(&self.messages_data, message)?;
            let date = chrono::DateTime::from_timestamp(message.unix_time, 0).ok_or("Invalid Blue Wave message date")?;
            return Ok(QWKMessage {
                status: if message.private { b'*' } else { b' ' },
                msg_number: message.number,
                date_time: if message.date_known {
                    date.format("%m-%d-%y%H:%M").to_string().into()
                } else {
                    message.date_raw.clone().into()
                },
                to: message.to.clone().into(),
                from: message.from.clone().into(),
                subj: message.subject.clone().into(),
                password: "".into(),
                ref_msg_number: message.reply_to,
                active_flag: MSG_ACTIVE,
                conference_number: message.area,
                logical_message_number: 1,
                net_tag: b' ',
                text: text.into(),
            });
        }
        let header = self
            .descriptors
            .get(index)
            .ok_or_else(|| fl!(LANGUAGE_LOADER, "packet-error-message-index-out-of-range"))?;
        let data = self.messages_data.read_range(header.offset, u64::from(header.block_count) * 128)?;
        let mut cursor = Cursor::new(data);
        QWKMessage::read(&mut cursor, true)
    }

    /// Whether any file-backed raw-body chunks have yet to be loaded into RAM.
    pub fn needs_preload(&self) -> bool {
        self.messages_data.needs_preload()
    }

    /// Load verified raw-body chunks, stopping between chunks when the job is superseded.
    pub fn preload_messages(&self, cancelled: impl Fn() -> bool) -> Res<bool> {
        self.messages_data.preload(cancelled)
    }

    /// Clear the message cache to free memory
    pub fn clear_cache(&self) {
        let mut cache = self.message_cache.lock().unwrap();
        cache.clear();
    }

    /// Get cache statistics (for debugging/monitoring)
    #[must_use]
    pub fn cache_stats(&self) -> (usize, usize) {
        let cache = self.message_cache.lock().unwrap();
        (cache.len(), self.descriptors.len())
    }

    #[must_use]
    pub fn message_count(&self) -> usize {
        self.descriptors.len()
    }

    /// Conferences that actually carry messages, as `(number, name, message count)`.
    #[must_use]
    pub fn conferences(&self) -> Vec<(u16, String, usize)> {
        let mut counts: HashMap<u16, usize> = HashMap::new();
        for info in &self.infos {
            *counts.entry(info.conference).or_default() += 1;
        }

        let mut list: Vec<(u16, String, usize)> = self
            .control_file
            .conferences
            .iter()
            .filter_map(|conference| {
                let name = HeaderText::new(conference.name.trim()).to_string();
                let count = counts.remove(&conference.number).unwrap_or(0);
                (!name.is_empty() && count > 0).then_some((conference.number, name, count))
            })
            .collect();

        // Conferences present in MESSAGES.DAT but missing from CONTROL.DAT.
        list.extend(
            counts
                .into_iter()
                .map(|(number, count)| (number, fl!(LANGUAGE_LOADER, "packet-conference-fallback-name", number = number), count)),
        );
        list.sort_by_key(|(number, _, _)| *number);
        list
    }
}

fn blue_wave_member(name: &str) -> bool {
    ["INF", "MIX", "FTI", "DAT"].iter().any(|extension| name.ends_with(&format!(".{extension}")))
}

fn blue_wave_body(data: &MessageData, message: &crate::blue_wave::Message) -> Res<Vec<u8>> {
    let offset = message.body_offset.checked_sub(1).ok_or("Invalid Blue Wave body offset")?;
    let length = message.body_len.checked_add(1).ok_or("Invalid Blue Wave body length")?;
    let bytes = data.read_range(offset, length)?;
    // Some doors (e.g. OLMS) omit the marker for some messages; the span is then all body text.
    if bytes.first() != Some(&b' ') {
        log::warn!(
            "Blue Wave area {} message {} at DAT offset {offset}: missing leading marker; retaining entire body span",
            message.area,
            message.number
        );
    }
    let bytes = bytes.strip_prefix(b" ").unwrap_or(&bytes);
    let mut body = Vec::with_capacity(bytes.len());
    let mut text = bytes.iter().copied().peekable();
    while let Some(byte) = text.next() {
        match byte {
            b'\r' => {
                if text.peek() == Some(&b'\n') {
                    text.next();
                }
                body.push(b'\n');
            }
            byte => body.push(byte),
        }
    }
    Ok(body)
}

mod index_date {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(date: &chrono::NaiveDateTime, serializer: S) -> Result<S::Ok, S::Error> {
        let utc = date.and_utc();
        (utc.timestamp(), utc.timestamp_subsec_nanos()).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<chrono::NaiveDateTime, D::Error> {
        let (seconds, nanos) = <(i64, u32)>::deserialize(deserializer)?;
        chrono::DateTime::from_timestamp(seconds, nanos)
            .map(|date| date.naive_utc())
            .ok_or_else(|| serde::de::Error::custom("invalid cached message date"))
    }
}

fn trim_field(field: &[u8]) -> String {
    String::from_utf8_lossy(field).trim().to_string()
}

fn parse_qwk_date(value: &str) -> Result<chrono::NaiveDateTime, chrono::ParseError> {
    chrono::NaiveDateTime::parse_from_str(value, "%m-%d-%y%H:%M").or_else(|_| chrono::NaiveDateTime::parse_from_str(value, "%m/%d/%y%H:%M"))
}

fn parse_qwk_number(data: &[u8]) -> Result<u32, Box<dyn Error>> {
    // Trim spaces and parse - avoid String allocation
    let trimmed = data.trim_ascii();
    if trimmed.is_empty() {
        return Ok(0);
    }

    // Parse directly from bytes
    std::str::from_utf8(trimmed)?.parse::<u32>().map_err(std::convert::Into::into)
}
