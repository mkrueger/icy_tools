use std::{fs::File, io::Write, path::Path, sync::Arc};

use crate::{
    drafts::{Compose, DraftField, DraftStore},
    qwk::{tests::TempDir, ExtractionCache, PacketFormat, QwkPackage},
    reader::Reader,
};

pub(crate) fn packet(path: &Path, files: &[(String, Vec<u8>)]) {
    let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
    for (name, data) in files {
        zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn blue_wave_native_metadata_and_bodies_survive_cold_warm_and_disabled_cache() {
    for (level, uses_upl) in [(2, false), (3, false), (3, true)] {
        let dir = TempDir::new();
        let path = dir.path().join("incoming.SU0");
        let mut files = crate::blue_wave::fixture_files(level, uses_upl);
        files.push(("WELCOME.ANS".into(), b"Welcome\r\n".to_vec()));
        files.push(("NEWS".into(), b"News\r\n".to_vec()));
        files.push(("BLT-1".into(), b"Bulletin\r\n".to_vec()));
        packet(&path, &files);
        let cache = ExtractionCache::new(dir.path().join("cache"), 30);
        let plain = QwkPackage::load_from_file(&path).unwrap();
        let cold = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        assert!(warm.needs_preload(), "metadata must not eagerly read DAT bodies");
        assert_eq!(warm.format(), PacketFormat::BlueWave);
        assert_eq!(warm.blue_wave.as_ref().unwrap().info.uses_upl, uses_upl);
        assert_eq!(warm.bbs_name, "My BBS!");
        assert_eq!(warm.message_count(), 1);
        assert_eq!(warm.descriptors[0].body_len, Some(9));
        assert_eq!(warm.descriptors[0].offset, 1);
        assert_eq!(warm.infos[0].number, 45);
        assert_eq!(warm.infos[0].ref_number, 42);
        assert_eq!(warm.infos[0].conference, 12);
        assert_eq!(warm.infos[0].lines, 1);
        assert!(warm.infos[0].private);
        assert_eq!(warm.infos[0].from.as_str(), "Bob");
        assert_eq!(warm.infos[0].to.as_str(), "Alice");
        assert_eq!(warm.infos[0].subject.as_str(), "Hey");
        assert_eq!(warm.files.len(), 3);
        assert!(!warm.capabilities.supports_subscriptions());
        assert_eq!(plain.infos, cold.infos);
        assert_eq!(cold.infos, warm.infos);
        assert_eq!(cold.cached_threads(), warm.cached_threads());
        for package in [&plain, &cold, &warm] {
            assert_eq!(package.get_message(0).unwrap().text.as_slice(), b"Hello\n\x82!");
            assert_eq!(package.clone().get_message(0).unwrap().ref_msg_number, 42);
        }
        assert!(warm.preload_messages(|| false).unwrap());
        assert!(!warm.needs_preload());
        let disabled = ExtractionCache::new(dir.path().join("disabled"), 0);
        assert_eq!(QwkPackage::load_from_file_cached(&path, &disabled).unwrap().infos, plain.infos);
        assert!(!dir.path().join("disabled").exists());
    }
}

#[test]
fn blue_wave_reply_export_and_import_use_native_headers_and_keep_local_drafts() {
    for (level, uses_upl) in [(2, false), (3, false), (3, true)] {
        let dir = TempDir::new();
        let path = dir.path().join("mail.BW");
        packet(&path, &crate::blue_wave::fixture_files(level, uses_upl));
        let package = QwkPackage::load_from_file(&path).unwrap();
        let mut store = DraftStore::open(&path, &package).unwrap();
        assert_eq!(store.reply_extension(), "new");
        assert_eq!(store.default_export_path(&path), dir.path().join("TEST.NEW"));
        assert_eq!(store.field_limit(DraftField::From), 35);
        assert_eq!(store.field_limit(DraftField::To), 35);
        assert_eq!(store.field_limit(DraftField::Subject), 71);
        let mut draft = store.prepare(&package, Compose::Reply { index: 0 }).unwrap();
        draft.subject = "S".repeat(71);
        draft.body = "Pi \u{03c0}, \u{00ec}\n\x1b[31mCaf\u{00e9}\x1b[0m".into();
        if uses_upl {
            draft.ref_number = u32::MAX;
        }
        assert!(store.issues(&draft).is_empty(), "{:?}", store.issues(&draft));
        draft.to = "T".repeat(36);
        assert!(store.issues(&draft).iter().any(|issue| issue.field == DraftField::To));
        draft.to = "Bob".into();
        store.insert(draft.clone()).unwrap();
        let destination = store.default_export_path(&path);
        assert!(store.export(&path).is_err());
        store.export(&destination).unwrap();
        assert_eq!(store.drafts(), &[draft.clone()]);
        assert_eq!(DraftStore::open(&path, &package).unwrap().drafts(), &[draft.clone()]);
        let mut zip = zip::ZipArchive::new(File::open(&destination).unwrap()).unwrap();
        let header = if uses_upl { "TEST.UPL" } else { "TEST.UPI" };
        assert!(zip.by_name(header).is_ok());
        assert!(zip.by_name("TEST.MSG").is_err(), "Blue Wave must never export lossy QWK records");
        let imported = store.read_rep(&destination).unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].subject, draft.subject);
        assert_eq!(imported[0].to, draft.to);
        assert_eq!(imported[0].from, draft.from);
        assert_eq!(imported[0].body, draft.text());
        assert_eq!(imported[0].ref_number, if uses_upl { draft.ref_number } else { 0 });
        assert_eq!(imported[0].private, draft.private);
        assert_eq!(store.import_drafts(imported).unwrap(), 1);
        assert_eq!(store.drafts().len(), 2);
        assert_ne!(store.drafts()[0].id, store.drafts()[1].id);
        assert!(store.set_subscription(12, true).is_err());
    }
}

#[test]
fn blue_wave_reader_search_and_personal_mail_accept_real_name_and_alias() {
    let dir = TempDir::new();
    let path = dir.path().join("mail.zip");
    packet(&path, &crate::blue_wave::fixture_files(3, true));
    let package = QwkPackage::load_from_file(&path).unwrap();
    assert!(package.matches_personal("Alice", "Alice"));
    assert!(package.matches_personal("Ally", "Alice"));
    assert!(package.matches_personal("Alice", "Ally"));
    assert!(!package.matches_personal("Bob", "Alice"));
    assert!(!package.matches_personal("Ally", "Unrelated"));
    let mut reader = Reader::default();
    reader.set_package(Arc::new(package));
    reader.personal = Some("Ally".into());
    reader.rebuild_messages();
    assert_eq!(reader.messages.len(), 1);
    reader.personal = Some("Bob".into());
    reader.rebuild_messages();
    assert!(reader.messages.is_empty());
}

#[test]
fn blue_wave_rejects_incomplete_duplicate_and_mixed_packets() {
    let dir = TempDir::new();
    for scenario in ["incomplete", "duplicate", "mixed"] {
        let mut files = crate::blue_wave::fixture_files(3, true);
        match scenario {
            "incomplete" => files.retain(|(name, _)| !name.ends_with(".MIX")),
            "duplicate" => files.push(("subdir/TEST.INF".into(), files[0].1.clone())),
            "mixed" => {
                files.push(("CONTROL.DAT".into(), Vec::new()));
                files.push(("MESSAGES.DAT".into(), Vec::new()));
            }
            _ => unreachable!(),
        }
        let path = dir.path().join(format!("{scenario}.zip"));
        packet(&path, &files);
        assert!(QwkPackage::load_from_file(&path).is_err(), "{scenario} packet must fail");
    }
}

#[test]
fn blue_wave_body_without_leading_marker_keeps_all_text() {
    let dir = TempDir::new();
    let mut files = crate::blue_wave::fixture_files(3, true);
    files.iter_mut().find(|(name, _)| name.ends_with(".DAT")).unwrap().1[0] = b'X';
    let path = dir.path().join("marker.zip");
    packet(&path, &files);
    let package = QwkPackage::load_from_file(&path).unwrap();
    assert_eq!(package.read_message(0).unwrap().text.as_slice(), b"XHello\n\x82!");
}

#[test]
fn blue_wave_reading_tolerates_unused_or_unusable_metadata_in_all_cache_modes() {
    for scenario in [
        "missing_login",
        "long_login",
        "invalid_alias",
        "empty_tag",
        "long_tag",
        "duplicate_tag",
        "cp437_tag",
        "personal_count",
        "empty_area_pointer",
        "padded_area_number",
    ] {
        let dir = TempDir::new();
        let mut files = crate::blue_wave::fixture_files(3, true);
        match scenario {
            "missing_login" => files[0].1[76..119].fill(0),
            "long_login" => files[0].1[76..119].fill(b'A'),
            "invalid_alias" => files[0].1[119] = b'\r',
            "empty_tag" => files[0].1[1236..1257].fill(0),
            "long_tag" => files[0].1[1236..1257].fill(b'A'),
            "duplicate_tag" => {
                let tag = files[0].1[1236..1257].to_vec();
                files[0].1[1316..1337].copy_from_slice(&tag);
            }
            "cp437_tag" => files[0].1[1236] = 0x82,
            "personal_count" => files[1].1[8..10].copy_from_slice(&u16::MAX.to_le_bytes()),
            "empty_area_pointer" => files[1].1[24..28].copy_from_slice(&u32::MAX.to_le_bytes()),
            "padded_area_number" => {
                files[0].1[1230..1236].copy_from_slice(b"  12  ");
                files[1].1[..6].copy_from_slice(b"  12  ");
            }
            _ => unreachable!(),
        }
        let path = dir.path().join(format!("{scenario}.zip"));
        packet(&path, &files);
        let cache = ExtractionCache::new(dir.path().join("cache"), 30);
        let plain = QwkPackage::load_from_file(&path).unwrap();
        let cold = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        assert_eq!(plain.infos, cold.infos, "{scenario}");
        assert_eq!(cold.infos, warm.infos, "{scenario}");
        for package in [&plain, &cold, &warm] {
            assert_eq!(package.message_count(), 1, "{scenario}");
            assert_eq!(package.read_message(0).unwrap().text.as_slice(), b"Hello\n\x82!", "{scenario}");
            let posting = crate::blue_wave::posting_defaults(package.blue_wave.as_ref().unwrap(), 12);
            assert_eq!(
                posting.is_ok(),
                matches!(scenario, "personal_count" | "empty_area_pointer" | "padded_area_number"),
                "{scenario}"
            );
        }
    }
}

#[test]
fn blue_wave_cached_dat_corruption_recovers_without_turning_into_qwk_data() {
    let dir = TempDir::new();
    let path = dir.path().join("mail.BW");
    packet(&path, &crate::blue_wave::fixture_files(3, true));
    let cache_directory = dir.path().join("cache");
    let cache = ExtractionCache::new(cache_directory.clone(), 30);
    QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    assert!(warm.needs_preload());
    let extraction = std::fs::read_dir(cache_directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.join("manifest.toml").is_file())
        .unwrap();
    let body = extraction.join("1.dat");
    let mut data = std::fs::read(&body).unwrap();
    data[2] ^= 1;
    std::fs::write(body, data).unwrap();
    assert_eq!(warm.read_message(0).unwrap().text.as_slice(), b"Hello\n\x82!");
    assert_eq!(warm.format(), PacketFormat::BlueWave);
}

#[test]
fn blue_wave_reply_import_is_atomic_on_malformed_archives() {
    let dir = TempDir::new();
    let path = dir.path().join("mail.BW");
    packet(&path, &crate::blue_wave::fixture_files(3, true));
    let package = QwkPackage::load_from_file(&path).unwrap();
    let mut store = DraftStore::open(&path, &package).unwrap();
    let mut draft = store.prepare(&package, Compose::Reply { index: 0 }).unwrap();
    draft.body = "Keep me".into();
    store.insert(draft.clone()).unwrap();
    let invalid = dir.path().join("bad.NEW");
    packet(&invalid, &[("TEST.UPL".into(), vec![0; 10])]);
    assert!(store.import_rep(&invalid).is_err());
    assert_eq!(store.drafts(), &[draft.clone()]);
    assert_eq!(DraftStore::open(&path, &package).unwrap().drafts(), &[draft]);
}

#[test]
fn blue_wave_message_facade_preserves_unparsed_dates() {
    let dir = TempDir::new();
    let path = dir.path().join("mail.BW");
    packet(&path, &crate::blue_wave::fixture_files(3, true));
    let mut package = QwkPackage::load_from_file(&path).unwrap();
    let mut native = package.blue_wave.as_ref().unwrap().as_ref().clone();
    native.messages[0].date_known = false;
    native.messages[0].date_raw = b"Unspecified".to_vec();
    package.blue_wave = Some(Arc::new(native));
    assert_eq!(package.read_message(0).unwrap().date_time.as_slice(), b"Unspecified");
}

#[test]
fn blue_wave_composing_respects_writable_areas_aliases_and_required_privacy() {
    let dir = TempDir::new();
    let path = dir.path().join("mail.BW");
    packet(&path, &crate::blue_wave::fixture_files(3, true));
    let mut package = QwkPackage::load_from_file(&path).unwrap();
    let native = Arc::make_mut(package.blue_wave.as_mut().unwrap());
    native.areas[0].flags &= !crate::blue_wave::INF_POST;
    native.areas[1].flags |= crate::blue_wave::INF_ALIAS_NAME | crate::blue_wave::INF_NO_PUBLIC;
    let store = DraftStore::open(&path, &package).unwrap();
    assert!(!store.can_post(12));
    assert!(store.can_post(24));
    assert!(store.prepare(&package, Compose::Reply { index: 0 }).is_err());
    let forwarded = store.prepare(&package, Compose::Forward { index: 0 }).unwrap();
    assert_eq!(forwarded.conference, 24);
    assert_eq!(forwarded.from, "Ally");
    assert!(forwarded.private);
    let mut draft = store.prepare(&package, Compose::New { conference: 24 }).unwrap();
    draft.to = "Bob".into();
    draft.subject = "Private message".into();
    draft.body = "Hello".into();
    assert!(store.issues(&draft).is_empty());
    draft.private = false;
    assert!(!store.issues(&draft).is_empty());
    draft.private = true;
    draft.from = "Alice".into();
    assert!(!store.issues(&draft).is_empty());
}

#[test]
fn blue_wave_unnumbered_messages_have_distinct_local_marks_but_no_wire_reference() {
    let dir = TempDir::new();
    let path = dir.path().join("unnumbered.BW");
    let mut files = crate::blue_wave::fixture_files(3, true);
    let dat_len = files.iter().find(|(name, _)| name.ends_with(".DAT")).unwrap().1.len();
    let fti = &mut files.iter_mut().find(|(name, _)| name.ends_with(".FTI")).unwrap().1;
    fti[164..168].fill(0);
    let mut second = fti.clone();
    second[170..174].copy_from_slice(&(dat_len as u32).to_le_bytes());
    fti.extend(second);
    let mix = &mut files.iter_mut().find(|(name, _)| name.ends_with(".MIX")).unwrap().1;
    mix[6..8].copy_from_slice(&2u16.to_le_bytes());
    mix[24..28].copy_from_slice(&372u32.to_le_bytes());
    let dat = &mut files.iter_mut().find(|(name, _)| name.ends_with(".DAT")).unwrap().1;
    dat.extend(dat.clone());
    packet(&path, &files);
    let cache = ExtractionCache::new(dir.path().join("cache"), 30);
    let cold = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    assert_eq!(cold.infos, warm.infos);
    assert_eq!(warm.message_count(), 2);
    assert_ne!(warm.infos[0].number, warm.infos[1].number);
    assert!(warm.infos.iter().all(|info| info.number > u16::MAX as u32));
    let mut marks = crate::state::ReadState::open_in(&path, &warm, dir.path()).unwrap();
    marks.set([&warm.infos[0]], true).unwrap();
    marks.set_starred(&warm.infos[1], true).unwrap();
    let marks = crate::state::ReadState::open_in(&path, &warm, dir.path()).unwrap();
    assert!(marks.is_read(&warm.infos[0]));
    assert!(!marks.is_read(&warm.infos[1]));
    assert!(!marks.is_starred(&warm.infos[0]));
    assert!(marks.is_starred(&warm.infos[1]));
    let mut store = DraftStore::open(&path, &warm).unwrap();
    let mut draft = store.prepare(&warm, Compose::Reply { index: 1 }).unwrap();
    assert_eq!(draft.ref_number, 0);
    draft.body = "Reply without a fabricated reference".into();
    store.insert(draft).unwrap();
    let destination = store.default_export_path(&path);
    store.export(&destination).unwrap();
    assert_eq!(store.read_rep(&destination).unwrap()[0].ref_number, 0);

    let fti = &mut files.iter_mut().find(|(name, _)| name.ends_with(".FTI")).unwrap().1;
    fti[164..166].copy_from_slice(&45u16.to_le_bytes());
    fti[350..352].copy_from_slice(&45u16.to_le_bytes());
    packet(&path, &files);
    assert!(QwkPackage::load_from_file(&path).err().unwrap().to_string().contains("Duplicate"));
}

#[test]
#[ignore = "requires ICY_MAIL_TEST_BLUE_WAVE_PACKET pointing to a local real-world packet"]
fn real_world_blue_wave_packet_opens_all_bodies_and_cached_metadata() {
    let path = std::path::PathBuf::from(std::env::var_os("ICY_MAIL_TEST_BLUE_WAVE_PACKET").expect("set ICY_MAIL_TEST_BLUE_WAVE_PACKET"));
    let dir = TempDir::new();
    let cache = ExtractionCache::new(dir.path().join("cache"), 30);
    let package = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    assert!(package.message_count() > 0);
    for index in 0..package.message_count() {
        package.read_message(index).unwrap();
    }
    let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    assert_eq!(package.infos, warm.infos);
    assert_eq!(package.cached_threads(), warm.cached_threads());
    for index in 0..warm.message_count() {
        assert_eq!(package.read_message(index).unwrap().text, warm.read_message(index).unwrap().text);
    }
}
