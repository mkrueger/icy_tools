//! ANSI music documents: the CTerm "ANSI music" strings of `.ams` and `.mus` files as a
//! sequence of notes, rests and setting changes, and back.
//!
//! The editor keeps the music string as text, as the user typed it; the piano roll edits
//! the [`Event`]s parsed from it and writes the text anew.

use std::ops::Range;

use icy_parser_core::{AnsiMusic, MusicAction, MusicStyle, FREQ};

/// The highest note, `O6 B`; the lowest is `O0 C`, 0.
pub const HIGHEST_PITCH: u8 = 83;
pub const OCTAVES: u8 = 7;
pub const DEFAULT_TEMPO: u8 = 120;
pub const DEFAULT_OCTAVE: u8 = 4;
pub const DEFAULT_LENGTH: u8 = 4;
/// The note lengths music strings can give, from a whole note to a 64th.
pub const LENGTHS: [u8; 7] = [1, 2, 4, 8, 16, 32, 64];

/// CTerm keeps at most this many characters of a music string; longer music is split.
pub const MAX_STRING_LEN: usize = 1022;
/// The characters a music string may contain; any other one makes terminals discard it.
const MUSIC_CHARS: &[u8] = b"aAbBcCdDeEfFgGlLmMnNoOpPsStT0123456789.-+#<> ";
/// Line breaks and tabs only format the text; they are left out of the file.
const FORMATTING: &[u8] = b"\r\n\t";
const SCALE: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
/// Semitones of the letter notes within an octave.
const LETTERS: &[u8; 12] = b"C#D#EF#G#A#B";

/// The escape sequence that starts a music string.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Introducer {
    /// `ESC [ |`, SyncTERM's own; no terminal takes it for anything else.
    #[default]
    SyncTerm,
    /// `ESC [ N`, BananaCom's.
    Banana,
    /// `ESC [ M`, common in old files but also Delete Line.
    Conflicting,
}

impl Introducer {
    pub const ALL: [Introducer; 3] = [Introducer::SyncTerm, Introducer::Banana, Introducer::Conflicting];

    pub fn bytes(self) -> &'static [u8] {
        match self {
            Introducer::SyncTerm => b"\x1b[|",
            Introducer::Banana => b"\x1b[N",
            Introducer::Conflicting => b"\x1b[M",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Introducer::SyncTerm => "ESC [ |",
            Introducer::Banana => "ESC [ N",
            Introducer::Conflicting => "ESC [ M",
        }
    }

    /// After `ESC [ |` and `ESC [ M` a leading F, B, L, S or N is a music style, as if `M` came first.
    fn style_prefix(self) -> bool {
        self != Introducer::Banana
    }

    fn from_final(byte: u8) -> Option<Self> {
        match byte {
            b'|' => Some(Introducer::SyncTerm),
            b'N' => Some(Introducer::Banana),
            b'M' => Some(Introducer::Conflicting),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Articulation {
    /// Notes sound 7/8 of their length.
    #[default]
    Normal,
    /// Notes sound their full length.
    Legato,
    /// Notes sound 3/4 of their length.
    Staccato,
}

impl Articulation {
    pub const ALL: [Articulation; 3] = [Articulation::Normal, Articulation::Legato, Articulation::Staccato];

    fn code(self) -> &'static str {
        match self {
            Articulation::Normal => "MN",
            Articulation::Legato => "ML",
            Articulation::Staccato => "MS",
        }
    }
}

/// One step of a music string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// A note, or a rest without `pitch`. `length` is the note value: 4 is a quarter note.
    Note { pitch: Option<u8>, length: u8, dotted: bool },
    /// `T`: quarter notes per minute.
    Tempo(u8),
    /// `MN`, `ML`, `MS`.
    Articulation(Articulation),
    /// `MB` (true) and `MF`: whether the terminal goes on while the music plays.
    Background(bool),
}

impl Event {
    pub fn note(pitch: u8, length: u8, dotted: bool) -> Self {
        Event::Note {
            pitch: Some(pitch),
            length,
            dotted,
        }
    }

    pub fn rest(length: u8, dotted: bool) -> Self {
        Event::Note { pitch: None, length, dotted }
    }

    /// How long the event lasts, in whole notes.
    pub fn whole_notes(&self) -> f32 {
        match self {
            Event::Note { length, dotted, .. } => (if *dotted { 1.5 } else { 1.0 }) / f32::from(*length),
            _ => 0.0,
        }
    }
}

/// `O4 C`-style name of a pitch.
pub fn note_name(pitch: u8) -> String {
    format!("O{} {}", pitch / 12, SCALE[usize::from(pitch % 12)])
}

pub fn is_black_key(pitch: u8) -> bool {
    SCALE[usize::from(pitch % 12)].len() > 1
}

/// A character terminals reject, which makes them discard the whole music string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// Byte position in the text.
    pub position: usize,
    pub character: char,
}

/// A music string, read like a terminal does.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parsed {
    pub events: Vec<Event>,
    /// The text of each event, as byte ranges.
    pub spans: Vec<Range<usize>>,
    pub problems: Vec<Problem>,
    /// Where commands start, the places a long string can be split.
    boundaries: Vec<usize>,
}

fn number(s: &[u8], i: &mut usize) -> Option<i32> {
    let start = *i;
    let mut value: i32 = 0;
    while let Some(digit) = s.get(*i).filter(|b| b.is_ascii_digit()) {
        value = value.saturating_mul(10).saturating_add(i32::from(digit - b'0'));
        *i += 1;
    }
    (*i > start).then_some(value)
}

/// Reads `text` like CTerm reads a music string (see `icy_parser_core`'s music parser),
/// keeping where each event came from. A style needs its `M`; a leading style letter only
/// means one right after some introducers, which [`decode`] makes explicit.
pub fn parse(text: &str) -> Parsed {
    let s = text.as_bytes();
    let mut parsed = Parsed::default();
    let (mut octave, mut length) = (i32::from(DEFAULT_OCTAVE), i32::from(DEFAULT_LENGTH));
    let mut i = 0;
    while i < s.len() {
        let start = i;
        let c = s[i].to_ascii_uppercase();
        i += 1;
        if !MUSIC_CHARS.contains(&s[start]) {
            if !FORMATTING.contains(&s[start]) {
                let character = text[start..].chars().next().unwrap_or('?');
                parsed.problems.push(Problem { position: start, character });
                i = start + character.len_utf8();
            }
            continue;
        }
        parsed.boundaries.push(start);
        let event = match c {
            b'M' => match s.get(i).map(u8::to_ascii_uppercase) {
                Some(style @ (b'F' | b'B' | b'N' | b'L' | b'S')) => {
                    i += 1;
                    Some(match style {
                        b'F' => Event::Background(false),
                        b'B' => Event::Background(true),
                        b'N' => Event::Articulation(Articulation::Normal),
                        b'L' => Event::Articulation(Articulation::Legato),
                        _ => Event::Articulation(Articulation::Staccato),
                    })
                }
                _ => None,
            },
            b'T' => Some(Event::Tempo(number(s, &mut i).unwrap_or(0).clamp(32, 255) as u8)),
            b'O' => {
                octave = number(s, &mut i).unwrap_or(0).min(6);
                None
            }
            b'L' => {
                length = number(s, &mut i).unwrap_or(0).clamp(1, 64);
                None
            }
            b'<' => {
                octave = (octave - 1).max(0);
                None
            }
            b'>' => {
                octave = (octave + 1).min(6);
                None
            }
            b'A'..=b'G' | b'P' | b'N' => {
                let mut note = match c {
                    b'P' => None,
                    b'N' => Some(number(s, &mut i).unwrap_or(0)).filter(|n| *n <= 71),
                    _ => Some(octave * 12 + LETTERS.iter().position(|&x| x == c).unwrap_or(0) as i32),
                };
                let mut note_length = length;
                let mut dotted = false;
                loop {
                    match s.get(i) {
                        Some(b'+' | b'#') => note = note.map(|n| n + 1),
                        Some(b'-') => note = note.map(|n| n - 1),
                        Some(b'.') => dotted = true,
                        Some(d) if d.is_ascii_digit() => {
                            note_length = number(s, &mut i).unwrap_or(0);
                            continue;
                        }
                        _ => break,
                    }
                    i += 1;
                }
                Some(Event::Note {
                    pitch: note.filter(|n| (0..=i32::from(HIGHEST_PITCH)).contains(n)).map(|n| n as u8),
                    length: note_length.clamp(1, 64) as u8,
                    dotted,
                })
            }
            _ => None,
        };
        if let Some(event) = event {
            parsed.events.push(event);
            parsed.spans.push(start..i);
        }
    }
    parsed
}

/// Writes `events` as a music string, choosing the octave and length commands that keep it short.
pub fn generate(events: &[Event]) -> String {
    let mut counts = [0usize; 65];
    for event in events {
        if let Event::Note { length, .. } = event {
            counts[usize::from(*length)] += 1;
        }
    }
    let default_length = (1..=64u8)
        .max_by_key(|&length| (counts[usize::from(length)], length == DEFAULT_LENGTH))
        .unwrap_or(DEFAULT_LENGTH);
    let mut text = String::new();
    if default_length != DEFAULT_LENGTH {
        text.push_str(&format!("L{default_length}"));
    }
    let mut octave = DEFAULT_OCTAVE;
    for event in events {
        match *event {
            Event::Tempo(tempo) => text.push_str(&format!("T{tempo}")),
            Event::Articulation(articulation) => text.push_str(articulation.code()),
            Event::Background(background) => text.push_str(if background { "MB" } else { "MF" }),
            Event::Note { pitch, length, dotted } => {
                match pitch {
                    Some(pitch) => {
                        let target = pitch / 12;
                        if target == octave + 1 {
                            text.push('>');
                        } else if target + 1 == octave {
                            text.push('<');
                        } else if target != octave {
                            text.push_str(&format!("O{target}"));
                        }
                        octave = target;
                        text.push_str(SCALE[usize::from(pitch % 12)]);
                    }
                    None => text.push('P'),
                }
                if length != default_length {
                    text.push_str(&length.to_string());
                }
                if dotted {
                    text.push('.');
                }
            }
        }
    }
    text
}

/// The settings in effect before event `index`: tempo, articulation and background play.
fn settings_before(events: &[Event], index: usize) -> (u8, Articulation, bool) {
    let mut settings = (DEFAULT_TEMPO, Articulation::Normal, false);
    for event in &events[..index.min(events.len())] {
        match *event {
            Event::Tempo(tempo) => settings.0 = tempo,
            Event::Articulation(articulation) => settings.1 = articulation,
            Event::Background(background) => settings.2 = background,
            Event::Note { .. } => {}
        }
    }
    settings
}

fn style(articulation: Articulation) -> MusicStyle {
    match articulation {
        Articulation::Normal => MusicStyle::Normal,
        Articulation::Legato => MusicStyle::Legato,
        Articulation::Staccato => MusicStyle::Staccato,
    }
}

/// The music to play from event `from` on, with the settings of the events before it.
pub fn to_ansi_music(events: &[Event], from: usize) -> AnsiMusic {
    let (mut tempo, articulation, _) = settings_before(events, from);
    let mut music = AnsiMusic::default();
    if articulation != Articulation::Normal {
        music.music_actions.push(MusicAction::SetStyle(style(articulation)));
    }
    for event in events.iter().skip(from) {
        match *event {
            Event::Tempo(value) => tempo = value,
            Event::Articulation(articulation) => music.music_actions.push(MusicAction::SetStyle(style(articulation))),
            // The editor plays the music by itself, so it does not matter whether a terminal would wait.
            Event::Background(_) => {}
            Event::Note { pitch, length, dotted } => {
                let tempo_length = i32::from(tempo) * i32::from(length);
                music.music_actions.push(match pitch {
                    Some(pitch) => MusicAction::PlayNote(FREQ[usize::from(pitch)], tempo_length, dotted),
                    None => MusicAction::Pause(tempo_length, dotted),
                });
            }
        }
    }
    music
}

/// When each event starts and how long it lasts, in milliseconds.
pub fn timing(events: &[Event]) -> Vec<(f64, f64)> {
    let mut tempo = DEFAULT_TEMPO;
    let mut time = 0.0;
    events
        .iter()
        .map(|event| {
            let duration = match *event {
                Event::Tempo(value) => {
                    tempo = value;
                    0.0
                }
                Event::Note { length, dotted, .. } => f64::from(MusicAction::Pause(i32::from(tempo) * i32::from(length), dotted).get_duration()),
                _ => 0.0,
            };
            let start = time;
            time += duration;
            (start, duration)
        })
        .collect()
}

/// The music strings of a file and how they start.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MusicFile {
    /// The strings, each on a line of its own.
    pub text: String,
    pub introducer: Introducer,
}

/// Whether `string` starts with a style letter a style-prefix introducer reads as a style:
/// F, B, L, S, or N without a note number after it.
fn leading_style(string: &str) -> bool {
    let bytes = string.as_bytes();
    match bytes.first().map(u8::to_ascii_uppercase) {
        Some(b'F' | b'B' | b'L' | b'S') => true,
        Some(b'N') => !bytes.get(1).is_some_and(u8::is_ascii_digit),
        _ => false,
    }
}

/// Reads the music strings of an `.ams` or `.mus` file. Text between them is dropped; a file
/// without escape sequences may be a bare music string.
pub fn decode(bytes: &[u8]) -> Result<MusicFile, String> {
    let bytes = bytes.iter().position(|&b| b == 0x1A).map_or(bytes, |eof| &bytes[..eof]);
    let mut strings: Vec<String> = Vec::new();
    let mut introducer = None;
    let mut i = 0;
    while i + 2 < bytes.len() {
        let Some(found) = (bytes[i] == 0x1B && bytes[i + 1] == b'[')
            .then(|| Introducer::from_final(bytes[i + 2]))
            .flatten()
        else {
            i += 1;
            continue;
        };
        let start = i + 3;
        let end = bytes[start..]
            .iter()
            .position(|b| !MUSIC_CHARS.contains(b))
            .map_or(bytes.len(), |end| start + end);
        if bytes.get(end) != Some(&0x0E) {
            // Terminals discard a string that something other than SO ends.
            i = end.max(i + 1);
            continue;
        }
        let mut string = String::from_utf8_lossy(&bytes[start..end]).into_owned();
        if found.style_prefix() && leading_style(&string) {
            string.insert(0, 'M');
        }
        introducer.get_or_insert(found);
        strings.push(string);
        i = end + 1;
    }
    if let Some(introducer) = introducer {
        return Ok(MusicFile {
            text: strings.join("\n"),
            introducer,
        });
    }
    if bytes.iter().all(|b| MUSIC_CHARS.contains(b) || FORMATTING.contains(b)) {
        return Ok(MusicFile {
            text: String::from_utf8_lossy(bytes).trim().to_owned(),
            introducer: Introducer::default(),
        });
    }
    Err("The file contains no ANSI music.".to_owned())
}

/// Writes `text` as music strings that start with `introducer`: one for each line, and lines
/// longer than CTerm keeps split at command boundaries.
pub fn encode(text: &str, introducer: Introducer) -> Result<Vec<u8>, Problem> {
    if let Some(problem) = parse(text).problems.first() {
        return Err(problem.clone());
    }
    let mut bytes = Vec::new();
    for line in text.split('\n') {
        let line: String = line.chars().filter(|c| !FORMATTING.contains(&(*c as u8))).collect();
        let boundaries = parse(&line).boundaries;
        let mut start = 0;
        while start < line.len() {
            // One character is kept free for the space that keeps a leading letter from being a style.
            let limit = start + MAX_STRING_LEN - 1;
            let end = if line.len() <= limit {
                line.len()
            } else {
                boundaries
                    .iter()
                    .copied()
                    .filter(|&boundary| boundary > start && boundary <= limit)
                    .max()
                    .unwrap_or(limit)
            };
            let mut chunk = &line[start..end];
            bytes.extend_from_slice(introducer.bytes());
            if introducer.style_prefix() {
                // These introducers take a leading style letter as a style: `M` can go, as the
                // file had it, but another letter must not be read as one.
                if let Some(implied) = chunk.strip_prefix(['M', 'm']).filter(|rest| leading_style(rest)) {
                    chunk = implied;
                } else if leading_style(chunk) {
                    bytes.push(b' ');
                }
            }
            bytes.extend_from_slice(chunk.as_bytes());
            bytes.push(0x0E);
            start = end;
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_parser_core::{AnsiParser, CommandParser, CommandSink, MusicOption, TerminalCommand};

    /// What a terminal plays for `bytes`.
    fn terminal(bytes: &[u8]) -> Vec<MusicAction> {
        struct Sink(Vec<MusicAction>);
        impl CommandSink for Sink {
            fn print(&mut self, _text: &[u8]) {}
            fn emit(&mut self, _cmd: TerminalCommand) {}
            fn play_music(&mut self, music: AnsiMusic) {
                self.0.extend(music.music_actions);
            }
        }
        let mut parser = AnsiParser::new();
        parser.set_music_option(MusicOption::Both);
        let mut sink = Sink(Vec::new());
        parser.parse(bytes, &mut sink);
        sink.0
    }

    fn sounds(actions: Vec<MusicAction>) -> Vec<MusicAction> {
        actions.into_iter().filter(|action| !matches!(action, MusicAction::SetStyle(_))).collect()
    }

    #[test]
    fn parses_like_a_terminal() {
        for text in [
            "T150 O3 L8 CDEFGAB>C",
            "c+4. d-16 e#2 p8 n37 n80 o6 b+ <<a",
            "MLL16CDE MS P4. ms T255 G L64 T10 A",
            "O0 C- O6 B B+ N71 N72 L0 C L99 D",
        ] {
            let parsed = parse(text);
            assert!(parsed.problems.is_empty(), "{text}");
            let bytes = [b"\x1b[N".as_slice(), text.as_bytes(), b"\x0e"].concat();
            assert_eq!(sounds(to_ansi_music(&parsed.events, 0).music_actions), sounds(terminal(&bytes)), "{text}");
        }
    }

    #[test]
    fn spans_point_at_the_event_text() {
        let text = "T90 O3 C#8. MS P";
        let parsed = parse(text);
        let spans: Vec<&str> = parsed.spans.iter().map(|span| &text[span.clone()]).collect();
        assert_eq!(spans, ["T90", "C#8.", "MS", "P"]);
        assert_eq!(
            parsed.events,
            [
                Event::Tempo(90),
                Event::note(37, 8, true),
                Event::Articulation(Articulation::Staccato),
                Event::rest(4, false)
            ]
        );
    }

    #[test]
    fn reports_characters_terminals_reject() {
        let parsed = parse("C D\nE X ö");
        assert_eq!(
            parsed.problems,
            [Problem { position: 6, character: 'X' }, Problem { position: 8, character: 'ö' }]
        );
        assert_eq!(parsed.events.len(), 3);
    }

    #[test]
    fn generated_text_reads_back_as_the_same_events() {
        let events = vec![
            Event::Tempo(140),
            Event::note(48, 8, false),
            Event::note(49, 8, true),
            Event::note(61, 8, false),
            Event::note(23, 4, false),
            Event::note(83, 64, false),
            Event::rest(2, true),
            Event::Articulation(Articulation::Legato),
            Event::Background(true),
            Event::note(0, 1, false),
            Event::Background(false),
        ];
        let text = generate(&events);
        assert_eq!(text, "L8T140CC#.>C#O1B4O6B64P2.MLMBO0C1MF");
        assert_eq!(parse(&text).events, events);
        assert_eq!(generate(&[]), "");
    }

    #[test]
    fn timing_follows_tempo_changes() {
        let events = [Event::note(48, 4, false), Event::Tempo(60), Event::note(48, 4, true), Event::rest(8, false)];
        assert_eq!(timing(&events), [(0.0, 500.0), (500.0, 0.0), (500.0, 1500.0), (2000.0, 500.0)]);
    }

    #[test]
    fn playing_from_an_event_keeps_earlier_settings() {
        let events = [
            Event::Tempo(60),
            Event::Articulation(Articulation::Staccato),
            Event::note(48, 4, false),
            Event::note(50, 2, false),
        ];
        assert_eq!(
            to_ansi_music(&events, 3).music_actions,
            [MusicAction::SetStyle(MusicStyle::Staccato), MusicAction::PlayNote(FREQ[50], 120, false)]
        );
    }

    #[test]
    fn decodes_every_introducer_and_explicit_leading_styles() {
        let file = decode(b"Title\r\n\x1b[|MFT120CDE\x0e\x1b[MLCD\x0e\x1b[NLE\x0e\x1a\x1b[|SAUCE\x0e").unwrap();
        assert_eq!(file.introducer, Introducer::SyncTerm);
        // After ESC [ M a leading L is legato; after ESC [ N it is a length.
        assert_eq!(file.text, "MFT120CDE\nMLCD\nLE");
        let file = decode(b"\x1b[MN12C\x0e").unwrap();
        assert_eq!((file.text.as_str(), file.introducer), ("N12C", Introducer::Conflicting));
        assert_eq!(decode(b"T120 L8 CDE\r\n").unwrap().text, "T120 L8 CDE");
        assert!(decode(b"\x1b[31mHello").is_err());
        // A string with a character terminals reject is dropped, as they do.
        assert_eq!(decode(b"\x1b[|C\x07D\x0e\x1b[|E\x0e").unwrap().text, "E");
    }

    #[test]
    fn encoded_files_play_what_the_text_says() {
        let text = "LSC\nT90 MS CDE";
        for introducer in Introducer::ALL {
            let bytes = encode(text, introducer).unwrap();
            assert!(bytes.starts_with(introducer.bytes()));
            assert_eq!(
                sounds(terminal(&bytes)),
                sounds(to_ansi_music(&parse(text).events, 0).music_actions),
                "{introducer:?}"
            );
            assert_eq!(parse(&decode(&bytes).unwrap().text).events, parse(text).events);
        }
        assert_eq!(encode("C X", Introducer::SyncTerm), Err(Problem { position: 2, character: 'X' }));
        // Files read and written unchanged stay byte for byte the same.
        for bytes in [
            b"\x1b[MFT150L8CDEFG\x0e".as_slice(),
            b"\x1b[|NC\x0e\x1b[|N12C\x0e",
            b"\x1b[NLE\x0e",
            b"\x1b[M MN\x0e",
        ] {
            let file = decode(bytes).unwrap();
            assert_eq!(encode(&file.text, file.introducer).unwrap(), bytes, "{file:?}");
        }
        assert_eq!(encode("", Introducer::SyncTerm), Ok(Vec::new()));
    }

    #[test]
    fn long_music_is_split_at_command_boundaries() {
        let text = "L16".to_owned() + &"C#D#ESF".repeat(400);
        let bytes = encode(&text, Introducer::Conflicting).unwrap();
        let strings: Vec<&[u8]> = bytes.split(|&b| b == 0x0E).filter(|s| !s.is_empty()).collect();
        assert!(strings.len() > 2);
        for string in &strings {
            assert!(string.len() - 3 <= MAX_STRING_LEN);
        }
        assert_eq!(sounds(terminal(&bytes)), sounds(to_ansi_music(&parse(&text).events, 0).music_actions));
    }
}
