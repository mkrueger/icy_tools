//! Message encodings are declared by FidoNet kludges, never guessed from byte validity.

use std::{
    borrow::Cow,
    collections::{BTreeMap, HashSet},
    sync::{LazyLock, Mutex},
};

use serde::{Deserialize, Serialize};

#[path = "charset_tables.rs"]
mod tables;

/// Logs each distinct decoding problem once; packets repeat the same one across thousands of messages.
fn warn_once(message: String) {
    static SEEN: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);
    let first = SEEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(message.clone());
    if first {
        log::warn!("{message}");
    }
}

/// Drops a UTF-8 sequence cut off by a fixed-width header field, so it does not show as U+FFFD.
pub fn trim_incomplete_utf8(bytes: &[u8]) -> &[u8] {
    let mut start = bytes.len();
    let mut continuation = 0;
    while continuation < 3 && start > 0 && bytes[start - 1] & 0xc0 == 0x80 {
        start -= 1;
        continuation += 1;
    }
    let needed = match start.checked_sub(1).map(|lead| bytes[lead]) {
        Some(0xc2..=0xdf) => 1,
        Some(0xe0..=0xef) => 2,
        Some(0xf0..=0xf4) => 3,
        _ => return bytes,
    };
    if continuation < needed {
        &bytes[..start - 1]
    } else {
        bytes
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Charset {
    #[default]
    Cp437,
    Utf8,
    Ascii,
    Dutch,
    Finnish,
    French,
    Canadian,
    German,
    Italian,
    Norwegian,
    Portuguese,
    Spanish,
    Swedish,
    Swiss,
    Uk,
    Cp850,
    Cp852,
    Cp866,
    Cp848,
    Cp1250,
    Cp1251,
    Cp1252,
    Cp10000,
    Cp10029,
    Cp10007,
    Latin1,
    Latin2,
    Latin5,
    Latin9,
    Koi8R,
    Koi8U,
    Iso8859_5,
}

impl Charset {
    pub const ALL: [Self; 32] = [
        Self::Cp437,
        Self::Utf8,
        Self::Ascii,
        Self::Dutch,
        Self::Finnish,
        Self::French,
        Self::Canadian,
        Self::German,
        Self::Italian,
        Self::Norwegian,
        Self::Portuguese,
        Self::Spanish,
        Self::Swedish,
        Self::Swiss,
        Self::Uk,
        Self::Cp850,
        Self::Cp852,
        Self::Cp866,
        Self::Cp848,
        Self::Cp1250,
        Self::Cp1251,
        Self::Cp1252,
        Self::Cp10000,
        Self::Cp10029,
        Self::Cp10007,
        Self::Latin1,
        Self::Latin2,
        Self::Latin5,
        Self::Latin9,
        Self::Koi8R,
        Self::Koi8U,
        Self::Iso8859_5,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Cp437 => "CP437",
            Self::Utf8 => "UTF-8",
            Self::Ascii => "ASCII",
            Self::Dutch => "DUTCH",
            Self::Finnish => "FINNISH",
            Self::French => "FRENCH",
            Self::Canadian => "CANADIAN",
            Self::German => "GERMAN",
            Self::Italian => "ITALIAN",
            Self::Norwegian => "NORWEIG",
            Self::Portuguese => "PORTU",
            Self::Spanish => "SPANISH",
            Self::Swedish => "SWEDISH",
            Self::Swiss => "SWISS",
            Self::Uk => "UK",
            Self::Cp850 => "CP850",
            Self::Cp852 => "CP852",
            Self::Cp866 => "CP866",
            Self::Cp848 => "CP848",
            Self::Cp1250 => "CP1250",
            Self::Cp1251 => "CP1251",
            Self::Cp1252 => "CP1252",
            Self::Cp10000 => "CP10000",
            Self::Cp10029 => "CP10029",
            Self::Cp10007 => "CP10007",
            Self::Latin1 => "LATIN-1",
            Self::Latin2 => "LATIN-2",
            Self::Latin5 => "LATIN-5",
            Self::Latin9 => "LATIN-9",
            Self::Koi8R => "KOI8-R",
            Self::Koi8U => "KOI8-U",
            Self::Iso8859_5 => "ISO-8859-5",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        let mut words = value.split_ascii_whitespace();
        let name = words.next()?;
        if let Some(level) = words.next() {
            if !level.bytes().all(|byte| byte.is_ascii_digit()) || words.next().is_some() {
                return None;
            }
        }
        match name.to_ascii_uppercase().as_str() {
            "CP437" | "IBM437" | "437" => Some(Self::Cp437),
            "UTF-8" | "UTF8" => Some(Self::Utf8),
            "US-ASCII" => Some(Self::Ascii),
            "ISO-10" | "ISO646-FI" => Some(Self::Finnish),
            "ISO646-NL" => Some(Self::Dutch),
            "ISO646-FR" => Some(Self::French),
            "ISO646-CA" => Some(Self::Canadian),
            "ISO646-DE" => Some(Self::German),
            "ISO646-IT" => Some(Self::Italian),
            "NORWEG" | "ISO646-NO" => Some(Self::Norwegian),
            "ISO646-PT" => Some(Self::Portuguese),
            "ISO646-ES" => Some(Self::Spanish),
            "ISO646-SE" => Some(Self::Swedish),
            "ISO646-CH" => Some(Self::Swiss),
            "ISO646-GB" => Some(Self::Uk),
            "IBM850" | "850" => Some(Self::Cp850),
            "IBM852" | "852" => Some(Self::Cp852),
            "+7_FIDO" | "IBM866" | "866" => Some(Self::Cp866),
            "IBM848" | "848" => Some(Self::Cp848),
            "WINDOWS-1250" | "WIN1250" | "1250" => Some(Self::Cp1250),
            "WINDOWS-1251" | "WIN1251" | "1251" => Some(Self::Cp1251),
            "WINDOWS-1252" | "WIN1252" | "1252" => Some(Self::Cp1252),
            "MAC" | "MACINTOSH" | "MACROMAN" | "10000" => Some(Self::Cp10000),
            "MAC-CENTRALEUROPE" | "10029" => Some(Self::Cp10029),
            "MAC-CYRILLIC" | "10007" => Some(Self::Cp10007),
            "ISO-8859-1" | "LATIN1" => Some(Self::Latin1),
            "ISO-8859-2" | "LATIN2" => Some(Self::Latin2),
            "ISO-8859-9" | "LATIN5" => Some(Self::Latin5),
            "ISO-8859-15" | "LATIN9" => Some(Self::Latin9),
            "KOI8" | "KOI8R" => Some(Self::Koi8R),
            "KOI8U" => Some(Self::Koi8U),
            "ISO8859-5" => Some(Self::Iso8859_5),
            _ => Self::ALL.into_iter().find(|charset| charset.name().eq_ignore_ascii_case(name)),
        }
    }

    pub fn decode(self, bytes: &[u8]) -> Cow<'_, str> {
        match self {
            Self::Cp437 => Cow::Owned(
                bytes
                    .iter()
                    .map(|&byte| if byte < 0x80 { char::from(byte) } else { crate::editor::cp437_char(byte) })
                    .collect(),
            ),
            Self::Utf8 => {
                if std::str::from_utf8(bytes).is_err() {
                    warn_once("Invalid UTF-8 in message text; replacing invalid sequences".to_string());
                }
                String::from_utf8_lossy(bytes)
            }
            charset => {
                let table = charset.table();
                let mut invalid = false;
                let text = bytes
                    .iter()
                    .map(|&byte| {
                        let ch = table[usize::from(byte)];
                        invalid |= ch == '\u{fffd}';
                        ch
                    })
                    .collect();
                if invalid {
                    warn_once(format!("Undefined bytes in {} message text; replacing invalid characters", charset.name()));
                }
                Cow::Owned(text)
            }
        }
    }

    fn table(self) -> &'static [char; 256] {
        match self {
            Self::Ascii => &tables::ASCII,
            Self::Dutch => &tables::DUTCH,
            Self::Finnish | Self::Swedish => &tables::SWEDISH,
            Self::French => &tables::FRENCH,
            Self::Canadian => &tables::CANADIAN,
            Self::German => &tables::GERMAN,
            Self::Italian => &tables::ITALIAN,
            Self::Norwegian => &tables::NORWEGIAN,
            Self::Portuguese => &tables::PORTUGUESE,
            Self::Spanish => &tables::SPANISH,
            Self::Swiss => &tables::SWISS,
            Self::Uk => &tables::UK,
            Self::Cp850 => &tables::CP850,
            Self::Cp852 => &tables::CP852,
            Self::Cp866 => &tables::CP866,
            Self::Cp848 => &tables::CP848,
            Self::Cp1250 => &tables::CP1250,
            Self::Cp1251 => &tables::CP1251,
            Self::Cp1252 => &tables::CP1252,
            Self::Cp10000 => &tables::CP10000,
            Self::Cp10029 => &tables::CP10029,
            Self::Cp10007 => &tables::CP10007,
            Self::Latin1 => &tables::LATIN1,
            Self::Latin2 => &tables::LATIN2,
            Self::Latin5 => &tables::LATIN5,
            Self::Latin9 => &tables::LATIN9,
            Self::Koi8R => &tables::KOI8_R,
            Self::Koi8U => &tables::KOI8_U,
            Self::Iso8859_5 => &tables::ISO8859_5,
            Self::Cp437 | Self::Utf8 => unreachable!("CP437 and UTF-8 have dedicated decoders"),
        }
    }

    /// Transcode text without changing escape syntax, including ISO 646's redefined brackets.
    pub fn decode_ansi(self, bytes: &[u8]) -> Cow<'_, str> {
        if !matches!(
            self,
            Self::Dutch
                | Self::Finnish
                | Self::French
                | Self::Canadian
                | Self::German
                | Self::Italian
                | Self::Norwegian
                | Self::Portuguese
                | Self::Spanish
                | Self::Swedish
                | Self::Swiss
                | Self::Uk
        ) {
            return self.decode(bytes);
        }
        let mut output = String::new();
        let mut offset = 0;
        while offset < bytes.len() {
            let start = offset;
            if bytes[offset] != 0x1b {
                offset += bytes[offset..].iter().position(|&byte| byte == 0x1b).unwrap_or(bytes.len() - offset);
                output.push_str(&self.decode(&bytes[start..offset]));
                continue;
            }
            offset += 1;
            match bytes.get(offset) {
                Some(b'[') => {
                    offset += 1;
                    while let Some(&byte) = bytes.get(offset) {
                        offset += 1;
                        if (0x40..=0x7e).contains(&byte) {
                            break;
                        }
                    }
                }
                Some(b']' | b'P' | b'X' | b'^' | b'_') => {
                    let osc = bytes[offset] == b']';
                    offset += 1;
                    while offset < bytes.len() {
                        if osc && bytes[offset] == 7 {
                            offset += 1;
                            break;
                        }
                        if bytes[offset..].starts_with(b"\x1b\\") {
                            offset += 2;
                            break;
                        }
                        offset += 1;
                    }
                }
                _ => {
                    while bytes.get(offset).is_some_and(|byte| (0x20..=0x2f).contains(byte)) {
                        offset += 1;
                    }
                    offset = (offset + 1).min(bytes.len());
                }
            }
            output.extend(bytes[start..offset].iter().map(|&byte| char::from(byte)));
        }
        Cow::Owned(output)
    }

    pub fn message_text(self, bytes: &[u8]) -> String {
        self.decode_ansi(bytes)
            .chars()
            .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\x1b'))
            .collect()
    }

    /// Encode the generated header of a raw message export in the body's encoding.
    pub fn encode_header(self, text: &str) -> Result<Vec<u8>, String> {
        if self == Self::Utf8 {
            return Ok(text.as_bytes().to_vec());
        }
        text.chars()
            .map(|ch| {
                let byte = if self == Self::Cp437 {
                    crate::editor::cp437_byte(ch)
                } else if ch == '\u{fffd}' {
                    None
                } else {
                    self.table().iter().position(|&mapped| mapped == ch).map(|index| index as u8)
                };
                byte.ok_or_else(|| format!("Message header character U+{:04X} cannot be saved as {}", u32::from(ch), self.name()))
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DecodeOptions {
    pub default: Charset,
    pub override_charset: Option<Charset>,
}

impl DecodeOptions {
    pub fn detect(self, bytes: &[u8]) -> Charset {
        self.override_charset.or_else(|| declared(bytes)).unwrap_or(self.default)
    }
}

/// Decode only printable runs: ISO 646 replaces punctuation also used by ANSI syntax.
pub(crate) struct CharsetParser {
    pub parser: icy_parser_core::AnsiParser,
    pub charset: Charset,
}

impl icy_parser_core::CommandParser for CharsetParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn icy_parser_core::CommandSink) {
        use icy_parser_core::*;

        struct DecodingSink<'a> {
            target: &'a mut dyn CommandSink,
            charset: Charset,
        }
        impl CommandSink for DecodingSink<'_> {
            fn print(&mut self, text: &[u8]) {
                self.target.print(self.charset.decode(text).as_bytes());
            }
            fn emit(&mut self, command: TerminalCommand) {
                self.target.emit(command);
            }
            fn device_control(&mut self, command: DeviceControlString) {
                self.target.device_control(command);
            }
            fn operating_system_command(&mut self, command: OperatingSystemCommand) {
                self.target.operating_system_command(command);
            }
            fn aps(&mut self, data: &[u8]) {
                self.target.aps(data);
            }
            fn play_music(&mut self, music: AnsiMusic) {
                self.target.play_music(music);
            }
            fn request(&mut self, request: TerminalRequest) {
                self.target.request(request);
            }
            fn report_error(&mut self, error: ParseError, level: ErrorLevel) {
                self.target.report_error(error, level);
            }
        }
        self.parser.parse(
            input,
            &mut DecodingSink {
                target: sink,
                charset: self.charset,
            },
        );
    }
}

pub fn declared(bytes: &[u8]) -> Option<Charset> {
    let mut primary = None;
    let mut codepage = None;
    let mut has_primary = false;
    let mut ibmpc = false;
    for line in bytes.split(|byte| matches!(byte, b'\r' | b'\n')) {
        let Some(line) = line.strip_prefix(b"\x01") else { continue };
        let Some(colon) = line.iter().position(|&byte| byte == b':') else { continue };
        let Ok(name) = std::str::from_utf8(&line[..colon]) else { continue };
        if ["CHRS", "CHARSET", "CODEPAGE"].iter().any(|key| name.trim().eq_ignore_ascii_case(key)) {
            let value = match std::str::from_utf8(&line[colon + 1..]) {
                Ok(value) => value,
                Err(_) => {
                    warn_once("Non-ASCII message charset declaration; using the reading default".to_string());
                    return None;
                }
            };
            let charset = Charset::parse(value);
            let is_ibmpc = value.split_ascii_whitespace().next().is_some_and(|name| name.eq_ignore_ascii_case("IBMPC"));
            // IBMPC is a valid obsolete name for "some IBM code page", resolved by CODEPAGE or the default.
            if charset.is_none() && !is_ibmpc {
                warn_once(format!(
                    "Unsupported or malformed message charset {:?}; using the reading default",
                    value.trim()
                ));
            }
            if name.trim().eq_ignore_ascii_case("CODEPAGE") {
                codepage = charset;
            } else if !has_primary {
                primary = charset;
                has_primary = true;
                ibmpc = is_ibmpc;
            }
        }
    }
    if !has_primary || ibmpc {
        codepage.or(primary)
    } else {
        primary
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReadingEncodings {
    pub default: Charset,
    /// Keys include the board identity so conference numbers cannot leak between boards.
    pub conferences: BTreeMap<String, Charset>,
    #[serde(skip)]
    pub messages: BTreeMap<usize, Charset>,
}

impl ReadingEncodings {
    /// The saved settings, without the per-message overrides that last only until the packet is reopened.
    pub fn persistent(&self) -> Self {
        Self {
            default: self.default,
            conferences: self.conferences.clone(),
            messages: BTreeMap::new(),
        }
    }

    pub fn conference_key(package: &crate::qwk::QwkPackage, conference: u16) -> String {
        let board = if package.control_file.bbs_id.is_empty() {
            package.bbs_name.as_bytes()
        } else {
            &package.control_file.bbs_id
        };
        format!("{}:{conference}", board.iter().map(|byte| format!("{byte:02x}")).collect::<String>())
    }

    pub fn options(&self, package: &crate::qwk::QwkPackage, index: usize) -> DecodeOptions {
        let default = package
            .infos
            .get(index)
            .and_then(|info| self.conferences.get(&Self::conference_key(package, info.conference)))
            .copied()
            .unwrap_or(self.default);
        DecodeOptions {
            default,
            override_charset: self.messages.get(&index).copied(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ftsc_identifier_and_obsolete_alias_is_recognized() {
        for charset in Charset::ALL {
            assert_eq!(Charset::parse(charset.name()), Some(charset));
            assert_eq!(Charset::parse(&format!("{} 2", charset.name().to_ascii_lowercase())), Some(charset));
            let saved = toml::to_string(&ReadingEncodings {
                default: charset,
                ..Default::default()
            })
            .unwrap();
            assert_eq!(toml::from_str::<ReadingEncodings>(&saved).unwrap().default, charset);
        }
        for (alias, expected) in [
            ("ISO-10", Charset::Finnish),
            ("+7_FIDO", Charset::Cp866),
            ("MAC", Charset::Cp10000),
            ("IBM848", Charset::Cp848),
            ("WINDOWS-1251", Charset::Cp1251),
            ("KOI8", Charset::Koi8R),
            ("KOI8U", Charset::Koi8U),
        ] {
            assert_eq!(Charset::parse(alias), Some(expected));
        }
        assert_eq!(Charset::parse("IBMPC"), None, "IBMPC does not identify a specific code page");
    }

    #[test]
    fn national_variants_decode_replacement_positions_without_rewriting_ansi() {
        for (charset, expected) in [
            (Charset::Dutch, "\u{a3}$\u{be}\u{133}\u{bd}|^_`\u{a8}\u{192}\u{bc}\u{b4}"),
            (Charset::Finnish, "#¤@ÄÖÅ^_`äöå‾"),
            (Charset::French, "£$à°ç§^_µéùè¨"),
            (Charset::Canadian, "#$àâçêî_ôéùèû"),
            (Charset::German, "#$§ÄÖÜ^_`äöüß"),
            (Charset::Italian, "£$§°çé^_ùàòèì"),
            (Charset::Norwegian, "#$@ÆØÅ^_`æøå‾"),
            (Charset::Portuguese, "#$§ÃÇÕ^_`ãçõ°"),
            (Charset::Spanish, "£$§¡Ñ¿^_`°ñç~"),
            (Charset::Swedish, "#¤@ÄÖÅ^_`äöå‾"),
            (Charset::Swiss, "ù$àéçêîèôäöüû"),
            (Charset::Uk, "£$@[\\]^_`{|}‾"),
        ] {
            let bytes = b"#$@[\\]^_`{|}~";
            assert_eq!(charset.decode(bytes), expected, "{}", charset.name());
            let mut ansi = b"\x1b[31m".to_vec();
            ansi.extend(bytes);
            ansi.extend(b"\x1b[0m");
            assert_eq!(charset.decode_ansi(&ansi), format!("\x1b[31m{expected}\x1b[0m"));
            assert_eq!(crate::text::HeaderText::with_charset(&ansi, charset).as_str(), expected);
            let screen = crate::reader::render_body_encoded(&ansi, charset, 80).unwrap();
            let styled = crate::text::styled_lines(&screen);
            assert_eq!(styled[0].iter().map(|span| span.text.as_str()).collect::<String>(), expected);
            assert!(styled[0].iter().any(|span| span.foreground.is_some()), "ANSI syntax must not be recoded");
            assert_eq!(crate::reader::body_search_text_encoded(&ansi, charset), expected.to_lowercase());
        }
    }

    #[test]
    fn eight_bit_code_pages_decode_and_render_as_unicode() {
        for (charset, bytes, expected) in [
            (Charset::Cp850, b"\x9b\x82".as_slice(), "øé"),
            (Charset::Cp852, b"\x9d\x9f", "Łč"),
            (Charset::Cp866, b"\x8f\xe0\xa8\xa2\xa5\xe2", "Привет"),
            (Charset::Cp848, b"\xf2\xf3\xf4\xf5\xf6\xf7", "ҐґЄєІі"),
            (Charset::Cp1250, b"\xa3\xe8", "Łč"),
            (Charset::Cp1251, b"\xcf\xf0\xe8\xe2\xe5\xf2", "Привет"),
            (Charset::Cp1252, b"\x80\xe9", "€é"),
            (Charset::Cp10000, b"\x8e\xf0", "é\u{f8ff}"),
            (Charset::Cp10029, b"\x81\x82", "Āā"),
            (Charset::Cp10007, b"\x80\xe0", "Аа"),
            (Charset::Latin1, b"\xe9", "é"),
            (Charset::Latin2, b"\xa3\xe8", "Łč"),
            (Charset::Latin5, b"\xd0\xfd", "Ğı"),
            (Charset::Latin9, b"\xa4\xbd", "€œ"),
            (Charset::Koi8R, b"\xf0\xd2\xc9\xd7\xc5\xd4", "Привет"),
            (Charset::Koi8U, b"\xa4\xa6\xa7\xad", "єіїґ"),
            (Charset::Iso8859_5, b"\xbf\xe0\xd8\xd2\xd5\xe2", "Привет"),
            (Charset::Ascii, b"ASCII", "ASCII"),
        ] {
            assert_eq!(charset.decode(bytes), expected, "{}", charset.name());
            assert_eq!(charset.encode_header(expected).unwrap(), bytes);
            let screen = crate::reader::render_body_encoded(bytes, charset, 80).unwrap();
            let text: String = crate::text::styled_lines(&screen).iter().flatten().map(|span| span.text.as_str()).collect();
            assert_eq!(text, expected, "{}", charset.name());
            assert_eq!(crate::reader::body_search_text_encoded(bytes, charset), expected.to_lowercase());
            assert_eq!(crate::text::HeaderText::with_charset(bytes, charset).as_str(), expected);
        }
        assert_eq!(Charset::Ascii.decode(b"\x80"), "\u{fffd}");
        assert_eq!(Charset::Cp1252.decode(b"\x81"), "\u{fffd}");
        assert_eq!(Charset::Latin1.decode(b"\x80"), "\u{80}", "Latin-1 is not Windows-1252");
    }

    #[test]
    fn incomplete_trailing_utf8_is_trimmed_and_valid_text_kept() {
        assert_eq!(trim_incomplete_utf8("Лип".as_bytes()), "Лип".as_bytes());
        assert_eq!(trim_incomplete_utf8(b"Lip\xd0"), b"Lip");
        assert_eq!(trim_incomplete_utf8(b"A\xe3\x81"), b"A");
        assert_eq!(trim_incomplete_utf8(b"A\xf0\x9f\x98"), b"A");
        assert_eq!(trim_incomplete_utf8("A😀".as_bytes()), "A😀".as_bytes());
        assert_eq!(
            trim_incomplete_utf8(b"\x82\x82"),
            b"\x82\x82",
            "stray continuation bytes are left to the decoder"
        );
        assert_eq!(trim_incomplete_utf8(b""), b"");
        assert_eq!(crate::text::HeaderText::with_declared(b"Lip\xd0 ", Some(Charset::Utf8)).as_str(), "Lip");
    }

    #[test]
    fn persistent_encodings_exclude_per_message_overrides() {
        let mut encodings = ReadingEncodings {
            default: Charset::Cp866,
            ..Default::default()
        };
        encodings.conferences.insert("board:1".into(), Charset::Utf8);
        encodings.messages.insert(3, Charset::Koi8R);
        let persistent = encodings.persistent();
        assert!(persistent.messages.is_empty());
        assert_eq!((persistent.default, persistent.conferences), (encodings.default, encodings.conferences));
    }

    #[test]
    fn ibmpc_uses_codepage_regardless_of_kludge_order() {
        for body in [
            b"\x01CHRS: IBMPC 2\n\x01CODEPAGE: 866\n".as_slice(),
            b"\x01CODEPAGE: 866\n\x01CHARSET: IBMPC 2\n",
            b"\x01CODEPAGE: 866\n",
        ] {
            assert_eq!(declared(body), Some(Charset::Cp866));
        }
        assert_eq!(declared(b"\x01CODEPAGE: 866\n\x01CHRS: CP1251 2\n"), Some(Charset::Cp1251));
        assert_eq!(declared(b"\x01CHRS: UNKNOWN 2\n\x01CODEPAGE: 866\n"), None);
    }

    #[test]
    fn national_recoding_preserves_control_strings_and_reports_unencodable_headers() {
        let input = b"\x1b]8;;https://example.org/[path]\x1b\\[\\]\x1b]8;;\x07\x1bPq#1;2;100;0;0~\x1b\\";
        let decoded = Charset::German.decode_ansi(input);
        assert_eq!(decoded, "\x1b]8;;https://example.org/[path]\x1b\\ÄÖÜ\x1b]8;;\x07\x1bPq#1;2;100;0;0~\x1b\\");
        for charset in [Charset::Cp437, Charset::Cp1252, Charset::German, Charset::Ascii] {
            assert!(charset.encode_header("日本語").unwrap_err().contains(charset.name()));
        }
        assert_eq!(Charset::Utf8.encode_header("日本語").unwrap(), "日本語".as_bytes());
    }

    #[test]
    fn declarations_not_utf8_validity_select_the_encoding() {
        let options = DecodeOptions::default();
        assert_eq!(options.detect("Grüße".as_bytes()), Charset::Cp437);
        for key in ["CHRS", "charset", "CODEPAGE"] {
            assert_eq!(options.detect(format!("\x01{key}: UTF-8 4\r\nGrüße").as_bytes()), Charset::Utf8);
        }
        assert_eq!(options.detect(b"CHRS: UTF-8 4\n"), Charset::Cp437);
        for value in ["IBMPC 2", "+7 FIDO 2", "UTF-8 bad", "UNKNOWN 2"] {
            assert_eq!(options.detect(format!("\x01CHRS: {value}\n").as_bytes()), Charset::Cp437);
        }
        assert_eq!(
            DecodeOptions {
                default: Charset::Utf8,
                override_charset: None
            }
            .detect(b"\x01CHRS: IBM437 2\n"),
            Charset::Cp437
        );
        assert_eq!(
            DecodeOptions {
                override_charset: Some(Charset::Cp437),
                ..options
            }
            .detect(b"\x01CHRS: UTF8 4\n"),
            Charset::Cp437
        );
    }

    #[test]
    fn decoding_keeps_controls_and_replaces_invalid_utf8_without_cp437_fallback() {
        assert_eq!(Charset::Cp437.decode(b"\x1b[31m\x82\n"), "\x1b[31mé\n");
        assert_eq!(Charset::Utf8.decode(b"\xc3\xa9\xff"), "é\u{fffd}");
        assert_eq!(Charset::Cp437.decode(b"\xc3\xa9"), "├⌐");
    }

    #[test]
    fn conference_defaults_are_scoped_to_boards_and_message_overrides_are_not_saved() {
        let (_dir, package) = crate::qwk::tests::load();
        let mut encodings = ReadingEncodings::default();
        encodings
            .conferences
            .insert(ReadingEncodings::conference_key(&package, package.infos[0].conference), Charset::Utf8);
        assert_eq!(encodings.options(&package, 0).default, Charset::Utf8);
        let mut other = package.clone();
        other.control_file.bbs_id = b"OTHER".as_slice().into();
        assert_eq!(encodings.options(&other, 0).default, Charset::Cp437);
        encodings.messages.insert(0, Charset::Cp437);
        assert_eq!(encodings.options(&package, 0).override_charset, Some(Charset::Cp437));
        let saved = toml::to_string(&encodings).unwrap();
        let restored: ReadingEncodings = toml::from_str(&saved).unwrap();
        assert!(restored.messages.is_empty());
        assert_eq!(restored.conferences, encodings.conferences);
    }

    #[test]
    fn utf8_qwk_reading_preserves_e3_characters_before_hiding_kludges() {
        let (dir, package) = crate::qwk::tests::load_utf8();
        let encodings = ReadingEncodings::default();
        let message = package.get_message(0).unwrap();
        let charset = encodings.options(&package, 0).detect(&message.text);
        assert_eq!(charset, Charset::Utf8);
        let bytes = package.text_for_charset(&message, 0, charset).unwrap();
        let text = package.readable_text(&bytes, false);
        assert!(!text.windows(4).any(|part| part == b"CHRS"));
        assert!(charset.decode(&text).contains("Grüße Привет あ"));
        let screen = crate::reader::render_body_encoded(&text, charset, 80).unwrap();
        let styled = crate::text::styled_lines(&screen);
        assert_eq!(styled[0].iter().map(|span| span.text.as_str()).collect::<String>(), "Grüße Привет あ");
        assert!(styled[0].iter().any(|span| span.foreground.is_some()));
        assert!(crate::reader::body_search_text_encoded(&text, charset).contains("grüße привет あ"));
        let other = package.get_message(1).unwrap();
        assert_eq!(encodings.options(&package, 1).detect(&other.text), Charset::Cp437);
        assert!(Charset::Cp437.decode(&other.text).starts_with("├⌐"));
        let destination = dir.path().join("utf8.txt");
        crate::transcript::save_transcript_encoded(&package, &[0, 1], &dir.path().join("UTF8.QWK"), &destination, &encodings).unwrap();
        let transcript = std::fs::read_to_string(destination).unwrap();
        assert!(transcript.contains("Grüße Привет あ"));
        assert!(transcript.contains("├⌐"));
    }
}
