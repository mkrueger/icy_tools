use jamjam::qwk::control::ControlDat;
use jamjam::qwk::qwk_message::QWKMessage;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::Res;

#[derive(Clone)]
pub struct MessageDescriptor {
    pub number: u32,
    pub conference: u16,
    pub offset: u64,
    pub block_count: u32,
}

pub struct QwkPackage {
    pub bbs_name: String,
    pub descriptors: Vec<MessageDescriptor>,
    pub control_file: ControlDat,
    messages_data: Arc<Vec<u8>>,                           // Keep the raw data for lazy loading
    message_cache: Arc<Mutex<HashMap<usize, QWKMessage>>>, // Thread-safe cache
}

impl Clone for QwkPackage {
    fn clone(&self) -> Self {
        Self {
            bbs_name: self.bbs_name.clone(),
            descriptors: self.descriptors.clone(),
            control_file: self.control_file.clone(),
            messages_data: self.messages_data.clone(),
            message_cache: self.message_cache.clone(), // Share the cache across clones
        }
    }
}

impl QwkPackage {
    pub fn load_from_file(path: impl AsRef<Path>) -> Res<Self> {
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
        let control_file = ControlDat::read(&control_data).map_err(|e| format!("Failed to parse CONTROL.DAT: {:?}", e))?;

        // Use BBS name from control file if we don't have one yet
        if !control_file.bbs_name.is_empty() && bbs_id.is_empty() {
            bbs_id = control_file.bbs_name.to_string();
        }

        // Parse just the headers, not full messages
        let messages_data = messages_dat.ok_or("MESSAGES.DAT not found in archive")?;
        let headers = Self::parse_headers(&messages_data)?;
        let messages_data = Arc::new(messages_data);

        // Use filename as fallback for BBS name
        if bbs_id.is_empty() {
            bbs_id = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        }

        Ok(QwkPackage {
            bbs_name: bbs_id,
            descriptors: headers,
            control_file,
            messages_data,
            message_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn parse_headers(data: &[u8]) -> Res<Vec<MessageDescriptor>> {
        let mut headers = Vec::with_capacity(data.len() / 256); // Pre-allocate estimated capacity
        const HEADER_SIZE: usize = 128;

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

        Ok(headers)
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
            let mut cache = self.message_cache.lock().unwrap();

            // Optional: Limit cache size to prevent excessive memory usage
            const MAX_CACHE_SIZE: usize = 1000;
            if cache.len() >= MAX_CACHE_SIZE {
                // Remove oldest entries (simple FIFO for now)
                // In production, you might want LRU eviction
                let keys_to_remove: Vec<usize> = cache.keys().take(cache.len() - MAX_CACHE_SIZE / 2).cloned().collect();
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
    pub fn cache_stats(&self) -> (usize, usize) {
        let cache = self.message_cache.lock().unwrap();
        (cache.len(), self.descriptors.len())
    }

    pub fn message_count(&self) -> usize {
        self.descriptors.len()
    }
}

fn parse_qwk_number(data: &[u8]) -> Result<u32, Box<dyn Error>> {
    // Trim spaces and parse - avoid String allocation
    let trimmed = data.trim_ascii();
    if trimmed.is_empty() {
        return Ok(0);
    }

    // Parse directly from bytes
    std::str::from_utf8(trimmed)?.parse::<u32>().map_err(|e| e.into())
}
