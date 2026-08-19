use std::io::Write;

use crate::qwk::QwkPackage;

/// Builds a 128-byte QWK message header.
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

/// Writes a synthetic QWK packet and returns its path.
fn write_packet(dir: &std::path::Path) -> std::path::PathBuf {
    let mut messages = vec![b' '; 128]; // packet header block

    message(&mut messages, 10, "01-02-2010:00", "alice", "Coffee machine", 0, 1, 3);
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
    zip.finish().unwrap();

    path
}

pub fn load() -> (TempDir, QwkPackage) {
    let dir = TempDir::new();
    let path = write_packet(dir.path());
    let package = QwkPackage::load_from_file(&path).unwrap();
    (dir, package)
}

/// Unique scratch directory that removes itself when the test ends.
pub struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("icy_mail_qwk_{}_{id}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
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
    let rows = crate::ui::threading::build_threads(&retro);
    assert_eq!(rows.iter().map(|r| r.depth).collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(package.infos[rows[0].index].number, 12);
    assert_eq!(package.infos[rows[1].index].number, 13);
}
