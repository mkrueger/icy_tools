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
