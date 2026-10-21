//! FidoNet message identity and timezone declarations shared by packets and JAM.

use chrono::{FixedOffset, Local, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    pub msgid: Option<Vec<u8>>,
    pub reply: Option<Vec<u8>>,
    /// Seconds east of UTC, as declared by TZUTC or TZUTCINFO.
    pub utc_offset: Option<i32>,
    /// Known instant, retained so cached dates can follow the reader's timezone.
    pub utc_timestamp: Option<i64>,
}

impl Metadata {
    pub fn parse(text: &[u8]) -> Self {
        let mut metadata = Self::default();
        for line in text.split(|byte| matches!(byte, b'\r' | b'\n')) {
            let Some(line) = line.strip_prefix(b"\x01") else { continue };
            let Some(colon) = line.iter().position(|&byte| byte == b':') else { continue };
            let name = line[..colon].trim_ascii();
            let value = line[colon + 1..].trim_ascii();
            if name.eq_ignore_ascii_case(b"MSGID") && !value.is_empty() && metadata.msgid.is_none() {
                metadata.msgid = Some(value.to_vec());
            } else if name.eq_ignore_ascii_case(b"REPLY") && !value.is_empty() && metadata.reply.is_none() {
                metadata.reply = Some(value.to_vec());
            } else if (name.eq_ignore_ascii_case(b"TZUTC") || name.eq_ignore_ascii_case(b"TZUTCINFO")) && metadata.utc_offset.is_none() {
                metadata.utc_offset = parse_offset(value);
                if metadata.utc_offset.is_none() {
                    log::warn!(
                        "Invalid FidoNet timezone offset {:?}; keeping the message's undetermined timezone",
                        String::from_utf8_lossy(value)
                    );
                }
            }
        }
        metadata
    }

    /// Undeclared dates keep their existing interpretation.
    pub fn local_date(&mut self, date: NaiveDateTime) -> NaiveDateTime {
        let Some(offset) = self.utc_offset.and_then(FixedOffset::east_opt) else {
            return date;
        };
        match offset.from_local_datetime(&date).single() {
            Some(date) => {
                self.utc_timestamp = Some(date.timestamp());
                date.with_timezone(&Local).naive_local()
            }
            None => {
                log::warn!("FidoNet date {date} is outside the supported range for offset {offset}");
                date
            }
        }
    }
}

fn parse_offset(value: &[u8]) -> Option<i32> {
    let (sign, digits) = match value.first()? {
        b'-' => (-1, &value[1..]),
        b'+' => (1, &value[1..]),
        _ => (1, value),
    };
    if digits.len() != 4 || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let hours = i32::from(digits[0] - b'0') * 10 + i32::from(digits[1] - b'0');
    let minutes = i32::from(digits[2] - b'0') * 10 + i32::from(digits[3] - b'0');
    if minutes >= 60 {
        return None;
    }
    let seconds = sign * (hours * 3600 + minutes * 60);
    FixedOffset::east_opt(seconds).map(|_| seconds)
}

pub fn format_offset(seconds: i32) -> String {
    let minutes = seconds.unsigned_abs() / 60;
    format!("{}{:02}{:02}", if seconds < 0 { "-" } else { "" }, minutes / 60, minutes % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_accept_legacy_names_signs_and_fractional_hours() {
        for (text, seconds) in [
            ("\x01TZUTC: 0000\r", 0),
            ("\x01TZUTC: 1400\n", 14 * 3600),
            ("\x01TZUTCINFO: -0330\r\n", -3 * 3600 - 30 * 60),
            ("\x01tzutc: +0545\n", 5 * 3600 + 45 * 60),
        ] {
            let mut metadata = Metadata::parse(text.as_bytes());
            assert_eq!(metadata.utc_offset, Some(seconds));
            let date = NaiveDateTime::parse_from_str("2026-01-01 01:15", "%Y-%m-%d %H:%M").unwrap();
            let expected = FixedOffset::east_opt(seconds)
                .unwrap()
                .from_local_datetime(&date)
                .unwrap()
                .with_timezone(&Local)
                .naive_local();
            assert_eq!(metadata.local_date(date), expected);
        }
        assert_eq!(format_offset(14 * 3600), "1400");
        assert_eq!(format_offset(-3 * 3600 - 30 * 60), "-0330");
        assert_eq!(format_offset(0), "0000");
    }

    #[test]
    fn malformed_offsets_and_ordinary_text_do_not_declare_metadata() {
        for value in ["", "100", "12345", "1260", "-2400", "ab00", "+-0100", "0100 extra"] {
            assert_eq!(Metadata::parse(format!("\x01TZUTC: {value}\n").as_bytes()).utc_offset, None);
        }
        assert_eq!(Metadata::parse(b"TZUTC: 0100\nMSGID: 2:1/1 abc\n"), Metadata::default());
        assert_eq!(Metadata::parse(b"\x01TZUTC: bad\r\x01TZUTCINFO: -0100\r").utc_offset, Some(-3600));
    }

    #[test]
    fn identities_remain_exact_including_quoted_addresses() {
        let metadata = Metadata::parse(b"\x01MSGID: \"an \"\"address\"\"\" 1234ABCD\r\x01REPLY: 2:1/1 000000ab\n");
        assert_eq!(metadata.msgid.as_deref(), Some(b"\"an \"\"address\"\"\" 1234ABCD".as_slice()));
        assert_eq!(metadata.reply.as_deref(), Some(b"2:1/1 000000ab".as_slice()));
    }
}
