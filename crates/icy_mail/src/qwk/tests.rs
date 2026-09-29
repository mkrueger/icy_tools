use std::io::Write;

use crate::qwk::QwkPackage;

/// Builds a 128-byte QWK message header.
#[allow(clippy::too_many_arguments)] // each argument maps directly to a fixed QWK header field
fn header(status: u8, number: u32, date_time: &str, to: &str, from: &str, subject: &str, ref_number: u32, blocks: u32, conference: u16) -> Vec<u8> {
    fn field(value: &str, len: usize) -> Vec<u8> {
        let mut bytes = value.as_bytes().to_vec();
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

fn control_dat() -> Vec<u8> {
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
        let path = std::env::temp_dir().join(format!("icy_mail_qwk_{}_{id}", std::process::id()));
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
fn screens_bulletins_and_new_files_are_listed_like_multimail() {
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
