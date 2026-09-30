//! The chip tune editor: IG's "Tap A Tune" as a piano roll. Notes are bars over time on the
//! three voices of the sound chip, played and recorded from the computer keyboard, and
//! written as `n` chip music commands.

use std::collections::HashMap;

use eframe::egui::{self, Color32, Key, Stroke};
use icy_draw::fl;
use icy_draw::igs_tune::{is_black_key, note_name, NoteEnd, Tune, TuneNote, HIGHEST_PITCH, LOWEST_PITCH, TICKS_PER_SECOND, VOICES};
use icy_engine_gui::egui::appearance::{self, labels, DialogButton, DialogSize};
use icy_parser_core::SoundEffect;

use super::{Sound, SoundPlayer, SoundTable};

const KEYBOARD_WIDTH: f32 = 46.0;
const ROW_HEIGHT: f32 = 11.0;
const RULER_HEIGHT: f32 = 18.0;
/// What the dialog around the roll takes of the window's height: title, settings, the
/// selected note, the key hints and the buttons.
const AROUND_ROLL: f32 = 510.0;
/// Pixels from a note's right end that resize it rather than move it.
const RESIZE_EDGE: f32 = 5.0;
pub const VOICE_COLORS: [Color32; VOICES as usize] = [
    Color32::from_rgb(0x4C, 0x9A, 0xFF),
    Color32::from_rgb(0xF5, 0xA6, 0x23),
    Color32::from_rgb(0x5C, 0xC8, 0x6E),
];
/// The grids notes snap to, in ticks (1/200 s); 0 places them freely.
const SNAPS: [u32; 7] = [0, 5, 10, 20, 25, 50, 100];

/// The keys that play notes, like IG's Tap A Tune and trackers: the bottom row from C of the
/// keyboard octave, the top row an octave higher. Given as semitones above that C.
const NOTE_KEYS: [(Key, u8); 37] = [
    (Key::Z, 0),
    (Key::S, 1),
    (Key::X, 2),
    (Key::D, 3),
    (Key::C, 4),
    (Key::V, 5),
    (Key::G, 6),
    (Key::B, 7),
    (Key::H, 8),
    (Key::N, 9),
    (Key::J, 10),
    (Key::M, 11),
    (Key::Comma, 12),
    (Key::L, 13),
    (Key::Period, 14),
    (Key::Semicolon, 15),
    (Key::Slash, 16),
    (Key::Q, 12),
    (Key::Num2, 13),
    (Key::W, 14),
    (Key::Num3, 15),
    (Key::E, 16),
    (Key::R, 17),
    (Key::Num5, 18),
    (Key::T, 19),
    (Key::Num6, 20),
    (Key::Y, 21),
    (Key::Num7, 22),
    (Key::U, 23),
    (Key::I, 24),
    (Key::Num9, 25),
    (Key::O, 26),
    (Key::Num0, 27),
    (Key::P, 28),
    (Key::OpenBracket, 29),
    (Key::Equals, 30),
    (Key::CloseBracket, 31),
];
/// The C the bottom row starts at with the keyboard octave at 0.
const KEYBOARD_BASE: i32 = 48;
const OCTAVES: std::ops::RangeInclusive<i32> = -2..=3;

pub enum TuneResult {
    Open,
    Cancel,
    Apply,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    /// The note follows the pointer, which grabbed it `offset` ticks after its start.
    Move {
        index: usize,
        offset: i64,
    },
    Resize {
        index: usize,
    },
}

/// Playback and recording: the egui clock when the playhead was at `from`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Clock {
    started: f64,
    from: u32,
}

impl Clock {
    fn tick(&self, now: f64) -> u32 {
        self.from + ((now - self.started).max(0.0) * f64::from(TICKS_PER_SECOND)) as u32
    }
}

pub struct TuneDialog {
    pub tune: Tune,
    /// The items of the `n` commands the tune was read from, which applying replaces.
    pub target: Option<std::ops::Range<usize>>,
    sounds: SoundTable,
    /// Pixels per tick of the time axis.
    zoom: f32,
    /// The first tick and the pixel row shown at the top left of the roll.
    scroll: (f32, f32),
    pub playhead: u32,
    selected: Option<usize>,
    drag: Option<Drag>,
    clock: Option<Clock>,
    pub recording: bool,
    /// Notes being played on the keyboard, until their key is released.
    held: HashMap<Key, Vec<TuneNote>>,
    // What new notes are played and recorded with, as in IG's Tap A Tune.
    pub voice: u8,
    pub effect: SoundEffect,
    pub volume: u8,
    pub length: u32,
    pub end: NoteEnd,
    /// Recorded notes keep the default length instead of how long their key was held.
    pub fixed_length: bool,
    pub octave: i32,
    /// The other voices that play along with a pitch offset in semitones, as IG's multi voice.
    pub chord: [Option<i32>; VOICES as usize],
    pub muted: [bool; VOICES as usize],
    pub snap: u32,
    tempo: u32,
    /// egui runs a frame again when its layout changed; input is only handled the first time.
    repeated: bool,
    /// The roll fills the height the window leaves.
    roll_height: f32,
}

impl TuneDialog {
    pub(super) fn new(tune: Tune, target: Option<std::ops::Range<usize>>, sounds: SoundTable) -> Self {
        let first = tune.notes.first().copied();
        Self {
            tune,
            target,
            sounds,
            zoom: 1.0,
            // Middle C in view.
            scroll: (0.0, f32::from(HIGHEST_PITCH - 84) * ROW_HEIGHT),
            playhead: 0,
            selected: None,
            drag: None,
            clock: None,
            recording: false,
            held: HashMap::new(),
            voice: first.map_or(0, |note| note.voice),
            effect: first.map_or(SoundEffect::Longbell, |note| note.effect),
            volume: first.map_or(15, |note| note.volume),
            length: 40,
            end: NoteEnd::Release,
            fixed_length: false,
            octave: 0,
            chord: [None; VOICES as usize],
            muted: [false; VOICES as usize],
            snap: 10,
            tempo: 100,
            repeated: false,
            roll_height: 300.0,
        }
    }

    fn snapped(&self, tick: i64) -> u32 {
        let tick = tick.max(0) as u32;
        (tick + self.snap / 2).checked_div(self.snap).map_or(tick, |steps| steps * self.snap)
    }

    fn chip(&self, note: &TuneNote) -> Sound {
        Sound::Chip {
            data: self.sounds.effects[(note.effect as usize).min(19)].clone(),
            voice: note.voice,
            volume: note.volume,
            pitch: note.pitch,
        }
    }

    fn stop_sound(note: &TuneNote) -> Sound {
        match note.end {
            NoteEnd::Cut => Sound::Cut(note.voice),
            // Held notes would sound forever outside the tune.
            NoteEnd::Release | NoteEnd::Hold => Sound::Release(note.voice),
        }
    }

    /// The notes a key plays: the note on the voice, and the voices playing along.
    fn notes_for_key(&self, semitone: u8, start: u32) -> Vec<TuneNote> {
        let pitch = KEYBOARD_BASE + self.octave * 12 + i32::from(semitone);
        let voices = (0..VOICES).filter_map(|voice| {
            let offset = if voice == self.voice { Some(0) } else { self.chord[usize::from(voice)] }?;
            Some((voice, pitch + offset))
        });
        voices
            .filter(|&(voice, pitch)| !self.muted[usize::from(voice)] && (i32::from(LOWEST_PITCH)..=i32::from(HIGHEST_PITCH)).contains(&pitch))
            .map(|(voice, pitch)| TuneNote {
                start,
                length: self.length,
                voice,
                pitch: pitch as u8,
                effect: self.effect,
                volume: self.volume,
                end: self.end,
            })
            .collect()
    }

    /// Plays the notes of `key` and, while recording, starts recording them at the playhead.
    pub fn press_key(&mut self, key: Key, now: f64, player: &mut SoundPlayer) {
        let Some(&(_, semitone)) = NOTE_KEYS.iter().find(|(candidate, _)| *candidate == key) else {
            return;
        };
        if self.held.contains_key(&key) {
            return;
        }
        let start = match (self.recording, self.clock) {
            (true, Some(clock)) => self.snapped(i64::from(clock.tick(now))),
            _ => 0,
        };
        let notes = self.notes_for_key(semitone, start);
        player.play(notes.iter().map(|note| self.chip(note)).collect());
        self.held.insert(key, notes);
    }

    /// Ends the notes of `key` and keeps them if recording.
    pub fn release_key(&mut self, key: Key, now: f64, player: &mut SoundPlayer) {
        let Some(notes) = self.held.remove(&key) else {
            return;
        };
        player.play(notes.iter().map(Self::stop_sound).collect());
        if !self.recording {
            return;
        }
        let Some(clock) = self.clock else {
            return;
        };
        let end = self.snapped(i64::from(clock.tick(now)));
        for mut note in notes {
            if !self.fixed_length {
                note.length = end.saturating_sub(note.start).max(self.snap.max(1));
            }
            self.tune.notes.push(note);
        }
        self.tune.fit_voices();
    }

    /// Starts playing the tune from the playhead, and recording if armed.
    pub fn play(&mut self, now: f64, player: &mut SoundPlayer) {
        player.stop();
        let clock = Clock {
            started: now,
            from: self.playhead,
        };
        for note in self
            .tune
            .notes
            .iter()
            .filter(|note| note.start >= clock.from && !self.muted[usize::from(note.voice)])
        {
            let at = |tick: u32| clock.started + f64::from(tick - clock.from) / f64::from(TICKS_PER_SECOND);
            player.schedule(at(note.start), self.chip(note));
            if note.end != NoteEnd::Hold {
                player.schedule(at(note.end_tick()), Self::stop_sound(note));
            }
        }
        self.clock = Some(clock);
    }

    /// Stops playing and recording; the playhead stays where it got to.
    pub fn stop(&mut self, now: f64, player: &mut SoundPlayer) {
        if let Some(clock) = self.clock.take() {
            self.playhead = clock.tick(now);
        }
        let held: Vec<Key> = self.held.keys().copied().collect();
        for key in held {
            self.release_key(key, now, player);
        }
        self.recording = false;
        player.stop();
    }

    pub fn toggle_record(&mut self, now: f64, player: &mut SoundPlayer) {
        if self.recording {
            self.stop(now, player);
        } else {
            self.recording = true;
            if self.clock.is_none() {
                self.play(now, player);
            }
        }
    }

    pub fn clear(&mut self, now: f64, player: &mut SoundPlayer) {
        self.stop(now, player);
        self.tune.notes.clear();
        self.selected = None;
        self.playhead = 0;
    }

    /// Adds a note at `start` and `pitch` on the voice with the note settings, selects it
    /// and plays it.
    pub fn add_note(&mut self, start: u32, pitch: u8, player: &mut SoundPlayer, now: f64) -> usize {
        let note = TuneNote {
            start,
            length: self.length,
            voice: self.voice,
            pitch: pitch.clamp(LOWEST_PITCH, HIGHEST_PITCH),
            effect: self.effect,
            volume: self.volume,
            end: self.end,
        };
        self.preview(&note, player, now);
        self.tune.notes.push(note);
        self.selected = Some(self.tune.notes.len() - 1);
        self.selected.unwrap_or_default()
    }

    fn preview(&self, note: &TuneNote, player: &mut SoundPlayer, now: f64) {
        player.play(vec![self.chip(note)]);
        let seconds = f64::from(note.length.clamp(10, 100)) / f64::from(TICKS_PER_SECOND);
        player.schedule(now + seconds, Self::stop_sound(note));
    }

    /// Keeps one note per voice after an edit; the edited note wins and stays selected.
    fn settle(&mut self) {
        let edited = self
            .selected
            .and_then(|index| (index < self.tune.notes.len()).then(|| self.tune.notes.remove(index)));
        if let Some(note) = edited {
            self.tune.notes.push(note);
        }
        self.tune.fit_voices();
        self.selected = edited.and_then(|note| self.tune.notes.iter().position(|candidate| *candidate == note));
    }

    pub fn delete_selected(&mut self) {
        if let Some(index) = self.selected.take().filter(|&index| index < self.tune.notes.len()) {
            self.tune.notes.remove(index);
        }
    }

    #[cfg(test)]
    pub fn selected_note(&self) -> Option<&TuneNote> {
        self.selected.and_then(|index| self.tune.notes.get(index))
    }

    pub fn show(&mut self, context: &egui::Context, player: &mut SoundPlayer) -> TuneResult {
        #[derive(Clone, Copy)]
        enum Action {
            Cancel,
            Apply,
        }
        let now = context.input(|input| input.time);
        self.repeated = context.current_pass_index() > 0;
        self.roll_height = (context.content_rect().height() - AROUND_ROLL).clamp(140.0, 560.0);
        if let Some(clock) = self.clock {
            // Playback ends after the last note unless recording goes on.
            if !self.recording && self.held.is_empty() && clock.tick(now) > self.tune.duration() + TICKS_PER_SECOND / 2 {
                self.playhead = 0;
                self.clock = None;
                // Held notes would sound on after the tune.
                player.play(vec![Sound::ReleaseAll]);
            } else {
                context.request_repaint();
            }
        }
        let response = appearance::Dialog::new("igs-tune")
            .title(fl!("igs-tune-title"))
            .subtitle(fl!("igs-tune-subtitle"))
            .size(DialogSize::Width(1080.0))
            .max_height(context.content_rect().height())
            .confirm_on_enter(false)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    self.keyboard_input(ui, now, player);
                    self.transport_row(ui, now, player);
                    ui.add_space(4.0);
                    self.note_row(ui);
                    ui.add_space(4.0);
                    self.voice_row(ui);
                    ui.add_space(6.0);
                    self.roll(ui, now, player);
                    ui.add_space(4.0);
                    self.selection_row(ui);
                    ui.weak(fl!("igs-tune-keys-hint"));
                });
                dialog.buttons([
                    DialogButton::cancel(labels::cancel(), Action::Cancel),
                    DialogButton::primary(fl!("button-apply"), Action::Apply),
                ]);
            });
        let result = match response.action {
            Some(Action::Apply) => TuneResult::Apply,
            Some(Action::Cancel) => TuneResult::Cancel,
            None if response.dismissed => TuneResult::Cancel,
            None => TuneResult::Open,
        };
        if !matches!(result, TuneResult::Open) {
            self.stop(now, player);
        }
        result
    }

    /// The note keys and IG's Tap A Tune shortcuts, while no text field has the keyboard.
    fn keyboard_input(&mut self, ui: &egui::Ui, now: f64, player: &mut SoundPlayer) {
        if self.repeated || ui.memory(|memory| memory.focused().is_some()) {
            return;
        }
        let events = ui.input(|input| input.events.clone());
        for event in events {
            let egui::Event::Key {
                key,
                physical_key,
                pressed,
                repeat,
                modifiers,
            } = event
            else {
                continue;
            };
            let key = physical_key.unwrap_or(key);
            if repeat || modifiers.command || modifiers.ctrl {
                continue;
            }
            if !pressed {
                self.release_key(key, now, player);
                continue;
            }
            match key {
                Key::F1 => self.toggle_record(now, player),
                Key::F2 | Key::Space => {
                    if self.clock.is_some() {
                        self.stop(now, player);
                    } else {
                        self.play(now, player);
                    }
                }
                Key::F3 => self.clear(now, player),
                Key::ArrowLeft => self.length = self.length.saturating_sub(10).max(1),
                Key::ArrowRight => self.length = (self.length + 10).min(9999),
                Key::ArrowUp => {
                    self.end = match self.end {
                        NoteEnd::Release => NoteEnd::Cut,
                        NoteEnd::Cut => NoteEnd::Hold,
                        NoteEnd::Hold => NoteEnd::Release,
                    }
                }
                Key::ArrowDown => {
                    self.volume = if modifiers.shift {
                        (self.volume + 1).min(15)
                    } else {
                        self.volume.saturating_sub(1)
                    }
                }
                Key::PageUp => self.octave = (self.octave + 1).min(*OCTAVES.end()),
                Key::PageDown => self.octave = (self.octave - 1).max(*OCTAVES.start()),
                Key::Home => self.octave = 0,
                Key::Delete | Key::Backspace => self.delete_selected(),
                _ => self.press_key(key, now, player),
            }
        }
    }

    fn transport_row(&mut self, ui: &mut egui::Ui, now: f64, player: &mut SoundPlayer) {
        ui.horizontal(|ui| {
            let playing = self.clock.is_some();
            if ui
                .button(if playing { fl!("igs-tune-stop") } else { fl!("igs-tune-play") })
                .on_hover_text(fl!("igs-tune-play-tooltip"))
                .clicked()
                && !self.repeated
            {
                if playing {
                    self.stop(now, player);
                } else {
                    self.play(now, player);
                }
            }
            let record = egui::Button::new(egui::RichText::new(format!("● {}", fl!("igs-tune-record"))).color(if self.recording {
                Color32::from_rgb(0xE0, 0x40, 0x40)
            } else {
                ui.visuals().text_color()
            }))
            .selected(self.recording);
            if ui.add(record).on_hover_text(fl!("igs-tune-record-tooltip")).clicked() && !self.repeated {
                self.toggle_record(now, player);
            }
            ui.weak(if self.recording {
                fl!("igs-tune-recording")
            } else {
                fl!("igs-tune-practice")
            });
            ui.separator();
            let at = self.clock.map_or(self.playhead, |clock| clock.tick(now));
            ui.monospace(format!("{} / {}", seconds(at), seconds(self.tune.duration())));
            ui.separator();
            ui.label(fl!("igs-tune-snap"));
            egui::ComboBox::from_id_salt("igs-tune-snap")
                .selected_text(snap_name(self.snap))
                .show_ui(ui, |ui| {
                    for snap in SNAPS {
                        ui.selectable_value(&mut self.snap, snap, snap_name(snap));
                    }
                });
            ui.label(fl!("igs-tune-zoom"));
            ui.add(egui::Slider::new(&mut self.zoom, 0.1..=6.0).logarithmic(true).show_value(false));
            ui.separator();
            ui.label(fl!("igs-tune-tempo"));
            ui.add(egui::DragValue::new(&mut self.tempo).range(10..=1000).suffix(" %"));
            if ui
                .add_enabled(self.tempo != 100 && !self.tune.notes.is_empty(), egui::Button::new(fl!("igs-tune-tempo-apply")))
                .on_hover_text(fl!("igs-tune-tempo-tooltip"))
                .clicked()
                && !self.repeated
            {
                self.tune.scale_time(self.tempo);
                self.tempo = 100;
                self.selected = None;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(fl!("igs-tune-clear")).on_hover_text(fl!("igs-tune-clear-tooltip")).clicked() && !self.repeated {
                    self.clear(now, player);
                }
            });
        });
    }

    fn note_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(fl!("igs-tune-instrument"));
            effect_picker(ui, "igs-tune-effect", &mut self.effect);
            ui.label(fl!("igs-chip-volume"));
            ui.add(egui::DragValue::new(&mut self.volume).range(0..=15));
            ui.label(fl!("igs-tune-length"));
            ui.add(egui::DragValue::new(&mut self.length).range(1..=9999).suffix(" /200 s"))
                .on_hover_text(fl!("igs-tune-length-tooltip"));
            ui.checkbox(&mut self.fixed_length, fl!("igs-tune-fixed-length"))
                .on_hover_text(fl!("igs-tune-fixed-length-tooltip"));
            ui.label(fl!("igs-tune-end"));
            end_picker(ui, "igs-tune-end", &mut self.end);
            ui.separator();
            ui.label(fl!("igs-tune-octave"));
            if ui.add_enabled(self.octave > *OCTAVES.start(), egui::Button::new("−")).clicked() {
                self.octave -= 1;
            }
            ui.monospace(note_name((KEYBOARD_BASE + self.octave * 12) as u8));
            if ui.add_enabled(self.octave < *OCTAVES.end(), egui::Button::new("+")).clicked() {
                self.octave += 1;
            }
        });
    }

    fn voice_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(fl!("igs-tune-voice"));
            for voice in 0..VOICES {
                let index = usize::from(voice);
                let color = VOICE_COLORS[index];
                ui.group(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let (swatch, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(swatch, 2.0, if self.muted[index] { color.gamma_multiply(0.3) } else { color });
                    ui.radio_value(&mut self.voice, voice, (voice + 1).to_string())
                        .on_hover_text(fl!("igs-tune-voice-tooltip"));
                    let mut on = !self.muted[index];
                    if ui
                        .checkbox(&mut on, fl!("igs-tune-voice-on"))
                        .on_hover_text(fl!("igs-tune-voice-on-tooltip"))
                        .changed()
                    {
                        self.muted[index] = !on;
                    }
                    if voice != self.voice {
                        let mut along = self.chord[index].is_some();
                        if ui
                            .checkbox(&mut along, fl!("igs-tune-along"))
                            .on_hover_text(fl!("igs-tune-along-tooltip"))
                            .changed()
                        {
                            self.chord[index] = along.then_some(0);
                        }
                        if let Some(offset) = &mut self.chord[index] {
                            for (label, step) in [("−12", -12), ("−1", -1)] {
                                if ui.small_button(label).clicked() {
                                    *offset = (*offset + step).clamp(-48, 48);
                                }
                            }
                            ui.add(egui::DragValue::new(offset).range(-48..=48));
                            for (label, step) in [("+1", 1), ("+12", 12)] {
                                if ui.small_button(label).clicked() {
                                    *offset = (*offset + step).clamp(-48, 48);
                                }
                            }
                        }
                    }
                });
            }
        });
    }

    /// The selected note's own settings, in a row of fixed height so the dialog keeps its
    /// place when a note is selected.
    fn selection_row(&mut self, ui: &mut egui::Ui) {
        let height = ui.spacing().interact_size.y + 4.0;
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_height(height);
                let Some(index) = self.selected.filter(|&index| index < self.tune.notes.len()) else {
                    ui.weak(fl!("igs-tune-selection-hint"));
                    return;
                };
                let mut note = self.tune.notes[index];
                ui.strong(note_name(note.pitch));
                ui.label(fl!("igs-tune-start"));
                ui.add(egui::DragValue::new(&mut note.start).range(0..=1_000_000));
                ui.label(fl!("igs-tune-length"));
                ui.add(egui::DragValue::new(&mut note.length).range(1..=100_000));
                ui.label(fl!("igs-chip-pitch"));
                ui.add(egui::DragValue::new(&mut note.pitch).range(LOWEST_PITCH..=HIGHEST_PITCH));
                ui.label(fl!("igs-tune-voice"));
                let mut voice = note.voice + 1;
                ui.add(egui::DragValue::new(&mut voice).range(1..=VOICES));
                note.voice = voice - 1;
                effect_picker(ui, "igs-tune-note-effect", &mut note.effect);
                ui.label(fl!("igs-chip-volume"));
                ui.add(egui::DragValue::new(&mut note.volume).range(0..=15));
                end_picker(ui, "igs-tune-note-end", &mut note.end);
                if ui.button(fl!("igs-editor-delete")).clicked() {
                    self.delete_selected();
                    return;
                }
                if note != self.tune.notes[index] {
                    self.tune.notes[index] = note;
                    self.settle();
                }
            },
        );
    }

    fn roll(&mut self, ui: &mut egui::Ui, now: f64, player: &mut SoundPlayer) {
        let size = egui::vec2(ui.available_width(), self.roll_height);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let keyboard = egui::Rect::from_min_max(
            rect.min + egui::vec2(0.0, RULER_HEIGHT),
            egui::pos2(rect.left() + KEYBOARD_WIDTH, rect.bottom()),
        );
        let ruler = egui::Rect::from_min_max(egui::pos2(keyboard.right(), rect.top()), egui::pos2(rect.right(), rect.top() + RULER_HEIGHT));
        let grid = egui::Rect::from_min_max(egui::pos2(keyboard.right(), ruler.bottom()), rect.max);
        let rows = f32::from(HIGHEST_PITCH - LOWEST_PITCH + 1) * ROW_HEIGHT;
        self.scroll.1 = self.scroll.1.clamp(0.0, (rows - grid.height()).max(0.0));
        if response.hovered() {
            let (delta, modifiers) = ui.input(|input| (input.raw_scroll_delta, input.modifiers));
            if modifiers.command || modifiers.ctrl {
                if delta.y != 0.0 {
                    self.zoom = (self.zoom * (1.0 + delta.y * 0.002)).clamp(0.1, 6.0);
                }
            } else if modifiers.shift {
                self.scroll.0 = (self.scroll.0 - (delta.y + delta.x) / self.zoom).max(0.0);
            } else {
                self.scroll.0 = (self.scroll.0 - delta.x / self.zoom).max(0.0);
                self.scroll.1 = (self.scroll.1 - delta.y).clamp(0.0, (rows - grid.height()).max(0.0));
            }
        }
        let (zoom, scroll) = (self.zoom, self.scroll);
        let x_of = move |tick: f32| grid.left() + (tick - scroll.0) * zoom;
        let tick_of = move |x: f32| ((x - grid.left()) / zoom + scroll.0) as i64;
        let y_of = move |pitch: u8| grid.top() + f32::from(HIGHEST_PITCH - pitch) * ROW_HEIGHT - scroll.1;
        let pitch_of = move |y: f32| {
            let row = ((y - grid.top() + scroll.1) / ROW_HEIGHT).floor() as i32;
            (i32::from(HIGHEST_PITCH) - row).clamp(i32::from(LOWEST_PITCH), i32::from(HIGHEST_PITCH)) as u8
        };
        let note_rect = |note: &TuneNote| {
            egui::Rect::from_min_size(
                egui::pos2(x_of(note.start as f32), y_of(note.pitch) + 1.0),
                egui::vec2((note.length as f32 * zoom).max(3.0), ROW_HEIGHT - 2.0),
            )
        };
        // The whole row of a note is its target, not just the bar drawn inside it.
        let hit_rect = |note: &TuneNote| note_rect(note).expand2(egui::vec2(0.0, 1.0));

        // Editing with the pointer.
        let pointer = response.interact_pointer_pos();
        // Once, when the button goes down: a press both starts a drag and ends as a click.
        let pressed = !self.repeated && response.contains_pointer() && ui.input(|input| input.pointer.primary_pressed());
        if pressed {
            if let Some(pos) = ui.input(|input| input.pointer.interact_pos()) {
                if grid.contains(pos) {
                    let hit = self.tune.notes.iter().rposition(|note| hit_rect(note).contains(pos));
                    self.drag = Some(match hit {
                        Some(index) if hit_rect(&self.tune.notes[index]).right() - pos.x <= RESIZE_EDGE => {
                            self.selected = Some(index);
                            Drag::Resize { index }
                        }
                        Some(index) => {
                            self.selected = Some(index);
                            let note = self.tune.notes[index];
                            self.preview(&note, player, now);
                            Drag::Move {
                                index,
                                offset: tick_of(pos.x) - i64::from(note.start),
                            }
                        }
                        None => {
                            let start = self.snapped(tick_of(pos.x));
                            let index = self.add_note(start, pitch_of(pos.y), player, now);
                            Drag::Move { index, offset: 0 }
                        }
                    });
                } else if keyboard.contains(pos) {
                    let note = TuneNote {
                        start: 0,
                        length: self.length,
                        voice: self.voice,
                        pitch: pitch_of(pos.y),
                        effect: self.effect,
                        volume: self.volume,
                        end: self.end,
                    };
                    self.preview(&note, player, now);
                } else if ruler.contains(pos) {
                    self.playhead = self.snapped(tick_of(pos.x));
                }
            }
        }
        if let (Some(drag), Some(pos)) = (self.drag, pointer) {
            match drag {
                Drag::Move { index, offset } if index < self.tune.notes.len() => {
                    let start = self.snapped(tick_of(pos.x) - offset);
                    let pitch = pitch_of(pos.y);
                    let note = &mut self.tune.notes[index];
                    if note.pitch != pitch {
                        note.pitch = pitch;
                        let note = *note;
                        self.preview(&note, player, now);
                    }
                    self.tune.notes[index].start = start;
                }
                Drag::Resize { index } if index < self.tune.notes.len() => {
                    let start = self.tune.notes[index].start;
                    let end = tick_of(pos.x).max(i64::from(start) + 1);
                    let end = if self.snap == 0 { end as u32 } else { self.snapped(end).max(start + 1) };
                    self.tune.notes[index].length = end - start;
                }
                _ => {}
            }
        }
        if self.drag.is_some() && !ui.input(|input| input.pointer.any_down()) {
            self.drag = None;
            self.settle();
        }
        if response.secondary_clicked() && !self.repeated {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(index) = self.tune.notes.iter().rposition(|note| hit_rect(note).contains(pos)) {
                    self.tune.notes.remove(index);
                    self.selected = None;
                }
            }
        }

        // Drawing.
        let painter = ui.painter_at(rect);
        let visuals = ui.visuals();
        painter.rect_filled(rect, 0.0, visuals.extreme_bg_color);
        let grid_painter = painter.with_clip_rect(grid);
        for pitch in LOWEST_PITCH..=HIGHEST_PITCH {
            let y = y_of(pitch);
            if y > grid.bottom() || y + ROW_HEIGHT < grid.top() {
                continue;
            }
            let row = egui::Rect::from_min_size(egui::pos2(grid.left(), y), egui::vec2(grid.width(), ROW_HEIGHT));
            if is_black_key(pitch) {
                grid_painter.rect_filled(row, 0.0, visuals.faint_bg_color);
            }
            if pitch % 12 == 0 {
                grid_painter.line_segment(
                    [row.left_bottom(), row.right_bottom()],
                    Stroke::new(1.0, visuals.weak_text_color().gamma_multiply(0.4)),
                );
            }
        }
        // A line every second and, zoomed in, at every snap step.
        let first = scroll.0.max(0.0) as u32;
        let last = (scroll.0 + grid.width() / zoom) as u32;
        let step = if self.snap > 0 && self.snap as f32 * zoom >= 6.0 {
            self.snap
        } else {
            TICKS_PER_SECOND
        };
        let mut tick = first / step * step;
        while tick <= last {
            let x = x_of(tick as f32);
            let strong = tick.is_multiple_of(TICKS_PER_SECOND);
            let color = visuals.weak_text_color().gamma_multiply(if strong { 0.5 } else { 0.15 });
            grid_painter.line_segment([egui::pos2(x, grid.top()), egui::pos2(x, grid.bottom())], Stroke::new(1.0, color));
            if strong {
                painter.with_clip_rect(ruler).text(
                    egui::pos2(x + 3.0, ruler.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("{}s", tick / TICKS_PER_SECOND),
                    egui::FontId::proportional(10.0),
                    visuals.weak_text_color(),
                );
            }
            tick += step;
        }
        // The notes, those being recorded included.
        let held: Vec<TuneNote> = match self.clock {
            Some(clock) if self.recording => self
                .held
                .values()
                .flatten()
                .map(|note| TuneNote {
                    length: clock.tick(now).saturating_sub(note.start).max(1),
                    ..*note
                })
                .collect(),
            _ => Vec::new(),
        };
        for (index, note) in self.tune.notes.iter().chain(&held).enumerate() {
            let color = VOICE_COLORS[usize::from(note.voice.min(VOICES - 1))];
            let color = if self.muted[usize::from(note.voice.min(VOICES - 1))] {
                color.gamma_multiply(0.3)
            } else {
                color
            };
            let bar = note_rect(note);
            grid_painter.rect_filled(bar, 2.0, color);
            // The end: a fade for a release, a hard edge for a cut, an open end for a hold.
            match note.end {
                NoteEnd::Release => {
                    let fade = egui::Rect::from_min_max(egui::pos2((bar.right() - 6.0).max(bar.left()), bar.top()), bar.max);
                    grid_painter.rect_filled(fade, 0.0, color.gamma_multiply(0.5));
                }
                NoteEnd::Cut => {
                    grid_painter.line_segment([bar.right_top(), bar.right_bottom()], Stroke::new(2.0, Color32::BLACK));
                }
                NoteEnd::Hold => {
                    grid_painter.line_segment([bar.right_center(), bar.right_center() + egui::vec2(8.0, 0.0)], Stroke::new(1.0, color));
                }
            }
            if Some(index) == self.selected {
                grid_painter.rect_stroke(bar.expand(1.0), 2.0, Stroke::new(2.0, visuals.strong_text_color()), egui::StrokeKind::Outside);
            }
        }
        // The keyboard, with the octaves named at their C.
        let keys = painter.with_clip_rect(keyboard);
        for pitch in LOWEST_PITCH..=HIGHEST_PITCH {
            let y = y_of(pitch);
            let key = egui::Rect::from_min_size(egui::pos2(keyboard.left(), y), egui::vec2(KEYBOARD_WIDTH, ROW_HEIGHT));
            let black = is_black_key(pitch);
            keys.rect_filled(
                if black {
                    egui::Rect::from_min_size(key.min, egui::vec2(KEYBOARD_WIDTH * 0.62, ROW_HEIGHT))
                } else {
                    key
                },
                0.0,
                if black { Color32::from_gray(30) } else { Color32::from_gray(235) },
            );
            keys.line_segment([key.left_bottom(), key.right_bottom()], Stroke::new(1.0, Color32::from_gray(150)));
            if pitch % 12 == 0 {
                keys.text(
                    key.right_center() - egui::vec2(3.0, 0.0),
                    egui::Align2::RIGHT_CENTER,
                    note_name(pitch),
                    egui::FontId::proportional(9.0),
                    Color32::from_gray(60),
                );
            }
        }
        // The playhead.
        let head = self.clock.map_or(self.playhead, |clock| clock.tick(now));
        let x = x_of(head as f32);
        if (grid.left()..=grid.right()).contains(&x) {
            let color = if self.recording {
                Color32::from_rgb(0xE0, 0x40, 0x40)
            } else {
                visuals.selection.stroke.color
            };
            painter.line_segment([egui::pos2(x, ruler.top()), egui::pos2(x, grid.bottom())], Stroke::new(2.0, color));
        } else if self.clock.is_some() && x > grid.right() {
            // Keep the playhead in view while playing.
            self.scroll.0 = head as f32 - grid.width() / zoom * 0.1;
        }
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
            egui::StrokeKind::Inside,
        );
    }
}

fn seconds(ticks: u32) -> String {
    format!(
        "{}:{:05.2}",
        ticks / TICKS_PER_SECOND / 60,
        f64::from(ticks % (60 * TICKS_PER_SECOND)) / f64::from(TICKS_PER_SECOND)
    )
}

fn snap_name(snap: u32) -> String {
    if snap == 0 {
        fl!("igs-tune-snap-off")
    } else {
        format!("{} ms", snap * 1000 / TICKS_PER_SECOND)
    }
}

fn end_name(end: NoteEnd) -> String {
    match end {
        NoteEnd::Release => fl!("igs-chip-stop-release"),
        NoteEnd::Cut => fl!("igs-chip-stop-voice"),
        NoteEnd::Hold => fl!("igs-chip-stop-none"),
    }
}

fn end_picker(ui: &mut egui::Ui, id: &str, value: &mut NoteEnd) {
    egui::ComboBox::from_id_salt(id).selected_text(end_name(*value)).show_ui(ui, |ui| {
        for end in [NoteEnd::Release, NoteEnd::Cut, NoteEnd::Hold] {
            ui.selectable_value(value, end, end_name(end));
        }
    });
}

fn effect_picker(ui: &mut egui::Ui, id: &str, value: &mut SoundEffect) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(format!("{} · {}", *value as usize, super::sound_name(*value as usize)))
        .show_ui(ui, |ui| {
            for index in 0..20 {
                if let Ok(effect) = SoundEffect::try_from(index as i32) {
                    ui.selectable_value(value, effect, format!("{index} · {}", super::sound_name(index)));
                }
            }
        });
}
