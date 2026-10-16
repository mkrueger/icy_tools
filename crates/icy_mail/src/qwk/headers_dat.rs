//! Synchronet's `HEADERS.DAT`: per-message header fields beside `MESSAGES.DAT`.
//!
//! An INI-style text file; each section is named by the hexadecimal byte offset of a message's
//! header block in `MESSAGES.DAT`. Its fields take precedence over the 128-byte header block and
//! QWKE kludge lines. `Utf8: true` marks a message whose header fields and body are UTF-8; such
//! bodies end lines with LF instead of QWK's `0xE3`. See <http://wiki.synchro.net/ref:qwk>.

use std::collections::HashMap;

/// Header fields IcyMail uses from one `HEADERS.DAT` section. Values are raw bytes, decoded as
/// UTF-8 when `utf8` is set and in the message's own encoding otherwise.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExtendedHeaders {
    pub utf8: bool,
    pub sender: Option<Vec<u8>>,
    pub to: Option<Vec<u8>>,
    pub subject: Option<Vec<u8>>,
}

/// Sections by the byte offset of their message's header block.
pub fn parse(data: &[u8]) -> HashMap<u64, ExtendedHeaders> {
    let mut sections = HashMap::new();
    let mut current: Option<u64> = None;
    for line in data.split(|&byte| byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let trimmed = line.trim_ascii();
        if trimmed.is_empty() || trimmed.starts_with(b";") {
            continue;
        }
        if let Some(name) = trimmed.strip_prefix(b"[").and_then(|rest| rest.strip_suffix(b"]")) {
            current = std::str::from_utf8(name).ok().and_then(|name| u64::from_str_radix(name.trim(), 16).ok());
            if current.is_none() {
                log::warn!("HEADERS.DAT section {:?} is not a hexadecimal message offset", String::from_utf8_lossy(name));
            }
            continue;
        }
        let Some(offset) = current else { continue };
        let Some((key, value)) = split_field(line) else { continue };
        let headers: &mut ExtendedHeaders = sections.entry(offset).or_default();
        match key.to_ascii_lowercase().as_slice() {
            b"utf8" => headers.utf8 = is_true(value),
            b"sender" => headers.sender = Some(value.to_vec()),
            b"to" => headers.to = Some(value.to_vec()),
            b"subject" => headers.subject = Some(value.to_vec()),
            _ => {}
        }
    }
    sections
}

/// `key: value` keeps trailing white-space; `key = value` is trimmed. Leading white-space of a
/// value is never significant.
fn split_field(line: &[u8]) -> Option<(&[u8], &[u8])> {
    let colon = line.iter().position(|&byte| byte == b':');
    let equals = line.iter().position(|&byte| byte == b'=');
    let (position, trim_end) = match (colon, equals) {
        (Some(colon), Some(equals)) if equals < colon => (equals, true),
        (Some(colon), _) => (colon, false),
        (None, Some(equals)) => (equals, true),
        (None, None) => return None,
    };
    let key = line[..position].trim_ascii();
    if key.is_empty() {
        return None;
    }
    let value = line[position + 1..].trim_ascii_start();
    Some((key, if trim_end { value.trim_ascii_end() } else { value }))
}

fn is_true(value: &[u8]) -> bool {
    let value = value.trim_ascii();
    ["true", "yes", "on", "1"].iter().any(|word| value.eq_ignore_ascii_case(word.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_are_hex_offsets_and_fields_follow_synchronet_syntax() {
        let data = "; comment\r\n\
                    [c80]\r\n\
                    Message-ID: <5D4AFDF1.40645.dove_dove-gen@somebbs.com>\r\n\
                    Utf8: true\r\n\
                    Sender: Dmitry Lipatnikov\r\n\
                    Subject:   Name in Cyrillic  \r\n\
                    [100]\n\
                    utf8 = FALSE\n\
                    to = All  \n\
                    [not-hex]\n\
                    Sender: ignored\n\
                    [180]\n\
                    Subject: Привет\n";
        let sections = parse(data.as_bytes());
        assert_eq!(sections.len(), 3);
        let first = &sections[&0xc80];
        assert!(first.utf8);
        assert_eq!(first.sender.as_deref(), Some(b"Dmitry Lipatnikov".as_slice()));
        assert_eq!(first.subject.as_deref(), Some(b"Name in Cyrillic  ".as_slice()), "`key:` keeps trailing spaces");
        let second = &sections[&0x100];
        assert!(!second.utf8);
        assert_eq!(second.to.as_deref(), Some(b"All".as_slice()), "`key =` is trimmed");
        assert_eq!(sections[&0x180].subject.as_deref(), Some("Привет".as_bytes()));
    }

    #[test]
    fn values_may_contain_separators_and_garbage_is_skipped() {
        let sections = parse(b"[0]\nSubject: Re: a=b\nnot a field\n= value\nTo = x: y\n");
        assert_eq!(sections[&0].subject.as_deref(), Some(b"Re: a=b".as_slice()));
        assert_eq!(sections[&0].to.as_deref(), Some(b"x: y".as_slice()));
        assert!(parse(b"Sender: outside any section\n").is_empty());
    }
}
