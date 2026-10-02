use bstr::ByteSlice;
use i18n_embed_fl::fl;
use jamjam::qwk::control::ControlDat;
use jamjam::qwk::qwk_message::QWKMessage;
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
use bodies::MessageData;
pub use cache::ExtractionCache;

#[cfg(test)]
pub mod tests;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageDescriptor {
    pub number: u32,
    pub conference: u16,
    pub offset: u64,
    pub block_count: u32,
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
    /// Welcome, news and goodbye screens, bulletins and new files lists, in display order.
    pub files: Arc<Vec<PacketFile>>,
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

/// A text screen shipped next to the messages, like MultiMail's bulletin and new files viewer shows.
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
    control: Vec<u8>,
    messages: Vec<u8>,
    message_source: Option<Arc<MessageData>>,
    others: Vec<(String, Vec<u8>)>,
    bbs_name: String,
    metadata: Option<cache::MetadataIndex>,
    cache_identity: Option<cache::CacheIdentity>,
}

/// Picks the screens named in CONTROL.DAT and the `BLT*`, `NEWFILES*` and `NFILE*` files, like MultiMail.
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
        // Exact names first; otherwise the first file starting with it, as MultiMail matches them.
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
            files: self.files.clone(),
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
        Self::from_extracted(path, Self::extract_packet(path)?)
    }

    pub fn load_from_file_cached(path: impl AsRef<Path>, cache: &ExtractionCache) -> Res<Self> {
        let _timer = crate::perf::Timer::new("qwk::load_from_file_cached");
        let path = path.as_ref();
        let mut extracted = cache.load(path, || Self::extract_packet(path))?;
        let identity = extracted.cache_identity.take();
        let indexed = extracted.metadata.is_some();
        let mut package = Self::from_extracted(path, extracted)?;
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
            } else if !base_name.is_empty() && !base_name.ends_with(".NDX") && entry.original_size() <= MAX_PACKET_FILE_SIZE {
                // The screen names are only known once CONTROL.DAT is parsed, which may come later.
                let name = entry.name().replace('\\', "/").rsplit('/').next().unwrap_or_default().to_string();
                let data = archive.read(&entry)?;
                if data.len() as u64 <= MAX_PACKET_FILE_SIZE {
                    others.push((name, data));
                }
            } else {
                archive.skip(&entry)?;
            }
        }

        Ok(ExtractedPacket {
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
        let ExtractedPacket {
            control,
            messages,
            message_source,
            others,
            bbs_name: mut bbs_id,
            metadata,
            cache_identity: _,
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

        Ok(QwkPackage {
            bbs_name: bbs_id,
            infos,
            descriptors: headers,
            files: Arc::new(packet_files(&control_file, others)),
            control_file,
            messages_data,
            message_cache: Arc::new(Mutex::new(HashMap::new())),
            thread_rows,
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
