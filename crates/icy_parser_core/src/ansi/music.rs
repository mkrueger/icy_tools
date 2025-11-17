use serde::{Deserialize, Serialize};

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
