use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bincode::Options;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::bodies::{MessageData, CHUNK_SIZE};
use super::{blue_wave_member, ExtractedPacket, MessageDescriptor, MessageInfo, PacketFormat, QwkPackage, MAX_ARCHIVE_ENTRY_SIZE, MAX_PACKET_FILE_SIZE};
use crate::Res;

const VERSION: u32 = 1;
const PREFIX: &str = "packet-v1-";
const PENDING_PREFIX: &str = ".pending-v1-";
const MANIFEST: &str = "manifest.toml";
const MAX_MANIFEST_SIZE: u64 = 1024 * 1024;
const DAY: u64 = 24 * 60 * 60;
const INDEX_FILE: &str = "index-v4.bin";
const INDEX_VERSION: u32 = 4;
const MAX_INDEX_SIZE: u64 = 1024 * 1024 * 1024;
const INDEX_CHUNK_MESSAGES: usize = 4096;

fn chunk_checksums(bytes: &[u8]) -> Vec<u32> {
    bytes.par_chunks(CHUNK_SIZE).map(crc32fast::hash).collect()
}

/// Decompressed packet data in the user's cache directory. Zero days disables caching.
#[derive(Clone, Debug)]
pub struct ExtractionCache {
    directory: PathBuf,
    retention_days: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SourceStamp {
    path: Vec<u8>,
    size: u64,
    modified_seconds: u64,
    modified_nanos: u32,
    changed: Option<(i64, i64)>,
}

impl SourceStamp {
    fn read(path: &Path) -> Res<Self> {
        let canonical = path.canonicalize()?;
        let metadata = fs::metadata(&canonical)?;
        let modified = metadata.modified()?.duration_since(UNIX_EPOCH)?;
        #[cfg(unix)]
        let changed = {
            use std::os::unix::fs::MetadataExt;
            Some((metadata.ctime(), metadata.ctime_nsec()))
        };
        #[cfg(not(unix))]
        let changed = None;
        Ok(Self {
            path: canonical.as_os_str().as_encoded_bytes().to_vec(),
            size: metadata.len(),
            modified_seconds: modified.as_secs(),
            modified_nanos: modified.subsec_nanos(),
            changed,
        })
    }

    fn key(&self) -> String {
        // Stable FNV-1a names; the manifest also checks the complete stamp for collisions.
        let bytes = self
            .path
            .iter()
            .copied()
            .chain(self.size.to_le_bytes())
            .chain(self.modified_seconds.to_le_bytes())
            .chain(self.modified_nanos.to_le_bytes())
            .chain(self.changed.unwrap_or_default().0.to_le_bytes())
            .chain(self.changed.unwrap_or_default().1.to_le_bytes());
        let hash = bytes.fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3));
        format!("{PREFIX}{hash:016x}")
    }
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u32,
    source: SourceStamp,
    bbs_name: String,
    #[serde(default)]
    format: PacketFormat,
    files: Vec<CachedFile>,
}

#[derive(Serialize, Deserialize)]
struct CachedFile {
    name: String,
    size: u64,
    crc32: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    chunks: Vec<u32>,
}

pub(super) struct CacheIdentity {
    format: PacketFormat,
    source: SourceStamp,
    messages_size: u64,
    messages_crc32: u32,
}

#[derive(Serialize, Deserialize)]
pub(super) struct MetadataIndex {
    pub descriptors: Vec<MessageDescriptor>,
    pub infos: Vec<MessageInfo>,
    pub threads: Vec<crate::threading::Row>,
}

#[derive(Serialize)]
struct MetadataRef<'a> {
    descriptors: &'a [MessageDescriptor],
    infos: &'a [MessageInfo],
    threads: &'a [crate::threading::Row],
}

#[derive(Serialize, Deserialize)]
struct IndexChunks {
    #[serde(serialize_with = "serialize_chunks")]
    chunks: Vec<Vec<u8>>,
}

fn serialize_chunks<S: serde::Serializer>(chunks: &[Vec<u8>], serializer: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    struct Bytes<'a>(&'a [u8]);
    impl Serialize for Bytes<'_> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_bytes(self.0)
        }
    }
    let mut sequence = serializer.serialize_seq(Some(chunks.len()))?;
    for chunk in chunks {
        sequence.serialize_element(&Bytes(chunk))?;
    }
    sequence.end()
}

#[derive(Deserialize)]
struct BorrowedChunks<'a> {
    #[serde(borrow)]
    chunks: Vec<&'a [u8]>,
}

#[derive(Serialize, Deserialize)]
struct IndexRecord<T> {
    version: u32,
    language: String,
    source: SourceStamp,
    messages_size: u64,
    messages_crc32: u32,
    index: T,
}

impl ExtractionCache {
    #[must_use]
    pub fn new(directory: PathBuf, retention_days: u32) -> Self {
        Self { directory, retention_days }
    }

    pub fn default_directory() -> Res<PathBuf> {
        Ok(directories::ProjectDirs::from("com", "GitHub", "icy_mail")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no package cache directory available"))?
            .cache_dir()
            .join("packet-extractions"))
    }

    pub(super) fn load(&self, path: &Path, extract: impl FnOnce() -> Res<ExtractedPacket>) -> Res<ExtractedPacket> {
        if let Err(error) = self.cleanup(SystemTime::now()) {
            log::warn!("unable to clean packet extraction cache {}: {error}", self.directory.display());
        }
        if self.retention_days == 0 {
            return extract();
        }
        let source = match SourceStamp::read(path) {
            Ok(source) => source,
            Err(error) => {
                log::warn!("unable to identify packet {} for caching: {error}", path.display());
                return extract();
            }
        };
        let destination = self.directory.join(source.key());
        match self.read(&destination, &source, path) {
            Ok(Some(packet)) => {
                if SourceStamp::read(path)? == source {
                    if let Err(error) = File::options()
                        .write(true)
                        .open(destination.join(MANIFEST))
                        .and_then(|file| file.set_modified(SystemTime::now()))
                    {
                        log::warn!("unable to refresh packet cache age {}: {error}", destination.display());
                    }
                    log::debug!("packet extraction cache hit for {}", path.display());
                    return Ok(packet);
                }
                log::warn!("packet {} changed while reading its cache; extracting the current source", path.display());
            }
            Ok(None) => {}
            Err(error) => {
                log::warn!("invalid packet extraction cache {}: {error}; extracting the source", destination.display());
                if let Err(error) = remove_entry(&destination) {
                    log::warn!("unable to remove invalid packet cache {}: {error}", destination.display());
                }
            }
        }
        let mut packet = extract()?;
        if SourceStamp::read(path)? == source {
            match self.store(&destination, &source, &packet) {
                Ok(identity) => packet.cache_identity = Some(identity),
                Err(error) => log::warn!("unable to store packet extraction cache for {}: {error}", path.display()),
            }
        } else {
            log::warn!("packet {} changed during extraction; not caching the result", path.display());
        }
        Ok(packet)
    }

    fn read(&self, directory: &Path, source: &SourceStamp, path: &Path) -> Res<Option<ExtractedPacket>> {
        let metadata = match fs::symlink_metadata(directory) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if !metadata.file_type().is_dir() {
            return Err(invalid("cache entry is not a directory").into());
        }
        let manifest = read_file(&directory.join(MANIFEST), MAX_MANIFEST_SIZE)?;
        let mut manifest: Manifest = toml::from_str(std::str::from_utf8(&manifest)?)?;
        if manifest.version != VERSION || &manifest.source != source || manifest.files.len() < 2 {
            return Err(invalid("cache manifest does not match the packet").into());
        }
        let identity = CacheIdentity {
            format: manifest.format,
            source: source.clone(),
            messages_size: manifest.files[1].size,
            messages_crc32: manifest.files[1].crc32,
        };
        let metadata = match self.read_index(directory, &identity) {
            Ok(metadata) => metadata,
            Err(error) => {
                log::warn!("invalid packet metadata cache {}: {error}; rebuilding its index", directory.display());
                if let Err(error) = fs::remove_file(directory.join(INDEX_FILE)) {
                    if error.kind() != io::ErrorKind::NotFound {
                        log::warn!("unable to remove invalid metadata cache {}: {error}", directory.display());
                    }
                }
                None
            }
        };
        let mut message_source = None;
        let mut upgraded = false;
        let mut files = Vec::with_capacity(manifest.files.len());
        for (index, entry) in manifest.files.iter_mut().enumerate() {
            let limit = if index < 2 || manifest.format == PacketFormat::BlueWave && blue_wave_member(&entry.name.to_ascii_uppercase()) {
                MAX_ARCHIVE_ENTRY_SIZE
            } else {
                MAX_PACKET_FILE_SIZE
            };
            if entry.size > limit {
                return Err(invalid("cached file exceeds its size limit").into());
            }
            if index == 1 && (metadata.is_some() || manifest.format == PacketFormat::BlueWave) && !entry.chunks.is_empty() {
                let file_path = directory.join("1.dat");
                if !fs::symlink_metadata(&file_path)?.file_type().is_file() {
                    return Err(invalid("cached messages are not a regular file").into());
                }
                let original = path.canonicalize()?;
                let stamp = source.clone();
                let expected_size = entry.size;
                let expected_crc32 = entry.crc32;
                let manifest_path = directory.join(MANIFEST);
                message_source = Some(std::sync::Arc::new(MessageData::file(
                    File::open(file_path)?,
                    entry.size,
                    entry.crc32,
                    entry.chunks.clone(),
                    move || {
                        // Remove only the manifest: existing readers keep their open raw-data handle.
                        if let Err(error) = fs::remove_file(&manifest_path) {
                            if error.kind() != io::ErrorKind::NotFound {
                                log::warn!("unable to invalidate corrupt packet cache {}: {error}", manifest_path.display());
                            }
                        }
                        if SourceStamp::read(&original)? != stamp {
                            return Err(invalid("source packet changed since opening").into());
                        }
                        let recovered = QwkPackage::extract_packet(&original)?.messages;
                        if SourceStamp::read(&original)? != stamp || recovered.len() as u64 != expected_size || crc32fast::hash(&recovered) != expected_crc32 {
                            return Err(invalid("source packet does not match cached message metadata").into());
                        }
                        Ok(recovered)
                    },
                )?));
                files.push((entry.name.clone(), Vec::new()));
                continue;
            }
            // Archive names are metadata only, never used as filesystem paths.
            let data = read_file(&directory.join(format!("{index}.dat")), entry.size)?;
            if data.len() as u64 != entry.size || crc32fast::hash(&data) != entry.crc32 {
                return Err(invalid("cached file size or checksum does not match").into());
            }
            if index == 1 {
                let chunks = chunk_checksums(&data);
                if entry.chunks != chunks {
                    entry.chunks = chunks;
                    upgraded = true;
                }
            }
            files.push((entry.name.clone(), data));
        }
        if upgraded {
            let bytes = toml::to_string(&manifest)?;
            if bytes.len() as u64 > MAX_MANIFEST_SIZE {
                return Err(invalid("cache manifest exceeds its size limit").into());
            }
            if let Err(error) = crate::drafts::atomic_write(&directory.join(MANIFEST), |file| {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    file.set_permissions(fs::Permissions::from_mode(0o600))?;
                }
                file.write_all(bytes.as_bytes())?;
                Ok(())
            }) {
                log::warn!("unable to upgrade packet chunk checksums {}: {error}", directory.display());
            }
        }
        let mut files = files.into_iter();
        let control = files.next().expect("checked manifest length").1;
        let messages = files.next().expect("checked manifest length").1;
        Ok(Some(ExtractedPacket {
            format: manifest.format,
            control,
            messages,
            message_source,
            others: files.collect(),
            bbs_name: manifest.bbs_name,
            metadata,
            cache_identity: Some(identity),
        }))
    }

    fn read_index(&self, directory: &Path, identity: &CacheIdentity) -> Res<Option<MetadataIndex>> {
        let _timer = crate::perf::Timer::new("qwk::read_cached_index");
        let bytes = match read_file(&directory.join(INDEX_FILE), MAX_INDEX_SIZE) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let checksum = bytes.get(..4).ok_or_else(|| invalid("truncated metadata cache"))?;
        let expected = u32::from_le_bytes(checksum.try_into()?);
        let payload = &bytes[4..];
        if crc32fast::hash(payload) != expected {
            return Err(invalid("metadata cache checksum does not match").into());
        }
        if payload.get(..4) != Some(INDEX_VERSION.to_le_bytes().as_slice()) {
            return Err(invalid("unsupported metadata cache version").into());
        }
        let record: IndexRecord<BorrowedChunks<'_>> = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .with_limit(MAX_INDEX_SIZE)
            .deserialize(payload)?;
        if record.version != INDEX_VERSION
            || record.language != index_language()
            || record.source != identity.source
            || record.messages_size != identity.messages_size
            || record.messages_crc32 != identity.messages_crc32
        {
            return Err(invalid("metadata cache does not match the extracted message data").into());
        }
        let parts = record
            .index
            .chunks
            .par_iter()
            .map(|bytes| {
                let part: MetadataIndex = bincode::DefaultOptions::new()
                    .with_fixint_encoding()
                    .with_limit(MAX_INDEX_SIZE)
                    .deserialize(bytes)?;
                if part.infos.len() > INDEX_CHUNK_MESSAGES || part.descriptors.len() != part.infos.len() || part.threads.len() != part.infos.len() {
                    return Err(Box::new(bincode::ErrorKind::Custom("invalid metadata chunk lengths".into())));
                }
                Ok(part)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let count = parts.iter().map(|part| part.infos.len()).sum();
        let mut index = MetadataIndex {
            descriptors: Vec::with_capacity(count),
            infos: Vec::with_capacity(count),
            threads: Vec::with_capacity(count),
        };
        for part in parts {
            index.descriptors.extend(part.descriptors);
            index.infos.extend(part.infos);
            index.threads.extend(part.threads);
        }
        Self::validate_index(&index, identity.messages_size)?;
        if index
            .descriptors
            .iter()
            .any(|descriptor| descriptor.body_len.is_some() != (identity.format == PacketFormat::BlueWave))
        {
            return Err(invalid("metadata cache uses the wrong packet format").into());
        }
        Ok(Some(index))
    }

    pub(super) fn store_index(&self, identity: &CacheIdentity, package: &QwkPackage) -> Res<()> {
        let directory = self.directory.join(identity.source.key());
        let manifest: Manifest = toml::from_str(std::str::from_utf8(&read_file(&directory.join(MANIFEST), MAX_MANIFEST_SIZE)?)?)?;
        if manifest.source != identity.source
            || manifest.format != identity.format
            || package.format() != identity.format
            || manifest.files.len() < 2
            || manifest.files[1].size != identity.messages_size
            || manifest.files[1].crc32 != identity.messages_crc32
        {
            return Err(invalid("extraction cache changed before the metadata index was written").into());
        }
        let threads = package.cached_threads().ok_or_else(|| invalid("thread index was not built"))?;
        let chunks = package
            .infos
            .par_chunks(INDEX_CHUNK_MESSAGES)
            .enumerate()
            .map(|(chunk, infos)| {
                let start = chunk * INDEX_CHUNK_MESSAGES;
                let end = start + infos.len();
                bincode::DefaultOptions::new()
                    .with_fixint_encoding()
                    .with_limit(MAX_INDEX_SIZE)
                    .serialize(&MetadataRef {
                        descriptors: &package.descriptors[start..end],
                        infos,
                        threads: &threads[start..end],
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let record = IndexRecord {
            version: INDEX_VERSION,
            language: index_language(),
            source: identity.source.clone(),
            messages_size: identity.messages_size,
            messages_crc32: identity.messages_crc32,
            index: IndexChunks { chunks },
        };
        let payload = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .with_limit(MAX_INDEX_SIZE - 4)
            .serialize(&record)?;
        let checksum = crc32fast::hash(&payload).to_le_bytes();
        crate::drafts::atomic_write(&directory.join(INDEX_FILE), |file| {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                file.set_permissions(fs::Permissions::from_mode(0o600))?;
            }
            file.write_all(&checksum)?;
            file.write_all(&payload)?;
            Ok(())
        })?;
        Ok(())
    }

    fn store(&self, destination: &Path, source: &SourceStamp, packet: &ExtractedPacket) -> Res<CacheIdentity> {
        create_private_directory(&self.directory, true)?;
        let messages_crc32 = crc32fast::hash(&packet.messages);
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stage = loop {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let candidate = self.directory.join(format!("{PENDING_PREFIX}{}-{id}", std::process::id()));
            match create_private_directory(&candidate, false) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        };
        let result = (|| -> Res<()> {
            let mut manifest = Manifest {
                version: VERSION,
                source: source.clone(),
                bbs_name: packet.bbs_name.clone(),
                format: packet.format,
                files: Vec::new(),
            };
            let files = [("CONTROL.DAT", packet.control.as_slice()), ("MESSAGES.DAT", packet.messages.as_slice())]
                .into_iter()
                .chain(packet.others.iter().map(|(name, data)| (name.as_str(), data.as_slice())));
            for (index, (name, data)) in files.enumerate() {
                write_file(&stage.join(format!("{index}.dat")), data)?;
                manifest.files.push(CachedFile {
                    name: name.to_owned(),
                    size: data.len() as u64,
                    crc32: if index == 1 { messages_crc32 } else { crc32fast::hash(data) },
                    chunks: if index == 1 { chunk_checksums(data) } else { Vec::new() },
                });
            }

            let manifest = toml::to_string(&manifest)?;
            if manifest.len() as u64 > MAX_MANIFEST_SIZE {
                return Err(invalid("cache manifest exceeds its size limit").into());
            }
            write_file(&stage.join(MANIFEST), manifest.as_bytes())?;
            match fs::rename(&stage, destination) {
                Ok(()) => Ok(()),
                Err(error) if matches!(error.kind(), io::ErrorKind::AlreadyExists | io::ErrorKind::DirectoryNotEmpty) && destination.is_dir() => {
                    // Another loader published this packet while we were extracting.
                    log::debug!("packet cache already published at {}: {error}", destination.display());
                    Ok(())
                }
                Err(error) => Err(error.into()),
            }
        })();
        if stage.exists() {
            if let Err(error) = remove_entry(&stage) {
                log::warn!("unable to remove unfinished packet cache {}: {error}", stage.display());
            }
        }
        result?;
        Ok(CacheIdentity {
            format: packet.format,
            source: source.clone(),
            messages_size: packet.messages.len() as u64,
            messages_crc32,
        })
    }

    fn validate_index(index: &MetadataIndex, message_bytes: u64) -> io::Result<()> {
        let count = index.infos.len();
        if index.descriptors.len() != count || index.threads.len() != count {
            return Err(invalid("metadata cache contains inconsistent index lengths"));
        }
        for (position, (descriptor, info)) in index.descriptors.iter().zip(&index.infos).enumerate() {
            let length = descriptor.body_len.unwrap_or(u64::from(descriptor.block_count) * 128);
            let end = descriptor.offset.checked_add(length);
            let invalid_alignment = if descriptor.body_len.is_some() {
                descriptor.block_count != 0
            } else {
                descriptor.offset < 128 || descriptor.offset % 128 != 0 || descriptor.block_count == 0
            };
            if invalid_alignment
                || end.is_none_or(|end| end > message_bytes)
                || info.index != position
                || info.number != descriptor.number
                || info.conference != descriptor.conference
            {
                return Err(invalid("metadata cache contains an invalid message descriptor"));
            }
        }
        let mut seen = vec![false; count];
        let mut previous_depth = 0u16;
        for (position, row) in index.threads.iter().enumerate() {
            if row.index >= count
                || std::mem::replace(&mut seen[row.index], true)
                || row.descendants as usize > count - position - 1
                || row.has_children != (row.descendants > 0)
                || (position == 0 && row.depth != 0)
                || row.depth > previous_depth.saturating_add(1)
            {
                return Err(invalid("metadata cache contains invalid thread rows"));
            }
            previous_depth = row.depth;
        }
        Ok(())
    }

    fn cleanup(&self, now: SystemTime) -> Res<()> {
        let entries = match fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let published = name
                .strip_prefix(PREFIX)
                .is_some_and(|key| key.len() == 16 && key.bytes().all(|byte| byte.is_ascii_hexdigit()));
            let pending = name.strip_prefix(PENDING_PREFIX).is_some_and(|suffix| {
                suffix.split_once('-').is_some_and(|(process, counter)| {
                    !process.is_empty() && !counter.is_empty() && process.bytes().chain(counter.bytes()).all(|byte| byte.is_ascii_digit())
                })
            });
            if (!published && !pending) || !entry.file_type()?.is_dir() {
                continue;
            }
            let path = entry.path();
            let timestamp = if published { path.join(MANIFEST) } else { path.clone() };
            let metadata = match fs::symlink_metadata(&timestamp) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => fs::symlink_metadata(&path)?,
                Err(error) => return Err(error.into()),
            };
            let age = now.duration_since(metadata.modified()?).unwrap_or(Duration::ZERO);
            let days = if pending { self.retention_days.max(1) } else { self.retention_days };
            if age >= Duration::from_secs(u64::from(days) * DAY) {
                if let Err(error) = remove_entry(&path) {
                    log::warn!("unable to remove expired packet cache {}: {error}", path.display());
                }
            }
        }
        Ok(())
    }
}

fn index_language() -> String {
    crate::LANGUAGE_LOADER
        .current_languages()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn read_file(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() > limit {
        return Err(invalid("cache file is not a regular file within the size limit"));
    }
    let mut data = Vec::new();
    let capacity = usize::try_from(metadata.len()).map_err(|_| invalid("cache file cannot fit in memory on this platform"))?;
    data.try_reserve_exact(capacity).map_err(io::Error::other)?;
    File::open(path)?.take(limit + 1).read_to_end(&mut data)?;
    if data.len() as u64 > limit {
        return Err(invalid("cache file grew past its size limit"));
    }
    Ok(data)
}

fn write_file(path: &Path, data: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(data)?;
    file.sync_all()
}

fn create_private_directory(path: &Path, recursive: bool) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.recursive(recursive).create(path)
}

fn remove_entry(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qwk::{tests::TempDir, QwkPackage};
    use icy_engine::TextPane;

    #[test]
    #[ignore = "requires ICY_MAIL_TEST_PACKET with an existing default-directory extraction cache"]
    fn existing_packet_cache_timings() {
        let path = PathBuf::from(std::env::var("ICY_MAIL_TEST_PACKET").expect("set ICY_MAIL_TEST_PACKET to the cached packet"));
        let cache = ExtractionCache::new(ExtractionCache::default_directory().unwrap(), 30);
        let directory = entry(&cache, &path);
        assert!(directory.join(MANIFEST).is_file(), "this benchmark requires an existing extraction cache");
        let start = std::time::Instant::now();
        let first = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        println!("initial open (upgrades an extraction-only cache): {:?}", start.elapsed());
        let count = first.infos.len();
        assert!(first.cached_threads().is_some());
        println!("messages: {count}; index bytes: {}", fs::metadata(directory.join(INDEX_FILE)).unwrap().len());
        let body = first.read_message(0).unwrap().text;
        drop(first);
        for run in 0..3 {
            let start = std::time::Instant::now();
            let package = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
            println!("warm metadata cache load {run}: {:?}", start.elapsed());
            let opened = start;
            assert_eq!(package.infos.len(), count);
            assert!(package.needs_preload(), "opening metadata must not eagerly read all raw bodies");
            let start = std::time::Instant::now();
            assert_eq!(package.read_message(0).unwrap().text, body);
            println!("first verified message: {:?}; open-to-first body: {:?}", start.elapsed(), opened.elapsed());
            let start = std::time::Instant::now();
            let classic = crate::reader::render_body(&body).unwrap();
            let wide = crate::reader::render_body_wide(&body).unwrap();
            println!(
                "classic + wide preparation: {:?}; classic rows: {}; wide rows: {}",
                start.elapsed(),
                classic.height(),
                wide.height()
            );
            let start = std::time::Instant::now();
            assert!(package.preload_messages(|| false).unwrap());
            assert!(!package.needs_preload());
            println!("background raw-byte preload: {:?}", start.elapsed());
            let start = std::time::Instant::now();
            let fresh = crate::threading::build_threads(&package.infos.iter().collect::<Vec<_>>());
            println!("fresh thread construction: {:?}", start.elapsed());
            assert_eq!(package.cached_threads().unwrap(), fresh.as_slice());
            drop(fresh);
            let mut reader = crate::reader::Reader::default();
            let start = std::time::Instant::now();
            reader.set_package(std::sync::Arc::new(package));
            println!("reader initialization: {:?}", start.elapsed());
            reader.view_mode = crate::reader::ViewMode::Threads;
            let start = std::time::Instant::now();
            reader.rebuild_messages();
            println!("cached threaded-list initialization: {:?}", start.elapsed());
            assert_eq!(reader.messages.len(), count);
        }
    }

    fn fixture() -> (TempDir, PathBuf, ExtractionCache) {
        let (dir, _) = crate::qwk::tests::load();
        let path = dir.path().join("TEST.QWK");
        let cache = ExtractionCache::new(dir.path().join("cache"), 30);
        (dir, path, cache)
    }

    #[test]
    fn native_body_ranges_are_not_interpreted_as_qwk_blocks() {
        let (_dir, path, cache) = fixture();
        let package = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let mut index = MetadataIndex {
            descriptors: package.descriptors.clone(),
            infos: package.infos.clone(),
            threads: package.cached_threads().unwrap().to_vec(),
        };
        for descriptor in &mut index.descriptors {
            descriptor.offset = 1;
            descriptor.block_count = 0;
            descriptor.body_len = Some(3);
        }
        ExtractionCache::validate_index(&index, 4).unwrap();
        assert!(ExtractionCache::validate_index(&index, 3).is_err());
        index.descriptors[0].body_len = None;
        assert!(ExtractionCache::validate_index(&index, 4).is_err());
        index.descriptors[0].body_len = Some(1);
        index.descriptors[0].offset = u64::MAX;
        assert!(ExtractionCache::validate_index(&index, u64::MAX).is_err());
        index.descriptors[0].body_len = Some(0);
        index.descriptors[0].offset = 4;
        ExtractionCache::validate_index(&index, 4).unwrap();
    }

    fn entry(cache: &ExtractionCache, path: &Path) -> PathBuf {
        cache.directory.join(SourceStamp::read(path).unwrap().key())
    }

    #[test]
    fn cached_packet_matches_uncached_and_skips_extraction() {
        let (_dir, path, cache) = fixture();
        let original = QwkPackage::load_from_file(&path).unwrap();
        let cold = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let extracted = cache.load(&path, || panic!("cache hit must not extract the archive")).unwrap();
        assert!(extracted.metadata.is_some(), "warm cache must contain decoded metadata");
        assert!(extracted.messages.is_empty(), "warm load must not read all bodies");
        assert!(extracted.message_source.is_some());
        for package in [&cold, &warm] {
            assert_eq!(package.bbs_name, original.bbs_name);
            assert_eq!(package.conferences(), original.conferences());
            assert_eq!(package.infos.len(), original.infos.len());
            for (index, info) in package.infos.iter().enumerate() {
                assert_eq!(info.subject.as_str(), original.infos[index].subject.as_str());
                assert_eq!(info.subject_key, original.infos[index].subject_key);
                assert_eq!(package.read_message(index).unwrap().text, original.read_message(index).unwrap().text);
            }
            assert_eq!(package.files.len(), original.files.len());
            for (cached, original) in package.files.iter().zip(original.files.iter()) {
                assert_eq!(cached.name, original.name);
                assert_eq!(cached.kind, original.kind);
                assert_eq!(cached.data, original.data);
            }
        }
        assert_eq!(fs::read_dir(&cache.directory).unwrap().count(), 1);
    }

    #[test]
    fn extraction_only_cache_is_upgraded_without_decompressing_again() {
        let (_dir, path, cache) = fixture();
        cache.load(&path, || QwkPackage::extract_packet(&path)).unwrap();
        let directory = entry(&cache, &path);
        assert!(!directory.join(INDEX_FILE).exists());
        let extracted = cache.load(&path, || panic!("migration must reuse extracted data")).unwrap();
        assert!(extracted.metadata.is_none());
        let raw_modified = fs::metadata(directory.join("1.dat")).unwrap().modified().unwrap();
        let migrated = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        assert!(migrated.cached_threads().is_some());
        assert_eq!(fs::metadata(directory.join("1.dat")).unwrap().modified().unwrap(), raw_modified);
        let warm = cache.load(&path, || panic!("migrated cache must skip extraction")).unwrap();
        assert!(warm.metadata.is_some());
    }

    #[test]
    fn older_manifests_gain_chunk_checksums_without_reextracting() {
        let (_dir, path, cache) = fixture();
        let original = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let directory = entry(&cache, &path);
        let modified = fs::metadata(directory.join("1.dat")).unwrap().modified().unwrap();
        let mut manifest: Manifest = toml::from_str(&fs::read_to_string(directory.join(MANIFEST)).unwrap()).unwrap();
        manifest.files[1].chunks.clear();
        fs::write(directory.join(MANIFEST), toml::to_string(&manifest).unwrap()).unwrap();
        let upgrade = cache.load(&path, || panic!("old manifests must reuse existing raw data")).unwrap();
        assert!(!upgrade.messages.is_empty(), "migration verifies the complete old body file once");
        assert!(upgrade.metadata.is_some());
        assert_eq!(fs::metadata(directory.join("1.dat")).unwrap().modified().unwrap(), modified);
        let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        assert!(warm.needs_preload());
        for index in 0..original.message_count() {
            assert_eq!(warm.read_message(index).unwrap().text, original.read_message(index).unwrap().text);
        }
    }

    #[test]
    fn invalid_chunk_count_rebuilds_and_chunk_checksum_errors_recover() {
        let (_dir, path, cache) = fixture();
        let original = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let directory = entry(&cache, &path);
        for count_error in [true, false] {
            let mut manifest: Manifest = toml::from_str(&fs::read_to_string(directory.join(MANIFEST)).unwrap()).unwrap();
            if count_error {
                manifest.files[1].chunks.push(1);
            } else {
                manifest.files[1].chunks[0] ^= 1;
            }
            fs::write(directory.join(MANIFEST), toml::to_string(&manifest).unwrap()).unwrap();
            let package = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
            assert_eq!(package.read_message(0).unwrap().text, original.read_message(0).unwrap().text);
            QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        }
    }

    #[test]
    fn corrupt_bodies_cannot_recover_from_a_changed_or_missing_source() {
        let (_dir, path, cache) = fixture();
        QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let directory = entry(&cache, &path);
        let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let original = fs::read(directory.join("1.dat")).unwrap();
        let mut corrupt = original.clone();
        corrupt[128 + 71] ^= 1;
        fs::write(directory.join("1.dat"), corrupt).unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified + Duration::from_secs(2))
            .unwrap();
        assert!(warm.read_message(0).unwrap_err().to_string().contains("source packet changed"));
        assert!(warm.preload_messages(|| false).is_err());

        QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let directory = entry(&cache, &path);
        let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let mut corrupt = fs::read(directory.join("1.dat")).unwrap();
        corrupt[128 + 71] ^= 1;
        fs::write(directory.join("1.dat"), corrupt).unwrap();
        fs::remove_file(path).unwrap();
        assert!(warm.read_message(0).is_err());
        assert!(warm.preload_messages(|| false).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn an_open_packet_keeps_its_raw_data_handle_after_cache_expiry() {
        let (_dir, path, cache) = fixture();
        let original = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        cache.cleanup(SystemTime::now() + Duration::from_secs(31 * DAY)).unwrap();
        assert!(!entry(&cache, &path).exists());
        assert_eq!(warm.read_message(0).unwrap().text, original.read_message(0).unwrap().text);
        assert!(warm.preload_messages(|| false).unwrap());
    }

    fn rewrite_metadata(directory: &Path, mutate: impl FnOnce(&mut IndexRecord<IndexChunks>)) {
        let bytes = fs::read(directory.join(INDEX_FILE)).unwrap();
        let mut record: IndexRecord<IndexChunks> = bincode::DefaultOptions::new().with_fixint_encoding().deserialize(&bytes[4..]).unwrap();
        mutate(&mut record);
        let payload = bincode::DefaultOptions::new().with_fixint_encoding().serialize(&record).unwrap();
        let mut output = crc32fast::hash(&payload).to_le_bytes().to_vec();
        output.extend(payload);
        fs::write(directory.join(INDEX_FILE), output).unwrap();
    }

    #[test]
    fn corrupt_stale_or_invalid_metadata_rebuilds_only_the_index() {
        let (_dir, path, cache) = fixture();
        let original = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let directory = entry(&cache, &path);
        let raw_modified = fs::metadata(directory.join("1.dat")).unwrap().modified().unwrap();
        for corruption in [
            "checksum",
            "version",
            "language",
            "source",
            "binding",
            "lengths",
            "offset",
            "thread",
            "truncated",
        ] {
            match corruption {
                "checksum" => {
                    let mut bytes = fs::read(directory.join(INDEX_FILE)).unwrap();
                    *bytes.last_mut().unwrap() ^= 1;
                    fs::write(directory.join(INDEX_FILE), bytes).unwrap();
                }
                "version" => rewrite_metadata(&directory, |record| record.version += 1),
                "language" => rewrite_metadata(&directory, |record| record.language = "unsupported-locale".into()),
                "source" => rewrite_metadata(&directory, |record| record.source.size += 1),
                "binding" => rewrite_metadata(&directory, |record| record.messages_crc32 ^= 1),
                "lengths" | "offset" | "thread" => rewrite_metadata(&directory, |record| {
                    let mut index: MetadataIndex = bincode::DefaultOptions::new()
                        .with_fixint_encoding()
                        .deserialize(&record.index.chunks[0])
                        .unwrap();
                    match corruption {
                        "lengths" => index.infos.pop().map(|_| ()).unwrap(),
                        "offset" => index.descriptors[0].offset = record.messages_size + 128,
                        "thread" => index.threads[1].index = index.threads[0].index,
                        _ => unreachable!(),
                    }
                    record.index.chunks[0] = bincode::DefaultOptions::new().with_fixint_encoding().serialize(&index).unwrap();
                }),
                "truncated" => fs::write(directory.join(INDEX_FILE), b"partial").unwrap(),
                _ => unreachable!(),
            }
            let extracted = cache.load(&path, || panic!("bad metadata must not discard valid extracted data")).unwrap();
            assert!(extracted.metadata.is_none(), "{corruption}");
            let rebuilt = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
            assert_eq!(rebuilt.infos, original.infos);
            assert_eq!(rebuilt.descriptors, original.descriptors);
            assert_eq!(rebuilt.cached_threads(), original.cached_threads());
            assert_eq!(fs::metadata(directory.join("1.dat")).unwrap().modified().unwrap(), raw_modified);
            assert!(cache.load(&path, || panic!("repaired cache must be reusable")).unwrap().metadata.is_some());
        }
    }

    #[test]
    fn metadata_preserves_header_styles_dates_and_chunk_order() {
        let (_dir, path, cache) = fixture();
        let packet = QwkPackage::extract_packet(&path).unwrap();
        let source = SourceStamp::read(&path).unwrap();
        let identity = cache.store(&entry(&cache, &path), &source, &packet).unwrap();
        let mut package = QwkPackage::from_extracted(&path, packet).unwrap();
        package.infos[0].from = b"\x1b[1;31mAlice".as_slice().into();
        package.infos[0].subject = b"\x1b[32m\xb0 News\x1b[0m".as_slice().into();
        package.thread_rows = Some(std::sync::Arc::new(crate::threading::build_threads(&package.infos.iter().collect::<Vec<_>>())));
        cache.store_index(&identity, &package).unwrap();
        let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        assert_eq!(warm.infos, package.infos);
        assert_eq!(warm.descriptors, package.descriptors);
        assert_eq!(warm.cached_threads(), package.cached_threads());

        let count = INDEX_CHUNK_MESSAGES + 5;
        let mut many = package.clone();
        many.infos = (0..count)
            .map(|index| {
                let mut info = package.infos[index % package.infos.len()].clone();
                info.index = index;
                info
            })
            .collect();
        many.descriptors = (0..count).map(|index| package.descriptors[index % package.descriptors.len()].clone()).collect();
        many.thread_rows = Some(std::sync::Arc::new(crate::threading::build_threads(&many.infos.iter().collect::<Vec<_>>())));
        cache.store_index(&identity, &many).unwrap();
        let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        assert_eq!(warm.infos, many.infos);
        assert_eq!(warm.descriptors, many.descriptors);
        assert_eq!(warm.cached_threads(), many.cached_threads());
    }

    #[test]
    fn changed_source_invalidates_cache_even_when_size_is_unchanged() {
        let (_dir, path, cache) = fixture();
        QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let previous = entry(&cache, &path);
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified + Duration::from_secs(2))
            .unwrap();
        let mut extracted = false;
        cache
            .load(&path, || {
                extracted = true;
                QwkPackage::extract_packet(&path)
            })
            .unwrap();
        assert!(extracted);
        assert_ne!(entry(&cache, &path), previous);
        assert_eq!(fs::read_dir(&cache.directory).unwrap().count(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn replacing_source_invalidates_cache_when_modification_time_is_preserved() {
        let (dir, path, cache) = fixture();
        QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let previous = SourceStamp::read(&path).unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let replacement = dir.path().join("replacement");
        fs::copy(&path, &replacement).unwrap();
        File::options().write(true).open(&replacement).unwrap().set_modified(modified).unwrap();
        fs::rename(replacement, &path).unwrap();
        let changed = SourceStamp::read(&path).unwrap();
        assert_eq!(previous.size, changed.size);
        assert_eq!(previous.modified_seconds, changed.modified_seconds);
        assert_eq!(previous.modified_nanos, changed.modified_nanos);
        let mut extracted = false;
        cache
            .load(&path, || {
                extracted = true;
                QwkPackage::extract_packet(&path)
            })
            .unwrap();
        assert!(extracted);
    }

    #[test]
    fn missing_source_does_not_return_stale_mail() {
        let (_dir, path, cache) = fixture();
        QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(QwkPackage::load_from_file_cached(&path, &cache).is_err());
    }

    #[test]
    fn corrupted_or_incomplete_cache_is_reextracted() {
        let (_dir, path, cache) = fixture();
        QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let directory = entry(&cache, &path);
        for corruption in ["checksum", "truncated", "manifest", "missing"] {
            match corruption {
                "checksum" => {
                    let file = directory.join("1.dat");
                    let mut data = fs::read(&file).unwrap();
                    data[128 + 71] ^= 1;
                    fs::write(file, data).unwrap();
                }
                "truncated" => fs::write(directory.join("1.dat"), b"partial").unwrap(),
                "manifest" => fs::write(directory.join(MANIFEST), b"not a manifest").unwrap(),
                "missing" => fs::remove_file(directory.join("1.dat")).unwrap(),
                _ => unreachable!(),
            }
            let mut extracted = false;
            let packet = cache
                .load(&path, || {
                    extracted = true;
                    QwkPackage::extract_packet(&path)
                })
                .unwrap();
            if corruption == "checksum" {
                assert!(!extracted, "body checksums are checked on demand");
                let package = QwkPackage::from_extracted(&path, packet).unwrap();
                assert_eq!(
                    package.read_message(0).unwrap().text,
                    QwkPackage::load_from_file(&path).unwrap().read_message(0).unwrap().text
                );
                assert!(!directory.join(MANIFEST).exists(), "corrupt extraction must be invalidated");
                QwkPackage::load_from_file_cached(&path, &cache).unwrap();
            } else {
                assert!(extracted, "{corruption}");
                // cache.load alone does not build the metadata index.
                QwkPackage::load_from_file_cached(&path, &cache).unwrap();
            }
            cache.load(&path, || panic!("repaired cache must be reusable")).unwrap();
        }
        assert_eq!(fs::read_dir(&cache.directory).unwrap().count(), 1);
    }

    #[test]
    fn expiry_uses_last_access_and_respects_exact_retention_boundary() {
        let (_dir, path, cache) = fixture();
        QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let directory = entry(&cache, &path);
        let now = SystemTime::now();
        let manifest = File::options().write(true).open(directory.join(MANIFEST)).unwrap();
        manifest.set_modified(now - Duration::from_secs(29 * DAY)).unwrap();
        cache.cleanup(now).unwrap();
        assert!(directory.exists());
        cache.load(&path, || panic!("recent cache should still be valid")).unwrap();
        cache.cleanup(now + Duration::from_secs(2 * DAY)).unwrap();
        assert!(directory.exists(), "a hit refreshes the idle age");
        manifest.set_modified(now - Duration::from_secs(30 * DAY)).unwrap();
        cache.cleanup(now).unwrap();
        assert!(!directory.exists(), "the exact retention boundary expires");
    }

    #[test]
    fn lowering_retention_removes_entries_on_the_next_open() {
        let (_dir, path, cache) = fixture();
        QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let directory = entry(&cache, &path);
        let previous = SystemTime::now() - Duration::from_secs(15 * DAY);
        File::options()
            .write(true)
            .open(directory.join(MANIFEST))
            .unwrap()
            .set_modified(previous)
            .unwrap();
        let shorter = ExtractionCache::new(cache.directory.clone(), 14);
        let mut extracted = false;
        shorter
            .load(&path, || {
                extracted = true;
                QwkPackage::extract_packet(&path)
            })
            .unwrap();
        assert!(extracted, "the shorter retention is applied before a cache hit");
        cache.load(&path, || panic!("the expired entry must have been replaced")).unwrap();
    }

    #[test]
    fn disabling_cache_removes_only_owned_entries_and_does_not_write_new_ones() {
        let (_dir, path, cache) = fixture();
        QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let directory = entry(&cache, &path);
        let unrelated = cache.directory.join("unrelated");
        fs::create_dir(&unrelated).unwrap();
        fs::write(unrelated.join("keep"), b"untouched").unwrap();
        let disabled = ExtractionCache::new(cache.directory.clone(), 0);
        let package = QwkPackage::load_from_file_cached(&path, &disabled).unwrap();
        assert_eq!(package.infos.len(), 4);
        assert!(!directory.exists());
        assert_eq!(fs::read(unrelated.join("keep")).unwrap(), b"untouched");
        assert_eq!(fs::read_dir(&cache.directory).unwrap().count(), 1);
    }

    #[test]
    fn unwritable_cache_does_not_prevent_opening_the_packet() {
        let (dir, path, _cache) = fixture();
        let blocked = dir.path().join("not-a-directory");
        fs::write(&blocked, b"untouched").unwrap();
        let cache = ExtractionCache::new(blocked.clone(), 30);
        let package = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        assert_eq!(package.infos.len(), 4);
        assert_eq!(fs::read(blocked).unwrap(), b"untouched");
    }

    #[test]
    fn cache_never_uses_archive_names_as_paths() {
        let (dir, path, cache) = fixture();
        let source = SourceStamp::read(&path).unwrap();
        let mut packet = QwkPackage::extract_packet(&path).unwrap();
        packet.others.push(("../outside".into(), b"still metadata".to_vec()));
        cache.store(&entry(&cache, &path), &source, &packet).unwrap();
        let cached = cache.load(&path, || panic!("expected a cache hit")).unwrap();
        assert_eq!(cached.others.last().unwrap().0, "../outside");
        assert!(!dir.path().join("outside").exists());
        assert!(!cache.directory.join("outside").exists());
    }

    #[test]
    fn expired_unfinished_writes_are_removed_but_unknown_directories_are_preserved() {
        let (_dir, path, cache) = fixture();
        QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let pending = cache.directory.join(format!("{PENDING_PREFIX}1234-0"));
        let unknown = cache.directory.join(format!("{PENDING_PREFIX}user-notes"));
        fs::create_dir(&pending).unwrap();
        fs::create_dir(&unknown).unwrap();
        fs::write(pending.join("partial"), b"partial extraction").unwrap();
        cache.cleanup(SystemTime::now() + Duration::from_secs(31 * DAY)).unwrap();
        assert!(!pending.exists());
        assert!(unknown.exists());
    }

    #[test]
    fn non_zip_packets_are_cached_too() {
        let dir = TempDir::new();
        let cache = ExtractionCache::new(dir.path().join("cache"), 30);
        for name in ["TEST_ARJ.QWK", "TEST_7Z.QWK"] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/qwk/test_data").join(name);
            let cold = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
            let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
            assert_eq!(cold.infos.len(), warm.infos.len());
            assert_eq!(cold.read_message(0).unwrap().text, warm.read_message(0).unwrap().text);
            cache.load(&path, || panic!("non-ZIP packet should also skip extraction")).unwrap();
        }
    }

    #[test]
    fn concurrent_loaders_publish_complete_entries() {
        let (_dir, path, cache) = fixture();
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let path = &path;
                let cache = &cache;
                scope.spawn(move || {
                    let package = QwkPackage::load_from_file_cached(path, cache).unwrap();
                    assert_eq!(package.infos.len(), 4);
                });
            }
        });
        cache.load(&path, || panic!("one complete cache must have been published")).unwrap();
        assert_eq!(fs::read_dir(&cache.directory).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn cache_mail_is_private_and_symlinks_are_not_followed() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let (dir, path, cache) = fixture();
        QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let directory = entry(&cache, &path);
        assert_eq!(fs::metadata(&directory).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(fs::metadata(directory.join("1.dat")).unwrap().permissions().mode() & 0o777, 0o600);
        let outside = dir.path().join("outside");
        fs::write(&outside, b"private outside").unwrap();
        fs::remove_file(directory.join("1.dat")).unwrap();
        symlink(&outside, directory.join("1.dat")).unwrap();
        QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        assert_eq!(fs::read(&outside).unwrap(), b"private outside");
        assert!(!fs::symlink_metadata(directory.join("1.dat")).unwrap().file_type().is_symlink());
    }
}
