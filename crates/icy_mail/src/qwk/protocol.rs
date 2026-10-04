//! Advertised QWK door capabilities and reply control formats.
//!
//! Wire formats follow the QWK Mail Packet File Layout §4.3 and QWKE 1.02:
//! DOOR.ID names control recipients/types; Qmail uses CONFIG body commands,
//! ordinary ADD/DROP controls use the subject in the target conference, and
//! QWKE uses TODOOR.EXT AREA records.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SubscriptionFormat {
    #[default]
    None,
    Subject,
    Config,
    Qwke,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub qwke: bool,
    pub subscription_support: bool,
    pub control_name: Option<String>,
    pub(crate) subscription_format: SubscriptionFormat,
}

impl Capabilities {
    pub fn supports_subscriptions(&self) -> bool {
        self.subscription_support
    }

    /// QWKE has variable length kludges; bound editable fields to 255 CP437
    /// bytes. Plain QWK always retains its fixed 25-byte header limit.
    pub fn header_limit(&self) -> usize {
        if self.qwke {
            255
        } else {
            25
        }
    }

    pub(crate) fn parse(files: &[(String, Vec<u8>)]) -> Self {
        let mut result = Self::default();
        let extended = files.iter().filter(|(name, _)| name.eq_ignore_ascii_case("TOREADER.EXT")).collect::<Vec<_>>();
        if extended.len() == 1 {
            let data = &extended[0].1;
            result.qwke = data.len() <= 64 * 1024
                && data.is_ascii()
                && data.iter().all(|b| !b.is_ascii_control() || matches!(b, b'\r' | b'\n' | b'\t'))
                && std::str::from_utf8(data).unwrap_or_default().lines().any(|line| {
                    let mut fields = line.split_ascii_whitespace();
                    fields.next() == Some("AREA") && fields.next().is_some_and(|n| n.parse::<u16>().is_ok()) && fields.next().is_some()
                });
        }
        let doors: Vec<_> = files.iter().filter(|(name, _)| name.eq_ignore_ascii_case("DOOR.ID")).collect();
        // Ambiguous or malformed advertisements must never enable commands.
        if doors.len() > 1 {
            return Self::default();
        }
        let data = doors.first().map_or(&[][..], |entry| entry.1.as_slice());
        if data.len() > 64 * 1024 || !data.is_ascii() || data.iter().any(|b| b.is_ascii_control() && !matches!(b, b'\r' | b'\n' | b'\t')) {
            return result;
        }
        let mut add = false;
        let mut drop = false;
        let mut door = String::new();
        for line in std::str::from_utf8(data).unwrap().lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let value = value.trim();
            match key.trim().to_ascii_uppercase().as_str() {
                "DOOR" => door = value.to_ascii_uppercase(),
                "CONTROLNAME" => {
                    if result.control_name.is_some() || value.is_empty() || value.len() > 25 || !value.bytes().all(|b| (b' '..=b'~').contains(&b)) {
                        return Self::default();
                    }
                    result.control_name = Some(value.to_owned());
                }
                "CONTROLTYPE" => match value.to_ascii_uppercase().as_str() {
                    "ADD" => add = true,
                    "DROP" => drop = true,
                    "QWKE" => result.qwke = true,
                    _ => {}
                },
                _ => {}
            }
        }
        result.subscription_format = if result.qwke {
            SubscriptionFormat::Qwke
        } else if add && drop && result.control_name.is_some() {
            if door.starts_with("QMAIL") || result.control_name.as_deref().is_some_and(|s| s.eq_ignore_ascii_case("QMAIL")) {
                SubscriptionFormat::Config
            } else {
                SubscriptionFormat::Subject
            }
        } else {
            SubscriptionFormat::None
        };
        result.subscription_support = result.subscription_format != SubscriptionFormat::None;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advertisements_are_explicit_and_bounded() {
        let parse = |data: &[u8]| Capabilities::parse(&[("door.id".into(), data.to_vec())]);
        assert!(!parse(b"DOOR=Unknown").subscription_support);
        assert!(!parse(b"CONTROLNAME=CONTROL\nCONTROLTYPE=ADD").subscription_support);
        assert!(parse(b"CONTROLNAME=CONTROL\nCONTROLTYPE=ADD\nCONTROLTYPE=DROP").subscription_support);
        assert_eq!(
            parse(b"CONTROLNAME=QMAIL\nCONTROLTYPE=ADD\nCONTROLTYPE=DROP").subscription_format,
            SubscriptionFormat::Config
        );
        assert!(parse(b"CONTROLTYPE=QWKE").qwke);
        assert!(!parse(b"CONTROLNAME=BAD\0\nCONTROLTYPE=QWKE").subscription_support);
        assert!(!parse(b"CONTROLNAME=ONE\nCONTROLNAME=TWO\nCONTROLTYPE=QWKE").subscription_support);
        assert!(!parse(&vec![b' '; 65_537]).qwke);
    }
}
