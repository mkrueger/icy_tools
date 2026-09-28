//! Crash recovery for unsaved documents.
//!
//! Every running editor owns one entry in the recovery directory, identified by a unique
//! instance id and consisting of:
//! - `<id>.lock`: held with an exclusive OS file lock for the lifetime of the process. The OS
//!   releases it when the process ends in any way, including crashes and power loss.
//! - `<id>.recovery`: the latest snapshot of the unsaved document, replaced atomically
//!   (write temporary file, sync, rename) so a crash while writing keeps the previous snapshot.
//!
//! A recovery file whose lock can be acquired belongs to an editor that is no longer running
//! and is offered for recovery. Snapshots are never deleted without an explicit user action,
//! a successful save, or the document becoming unmodified.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread::JoinHandle,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

const MAGIC: &[u8; 16] = b"ICYDRAW-RECOVERY";
const FORMAT_VERSION: u32 = 1;
const DATA_EXTENSION: &str = "recovery";
const LOCK_EXTENSION: &str = "lock";
const TEMP_EXTENSION: &str = "tmp";
/// Headers are small; anything larger indicates a damaged file.
const MAX_HEADER_LEN: usize = 1 << 20;

/// The editor the snapshot belongs to, which determines how the payload is read back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryKind {
    /// An ANSI document; the payload is an `.icy` file.
    Ansi,
    /// A TheDraw font collection; the payload is a `.tdf` file.
    CharFont,
    /// A bitmap font; the payload is a PSF2 font.
    BitFont,
    /// A Lua animation; the payload is the UTF-8 script.
    Animation,
}

/// Size and checksum of the file on disk the document was loaded from or last saved to.
///
/// Recovery uses it to tell whether that file changed while the editor was not running, so a
/// restored document never silently overwrites newer content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub len: u64,
    pub crc: u32,
}

impl Fingerprint {
    pub fn of(bytes: &[u8]) -> Self {
        Self {
            len: bytes.len() as u64,
            crc: crc32fast::hash(bytes),
        }
    }

    /// Reads `path` and returns its content if it still matches this fingerprint.
    pub fn read_matching(&self, path: &Path) -> Option<Vec<u8>> {
        let bytes = fs::read(path).ok()?;
        (Self::of(&bytes) == *self).then_some(bytes)
    }
}

/// An unsaved document to store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub kind: RecoveryKind,
    /// The file the document belongs to, `None` for untitled documents.
    pub path: Option<PathBuf>,
    /// The content of `path` the edits are based on.
    pub disk: Option<Fingerprint>,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Header {
    pub version: u32,
    pub kind: RecoveryKind,
    pub path: Option<PathBuf>,
    pub disk: Option<Fingerprint>,
    /// Seconds since the Unix epoch.
    pub saved_at: u64,
    pub app_version: String,
    pub payload_len: u64,
    pub payload_crc: u32,
}

impl Header {
    /// File name shown to the user.
    pub fn title(&self) -> Option<String> {
        self.path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
    }
}

fn encode(snapshot: &Snapshot) -> io::Result<Vec<u8>> {
    let header = Header {
        version: FORMAT_VERSION,
        kind: snapshot.kind,
        path: snapshot.path.clone(),
        disk: snapshot.disk,
        saved_at: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |time| time.as_secs()),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        payload_len: snapshot.payload.len() as u64,
        payload_crc: crc32fast::hash(&snapshot.payload),
    };
    let header = serde_json::to_vec(&header).map_err(io::Error::other)?;
    let mut bytes = Vec::with_capacity(MAGIC.len() + 4 + header.len() + snapshot.payload.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(header.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(&snapshot.payload);
    Ok(bytes)
}

fn damaged(reason: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("damaged recovery file: {reason}"))
}

fn decode_header(bytes: &[u8]) -> io::Result<(Header, usize)> {
    let rest = bytes.strip_prefix(MAGIC.as_slice()).ok_or_else(|| damaged("unknown format"))?;
    let (length, rest) = rest.split_first_chunk::<4>().ok_or_else(|| damaged("truncated"))?;
    let length = u32::from_le_bytes(*length) as usize;
    if length > MAX_HEADER_LEN || length > rest.len() {
        return Err(damaged("truncated header"));
    }
    let header: Header = serde_json::from_slice(&rest[..length]).map_err(|error| damaged(&error.to_string()))?;
    if header.version > FORMAT_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("recovery file was written by the newer Icy Draw {}", header.app_version),
        ));
    }
    Ok((header, MAGIC.len() + 4 + length))
}

fn decode(bytes: &[u8]) -> io::Result<(Header, Snapshot)> {
    let (header, start) = decode_header(bytes)?;
    let payload = &bytes[start..];
    if payload.len() as u64 != header.payload_len || crc32fast::hash(payload) != header.payload_crc {
        return Err(damaged("checksum mismatch"));
    }
    let snapshot = Snapshot {
        kind: header.kind,
        path: header.path.clone(),
        disk: header.disk,
        payload: payload.to_vec(),
    };
    Ok((header, snapshot))
}

fn data_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.{DATA_EXTENSION}"))
}

fn lock_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.{LOCK_EXTENSION}"))
}

fn temp_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.{TEMP_EXTENSION}"))
}

fn open_lock(dir: &Path, id: &str) -> io::Result<File> {
    OpenOptions::new().read(true).write(true).create(true).truncate(false).open(lock_path(dir, id))
}

/// Locks `<id>.lock` if no running editor holds it.
fn try_claim(dir: &Path, id: &str) -> io::Result<Option<File>> {
    let file = open_lock(dir, id)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(fs::TryLockError::WouldBlock) => Ok(None),
        Err(fs::TryLockError::Error(error)) => Err(error),
    }
}

fn sync_dir(dir: &Path) {
    // Makes the rename durable. Directories cannot be opened as files on Windows, where
    // `MoveFileEx` is used by `rename` and the metadata update is journaled by NTFS.
    #[cfg(unix)]
    if let Ok(dir) = File::open(dir) {
        let _ = dir.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

fn write_atomic(dir: &Path, id: &str, bytes: &[u8]) -> io::Result<()> {
    let temporary = temp_path(dir, id);
    let result = (|| {
        let mut file = File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, data_path(dir, id))?;
        sync_dir(dir);
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn remove(dir: &Path, id: &str) -> io::Result<()> {
    match fs::remove_file(data_path(dir, id)) {
        Ok(()) => {
            sync_dir(dir);
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

enum Job {
    Write(Vec<u8>),
    Remove,
    Flush(Sender<()>),
}

/// Result of the most recent write or removal on the writer thread.
#[derive(Default)]
struct Status {
    error: Option<String>,
    /// Increased with every finished job, so the editor can tell a stale error from a new one.
    completed: u64,
}

/// The recovery entry of this editor instance.
pub struct Recovery {
    dir: PathBuf,
    id: String,
    _lock: File,
    jobs: Option<Sender<Job>>,
    writer: Option<JoinHandle<()>>,
    status: Arc<Mutex<Status>>,
    /// Checksum of the last snapshot handed to the writer; `None` when no snapshot is stored.
    stored: Option<(u32, u64)>,
    reported: u64,
}

impl Recovery {
    /// Creates the recovery directory and takes ownership of a new entry.
    pub fn start(dir: &Path) -> io::Result<Self> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |time| time.as_nanos());
        Self::start_with_id(dir, format!("{}-{nanos:x}", std::process::id()))
    }

    fn start_with_id(dir: &Path, id: String) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let lock = open_lock(dir, &id)?;
        lock.lock()?;
        let status = Arc::new(Mutex::new(Status::default()));
        let (jobs, receiver) = mpsc::channel();
        let writer = {
            let dir = dir.to_path_buf();
            let id = id.clone();
            let status = status.clone();
            std::thread::Builder::new()
                .name("icy_draw recovery".into())
                .spawn(move || write_jobs(&dir, &id, &receiver, &status))?
        };
        Ok(Self {
            dir: dir.to_path_buf(),
            id,
            _lock: lock,
            jobs: Some(jobs),
            writer: Some(writer),
            status,
            stored: None,
            reported: 0,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn send(&self, job: Job) {
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(job);
        }
    }

    /// Stores `snapshot` in the background unless it equals the stored one.
    pub fn store(&mut self, snapshot: &Snapshot) -> io::Result<()> {
        let bytes = encode(snapshot)?;
        // The header contains the time, so compare everything else.
        let digest = {
            let mut hasher = crc32fast::Hasher::new();
            hasher.update(&snapshot.payload);
            hasher.update(format!("{:?}{:?}{:?}", snapshot.kind, snapshot.path, snapshot.disk).as_bytes());
            (hasher.finalize(), snapshot.payload.len() as u64)
        };
        if self.stored != Some(digest) {
            self.stored = Some(digest);
            self.send(Job::Write(bytes));
        }
        Ok(())
    }

    /// Removes the stored snapshot, e.g. after saving or discarding the changes.
    pub fn clear(&mut self) {
        if self.stored.take().is_some() {
            self.send(Job::Remove);
        }
    }

    /// Whether a snapshot is stored or being written.
    pub fn has_snapshot(&self) -> bool {
        self.stored.is_some()
    }

    /// Waits until all queued writes and removals are on disk.
    pub fn flush(&self) {
        let (sender, receiver) = mpsc::channel();
        self.send(Job::Flush(sender));
        let _ = receiver.recv();
    }

    /// Returns the result of the writes and removals finished since the last call, once.
    /// A failed snapshot is written again by the next `store`.
    pub fn take_status(&mut self) -> Option<Result<(), String>> {
        let status = self.status.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if status.completed == self.reported {
            return None;
        }
        self.reported = status.completed;
        let error = status.error.clone();
        drop(status);
        match error {
            Some(error) => {
                self.stored = None;
                Some(Err(error))
            }
            None => Some(Ok(())),
        }
    }

    /// Snapshots left behind by editors that are no longer running.
    pub fn orphans(&self) -> Vec<Orphan> {
        orphans(&self.dir, Some(&self.id))
    }
}

impl Drop for Recovery {
    fn drop(&mut self) {
        // Finishes pending writes; the snapshot itself stays until it is explicitly cleared.
        self.jobs = None;
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}

fn write_jobs(dir: &Path, id: &str, receiver: &Receiver<Job>, status: &Mutex<Status>) {
    while let Ok(job) = receiver.recv() {
        // Only the newest write or removal matters; flushes are answered after it is done.
        let mut latest = None;
        let mut flushes = Vec::new();
        for job in std::iter::once(job).chain(receiver.try_iter()) {
            match job {
                Job::Flush(sender) => flushes.push(sender),
                job => latest = Some(job),
            }
        }
        if let Some(job) = latest {
            let result = match job {
                Job::Write(bytes) => write_atomic(dir, id, &bytes),
                Job::Remove => remove(dir, id),
                Job::Flush(_) => unreachable!(),
            };
            if let Err(error) = &result {
                log::error!("autosave in {} failed: {error}", dir.display());
            }
            let mut status = status.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            status.error = result.err().map(|error| error.to_string());
            status.completed += 1;
        }
        for sender in flushes {
            let _ = sender.send(());
        }
    }
}

/// A snapshot of an editor that is no longer running, locked so that no other editor offers
/// it at the same time. Dropping it releases the lock and keeps the snapshot.
pub struct Orphan {
    dir: PathBuf,
    id: String,
    lock: Option<File>,
    /// The header, or why the file cannot be read.
    pub header: Result<Header, String>,
    /// Modification time of the recovery file.
    pub modified: Option<SystemTime>,
}

impl Orphan {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn load(&self) -> Result<Snapshot, String> {
        let bytes = fs::read(data_path(&self.dir, &self.id)).map_err(|error| error.to_string())?;
        decode(&bytes).map(|(_, snapshot)| snapshot).map_err(|error| error.to_string())
    }

    /// Deletes the snapshot. Call only after the user chose to discard it or its content is
    /// stored elsewhere, e.g. in the recovery entry of the editor that restored it.
    pub fn discard(mut self) -> io::Result<()> {
        remove(&self.dir, &self.id)?;
        let _ = fs::remove_file(temp_path(&self.dir, &self.id));
        drop(self.lock.take());
        let _ = fs::remove_file(lock_path(&self.dir, &self.id));
        Ok(())
    }
}

/// Finds and locks the snapshots in `dir` whose editors are no longer running, newest first.
/// Leftover lock and temporary files of those editors are removed.
pub fn orphans(dir: &Path, own: Option<&str>) -> Vec<Orphan> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut ids: Vec<(String, bool)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let (Some(stem), Some(extension)) = (path.file_stem().and_then(|stem| stem.to_str()), path.extension().and_then(|e| e.to_str())) else {
            continue;
        };
        if Some(stem) == own || ![DATA_EXTENSION, LOCK_EXTENSION, TEMP_EXTENSION].contains(&extension) {
            continue;
        }
        match ids.iter_mut().find(|(id, _)| id == stem) {
            Some((_, data)) => *data |= extension == DATA_EXTENSION,
            None => ids.push((stem.to_owned(), extension == DATA_EXTENSION)),
        }
    }
    let mut found = Vec::new();
    for (id, has_data) in ids {
        let lock = match try_claim(dir, &id) {
            Ok(Some(lock)) => lock,
            Ok(None) => continue,
            Err(error) => {
                log::warn!("cannot check recovery entry {id}: {error}");
                continue;
            }
        };
        // A temporary file is an unfinished write; the previous snapshot (if any) is intact.
        let _ = fs::remove_file(temp_path(dir, &id));
        if !has_data {
            drop(lock);
            let _ = fs::remove_file(lock_path(dir, &id));
            continue;
        }
        let path = data_path(dir, &id);
        let header = fs::read(&path)
            .and_then(|bytes| decode_header(&bytes).map(|(header, _)| header))
            .map_err(|error| error.to_string());
        let modified = fs::metadata(&path).and_then(|metadata| metadata.modified()).ok();
        found.push(Orphan {
            dir: dir.to_path_buf(),
            id,
            lock: Some(lock),
            header,
            modified,
        });
    }
    found.sort_by(|a, b| b.modified.cmp(&a.modified));
    found
}

/// Locks and returns the snapshot `id`, e.g. to restore it in a new editor window.
pub fn claim(dir: &Path, id: &str) -> Result<Orphan, String> {
    orphans(dir, None)
        .into_iter()
        .find(|orphan| orphan.id == id)
        .ok_or_else(|| format!("The recovered document {id} is no longer available."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(payload: &[u8]) -> Snapshot {
        Snapshot {
            kind: RecoveryKind::Ansi,
            path: Some(PathBuf::from("/art/logo.ans")),
            disk: Some(Fingerprint::of(b"original")),
            payload: payload.to_vec(),
        }
    }

    #[test]
    fn stored_snapshots_are_offered_after_the_owner_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let mut recovery = Recovery::start(dir.path()).unwrap();
        recovery.store(&snapshot(b"first")).unwrap();
        recovery.store(&snapshot(b"second")).unwrap();
        recovery.flush();
        assert_eq!(recovery.take_status(), Some(Ok(())));
        assert_eq!(recovery.take_status(), None);
        assert!(orphans(dir.path(), None).is_empty(), "a running editor's snapshot is locked");
        let id = recovery.id().to_owned();
        drop(recovery);

        let found = orphans(dir.path(), None);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id(), id);
        let header = found[0].header.as_ref().unwrap();
        assert_eq!(header.title().as_deref(), Some("logo.ans"));
        assert_eq!(found[0].load().unwrap(), snapshot(b"second"));
        assert!(orphans(dir.path(), None).is_empty(), "an offered snapshot is not offered twice");
        drop(found);
        let found = orphans(dir.path(), None);
        assert_eq!(found.len(), 1, "releasing an orphan keeps it for later");
        found.into_iter().next().unwrap().discard().unwrap();
        assert!(orphans(dir.path(), None).is_empty());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0, "discarding removes all files");
    }

    #[test]
    fn clearing_removes_the_snapshot_and_a_later_change_writes_it_again() {
        let dir = tempfile::tempdir().unwrap();
        let mut recovery = Recovery::start(dir.path()).unwrap();
        let data = data_path(dir.path(), recovery.id());
        recovery.store(&snapshot(b"a")).unwrap();
        recovery.flush();
        assert!(data.exists());
        recovery.clear();
        recovery.flush();
        assert!(!data.exists());
        recovery.store(&snapshot(b"a")).unwrap();
        recovery.flush();
        assert!(data.exists(), "the same content is written again after clearing");
    }

    #[test]
    fn unfinished_writes_keep_the_previous_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let mut recovery = Recovery::start(dir.path()).unwrap();
        recovery.store(&snapshot(b"complete")).unwrap();
        recovery.flush();
        let id = recovery.id().to_owned();
        drop(recovery);
        // A crash while writing the next snapshot leaves a partial temporary file.
        fs::write(temp_path(dir.path(), &id), b"ICYDRAW-RECO").unwrap();
        let found = orphans(dir.path(), None);
        assert_eq!(found[0].load().unwrap().payload, b"complete");
        assert!(!temp_path(dir.path(), &id).exists());
    }

    #[test]
    fn damaged_files_are_reported_instead_of_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let mut recovery = Recovery::start(dir.path()).unwrap();
        recovery.store(&snapshot(b"payload")).unwrap();
        recovery.flush();
        let path = data_path(dir.path(), recovery.id());
        drop(recovery);
        let mut bytes = fs::read(&path).unwrap();
        *bytes.last_mut().unwrap() ^= 0xFF;
        fs::write(&path, bytes).unwrap();
        let found = orphans(dir.path(), None);
        assert_eq!(found.len(), 1);
        assert!(found[0].header.is_ok());
        assert!(found[0].load().unwrap_err().contains("checksum"));

        fs::write(&path, b"garbage").unwrap();
        drop(found);
        let found = orphans(dir.path(), None);
        assert!(found[0].header.as_ref().unwrap_err().contains("damaged"));
        assert!(path.exists());
    }

    #[test]
    fn stale_lock_files_are_cleaned_up_and_claims_are_exclusive() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(lock_path(dir.path(), "old"), b"").unwrap();
        let mut recovery = Recovery::start_with_id(dir.path(), "old-editor".into()).unwrap();
        recovery.store(&snapshot(b"x")).unwrap();
        recovery.flush();
        drop(recovery);
        assert!(orphans(dir.path(), None).len() == 1);
        assert!(!lock_path(dir.path(), "old").exists());
        let claimed = claim(dir.path(), "old-editor").unwrap();
        assert!(claim(dir.path(), "old-editor").is_err(), "a claimed snapshot is locked");
        assert_eq!(claimed.load().unwrap().payload, b"x");
    }

    #[test]
    fn write_errors_are_reported_once_and_retried() {
        let dir = tempfile::tempdir().unwrap();
        let mut recovery = Recovery::start(dir.path()).unwrap();
        // A directory in place of the target makes the rename fail.
        fs::create_dir(data_path(dir.path(), recovery.id())).unwrap();
        fs::write(data_path(dir.path(), recovery.id()).join("blocker"), b"").unwrap();
        recovery.store(&snapshot(b"x")).unwrap();
        recovery.flush();
        assert!(recovery.take_status().unwrap().is_err());
        assert_eq!(recovery.take_status(), None);
        assert!(!recovery.has_snapshot(), "a failed snapshot is written again by the next store");
        fs::remove_dir_all(data_path(dir.path(), recovery.id())).unwrap();
        recovery.store(&snapshot(b"x")).unwrap();
        recovery.flush();
        assert_eq!(recovery.take_status(), Some(Ok(())));
        assert!(data_path(dir.path(), recovery.id()).is_file());
    }
}
