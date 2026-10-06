use std::{
    fs::File,
    io,
    sync::{Arc, Mutex, OnceLock},
};

use crate::Res;
#[cfg(not(unix))]
use std::io::{Read, Seek, SeekFrom};

pub(super) const CHUNK_SIZE: usize = 1024 * 1024;

pub(super) enum MessageData {
    Memory(Vec<u8>),
    File(FileBodies),
}

pub(super) struct FileBodies {
    file: Mutex<File>,
    size: usize,
    checksums: Vec<u32>,
    chunks: Vec<OnceLock<Result<Arc<Vec<u8>>, String>>>,
    recovered: OnceLock<Arc<Vec<u8>>>,
    recovery: Mutex<Option<String>>,
    recover: Box<dyn Fn() -> Res<Vec<u8>> + Send + Sync>,
}

impl MessageData {
    pub fn len(&self) -> u64 {
        match self {
            Self::Memory(bytes) => bytes.len() as u64,
            Self::File(bodies) => bodies.size as u64,
        }
    }

    pub fn file(file: File, size: u64, crc32: u32, checksums: Vec<u32>, recover: impl Fn() -> Res<Vec<u8>> + Send + Sync + 'static) -> Res<Self> {
        let size = usize::try_from(size)?;
        if file.metadata()?.len() != size as u64 || checksums.len() != size.div_ceil(CHUNK_SIZE) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "cached message size or chunk count does not match").into());
        }
        // Bind individual chunk hashes to the metadata's whole-file CRC without reading the body.
        let mut combined = crc32fast::Hasher::new();
        for (index, checksum) in checksums.iter().enumerate() {
            let len = CHUNK_SIZE.min(size - index * CHUNK_SIZE);
            combined.combine(&crc32fast::Hasher::new_with_initial_len(*checksum, len as u64));
        }
        if combined.finalize() != crc32 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "cached chunk checksums do not match the message metadata").into());
        }
        Ok(Self::File(FileBodies {
            file: Mutex::new(file),
            size,
            chunks: (0..checksums.len()).map(|_| OnceLock::new()).collect(),
            checksums,
            recovered: OnceLock::new(),
            recovery: Mutex::new(None),
            recover: Box::new(recover),
        }))
    }

    pub fn read_range(&self, offset: u64, len: u64) -> Res<Vec<u8>> {
        let start = usize::try_from(offset)?;
        let end = start.checked_add(usize::try_from(len)?).ok_or_else(|| invalid_range())?;
        match self {
            Self::Memory(bytes) => Ok(bytes.get(start..end).ok_or_else(|| invalid_range())?.to_vec()),
            Self::File(bodies) => {
                if start > end || end > bodies.size {
                    return Err(invalid_range().into());
                }
                if let Some(bytes) = bodies.recovered.get() {
                    return Ok(bytes[start..end].to_vec());
                }
                let mut bytes = Vec::with_capacity(end - start);
                if start != end {
                    for chunk in start / CHUNK_SIZE..=(end - 1) / CHUNK_SIZE {
                        let data = bodies.chunk(chunk)?;
                        let base = chunk * CHUNK_SIZE;
                        bytes.extend_from_slice(&data[start.saturating_sub(base)..(end - base).min(data.len())]);
                    }
                }
                Ok(bytes)
            }
        }
    }

    pub fn needs_preload(&self) -> bool {
        match self {
            Self::Memory(_) => false,
            Self::File(bodies) => bodies.recovered.get().is_none() && bodies.chunks.iter().any(|chunk| !matches!(chunk.get(), Some(Ok(_)))),
        }
    }

    pub fn preload(&self, cancelled: impl Fn() -> bool) -> Res<bool> {
        if let Self::File(bodies) = self {
            for index in 0..bodies.chunks.len() {
                if cancelled() {
                    return Ok(false);
                }
                if bodies.recovered.get().is_some() {
                    break;
                }
                bodies.chunk(index)?;
                // A verified chunk is useful even if a later selection cancels preloading.
                std::thread::yield_now();
            }
        }
        Ok(!cancelled())
    }
}

impl FileBodies {
    fn chunk(&self, index: usize) -> Res<Arc<Vec<u8>>> {
        let start = index * CHUNK_SIZE;
        let end = (start + CHUNK_SIZE).min(self.size);
        if let Some(bytes) = self.recovered.get() {
            return Ok(Arc::new(bytes[start..end].to_vec()));
        }
        match self.chunks[index].get_or_init(|| {
            let mut bytes = vec![0; end - start];
            let result = (|| -> io::Result<()> {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::FileExt;
                    // Independent positions allow selected-message reads alongside preloading/search.
                    let file = self.file.lock().unwrap().try_clone()?;
                    file.read_exact_at(&mut bytes, start as u64)?;
                }
                #[cfg(not(unix))]
                {
                    let mut file = self.file.lock().unwrap();
                    file.seek(SeekFrom::Start(start as u64))?;
                    file.read_exact(&mut bytes)?;
                }
                if crc32fast::hash(&bytes) != self.checksums[index] {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("cached message chunk {index} checksum does not match"),
                    ));
                }
                Ok(())
            })();
            result.map(|()| Arc::new(bytes)).map_err(|error| error.to_string())
        }) {
            Ok(bytes) => Ok(bytes.clone()),
            Err(error) => {
                let mut failure = self.recovery.lock().unwrap();
                if self.recovered.get().is_none() && failure.is_none() {
                    log::warn!("unable to read cached message data: {error}; recovering from the packet");
                    match (self.recover)() {
                        Ok(bytes) if bytes.len() == self.size => {
                            let _ = self.recovered.set(Arc::new(bytes));
                        }
                        Ok(_) => *failure = Some("recovered message data has an unexpected length".into()),
                        Err(error) => *failure = Some(error.to_string()),
                    }
                }
                if let Some(bytes) = self.recovered.get() {
                    Ok(Arc::new(bytes[start..end].to_vec()))
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "unable to recover cached message data: {}; reopen the packet",
                            failure.as_deref().unwrap_or(error)
                        ),
                    )
                    .into())
                }
            }
        }
    }
}

fn invalid_range() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "message range lies outside the packet")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn fixture() -> (super::super::tests::TempDir, MessageData, Vec<u8>) {
        let dir = super::super::tests::TempDir::new();
        let bytes: Vec<_> = (0..CHUNK_SIZE * 3 + 128).map(|index| (index % 251) as u8).collect();
        let path = dir.path().join("bodies.dat");
        std::fs::write(&path, &bytes).unwrap();
        let checksums = bytes.chunks(CHUNK_SIZE).map(crc32fast::hash).collect();
        let data = MessageData::file(File::open(&path).unwrap(), bytes.len() as u64, crc32fast::hash(&bytes), checksums, || {
            Err("unexpected recovery".into())
        })
        .unwrap();
        (dir, data, bytes)
    }

    #[test]
    fn range_crosses_chunks_without_loading_unrelated_bytes() {
        let (_dir, data, bytes) = fixture();
        assert!(data.needs_preload());
        let start = CHUNK_SIZE - 128;
        assert_eq!(data.read_range(start as u64, 512).unwrap(), bytes[start..start + 512]);
        let MessageData::File(bodies) = &data else { panic!("file expected") };
        assert!(bodies.chunks[0].get().is_some());
        assert!(bodies.chunks[1].get().is_some());
        assert!(bodies.chunks[2].get().is_none());
        assert!(data.read_range(bytes.len() as u64, 1).is_err());
        assert!(data.read_range(u64::MAX, 2).is_err());
        assert!(data.read_range(bytes.len() as u64, 0).unwrap().is_empty());
    }

    #[test]
    fn cancelling_preload_preserves_only_verified_chunks_and_can_resume() {
        let (_dir, data, bytes) = fixture();
        let calls = AtomicUsize::new(0);
        assert!(!data.preload(|| calls.fetch_add(1, Ordering::Relaxed) >= 1).unwrap());
        let MessageData::File(bodies) = &data else { panic!("file expected") };
        assert!(bodies.chunks[0].get().is_some());
        assert!(bodies.chunks[1].get().is_none());
        assert!(data.needs_preload());
        assert_eq!(data.read_range(0, 256).unwrap(), bytes[..256]);
        assert!(data.preload(|| false).unwrap());
        assert!(!data.needs_preload());
    }

    #[test]
    fn selected_reads_and_bulk_reads_can_run_with_preload() {
        let (_dir, data, bytes) = fixture();
        std::thread::scope(|scope| {
            scope.spawn(|| assert!(data.preload(|| false).unwrap()));
            for chunk in 0..4 {
                let data = &data;
                let bytes = &bytes;
                scope.spawn(move || {
                    let start = chunk * CHUNK_SIZE;
                    let end = (start + CHUNK_SIZE).min(bytes.len());
                    for _ in 0..4 {
                        assert_eq!(data.read_range(start as u64, (end - start) as u64).unwrap(), bytes[start..end]);
                    }
                });
            }
        });
        assert!(!data.needs_preload());
    }

    #[test]
    fn late_chunk_corruption_and_truncation_never_publish_a_complete_preload() {
        use std::io::{Seek, SeekFrom, Write};
        for truncate in [false, true] {
            let (dir, data, bytes) = fixture();
            assert_eq!(data.read_range(0, 128).unwrap(), bytes[..128]);
            let mut file = File::options().write(true).open(dir.path().join("bodies.dat")).unwrap();
            if truncate {
                file.set_len((CHUNK_SIZE + 128) as u64).unwrap();
            } else {
                file.seek(SeekFrom::Start((CHUNK_SIZE * 2) as u64)).unwrap();
                file.write_all(&[0xFF]).unwrap();
            }
            assert!(data.preload(|| false).is_err());
            assert!(data.needs_preload());
            assert!(data.read_range((CHUNK_SIZE * 2) as u64, 128).is_err());
            assert_eq!(data.read_range(0, 128).unwrap(), bytes[..128], "already verified data remains usable");
        }
    }

    #[test]
    fn corrupt_chunk_recovers_once_and_failed_recovery_stays_an_error() {
        let dir = super::super::tests::TempDir::new();
        let path = dir.path().join("corrupt.dat");
        let bytes = vec![42; CHUNK_SIZE + 128];
        std::fs::write(&path, vec![0; bytes.len()]).unwrap();
        let attempts = Arc::new(AtomicUsize::new(0));
        let recoveries = attempts.clone();
        let original = bytes.clone();
        let data = MessageData::file(
            File::open(&path).unwrap(),
            bytes.len() as u64,
            crc32fast::hash(&bytes),
            bytes.chunks(CHUNK_SIZE).map(crc32fast::hash).collect(),
            move || {
                recoveries.fetch_add(1, Ordering::Relaxed);
                Ok(original.clone())
            },
        )
        .unwrap();
        std::thread::scope(|scope| {
            for offset in [0, CHUNK_SIZE] {
                let data = &data;
                scope.spawn(move || assert_eq!(data.read_range(offset as u64, 128).unwrap(), vec![42; 128]));
            }
        });
        assert_eq!(attempts.load(Ordering::Relaxed), 1);
        assert!(!data.needs_preload());
        let failed = MessageData::file(
            File::open(path).unwrap(),
            bytes.len() as u64,
            crc32fast::hash(&bytes),
            bytes.chunks(CHUNK_SIZE).map(crc32fast::hash).collect(),
            || Err("source missing".into()),
        )
        .unwrap();
        assert!(failed.read_range(0, 128).unwrap_err().to_string().contains("source missing"));
        assert!(failed.preload(|| false).is_err());
        assert!(failed.needs_preload());
    }
}
