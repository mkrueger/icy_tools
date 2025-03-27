use crate::{CallbackAction, EngineResult};

use super::{EngineState, Parser, parse_next_number};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicStyle {
    Foreground,
    Background,
    Normal,
    Legato,
    Staccato,
}

impl MusicStyle {
    pub fn get_pause_length(&self, duration: i32) -> i32 {
        match self {
            MusicStyle::Legato => 0,
            MusicStyle::Staccato => duration / 4,
            _ => duration / 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MusicAction {
    PlayNote(f32, i32, bool), // freq / note length / dotted
    Pause(i32),
    SetStyle(MusicStyle),
}

impl MusicAction {
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

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnsiMusic {
    pub music_actions: Vec<MusicAction>,
}

/*
Generated with:
for oct in range(1, 8):
    for i in range(16, 28):
        n = i + (28-16) * (oct - 1)
        freq = 440.0 * pow(2.0, (n - 49.0) / 12.0)
        print("{:.4f}".format(freq), end=", ")
    print()
*/
pub const FREQ: [f32; 12 * 7] = [
    //  C      C#       D        D#       E        F        F#       G         G#        A         A#        B
    65.4064, 69.2957, 73.4162, 77.7817, 82.4069, 87.3071, 92.4986, 97.9989, 103.8262, 110.0000, 116.5409, 123.4708, 130.8128, 138.5913, 146.8324, 155.5635,
    164.8138, 174.6141, 184.9972, 195.9977, 207.6523, 220.0000, 233.0819, 246.9417, 261.6256, 277.1826, 293.6648, 311.127, 329.6276, 349.2282, 369.9944,
    391.9954, 415.3047, 440.0000, 466.1638, 493.8833, 523.2511, 554.3653, 587.3295, 622.254, 659.2551, 698.4565, 739.9888, 783.9909, 830.6094, 880.0000,
    932.3275, 987.7666, 1046.5023, 1108.7305, 1_174.659, 1244.5079, 1318.5102, 1396.9129, 1479.9777, 1567.9817, 1661.2188, 1760.0000, 1_864.655, 1975.5332,
    2093.0045, 2217.461, 2_349.318, 2489.0159, 2637.0205, 2_793.826, 2959.9554, 3135.9635, 3322.4376, 3520.0000, 3_729.31, 3951.0664, 4_186.009, 4_434.922,
    4_698.636, 4978.0317, 5_274.041, 5_587.652, 5919.9108, 6_271.927, 6_644.875, 7040.0000, 7_458.62, 7_902.132,
];

impl Parser {
    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn parse_ansi_music(&mut self, ch: char) -> EngineResult<CallbackAction> {
        if let EngineState::ParseAnsiMusic(state) = self.state {
            match state {
                MusicState::ParseMusicStyle => {
                    self.state = EngineState::ParseAnsiMusic(MusicState::Default);
                    match ch {
                        'F' => self
                            .cur_music
                            .as_mut()
                            .unwrap()
                            .music_actions
                            .push(MusicAction::SetStyle(MusicStyle::Foreground)),
                        'B' => self
                            .cur_music
                            .as_mut()
                            .unwrap()
                            .music_actions
                            .push(MusicAction::SetStyle(MusicStyle::Background)),
                        'N' => self.cur_music.as_mut().unwrap().music_actions.push(MusicAction::SetStyle(MusicStyle::Normal)),
                        'L' => self.cur_music.as_mut().unwrap().music_actions.push(MusicAction::SetStyle(MusicStyle::Legato)),
                        'S' => self.cur_music.as_mut().unwrap().music_actions.push(MusicAction::SetStyle(MusicStyle::Staccato)),
                        _ => return self.parse_ansi_music(ch),
                    }
                }
                MusicState::SetTempo(x) => {
                    let mut x = x;
                    if ch.is_ascii_digit() {
                        x = parse_next_number(x as i32, ch as u8) as u16;
                        self.state = EngineState::ParseAnsiMusic(MusicState::SetTempo(x));
                    } else {
                        self.state = EngineState::ParseAnsiMusic(MusicState::Default);
                        self.cur_tempo = x.clamp(32, 255) as i32;
                        return Ok(self.parse_default_ansi_music(ch));
                    }
                }
                MusicState::SetOctave => {
                    if ('0'..='6').contains(&ch) {
                        self.cur_octave = ((ch as u8) - b'0') as usize;
                        self.state = EngineState::ParseAnsiMusic(MusicState::Default);
                    } else {
                        return self.unsupported_escape_error();
                    }
                }
                MusicState::Note(n, len) => {
                    self.state = EngineState::ParseAnsiMusic(MusicState::Default);
                    match ch {
                        '+' | '#' => {
                            if n + 1 < FREQ.len() {
                                self.state = EngineState::ParseAnsiMusic(MusicState::Note(n + 1, len));
                            }
                        }
                        '-' => {
                            if n > 0 {
                                // B
                                self.state = EngineState::ParseAnsiMusic(MusicState::Note(n - 1, len));
                            }
                        }
                        '0'..='9' => {
                            let len = parse_next_number(len, ch as u8);
                            self.state = EngineState::ParseAnsiMusic(MusicState::Note(n, len));
                        }
                        '.' => {
                            let len = len * 3 / 2;
                            self.state = EngineState::ParseAnsiMusic(MusicState::Note(n, len));
                            self.dotted_note = true;
                        }
                        _ => {
                            self.state = EngineState::ParseAnsiMusic(MusicState::Default);
                            let len = if len == 0 { self.cur_length } else { len };
                            self.cur_music.as_mut().unwrap().music_actions.push(MusicAction::PlayNote(
                                FREQ[n + (self.cur_octave * 12)],
                                self.cur_tempo * len,
                                self.dotted_note,
                            ));
                            self.dotted_note = false;
                            return Ok(self.parse_default_ansi_music(ch));
                        }
                    }
                }
                MusicState::SetLength(x) => {
                    let mut x = x;
                    if ch.is_ascii_digit() {
                        x = parse_next_number(x, ch as u8);
                        self.state = EngineState::ParseAnsiMusic(MusicState::SetLength(x));
                    } else if ch == '.' {
                        x = x * 3 / 2;
                        self.state = EngineState::ParseAnsiMusic(MusicState::SetLength(x));
                    } else {
                        self.state = EngineState::ParseAnsiMusic(MusicState::Default);
                        self.cur_length = x.clamp(1, 64);
                        return Ok(self.parse_default_ansi_music(ch));
                    }
                }

                MusicState::PlayNoteByNumber(x) => {
                    if ch.is_ascii_digit() {
                        let x = parse_next_number(x as i32, ch as u8) as usize;
                        self.state = EngineState::ParseAnsiMusic(MusicState::PlayNoteByNumber(x));
                    } else {
                        self.state = EngineState::ParseAnsiMusic(MusicState::Default);
                        let len = self.cur_length;
                        let x = x.clamp(0, FREQ.len() - 1);
                        self.cur_music
                            .as_mut()
                            .unwrap()
                            .music_actions
                            .push(MusicAction::PlayNote(FREQ[x], self.cur_tempo * len, false));
                        self.dotted_note = false;
                        return Ok(self.parse_default_ansi_music(ch));
                    }
                }

                MusicState::Pause(x) => {
                    let mut x = x;
                    if ch.is_ascii_digit() {
                        x = parse_next_number(x, ch as u8);
                        self.state = EngineState::ParseAnsiMusic(MusicState::Pause(x));
                    } else if ch == '.' {
                        x = x * 3 / 2;
                        self.state = EngineState::ParseAnsiMusic(MusicState::Pause(x));
                    } else {
                        self.state = EngineState::ParseAnsiMusic(MusicState::Default);
                        let pause = x.clamp(1, 64);
                        self.cur_music.as_mut().unwrap().music_actions.push(MusicAction::Pause(self.cur_tempo * pause));
                        return Ok(self.parse_default_ansi_music(ch));
                    }
                }
                MusicState::Default => {
                    return Ok(self.parse_default_ansi_music(ch));
                }
            }
        }
        Ok(CallbackAction::NoUpdate)
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn parse_default_ansi_music(&mut self, ch: char) -> CallbackAction {
        match ch {
            '\x0E' => {
                self.state = EngineState::Default;
                self.cur_octave = 3;
                return CallbackAction::PlayMusic(self.cur_music.replace(AnsiMusic::default()).unwrap());
            }
            'T' => self.state = EngineState::ParseAnsiMusic(MusicState::SetTempo(0)),
            'L' => self.state = EngineState::ParseAnsiMusic(MusicState::SetLength(0)),
            'O' => self.state = EngineState::ParseAnsiMusic(MusicState::SetOctave),
            'C' => self.state = EngineState::ParseAnsiMusic(MusicState::Note(0, 0)),
            'D' => self.state = EngineState::ParseAnsiMusic(MusicState::Note(2, 0)),
            'E' => self.state = EngineState::ParseAnsiMusic(MusicState::Note(4, 0)),
            'F' => self.state = EngineState::ParseAnsiMusic(MusicState::Note(5, 0)),
            'G' => self.state = EngineState::ParseAnsiMusic(MusicState::Note(7, 0)),
            'A' => self.state = EngineState::ParseAnsiMusic(MusicState::Note(9, 0)),
            'B' => self.state = EngineState::ParseAnsiMusic(MusicState::Note(11, 0)),
            'M' => self.state = EngineState::ParseAnsiMusic(MusicState::ParseMusicStyle),
            'N' => self.state = EngineState::ParseAnsiMusic(MusicState::PlayNoteByNumber(0)),
            '<' => {
                if self.cur_octave > 0 {
                    self.cur_octave -= 1;
                }
            }
            '>' => {
                if self.cur_octave < 6 {
                    self.cur_octave += 1;
                }
            }
            'P' => {
                self.state = EngineState::ParseAnsiMusic(MusicState::Pause(0));
            }
            _ => {}
        }
        CallbackAction::NoUpdate
    }
}
