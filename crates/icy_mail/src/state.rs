//! Per-packet read marks and the list of recently opened packets.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use i18n_embed_fl::fl;
use serde::{Deserialize, Serialize};

use crate::drafts::{atomic_write, bbs_id, storage_path, DraftError};
use crate::qwk::{MessageInfo, QwkPackage};
use crate::LANGUAGE_LOADER;

const MAX_RECENT: usize = 10;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredRead {
    bbs_id: String,
    /// `(conference, message number)`; written by earlier versions, still accepted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    read: Vec<(u16, u32)>,
    /// `(conference, first, last)` inclusive runs of read message numbers, which stay valid when
    /// the packet is reloaded and keep the file small for packets with many read messages.
    #[serde(default)]
    ranges: Vec<(u16, u32, u32)>,
}

/// Which messages of a packet the user has already read.
#[derive(Clone, Debug)]
pub struct ReadState {
    path: PathBuf,
    bbs_id: String,
    read: BTreeSet<(u16, u32)>,
}

impl ReadState {
    pub fn open(packet_path: &Path, package: &QwkPackage) -> crate::Res<Self> {
        Self::load(packet_path, package, None)
    }

    /// Like [`Self::open`], but keeps the file in `directory` instead of the user's data directory.
    pub fn open_in(packet_path: &Path, package: &QwkPackage, directory: &Path) -> crate::Res<Self> {
        Self::load(packet_path, package, Some(directory))
    }

    fn load(packet_path: &Path, package: &QwkPackage, directory: Option<&Path>) -> crate::Res<Self> {
        let bbs_id = bbs_id(package)?;
        let path = storage_path(packet_path, &bbs_id, directory, ".read.toml")?;
        let read = if path.exists() {
            let stored: StoredRead = toml::from_str(&fs::read_to_string(&path)?)?;
            if stored.bbs_id != bbs_id {
                return Err(DraftError::WrongPacket.into());
            }
            let mut ranges = stored.ranges;
            ranges.sort_unstable();
            // Only this packet's messages, so a damaged file cannot expand into huge ranges.
            let in_range = |conference: u16, number: u32| {
                let end = ranges.partition_point(|range| (range.0, range.1) <= (conference, number));
                end > 0 && ranges[end - 1].0 == conference && ranges[end - 1].2 >= number
            };
            let mut read: BTreeSet<_> = stored.read.into_iter().collect();
            if !ranges.is_empty() {
                read.extend(
                    package
                        .infos
                        .iter()
                        .filter(|info| in_range(info.conference, info.number))
                        .map(|info| (info.conference, info.number)),
                );
            }
            read
        } else {
            BTreeSet::new()
        };
        Ok(Self { path, bbs_id, read })
    }

    pub fn is_read(&self, info: &MessageInfo) -> bool {
        self.read.contains(&(info.conference, info.number))
    }

    /// Package indices of the read messages.
    pub fn indices(&self, package: &QwkPackage) -> HashSet<usize> {
        if self.read.is_empty() {
            return HashSet::new();
        }
        package.infos.iter().filter(|info| self.is_read(info)).map(|info| info.index).collect()
    }

    /// Marks the messages read or unread and saves the change. Returns whether anything changed.
    ///
    /// The in-memory state keeps the change when saving fails, so the list does not flip back while
    /// the caller reports the error.
    pub fn set<'a>(&mut self, infos: impl IntoIterator<Item = &'a MessageInfo>, read: bool) -> crate::Res<bool> {
        let mut changed = false;
        for info in infos {
            let key = (info.conference, info.number);
            changed |= if read { self.read.insert(key) } else { self.read.remove(&key) };
        }
        if changed {
            self.save()?;
        }
        Ok(changed)
    }

    fn save(&self) -> crate::Res<()> {
        let mut ranges: Vec<(u16, u32, u32)> = Vec::new();
        for &(conference, number) in &self.read {
            match ranges.last_mut() {
                Some((last_conference, _, last)) if *last_conference == conference && last.checked_add(1) == Some(number) => *last = number,
                _ => ranges.push((conference, number, number)),
            }
        }
        let content = toml::to_string(&StoredRead {
            bbs_id: self.bbs_id.clone(),
            read: Vec::new(),
            ranges,
        })?;
        atomic_write(&self.path, |file| {
            file.write_all(content.as_bytes())?;
            Ok(())
        })?;
        Ok(())
    }
}

/// Where the reader keeps the recent packet list, taglines and the address book.
pub fn data_directory() -> crate::Res<PathBuf> {
    Ok(directories::ProjectDirs::from("com", "GitHub", "icy_mail")
        .ok_or_else(|| fl!(LANGUAGE_LOADER, "packet-user-data-directory-unavailable"))?
        .data_local_dir()
        .to_path_buf())
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredRecent {
    packets: Vec<PathBuf>,
}

/// Most recently opened packets, newest first.
#[derive(Clone, Debug)]
pub struct RecentPackets {
    path: PathBuf,
    pub packets: Vec<PathBuf>,
}

impl RecentPackets {
    pub fn open() -> crate::Res<Self> {
        Self::open_in(&data_directory()?)
    }

    pub fn open_in(directory: &Path) -> crate::Res<Self> {
        let path = directory.join("recent.toml");
        let packets = Self::read(&path)?;
        Ok(Self { path, packets })
    }

    fn read(path: &Path) -> crate::Res<Vec<PathBuf>> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let stored: StoredRecent = toml::from_str(&fs::read_to_string(path)?)?;
        Ok(stored.packets)
    }

    /// The stored list, which another window may have changed since this one was opened.
    fn current(&self) -> Vec<PathBuf> {
        Self::read(&self.path).unwrap_or_else(|_| self.packets.clone())
    }

    pub fn add(&mut self, packet: &Path) -> crate::Res<()> {
        let packet = packet.canonicalize().unwrap_or_else(|_| packet.to_path_buf());
        let mut packets = self.current();
        packets.retain(|existing| *existing != packet);
        packets.insert(0, packet);
        packets.truncate(MAX_RECENT);
        self.commit(packets)
    }

    pub fn remove(&mut self, packet: &Path) -> crate::Res<()> {
        let mut packets = self.current();
        packets.retain(|existing| existing != packet);
        self.commit(packets)
    }

    fn commit(&mut self, packets: Vec<PathBuf>) -> crate::Res<()> {
        if packets == self.packets {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string(&StoredRecent { packets: packets.clone() })?;
        atomic_write(&self.path, |file| {
            file.write_all(content.as_bytes())?;
            Ok(())
        })?;
        self.packets = packets;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_marks_survive_reloading_and_ignore_other_packets() {
        let (dir, package) = crate::qwk::tests::load();
        let packet = dir.path().join("TEST.QWK");
        let mut state = ReadState::open_in(&packet, &package, dir.path()).unwrap();
        assert!(state.indices(&package).is_empty());
        assert!(state.set([&package.infos[1], &package.infos[3]], true).unwrap());
        assert!(!state.set([&package.infos[1]], true).unwrap());
        let state = ReadState::open_in(&packet, &package, dir.path()).unwrap();
        assert_eq!(state.indices(&package), HashSet::from([1, 3]));
        let mut state = state;
        state.set([&package.infos[1]], false).unwrap();
        assert_eq!(ReadState::open_in(&packet, &package, dir.path()).unwrap().indices(&package), HashSet::from([3]));
        let mut other = package.clone();
        other.control_file.bbs_id = "OTHER".into();
        assert!(ReadState::open_in(&packet, &other, dir.path()).unwrap().indices(&other).is_empty());
    }

    #[test]
    fn read_marks_are_stored_as_ranges_and_accept_the_old_format() {
        let (dir, package) = crate::qwk::tests::load();
        let packet = dir.path().join("TEST.QWK");
        let mut state = ReadState::open_in(&packet, &package, dir.path()).unwrap();
        state.set(package.infos.iter(), true).unwrap();
        let content = fs::read_to_string(&state.path).unwrap();
        let stored: StoredRead = toml::from_str(&content).unwrap();
        assert!(stored.read.is_empty());
        assert_eq!(stored.ranges, [(1, 10, 11), (2, 12, 13)]);
        assert_eq!(ReadState::open_in(&packet, &package, dir.path()).unwrap().indices(&package).len(), 4);

        let damaged = format!("bbs_id = {:?}\nranges = [[1, 0, 4294967295], [3, 5, 1]]\n", stored.bbs_id);
        fs::write(&state.path, damaged).unwrap();
        assert_eq!(
            ReadState::open_in(&packet, &package, dir.path()).unwrap().indices(&package),
            HashSet::from([0, 1])
        );

        let old = format!("bbs_id = {:?}\nread = [[1, 11], [2, 13]]\n", stored.bbs_id);
        fs::write(&state.path, old).unwrap();
        assert_eq!(
            ReadState::open_in(&packet, &package, dir.path()).unwrap().indices(&package),
            HashSet::from([1, 3])
        );
    }

    #[test]
    fn recent_packets_are_unique_newest_first_and_capped() {
        let (dir, _package) = crate::qwk::tests::load();
        let mut recent = RecentPackets::open_in(dir.path()).unwrap();
        for index in 0..12 {
            recent.add(&dir.path().join(format!("P{index}.QWK"))).unwrap();
        }
        recent.add(&dir.path().join("P5.QWK")).unwrap();
        let reloaded = RecentPackets::open_in(dir.path()).unwrap();
        assert_eq!(reloaded.packets.len(), MAX_RECENT);
        assert_eq!(reloaded.packets[0], dir.path().join("P5.QWK"));
        assert_eq!(reloaded.packets.iter().filter(|path| path.ends_with("P5.QWK")).count(), 1);
        let mut reloaded = reloaded;
        reloaded.remove(&dir.path().join("P5.QWK")).unwrap();
        assert!(!RecentPackets::open_in(dir.path()).unwrap().packets.iter().any(|path| path.ends_with("P5.QWK")));
    }
}
