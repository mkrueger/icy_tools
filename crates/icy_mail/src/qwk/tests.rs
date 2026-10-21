use std::io::Write;

use crate::qwk::QwkPackage;

#[test]
fn open_packet_filter_includes_every_blue_wave_weekday_sequence_in_both_cases() {
    let extensions = crate::qwk::packet_extensions();
    for day in ["su", "mo", "tu", "we", "th", "fr", "sa"] {
        for sequence in 0..=9 {
            let extension = format!("{day}{sequence}");
            assert!(extensions.contains(&extension));
            assert!(extensions.contains(&extension.to_ascii_uppercase()));
        }
    }
    for extension in ["qwk", "bw", "zip", "arj", "lzh", "lha", "rar", "7z", "arc", "zoo", "rep"] {
        assert!(extensions.iter().any(|candidate| candidate == extension));
        assert!(extensions.contains(&extension.to_ascii_uppercase()));
    }
    assert_eq!(extensions.len(), 162);
    assert_eq!(extensions.iter().collect::<std::collections::HashSet<_>>().len(), extensions.len());
    assert!(!extensions.iter().any(|extension| extension.contains(['*', '?', '.'])));
}

/// Builds a 128-byte QWK message header.
#[allow(clippy::too_many_arguments)] // each argument maps directly to a fixed QWK header field
pub(super) fn header(status: u8, number: u32, date_time: &str, to: &str, from: &str, subject: &str, ref_number: u32, blocks: u32, conference: u16) -> Vec<u8> {
    fn field(value: &str, len: usize) -> Vec<u8> {
        let mut bytes = value.as_bytes().to_vec();
        bytes.truncate(len);
        bytes.resize(len, b' ');
        bytes
    }

    let mut out = Vec::with_capacity(128);
    out.push(status);
    out.extend(field(&number.to_string(), 7));
    out.extend(field(date_time, 13));
    out.extend(field(to, 25));
    out.extend(field(from, 25));
    out.extend(field(subject, 25));
    out.extend(field("", 12));
    out.extend(field(&if ref_number == 0 { String::new() } else { ref_number.to_string() }, 8));
    out.extend(field(&blocks.to_string(), 6));
    out.push(225);
    out.extend(conference.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.push(b' ');
    assert_eq!(out.len(), 128, "QWK headers are always 128 bytes");
    out
}

/// Appends a message (header + body padded to whole 128-byte blocks) to MESSAGES.DAT.
#[allow(clippy::too_many_arguments)] // each argument maps directly to a fixed QWK header field
fn message(out: &mut Vec<u8>, number: u32, date_time: &str, from: &str, subject: &str, ref_number: u32, conference: u16, body_lines: usize) {
    // QWK separates body lines with 0xE3, not LF.
    let mut body: Vec<u8> = Vec::new();
    if subject.len() > 25 {
        body.extend(b"Subject: ");
        body.extend(subject.as_bytes());
        body.extend([0xE3, 0xE3]);
    }
    for line in 0..body_lines {
        body.extend(format!("line {line}").as_bytes());
        body.push(0xE3);
    }
    let body_blocks = body.len().div_ceil(128).max(1);
    body.resize(body_blocks * 128, b' ');

    out.extend(header(
        b' ',
        number,
        date_time,
        "ALL",
        from,
        subject,
        ref_number,
        body_blocks as u32 + 1,
        conference,
    ));
    out.extend(body);
}

pub(super) fn control_dat() -> Vec<u8> {
    let mut out = String::new();
    out.push_str("TEST BBS\r\n");
    out.push_str("Somewhere\r\n");
    out.push_str("000-000-0000\r\n");
    out.push_str("Sysop\r\n");
    out.push_str("00000,TEST\r\n");
    out.push_str("01-01-202000:00\r\n");
    out.push_str("USER\r\n");
    out.push_str("\r\n");
    out.push_str("0\r\n");
    out.push_str("4\r\n"); // message count
    out.push_str("2\r\n"); // conference count
    out.push_str("1\r\nGeneral\r\n");
    out.push_str("2\r\nRetro\r\n");
    out.push_str("HELLO\r\nNEWS\r\nGOODBYE\r\n");
    out.into_bytes()
}

#[test]
fn ftn_identity_and_timezones_survive_qwk_indexing_and_warm_cache() {
    let dir = TempDir::new();
    let path = dir.path().join("FTN.QWK");
    let mut messages = vec![b' '; 128];
    for (number, subject, body) in [
        (1, "Original", b"\x01MSGID: 2:1/1 12345678\xe3\x01TZUTC: 0545\xe3First\xe3".as_slice()),
        (2, "Changed", b"\x01REPLY: 2:1/1 12345678\xe3\x01TZUTCINFO: +0545\xe3Answer\xe3".as_slice()),
    ] {
        let blocks = body.len().div_ceil(128);
        messages.extend(header(b' ', number, "01-01-2601:15", "All", "Alice", subject, 0, blocks as u32 + 1, 1));
        messages.extend(body);
        messages.resize(messages.len() + blocks * 128 - body.len(), b' ');
    }
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("CONTROL.DAT", options).unwrap();
    zip.write_all(&control_dat()).unwrap();
    zip.start_file("MESSAGES.DAT", options).unwrap();
    zip.write_all(&messages).unwrap();
    zip.finish().unwrap();
    let cache = crate::qwk::ExtractionCache::new(dir.path().join("cache"), 30);
    let plain = QwkPackage::load_from_file(&path).unwrap();
    let cold = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    let expected = chrono::DateTime::parse_from_rfc3339("2025-12-31T19:30:00Z").unwrap();
    for package in [&plain, &cold, &warm] {
        assert_eq!(package.infos, plain.infos);
        assert_eq!(package.infos[0].ftn.utc_offset, Some(20700));
        assert_eq!(package.infos[0].ftn.utc_timestamp, Some(expected.timestamp()));
        assert_eq!(package.infos[0].date, expected.with_timezone(&chrono::Local).naive_local());
        assert_eq!(package.infos[1].date, package.infos[0].date);
        let rows = crate::threading::build_threads(&package.infos.iter().collect::<Vec<_>>());
        assert_eq!(rows.iter().map(|row| (row.index, row.depth)).collect::<Vec<_>>(), [(0, 0), (1, 1)]);
    }
    assert_eq!(warm.cached_threads(), cold.cached_threads());
}

#[test]
fn all_qwk_statuses_open_without_scanning_bodies_as_headers() {
    let statuses = b" -*+~`%^!#$";
    let mut messages = vec![b' '; 128];
    for (index, status) in statuses.iter().enumerate() {
        messages.extend(header(*status, index as u32 + 1, "10-12-2618:14", "ALL", "AUTHOR", "Status", 0, 3, 2));
        // A body block that looks like a header must never become a message.
        messages.extend(header(b' ', 999, "10-12-2618:14", "ALL", "BODY", "Not a message", 0, 1, 1));
        messages.extend([b' '; 128]);
    }
    let mut deleted = header(b' ', 100, "10-12-2618:14", "ALL", "AUTHOR", "Deleted", 0, 3, 1);
    deleted[122] = jamjam::qwk::qwk_message::MSG_INACTIVE;
    messages.extend(deleted);
    messages.extend(header(b' ', 998, "10-12-2618:14", "ALL", "BODY", "Also not a message", 0, 1, 1));
    messages.extend([b' '; 128]);
    message(&mut messages, 101, "10-12-2618:14", "AUTHOR", "After deleted", 0, 1, 1);

    let dir = TempDir::new();
    let path = dir.path().join("STATUSES.QWK");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("CONTROL.DAT", options).unwrap();
    let control = String::from_utf8(control_dat()).unwrap().replacen("4\r\n2\r\n", "12\r\n1\r\n", 1);
    zip.write_all(control.as_bytes()).unwrap();
    zip.start_file("MESSAGES.DAT", options).unwrap();
    zip.write_all(&messages).unwrap();
    zip.finish().unwrap();

    let cache = crate::qwk::ExtractionCache::new(dir.path().join("cache"), 30);
    let uncached = QwkPackage::load_from_file(&path).unwrap();
    let cold = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    assert!(warm.cached_threads().is_some());
    for package in [&uncached, &cold, &warm] {
        assert_eq!(package.message_count(), statuses.len() + 1);
        assert_eq!(package.infos, uncached.infos);
        assert_eq!(package.control_file.conferences[1].name, "Retro");
        for (index, status) in statuses.iter().enumerate() {
            let message = package.read_message(index).unwrap();
            assert_eq!(message.status, *status);
            assert_eq!(message.msg_number, index as u32 + 1);
        }
        assert_eq!(package.read_message(statuses.len()).unwrap().msg_number, 101);
    }
}

const MYSTIC_CONFERENCE_COUNTS: &[(u16, usize)] = &[
    (2, 1),
    (10, 1),
    (12, 2),
    (15, 3),
    (21, 83),
    (24, 1),
    (25, 1),
    (26, 1),
    (27, 2),
    (32, 1),
    (46, 8),
    (48, 1),
    (50, 1),
    (51, 1),
    (58, 1),
    (87, 17),
    (147, 2),
    (165, 3),
    (166, 3),
    (167, 4),
    (168, 3),
    (187, 17),
    (190, 4),
    (193, 2),
    (198, 7),
    (209, 1),
    (212, 1),
    (253, 2),
    (261, 39),
    (287, 1),
    (311, 1),
    (353, 3),
    (381, 5),
    (391, 2),
    (392, 2),
    (395, 2),
];

fn mystic_conference_name(number: u16) -> String {
    match number {
        2 => "localz.generalzZz".into(),
        21 => "fsx.InterBBS Data".into(),
        261 => "tqw.General Tech News".into(),
        391 => "piNet.BBS Ads".into(),
        _ => format!("Area {number}"),
    }
}

#[test]
fn mystic_packet_keeps_messages_and_conferences_uncached_cold_and_warm() {
    // Reproduce BEERS24.1: lowercase members, 289 areas, 229 messages in 36
    // populated areas, missing screens, and Mystic's 0xFF active-message flag.
    let mut control = b"TEST BBS\r\n\r\n\r\nSysop\r\n0,TEST\r\n09/28/26,07:26\r\nUSER\r\n\r\n0\r\n229\r\n288\r\n".to_vec();
    let mut areas: Vec<u16> = MYSTIC_CONFERENCE_COUNTS
        .iter()
        .map(|(number, _)| *number)
        .filter(|number| *number != 391)
        .collect();
    areas.extend(1000..1253);
    areas.push(391);
    assert_eq!(areas.len(), 289);
    for number in areas {
        control.extend(format!("{number}\r\n{}\r\n", mystic_conference_name(number)).as_bytes());
    }
    control.extend(b"20welcome\r\n20news\r\n20goodbye\r\n");
    let mut messages = vec![b' '; 128];
    let mut number = 0;
    for (conference, count) in MYSTIC_CONFERENCE_COUNTS {
        for _ in 0..*count {
            number += 1;
            let mut entry = header(b' ', number, "09-28-2607:26", "ALL", "AUTHOR", "Mystic", 0, 2, *conference);
            entry[122] = 0xFF;
            messages.extend(entry);
            messages.extend([b' '; 128]);
        }
    }
    let dir = TempDir::new();
    let path = dir.path().join("MYSTIC.QWK");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("control.dat", options).unwrap();
    zip.write_all(&control).unwrap();
    zip.start_file("messages.dat", options).unwrap();
    zip.write_all(&messages).unwrap();
    zip.start_file("door.id", options).unwrap();
    zip.write_all(b"DOOR = Mystic BBS\r\nVERSION = 1.12 A49\r\n").unwrap();
    zip.finish().unwrap();

    let cache = crate::qwk::ExtractionCache::new(dir.path().join("cache"), 30);
    let uncached = QwkPackage::load_from_file(&path).unwrap();
    let cold = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    assert!(warm.cached_threads().is_some());
    let expected: Vec<_> = MYSTIC_CONFERENCE_COUNTS
        .iter()
        .map(|(number, count)| (*number, mystic_conference_name(*number), *count))
        .collect();
    for package in [&uncached, &cold, &warm] {
        assert_eq!(package.message_count(), 229);
        assert_eq!(package.control_file.conferences.len(), 289);
        assert_eq!(package.control_file.conferences.last().unwrap().name, "piNet.BBS Ads");
        assert_eq!(package.control_file.welcome_screen, "20welcome");
        assert_eq!(package.control_file.news_screen, "20news");
        assert_eq!(package.control_file.logoff_screen, "20goodbye");
        assert_eq!(package.conferences(), expected);
        assert_eq!(package.infos, uncached.infos);
        for index in 0..229 {
            assert_eq!(package.read_message(index).unwrap().active_flag, 0xFF);
        }
    }
}

#[test]
#[ignore = "requires ICY_MAIL_BEERS24_PACKET pointing to BEERS24.1.qwk"]
fn beers24_packet_keeps_all_populated_conferences() {
    let path = std::env::var_os("ICY_MAIL_BEERS24_PACKET").expect("set ICY_MAIL_BEERS24_PACKET to BEERS24.1.qwk");
    let dir = TempDir::new();
    let cache = crate::qwk::ExtractionCache::new(dir.path().join("cache"), 30);
    let uncached = QwkPackage::load_from_file(&path).unwrap();
    let cold = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    assert!(warm.cached_threads().is_some());
    for package in [&uncached, &cold, &warm] {
        assert_eq!(package.message_count(), 229);
        assert_eq!(package.control_file.conferences.len(), 289);
        let conferences = package.conferences();
        assert_eq!(conferences.len(), 36);
        assert_eq!(
            conferences.iter().map(|(number, _, count)| (*number, *count)).collect::<Vec<_>>(),
            MYSTIC_CONFERENCE_COUNTS
        );
        for number in [2, 21, 261, 391] {
            assert_eq!(
                conferences.iter().find(|(area, _, _)| *area == number).unwrap().1,
                mystic_conference_name(number)
            );
        }
        assert_eq!(package.infos, uncached.infos);
        for index in 0..229 {
            assert_eq!(package.read_message(index).unwrap().text, uncached.read_message(index).unwrap().text);
        }
    }
}

/// Screens and bulletins next to the messages; `DOOR.ID` and the index are no bulletins.
fn packet_files() -> &'static [(&'static str, &'static [u8])] {
    &[
        ("GOODBYE", b"Bye!\r\n"),
        ("BLT-0.10", b"Tenth bulletin\r\n"),
        ("HELLO", b"\x1b[1;33mWelcome\x1b[0m to TEST BBS\r\n\x1aSAUCE garbage"),
        ("NEWFILES.DAT", b"@X0EDEMO.ZIP@X07  A demo\r\nTOOL.ZIP  A tool\r\n"),
        ("BLT-0.2", b"Second bulletin\r\n"),
        ("NEWS", b"News of the day\r\n"),
        ("DOOR.ID", b"DOOR = Test\r\n"),
        ("1.NDX", &[0; 5]),
    ]
}

/// Writes a synthetic QWK packet and returns its path.
fn write_packet(dir: &std::path::Path) -> std::path::PathBuf {
    write_packet_with_newfiles(dir, None)
}

pub(crate) fn write_packet_with_newfiles(dir: &std::path::Path, newfiles: Option<&[u8]>) -> std::path::PathBuf {
    let mut messages = vec![b' '; 128]; // packet header block

    message(&mut messages, 10, "01/02/2010:00", "alice", "Coffee machine", 0, 1, 3);
    message(&mut messages, 11, "01-02-2011:00", "bob", "Re: Coffee machine", 10, 1, 1);
    message(&mut messages, 12, "01-03-2009:00", "carol", "Amiga demos", 0, 2, 5);
    message(&mut messages, 13, "01-04-2009:00", "dave", "Re: Amiga demos", 0, 2, 2);

    let path = dir.join("TEST.QWK");
    let file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("CONTROL.DAT", options).unwrap();
    zip.write_all(&control_dat()).unwrap();
    zip.start_file("MESSAGES.DAT", options).unwrap();
    zip.write_all(&messages).unwrap();
    for (name, data) in packet_files() {
        zip.start_file(*name, options).unwrap();
        zip.write_all(if *name == "NEWFILES.DAT" { newfiles.unwrap_or(data) } else { data }).unwrap();
    }
    zip.finish().unwrap();

    path
}

#[test]
fn capabilities_survive_cold_warm_and_cloned_packages() {
    use std::io::Read;
    let dir = TempDir::new();
    let original = write_packet(dir.path());
    let mut source = zip::ZipArchive::new(std::fs::File::open(original).unwrap()).unwrap();
    let path = dir.path().join("CAPABLE.QWK");
    let mut destination = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for index in 0..source.len() {
        let mut entry = source.by_index(index).unwrap();
        destination.start_file(entry.name(), zip::write::SimpleFileOptions::default()).unwrap();
        if entry.name() == "DOOR.ID" {
            destination
                .write_all(b"CONTROLNAME=CUSTOM\r\nCONTROLTYPE=ADD\r\nCONTROLTYPE=DROP\r\nCONTROLTYPE=QWKE\r\n")
                .unwrap();
        } else {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            destination.write_all(&bytes).unwrap();
        }
    }
    destination.finish().unwrap();
    let cache = crate::qwk::ExtractionCache::new(dir.path().join("cache"), 30);
    let cold = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    assert!(cold.capabilities.qwke);
    assert!(cold.capabilities.supports_subscriptions());
    assert_eq!(cold.capabilities.control_name.as_deref(), Some("CUSTOM"));
    assert_eq!(cold.clone().capabilities, cold.capabilities);
    let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    assert!(warm.cached_threads().is_some(), "metadata cache was used");
    assert_eq!(cold.capabilities, warm.capabilities);
}

#[test]
fn large_newfiles_list_is_discoverable_and_paged() {
    let dir = TempDir::new();
    let data = b"0123456789abcdef0123456789abcdef\n".repeat(252_000);
    let path = write_packet_with_newfiles(dir.path(), Some(&data));
    let package = QwkPackage::load_from_file(&path).unwrap();
    let file = package
        .files
        .iter()
        .find(|file| file.name == "NEWFILES.DAT")
        .expect("newfiles file in the packet");
    assert_eq!(file.data.len(), data.len());
    assert_eq!(file.pages(), 124);
}

#[must_use]
pub fn load() -> (TempDir, QwkPackage) {
    let dir = TempDir::new();
    let path = write_packet(dir.path());
    let package = QwkPackage::load_from_file(&path).unwrap();
    (dir, package)
}

/// Unique scratch directory that removes itself when the test ends.
pub struct TempDir(std::path::PathBuf);

impl TempDir {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("icy_mail_qwk_{}_{id}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn bulk_reads_match_cached_message_text() {
    let (_dir, package) = load();
    for index in 0..package.infos.len() {
        let bulk = package.read_message(index).unwrap();
        assert_eq!(bulk.text, package.get_message(index).unwrap().text);
        package.clear_cache();
    }
    assert!(package.read_message(package.infos.len()).is_err());
}

#[test]
fn index_covers_every_message() {
    let (_dir, package) = load();
    assert_eq!(package.infos.len(), 4);
    assert_eq!(package.infos.len(), package.descriptors.len());
}

#[test]
#[ignore = "requires ICY_MAIL_TEST_PACKET pointing to a packet with over 1 GiB of message data"]
fn large_packet_opens() {
    let path = std::env::var("ICY_MAIL_TEST_PACKET").expect("set ICY_MAIL_TEST_PACKET to a large QWK packet");
    let package = QwkPackage::load_from_file(&path).unwrap();
    let message_bytes: u64 = package.descriptors.iter().map(|descriptor| u64::from(descriptor.block_count) * 128).sum();
    assert!(
        message_bytes > 1024 * 1024 * 1024,
        "the test packet must exceed the archive reader's default 1 GiB entry limit"
    );
    assert_eq!(package.infos.len(), package.descriptors.len());
    assert!(package.infos.iter().enumerate().all(|(index, info)| info.index == index));
    package.read_message(0).unwrap();
    package.read_message(package.infos.len() - 1).unwrap();
}

#[test]
#[ignore = "requires ICY_MAIL_TEST_PACKET and disk space for its decompressed data"]
fn large_packet_cache_roundtrip() {
    let path = std::env::var("ICY_MAIL_TEST_PACKET").expect("set ICY_MAIL_TEST_PACKET to a large QWK packet");
    let directory = TempDir::new();
    let cache = crate::qwk::ExtractionCache::new(directory.path().join("cache"), 30);
    let start = std::time::Instant::now();
    let cold = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    println!("cold extraction and cache write: {:?}", start.elapsed());
    let count = cold.infos.len();
    println!("indexed {count} messages");
    assert!(count > 0);
    let uncached = QwkPackage::load_from_file(&path).unwrap();
    assert_eq!(cold.infos, uncached.infos);
    let first = cold.read_message(0).unwrap();
    let last = cold.read_message(count - 1).unwrap();
    let subjects = [cold.infos[0].subject.as_str().to_owned(), cold.infos[count - 1].subject.as_str().to_owned()];
    drop(cold);
    for run in 0..3 {
        let start = std::time::Instant::now();
        let warm = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        println!("warm cache load {run}: {:?}", start.elapsed());
        assert_eq!(warm.infos.len(), count);
        assert!(warm.infos.iter().enumerate().all(|(index, info)| info.index == index));
        assert_eq!(warm.read_message(0).unwrap().text, first.text);
        assert_eq!(warm.read_message(count - 1).unwrap().text, last.text);
        assert_eq!(warm.infos[0].subject.as_str(), subjects[0]);
        assert_eq!(warm.infos[count - 1].subject.as_str(), subjects[1]);
        for index in 0..count {
            assert_eq!(warm.read_message(index).unwrap().text, uncached.read_message(index).unwrap().text);
        }
    }
}

#[test]
fn index_extracts_header_fields() {
    let (_dir, package) = load();
    let first = &package.infos[0];
    assert_eq!(first.number, 10);
    assert_eq!(first.from, "alice");
    assert_eq!(first.subject, "Coffee machine");
    assert_eq!(first.conference, 1);
    assert_eq!(first.lines, 3);
    assert_eq!(first.date_str, "2020-01-02 10:00");
}

#[test]
fn index_extracts_extended_subjects_for_the_message_list() {
    let subject = "A complete subject that is much longer than twenty-five characters";
    let mut messages = vec![b' '; 128];
    message(&mut messages, 14, "01-05-2600:00", "erin", subject, 0, 1, 1);

    let dir = TempDir::new();
    let path = dir.path().join("LONG.QWK");
    let file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("CONTROL.DAT", options).unwrap();
    zip.write_all(&control_dat()).unwrap();
    zip.start_file("MESSAGES.DAT", options).unwrap();
    zip.write_all(&messages).unwrap();
    zip.finish().unwrap();

    let package = QwkPackage::load_from_file(&path).unwrap();
    assert_eq!(package.infos.len(), 1);
    assert_eq!(package.infos[0].subject, subject);
    assert_eq!(package.infos[0].subject_key, subject.to_ascii_lowercase());
}

#[test]
fn message_bodies_keep_hidden_lines_until_they_are_read() {
    let mut body = b"\x01PID: Other 1\xE3\x01MSGID: 1:2/3 4\xE3Hello\xE3\x01hBright\xE3".to_vec();
    body.resize(128, b' ');
    let mut messages = vec![b' '; 128];
    messages.extend(header(b' ', 1, "01-05-2600:00", "ALL", "erin", "Kludges", 0, 2, 1));
    messages.extend(body);

    let dir = TempDir::new();
    let path = dir.path().join("KLUDGE.QWK");
    let file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("CONTROL.DAT", options).unwrap();
    zip.write_all(&control_dat()).unwrap();
    zip.start_file("MESSAGES.DAT", options).unwrap();
    zip.write_all(&messages).unwrap();
    zip.finish().unwrap();

    let package = QwkPackage::load_from_file(&path).unwrap();
    let text = package.get_message(0).unwrap().text;
    assert!(text.starts_with(b"\x01PID: Other 1\n"), "the stored message keeps its hidden lines");
    assert!(package.readable_text(&text, false).starts_with(b"Hello\n "), "every Ctrl-A line is hidden");
    assert!(package.readable_text(&text, true).starts_with(b"\x1b[0;36mPID: Other 1\x1b[0m\n"));
    let mut synchronet = package.clone();
    synchronet.capabilities.synchronet = true;
    assert_eq!(synchronet.readable_text(&text, false).as_ref(), text.as_slice());
}

pub(crate) fn load_utf8() -> (TempDir, QwkPackage) {
    let dir = TempDir::new();
    let path = dir.path().join("UTF8.QWK");
    let mut messages = vec![b' '; 128];
    for (number, body) in [
        (1, "\x01CHRS: UTF-8 4\n\x1b[31mGrüße Привет あ\x1b[0m\n... Grüße\n".as_bytes()),
        (2, b"\xc3\xa9\n".as_slice()),
    ] {
        let mut body: Vec<u8> = body.iter().map(|&byte| if byte == b'\n' { 0xe3 } else { byte }).collect();
        let blocks = body.len().div_ceil(128);
        body.resize(blocks * 128, b' ');
        messages.extend(header(b' ', number, "01-05-2600:00", "ALL", "erin", "Encoding", 0, blocks as u32 + 1, 1));
        messages.extend(body);
    }
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("CONTROL.DAT", options).unwrap();
    zip.write_all(&control_dat()).unwrap();
    zip.start_file("MESSAGES.DAT", options).unwrap();
    zip.write_all(&messages).unwrap();
    zip.finish().unwrap();
    let package = QwkPackage::load_from_file(&path).unwrap();
    (dir, package)
}

/// A Synchronet packet: `HEADERS.DAT` marks the first message as UTF-8 with LF line ends and
/// whole header fields; the second has whole CP437 fields without `Utf8`.
pub(crate) fn load_synchronet() -> (TempDir, QwkPackage) {
    let dir = TempDir::new();
    let path = dir.path().join("SYNC.QWK");
    let mut messages = vec![b' '; 128];
    let mut headers = Vec::new();
    for (number, from, subject, body, utf8) in [
        (1, "Dmitry", "Name in", "Привет あい\nThe quick brown fox\n".as_bytes(), true),
        (2, "erin", "Plain", b"Caf\x82\xe3".as_slice(), false),
    ] {
        headers.extend_from_slice(format!("[{:x}]\r\n", messages.len()).as_bytes());
        if utf8 {
            headers.extend_from_slice("Utf8: true\r\nSender: Дмитрий Липатников\r\nSubject: Имя на кириллице\r\n".as_bytes());
        } else {
            headers.extend_from_slice(b"Sender: Erin Andr\x82 with a longer sender name\r\nSubject: Caf\x82 and the complete CP437 subject\r\n");
        }
        headers.extend_from_slice(b"\r\n");
        let mut body = body.to_vec();
        let blocks = body.len().div_ceil(128);
        body.resize(blocks * 128, b' ');
        messages.extend(header(b' ', number, "01-05-2600:00", "USER", from, subject, 0, blocks as u32 + 1, 1));
        messages.extend(body);
    }
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    for (name, data) in [("CONTROL.DAT", control_dat()), ("MESSAGES.DAT", messages), ("HEADERS.DAT", headers)] {
        zip.start_file(name, options).unwrap();
        zip.write_all(&data).unwrap();
    }
    zip.finish().unwrap();
    let package = QwkPackage::load_from_file(&path).unwrap();
    (dir, package)
}

pub(crate) fn load_legacy_charsets() -> (TempDir, QwkPackage) {
    let dir = TempDir::new();
    let path = dir.path().join("CHARSETS.QWK");
    let mut messages = vec![b' '; 128];
    for (number, from, body) in [
        (
            1,
            b"\x8f\xe0\xa8\xa2\xa5\xe2".as_slice(),
            b"\x01CHRS: CP866 2\xe3\x1b[31m\x8f\xe0\xa8\xa2\xa5\xe2\x1b[0m\xe3".as_slice(),
        ),
        (2, b"erin", b"\x01CHRS: GERMAN 1\xe3\x1b[31m[\\]\x1b[0m\xe3"),
    ] {
        let mut header = header(b' ', number, "01-05-2600:00", "ALL", "", "Encoding", 0, 2, 1);
        header[46..46 + from.len()].copy_from_slice(from);
        messages.extend(header);
        let mut padded = body.to_vec();
        padded.resize(128, b' ');
        messages.extend(padded);
    }
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("CONTROL.DAT", options).unwrap();
    zip.write_all(&control_dat()).unwrap();
    zip.start_file("MESSAGES.DAT", options).unwrap();
    zip.write_all(&messages).unwrap();
    zip.finish().unwrap();
    let package = QwkPackage::load_from_file(&path).unwrap();
    (dir, package)
}

#[test]
fn declared_legacy_encodings_decode_index_headers() {
    let (_dir, package) = load_legacy_charsets();
    assert_eq!(package.infos[0].from, "Привет");
    assert_eq!(package.infos[1].from, "erin");
}

#[test]
fn reply_keeps_ref_number_and_shares_the_subject_key() {
    let (_dir, package) = load();
    let reply = &package.infos[1];
    assert_eq!(reply.ref_number, 10);
    assert_eq!(reply.subject_key, package.infos[0].subject_key);
}

#[test]
fn conferences_report_only_populated_areas_with_counts() {
    let (_dir, package) = load();
    assert_eq!(package.conferences(), vec![(1, "General".to_string(), 2), (2, "Retro".to_string(), 2)]);
}

#[test]
fn message_bodies_are_lazily_readable() {
    let (_dir, package) = load();
    let body = package.get_message(2).unwrap();
    assert_eq!(body.from, "carol");
    // 0xE3 line separators are translated to LF by the parser.
    assert!(body.text.contains(&b'\n'));
    assert!(!body.text.contains(&0xE3));
}

#[test]
fn threading_groups_replies_under_their_root() {
    let (_dir, package) = load();
    let retro: Vec<&crate::qwk::MessageInfo> = package.infos.iter().filter(|info| info.conference == 2).collect();

    // #13 has no ref number, so it must attach via the normalized subject.
    let rows = crate::threading::build_threads(&retro);
    assert_eq!(rows.iter().map(|r| r.depth).collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(package.infos[rows[0].index].number, 12);
    assert_eq!(package.infos[rows[1].index].number, 13);
}

#[test]
fn packets_open_from_other_archive_formats() {
    for name in ["TEST_ARJ.QWK", "TEST_7Z.QWK"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/qwk/test_data").join(name);
        let package = QwkPackage::load_from_file(&path).unwrap_or_else(|error| panic!("{name}: {error}"));
        let (_dir, zip) = load();
        assert_eq!(package.infos.len(), 4, "{name}");
        assert_eq!(package.bbs_name, zip.bbs_name, "{name}");
        assert_eq!(package.infos[0].from, "alice", "{name}");
        assert_eq!(package.conferences(), zip.conferences(), "{name}");
    }
}

#[test]
fn unknown_archives_are_rejected() {
    let dir = TempDir::new();
    let path = dir.path().join("BROKEN.QWK");
    std::fs::write(&path, b"this is not an archive").unwrap();
    assert!(QwkPackage::load_from_file(&path).is_err());
}

#[test]
fn screens_bulletins_and_new_files_are_listed_in_packet_order() {
    use crate::qwk::PacketFileKind::*;
    let (_dir, package) = load();
    let files: Vec<_> = package.files.iter().map(|file| (file.name.as_str(), file.kind)).collect();
    assert_eq!(
        files,
        [
            ("HELLO", Welcome),
            ("NEWS", News),
            ("BLT-0.2", Bulletin),
            ("BLT-0.10", Bulletin),
            ("NEWFILES.DAT", NewFiles),
            ("GOODBYE", Goodbye)
        ]
    );
    assert_eq!(package.files[0].lines(), 1, "text after the end-of-file mark does not count");
}

#[test]
fn screen_names_match_by_prefix_when_no_file_has_the_exact_name() {
    use crate::qwk::PacketFileKind::*;
    let (_dir, package) = load();
    let mut control = package.control_file.clone();
    control.welcome_screen = "WELCOME".into();
    control.news_screen = "".into();
    let files = vec![
        ("WELCOMEG".to_string(), b"ansi".to_vec()),
        ("NFILES.TXT".to_string(), b"x".to_vec()),
        ("README.TXT".to_string(), b"x".to_vec()),
        ("NEWS".to_string(), b"x".to_vec()),
    ];
    let files: Vec<_> = crate::qwk::packet_files(&control, files)
        .into_iter()
        .map(|file| (file.name, file.kind))
        .collect();
    assert_eq!(files, [("WELCOMEG".to_string(), Welcome), ("NFILES.TXT".to_string(), NewFiles)]);
}
