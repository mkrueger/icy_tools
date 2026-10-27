use serde::{Deserialize, Serialize};

use crate::{AnsiParser, CommandSink, ParseError, ansi::ParserState};

/// Characters allowed in a music string (CTerm manual, "ANSI" Music). Any
/// other byte ends the string, and the string is discarded.
const MUSIC_CHARS: &[u8] = b"aAbBcCdDeEfFgGlLmMnNoOpPsStT0123456789.-+#<> ";

/// CTerm keeps at most this many characters of a music string.
const MAX_MUSIC_STRING_LEN: usize = 1022;

/// Semitones within an octave, as letter notes and sharps.
const SCALE: &[u8; 12] = b"C#D#EF#G#A#B";

/// Letter notes play this many semitones above their note number. The CTerm
/// manual numbers notes from C in octave 0, so `O4 C` is note 48 (1046.5 Hz).
/// SyncTERM indexes its frequency table with `octave * 12 + 1`, one semitone
/// sharp; that would be 1.
const LETTER_NOTE_OFFSET: i32 = 0;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MusicOption {
    #[default]
    Off,
    /// `CSI |` and `CSI M` start music.
    Conflicting,
    /// `CSI |` and `CSI N` start music (CTerm's `CSI = 1 M`).
    Banana,
    /// `CSI |`, `CSI N` and `CSI M` start music (CTerm's `CSI = 2 M`).
    Both,
    /// Only `CSI |` starts music (CTerm's `CSI = 0 M`).
    SyncTerm,
}

impl std::fmt::Display for MusicOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MusicOption::Off => write!(f, "Off"),
            MusicOption::Conflicting => write!(f, "Conflicting"),
            MusicOption::Banana => write!(f, "Banana"),
            MusicOption::Both => write!(f, "Both"),
            MusicOption::SyncTerm => write!(f, "SyncTERM"),
        }
    }
}

/// Music style for ANSI music playback
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicStyle {
    /// Play music in foreground (blocks)
    Foreground,
    /// Play music in background (non-blocking)
    Background,
    /// Normal note articulation (7/8 of note duration)
    Normal,
    /// Legato articulation (full note duration, no pause between notes)
    Legato,
    /// Staccato articulation (3/4 of note duration, 1/4 pause)
    Staccato,
}

impl MusicStyle {
    /// Calculate the pause length after a note based on the music style
    pub fn get_pause_length(&self, duration: i32) -> i32 {
        match self {
            MusicStyle::Legato => 0,
            MusicStyle::Staccato => duration / 4,
            _ => duration / 8,
        }
    }
}

/// ANSI music action - a single music command
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MusicAction {
    /// Play a note: frequency (Hz), tempo * length, is_dotted
    PlayNote(f32, i32, bool),
    /// Pause for given tempo * length
    Pause(i32),
    /// Change music style
    SetStyle(MusicStyle),
}

impl MusicAction {
    /// Get the duration of this music action in milliseconds
    pub fn get_duration(&self) -> i32 {
        match self {
            MusicAction::PlayNote(_, len, dotted) => {
                if *dotted {
                    360000 / *len
                } else {
                    240000 / *len
                }
            }
            MusicAction::Pause(len) => 240000 / *len,
            _ => 0,
        }
    }
}

/// ANSI music sequence - a collection of music actions
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnsiMusic {
    /// The music actions to perform
    pub music_actions: Vec<MusicAction>,
}

/// Note frequencies for ANSI music (C1-B7)
/// Generated with: 440.0 * pow(2.0, (n - 49.0) / 12.0)
pub const FREQ: [f32; 12 * 7] = [
    //  C      C#       D        D#       E        F        F#       G         G#        A         A#        B
    65.4064, 69.2957, 73.4162, 77.7817, 82.4069, 87.3071, 92.4986, 97.9989, 103.8262, 110.0000, 116.5409, 123.4708, 130.8128, 138.5913, 146.8324, 155.5635,
    164.8138, 174.6141, 184.9972, 195.9977, 207.6523, 220.0000, 233.0819, 246.9417, 261.6256, 277.1826, 293.6648, 311.127, 329.6276, 349.2282, 369.9944,
    391.9954, 415.3047, 440.0000, 466.1638, 493.8833, 523.2511, 554.3653, 587.3295, 622.254, 659.2551, 698.4565, 739.9888, 783.9909, 830.6094, 880.0000,
    932.3275, 987.7666, 1046.5023, 1108.7305, 1_174.659, 1244.5079, 1318.5102, 1396.9129, 1479.9777, 1567.9817, 1661.2188, 1760.0000, 1_864.655, 1975.5332,
    2093.0045, 2217.461, 2_349.318, 2489.0159, 2637.0205, 2_793.826, 2959.9554, 3135.9635, 3322.4376, 3520.0000, 3_729.31, 3951.0664, 4_186.009, 4_434.922,
    4_698.636, 4978.0317, 5_274.041, 5_587.652, 5919.9108, 6_271.927, 6_644.875, 7040.0000, 7_458.62, 7_902.132,
];

/// Music state that carries over from one music string to the next, as in
/// CTerm; it resets with the terminal (RIS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MusicSettings {
    tempo: i32,
    octave: i32,
    length: i32,
    articulation: MusicStyle,
    foreground: bool,
}

impl Default for MusicSettings {
    fn default() -> Self {
        Self {
            tempo: 120,
            octave: 4,
            length: 4,
            articulation: MusicStyle::Normal,
            foreground: true,
        }
    }
}

fn parse_number(s: &[u8], i: &mut usize) -> Option<i32> {
    let start = *i;
    let mut value: i32 = 0;
    while let Some(digit) = s.get(*i).filter(|b| b.is_ascii_digit()) {
        value = value.saturating_mul(10).saturating_add(i32::from(digit - b'0'));
        *i += 1;
    }
    (*i > start).then_some(value)
}

impl MusicSettings {
    fn set_style(&mut self, style: MusicStyle, music: &mut AnsiMusic) {
        match style {
            MusicStyle::Foreground => self.foreground = true,
            MusicStyle::Background => self.foreground = false,
            articulation => self.articulation = articulation,
        }
        music.music_actions.push(MusicAction::SetStyle(style));
    }

    /// Parse one music string. `style_prefix`: the introducer was `CSI |` or
    /// `CSI M`, so a leading F, B, L, S, or N not followed by a digit, is a
    /// style, as if `M` came before it.
    pub(crate) fn play(&mut self, string: &[u8], style_prefix: bool) -> AnsiMusic {
        let s: Vec<u8> = string.iter().map(u8::to_ascii_uppercase).collect();
        let mut music = AnsiMusic::default();

        // Articulation and background play carry over from earlier strings.
        if self.articulation != MusicStyle::Normal {
            music.music_actions.push(MusicAction::SetStyle(self.articulation));
        }
        if !self.foreground {
            music.music_actions.push(MusicAction::SetStyle(MusicStyle::Background));
        }

        let style = |c: Option<&u8>| match c {
            Some(b'F') => Some(MusicStyle::Foreground),
            Some(b'B') => Some(MusicStyle::Background),
            Some(b'N') => Some(MusicStyle::Normal),
            Some(b'L') => Some(MusicStyle::Legato),
            Some(b'S') => Some(MusicStyle::Staccato),
            _ => None,
        };

        let mut i = 0;
        let note_number_first = s.first() == Some(&b'N') && s.get(1).is_some_and(u8::is_ascii_digit);
        if let Some(style) = style(s.first()).filter(|_| style_prefix && !note_number_first) {
            self.set_style(style, &mut music);
            i = 1;
        }

        while i < s.len() {
            let c = s[i];
            i += 1;
            match c {
                b'M' => {
                    if let Some(style) = style(s.get(i)) {
                        self.set_style(style, &mut music);
                        i += 1;
                    }
                }
                b'T' => self.tempo = parse_number(&s, &mut i).unwrap_or(0).clamp(32, 255),
                b'O' => self.octave = parse_number(&s, &mut i).unwrap_or(0).min(6),
                b'L' => self.length = parse_number(&s, &mut i).unwrap_or(0).clamp(1, 64),
                b'<' => self.octave = (self.octave - 1).max(0),
                b'>' => self.octave = (self.octave + 1).min(6),
                b'A'..=b'G' | b'P' | b'N' => {
                    // `None` is silence: a pause, or a note number above 71.
                    let mut note = match c {
                        b'P' => None,
                        b'N' => Some(parse_number(&s, &mut i).unwrap_or(0)).filter(|n| *n <= 71),
                        _ => Some(self.octave * 12 + LETTER_NOTE_OFFSET + SCALE.iter().position(|&x| x == c).unwrap_or(0) as i32),
                    };
                    let mut length = self.length;
                    let mut dotted = false;
                    // Modifiers, in any order; a later length overrides an earlier one.
                    loop {
                        match s.get(i) {
                            Some(b'+' | b'#') => note = note.map(|n| n + 1),
                            Some(b'-') => note = note.map(|n| n - 1),
                            Some(b'.') => dotted = true,
                            Some(d) if d.is_ascii_digit() => {
                                length = parse_number(&s, &mut i).unwrap_or(0);
                                continue;
                            }
                            _ => break,
                        }
                        i += 1;
                    }
                    let tempo_length = self.tempo * length.clamp(1, 64);
                    match note.filter(|n| (0..FREQ.len() as i32).contains(n)) {
                        Some(n) => music.music_actions.push(MusicAction::PlayNote(FREQ[n as usize], tempo_length, dotted)),
                        // Pause lasts 240000 / len ms; a dotted one 1.5 times that.
                        None => music
                            .music_actions
                            .push(MusicAction::Pause(if dotted { (tempo_length * 2 / 3).max(1) } else { tempo_length })),
                    }
                }
                // Spaces, and digits or modifiers with nothing to modify.
                _ => {}
            }
        }
        music
    }
}

impl AnsiParser {
    /// Start collecting a music string. `style_prefix`: see [`MusicSettings::play`].
    pub(crate) fn start_ansi_music(&mut self, style_prefix: bool) {
        self.state = ParserState::AnsiMusic;
        self.music_string.clear();
        self.music_style_prefix = style_prefix;
    }

    /// Collect a music string up to SO (0x0E), then play it.
    pub(crate) fn parse_ansi_music(&mut self, byte: u8, sink: &mut dyn CommandSink) {
        if byte == 0x0E {
            self.state = ParserState::Default;
            let music = self.music_settings.play(&self.music_string, self.music_style_prefix);
            self.music_string.clear();
            sink.play_music(music);
            self.blocking_construct = true;
        } else if MUSIC_CHARS.contains(&byte) {
            if self.music_string.len() < MAX_MUSIC_STRING_LEN {
                self.music_string.push(byte);
            }
        } else {
            // CTerm discards the string, and the byte that ended it.
            self.state = ParserState::Default;
            self.music_string.clear();
            sink.report_error(
                ParseError::MalformedSequence {
                    description: "Invalid character in ANSI music",
                    sequence: Some(format!("0x{byte:02X}")),
                    context: Some("Music string discarded".to_string()),
                },
                crate::ErrorLevel::Warning,
            );
        }
    }

    /// Reset the music state that carries over between strings (RIS).
    pub(crate) fn reset_music(&mut self) {
        self.music_settings = MusicSettings::default();
    }
}
