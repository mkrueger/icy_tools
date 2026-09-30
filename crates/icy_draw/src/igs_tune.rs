//! Chip tunes of IGS drawings: notes on the three voices of the sound chip, read from and
//! written as the `n` chip music commands IG's "Tap A Tune" records.
//!
//! An `n` command plays a note (unless its pitch is 0), waits its timing in 1/200 s and then
//! applies its stop type. Notes that start together are written with a timing of 0, and the
//! wait until the next change goes on the last command, the way IG plays them one after the
//! other without flow control.

use icy_parser_core::{IgsCommand, SoundEffect, StopType};

/// Ticks per second: `n` timings are in 1/200 s.
pub const TICKS_PER_SECOND: u32 = 200;
/// The longest wait one `n` command holds.
const MAX_TIMING: u32 = 9999;
pub const VOICES: u8 = 3;
/// The pitches the chip plays as notes, as MIDI note numbers (60 = middle C).
pub const LOWEST_PITCH: u8 = 24;
pub const HIGHEST_PITCH: u8 = 108;

/// How a note ends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NoteEnd {
    /// The voice fades out with the release of its sound (stop type 1).
    #[default]
    Release,
    /// The voice stops at once (stop type 2).
    Cut,
    /// The note keeps sounding until the next note on its voice (stop type 0).
    Hold,
}

impl NoteEnd {
    fn stop(self) -> StopType {
        match self {
            NoteEnd::Release => StopType::SndOff,
            NoteEnd::Cut => StopType::StopSnd,
            NoteEnd::Hold => StopType::NoEffect,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TuneNote {
    /// When the note starts, in ticks from the start of the tune.
    pub start: u32,
    /// How long it sounds before it ends, in ticks, at least 1.
    pub length: u32,
    pub voice: u8,
    /// The MIDI note number.
    pub pitch: u8,
    /// The sound the note is played with, as IG's `n` uses its sound effects as instruments.
    pub effect: SoundEffect,
    pub volume: u8,
    pub end: NoteEnd,
}

impl TuneNote {
    pub fn end_tick(&self) -> u32 {
        self.start + self.length.max(1)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tune {
    /// The notes, ordered by start, then voice.
    pub notes: Vec<TuneNote>,
}

impl Tune {
    /// When the last note ends.
    pub fn duration(&self) -> u32 {
        self.notes.iter().map(TuneNote::end_tick).max().unwrap_or(0)
    }

    pub fn sort(&mut self) {
        self.notes.sort_by_key(|note| (note.start, note.voice, note.pitch));
    }

    /// Keeps one note per voice at a time, as the chip plays them: a note ends where the next
    /// note on its voice starts, and of notes starting together on a voice the one later in
    /// the list stays, so an edited note moved to the end replaces what it covers.
    pub fn fit_voices(&mut self) {
        let mut order: Vec<usize> = (0..self.notes.len()).collect();
        order.sort_by_key(|&index| (self.notes[index].voice, self.notes[index].start, index));
        let mut keep = vec![true; self.notes.len()];
        for pair in order.windows(2) {
            let (earlier, later) = (pair[0], pair[1]);
            let (first, second) = (self.notes[earlier], self.notes[later]);
            if first.voice != second.voice || first.end_tick() <= second.start {
                continue;
            }
            if first.start == second.start {
                keep[earlier] = false;
            } else {
                self.notes[earlier].length = second.start - first.start;
            }
        }
        let mut index = 0;
        self.notes.retain(|_| {
            index += 1;
            keep[index - 1]
        });
        self.sort();
    }

    /// Plays the tune at `percent` of its speed: starts and lengths are stretched or squeezed,
    /// each note keeping at least one tick.
    pub fn scale_time(&mut self, percent: u32) {
        let percent = percent.max(1);
        let scale = |ticks: u32| (u64::from(ticks) * 100 / u64::from(percent)) as u32;
        for note in &mut self.notes {
            let end = scale(note.end_tick());
            note.start = scale(note.start);
            note.length = end.saturating_sub(note.start).max(1);
        }
        self.fit_voices();
    }

    /// Reads the tune the `n` commands play, following IG: a note on a busy voice replaces
    /// the note there, and stop types end notes after the command's wait. Other commands are
    /// ignored.
    pub fn from_commands<'a>(commands: impl IntoIterator<Item = &'a IgsCommand>) -> Self {
        let mut tune = Tune::default();
        let mut sounding: [Option<TuneNote>; VOICES as usize] = [None; VOICES as usize];
        let mut time = 0;
        let finish = |tune: &mut Tune, sounding: &mut Option<TuneNote>, at: u32, end: NoteEnd| {
            if let Some(mut note) = sounding.take() {
                note.length = at.saturating_sub(note.start).max(1);
                note.end = end;
                tune.notes.push(note);
            }
        };
        for command in commands {
            let IgsCommand::ChipMusic {
                sound_effect,
                voice,
                volume,
                pitch,
                timing,
                stop_type,
            } = command
            else {
                continue;
            };
            let voice = usize::from((*voice).min(VOICES - 1));
            if *pitch > 0 {
                finish(&mut tune, &mut sounding[voice], time, NoteEnd::Hold);
                sounding[voice] = Some(TuneNote {
                    start: time,
                    length: 1,
                    voice: voice as u8,
                    pitch: *pitch,
                    effect: *sound_effect,
                    volume: (*volume).min(15),
                    end: NoteEnd::Hold,
                });
            }
            time += (*timing).clamp(0, MAX_TIMING as i32) as u32;
            let (voices, end) = match stop_type {
                StopType::NoEffect => continue,
                StopType::SndOff => (voice..=voice, NoteEnd::Release),
                StopType::StopSnd => (voice..=voice, NoteEnd::Cut),
                StopType::SndOffAll => (0..=usize::from(VOICES - 1), NoteEnd::Release),
                StopType::StopSndAll => (0..=usize::from(VOICES - 1), NoteEnd::Cut),
            };
            for index in voices {
                finish(&mut tune, &mut sounding[index], time, end);
            }
        }
        // Notes still sounding at the end hold until the tune is over.
        for slot in &mut sounding {
            finish(&mut tune, slot, time, NoteEnd::Hold);
        }
        tune.sort();
        tune
    }

    /// The `n` commands that play the tune: at every moment something changes, the ending
    /// notes are stopped and the starting ones played, and the wait until the next moment
    /// goes on the last started note, or a command of its own, since commands stop after
    /// their wait. A note that ends before anything else happens gets its wait
    /// and stop type on its own command, as Tap A Tune writes single notes.
    pub fn to_commands(&self) -> Vec<IgsCommand> {
        let mut notes = self.notes.clone();
        notes.sort_by_key(|note| (note.start, note.voice, note.pitch));
        // The tune starts at 0, so a rest before the first note is waited too.
        let mut moments: Vec<u32> = notes.iter().flat_map(|note| [note.start, note.end_tick()]).chain([0]).collect();
        moments.sort_unstable();
        moments.dedup();

        let mut commands = Vec::new();
        // The notes whose stop is already written on their own command.
        let mut stopped = vec![false; notes.len()];
        for (index, &moment) in moments.iter().enumerate() {
            for (note, done) in notes.iter().zip(&mut stopped) {
                if note.end_tick() == moment && note.end != NoteEnd::Hold && !*done {
                    *done = true;
                    commands.push(silent(note.voice, 0, note.end.stop()));
                }
            }
            let mut last_started = None;
            for (position, note) in notes.iter().enumerate().filter(|(_, note)| note.start == moment) {
                commands.push(IgsCommand::ChipMusic {
                    sound_effect: note.effect,
                    voice: note.voice,
                    volume: note.volume.min(15),
                    pitch: note.pitch,
                    timing: 0,
                    stop_type: StopType::NoEffect,
                });
                last_started = Some(position);
            }
            let Some(&next) = moments.get(index + 1) else {
                break;
            };
            let mut wait = next - moment;
            // A command stops after its wait, so only a started note can carry the wait.
            if last_started.is_none() {
                commands.push(silent(0, 0, StopType::NoEffect));
            }
            // The last note started here ends at the next moment: its command waits and stops.
            let folded = last_started.filter(|&position| {
                let note = &notes[position];
                note.end_tick() == next && note.end != NoteEnd::Hold && wait <= MAX_TIMING
            });
            if let Some(IgsCommand::ChipMusic { timing, stop_type, .. }) = commands.last_mut() {
                *timing = wait.min(MAX_TIMING) as i32;
                if let Some(position) = folded {
                    *stop_type = notes[position].end.stop();
                    stopped[position] = true;
                }
            }
            wait -= wait.min(MAX_TIMING);
            while wait > 0 {
                let part = wait.min(MAX_TIMING);
                commands.push(silent(0, part, StopType::NoEffect));
                wait -= part;
            }
        }
        commands
    }
}

/// The name of MIDI note `pitch`, e.g. `C4` for middle C (60) or `F#3`.
pub fn note_name(pitch: u8) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    format!("{}{}", NAMES[usize::from(pitch % 12)], i32::from(pitch / 12) - 1)
}

/// Whether `pitch` is a black key on a piano.
pub fn is_black_key(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

/// An `n` command that plays nothing but waits `timing` and applies `stop` to `voice`.
fn silent(voice: u8, timing: u32, stop: StopType) -> IgsCommand {
    IgsCommand::ChipMusic {
        sound_effect: SoundEffect::default(),
        voice,
        volume: 0,
        pitch: 0,
        timing: timing as i32,
        stop_type: stop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(start: u32, length: u32, voice: u8, pitch: u8, end: NoteEnd) -> TuneNote {
        TuneNote {
            start,
            length,
            voice,
            pitch,
            effect: SoundEffect::Longbell,
            volume: 15,
            end,
        }
    }

    fn n(voice: u8, pitch: u8, timing: i32, stop_type: StopType) -> IgsCommand {
        IgsCommand::ChipMusic {
            sound_effect: if pitch > 0 { SoundEffect::Longbell } else { SoundEffect::default() },
            voice,
            volume: if pitch > 0 { 15 } else { 0 },
            pitch,
            timing,
            stop_type,
        }
    }

    #[test]
    fn a_melody_is_one_command_per_note_like_tap_a_tune() {
        let tune = Tune {
            notes: vec![note(0, 40, 0, 60, NoteEnd::Release), note(40, 20, 0, 62, NoteEnd::Cut)],
        };
        assert_eq!(tune.to_commands(), vec![n(0, 60, 40, StopType::SndOff), n(0, 62, 20, StopType::StopSnd)]);
        assert_eq!(Tune::from_commands(&tune.to_commands()), tune);
    }

    #[test]
    fn rests_chords_and_overlaps_round_trip() {
        let tune = Tune {
            notes: vec![
                note(0, 50, 0, 60, NoteEnd::Release),
                note(0, 50, 1, 64, NoteEnd::Release),
                note(0, 100, 2, 67, NoteEnd::Cut),
                note(80, 30, 0, 72, NoteEnd::Release),
                note(150, 10, 1, 48, NoteEnd::Hold),
            ],
        };
        let commands = tune.to_commands();
        assert!(commands
            .iter()
            .all(|command| matches!(command, IgsCommand::ChipMusic { timing, .. } if (0..=9999).contains(timing))));
        let total: i32 = commands
            .iter()
            .map(|command| match command {
                IgsCommand::ChipMusic { timing, .. } => *timing,
                _ => 0,
            })
            .sum();
        assert_eq!(total as u32, tune.duration(), "the waits add up to the tune's length");
        assert_eq!(Tune::from_commands(&commands), tune);
    }

    #[test]
    fn long_waits_are_split_into_several_commands() {
        let tune = Tune {
            notes: vec![note(0, 10, 0, 60, NoteEnd::Release), note(25_000, 10, 0, 62, NoteEnd::Release)],
        };
        let commands = tune.to_commands();
        assert!(commands.len() > 3);
        assert_eq!(Tune::from_commands(&commands), tune);
    }

    #[test]
    fn random_tunes_round_trip() {
        // A voice plays one note at a time; the editor keeps notes on a voice apart.
        let mut seed = 0x2545_F491_u64;
        let mut next = |range: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % u64::from(range)) as u32
        };
        for _ in 0..500 {
            let mut tune = Tune::default();
            for voice in 0..VOICES {
                let mut time = next(50);
                for _ in 0..next(8) {
                    let end = [NoteEnd::Release, NoteEnd::Cut][next(2) as usize];
                    let longest = if next(10) == 0 { 30_000 } else { 120 };
                    let length = 1 + next(longest);
                    tune.notes.push(note(time, length, voice, 24 + next(85) as u8, end));
                    let rest = next(3);
                    time += length + rest * next(60);
                }
            }
            tune.sort();
            assert_eq!(Tune::from_commands(&tune.to_commands()), tune);
        }
    }

    #[test]
    fn a_voice_plays_one_note_at_a_time() {
        let mut tune = Tune {
            notes: vec![
                note(0, 100, 0, 60, NoteEnd::Release),
                note(40, 30, 0, 62, NoteEnd::Release),
                note(40, 30, 1, 64, NoteEnd::Release),
                // Edited last: replaces the note starting with it on voice 0.
                note(40, 50, 0, 65, NoteEnd::Cut),
            ],
        };
        tune.fit_voices();
        assert_eq!(
            tune.notes,
            vec![
                note(0, 40, 0, 60, NoteEnd::Release),
                note(40, 50, 0, 65, NoteEnd::Cut),
                note(40, 30, 1, 64, NoteEnd::Release)
            ]
        );
        tune.scale_time(200);
        assert_eq!(tune.notes[1], note(20, 25, 0, 65, NoteEnd::Cut), "twice as fast");
        assert_eq!(note_name(60), "C4");
        assert_eq!(note_name(61), "C#4");
        assert_eq!(note_name(24), "C1");
        assert!(is_black_key(61) && !is_black_key(64));
    }

    #[test]
    fn ig_commands_are_read_with_their_stop_types() {
        let commands = [
            n(0, 60, 0, StopType::NoEffect),
            n(1, 64, 30, StopType::NoEffect),
            // A new note on a busy voice replaces the old one.
            n(0, 62, 20, StopType::NoEffect),
            n(0, 0, 10, StopType::SndOffAll),
            IgsCommand::StopAllSound,
        ];
        let tune = Tune::from_commands(&commands);
        assert_eq!(
            tune.notes,
            vec![
                note(0, 30, 0, 60, NoteEnd::Hold),
                note(0, 60, 1, 64, NoteEnd::Release),
                note(30, 30, 0, 62, NoteEnd::Release),
            ]
        );
    }
}
