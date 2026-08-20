use jamjam::qwk::control::ControlDat;
use jamjam::qwk::qwk_message::QWKMessage;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::Res;

#[cfg(test)]
pub mod tests;

#[derive(Clone)]
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
#[derive(Clone)]
pub struct MessageInfo {
    pub index: usize,
    pub number: u32,
    /// Message this one replies to, `0` when it starts a thread.
    pub ref_number: u32,
    pub conference: u16,
    pub from: String,
    pub to: String,
    pub subject: String,
    /// Subject with all `Re:` prefixes stripped, lowercased - the thread key.
    pub subject_key: String,
    pub date: chrono::NaiveDateTime,
    pub date_str: String,
    pub lines: u32,
    pub private: bool,
}

/// Strips any number of leading `Re:` / `Re[2]:` / `Fwd:` prefixes and lowercases the rest.
#[must_use]
pub fn normalize_subject(subject: &str) -> String {
    let mut rest = subject.trim();
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
    rest.to_ascii_lowercase()
}

pub struct QwkPackage {
    pub bbs_name: String,
    pub descriptors: Vec<MessageDescriptor>,
    /// Header index, parallel to `descriptors`.
    pub infos: Vec<MessageInfo>,
    pub control_file: ControlDat,
    messages_data: Arc<Vec<u8>>,                           // Keep the raw data for lazy loading
    message_cache: Arc<Mutex<HashMap<usize, QWKMessage>>>, // Thread-safe cache
}

impl Clone for QwkPackage {
    fn clone(&self) -> Self {
        Self {
            bbs_name: self.bbs_name.clone(),
            descriptors: self.descriptors.clone(),
            infos: self.infos.clone(),
            control_file: self.control_file.clone(),
            messages_data: self.messages_data.clone(),
            message_cache: self.message_cache.clone(), // Share the cache across clones
        }
    }
}

impl QwkPackage {
    pub fn load_from_file(path: impl AsRef<Path>) -> Res<Self> {
        let _timer = crate::perf::Timer::new("qwk::load_from_file");
        let path = path.as_ref();
        let file = fs::File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        let mut messages_dat: Option<Vec<u8>> = None;
        let mut control_dat: Option<Vec<u8>> = None;
        let mut bbs_id = String::new();

        // Extract relevant files from the archive
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let file_name = file.name().to_uppercase();

            if file_name.ends_with("MESSAGES.DAT") || file_name == "MESSAGES.DAT" {
                let mut buffer = Vec::with_capacity(file.size() as usize);
                file.read_to_end(&mut buffer)?;
                messages_dat = Some(buffer);

                if let Some(dot_pos) = file_name.find('.') {
                    if dot_pos > 0 {
                        bbs_id = file_name[..dot_pos].to_string();
                    }
                }
            } else if file_name == "CONTROL.DAT" {
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer)?;
                control_dat = Some(buffer);
            }
        }

        // CONTROL.DAT is required
        let control_data = control_dat.ok_or("CONTROL.DAT not found in archive")?;

        // Parse CONTROL.DAT
        let control_file = ControlDat::read(&control_data).map_err(|e| format!("Failed to parse CONTROL.DAT: {e:?}"))?;

        // Use BBS name from control file if we don't have one yet
        if !control_file.bbs_name.is_empty() && bbs_id.is_empty() {
            bbs_id = control_file.bbs_name.to_string();
        }

        // Parse just the headers, not full messages
        let messages_data = messages_dat.ok_or("MESSAGES.DAT not found in archive")?;
        let headers = Self::parse_headers(&messages_data);
        let messages_data = Arc::new(messages_data);

        // Use filename as fallback for BBS name
        if bbs_id.is_empty() {
            bbs_id = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        }

        Ok(QwkPackage {
            bbs_name: bbs_id,
            infos: Self::build_index(&messages_data, &headers),
            descriptors: headers,
            control_file,
            messages_data,
            message_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Reads every message header once so the list view can sort/filter/thread without I/O.
    fn build_index(data: &[u8], descriptors: &[MessageDescriptor]) -> Vec<MessageInfo> {
        let _timer = crate::perf::Timer::with("qwk::build_index", format!("{} messages", descriptors.len()));
        descriptors
            .iter()
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
                        from: String::new(),
                        to: String::new(),
                        subject: format!("<unreadable message #{}>", descriptor.number),
                        subject_key: String::new(),
                        date: chrono::NaiveDateTime::default(),
                        date_str: String::new(),
                        lines: 0,
                        private: false,
                    };
                };

                let subject = trim_field(&msg.subj);
                let date = msg.date_time();
                MessageInfo {
                    index,
                    number: msg.msg_number,
                    ref_number: msg.ref_msg_number,
                    conference: msg.conference_number,
                    from: trim_field(&msg.from),
                    to: trim_field(&msg.to),
                    subject_key: normalize_subject(&subject),
                    subject,
                    date,
                    date_str: date.format("%Y-%m-%d %H:%M").to_string(),
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
            return Err("Message index out of range".into());
        }

        // Check cache first
        {
            let cache: std::sync::MutexGuard<'_, HashMap<usize, QWKMessage>> = self.message_cache.lock().unwrap();
            if let Some(message) = cache.get(&index) {
                return Ok(message.clone());
            }
        }

        // Load message from raw data
        let header = &self.descriptors[index];
        let mut cursor = Cursor::new(&*self.messages_data);
        cursor.seek(SeekFrom::Start(header.offset))?;

        let msg = QWKMessage::read(&mut cursor, true)?;

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
                let name = trim_field(&conference.name);
                let count = counts.remove(&conference.number).unwrap_or(0);
                (!name.is_empty() && count > 0).then_some((conference.number, name, count))
            })
            .collect();

        // Conferences present in MESSAGES.DAT but missing from CONTROL.DAT.
        list.extend(counts.into_iter().map(|(number, count)| (number, format!("Conference {number}"), count)));
        list.sort_by_key(|(number, _, _)| *number);
        list
    }
}

fn trim_field(field: &[u8]) -> String {
    String::from_utf8_lossy(field).trim().to_string()
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
