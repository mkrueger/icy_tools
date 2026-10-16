use std::path::Path;

use bstr::BString;
use jamjam::jam::attributes;
use jamjam::jam::msg_header::{JamMessageHeader, MessageSubfield, SubfieldType};
use jamjam::jam::{JamMessage, JamMessageBase};

use super::*;
use crate::drafts::{Compose, DraftStore};
use crate::qwk::{tests::TempDir, ExtractionCache, PacketFormat, QwkPackage};

fn field(kind: SubfieldType, value: &str) -> MessageSubfield {
    MessageSubfield::new(kind, BString::from(value))
}

/// Writes a FidoNet-style message: CR line ends, local date, kludges in subfields.
fn write_fido(base: &mut JamMessageBase, from: &str, subject: &str, text: &str, reply_to: u32, extra: Vec<MessageSubfield>) -> u32 {
    let mut header = JamMessageHeader::default();
    header.sub_fields.push(field(SubfieldType::SenderName, from));
    header.sub_fields.push(field(SubfieldType::RecvName, "All"));
    header.sub_fields.push(field(SubfieldType::Subject, subject));
    header.sub_fields.extend(extra);
    header.reply_to = reply_to;
    header.date_written = 1_700_000_000;
    header.attributes = attributes::MSG_TYPEECHO;
    base.write_message(&JamMessage::from_stored(header, BString::from(text))).unwrap()
}

fn fido_base(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("fidotest");
    let mut base = JamMessageBase::create(&path).unwrap();
    write_fido(
        &mut base,
        "Alice",
        "Hello",
        "First line\rSecond line\r",
        0,
        vec![
            field(SubfieldType::MsgID, "2:240/1 0001abcd"),
            field(SubfieldType::FTSKludge, "CHRS: CP437 2"),
            field(SubfieldType::SeenBy2D, "240/1"),
        ],
    );
    write_fido(
        &mut base,
        "Bob",
        "Re: Hello",
        "A reply\r",
        1,
        vec![field(SubfieldType::MsgID, "2:240/2 0002abcd"), field(SubfieldType::ReplyID, "2:240/1 0001abcd")],
    );
    path.with_extension("jhr")
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, text).unwrap();
}

/// An IcyBoard installation with an echo conference (one area with messages, one without a base
/// yet), a read-only conference and the email base.
fn icyboard(dir: &Path) -> std::path::PathBuf {
    write(
        &dir.join("icboard.toml"),
        r#"[board]
name = "Icy Test BBS"

[sysop]
name = "SYSOP"

[qwk_settings]
bbs_id = "ICYTEST"

[paths]
conferences = "main/conferences.toml"
email_msgbase = "main/email"
user_file = "main/users.toml"
ftn_file = "main/ftn.toml"
"#,
    );
    write(&dir.join("main/users.toml"), "[[users]]\nname = \"Caller\"\n\n[[users]]\nname = \"Sysop\"\n");
    write(&dir.join("main/ftn.toml"), "netmail = \"ftn/netmail\"\n");
    write(
        &dir.join("main/conferences.toml"),
        r#"[[conference]]
name = "Main Board"
echo_mail_in_conference = true
area_file = "conferences/main/area.toml"

[[conference]]
name = "News"
is_read_only = true
area_file = "conferences/news/area.toml"

[[conference]]
name = "No Areas"
"#,
    );
    write(
        &dir.join("conferences/main/area.toml"),
        r#"[[area]]
name = "General"
path = "conferences/main/messages/general"

[[area]]
name = "Empty"
path = "conferences/main/messages/empty"
"#,
    );
    write(
        &dir.join("conferences/news/area.toml"),
        r#"[[area]]
name = "News"
path = "conferences/news/messages/news"
"#,
    );
    for messages in ["conferences/main/messages", "conferences/news/messages"] {
        fs::create_dir_all(dir.join(messages)).unwrap();
    }
    let mut general = JamMessageBase::create(dir.join("conferences/main/messages/general")).unwrap();
    general
        .write_message(
            &JamMessage::default()
                .with_from("Caller".into())
                .with_to("SYSOP".into())
                .with_subject("Hi Sysop".into())
                .with_date_time(chrono::Utc::now())
                .with_attributes(attributes::MSG_LOCAL | attributes::MSG_PRIVATE)
                .with_text("Line one\nLine two".into()),
        )
        .unwrap();
    // IcyBoard's tosser converts packets to UTF-8 but keeps their CHRS line.
    let mut news = JamMessageBase::create(dir.join("conferences/news/messages/news")).unwrap();
    news.write_message(
        &JamMessage::default()
            .with_from("Jürgen".into())
            .with_to("All".into())
            .with_subject("Grüße".into())
            .with_date_time(chrono::Utc::now())
            .with_attributes(attributes::MSG_TYPEECHO)
            .with_sub_field(field(SubfieldType::FTSKludge, "CHRS: CP437 2"))
            .with_text("Schöne Grüße\n".into()),
    )
    .unwrap();
    JamMessageBase::create(dir.join("main/email")).unwrap();
    dir.join("icboard.toml")
}

fn stored(path: &Path, number: u32) -> JamMessage {
    let mut base = JamMessageBase::open(path).unwrap();
    base.read_transaction(|base| base.read_message(number)).unwrap()
}

fn subfield_text(message: &JamMessage, kind: SubfieldType) -> Option<String> {
    subfield(message.header(), kind).map(ToString::to_string)
}

#[test]
fn recognizes_message_base_paths() {
    assert!(is_message_base(Path::new("/bbs/general.JHR")));
    assert!(is_message_base(Path::new("/bbs/icboard.toml")));
    assert!(!is_message_base(Path::new("/bbs/BBS.QWK")));
    assert!(!is_message_base(Path::new("/bbs/general.jdt")));
}

#[test]
fn single_base_indexes_headers_and_shows_kludges_as_hidden_lines() {
    let dir = TempDir::new();
    let path = fido_base(dir.path());
    let base = MessageBase::open(&path).unwrap();
    assert_eq!(base.kind, BaseKind::Single);
    assert_eq!(base.bbs_id, "FIDOTEST");
    assert_eq!(base.areas.len(), 1);
    assert!(base.areas[0].echo, "messages flagged as echomail make the area an echo area");
    assert_eq!(base.messages.len(), 2);

    let first = &base.messages[0];
    assert_eq!((first.number, first.reply_to, first.utf8), (1, 0, false));
    assert_eq!(first.from, b"Alice");
    assert_eq!(first.subject, b"Hello");
    assert_eq!(first.charset, Some(crate::charset::Charset::Cp437));
    assert_eq!(first.date, chrono::DateTime::from_timestamp(1_700_000_000, 0).map(|date| date.naive_utc()));
    assert_eq!(base.messages[1].reply_to, 1);

    let message = base.read(0).unwrap();
    assert_eq!(
        message.text.to_string(),
        "\x01MSGID: 2:240/1 0001abcd\n\x01CHRS: CP437 2\nFirst line\nSecond line\n\x01SEEN-BY: 240/1\n"
    );
    assert_eq!(crate::text::hidden_lines(&message.text, false).as_ref(), b"First line\nSecond line\n");
}

#[test]
fn posting_to_a_single_base_follows_the_jam_specification() {
    let dir = TempDir::new();
    let path = fido_base(dir.path());
    let base = MessageBase::open(&path).unwrap();
    let number = base
        .post(&Post {
            area: 0,
            from: b"Carol".to_vec(),
            to: b"Bob".to_vec(),
            subject: b"Re: Hello".to_vec(),
            body: b"Quoted\nAnswer".to_vec(),
            private: false,
            reply_to: 2,
            utf8: false,
            address: None,
        })
        .unwrap();
    assert_eq!(number, 3);

    let message = stored(&path.with_extension(""), number);
    assert_eq!(message.text().to_string(), "Quoted\rAnswer\r");
    assert_eq!(message.header().reply_to, 2);
    assert_eq!(subfield_text(&message, SubfieldType::ReplyID).as_deref(), Some("2:240/2 0002abcd"));
    assert_eq!(message.header().reply_crc, JamMessageBase::crc(&BString::from("2:240/2 0002abcd")));
    assert_eq!(subfield_text(&message, SubfieldType::FTSKludge).as_deref(), Some("CHRS: CP437 2"));
    assert!(subfield_text(&message, SubfieldType::PID).is_some_and(|pid| pid.starts_with("IcyMail ")));
    assert!(
        subfield_text(&message, SubfieldType::DateWritten).is_none(),
        "JAM dates are local time without IcyBoard's subfield"
    );
    assert_eq!(
        message.header().attributes,
        attributes::MSG_LOCAL | attributes::MSG_TYPEECHO,
        "an echo area's posts go out with the tosser"
    );
}

#[test]
fn icyboard_installation_lists_every_area() {
    let dir = TempDir::new();
    let path = icyboard(dir.path());
    let base = MessageBase::open(&path).unwrap();
    assert_eq!(base.kind, BaseKind::IcyBoard);
    assert_eq!(base.name, "Icy Test BBS");
    assert_eq!(base.bbs_id, "ICYTEST");
    assert_eq!(base.user_name, "SYSOP");
    let areas: Vec<_> = base
        .areas
        .iter()
        .map(|area| (area.number, area.name.as_str(), area.read_only, area.echo))
        .collect();
    assert_eq!(
        areas,
        [
            (0, "Main Board / General", false, true),
            (1, "Main Board / Empty", false, true),
            (2, "News", true, false),
            (3, "Email", false, false),
            (4, "Netmail", false, false),
        ]
    );
    assert_eq!(base.messages.len(), 2);
    let message = &base.messages[0];
    assert!(message.private);
    assert_eq!(message.to, b"SYSOP");
    assert!(message.date.is_some());
    assert_eq!(base.read(0).unwrap().text.to_string(), "Line one\nLine two");
    assert!(base
        .messages
        .iter()
        .all(|message| message.utf8 && message.charset == Some(crate::charset::Charset::Utf8)));
}

#[test]
fn icyboard_text_is_utf8_whatever_tossed_kludges_say() {
    let dir = TempDir::new();
    let path = icyboard(dir.path());
    let package = QwkPackage::load_from_file(&path).unwrap();
    let tossed = package.infos.iter().find(|info| info.conference == 2).unwrap();
    assert_eq!((tossed.from.trim(), tossed.subject.trim()), ("Jürgen", "Grüße"));
    assert!(package.declares_utf8(tossed.index));
    let message = package.get_message(tossed.index).unwrap();
    assert_eq!(message.text.to_string(), "\x01CHRS: CP437 2\nSchöne Grüße\n");
    let (charset, source) = crate::charset::ReadingEncodings::default()
        .options(&package, tossed.index)
        .resolve(&message.text);
    assert_eq!((charset, source), (crate::charset::Charset::Utf8, crate::charset::CharsetSource::Declared));

    // Alone, an area base of the installation is still read as UTF-8.
    let news = MessageBase::open(&dir.path().join("conferences/news/messages/news.jhr")).unwrap();
    assert!(news.declares_utf8(0));
}

#[test]
fn posting_to_icyboard_writes_like_icyboard_and_creates_missing_bases() {
    let dir = TempDir::new();
    let path = icyboard(dir.path());
    let base = MessageBase::open(&path).unwrap();
    let post = Post {
        area: 1,
        from: b"SYSOP".to_vec(),
        to: "Jürgen".as_bytes().to_vec(),
        subject: b"Welcome".to_vec(),
        body: "Grüße\nfrom the sysop".as_bytes().to_vec(),
        private: true,
        reply_to: 0,
        utf8: true,
        address: None,
    };
    assert_eq!(base.post(&post).unwrap(), 1);
    let empty = dir.path().join("conferences/main/messages/empty");
    let message = stored(&empty, 1);
    assert_eq!(message.text().to_string(), "Grüße\nfrom the sysop");
    assert_eq!(subfield_text(&message, SubfieldType::FTSKludge).as_deref(), Some("CHRS: UTF-8 4"));
    assert!(subfield_text(&message, SubfieldType::DateWritten).is_some());
    assert_eq!(
        message.header().attributes,
        attributes::MSG_LOCAL | attributes::MSG_PRIVATE | attributes::MSG_TYPEECHO
    );

    // CP437 drafts are converted, IcyBoard reads only UTF-8.
    let cp437 = Post {
        area: 0,
        to: b"J\x81rgen".to_vec(),
        body: b"Gr\x81\xE1e \x1b[1mbold\x1b[0m".to_vec(),
        utf8: false,
        ..post.clone()
    };
    assert_eq!(base.post(&cp437).unwrap(), 2);
    let message = stored(&dir.path().join("conferences/main/messages/general"), 2);
    assert_eq!(message.text().to_string(), "Grüße \x1b[1mbold\x1b[0m");
    assert_eq!(message.header().to().map(ToString::to_string).as_deref(), Some("Jürgen"));
    assert_eq!(subfield_text(&message, SubfieldType::FTSKludge).as_deref(), Some("CHRS: UTF-8 4"));

    let read_only = Post { area: 2, ..post.clone() };
    assert!(base.post(&read_only).is_err());
    let email = Post { area: 3, ..post };
    base.post(&email).unwrap();
    assert_eq!(
        stored(&dir.path().join("main/email"), 1).header().attributes & attributes::MSG_TYPELOCAL,
        attributes::MSG_TYPELOCAL
    );
}

#[test]
fn package_reads_message_bases_in_place() {
    let dir = TempDir::new();
    let path = fido_base(dir.path());
    let cache = ExtractionCache::new(dir.path().join("cache"), 7);
    let package = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
    assert_eq!(package.format(), PacketFormat::Jam);
    assert!(!dir.path().join("cache").exists(), "message bases are not extracted");
    assert_eq!(package.message_count(), 2);
    assert_eq!(package.infos[1].ref_number, 1);
    assert_eq!(package.infos[1].from.to_string(), "Bob");
    assert_eq!(package.infos[0].date_str, "2023-11-14 22:13");
    assert_eq!(package.conferences(), [(0, "fidotest".to_owned(), 2)]);
    let message = package.get_message(1).unwrap();
    assert_eq!((message.msg_number, message.ref_msg_number), (2, 1));
    assert_eq!(package.readable_text(&message.text, false).as_ref(), b"A reply\n");
}

#[test]
fn composed_messages_are_written_straight_into_the_base() {
    let dir = TempDir::new();
    let path = icyboard(dir.path());
    let package = QwkPackage::load_from_file(&path).unwrap();
    let drafts_dir = dir.path().join("drafts");
    let store = DraftStore::open_in(&path, &package, &drafts_dir).unwrap();
    assert!(store.is_message_base());
    assert!(store.supports_utf8());
    assert!(!store.reserves_e3());
    assert!(store.can_post(0) && !store.can_post(2));

    let mut reply = store.prepare(&package, Compose::Reply { index: 0 }).unwrap();
    assert_eq!((reply.to.as_str(), reply.from.as_str(), reply.ref_number), ("Caller", "SYSOP", 1));
    assert!(reply.private, "a reply to a private message stays private");
    reply.body = "Thanks for calling".into();
    assert_eq!(store.post_draft(&reply).unwrap(), (0, 2));

    let mut news = store.prepare(&package, Compose::New { conference: 2 }).unwrap();
    news.to = "All".into();
    news.subject = "News".into();
    news.body = "Read-only".into();
    assert!(!store.issues(&news).is_empty(), "read-only areas are reported before writing");
    assert!(store.post_draft(&news).is_err());
    let mut empty = store.prepare(&package, Compose::New { conference: 0 }).unwrap();
    empty.to = "All".into();
    empty.subject = "Nothing".into();
    assert!(store.post_draft(&empty).is_err(), "a message needs text");
    assert!(store.export(&dir.path().join("REPLY.REP")).is_err(), "message bases have nothing to export");

    let reloaded = QwkPackage::load_from_file(&path).unwrap();
    assert_eq!(reloaded.message_count(), 3);
    let written = reloaded.infos.iter().find(|info| info.subject.trim() == "Re: Hi Sysop").unwrap();
    assert_eq!((written.conference, written.number, written.ref_number), (0, 2, 1));
    assert!(written.private);
    assert!(
        DraftStore::open_in(&path, &reloaded, &drafts_dir).unwrap().drafts().is_empty(),
        "nothing is kept as a draft"
    );
}

#[test]
fn single_bases_default_to_the_enclosing_sysop_or_the_local_author() {
    let dir = TempDir::new();
    icyboard(dir.path());
    let general = dir.path().join("conferences/main/messages/general.jhr");
    assert_eq!(MessageBase::open(&general).unwrap().user_name, "SYSOP");

    let elsewhere = TempDir::new();
    let path = fido_base(elsewhere.path());
    assert_eq!(MessageBase::open(&path).unwrap().user_name, "", "received echomail does not name the user");
    let mut base = JamMessageBase::open(path.with_extension("")).unwrap();
    let mut header = JamMessageHeader::default();
    header.sub_fields.push(field(SubfieldType::SenderName, "Point Operator"));
    header.attributes = attributes::MSG_LOCAL;
    base.write_message(&JamMessage::from_stored(header, BString::from("Mine\r"))).unwrap();
    drop(base);
    assert_eq!(MessageBase::open(&path).unwrap().user_name, "Point Operator");
}

#[test]
fn byte_order_marks_declare_utf8() {
    let dir = TempDir::new();
    let path = dir.path().join("utf8");
    let mut base = JamMessageBase::create(&path).unwrap();
    for text in ["\u{feff}Grüße\nfrom IcyBoard", "\u{feff}Kept\r"] {
        base.write_message(&JamMessage::default().with_from("Jürgen".into()).with_text(text.into()))
            .unwrap();
    }
    let mut declared = JamMessage::default().with_text("\u{feff}Declared".into());
    declared = declared.with_sub_field(field(SubfieldType::FTSKludge, "CHRS: LATIN-1 2"));
    base.write_message(&declared).unwrap();
    drop(base);

    let base = MessageBase::open(&path.with_extension("jhr")).unwrap();
    assert_eq!(base.read(0).unwrap().text.to_string(), "\x01CHRS: UTF-8 4\nGrüße\nfrom IcyBoard");
    assert_eq!(base.read(1).unwrap().text.to_string(), "\x01CHRS: UTF-8 4\nKept\n");
    assert_eq!(
        base.read(2).unwrap().text.to_string(),
        "\x01CHRS: LATIN-1 2\nDeclared",
        "a declared charset wins"
    );
    let package = QwkPackage::load_from_file(path.with_extension("jhr")).unwrap();
    assert_eq!(package.infos[0].from.trim(), "Jürgen");
}

#[test]
fn dotted_base_names_from_echo_tags_are_kept() {
    let dir = TempDir::new();
    let path = dir.path().join("r50.sysop.jhr");
    let mut base = JamMessageBase::create(&path).unwrap();
    write_fido(&mut base, "Alice", "Dots", "Echo tag names\r", 0, Vec::new());
    drop(base);
    assert!(dir.path().join("r50.sysop.jdt").exists());

    let base = MessageBase::open(&path).unwrap();
    assert_eq!((base.name.as_str(), base.bbs_id.as_str()), ("r50.sysop", "R50SYSOP"));
    assert_eq!(base.read(0).unwrap().text.to_string(), "Echo tag names\n");
    let post = Post {
        area: 0,
        from: b"Bob".to_vec(),
        to: b"Alice".to_vec(),
        subject: b"Re: Dots".to_vec(),
        body: b"Still here".to_vec(),
        private: false,
        reply_to: 1,
        utf8: false,
        address: None,
    };
    assert_eq!(base.post(&post).unwrap(), 2);
    assert!(!dir.path().join("r50.jhr").exists());
    assert_eq!(MessageBase::open(&path).unwrap().messages.len(), 2);
}

#[test]
fn replies_moved_to_another_area_are_not_linked() {
    let dir = TempDir::new();
    let path = icyboard(dir.path());
    let package = QwkPackage::load_from_file(&path).unwrap();
    let store = DraftStore::open_in(&path, &package, &dir.path().join("drafts")).unwrap();
    let mut reply = store.prepare(&package, Compose::Reply { index: 0 }).unwrap();
    assert_eq!((reply.ref_number, reply.ref_conference), (1, Some(0)));
    reply.conference = 3;
    reply.body = "Answered by email".into();
    assert_eq!(store.post_draft(&reply).unwrap(), (3, 1));
    let message = stored(&dir.path().join("main/email"), 1);
    assert_eq!(message.header().reply_to, 0, "message 1 of the email base is another message");
}

#[test]
fn deleted_messages_stay_restorable_until_the_base_is_packed() {
    let dir = TempDir::new();
    let path = fido_base(dir.path());
    let base = MessageBase::open(&path).unwrap();
    assert!(base.can_change(0));
    base.set_deleted(0, true).unwrap();
    assert!(base.is_deleted(0));
    assert_eq!(
        base.read(0).unwrap().text.to_string().lines().nth(2),
        Some("First line"),
        "deleted text stays readable"
    );

    let reopened = MessageBase::open(&path).unwrap();
    assert_eq!(reopened.messages.len(), 2, "deleted messages are listed until a pack");
    assert!(reopened.is_deleted(0) && !reopened.is_deleted(1));
    assert_eq!(reopened.deleted_count(), 1);
    assert_eq!(reopened.messages[0].from, b"Alice");
    reopened.set_deleted(0, false).unwrap();
    assert!(!MessageBase::open(&path).unwrap().is_deleted(0));

    reopened.set_deleted(1, true).unwrap();
    let summary = reopened.pack(PackChoice::default()).unwrap();
    assert_eq!(summary, PackSummary { removed: 1, areas: 1 });
    let packed = MessageBase::open(&path).unwrap();
    assert_eq!(packed.messages.len(), 1);
    assert_eq!((packed.messages[0].number, packed.deleted_count()), (1, 0));
    // Numbers stay, so the next message continues after the removed one.
    let post = Post {
        area: 0,
        from: b"Carol".to_vec(),
        to: b"All".to_vec(),
        subject: b"After the pack".to_vec(),
        body: b"Still counting".to_vec(),
        private: false,
        reply_to: 0,
        utf8: false,
        address: None,
    };
    assert_eq!(packed.post(&post).unwrap(), 3);
}

#[test]
fn read_only_areas_keep_their_delete_marks() {
    let dir = TempDir::new();
    let path = icyboard(dir.path());
    let base = MessageBase::open(&path).unwrap();
    let news = base.messages.iter().position(|message| message.area == 2).unwrap();
    assert!(!base.can_change(news));
    assert!(base.set_deleted(news, true).is_err());
    assert!(!base.is_deleted(news));
    assert!(base.can_change(0));
}

fn post(area: u16, subject: &str, body: &str) -> Post {
    Post {
        area,
        from: b"SYSOP".to_vec(),
        to: b"All".to_vec(),
        subject: subject.as_bytes().to_vec(),
        body: body.as_bytes().to_vec(),
        private: false,
        reply_to: 0,
        utf8: true,
        address: None,
    }
}

#[test]
fn edited_messages_keep_number_identity_and_date() {
    let dir = TempDir::new();
    let path = fido_base(dir.path());
    let base = MessageBase::open(&path).unwrap();
    let before = stored(&path.with_extension(""), 1);
    let mut change = post(0, "Hello again", "Changed\nText");
    change.from = b"Alice".to_vec();
    change.utf8 = false;
    base.update(0, &change).unwrap();
    assert!(base.update(0, &post(1, "Elsewhere", "x")).is_err(), "an edit cannot move a message");

    let after = stored(&path.with_extension(""), 1);
    assert_eq!(after.text().to_string(), "Changed\rText\r");
    assert_eq!(after.header().subject().map(ToString::to_string).as_deref(), Some("Hello again"));
    assert_eq!(subfield_text(&after, SubfieldType::MsgID).as_deref(), Some("2:240/1 0001abcd"));
    assert_eq!(after.header().date_written, before.header().date_written);
    let charsets: Vec<_> = after
        .header()
        .sub_fields
        .iter()
        .filter(|field| field.field_type() == SubfieldType::FTSKludge)
        .map(|field| field.content().to_string())
        .collect();
    assert_eq!(charsets, ["CHRS: CP437 2"], "the charset kludge is replaced, not added");
    let reopened = MessageBase::open(&path).unwrap();
    assert_eq!(reopened.messages.len(), 2, "the replaced header is not a deleted message");
    assert_eq!(reopened.deleted_count(), 0);
}

#[test]
fn editing_through_a_draft_rewrites_the_stored_message() {
    let dir = TempDir::new();
    let path = icyboard(dir.path());
    let package = QwkPackage::load_from_file(&path).unwrap();
    let store = DraftStore::open_in(&path, &package, &dir.path().join("drafts")).unwrap();
    let mut draft = store.prepare(&package, Compose::Edit { index: 0 }).unwrap();
    assert_eq!(draft.kind, crate::drafts::DraftKind::Edit);
    assert_eq!((draft.subject.as_str(), draft.body.as_str()), ("Hi Sysop", "Line one\nLine two"));
    assert_eq!(draft.charset, crate::charset::Charset::Utf8, "IcyBoard text is UTF-8");
    draft.body = "Line one, changed".into();
    let mut moved = draft.clone();
    moved.conference = 1;
    assert!(!store.issues(&moved).is_empty(), "an edited message stays in its area");
    assert_eq!(store.post_draft(&draft).unwrap(), (0, 1));
    let reloaded = QwkPackage::load_from_file(&path).unwrap();
    assert_eq!(reloaded.message_count(), 2);
    assert_eq!(reloaded.get_message(0).unwrap().text.to_string(), "\u{1}CHRS: UTF-8 4\nLine one, changed");
}

#[test]
fn messages_are_copied_and_moved_between_areas() {
    let dir = TempDir::new();
    let path = icyboard(dir.path());
    let base = MessageBase::open(&path).unwrap();
    let general = dir.path().join("conferences/main/messages/general");
    base.post(&post(0, "Original", "Body")).unwrap();
    let reply = Post {
        reply_to: 2,
        ..post(0, "Re: Original", "Answer")
    };
    base.post(&reply).unwrap();
    let base = MessageBase::open(&path).unwrap();
    let index = base.messages.iter().position(|message| message.area == 0 && message.number == 3).unwrap();

    assert_eq!(base.transfer(index, 1, true).unwrap(), 1);
    let copy = stored(&dir.path().join("conferences/main/messages/empty"), 1);
    assert_eq!(copy.text().to_string(), "Answer");
    assert_eq!(copy.header().reply_to, 0, "numbers link only within an area");
    assert!(subfield_text(&copy, SubfieldType::MsgID).is_none());
    assert!(!base.is_deleted(index), "a copy leaves the original");

    assert_eq!(base.transfer(index, 3, false).unwrap(), 1);
    let moved = stored(&dir.path().join("main/email"), 1);
    assert_eq!(
        moved.header().attributes & TYPE_BITS,
        attributes::MSG_TYPELOCAL,
        "filed as the target's kind of mail"
    );
    assert!(base.is_deleted(index), "moving marks the original deleted");
    assert_eq!(stored(&general, 2).text().to_string(), "Body");

    assert!(base.transfer(index, 2, true).is_err(), "read-only areas take no messages");
    assert!(base.transfer(index, 0, true).is_err());
}

#[test]
fn transfer_to_netmail_rejects_missing_or_invalid_destinations_without_changes() {
    let dir = TempDir::new();
    let path = icyboard(dir.path());
    let source_path = dir.path().join("conferences/main/messages/general");
    let mut source = JamMessageBase::open(&source_path).unwrap();
    write_fido(
        &mut source,
        "Alice",
        "Invalid address",
        "Body\r",
        0,
        vec![field(SubfieldType::AddressD, "not an address")],
    );
    drop(source);
    let base = MessageBase::open(&path).unwrap();
    let target = base.areas.iter().find(|area| area.netmail).unwrap();
    for number in [1, 2] {
        let index = base.messages.iter().position(|message| message.area == 0 && message.number == number).unwrap();
        for keep in [true, false] {
            let error = base.transfer(index, target.number, keep).unwrap_err();
            assert!(error.to_string().contains("destination address"), "{error}");
            assert!(!base.is_deleted(index));
            assert!(!stored(&source_path, number).header().is_deleted());
            assert!(!target.path.with_extension("jhr").exists(), "rejection must not create the target");
        }
    }
}

#[test]
fn transfer_to_netmail_keeps_valid_destinations_and_sets_privacy() {
    let dir = TempDir::new();
    let path = icyboard(dir.path());
    let mut source = JamMessageBase::open(dir.path().join("conferences/main/messages/general")).unwrap();
    write_fido(&mut source, "Alice", "Addressed", "Body\r", 0, vec![field(SubfieldType::AddressD, "2:240/1.5")]);
    drop(source);
    let base = MessageBase::open(&path).unwrap();
    let index = base.messages.iter().position(|message| message.area == 0 && message.number == 2).unwrap();
    let target = base.areas.iter().find(|area| area.netmail).unwrap();
    for keep in [true, false] {
        let number = base.transfer(index, target.number, keep).unwrap();
        let transferred = stored(&target.path, number);
        assert_eq!(subfield_text(&transferred, SubfieldType::AddressD).as_deref(), Some("2:240/1.5"));
        assert_eq!(transferred.header().attributes & TYPE_BITS, attributes::MSG_TYPENET);
        assert_ne!(transferred.header().attributes & attributes::MSG_PRIVATE, 0);
        assert_eq!(base.is_deleted(index), !keep);
    }
}

#[test]
fn copies_of_sent_messages_are_local_and_unsent_without_changing_the_original() {
    let dir = TempDir::new();
    let path = icyboard(dir.path());
    let base = MessageBase::open(&path).unwrap();
    base.set_attributes(0, attributes::MSG_SENT, attributes::MSG_LOCAL).unwrap();
    let number = base.transfer(0, 1, true).unwrap();
    let copy = stored(&dir.path().join("conferences/main/messages/empty"), number);
    assert_eq!(copy.header().attributes & attributes::MSG_SENT, 0);
    assert_ne!(
        copy.header().attributes & attributes::MSG_LOCAL,
        0,
        "the tosser must recognize the new copy as written here"
    );
    let original = stored(&dir.path().join("conferences/main/messages/general"), 1);
    assert_ne!(original.header().attributes & attributes::MSG_SENT, 0);
    assert_eq!(original.header().attributes & attributes::MSG_LOCAL, 0);
    assert!(!base.is_deleted(0));
}

#[test]
fn lastread_pointers_are_read_and_moved_forward() {
    let dir = TempDir::new();
    let path = icyboard(dir.path());
    let crc = JamMessageBase::crc(&BString::from("SYSOP"));
    let base = MessageBase::open(&path).unwrap();
    assert_eq!(base.lastread_user, Some((crc, 1)), "the sysop's position in IcyBoard's user list");
    assert!(base.syncs_last_read());
    assert_eq!(base.last_read(0), 0);
    assert!(base.advance_last_read(0, 1).unwrap());
    assert!(!base.advance_last_read(0, 1).unwrap(), "the pointer never moves back or stays");

    let general = dir.path().join("conferences/main/messages/general");
    let mut jam = JamMessageBase::open(&general).unwrap();
    let record = jam.find_last_read(crc, 1).unwrap().unwrap();
    assert_eq!((record.last_read_msg, record.high_read_msg), (1, 1));

    // Like IcyBoard, only the record under the user's own id counts.
    jam.write_last_read(&JamLastReadStorage {
        user_crc: crc,
        user_id: 7,
        last_read_msg: 5,
        high_read_msg: 5,
    })
    .unwrap();
    drop(jam);
    assert_eq!(MessageBase::open(&path).unwrap().last_read(0), 1);
}

#[test]
fn lastread_stays_untouched_without_a_known_user() {
    let dir = TempDir::new();
    let path = fido_base(dir.path());
    let mut jam = JamMessageBase::open(path.with_extension("")).unwrap();
    let mut header = JamMessageHeader::default();
    header.sub_fields.push(field(SubfieldType::SenderName, "Some Caller"));
    header.attributes = attributes::MSG_LOCAL;
    jam.write_message(&JamMessage::from_stored(header, BString::from("Local\r"))).unwrap();
    drop(jam);
    let base = MessageBase::open(&path).unwrap();
    assert_eq!(base.user_name, "Some Caller", "a guess good enough to fill in the sender");
    assert!(!base.syncs_last_read(), "but not to move anybody's lastread pointer");
    assert!(!base.advance_last_read(0, 3).unwrap());
    let jam = JamMessageBase::open(path.with_extension("")).unwrap();
    assert!(jam.read_last_read_file().unwrap().is_empty());
}

#[test]
fn packing_can_purge_old_and_received_private_messages() {
    let dir = TempDir::new();
    let path = fido_base(dir.path());
    let base = MessageBase::open(&path).unwrap();
    let keep = base
        .pack(PackChoice {
            purge_before: chrono::DateTime::from_timestamp(1_600_000_000, 0),
            purge_received_private: false,
        })
        .unwrap();
    assert_eq!(keep.removed, 0, "both messages are newer");
    let purged = base
        .pack(PackChoice {
            purge_before: Some(chrono::Utc::now()),
            purge_received_private: false,
        })
        .unwrap();
    assert_eq!(purged.removed, 2);
    assert!(MessageBase::open(&path).unwrap().messages.is_empty());
}

#[test]
fn bases_are_checked_and_repaired() {
    let dir = TempDir::new();
    let path = icyboard(dir.path());
    let base = MessageBase::open(&path).unwrap();
    let checks = base.verify().unwrap();
    assert_eq!(checks.len(), 3, "areas without a base yet are skipped");
    assert!(checks.iter().all(|check| check.ok()), "{checks:?}");
    // A torn lastread record, as a crash while writing leaves it.
    let lastread = dir.path().join("conferences/main/messages/general.jlr");
    fs::write(&lastread, [0u8; 5]).unwrap();
    let checks = base.verify().unwrap();
    let general = checks.iter().find(|check| check.area == "Main Board / General").unwrap();
    assert!(!general.ok(), "{general:?}");
    let repaired = base.repair().unwrap();
    assert!(repaired.iter().all(|check| check.ok()), "{repaired:?}");
    assert!(base.verify().unwrap().iter().all(|check| check.ok()));
}

#[test]
fn message_details_show_the_header_and_attributes_change() {
    let dir = TempDir::new();
    let path = fido_base(dir.path());
    let base = MessageBase::open(&path).unwrap();
    let details = base.details(1).unwrap();
    assert_eq!((details.number, details.reply_to, details.text_len), (2, 1, 8));
    assert!(details.fields.contains(&("MSGID".to_owned(), "2:240/2 0002abcd".to_owned())));
    assert!(details.fields.contains(&("REPLY".to_owned(), "2:240/1 0001abcd".to_owned())));
    base.set_attributes(1, attributes::MSG_SENT | attributes::MSG_DELETED, attributes::MSG_TYPEECHO)
        .unwrap();
    let details = base.details(1).unwrap();
    assert_eq!(details.attributes, attributes::MSG_SENT, "the delete flag only changes through its own command");
    assert!(!base.is_deleted(1));
}

#[test]
fn netmail_carries_its_destination_address() {
    let dir = TempDir::new();
    let path = icyboard(dir.path());
    let package = QwkPackage::load_from_file(&path).unwrap();
    let base = package.message_base.as_ref().unwrap();
    assert!(base.is_netmail(4) && !base.is_netmail(0));
    assert_eq!(parse_address(" 2:240/1.5 ").as_deref(), Some("2:240/1.5"));
    assert!(parse_address("240/1").is_none());

    let store = DraftStore::open_in(&path, &package, &dir.path().join("drafts")).unwrap();
    let mut draft = store.prepare(&package, Compose::New { conference: 4 }).unwrap();
    draft.to = "Point Operator".into();
    draft.subject = "Hello".into();
    draft.body = "Direct".into();
    let issue = |draft: &crate::drafts::Draft| store.issues(draft).into_iter().any(|issue| issue.field == crate::drafts::DraftField::Address);
    assert!(issue(&draft), "netmail needs an address");
    draft.address = "fido".into();
    assert!(issue(&draft));
    draft.address = "2:240/1".into();
    assert!(!issue(&draft));
    assert_eq!(store.post_draft(&draft).unwrap(), (4, 1));

    let message = stored(&dir.path().join("ftn/netmail"), 1);
    assert_eq!(subfield_text(&message, SubfieldType::AddressD).as_deref(), Some("2:240/1"));
    assert_eq!(
        message.header().attributes,
        attributes::MSG_LOCAL | attributes::MSG_PRIVATE | attributes::MSG_TYPENET,
        "netmail is private and goes out as netmail"
    );

    // An answer goes back to where the original was written.
    let mut netmail = JamMessageBase::open(dir.path().join("ftn/netmail")).unwrap();
    netmail
        .write_message(
            &JamMessage::default()
                .with_from("Point Operator".into())
                .with_to("SYSOP".into())
                .with_subject("Re: Hello".into())
                .with_sub_field(field(SubfieldType::Address0, "2:240/1.5"))
                .with_attributes(attributes::MSG_TYPENET | attributes::MSG_PRIVATE),
        )
        .unwrap();
    drop(netmail);
    let package = QwkPackage::load_from_file(&path).unwrap();
    let index = package.infos.iter().position(|info| info.conference == 4 && info.number == 2).unwrap();
    let store = DraftStore::open_in(&path, &package, &dir.path().join("drafts")).unwrap();
    let reply = store.prepare(&package, Compose::Reply { index }).unwrap();
    assert_eq!((reply.address.as_str(), reply.to.as_str()), ("2:240/1.5", "Point Operator"));
}
