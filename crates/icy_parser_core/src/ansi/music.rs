use serde::{Deserialize, Serialize};

use crate::{AnsiParser, CommandSink, ParseError, ansi::ParserState};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MusicOption {
    #[default]
    Off,
    Conflicting,
    Banana,
    Both,
}

impl std::fmt::Display for MusicOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MusicOption::Off => write!(f, "Off"),
            MusicOption::Conflicting => write!(f, "Conflicting"),
            MusicOption::Banana => write!(f, "Banana"),
            MusicOption::Both => write!(f, "Both"),
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

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MusicState {
    Default,
    ParseMusicStyle,
    SetTempo(u16),
    Pause(i32),
    SetOctave,
    Note(usize, i32),
    PlayNoteByNumber(usize),
    SetLength(i32),
}

impl AnsiParser {
    // ANSI Music parsing methods
    pub(crate) fn parse_ansi_music(&mut self, byte: u8, sink: &mut dyn CommandSink) {
        match self.music_state {
            MusicState::ParseMusicStyle => {
                self.music_state = MusicState::Default;
                match byte {
                    b'F' => self
                        .cur_music
                        .as_mut()
                        .unwrap()
                        .music_actions
                        .push(MusicAction::SetStyle(MusicStyle::Foreground)),
                    b'B' => self
                        .cur_music
                        .as_mut()
                        .unwrap()
                        .music_actions
                        .push(MusicAction::SetStyle(MusicStyle::Background)),
                    b'N' => self.cur_music.as_mut().unwrap().music_actions.push(MusicAction::SetStyle(MusicStyle::Normal)),
                    b'L' => self.cur_music.as_mut().unwrap().music_actions.push(MusicAction::SetStyle(MusicStyle::Legato)),
                    b'S' => self.cur_music.as_mut().unwrap().music_actions.push(MusicAction::SetStyle(MusicStyle::Staccato)),
                    _ => self.parse_default_ansi_music(byte, sink),
                }
            }
            MusicState::SetTempo(x) => {
                if byte.is_ascii_digit() {
                    let x = (x as i32).saturating_mul(10).saturating_add((byte - b'0') as i32) as u16;
                    self.music_state = MusicState::SetTempo(x);
                } else {
                    self.music_state = MusicState::Default;
                    self.cur_tempo = (x.clamp(32, 255)) as i32;
                    self.parse_default_ansi_music(byte, sink);
                }
            }
            MusicState::SetOctave => {
                if (b'0'..=b'6').contains(&byte) {
                    self.cur_octave = (byte - b'0') as usize;
                    self.music_state = MusicState::Default;
                } else {
                    sink.report_error(
                        ParseError::MalformedSequence {
                            description: "Invalid octave in ANSI music",
                            sequence: None,
                        },
                        crate::ErrorLevel::Error,
                    );
                    self.music_state = MusicState::Default;
                }
            }
            MusicState::Note(n, len) => {
                self.music_state = MusicState::Default;
                match byte {
                    b'+' | b'#' => {
                        if n + 1 < FREQ.len() {
                            self.music_state = MusicState::Note(n + 1, len);
                        }
                    }
                    b'-' => {
                        if n > 0 {
                            self.music_state = MusicState::Note(n - 1, len);
                        }
                    }
                    b'0'..=b'9' => {
                        let len = len.saturating_mul(10).saturating_add((byte - b'0') as i32);
                        self.music_state = MusicState::Note(n, len);
                    }
                    b'.' => {
                        let len = len * 3 / 2;
                        self.music_state = MusicState::Note(n, len);
                        self.dotted_note = true;
                    }
                    _ => {
                        self.music_state = MusicState::Default;
                        let len = if len == 0 { self.cur_length } else { len };
                        // Calculate frequency index: ANSI octave offset by 2 (O4 = array octave 2)
                        // O4C = (4-2)*12 + 0 = 24 (C4 = 261.63 Hz)
                        let freq_index = n + ((self.cur_octave.saturating_sub(2)) * 12);
                        let freq_index = freq_index.clamp(0, FREQ.len() - 1);
                        self.cur_music
                            .as_mut()
                            .unwrap()
                            .music_actions
                            .push(MusicAction::PlayNote(FREQ[freq_index], self.cur_tempo * len, self.dotted_note));
                        self.dotted_note = false;
                        self.parse_default_ansi_music(byte, sink);
                    }
                }
            }
            MusicState::SetLength(x) => {
                if byte.is_ascii_digit() {
                    let x = x.saturating_mul(10).saturating_add((byte - b'0') as i32);
                    self.music_state = MusicState::SetLength(x);
                } else if byte == b'.' {
                    let x = x * 3 / 2;
                    self.music_state = MusicState::SetLength(x);
                } else {
                    self.music_state = MusicState::Default;
                    self.cur_length = x.clamp(1, 64);
                    self.parse_default_ansi_music(byte, sink);
                }
            }
            MusicState::PlayNoteByNumber(x) => {
                if byte.is_ascii_digit() {
                    let x = (x as i32).saturating_mul(10).saturating_add((byte - b'0') as i32) as usize;
                    self.music_state = MusicState::PlayNoteByNumber(x);
                } else {
                    self.music_state = MusicState::Default;
                    let len = self.cur_length;
                    // QBASIC N notation: N0 starts at C0, but FREQ table starts at C1 (index 0)
                    // So there's an offset of 16 semitones: index = note_number - 16
                    // N49 = A4 (440Hz) = index 33
                    let note_index = if x >= 16 { x - 16 } else { 0 };
                    let note_index = note_index.clamp(0, FREQ.len() - 1);
                    self.cur_music
                        .as_mut()
                        .unwrap()
                        .music_actions
                        .push(MusicAction::PlayNote(FREQ[note_index], self.cur_tempo * len, false));
                    self.dotted_note = false;
                    self.parse_default_ansi_music(byte, sink);
                }
            }
            MusicState::Pause(x) => {
                if byte.is_ascii_digit() {
                    let x = x.saturating_mul(10).saturating_add((byte - b'0') as i32);
                    self.music_state = MusicState::Pause(x);
                } else if byte == b'.' {
                    let x = x * 3 / 2;
                    self.music_state = MusicState::Pause(x);
                } else {
                    self.music_state = MusicState::Default;
                    let pause = x.clamp(1, 64);
                    self.cur_music.as_mut().unwrap().music_actions.push(MusicAction::Pause(self.cur_tempo * pause));
                    self.parse_default_ansi_music(byte, sink);
                }
            }
            MusicState::Default => {
                self.parse_default_ansi_music(byte, sink);
            }
        }
    }

    fn parse_default_ansi_music(&mut self, byte: u8, sink: &mut dyn CommandSink) {
        match byte {
            0x0E => {
                // End of ANSI music sequence
                self.state = ParserState::Default;
                self.cur_octave = 5;
                if let Some(music) = self.cur_music.take() {
                    sink.play_music(music);
                }
            }
            b'T' => self.music_state = MusicState::SetTempo(0),
            b'L' => self.music_state = MusicState::SetLength(0),
            b'O' => self.music_state = MusicState::SetOctave,
            b'C' => self.music_state = MusicState::Note(0, 0),
            b'D' => self.music_state = MusicState::Note(2, 0),
            b'E' => self.music_state = MusicState::Note(4, 0),
            b'F' => self.music_state = MusicState::Note(5, 0),
            b'G' => self.music_state = MusicState::Note(7, 0),
            b'A' => self.music_state = MusicState::Note(9, 0),
            b'B' => self.music_state = MusicState::Note(11, 0),
            b'M' => self.music_state = MusicState::ParseMusicStyle,
            b'N' => self.music_state = MusicState::PlayNoteByNumber(0),
            b'<' => {
                if self.cur_octave > 0 {
                    self.cur_octave -= 1;
                }
            }
            b'>' => {
                if self.cur_octave < 6 {
                    self.cur_octave += 1;
                }
            }
            b'P' => {
                self.music_state = MusicState::Pause(0);
            }
            _ => {
                // Unknown music command - reset state and return to ground
                self.music_state = MusicState::Default;
                self.state = ParserState::Default;
            }
        }
    }
}
