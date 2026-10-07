//! Blue Wave level 2/3 wire records (little endian, without C alignment padding).
//!
//! Layout authority: Cutting Edge Computing's November 1995 packet structures,
//! mirrored at <https://raw.githubusercontent.com/wmcbrine/MultiMail/master/mmail/bluewave.h>.
//! Independently checked against MultiMail's `bw.cc` and ENiGMA's
//! <https://github.com/NuSkooler/enigma-bbs/blob/master/core/bluewave_mail_packet.js>.
//! See also <https://enigma-bbs.github.io/messageareas/bluewave/> and
//! <https://www.moon-soft.com/program/FORMAT/internet/bluewave.htm>.
//!
//! DAT spans exclude the required leading ASCII space. Some producers count the
//! marker in FTI's length and others do not (MultiMail and ENiGMA document this
//! interoperability difference). Adjacent offsets/the file end disambiguate
//! contiguous spans; otherwise parsing fails rather than truncating a body.
//! The caller must skip that marker (this API receives only DAT's length) and
//! normalize CRLF/CR to LF. Some doors (e.g. OLMS) omit the marker on a few
//! messages, so a missing marker means the whole span is body text.
//! Unusable posting identities/echotags and unused MIX fields are logged rather
//! than blocking reading. Reply generation still validates posting metadata.
//! Soft CR (0x8d) is optional wrapping, not a line ending. Bodies remain CP437.
//! FTI dates have no specified timezone; recognized dates are interpreted as UTC.

use std::collections::{HashMap, HashSet};

pub const FROM_LIMIT: usize = 35;
pub const TO_LIMIT: usize = 35;
pub const SUBJECT_LIMIT: usize = 71;
pub const INF_HEADER_LEN: usize = 1230;
pub const INF_AREA_LEN: usize = 80;
pub const MIX_LEN: usize = 14;
pub const FTI_LEN: usize = 186;
pub const UPL_HEADER_LEN: usize = 256;
pub const UPL_REC_LEN: usize = 320;
pub const UPI_HEADER_LEN: usize = 55;
pub const UPI_REC_LEN: usize = 184;
pub const NET_REC_LEN: usize = 232;

pub const INF_ALIAS_NAME: u16 = 0x0002;
pub const INF_ANY_NAME: u16 = 0x0004;
pub const INF_NETMAIL: u16 = 0x0010;
pub const INF_POST: u16 = 0x0020;
pub const INF_NO_PRIVATE: u16 = 0x0040;
pub const INF_NO_PUBLIC: u16 = 0x0080;
pub const INF_NO_HIGHBIT: u16 = 0x0200;

#[derive(Clone, Debug)]
pub struct Packet {
    pub info: Info,
    pub areas: Vec<Area>,
    pub messages: Vec<Message>,
}

#[derive(Clone, Debug)]
pub struct Info {
    pub bbs_id: String,
    pub bbs_name: String,
    pub user_name: Vec<u8>,
    pub alias: Vec<u8>,
    pub uses_upl: bool,
    pub level: u8,
    pub from_to_limit: usize,
    pub subject_limit: usize,
}

#[derive(Clone, Debug)]
pub struct Area {
    pub number: u16,
    pub echotag: String,
    pub title: Vec<u8>,
    pub flags: u16,
    pub network_type: u8,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub area: u16,
    pub number: u32,
    pub reply_to: u32,
    pub unix_time: i64,
    pub from: Vec<u8>,
    pub to: Vec<u8>,
    pub subject: Vec<u8>,
    pub body_offset: u64,
    pub body_len: u64,
    pub private: bool,
    /// Retained because the specification deliberately leaves the date format
    /// to the BBS. `unix_time` is zero when this is empty or unrecognized.
    pub date_raw: Vec<u8>,
    /// Distinguishes a decoded timestamp (including epoch zero) from a date
    /// whose BBS-specific syntax this reader cannot interpret.
    pub date_known: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    pub conference: u16,
    pub reply_to: u32,
    pub unix_time: i64,
    pub from: Vec<u8>,
    pub to: Vec<u8>,
    pub subject: Vec<u8>,
    pub body: Vec<u8>,
    pub private: bool,
}

fn bad<T>(message: impl Into<String>) -> crate::Res<T> {
    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, message.into()).into())
}

fn word(b: &[u8], p: usize) -> u16 {
    u16::from_le_bytes([b[p], b[p + 1]])
}

fn dword(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]])
}

fn put_word(b: &mut [u8], p: usize, n: u16) {
    b[p..p + 2].copy_from_slice(&n.to_le_bytes());
}

fn put_dword(b: &mut [u8], p: usize, n: u32) {
    b[p..p + 4].copy_from_slice(&n.to_le_bytes());
}

fn text(b: &[u8]) -> Vec<u8> {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    let mut value = b[..end].to_vec();
    while value.last() == Some(&b' ') {
        value.pop();
    }
    value
}

fn ascii(b: &[u8], field: &str) -> crate::Res<String> {
    let b = text(b);
    if b.is_empty() || !b.iter().all(|c| c.is_ascii_graphic() || *c == b' ') {
        return bad(format!("Invalid Blue Wave {field}"));
    }
    Ok(String::from_utf8(b)?)
}

fn basename(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 12
        && !name.starts_with('.')
        && name.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
        && name.split('.').count() <= 2
        && name.split('.').next().is_some_and(|s| s.len() <= 8)
        && name.split('.').nth(1).is_none_or(|s| !s.is_empty() && s.len() <= 3)
}

fn packet_id(b: &[u8]) -> crate::Res<String> {
    let id = ascii(b, "packet ID")?;
    if id.len() > 8 || !basename(&id) || id.contains('.') {
        return bad("Blue Wave packet ID is not a safe DOS basename");
    }
    Ok(id)
}

fn number(b: &[u8]) -> crate::Res<u16> {
    let s = ascii(b, "area number")?;
    let s = s.trim();
    if !s.bytes().all(|c| c.is_ascii_digit()) {
        return bad("Blue Wave area number is not decimal");
    }
    Ok(s.parse()?)
}

fn size(b: &[u8], p: usize, minimum: usize) -> crate::Res<usize> {
    let n = word(b, p) as usize;
    if n == 0 {
        Ok(minimum)
    } else if n < minimum {
        bad("Blue Wave structure length is shorter than its defined layout")
    } else {
        Ok(n)
    }
}

fn records(b: &[u8], head: usize, stride: usize) -> crate::Res<&[u8]> {
    if head > b.len() || !(b.len() - head).is_multiple_of(stride) {
        return bad("Truncated Blue Wave header or record");
    }
    Ok(&b[head..])
}

fn limit(n: u8, maximum: usize) -> usize {
    if n == 0 || n as usize > maximum {
        maximum
    } else {
        n as usize
    }
}

fn date(b: &[u8]) -> Option<i64> {
    let Ok(s) = std::str::from_utf8(b) else { return None };
    for format in [
        "%d %b %y  %H:%M:%S",
        "%d %b %y %H:%M:%S",
        "%d %b %Y %H:%M:%S",
        "%m-%d-%y %H:%M:%S",
        "%m-%d-%y %H:%M",
        "%Y-%m-%d %H:%M:%S",
    ] {
        if let Ok(d) = chrono::NaiveDateTime::parse_from_str(s, format) {
            return Some(d.and_utc().timestamp());
        }
    }
    None
}

/// Decode metadata only. FTI's signed DAT pointers/lengths must be nonnegative;
/// message spans must not overlap. Unknown packet levels are not guessed.
pub fn parse(inf: &[u8], mix: &[u8], fti: &[u8], dat_len: u64) -> crate::Res<Packet> {
    if inf.len() < INF_HEADER_LEN {
        return bad("Truncated Blue Wave INF header");
    }
    let level = inf[0];
    if !matches!(level, 2 | 3) {
        return bad(format!("Unsupported Blue Wave packet level {level}"));
    }
    if word(inf, 301) != 0 {
        return bad("QWK-converted INF metadata is not a native Blue Wave packet");
    }
    let header_len = size(inf, 976, INF_HEADER_LEN)?;
    let area_len = size(inf, 978, INF_AREA_LEN)?;
    let mix_len = size(inf, 980, MIX_LEN)?;
    let fti_len = size(inf, 982, FTI_LEN)?;
    let mut packet = Packet {
        info: Info {
            bbs_id: packet_id(&inf[987..996])?,
            bbs_name: text(&inf[235..300]).into_iter().map(crate::editor::cp437_char).collect(),
            user_name: text(&inf[76..119]),
            alias: text(&inf[119..162]),
            uses_upl: inf[984] != 0,
            level,
            from_to_limit: limit(inf[985], FROM_LIMIT),
            subject_limit: limit(inf[986], SUBJECT_LIMIT),
        },
        areas: Vec::new(),
        messages: Vec::new(),
    };
    if packet.info.user_name.is_empty() {
        log::warn!("Blue Wave INF has no login identity; reading is available but replies are disabled");
    }
    for (value, name) in [(&packet.info.user_name, "login identity"), (&packet.info.alias, "alias identity")] {
        if let Err(error) = field(value, 42, name) {
            log::warn!("{error}; retaining Blue Wave metadata for reading");
        }
    }
    let mut area_numbers = HashSet::new();
    let mut tags = HashSet::new();
    for a in records(inf, header_len, area_len)?.chunks_exact(area_len) {
        let number = number(&a[..6])?;
        let echotag: String = text(&a[6..27]).into_iter().map(crate::editor::cp437_char).collect();
        if !area_numbers.insert(number) {
            return bad("Duplicate Blue Wave area number");
        }
        if let Err(error) = ascii(echotag.as_bytes(), "echotag").and_then(|_| field(echotag.as_bytes(), 20, "echotag")) {
            log::warn!("Blue Wave area {number}: {error}; retaining area for reading");
        }
        if !tags.insert(echotag.to_ascii_lowercase()) {
            log::warn!("Duplicate Blue Wave echotag {echotag:?} in area {number}; retaining area for reading");
        }
        packet.areas.push(Area {
            number,
            echotag,
            title: text(&a[27..77]),
            flags: word(a, 77),
            network_type: a[79],
        });
    }
    let fti_records = records(fti, 0, fti_len)?;
    let mut used_headers = HashSet::new();
    let mut used_mix = HashSet::new();
    let mut identities = HashSet::new();
    let mut spans = Vec::new();
    for m in records(mix, 0, mix_len)?.chunks_exact(mix_len) {
        let area = number(&m[..6])?;
        if !area_numbers.contains(&area) || !used_mix.insert(area) {
            return bad("Unknown or duplicate Blue Wave MIX area");
        }
        let count = word(m, 6) as usize;
        if word(m, 8) as usize > count {
            log::warn!("Blue Wave MIX area {area}: personal count exceeds message count; ignoring unused personal count");
        }
        let start = dword(m, 10) as usize;
        if count == 0 {
            if start > i32::MAX as usize || !start.is_multiple_of(fti_len) || start > fti.len() {
                log::warn!("Blue Wave MIX area {area}: invalid unused FTI pointer {start}; ignoring pointer for empty area");
            }
            continue;
        }
        if start > i32::MAX as usize || !start.is_multiple_of(fti_len) || start > fti.len() || count > (fti.len() - start) / fti_len {
            return bad("Blue Wave MIX header span is outside FTI");
        }
        for i in 0..count {
            let offset = start + i * fti_len;
            if !used_headers.insert(offset) {
                return bad("Overlapping Blue Wave MIX header spans");
            }
            let h = &fti[offset..offset + fti_len];
            let num = word(h, 164) as u32;
            // Zero means the door did not supply a BBS message number.
            if num != 0 && !identities.insert((area, num)) {
                return bad("Duplicate Blue Wave message identity");
            }
            let ptr = dword(h, 170) as u64;
            let len = dword(h, 174) as u64;
            if ptr > i32::MAX as u64 || ptr >= dat_len || len > i32::MAX as u64 || ptr + len > dat_len {
                return bad("Blue Wave DAT body span is invalid");
            }
            spans.push((ptr, len, packet.messages.len()));
            let raw = text(&h[144..164]);
            let parsed_date = date(&raw);
            if parsed_date.is_none() && !raw.is_empty() {
                log::warn!(
                    "Blue Wave area {area} message {num}: unrecognized date {:?}; preserving original date",
                    String::from_utf8_lossy(&raw)
                );
            }
            packet.messages.push(Message {
                area,
                number: num,
                reply_to: word(h, 166) as u32,
                unix_time: parsed_date.unwrap_or(0),
                from: text(&h[..36]),
                to: text(&h[36..72]),
                subject: text(&h[72..144]),
                body_offset: ptr + 1,
                body_len: 0,
                private: word(h, 178) & 1 != 0,
                date_raw: raw,
                date_known: parsed_date.is_some(),
            });
        }
    }
    if used_headers.len() != fti_records.len() / fti_len {
        return bad("Unindexed Blue Wave FTI records");
    }
    spans.sort_unstable();
    for (i, &(ptr, len, index)) in spans.iter().enumerate() {
        let boundary = spans.get(i + 1).map_or(dat_len, |s| s.0);
        let distance = boundary - ptr;
        packet.messages[index].body_len = if distance == len && len > 0 {
            len - 1
        } else if distance == len + 1 {
            len
        } else {
            return bad("Overlapping or ambiguous Blue Wave DAT spans/marker lengths");
        };
    }
    Ok(packet)
}

fn field(value: &[u8], maximum: usize, name: &str) -> crate::Res<()> {
    if value.len() > maximum || value.iter().any(|&c| c == 0 || c == b'\r' || c == b'\n') || value.last() == Some(&b' ') {
        return bad(format!("Blue Wave {name} exceeds its limit or contains invalid characters"));
    }
    Ok(())
}

fn copy(b: &mut [u8], p: usize, value: &[u8]) {
    b[p..p + value.len()].copy_from_slice(value);
}

fn validate_packet(packet: &Packet) -> crate::Res<()> {
    packet_id(packet.info.bbs_id.as_bytes())?;
    if !matches!(packet.info.level, 2 | 3)
        || packet.info.from_to_limit == 0
        || packet.info.from_to_limit > FROM_LIMIT
        || packet.info.subject_limit == 0
        || packet.info.subject_limit > SUBJECT_LIMIT
    {
        return bad("Invalid Blue Wave packet capabilities");
    }
    field(&packet.info.user_name, 42, "login identity")?;
    field(&packet.info.alias, 42, "alias identity")?;
    if packet.info.user_name.is_empty() {
        return bad("Blue Wave login identity is empty");
    }
    let mut numbers = HashSet::new();
    let mut tags = HashSet::new();
    for a in &packet.areas {
        field(a.echotag.as_bytes(), 20, "echotag")?;
        ascii(a.echotag.as_bytes(), "echotag")?;
        if !numbers.insert(a.number) || !tags.insert(a.echotag.to_ascii_lowercase()) {
            return bad("Duplicate Blue Wave area identity");
        }
    }
    Ok(())
}

/// Composer defaults: the area's login/alias (bounded by the host name limit)
/// and whether privacy is required. `INF_ANY_NAME` permits later sender edits.
/// Read-only, netmail, conflicting privacy permissions and unusable identities
/// are reported before composing, without generating any export members.
pub fn posting_defaults(packet: &Packet, conference: u16) -> crate::Res<(Vec<u8>, bool)> {
    validate_packet(packet)?;
    let Some(area) = packet.areas.iter().find(|a| a.number == conference) else {
        return bad("Unknown Blue Wave reply area");
    };
    if area.flags & INF_POST == 0 {
        return bad("Blue Wave area is read-only");
    }
    if area.flags & INF_NETMAIL != 0 {
        return bad("Blue Wave netmail requires a destination address; this API does not support it");
    }
    if area.flags & (INF_NO_PUBLIC | INF_NO_PRIVATE) == INF_NO_PUBLIC | INF_NO_PRIVATE {
        return bad("Blue Wave area permits neither public nor private messages");
    }
    let mut identity = if area.flags & INF_ALIAS_NAME != 0 {
        &packet.info.alias
    } else {
        &packet.info.user_name
    };
    if identity.is_empty() && area.flags & INF_ANY_NAME != 0 {
        identity = &packet.info.user_name;
    }
    let sender = text(&identity[..identity.len().min(packet.info.from_to_limit)]);
    if sender.is_empty() {
        return bad("Blue Wave area requires an unavailable sender identity");
    }
    if area.flags & INF_NO_HIGHBIT != 0 && sender.iter().any(|&c| c > 127) {
        return bad("Blue Wave area permits only seven-bit text");
    }
    Ok((sender, area.flags & INF_NO_PUBLIC != 0))
}

/// Preflight export validation for the composer without allocating export files.
/// Includes posting/privacy, From identity, host header limits, seven-bit-only
/// areas, unsupported netmail, signed 32-bit dates and legacy reply-to policy.
/// `uses_upl == false` selects UPI; it does not itself forbid posting.
pub fn validate_reply(packet: &Packet, reply: &Reply) -> crate::Res<()> {
    validate_packet(packet)?;
    validate_export_reply(packet, reply).map(|_| ())
}

fn validate_export_reply<'a>(packet: &'a Packet, reply: &Reply) -> crate::Res<&'a Area> {
    if !packet.info.uses_upl && reply.reply_to != 0 {
        return bad("Legacy Blue Wave UPI cannot encode reply-to numbers");
    }
    validate_reply_fields(packet, reply)
}

fn validate_reply_fields<'a>(packet: &'a Packet, reply: &Reply) -> crate::Res<&'a Area> {
    let Some(area) = packet.areas.iter().find(|a| a.number == reply.conference) else {
        return bad("Unknown Blue Wave reply area");
    };
    if area.flags & INF_POST == 0 || (reply.private && area.flags & INF_NO_PRIVATE != 0) || (!reply.private && area.flags & INF_NO_PUBLIC != 0) {
        return bad("Blue Wave area does not permit this reply");
    }
    if area.flags & INF_NETMAIL != 0 {
        return bad("Blue Wave netmail requires a destination address; this API does not support it");
    }
    let identity = if area.flags & INF_ALIAS_NAME != 0 {
        &packet.info.alias
    } else {
        &packet.info.user_name
    };
    let mut identity = &identity[..identity.len().min(packet.info.from_to_limit)];
    while identity.last() == Some(&b' ') {
        identity = &identity[..identity.len() - 1];
    }
    if area.flags & INF_ANY_NAME == 0 && !reply.from.eq_ignore_ascii_case(identity) {
        return bad("Blue Wave From identity does not match the area's login/alias requirement");
    }
    if reply.from.is_empty() || reply.to.is_empty() {
        return bad("Blue Wave reply requires From and To");
    }
    field(&reply.from, packet.info.from_to_limit, "From")?;
    field(&reply.to, packet.info.from_to_limit, "To")?;
    field(&reply.subject, packet.info.subject_limit, "Subject")?;
    if reply.body.contains(&0) || reply.body.len() > i32::MAX as usize {
        return bad("Invalid Blue Wave reply body");
    }
    if area.flags & INF_NO_HIGHBIT != 0 && reply.from.iter().chain(&reply.to).chain(&reply.subject).chain(&reply.body).any(|&c| c > 127) {
        return bad("Blue Wave area permits only seven-bit text");
    }
    i32::try_from(reply.unix_time)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "Blue Wave reply date is outside signed 32-bit Unix time"))?;
    Ok(area)
}

/// Generate DOS-basename members. Legacy UPI has no reply-to or login/alias
/// header fields; a nonzero reply-to is rejected rather than silently lost.
/// NET is empty because addressed netmail is deliberately unsupported.
pub fn reply_files(packet: &Packet, replies: &[Reply]) -> crate::Res<Vec<(String, Vec<u8>)>> {
    validate_packet(packet)?;
    if replies.len() > 999_999 {
        return bad("Too many Blue Wave replies");
    }
    let upl = packet.info.uses_upl;
    let (head, stride, ext) = if upl {
        (UPL_HEADER_LEN, UPL_REC_LEN, "UPL")
    } else {
        (UPI_HEADER_LEN, UPI_REC_LEN, "UPI")
    };
    let mut index = vec![0; head];
    if upl {
        copy(&mut index, 10, &b"icy_mail 0.1".iter().map(|c| c + 10).collect::<Vec<_>>());
        copy(&mut index, 32, b"icy_mail");
        index[31] = 1;
        copy(&mut index, 204, b"icy_mail");
        put_word(&mut index, 112, head as u16);
        put_word(&mut index, 114, stride as u16);
        copy(&mut index, 116, &packet.info.user_name);
        copy(&mut index, 160, &packet.info.alias);
        index[222] = 1;
    } else {
        copy(&mut index, 9, &b"icy_mail 0.1".iter().map(|c| c + 10).collect::<Vec<_>>());
    }
    let mut files = Vec::new();
    for (i, reply) in replies.iter().enumerate() {
        let area = validate_export_reply(packet, reply)?;
        let filename = format!("R{:06}.MSG", i + 1);
        let mut rec = vec![0; stride];
        copy(&mut rec, 0, &reply.from);
        copy(&mut rec, 36, &reply.to);
        copy(&mut rec, 72, &reply.subject);
        if upl {
            put_word(&mut rec, 152, (if reply.private { 2 } else { 0 }) | (if reply.reply_to != 0 { 32 } else { 0 }));
            put_dword(&mut rec, 156, reply.unix_time as i32 as u32);
            put_dword(&mut rec, 160, reply.reply_to);
            copy(&mut rec, 164, filename.as_bytes());
            copy(&mut rec, 177, area.echotag.as_bytes());
            put_word(&mut rec, 198, area.flags);
            rec[219] = area.network_type;
        } else {
            put_dword(&mut rec, 144, reply.unix_time as i32 as u32);
            copy(&mut rec, 148, filename.as_bytes());
            copy(&mut rec, 161, area.echotag.as_bytes());
            rec[182] = if reply.private { 0x40 } else { 0 };
        }
        index.extend_from_slice(&rec);
        files.push((filename, reply.body.clone()));
    }
    files.insert(0, (format!("{}.{}", packet.info.bbs_id, ext), index));
    if !upl {
        files.insert(1, (format!("{}.NET", packet.info.bbs_id), Vec::new()));
    }
    Ok(files)
}

/// Import UPL or UPI replies, resolving filenames case-insensitively. Offline
/// control members are ignored, but duplicate names, mixed index formats,
/// foreign roots, unknown areas and unsupported netmail are errors.
pub fn read_replies(packet: &Packet, files: &[(String, Vec<u8>)]) -> crate::Res<Vec<Reply>> {
    validate_packet(packet)?;
    let mut members = HashMap::new();
    let mut upl = None;
    let mut upi = None;
    let mut net = None;
    for (name, contents) in files {
        if !basename(name) || members.insert(name.to_ascii_lowercase(), contents.as_slice()).is_some() {
            return bad("Unsafe or duplicate Blue Wave reply filename");
        }
        let Some((root, extension)) = name.rsplit_once('.') else { continue };
        if matches!(extension.to_ascii_lowercase().as_str(), "upl" | "upi" | "net") {
            if !root.eq_ignore_ascii_case(&packet.info.bbs_id) {
                return bad("Blue Wave reply packet ID does not match");
            }
            match extension.to_ascii_lowercase().as_str() {
                "upl" => upl = Some(contents.as_slice()),
                "upi" => upi = Some(contents.as_slice()),
                "net" => net = Some(contents.as_slice()),
                _ => unreachable!(),
            }
        }
    }
    if upl.is_some() && (upi.is_some() || net.is_some()) {
        return bad("Ambiguous mixed Blue Wave reply index formats");
    }
    if let Some(net) = net {
        records(net, 0, NET_REC_LEN)?;
        if !net.is_empty() {
            return bad("Addressed Blue Wave NET replies are not supported");
        }
    }
    let (data, head, stride, modern) = if let Some(b) = upl {
        if b.len() < UPL_HEADER_LEN {
            return bad("Truncated Blue Wave UPL header");
        }
        if text(&b[116..160]) != packet.info.user_name || text(&b[160..204]) != packet.info.alias {
            return bad("Blue Wave UPL login/alias does not match the packet");
        }
        (b, size(b, 112, UPL_HEADER_LEN)?, size(b, 114, UPL_REC_LEN)?, true)
    } else if let Some(b) = upi {
        (b, UPI_HEADER_LEN, UPI_REC_LEN, false)
    } else {
        return bad("No Blue Wave UPL or UPI reply index");
    };
    let mut result = Vec::new();
    let mut referenced = HashSet::new();
    for rec in records(data, head, stride)?.chunks_exact(stride) {
        let (filename_pos, tag_pos, time_pos, private, reply_to) = if modern {
            let flags = word(rec, 152);
            if flags & 1 != 0 {
                continue;
            }
            if flags & !0x22 != 0
                || word(rec, 154) != 0
                || rec[144..152].iter().any(|&c| c != 0)
                || !text(&rec[200..213]).is_empty()
                || !text(&rec[220..320]).is_empty()
            {
                return bad("Unsupported Blue Wave UPL message attributes or addressing");
            }
            (164, 177, 156, flags & 2 != 0, dword(rec, 160))
        } else {
            if rec[182] & !0x40 != 0 {
                return bad("Unsupported Blue Wave UPI message attributes");
            }
            (148, 161, 144, rec[182] & 0x40 != 0, 0)
        };
        let tag = ascii(&rec[tag_pos..tag_pos + 21], "reply echotag")?;
        let Some(area) = packet.areas.iter().find(|a| a.echotag.eq_ignore_ascii_case(&tag)) else {
            return bad("Unknown Blue Wave reply echotag");
        };
        if modern && rec[219] != area.network_type {
            return bad("Blue Wave reply network type differs from its area");
        }
        let filename = ascii(&rec[filename_pos..filename_pos + 13], "reply body filename")?;
        if !basename(&filename)
            || !referenced.insert(filename.to_ascii_lowercase())
            || filename
                .rsplit_once('.')
                .is_some_and(|(_, e)| matches!(e.to_ascii_lowercase().as_str(), "upl" | "upi" | "net" | "olc" | "pdq" | "req"))
        {
            return bad("Unsafe or ambiguous Blue Wave reply body reference");
        }
        let Some(body) = members.get(&filename.to_ascii_lowercase()) else {
            return bad("Missing Blue Wave reply body file");
        };
        let reply = Reply {
            conference: area.number,
            reply_to,
            unix_time: dword(rec, time_pos) as i32 as i64,
            from: text(&rec[..36]),
            to: text(&rec[36..72]),
            subject: text(&rec[72..144]),
            body: body.to_vec(),
            private,
        };
        validate_reply_fields(packet, &reply)?;
        result.push(reply);
    }
    Ok(result)
}

/// Shared integration fixture, built from literal specification offsets.
/// TEST: Alice/Ally; areas 12 LOCAL and 24 SECOND; one private Bob -> Alice
/// message 45 replying to 42, subject Hey, CP437 body `Hello\r\n\x82!`.
#[cfg(test)]
pub(crate) fn fixture_files(level: u8, uses_upl: bool) -> Vec<(String, Vec<u8>)> {
    fixtures::fixture_files(level, uses_upl)
}

#[cfg(test)]
#[path = "../tests/fixtures/blue_wave.rs"]
mod fixtures;

#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::incoming;

    fn packet() -> Packet {
        let (inf, mix, fti, dat) = incoming(3, false);
        parse(&inf, &mix, &fti, dat.len() as u64).unwrap()
    }

    fn reply() -> Reply {
        Reply {
            conference: 12,
            reply_to: 45,
            unix_time: 1_704_110_400,
            from: b"Alice".to_vec(),
            to: b"Bob".to_vec(),
            subject: b"Re: Hey".to_vec(),
            body: b"Reply\r\n\x82".to_vec(),
            private: true,
        }
    }

    fn external_upl(extension: bool) -> Vec<(String, Vec<u8>)> {
        let head = if extension { 264usize } else { 256 };
        let stride = if extension { 328usize } else { 320 };
        let mut index = vec![0; head + stride];
        index[112..114].copy_from_slice(&(head as u16).to_le_bytes());
        index[114..116].copy_from_slice(&(stride as u16).to_le_bytes());
        index[116..121].copy_from_slice(b"Alice");
        index[160..164].copy_from_slice(b"Ally");
        let rec = &mut index[head..];
        rec[..5].copy_from_slice(b"Alice");
        rec[36..39].copy_from_slice(b"Bob");
        rec[72..79].copy_from_slice(b"Re: Hey");
        rec[152..154].copy_from_slice(&0x22u16.to_le_bytes());
        rec[156..160].copy_from_slice(&1_704_110_400u32.to_le_bytes());
        rec[160..164].copy_from_slice(&45u32.to_le_bytes());
        rec[164..173].copy_from_slice(b"00001.MSG");
        rec[177..182].copy_from_slice(b"LOCAL");
        rec[198..200].copy_from_slice(&0x21u16.to_le_bytes());
        vec![("TEST.UPL".into(), index), ("00001.MSG".into(), b"Reply\r\n\x82".to_vec())]
    }

    #[test]
    fn independent_incoming_levels_and_extensions() {
        for level in [2, 3] {
            for extended in [false, true] {
                let (inf, mix, fti, dat) = incoming(level, extended);
                let p = parse(&inf, &mix, &fti, dat.len() as u64).unwrap();
                assert_eq!(p.info.bbs_name, "My BBS!");
                assert_eq!(p.info.user_name, b"Alice");
                assert_eq!(p.info.alias, b"Ally");
                assert_eq!(p.info.level, level);
                assert_eq!(p.areas.len(), 2);
                assert_eq!(p.areas[0].title, b"Title");
                assert_eq!(p.messages.len(), 1);
                let m = &p.messages[0];
                assert_eq!((m.number, m.reply_to, m.area), (45, 42, 12));
                assert_eq!(
                    (&m.from[..], &m.to[..], &m.subject[..]),
                    (b"Bob".as_slice(), b"Alice".as_slice(), b"Hey".as_slice())
                );
                assert!(m.private && m.date_known);
                assert_eq!(m.unix_time, 1_704_112_496);
                assert_eq!(&dat[m.body_offset as usize..(m.body_offset + m.body_len) as usize], b"Hello\r\n\x82!");
            }
        }
    }

    #[test]
    fn independent_upl_and_extended_header_import() {
        for extension in [false, true] {
            assert_eq!(read_replies(&packet(), &external_upl(extension)).unwrap(), vec![reply()]);
        }
    }

    #[test]
    fn writer_matches_literal_wire_offsets() {
        let p = packet();
        let files = reply_files(&p, &[reply()]).unwrap();
        let index = &files[0].1;
        assert_eq!(files[0].0, "TEST.UPL");
        assert_eq!(index.len(), 576);
        assert_eq!(&index[112..116], &[0, 1, 64, 1]);
        assert_eq!(index[31], 1);
        assert_eq!(&index[10..22], &b"icy_mail 0.1".iter().map(|c| c + 10).collect::<Vec<_>>());
        assert_eq!(&index[116..121], b"Alice");
        assert_eq!(&index[160..164], b"Ally");
        assert_eq!(&index[256 + 152..256 + 154], &[34, 0]);
        assert_eq!(&index[256 + 160..256 + 164], &[45, 0, 0, 0]);
        assert_eq!(&index[256 + 164..256 + 175], b"R000001.MSG");
        assert_eq!(&index[256 + 177..256 + 182], b"LOCAL");
        assert_eq!(files[1].1, reply().body);
        assert_eq!(read_replies(&p, &files).unwrap(), vec![reply()]);
    }

    #[test]
    fn legacy_upi_literal_fixture_and_writer() {
        let mut p = packet();
        p.info.level = 2;
        p.info.uses_upl = false;
        let mut r = reply();
        assert!(reply_files(&p, &[r.clone()]).is_err());
        r.reply_to = 0;
        let files = reply_files(&p, &[r.clone()]).unwrap();
        assert_eq!((files[0].0.as_str(), files[0].1.len()), ("TEST.UPI", 239));
        assert_eq!(files[1], ("TEST.NET".into(), Vec::new()));
        assert_eq!(&files[0].1[55 + 148..55 + 159], b"R000001.MSG");
        assert_eq!(&files[0].1[55 + 161..55 + 166], b"LOCAL");
        assert_eq!(files[0].1[55 + 182], 0x40);
        assert_eq!(read_replies(&p, &files).unwrap(), vec![r.clone()]);
        let mut literal = vec![0u8; 239];
        literal[55..60].copy_from_slice(b"Alice");
        literal[91..94].copy_from_slice(b"Bob");
        literal[127..134].copy_from_slice(b"Re: Hey");
        literal[199..203].copy_from_slice(&1_704_110_400u32.to_le_bytes());
        literal[203..212].copy_from_slice(b"00001.MSG");
        literal[216..221].copy_from_slice(b"LOCAL");
        literal[237] = 0x40;
        assert_eq!(
            read_replies(&p, &[("TEST.UPI".into(), literal), ("00001.MSG".into(), r.body.clone())]).unwrap(),
            vec![r]
        );
    }

    #[test]
    fn incoming_rejects_bad_levels_sizes_counts_offsets_and_identities() {
        let (inf, mix, fti, dat) = incoming(3, false);
        for level in [0, 1, 4, 255] {
            let mut b = inf.clone();
            b[0] = level;
            assert!(parse(&b, &mix, &fti, dat.len() as u64).is_err());
        }
        for (offset, short) in [(976, 1229), (978, 79), (980, 13), (982, 185)] {
            let mut b = inf.clone();
            b[offset..offset + 2].copy_from_slice(&(short as u16).to_le_bytes());
            assert!(parse(&b, &mix, &fti, dat.len() as u64).is_err());
        }
        for len in [0, 1, 976, 1229, 1231, inf.len() - 1] {
            assert!(parse(&inf[..len], &mix, &fti, dat.len() as u64).is_err());
        }
        let mut bad_mix = mix.clone();
        bad_mix[6..8].copy_from_slice(&2u16.to_le_bytes());
        assert!(parse(&inf, &bad_mix, &fti, dat.len() as u64).is_err());
        for pointer in [1, 187, u32::MAX] {
            let mut b = mix.clone();
            b[10..14].copy_from_slice(&pointer.to_le_bytes());
            assert!(parse(&inf, &b, &fti, dat.len() as u64).is_err());
        }
        for length in [0, u32::MAX, dat.len() as u32 + 1] {
            let mut b = fti.clone();
            b[174..178].copy_from_slice(&length.to_le_bytes());
            assert!(parse(&inf, &mix, &b, dat.len() as u64).is_err());
        }
        let mut b = inf.clone();
        b[1310..1312].copy_from_slice(b"12");
        assert!(parse(&b, &mix, &fti, dat.len() as u64).is_err());
        let mut b = inf.clone();
        b[1316..1322].copy_from_slice(b"local\0");
        let p = parse(&b, &mix, &fti, dat.len() as u64).unwrap();
        assert_eq!(p.messages.len(), 1);
        assert!(validate_packet(&p).is_err());
        let mut b = mix.clone();
        b[14..16].copy_from_slice(b"12");
        assert!(parse(&inf, &b, &fti, dat.len() as u64).is_err());
        assert!(parse(&inf, &mix, &fti[..185], dat.len() as u64).is_err());
        let mut duplicate_fti = fti.clone();
        duplicate_fti.extend_from_slice(&fti);
        let mut b = mix.clone();
        b[6] = 2;
        assert!(parse(&inf, &b, &duplicate_fti, 100).is_err());
    }

    #[test]
    fn reject_overlapping_and_unindexed_records() {
        let (inf, mut mix, fti, dat) = incoming(3, false);
        mix[20] = 1;
        mix[24..28].fill(0);
        assert!(parse(&inf, &mix, &fti, dat.len() as u64).is_err());
        let mut fti2 = fti.clone();
        fti2.extend_from_slice(&fti);
        mix[24..28].copy_from_slice(&186u32.to_le_bytes());
        assert!(parse(&inf, &mix, &fti2, dat.len() as u64).is_err());
        mix[20] = 0;
        assert!(parse(&inf, &mix, &fti2, dat.len() as u64).is_err());
    }

    #[test]
    fn preserve_unspecified_dates_and_host_limits() {
        let (mut inf, mix, mut fti, dat) = incoming(3, false);
        inf[985] = 12;
        inf[986] = 30;
        fti[144..164].fill(0);
        fti[144..152].copy_from_slice(b"bbs date");
        let p = parse(&inf, &mix, &fti, dat.len() as u64).unwrap();
        assert_eq!((p.info.from_to_limit, p.info.subject_limit), (12, 30));
        assert_eq!(p.messages[0].date_raw, b"bbs date");
        assert!(!p.messages[0].date_known);
        inf[985] = 255;
        inf[986] = 255;
        let p = parse(&inf, &mix, &fti, dat.len() as u64).unwrap();
        assert_eq!((p.info.from_to_limit, p.info.subject_limit), (35, 71));
    }

    #[test]
    fn posting_identity_and_header_capabilities() {
        let r = reply();
        for flags in [
            0,
            INF_POST | INF_NO_PRIVATE,
            INF_POST | INF_NETMAIL,
            INF_POST | INF_NO_HIGHBIT,
            INF_POST | INF_ALIAS_NAME,
        ] {
            let mut p = packet();
            p.areas[0].flags = flags;
            assert!(reply_files(&p, &[r.clone()]).is_err());
        }
        let mut p = packet();
        let mut public = r.clone();
        public.private = false;
        p.areas[0].flags |= INF_NO_PUBLIC;
        assert!(reply_files(&p, &[public]).is_err());
        p = packet();
        p.areas[0].flags |= INF_ALIAS_NAME;
        let mut alias = r.clone();
        alias.from = b"Ally".to_vec();
        assert!(reply_files(&p, &[alias]).is_ok());
        p = packet();
        p.areas[0].flags |= INF_ANY_NAME;
        let mut any = r.clone();
        any.from = b"Somebody".to_vec();
        assert!(reply_files(&p, &[any]).is_ok());
        for (name, max) in [("from", 35), ("to", 35), ("subject", 71)] {
            let mut b = r.clone();
            let v = match name {
                "from" => &mut b.from,
                "to" => &mut b.to,
                _ => &mut b.subject,
            };
            *v = vec![b'A'; max + 1];
            assert!(reply_files(&p, &[b]).is_err());
        }
        let mut b = r;
        b.body.push(0);
        assert!(reply_files(&p, &[b]).is_err());
    }

    #[test]
    fn importer_rejects_foreign_missing_duplicate_and_unsupported_records() {
        let p = packet();
        for offset in [116, 160, 256 + 177, 256 + 219] {
            let mut files = external_upl(false);
            files[0].1[offset] = b'X';
            assert!(read_replies(&p, &files).is_err());
        }
        let mut files = external_upl(false);
        files[0].0 = "OTHER.UPL".into();
        assert!(read_replies(&p, &files).is_err());
        let mut files = external_upl(false);
        files[1].0 = "missing.MSG".into();
        assert!(read_replies(&p, &files).is_err());
        let mut files = external_upl(false);
        files.push(("00001.msg".into(), vec![]));
        assert!(read_replies(&p, &files).is_err());
        let mut files = external_upl(false);
        files.push(("TEST.UPI".into(), vec![0; 55]));
        assert!(read_replies(&p, &files).is_err());
        for bit in [4u16, 8, 16, 64] {
            let mut files = external_upl(false);
            files[0].1[408..410].copy_from_slice(&bit.to_le_bytes());
            assert!(read_replies(&p, &files).is_err());
        }
        let mut files = external_upl(false);
        files[0].1[408] = 1;
        files.remove(1);
        assert!(read_replies(&p, &files).unwrap().is_empty());
        let mut files = external_upl(false);
        let record = files[0].1[256..].to_vec();
        files[0].1.extend_from_slice(&record);
        assert!(read_replies(&p, &files).is_err());
        let mut files = external_upl(false);
        files[0].1[420..433].copy_from_slice(b"../SECRET.MSG");
        assert!(read_replies(&p, &files).is_err());
    }

    #[test]
    fn importer_rejects_truncation_and_ignores_controls() {
        let p = packet();
        let baseline = external_upl(false);
        for n in [0, 115, 255, 257, 575] {
            let mut files = baseline.clone();
            files[0].1.truncate(n);
            assert!(read_replies(&p, &files).is_err());
        }
        for offset in [112, 114] {
            let mut files = baseline.clone();
            files[0].1[offset..offset + 2].copy_from_slice(&1u16.to_le_bytes());
            assert!(read_replies(&p, &files).is_err());
        }
        let mut files = baseline;
        files.push(("TEST.OLC".into(), b"not a draft".to_vec()));
        files.push(("TEST.PDQ".into(), vec![0; 3]));
        assert_eq!(read_replies(&p, &files).unwrap(), vec![reply()]);
        assert!(read_replies(&p, &[]).is_err());
    }

    #[test]
    fn invalid_packet_ids_dates_and_long_identity_are_explicit() {
        let (mut inf, mix, fti, dat) = incoming(3, false);
        for id in [b"../BAD".as_slice(), b"BAD/ID", b"BAD.ID", b""] {
            inf[987..996].fill(0);
            inf[987..987 + id.len()].copy_from_slice(id);
            assert!(parse(&inf, &mix, &fti, dat.len() as u64).is_err());
        }
        let mut p = packet();
        let mut r = reply();
        assert_eq!(TO_LIMIT, 35);
        for timestamp in [i32::MIN as i64 - 1, i32::MAX as i64 + 1] {
            r.unix_time = timestamp;
            assert!(reply_files(&p, &[r.clone()]).is_err());
        }
        r = reply();
        p.info.user_name = vec![b'A'; 42];
        r.from = vec![b'A'; 35];
        assert_eq!(read_replies(&p, &reply_files(&p, &[r.clone()]).unwrap()).unwrap(), vec![r]);
        let mut files = external_upl(false);
        files[0].1[112..116].fill(0);
        assert_eq!(read_replies(&packet(), &files).unwrap(), vec![reply()]);
        let mut files = external_upl(false);
        files[0].1[476..576].fill(b'X');
        assert!(read_replies(&packet(), &files).is_err());
        assert!(read_replies(&packet(), &[("TEST.UPI".into(), vec![0; 55]), ("TEST.NET".into(), vec![0; 231])]).is_err());
        assert!(read_replies(&packet(), &[("TEST.UPI".into(), vec![0; 55]), ("TEST.NET".into(), vec![0; 232])]).is_err());
    }

    #[test]
    fn bbs_display_uses_cp437_and_signed_dat_offsets_are_rejected() {
        let (mut inf, mix, mut fti, dat) = incoming(3, false);
        inf[235] = 0x82;
        let p = parse(&inf, &mix, &fti, dat.len() as u64).unwrap();
        assert!(p.info.bbs_name.starts_with('é'));
        fti[170..174].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(&inf, &mix, &fti, u64::MAX).is_err());
    }

    #[test]
    fn both_dat_marker_length_conventions_and_ambiguous_gaps() {
        let (inf, mix, mut fti, dat) = incoming(2, false);
        fti[174..178].copy_from_slice(&(dat.len() as u32 - 1).to_le_bytes());
        let p = parse(&inf, &mix, &fti, dat.len() as u64).unwrap();
        assert_eq!((p.messages[0].body_offset, p.messages[0].body_len), (1, 9));
        assert!(parse(&inf, &mix, &fti, dat.len() as u64 + 2).is_err());
        fti[174..178].fill(0);
        let p = parse(&inf, &mix, &fti, 1).unwrap();
        assert_eq!((p.messages[0].body_offset, p.messages[0].body_len), (1, 0));
    }

    #[test]
    fn public_preflight_exposes_legacy_and_posting_policy_without_writing() {
        let mut p = packet();
        let mut r = reply();
        r.body.push(0xe3);
        r.body.push(0x8d);
        r.reply_to = u32::MAX;
        assert!(validate_reply(&p, &r).is_ok());
        assert_eq!(read_replies(&p, &reply_files(&p, &[r.clone()]).unwrap()).unwrap(), vec![r.clone()]);
        p.info.uses_upl = false;
        assert!(validate_reply(&p, &r).is_err());
        r.reply_to = 0;
        assert!(validate_reply(&p, &r).is_ok());
        assert_eq!(read_replies(&p, &reply_files(&p, &[r.clone()]).unwrap()).unwrap(), vec![r.clone()]);
        p.areas[0].flags &= !INF_POST;
        assert!(validate_reply(&p, &r).is_err());
    }

    #[test]
    fn shared_fixture_exports_requested_packet_capabilities() {
        for level in [2, 3] {
            for uses_upl in [false, true] {
                let files = fixture_files(level, uses_upl);
                assert_eq!(
                    files.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(),
                    ["TEST.INF", "TEST.MIX", "TEST.FTI", "TEST.DAT"]
                );
                let p = parse(&files[0].1, &files[1].1, &files[2].1, files[3].1.len() as u64).unwrap();
                assert_eq!((p.info.level, p.info.uses_upl), (level, uses_upl));
                assert_eq!(p.info.bbs_id, "TEST");
                assert_eq!((p.messages[0].area, p.messages[0].number), (12, 45));
            }
        }
    }

    #[test]
    fn posting_defaults_respect_alias_and_private_requirements() {
        let mut p = packet();
        assert_eq!(posting_defaults(&p, 12).unwrap(), (b"Alice".to_vec(), false));
        p.areas[0].flags |= INF_ALIAS_NAME | INF_NO_PUBLIC;
        assert_eq!(posting_defaults(&p, 12).unwrap(), (b"Ally".to_vec(), true));
        let mut r = reply();
        (r.from, r.private) = posting_defaults(&p, 12).unwrap();
        assert!(validate_reply(&p, &r).is_ok());
        p.areas[0].flags |= INF_NO_PRIVATE;
        assert!(posting_defaults(&p, 12).is_err());
        p.areas[0].flags = INF_POST | INF_ALIAS_NAME;
        p.info.alias.clear();
        assert!(posting_defaults(&p, 12).is_err());
        p.areas[0].flags |= INF_ANY_NAME;
        assert_eq!(posting_defaults(&p, 12).unwrap().0, b"Alice");
        p.areas[0].flags = INF_POST;
        p.info.from_to_limit = 6;
        p.info.user_name = b"Alice Smith".to_vec();
        r.from = posting_defaults(&p, 12).unwrap().0;
        assert_eq!(r.from, b"Alice");
        assert!(validate_reply(&p, &r).is_ok());
        assert!(posting_defaults(&p, 99).is_err());
        for flags in [0, INF_POST | INF_NETMAIL] {
            p.areas[0].flags = flags;
            assert!(posting_defaults(&p, 12).is_err());
        }
    }
}
