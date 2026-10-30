//! The ANSI music editor: a piano roll of the notes of a music string and the string itself,
//! kept in sync. Notes play from the keyboard like in a tracker, and the roll follows playback.

use std::ops::Range;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, Key, Stroke};
use icy_draw::ansi_music::{self, Articulation, Event, Introducer, Parsed, HIGHEST_PITCH, LENGTHS, OCTAVES};
use icy_draw::fl;
use icy_engine_gui::egui::appearance::PRIMARY;
use icy_parser_core::AnsiMusic;

use super::widgets::{self, Icons};

const KEYBOARD_WIDTH: f32 = 52.0;
const ROW_HEIGHT: f32 = 12.0;
const RULER_HEIGHT: f32 = 20.0;
/// The lane under the ruler: setting changes on top, rests below them.
const LANE_HEIGHT: f32 = 38.0;
const MARKER_HEIGHT: f32 = 18.0;
/// The player bar under the roll.
const PLAYER_HEIGHT: f32 = 66.0;
const RANGE_COLOR: Color32 = Color32::from_rgb(0x2E, 0xA0, 0x43);
/// Pixels per whole note at 100% zoom.
const WHOLE_NOTE: f32 = 160.0;
/// Pixels from a note's right end that change its length rather than its pitch.
const RESIZE_EDGE: f32 = 5.0;
pub(super) const NOTE_COLOR: Color32 = Color32::from_rgb(0x4C, 0x9A, 0xFF);
const MARKER_COLOR: Color32 = Color32::from_rgb(0xF5, 0xA6, 0x23);

/// The keys that enter notes, as semitones above C of the keyboard octave: the bottom row is
/// the white keys, the row above the black ones, like trackers.
const NOTE_KEYS: [(Key, u8); 12] = [
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
];
/// 1 to 7 pick a whole note to a 64th.
const LENGTH_KEYS: [Key; 7] = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7];

/// The audio output, opened on the first sound.
#[derive(Default)]
pub(super) struct Player {
    #[cfg(not(test))]
    thread: Option<icy_engine_gui::music::SoundThread>,
    #[cfg(test)]
    pub(super) played: Vec<AnsiMusic>,
    #[cfg(test)]
    pub(super) stopped: usize,
}

impl Player {
    /// Plays `music` instead of whatever plays now.
    fn play(&mut self, music: AnsiMusic) {
        self.stop();
        #[cfg(test)]
        self.played.push(music);
        #[cfg(not(test))]
        {
            let thread = self.thread.get_or_insert_with(icy_engine_gui::music::SoundThread::new);
            let _ = thread.play_music(music);
        }
    }

    fn stop(&mut self) {
        #[cfg(test)]
        {
            self.stopped += 1;
        }
        #[cfg(not(test))]
        if let Some(thread) = &self.thread {
            thread.clear();
        }
    }
}

impl Drop for Player {
    /// Closing the music also silences it.
    fn drop(&mut self) {
        #[cfg(not(test))]
        self.stop();
    }
}

struct Playback {
    /// The egui time playback started at.
    started: f64,
    /// The events played, which loop playback repeats.
    range: Range<usize>,
    /// When each event starts and how long it lasts, in milliseconds.
    timing: Vec<(f64, f64)>,
}

impl Playback {
    fn position(&self, now: f64) -> f64 {
        self.timing[self.range.start].0 + (now - self.started) * 1000.0
    }

    fn end(&self) -> f64 {
        let (start, duration) = self.timing[self.range.end - 1];
        start + duration
    }
}

/// What dragging with the pointer changes.
#[derive(Clone)]
enum Drag {
    /// Notes being moved: up or down to another pitch, sideways to another place.
    Notes {
        block: Range<usize>,
        /// The events when the drag started; every frame applies the whole move to them.
        origin: Vec<Event>,
        grab_pitch: u8,
        grab_x: f32,
        /// The pitch change previewed last, to sound each new pitch once.
        heard: i32,
    },
    Length(usize),
    /// The ruler, from this position: a click sets where playback starts, a drag the range it plays.
    Ruler {
        from: f32,
        moved: bool,
    },
}

/// Common time signatures: beats per measure and the note value of a beat.
const METERS: [(u8, u8); 6] = [(2, 4), (3, 4), (4, 4), (5, 4), (6, 8), (12, 8)];
/// Pointer movement that turns a click on a note into moving it sideways.
const MOVE_THRESHOLD: f32 = 6.0;

pub struct MusicEditor {
    text: String,
    parsed: Parsed,
    introducer: Introducer,
    path: Option<PathBuf>,
    /// The file as read or last written, to notice changes made by others before overwriting it.
    baseline: Option<Vec<u8>>,
    saved: (String, Introducer),
    undo_stack: Vec<String>,
    redo_stack: Vec<String>,
    /// Typing in the text view has an undo step since the field took focus.
    typing: bool,
    /// The selected event; with `anchor`, one end of the selected range.
    pub(super) selected: Option<usize>,
    /// The other end of a selected range.
    anchor: Option<usize>,
    /// Playback starts over at the end.
    looping: bool,
    /// Where playback starts without a range or a selection, in whole notes.
    cursor: f32,
    /// The range playback plays, in whole notes, as set on the ruler.
    play_range: Option<(f32, f32)>,
    /// The time signature the ruler shows measures in; ANSI music itself has none.
    meter: (u8, u8),
    /// The length and dot of entered notes.
    length: u8,
    dotted: bool,
    /// The octave the note keys play.
    octave: u8,
    zoom: f32,
    /// Horizontal scroll in whole notes, vertical in pixels.
    scroll: egui::Vec2,
    scrolled_to_notes: bool,
    /// What is being dragged, and whether that has an undo step yet.
    drag: Option<(Drag, bool)>,
    playback: Option<Playback>,
    pub(super) player: Player,
    icons: Icons,
}

impl Default for MusicEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl MusicEditor {
    pub fn new() -> Self {
        Self::from_text(String::new(), Introducer::default())
    }

    fn from_text(text: String, introducer: Introducer) -> Self {
        Self {
            parsed: ansi_music::parse(&text),
            saved: (text.clone(), introducer),
            text,
            introducer,
            path: None,
            baseline: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            typing: false,
            selected: None,
            anchor: None,
            looping: false,
            cursor: 0.0,
            play_range: None,
            meter: (4, 4),
            length: ansi_music::DEFAULT_LENGTH,
            dotted: false,
            octave: ansi_music::DEFAULT_OCTAVE,
            zoom: 1.0,
            scroll: egui::Vec2::ZERO,
            scrolled_to_notes: false,
            drag: None,
            playback: None,
            player: Player::default(),
            icons: Icons::default(),
        }
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
        let file = ansi_music::decode(&bytes).map_err(|_| fl!("music-error-no-music"))?;
        let mut editor = Self::from_text(file.text, file.introducer);
        editor.path = Some(path.to_path_buf());
        editor.baseline = Some(bytes);
        Ok(editor)
    }

    pub fn recovery_snapshot(&self) -> icy_draw::recovery::Snapshot {
        let introducer = Introducer::ALL.iter().position(|value| *value == self.introducer).unwrap_or(0) as u8;
        let mut payload = vec![introducer];
        payload.extend_from_slice(self.text.as_bytes());
        icy_draw::recovery::Snapshot {
            kind: icy_draw::recovery::RecoveryKind::Music,
            path: self.path.clone(),
            disk: self
                .path
                .as_ref()
                .zip(self.baseline.as_ref())
                .map(|(_, bytes)| icy_draw::recovery::Fingerprint::of(bytes)),
            payload,
        }
    }

    /// Restores a [`Self::recovery_snapshot`]; it counts as modified until saved.
    pub fn from_recovery(snapshot: &icy_draw::recovery::Snapshot) -> Result<Self, String> {
        let (&introducer, text) = snapshot.payload.split_first().ok_or("empty music recovery")?;
        let introducer = *Introducer::ALL.get(usize::from(introducer)).ok_or("unknown music introducer")?;
        let text = String::from_utf8(text.to_vec()).map_err(|error| error.to_string())?;
        let mut editor = Self::from_text(text, introducer);
        editor.path = snapshot.path.clone();
        // Saving expects this on disk; without the matching file it asks before overwriting.
        editor.baseline = snapshot
            .path
            .as_deref()
            .zip(snapshot.disk)
            .and_then(|(path, disk)| disk.read_matching(path))
            .or_else(|| snapshot.path.as_ref().map(|_| vec![0]));
        editor.saved = (String::from("\0"), introducer);
        Ok(editor)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn modified(&self) -> bool {
        (self.text.as_str(), self.introducer) != (self.saved.0.as_str(), self.saved.1)
    }

    pub fn save(&mut self, path: &Path, overwrite: bool) -> Result<(), String> {
        let bytes = ansi_music::encode(&self.text, self.introducer).map_err(|problem| problem_message(&self.text, &problem))?;
        icy_draw::files::save_bytes(path, &bytes, self.path.as_deref().zip(self.baseline.as_deref()), overwrite)?;
        self.path = Some(path.to_path_buf());
        self.baseline = Some(bytes);
        self.saved = (self.text.clone(), self.introducer);
        Ok(())
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    #[cfg(test)]
    pub fn events(&self) -> &[Event] {
        &self.parsed.events
    }

    #[cfg(test)]
    pub(super) fn range_set(&self) -> Option<(f32, f32)> {
        self.play_range
    }

    #[cfg(test)]
    pub(super) fn cursor(&self) -> f32 {
        self.cursor
    }

    pub fn introducer(&self) -> Introducer {
        self.introducer
    }

    /// Replaces the music text, as one undo step: the assistant's proposal.
    pub fn apply_text(&mut self, text: String) {
        if text != self.text {
            self.undo_stack.push(std::mem::replace(&mut self.text, text));
            self.redo_stack.clear();
            self.typing = false;
            self.stop();
            self.reparse();
        }
    }

    /// Plays music that is not the document's, such as a proposal.
    pub(super) fn listen(&mut self, music: AnsiMusic) {
        self.playback = None;
        self.player.play(music);
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn has_selection(&self) -> bool {
        self.selected.is_some_and(|index| index < self.parsed.events.len())
    }

    pub fn undo(&mut self, redo: bool) {
        let (from, to) = if redo {
            (&mut self.redo_stack, &mut self.undo_stack)
        } else {
            (&mut self.undo_stack, &mut self.redo_stack)
        };
        if let Some(text) = from.pop() {
            to.push(std::mem::replace(&mut self.text, text));
            self.reparse();
            self.typing = false;
            self.stop();
        }
    }

    fn reparse(&mut self) {
        self.parsed = ansi_music::parse(&self.text);
        let last = self.parsed.events.len().checked_sub(1);
        if self.selected.is_some_and(|index| index >= self.parsed.events.len()) {
            self.selected = last;
        }
        if self.anchor.is_some_and(|index| index >= self.parsed.events.len()) {
            self.anchor = last;
        }
    }

    /// Selects `index`; `extend` keeps the other end of the range where it is.
    pub(super) fn select(&mut self, index: Option<usize>, extend: bool) {
        if extend && index.is_some() {
            self.anchor = self.anchor.or(self.selected);
        } else {
            self.anchor = None;
        }
        self.selected = index;
        // Playback starts at the note picked last.
        if let Some(index) = index {
            self.cursor = self.positions().0.get(index).copied().unwrap_or(0.0);
        }
    }

    /// The first event that starts at or after `position`; the setting changes right before a
    /// note belong to it.
    fn index_at(&self, position: f32) -> usize {
        let (starts, _) = self.positions();
        starts.iter().position(|start| *start >= position - 1e-4).unwrap_or(starts.len())
    }

    /// The start or end of the event nearest to `position`.
    fn snap(&self, position: f32) -> f32 {
        let (starts, end) = self.positions();
        starts
            .into_iter()
            .chain(std::iter::once(end))
            .min_by(|a, b| (a - position).abs().total_cmp(&(b - position).abs()))
            .unwrap_or(0.0)
    }

    /// What Play plays: the range set on the ruler, the selected notes, or from the cursor on.
    fn playback_range(&self) -> Range<usize> {
        let count = self.parsed.events.len();
        if let Some((from, to)) = self.play_range {
            let start = self.index_at(from);
            return start..self.index_at(to).max(start + 1).min(count);
        }
        match self.selection() {
            Some(range) if range.len() > 1 => range,
            _ => self.index_at(self.cursor).min(count.saturating_sub(1))..count,
        }
    }

    /// The selected notes become the range playback plays.
    pub(super) fn range_from_selection(&mut self) {
        if let Some(range) = self.selection() {
            let (starts, end) = self.positions();
            let to = starts.get(range.end).copied().unwrap_or(end);
            self.play_range = Some((starts[range.start], to));
        }
    }

    /// Length of a measure and of a beat, in whole notes.
    fn measure(&self) -> (f32, f32) {
        let (beats, unit) = self.meter;
        (f32::from(beats) / f32::from(unit), 1.0 / f32::from(unit))
    }

    /// `measure.beat` of a position, counted from 1.
    fn bar_beat(&self, position: f32) -> String {
        let (measure, beat) = self.measure();
        let bar = (position / measure + 1e-4).floor();
        let beat_in_bar = ((position - bar * measure) / beat + 1e-4).floor();
        format!("{}.{}", bar as u32 + 1, beat_in_bar as u32 + 1)
    }

    /// The selected events, a single one or a range.
    fn selection(&self) -> Option<Range<usize>> {
        let selected = self.selected.filter(|index| *index < self.parsed.events.len())?;
        let anchor = self.anchor.unwrap_or(selected).min(self.parsed.events.len() - 1);
        Some(selected.min(anchor)..selected.max(anchor) + 1)
    }

    fn is_selected(&self, index: usize) -> bool {
        self.selection().is_some_and(|range| range.contains(&index))
    }

    pub(super) fn select_all(&mut self) {
        if !self.parsed.events.is_empty() {
            self.anchor = Some(0);
            self.selected = Some(self.parsed.events.len() - 1);
        }
    }

    /// Changes the events and writes the text anew, as one undo step.
    fn edit(&mut self, change: impl FnOnce(&mut Vec<Event>)) {
        let mut events = self.parsed.events.clone();
        change(&mut events);
        self.set_events(events, true);
    }

    fn set_events(&mut self, events: Vec<Event>, undo_step: bool) {
        let text = ansi_music::generate(&events);
        if text == self.text {
            return;
        }
        if undo_step {
            self.undo_stack.push(self.text.clone());
            self.redo_stack.clear();
        }
        self.text = text;
        self.typing = false;
        self.stop();
        self.reparse();
    }

    /// Where new events go: after the selection.
    fn insert_index(&self) -> usize {
        self.selection().map_or(self.parsed.events.len(), |range| range.end)
    }

    pub(super) fn insert(&mut self, event: Event) {
        self.insert_at(self.insert_index(), event);
    }

    fn insert_at(&mut self, index: usize, event: Event) {
        let index = index.min(self.parsed.events.len());
        self.edit(|events| events.insert(index, event));
        self.select(Some(index), false);
        if let Event::Note { pitch: Some(pitch), .. } = event {
            self.preview(pitch);
        }
    }

    pub(super) fn delete_selected(&mut self) {
        if let Some(range) = self.selection() {
            let start = range.start;
            self.edit(|events| {
                events.drain(range);
            });
            let index = start.checked_sub(1).or((!self.parsed.events.is_empty()).then_some(0));
            self.select(index, false);
        }
    }

    fn set_pitch(&mut self, index: usize, pitch: u8, undo_step: bool) {
        let mut events = self.parsed.events.clone();
        if let Some(Event::Note { pitch: current, .. }) = events.get_mut(index) {
            *current = Some(pitch);
            self.set_events(events, undo_step);
            self.preview(pitch);
        }
    }

    /// Changes every selected note.
    fn edit_selected_notes(&mut self, mut change: impl FnMut(&mut Option<u8>, &mut u8, &mut bool)) {
        if let Some(range) = self.selection() {
            self.edit(|events| {
                for event in &mut events[range] {
                    if let Event::Note { pitch, length, dotted } = event {
                        change(pitch, length, dotted);
                    }
                }
            });
        }
    }

    fn set_length(&mut self, length: u8) {
        self.length = length;
        self.edit_selected_notes(|_, current, _| *current = length);
    }

    fn set_dotted(&mut self, dotted: bool) {
        self.dotted = dotted;
        self.edit_selected_notes(|_, _, current| *current = dotted);
    }

    /// Moves the selected notes by `steps` semitones, as far as the lowest and highest note allow.
    pub(super) fn transpose(&mut self, steps: i32) {
        let Some(range) = self.selection() else { return };
        let pitches: Vec<i32> = self.parsed.events[range]
            .iter()
            .filter_map(|event| match event {
                Event::Note { pitch: Some(pitch), .. } => Some(i32::from(*pitch)),
                _ => None,
            })
            .collect();
        let (Some(low), Some(high)) = (pitches.iter().min(), pitches.iter().max()) else {
            return;
        };
        let steps = steps.clamp(-low, i32::from(HIGHEST_PITCH) - high);
        if steps == 0 {
            return;
        }
        self.edit_selected_notes(|pitch, _, _| *pitch = pitch.map(|pitch| (i32::from(pitch) + steps) as u8));
        if let Some(Event::Note { pitch: Some(pitch), .. }) = self.selected.and_then(|index| self.parsed.events.get(index)).copied() {
            self.preview(pitch);
        }
    }

    /// The selected events as a music string.
    fn selection_text(&self) -> Option<String> {
        self.selection().map(|range| ansi_music::generate(&self.parsed.events[range]))
    }

    /// Copies the selection to the clipboard; `cut` also removes it.
    pub(super) fn copy(&mut self, context: &egui::Context, cut: bool) {
        if let Some(text) = self.selection_text() {
            context.copy_text(text);
            if cut {
                self.delete_selected();
            }
        }
    }

    /// Inserts music text after the selection and selects it. Text with characters terminals
    /// reject is not music and is left alone.
    pub(super) fn paste(&mut self, text: &str) -> bool {
        let parsed = ansi_music::parse(text);
        if !parsed.problems.is_empty() || parsed.events.is_empty() {
            return false;
        }
        let index = self.insert_index();
        let count = parsed.events.len();
        self.edit(|events| {
            events.splice(index..index, parsed.events);
        });
        self.anchor = Some(index);
        self.selected = Some(index + count - 1);
        true
    }

    /// The note length closest to `whole_notes`, dotted or not.
    fn nearest_length(whole_notes: f32) -> (u8, bool) {
        LENGTHS
            .iter()
            .flat_map(|&length| [(length, false), (length, true)])
            .min_by(|a, b| {
                let value = |(length, dotted): (u8, bool)| (if dotted { 1.5 } else { 1.0 }) / f32::from(length);
                (value(*a) - whole_notes).abs().total_cmp(&(value(*b) - whole_notes).abs())
            })
            .unwrap_or((ansi_music::DEFAULT_LENGTH, false))
    }

    /// The settings in effect at the selection: tempo and articulation.
    fn settings_at_selection(&self) -> (u8, Articulation) {
        let end = self.selected.map_or(0, |index| index + 1);
        let mut settings = (ansi_music::DEFAULT_TEMPO, Articulation::Normal);
        for event in &self.parsed.events[..end.min(self.parsed.events.len())] {
            match *event {
                Event::Tempo(tempo) => settings.0 = tempo,
                Event::Articulation(articulation) => settings.1 = articulation,
                _ => {}
            }
        }
        settings
    }

    /// Changes a setting from the selected note on: the change right before it, or a new one.
    fn set_setting(&mut self, setting: Event) {
        let same_kind = |event: &Event| std::mem::discriminant(event) == std::mem::discriminant(&setting);
        let index = self.selection().map_or(0, |range| range.start);
        // Changes before the selected note apply to it, so look back over them.
        let mut existing = None;
        let mut before = index;
        while before > 0 && !matches!(self.parsed.events[before - 1], Event::Note { .. }) {
            before -= 1;
            if same_kind(&self.parsed.events[before]) {
                existing = Some(before);
                break;
            }
        }
        let (selected, anchor) = (self.selected, self.anchor);
        self.edit(|events| match existing {
            Some(existing) => events[existing] = setting,
            None => events.insert(index, setting),
        });
        // A new change goes before the selection, which moves along.
        let shift = |position: Option<usize>| position.map(|position| position + usize::from(existing.is_none()));
        self.selected = shift(selected);
        self.anchor = shift(anchor);
    }

    fn preview(&mut self, pitch: u8) {
        let (tempo, _) = self.settings_at_selection();
        let events = [Event::Tempo(tempo), Event::note(pitch, self.length.max(8), false)];
        self.playback = None;
        self.player.play(ansi_music::to_ansi_music(&events, 0));
    }

    pub(super) fn playing(&self) -> bool {
        self.playback.is_some()
    }

    /// Plays the range set on the ruler, the selected notes, or from the cursor to the end.
    pub(super) fn play(&mut self, now: f64) {
        if !self.parsed.events.is_empty() {
            let range = self.playback_range();
            self.play_range(now, range);
        }
    }

    fn play_range(&mut self, now: f64, range: Range<usize>) {
        let events = &self.parsed.events[..range.end];
        if !events[range.start..].iter().any(|event| matches!(event, Event::Note { .. })) {
            return;
        }
        let timing = ansi_music::timing(&self.parsed.events);
        self.player.play(ansi_music::to_ansi_music(events, range.start));
        self.playback = Some(Playback { started: now, range, timing });
    }

    pub(super) fn stop(&mut self) {
        if self.playback.take().is_some() {
            self.player.stop();
        }
    }

    /// The event playing at `time` and how far into it, from 0 to 1.
    fn playing_event(timing: &[(f64, f64)], time: f64) -> Option<(usize, f64)> {
        timing
            .iter()
            .position(|(start, duration)| *duration > 0.0 && time >= *start && time < start + duration)
            .map(|index| (index, (time - timing[index].0) / timing[index].1))
    }

    /// Where each event starts on the roll, in whole notes, and where the music ends.
    fn positions(&self) -> (Vec<f32>, f32) {
        let mut position = 0.0;
        let starts = self
            .parsed
            .events
            .iter()
            .map(|event| {
                let start = position;
                position += event.whole_notes();
                start
            })
            .collect();
        (starts, position)
    }

    pub fn show(&mut self, ui: &mut egui::Ui, blocked: bool) {
        let context = &ui.ctx().clone();
        let now = context.input(|input| input.time);
        if let Some(playback) = &self.playback {
            if playback.position(now) < playback.end() {
                context.request_repaint_after(std::time::Duration::from_millis(16));
            } else if self.looping {
                let range = playback.range.clone();
                self.play_range(now, range);
                context.request_repaint();
            } else {
                self.playback = None;
            }
        }
        if !blocked && !context.egui_wants_keyboard_input() {
            self.keyboard(context, now);
        }
        egui::Panel::top("music-toolbar").exact_size(44.0).show(ui, |ui| {
            if blocked {
                ui.disable();
            }
            ui.horizontal_centered(|ui| self.toolbar(ui));
        });
        // Bottom panels stack upwards: the music string at the bottom, the player above it.
        egui::Panel::bottom("music-text")
            .resizable(true)
            .default_size(150.0)
            .min_size(90.0)
            .show(ui, |ui| {
                if blocked {
                    ui.disable();
                }
                self.text_view(ui);
            });
        egui::Panel::bottom("music-player")
            .exact_size(PLAYER_HEIGHT)
            .show_separator_line(false)
            .show(ui, |ui| {
                if blocked {
                    ui.disable();
                }
                self.player_bar(ui, now);
            });
        egui::CentralPanel::default_margins().show(ui, |ui| {
            if blocked {
                ui.disable();
            }
            self.roll(ui, now);
        });
    }

    pub(super) fn keyboard(&mut self, context: &egui::Context, now: f64) {
        let consume = |key: Key, modifiers: egui::Modifiers| context.input_mut(|input| input.consume_key(modifiers, key));
        let none = egui::Modifiers::NONE;
        let shift = egui::Modifiers::SHIFT;
        for (key, semitone) in NOTE_KEYS {
            if consume(key, none) {
                let pitch = (self.octave * 12 + semitone).min(HIGHEST_PITCH);
                self.insert(Event::note(pitch, self.length, self.dotted));
            }
        }
        if consume(Key::R, none) {
            self.insert(Event::rest(self.length, self.dotted));
        }
        for (index, key) in LENGTH_KEYS.into_iter().enumerate() {
            if consume(key, none) {
                self.set_length(LENGTHS[index]);
            }
        }
        if consume(Key::Period, none) {
            self.set_dotted(!self.dotted);
        }
        if consume(Key::Delete, none) || consume(Key::Backspace, none) {
            self.delete_selected();
        }
        if consume(Key::Space, none) {
            if self.playing() {
                self.stop();
            } else {
                self.play(now);
            }
        }
        if consume(Key::L, none) {
            self.looping = !self.looping;
        }
        if consume(Key::Escape, none) {
            self.stop();
            self.select(None, false);
            self.play_range = None;
        }
        if consume(Key::Home, none) {
            self.cursor = 0.0;
            self.play_range = None;
        }
        if consume(Key::A, egui::Modifiers::COMMAND) {
            self.select_all();
        }
        let count = self.parsed.events.len();
        for (key, step) in [(Key::ArrowLeft, -1i32), (Key::ArrowRight, 1)] {
            let extend = if consume(key, shift) {
                true
            } else if consume(key, none) {
                false
            } else {
                continue;
            };
            if count > 0 {
                let next = match self.selected {
                    Some(index) => (index as i32 + step).clamp(0, count as i32 - 1) as usize,
                    None if step < 0 => count - 1,
                    None => 0,
                };
                self.select(Some(next), extend);
            }
        }
        if consume(Key::PageUp, none) {
            self.octave = (self.octave + 1).min(OCTAVES - 1);
        }
        if consume(Key::PageDown, none) {
            self.octave = self.octave.saturating_sub(1);
        }
        for (key, step) in [(Key::ArrowUp, 1i32), (Key::ArrowDown, -1)] {
            if consume(key, shift) {
                self.transpose(step * 12);
            } else if consume(key, none) {
                self.transpose(step);
            }
        }
        let events = context.input(|input| input.events.clone());
        for event in events {
            match event {
                egui::Event::Copy => self.copy(context, false),
                egui::Event::Cut => self.copy(context, true),
                egui::Event::Paste(text) => {
                    self.paste(&text);
                }
                _ => {}
            }
        }
    }

    /// The note and setting controls; playback is in the player under the roll.
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.label(fl!("music-length"));
        let mut length = self.length;
        egui::ComboBox::from_id_salt("music-length")
            .selected_text(length_name(length))
            .width(64.0)
            .show_ui(ui, |ui| {
                for value in LENGTHS {
                    ui.selectable_value(&mut length, value, length_name(value));
                }
            });
        if length != self.length {
            self.set_length(length);
        }
        let mut dotted = self.dotted;
        if ui.checkbox(&mut dotted, fl!("music-dotted")).changed() {
            self.set_dotted(dotted);
        }
        if ui.button(fl!("music-rest")).on_hover_text(fl!("music-rest-hint")).clicked() {
            self.insert(Event::rest(self.length, self.dotted));
        }
        let can_delete = self.has_selection();
        if ui
            .add_enabled_ui(can_delete, |ui| self.icons.button(ui, "delete", &fl!("music-delete"), false))
            .inner
            .clicked()
        {
            self.delete_selected();
        }
        ui.separator();
        let (mut tempo, mut articulation) = self.settings_at_selection();
        ui.label(fl!("music-tempo"));
        if ui.add(egui::DragValue::new(&mut tempo).range(32..=255)).changed() {
            self.set_setting(Event::Tempo(tempo));
        }
        let before = articulation;
        egui::ComboBox::from_id_salt("music-articulation")
            .selected_text(articulation_name(articulation))
            .show_ui(ui, |ui| {
                for value in Articulation::ALL {
                    ui.selectable_value(&mut articulation, value, articulation_name(value));
                }
            });
        if articulation != before {
            self.set_setting(Event::Articulation(articulation));
        }
        ui.label(fl!("music-meter"));
        egui::ComboBox::from_id_salt("music-meter")
            .selected_text(format!("{}/{}", self.meter.0, self.meter.1))
            .width(56.0)
            .show_ui(ui, |ui| {
                for meter in METERS {
                    ui.selectable_value(&mut self.meter, meter, format!("{}/{}", meter.0, meter.1));
                }
            })
            .response
            .on_hover_text(fl!("music-meter-hint"));
        ui.separator();
        ui.label(fl!("music-keyboard-octave"));
        ui.add(egui::DragValue::new(&mut self.octave).range(0..=OCTAVES - 1).prefix("O"))
            .on_hover_text(fl!("music-keyboard-hint"));
        let introducer = |ui: &mut egui::Ui, current: &mut Introducer| {
            egui::ComboBox::from_id_salt("music-introducer")
                .selected_text(current.label())
                .show_ui(ui, |ui| {
                    for value in Introducer::ALL {
                        ui.selectable_value(current, value, value.label());
                    }
                })
                .response
                .on_hover_text(fl!("music-introducer-hint"));
        };
        // Right-aligned with its caption when there is room, as when the assistant panel is closed.
        if ui.available_width() >= 220.0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                introducer(ui, &mut self.introducer);
                ui.label(fl!("music-introducer"));
            });
        } else {
            ui.separator();
            introducer(ui, &mut self.introducer);
        }
    }

    /// The player under the roll, like the other editors': where playback is, its buttons
    /// in the middle and the range it plays on the right.
    fn player_bar(&mut self, ui: &mut egui::Ui, now: f64) {
        let full = ui.available_rect_before_wrap().shrink2(egui::vec2(8.0, 6.0));
        let visuals = ui.visuals().clone();
        ui.painter().rect(
            full,
            8,
            visuals.faint_bg_color,
            visuals.widgets.noninteractive.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let inner = full.shrink2(egui::vec2(12.0, 0.0));
        let timing = ansi_music::timing(&self.parsed.events);
        let total = timing.last().map_or(0.0, |(start, duration)| start + duration);
        let (starts, end) = self.positions();
        // The position shown: the playhead while playing, else where playback would start.
        let (millis, position) = match &self.playback {
            Some(playback) => {
                let millis = playback.position(now).min(playback.end());
                let position = Self::playing_event(&playback.timing, millis).map_or(end, |(index, fraction)| {
                    starts[index] + self.parsed.events[index].whole_notes() * fraction as f32
                });
                (millis, position)
            }
            None => {
                let index = self.playback_range().start;
                (timing.get(index).map_or(total, |(start, _)| *start), starts.get(index).copied().unwrap_or(end))
            }
        };
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.monospace(format!("{} / {}", clock(millis), clock(total)));
                ui.label(egui::RichText::new(fl!("music-bar", position = self.bar_beat(position))).weak());
            },
        );
        let playing = self.playing();
        let has_notes = self.parsed.events.iter().any(|event| matches!(event, Event::Note { .. }));
        let group = 3.0 * widgets::TRANSPORT_BUTTON + widgets::PLAY_BUTTON + 3.0 * 4.0 + 20.0;
        let buttons = egui::Rect::from_center_size(full.center(), egui::vec2(group, full.height()));
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(buttons)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if widgets::transport_button(
                    &mut self.icons,
                    ui,
                    "first_page",
                    &fl!("music-to-start"),
                    self.cursor > 0.0 || self.play_range.is_some(),
                    false,
                )
                .clicked()
                {
                    self.cursor = 0.0;
                    self.play_range = None;
                }
                ui.add_space(6.0);
                let label = if playing {
                    fl!("music-stop")
                } else if self.play_range.is_some() {
                    fl!("music-play-range")
                } else if self.selection().is_some_and(|range| range.len() > 1) {
                    fl!("music-play-selection")
                } else {
                    fl!("music-play")
                };
                if widgets::play_button(&mut self.icons, ui, playing, has_notes || playing, &label).clicked() {
                    if playing {
                        self.stop();
                    } else {
                        self.play(now);
                    }
                }
                ui.add_space(6.0);
                if widgets::transport_button(&mut self.icons, ui, "stop", &fl!("music-stop"), playing, false).clicked() {
                    self.stop();
                }
                ui.add_space(4.0);
                if widgets::transport_button(&mut self.icons, ui, "repeat", &fl!("music-loop"), true, self.looping).clicked() {
                    self.looping = !self.looping;
                }
            },
        );
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::right_to_left(egui::Align::Center)),
            |ui| match self.play_range {
                Some((from, to)) => {
                    if self.icons.button_sized(ui, "deselect", &fl!("music-range-clear"), false, 28.0).clicked() {
                        self.play_range = None;
                    }
                    ui.label(fl!("music-range", from = self.bar_beat(from), to = self.bar_beat(to)));
                }
                None => {
                    let selection = self.selection().is_some_and(|range| range.len() > 1);
                    if ui
                        .add_enabled(selection, egui::Button::new(fl!("music-range-from-selection")))
                        .on_hover_text(fl!("music-range-hint"))
                        .clicked()
                    {
                        self.range_from_selection();
                    }
                }
            },
        );
        ui.advance_cursor_after_rect(full);
    }

    fn text_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.strong(fl!("music-text"));
            let notes = self
                .parsed
                .events
                .iter()
                .filter(|event| matches!(event, Event::Note { pitch: Some(_), .. }))
                .count();
            let millis = ansi_music::timing(&self.parsed.events).last().map_or(0.0, |(start, duration)| start + duration);
            ui.weak(fl!("music-summary", notes = notes, seconds = format!("{:.1}", millis / 1000.0)));
        });
        if let Some(problem) = self.parsed.problems.first() {
            ui.colored_label(ui.visuals().error_fg_color, problem_message(&self.text, problem));
        }
        let selected = self.selection();
        let mut layouter = move |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
            let text = text.as_str();
            let parsed = ansi_music::parse(text);
            let font = egui::TextStyle::Monospace.resolve(ui.style());
            let plain = egui::TextFormat::simple(font.clone(), ui.visuals().text_color());
            let mut marked = plain.clone();
            marked.background = PRIMARY.gamma_multiply(0.45);
            let mut wrong = plain.clone();
            wrong.color = ui.visuals().error_fg_color;
            wrong.underline = Stroke::new(1.0_f32, ui.visuals().error_fg_color);
            let span = selected
                .as_ref()
                .and_then(|range| Some(parsed.spans.get(range.start)?.start..parsed.spans.get(range.end - 1)?.end));
            let kind = |index: usize| {
                if parsed.problems.iter().any(|problem| problem.position == index) {
                    2
                } else {
                    usize::from(span.as_ref().is_some_and(|span| span.contains(&index)))
                }
            };
            let formats = [&plain, &marked, &wrong];
            let mut job = egui::text::LayoutJob::default();
            let mut run: Option<(usize, usize)> = None;
            for (index, _) in text.char_indices() {
                let current = kind(index);
                match run {
                    Some((_, run_kind)) if run_kind == current => {}
                    Some((start, run_kind)) => {
                        job.append(&text[start..index], 0.0, formats[run_kind].clone());
                        run = Some((index, current));
                    }
                    None => run = Some((index, current)),
                }
            }
            if let Some((start, run_kind)) = run {
                job.append(&text[start..], 0.0, formats[run_kind].clone());
            }
            job.wrap.max_width = wrap_width;
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        };
        let previous = self.text.clone();
        let output = egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::TextEdit::multiline(&mut self.text)
                    .id(egui::Id::new("music-text-editor"))
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(4)
                    .hint_text(fl!("music-text-hint"))
                    .layouter(&mut layouter)
                    .show(ui)
            })
            .inner;
        if output.response.changed() {
            if !self.typing {
                self.undo_stack.push(previous);
                self.redo_stack.clear();
                self.typing = true;
            }
            self.stop();
            self.reparse();
        }
        if output.response.lost_focus() {
            self.typing = false;
        }
        // The note at the text cursor is the selected one.
        if output.response.has_focus() {
            if let Some(range) = output.cursor_range {
                let byte = self.text.char_indices().nth(range.primary.index.0).map_or(self.text.len(), |(byte, _)| byte);
                if let Some(index) = self.parsed.spans.iter().position(|span| span.start <= byte && byte <= span.end) {
                    if self.selected != Some(index) {
                        self.select(Some(index), false);
                    }
                }
            }
        }
    }

    fn roll(&mut self, ui: &mut egui::Ui, now: f64) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        let keyboard = egui::Rect::from_min_max(
            rect.min + egui::vec2(0.0, RULER_HEIGHT + LANE_HEIGHT),
            egui::pos2(rect.left() + KEYBOARD_WIDTH, rect.bottom()),
        );
        let ruler = egui::Rect::from_min_max(egui::pos2(keyboard.right(), rect.top()), egui::pos2(rect.right(), rect.top() + RULER_HEIGHT));
        let lane = egui::Rect::from_min_max(egui::pos2(keyboard.right(), ruler.bottom()), egui::pos2(rect.right(), keyboard.top()));
        let rest_lane = egui::Rect::from_min_max(egui::pos2(lane.left(), lane.top() + MARKER_HEIGHT), lane.max);
        let grid = egui::Rect::from_min_max(egui::pos2(keyboard.right(), keyboard.top()), rect.max);
        let rows = f32::from(HIGHEST_PITCH + 1) * ROW_HEIGHT;
        let max_scroll = (rows - grid.height()).max(0.0);
        if !self.scrolled_to_notes && grid.height() > 0.0 {
            // Start with the octaves around the first note, or around O4, in view.
            let pitch = self
                .parsed
                .events
                .iter()
                .find_map(|event| match event {
                    Event::Note { pitch: Some(pitch), .. } => Some(*pitch),
                    _ => None,
                })
                .unwrap_or(ansi_music::DEFAULT_OCTAVE * 12 + 6);
            self.scroll.y = f32::from(HIGHEST_PITCH - pitch) * ROW_HEIGHT - grid.height() / 2.0;
            self.scrolled_to_notes = true;
        }
        if response.hovered() {
            let delta = super::widgets::wheel_delta(ui);
            let modifiers = ui.input(|input| input.modifiers);
            if modifiers.command || modifiers.ctrl {
                if delta.y != 0.0 {
                    self.zoom = (self.zoom * (1.0 + delta.y * 0.002)).clamp(0.25, 8.0);
                }
            } else if modifiers.shift {
                self.scroll.x -= (delta.y + delta.x) / (WHOLE_NOTE * self.zoom);
            } else {
                self.scroll.x -= delta.x / (WHOLE_NOTE * self.zoom);
                self.scroll.y -= delta.y;
            }
        }
        let (starts, end) = self.positions();
        let pixels = WHOLE_NOTE * self.zoom;
        // Follow the playhead.
        let playhead = self.playback.as_ref().and_then(|playback| {
            let (index, fraction) = Self::playing_event(&playback.timing, playback.position(now))?;
            Some(starts[index] + self.parsed.events[index].whole_notes() * fraction as f32)
        });
        if let Some(position) = playhead {
            let visible = grid.width() / pixels;
            if position < self.scroll.x || position > self.scroll.x + visible * 0.9 {
                self.scroll.x = position - visible * 0.1;
            }
        }
        self.scroll.x = self.scroll.x.clamp(0.0, (end - grid.width() / pixels * 0.5).max(0.0));
        self.scroll.y = self.scroll.y.clamp(0.0, max_scroll);
        let scroll = self.scroll;
        let x_of = move |position: f32| grid.left() + (position - scroll.x) * pixels;
        let position_of = move |x: f32| (x - grid.left()) / pixels + scroll.x;
        let y_of = move |pitch: u8| grid.top() + f32::from(HIGHEST_PITCH - pitch) * ROW_HEIGHT - scroll.y;
        let pitch_of = move |y: f32| {
            let row = ((y - grid.top() + scroll.y) / ROW_HEIGHT).floor() as i32;
            (i32::from(HIGHEST_PITCH) - row).clamp(0, i32::from(HIGHEST_PITCH)) as u8
        };
        let event_rect = move |starts: &[f32], index: usize, event: &Event| -> Option<egui::Rect> {
            let width = (event.whole_notes() * pixels).max(3.0);
            match event {
                Event::Note { pitch: Some(pitch), .. } => Some(egui::Rect::from_min_size(
                    egui::pos2(x_of(starts[index]), y_of(*pitch) + 1.0),
                    egui::vec2(width - 1.0, ROW_HEIGHT - 2.0),
                )),
                Event::Note { pitch: None, .. } => Some(egui::Rect::from_min_size(
                    egui::pos2(x_of(starts[index]), rest_lane.top() + 3.0),
                    egui::vec2(width - 1.0, rest_lane.height() - 7.0),
                )),
                _ => None,
            }
        };
        // Setting changes sit at the start of the next note, side by side when there are several.
        let marker_layout = move |events: &[Event], starts: &[f32]| {
            let mut markers: Vec<(usize, egui::Rect, String)> = Vec::new();
            let mut last_marker: Option<(f32, f32)> = None;
            for (index, event) in events.iter().enumerate() {
                let label = match event {
                    Event::Tempo(tempo) => format!("T{tempo}"),
                    Event::Articulation(articulation) => articulation_code(*articulation).to_owned(),
                    Event::Background(true) => "MB".to_owned(),
                    Event::Background(false) => "MF".to_owned(),
                    Event::Note { .. } => continue,
                };
                let x = x_of(starts[index]);
                let left = match last_marker {
                    Some((at, right)) if at == starts[index] => right + 2.0,
                    _ => x,
                };
                let width = 8.0 + label.len() as f32 * 7.0;
                let marker = egui::Rect::from_min_size(egui::pos2(left, lane.top() + 2.0), egui::vec2(width, MARKER_HEIGHT - 4.0));
                last_marker = Some((starts[index], marker.right()));
                markers.push((index, marker, label));
            }
            markers
        };
        let markers = marker_layout(&self.parsed.events, &starts);
        let note_at = |starts: &[f32], events: &[Event], pos: egui::Pos2| {
            events
                .iter()
                .enumerate()
                .position(|(index, event)| event_rect(starts, index, event).is_some_and(|rect| rect.contains(pos)))
        };
        // The right end of a note's bar changes its length.
        let resize_at = |starts: &[f32], events: &[Event], pos: egui::Pos2| {
            events.iter().enumerate().position(|(index, event)| {
                matches!(event, Event::Note { pitch: Some(_), .. })
                    && event_rect(starts, index, event)
                        .is_some_and(|rect| rect.expand2(egui::vec2(RESIZE_EDGE, 0.0)).contains(pos) && (rect.right() - pos.x).abs() <= RESIZE_EDGE)
            })
        };

        // Editing with the pointer.
        let pressed = response.contains_pointer() && ui.input(|input| input.pointer.primary_pressed());
        let extend = ui.input(|input| input.modifiers.shift);
        if pressed {
            if let Some(pos) = ui.input(|input| input.pointer.interact_pos()) {
                ui.memory_mut(|memory| memory.surrender_focus(egui::Id::new("music-text-editor")));
                let events = self.parsed.events.clone();
                if ruler.contains(pos) {
                    self.drag = Some((
                        Drag::Ruler {
                            from: self.snap(position_of(pos.x)),
                            moved: false,
                        },
                        false,
                    ));
                } else if let Some(index) = resize_at(&starts, &events, pos).filter(|_| grid.contains(pos) && !extend) {
                    self.select(Some(index), false);
                    self.drag = Some((Drag::Length(index), false));
                } else if let Some(index) = note_at(&starts, &events, pos).filter(|_| grid.contains(pos) || rest_lane.contains(pos)) {
                    // Pressing a note of a selected range moves the whole range.
                    let in_range = self.selection().filter(|range| range.len() > 1 && range.contains(&index));
                    if in_range.is_none() || extend {
                        self.select(Some(index), extend);
                    }
                    if !extend {
                        let pitch = match events[index] {
                            Event::Note { pitch: Some(pitch), .. } => {
                                self.preview(pitch);
                                pitch
                            }
                            _ => pitch_of(pos.y),
                        };
                        self.drag = Some((
                            Drag::Notes {
                                block: in_range.unwrap_or(index..index + 1),
                                origin: events,
                                grab_pitch: pitch,
                                grab_x: pos.x,
                                heard: 0,
                            },
                            false,
                        ));
                    }
                } else if grid.contains(pos) {
                    let pitch = pitch_of(pos.y);
                    let position = position_of(pos.x);
                    // On a rest it becomes a note; elsewhere a note goes in at the nearer end of what is there.
                    let rest = events.iter().enumerate().position(|(index, event)| {
                        matches!(event, Event::Note { pitch: None, .. }) && position >= starts[index] && position < starts[index] + event.whole_notes()
                    });
                    let placed = if let Some(index) = rest {
                        self.select(Some(index), false);
                        self.set_pitch(index, pitch, true);
                        Some(index)
                    } else if let Some((index, _)) = insertion(&events, &starts, end, position) {
                        self.insert_at(index, Event::note(pitch, self.length, self.dotted));
                        Some(index)
                    } else {
                        None
                    };
                    if let Some(index) = placed {
                        self.drag = Some((
                            Drag::Notes {
                                block: index..index + 1,
                                origin: self.parsed.events.clone(),
                                grab_pitch: pitch,
                                grab_x: pos.x,
                                heard: 0,
                            },
                            true,
                        ));
                    }
                } else if keyboard.contains(pos) {
                    self.preview(pitch_of(pos.y));
                } else if let Some((index, _, _)) = markers.iter().find(|(_, marker, _)| marker.contains(pos)) {
                    self.select(Some(*index), extend);
                }
            }
        }
        let pointer = ui.input(|input| input.pointer.latest_pos());
        if let (Some((drag, has_undo)), Some(pos)) = (self.drag.clone(), pointer.filter(|_| ui.input(|input| input.pointer.primary_down()))) {
            match drag {
                Drag::Notes {
                    block,
                    origin,
                    grab_pitch,
                    grab_x,
                    heard,
                } => {
                    let mut events = origin.clone();
                    // Up or down: the pitches move together, as far as the range allows.
                    let pitches: Vec<i32> = events[block.clone()]
                        .iter()
                        .filter_map(|event| match event {
                            Event::Note { pitch: Some(pitch), .. } => Some(i32::from(*pitch)),
                            _ => None,
                        })
                        .collect();
                    let (low, high) = (pitches.iter().copied().min().unwrap_or(0), pitches.iter().copied().max().unwrap_or(0));
                    let delta = (i32::from(pitch_of(pos.y)) - i32::from(grab_pitch)).clamp(-low, i32::from(HIGHEST_PITCH) - high);
                    for event in &mut events[block.clone()] {
                        if let Event::Note { pitch: Some(pitch), .. } = event {
                            *pitch = (i32::from(*pitch) + delta) as u8;
                        }
                    }
                    // Sideways: the notes go before the first other note whose middle is past them.
                    let mut moved_to = block.clone();
                    if (pos.x - grab_x).abs() > MOVE_THRESHOLD {
                        let moved: Vec<Event> = events.drain(block.clone()).collect();
                        let origin_starts = {
                            let mut position = 0.0;
                            origin
                                .iter()
                                .map(|event| {
                                    let start = position;
                                    position += event.whole_notes();
                                    start
                                })
                                .collect::<Vec<f32>>()
                        };
                        let target_start = origin_starts[block.start] + (pos.x - grab_x) / pixels;
                        let mut position = 0.0;
                        let mut target = events.len();
                        for (index, event) in events.iter().enumerate() {
                            let length = event.whole_notes();
                            if length > 0.0 && position + length / 2.0 > target_start {
                                target = index;
                                break;
                            }
                            position += length;
                        }
                        events.splice(target..target, moved);
                        moved_to = target..target + block.len();
                    }
                    if events != self.parsed.events {
                        self.set_events(events, !has_undo);
                        if delta != heard {
                            if let Some(Event::Note { pitch: Some(pitch), .. }) = self.parsed.events.get(moved_to.start).copied() {
                                self.preview(pitch);
                            }
                        }
                        self.drag = Some((
                            Drag::Notes {
                                block,
                                origin,
                                grab_pitch,
                                grab_x,
                                heard: delta,
                            },
                            true,
                        ));
                    }
                    self.anchor = (moved_to.len() > 1).then_some(moved_to.start);
                    self.selected = Some(moved_to.end - 1);
                }
                Drag::Length(index) => {
                    let (starts, _) = self.positions();
                    let (length, dotted) = Self::nearest_length(position_of(pos.x) - starts.get(index).copied().unwrap_or(0.0));
                    let mut events = self.parsed.events.clone();
                    if let Some(Event::Note {
                        length: current,
                        dotted: current_dotted,
                        ..
                    }) = events.get_mut(index)
                    {
                        if (*current, *current_dotted) != (length, dotted) {
                            (*current, *current_dotted) = (length, dotted);
                            (self.length, self.dotted) = (length, dotted);
                            self.set_events(events, !has_undo);
                            self.drag = Some((drag, true));
                        }
                    }
                }
                Drag::Ruler { from, .. } => {
                    let to = self.snap(position_of(pos.x));
                    if to != from {
                        self.play_range = Some((from.min(to), from.max(to)));
                        self.drag = Some((Drag::Ruler { from, moved: true }, has_undo));
                    }
                }
            }
        }
        if self.drag.is_some() && !ui.input(|input| input.pointer.any_down()) {
            // A click on the ruler sets where playback starts.
            if let Some((Drag::Ruler { from, moved: false }, _)) = self.drag.take() {
                self.cursor = from;
                self.play_range = None;
            }
        }
        if response.secondary_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(index) = note_at(&starts, &self.parsed.events.clone(), pos) {
                    self.select(Some(index), false);
                    self.delete_selected();
                }
            }
        }

        // Drawing, of the events as edited.
        let (starts, end) = self.positions();
        let markers = marker_layout(&self.parsed.events, &starts);
        let painter = ui.painter_at(rect);
        let visuals = ui.visuals().clone();
        painter.rect_filled(rect, 0.0, visuals.extreme_bg_color);
        painter.rect_filled(ruler, 0.0, visuals.faint_bg_color);
        let grid_painter = painter.with_clip_rect(grid);
        for pitch in 0..=HIGHEST_PITCH {
            let y = y_of(pitch);
            if y > grid.bottom() || y + ROW_HEIGHT < grid.top() {
                continue;
            }
            let row = egui::Rect::from_min_size(egui::pos2(grid.left(), y), egui::vec2(grid.width(), ROW_HEIGHT));
            if ansi_music::is_black_key(pitch) {
                grid_painter.rect_filled(row, 0.0, visuals.faint_bg_color);
            }
            if pitch % 12 == 0 {
                grid_painter.line_segment(
                    [row.left_bottom(), row.right_bottom()],
                    Stroke::new(1.0_f32, visuals.weak_text_color().gamma_multiply(0.4)),
                );
            }
        }
        // The range playback plays, under the measure lines.
        if let Some((from, to)) = self.play_range {
            let band = egui::Rect::from_x_y_ranges(x_of(from)..=x_of(to), ruler.top()..=grid.bottom());
            painter
                .with_clip_rect(grid.union(lane))
                .rect_filled(band, 0.0, RANGE_COLOR.gamma_multiply(0.08));
            painter.with_clip_rect(ruler).rect_filled(
                egui::Rect::from_x_y_ranges(band.x_range(), ruler.y_range()),
                0.0,
                RANGE_COLOR.gamma_multiply(0.6),
            );
            for x in [band.left(), band.right()] {
                painter
                    .with_clip_rect(rect)
                    .line_segment([egui::pos2(x, ruler.top()), egui::pos2(x, grid.bottom())], Stroke::new(1.0_f32, RANGE_COLOR));
            }
        }
        // Measures with their numbers, beats and, zoomed in, sixteenths.
        let (measure, beat) = self.measure();
        let beats = usize::from(self.meter.0);
        let divisions = if beat / 4.0 * pixels >= 9.0 {
            4
        } else if beat * pixels >= 7.0 {
            1
        } else {
            0
        };
        let label_every = (36.0 / (measure * pixels)).ceil().max(1.0) as u32;
        let lines = painter.with_clip_rect(grid.union(lane).union(ruler));
        let mut bar = (scroll.x / measure).floor().max(0.0) as u32;
        while x_of(bar as f32 * measure) <= grid.right() {
            let start = bar as f32 * measure;
            let x = x_of(start);
            lines.line_segment(
                [egui::pos2(x, ruler.top()), egui::pos2(x, grid.bottom())],
                Stroke::new(1.0_f32, visuals.weak_text_color().gamma_multiply(0.55)),
            );
            if bar.is_multiple_of(label_every) {
                painter.with_clip_rect(ruler).text(
                    egui::pos2(x + 4.0, ruler.center().y),
                    egui::Align2::LEFT_CENTER,
                    (bar + 1).to_string(),
                    egui::FontId::proportional(11.0),
                    visuals.text_color(),
                );
            }
            for step in 1..beats * divisions.max(1) {
                if divisions == 0 {
                    break;
                }
                let on_beat = step.is_multiple_of(divisions);
                let x = x_of(start + step as f32 * beat / divisions as f32);
                let color = visuals.weak_text_color().gamma_multiply(if on_beat { 0.22 } else { 0.08 });
                lines.line_segment(
                    [
                        egui::pos2(x, if on_beat { ruler.bottom() - 5.0 } else { lane.top() }),
                        egui::pos2(x, grid.bottom()),
                    ],
                    Stroke::new(1.0_f32, color),
                );
            }
            bar += 1;
        }
        let end_x = x_of(end);
        if end_x >= grid.left() && end_x <= grid.right() {
            painter.with_clip_rect(grid.union(lane)).line_segment(
                [egui::pos2(end_x, lane.top()), egui::pos2(end_x, grid.bottom())],
                Stroke::new(1.0_f32, visuals.weak_text_color()),
            );
        }
        let separator = Stroke::new(1.0_f32, visuals.widgets.noninteractive.bg_stroke.color);
        painter.line_segment([ruler.left_bottom(), ruler.right_bottom()], separator);
        painter.line_segment([lane.left_bottom(), lane.right_bottom()], separator);
        let playing = playhead.and_then(|_| {
            let playback = self.playback.as_ref()?;
            Self::playing_event(&playback.timing, playback.position(now)).map(|(index, _)| index)
        });
        for (index, event) in self.parsed.events.iter().enumerate() {
            let Some(bar) = event_rect(&starts, index, event) else { continue };
            let selected = self.is_selected(index);
            let rest = matches!(event, Event::Note { pitch: None, .. });
            let base = if rest { visuals.weak_text_color().gamma_multiply(0.5) } else { NOTE_COLOR };
            let fill = if playing == Some(index) { Color32::WHITE } else { base };
            let clip = if rest { rest_lane } else { grid };
            painter.with_clip_rect(clip).rect_filled(bar, 2.0, fill);
            if selected {
                painter
                    .with_clip_rect(clip)
                    .rect_stroke(bar.expand(1.0), 3.0, Stroke::new(2.0_f32, PRIMARY), egui::StrokeKind::Outside);
            }
            if let Event::Note { dotted: true, .. } = event {
                painter
                    .with_clip_rect(clip)
                    .circle_filled(egui::pos2(bar.right() - 4.0, bar.center().y), 1.5, visuals.extreme_bg_color);
            }
        }
        let marker_painter = painter.with_clip_rect(lane);
        for (index, marker, label) in &markers {
            let selected = self.is_selected(*index);
            marker_painter.rect_filled(*marker, 3.0, if selected { PRIMARY } else { MARKER_COLOR.gamma_multiply(0.85) });
            marker_painter.text(
                marker.center(),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::monospace(10.0),
                Color32::BLACK,
            );
        }
        // Where playback starts: a mark on the ruler.
        if self.play_range.is_none() && playhead.is_none() {
            let x = x_of(self.cursor);
            if x >= ruler.left() && x <= ruler.right() {
                let tip = egui::pos2(x, ruler.bottom());
                painter.add(egui::Shape::convex_polygon(
                    vec![tip, tip + egui::vec2(-5.0, -8.0), tip + egui::vec2(5.0, -8.0)],
                    widgets::PLAY,
                    Stroke::NONE,
                ));
                painter
                    .with_clip_rect(grid.union(lane))
                    .line_segment([tip, egui::pos2(x, grid.bottom())], Stroke::new(1.0_f32, widgets::PLAY.gamma_multiply(0.5)));
            }
        }
        if let Some(position) = playhead {
            let x = x_of(position);
            painter.line_segment([egui::pos2(x, ruler.top()), egui::pos2(x, grid.bottom())], Stroke::new(2.0_f32, widgets::PLAY));
        }
        // Where a click would put a note, and the cursor over a note's end or the ruler.
        let hover = ui.input(|input| input.pointer.hover_pos());
        if let Some(pos) = hover.filter(|pos| grid.contains(*pos) && self.drag.is_none()) {
            let events = &self.parsed.events;
            if resize_at(&starts, events, pos).is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            } else if note_at(&starts, events, pos).is_none() {
                if let Some((_, at)) = insertion(events, &starts, end, position_of(pos.x)) {
                    let ghost = egui::Rect::from_min_size(
                        egui::pos2(x_of(at), y_of(pitch_of(pos.y)) + 1.0),
                        egui::vec2(Event::note(0, self.length, self.dotted).whole_notes() * pixels - 1.0, ROW_HEIGHT - 2.0),
                    );
                    grid_painter.rect_filled(ghost, 2.0, NOTE_COLOR.gamma_multiply(0.35));
                    grid_painter.line_segment(
                        [egui::pos2(ghost.left(), grid.top()), egui::pos2(ghost.left(), grid.bottom())],
                        Stroke::new(1.0_f32, NOTE_COLOR.gamma_multiply(0.5)),
                    );
                }
            }
        } else if matches!(self.drag, Some((Drag::Length(_), _))) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        } else if matches!(self.drag, Some((Drag::Notes { .. }, true))) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if hover.is_some_and(|pos| ruler.contains(pos)) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        // The keyboard.
        painter.rect_filled(egui::Rect::from_min_max(rect.min, keyboard.right_top()), 0.0, visuals.panel_fill);
        let keys = painter.with_clip_rect(keyboard);
        for pitch in 0..=HIGHEST_PITCH {
            let y = y_of(pitch);
            if y > keyboard.bottom() || y + ROW_HEIGHT < keyboard.top() {
                continue;
            }
            let key = egui::Rect::from_min_size(egui::pos2(keyboard.left(), y), egui::vec2(KEYBOARD_WIDTH, ROW_HEIGHT));
            let black = ansi_music::is_black_key(pitch);
            keys.rect_filled(
                key.shrink2(egui::vec2(0.0, 0.5)),
                0.0,
                if black { Color32::from_gray(40) } else { Color32::from_gray(225) },
            );
            if pitch % 12 == 0 {
                keys.text(
                    key.right_center() - egui::vec2(4.0, 0.0),
                    egui::Align2::RIGHT_CENTER,
                    ansi_music::note_name(pitch).replace(' ', ""),
                    egui::FontId::proportional(9.0),
                    Color32::from_gray(60),
                );
            }
        }
        // The octave the note keys play.
        let top = y_of(self.octave * 12 + 11);
        let octave = egui::Rect::from_min_max(egui::pos2(keyboard.left(), top), egui::pos2(keyboard.left() + 3.0, top + 12.0 * ROW_HEIGHT));
        keys.rect_filled(octave, 0.0, PRIMARY);
        // The lane captions beside the ruler, setting changes and rests.
        let caption = |rect: egui::Rect, text: String| {
            painter.text(
                egui::pos2(rect.left() + 4.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                text,
                egui::FontId::proportional(9.0),
                visuals.weak_text_color(),
            );
        };
        caption(
            egui::Rect::from_min_max(rect.min, egui::pos2(keyboard.right(), ruler.bottom())),
            fl!("music-ruler-caption"),
        );
        caption(
            egui::Rect::from_min_max(egui::pos2(rect.left(), lane.top()), egui::pos2(keyboard.right(), rest_lane.top())),
            fl!("music-settings-caption"),
        );
        caption(
            egui::Rect::from_min_max(egui::pos2(rect.left(), rest_lane.top()), egui::pos2(keyboard.right(), lane.bottom())),
            fl!("music-rests-caption"),
        );
        if self.parsed.events.is_empty() {
            painter.text(
                grid.center(),
                egui::Align2::CENTER_CENTER,
                fl!("music-empty-hint"),
                egui::FontId::proportional(14.0),
                visuals.weak_text_color(),
            );
        }
        if let Some(pos) = hover {
            if response.hovered() && grid.contains(pos) {
                response.on_hover_text_at_pointer(ansi_music::note_name(pitch_of(pos.y)));
            } else if response.hovered() && ruler.contains(pos) {
                response.on_hover_text_at_pointer(fl!("music-ruler-hint"));
            }
        }
    }
}

/// Where a click at `position` (in whole notes) inserts a note, and where that is: the nearer
/// end of the event there, or the end of the music. A rest is turned into a note instead.
fn insertion(events: &[Event], starts: &[f32], end: f32, position: f32) -> Option<(usize, f32)> {
    if position < 0.0 {
        return None;
    }
    if position >= end {
        return Some((events.len(), end));
    }
    let index = events
        .iter()
        .enumerate()
        .position(|(index, event)| event.whole_notes() > 0.0 && position >= starts[index] && position < starts[index] + event.whole_notes())?;
    if matches!(events[index], Event::Note { pitch: None, .. }) {
        return None;
    }
    let length = events[index].whole_notes();
    Some(if position - starts[index] < length / 2.0 {
        (index, starts[index])
    } else {
        (index + 1, starts[index] + length)
    })
}

/// `m:ss.s` of milliseconds.
fn clock(millis: f64) -> String {
    // Rounded to tenths first, so 59.96 s shows as 1:00.0 rather than 0:60.0.
    let tenths = (millis.max(0.0) / 100.0).round() as u64;
    format!("{}:{:02}.{}", tenths / 600, tenths % 600 / 10, tenths % 10)
}

fn length_name(length: u8) -> String {
    format!("1/{length}")
}

fn articulation_code(articulation: Articulation) -> &'static str {
    match articulation {
        Articulation::Normal => "MN",
        Articulation::Legato => "ML",
        Articulation::Staccato => "MS",
    }
}

fn articulation_name(articulation: Articulation) -> String {
    match articulation {
        Articulation::Normal => fl!("music-articulation-normal"),
        Articulation::Legato => fl!("music-articulation-legato"),
        Articulation::Staccato => fl!("music-articulation-staccato"),
    }
}

fn problem_message(text: &str, problem: &ansi_music::Problem) -> String {
    let column = text[..problem.position].chars().count() + 1;
    fl!("music-error-character", character = problem.character.to_string(), column = column)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Presses each key in a frame of its own.
    fn press(context: &egui::Context, editor: &mut MusicEditor, keys: &[Key]) {
        for key in keys {
            let event = egui::Event::Key {
                key: *key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            };
            let _ = context
                .run_ui(
                    egui::RawInput {
                        events: vec![event],
                        ..Default::default()
                    },
                    |ui| editor.keyboard(ui.ctx(), 0.0),
                )
                .textures_delta
                .clear();
        }
    }

    #[test]
    fn note_keys_enter_notes_after_the_selection() {
        let context = egui::Context::default();
        let mut editor = MusicEditor::new();
        press(&context, &mut editor, &[Key::Z, Key::Num4, Key::S, Key::Period, Key::X, Key::R]);
        assert_eq!(editor.text(), "L8CC#.D.P.");
        assert_eq!(editor.player.played.len(), 3, "each entered note sounds");
        editor.selected = Some(0);
        press(&context, &mut editor, &[Key::PageUp, Key::M]);
        assert_eq!(
            editor.events()[1],
            Event::note(5 * 12 + 11, 8, true),
            "the length and dot stay as last set; the note goes after the selection"
        );
        press(&context, &mut editor, &[Key::ArrowUp, Key::Delete]);
        assert_eq!(editor.events().len(), 4);
        editor.undo(false);
        editor.undo(false);
        assert_eq!(editor.events()[1], Event::note(5 * 12 + 11, 8, true));
        editor.undo(true);
        assert_eq!(editor.events()[1], Event::note(6 * 12, 8, true));
        assert!(editor.modified());
    }

    #[test]
    fn settings_change_from_the_selected_note_on() {
        let mut editor = MusicEditor::from_text("CDE".into(), Introducer::SyncTerm);
        editor.selected = Some(1);
        editor.set_setting(Event::Tempo(90));
        assert_eq!(editor.text(), "CT90DE");
        assert_eq!(editor.selected, Some(2), "the selection stays on the note");
        editor.set_setting(Event::Tempo(100));
        assert_eq!(editor.text(), "CT100DE", "a change right before the note is updated");
        editor.set_setting(Event::Articulation(Articulation::Legato));
        assert_eq!(editor.text(), "CT100MLDE");
        assert_eq!(editor.settings_at_selection(), (100, Articulation::Legato));
    }

    #[test]
    fn playback_starts_at_the_selection_and_ends_by_itself() {
        let mut editor = MusicEditor::from_text("T60 C P D".into(), Introducer::SyncTerm);
        editor.select(Some(2), false);
        editor.play(10.0);
        assert_eq!(editor.player.played.last(), Some(&ansi_music::to_ansi_music(editor.events(), 2)));
        let playback = editor.playback.as_ref().unwrap();
        assert_eq!(MusicEditor::playing_event(&playback.timing, playback.position(10.5)), Some((2, 0.5)));
        let context = egui::Context::default();
        let _ = context
            .run_ui(
                egui::RawInput {
                    time: Some(13.1),
                    ..Default::default()
                },
                |ui| editor.show(ui, false),
            )
            .textures_delta
            .clear();
        assert!(!editor.playing());
    }

    fn run(context: &egui::Context, editor: &mut MusicEditor, events: Vec<egui::Event>) -> egui::FullOutput {
        let mut output = context.run_ui(egui::RawInput { events, ..Default::default() }, |ui| editor.keyboard(ui.ctx(), 0.0));
        output.textures_delta.clear();
        output
    }

    fn shift_key(key: Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::SHIFT,
        }
    }

    #[test]
    fn selections_transpose_copy_cut_and_paste() {
        let context = egui::Context::default();
        let mut editor = MusicEditor::from_text("CDEFG".into(), Introducer::SyncTerm);
        editor.select(Some(1), false);
        run(&context, &mut editor, vec![shift_key(Key::ArrowRight)]);
        run(&context, &mut editor, vec![shift_key(Key::ArrowRight)]);
        assert_eq!(editor.selection(), Some(1..4));
        editor.transpose(2);
        assert_eq!(editor.text(), "CEF#GG");
        // Transposing stops at the highest note instead of breaking the melody's shape.
        editor.transpose(100);
        assert_eq!(editor.selection_text().as_deref(), Some("O6G#A#B"));
        editor.undo(false);
        press(&context, &mut editor, &[Key::Num5]);
        assert_eq!(editor.text(), "L16C4EF#GG4", "lengths change for the whole selection");

        let output = run(&context, &mut editor, vec![egui::Event::Copy]);
        assert!(output.platform_output.commands.contains(&egui::OutputCommand::CopyText("L16EF#G".into())));
        run(&context, &mut editor, vec![egui::Event::Cut]);
        assert_eq!(editor.text(), "CG");
        assert_eq!(editor.selected, Some(0));
        run(&context, &mut editor, vec![egui::Event::Paste("L16EF#G".into())]);
        assert_eq!(editor.text(), "L16C4EF#GG4");
        assert_eq!(editor.selection(), Some(1..4), "the pasted notes are selected");
        assert!(!editor.paste("hello"), "other text is not music");

        run(
            &context,
            &mut editor,
            vec![egui::Event::Key {
                key: Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
        );
        press(&context, &mut editor, &[Key::Delete]);
        assert_eq!(editor.text(), "");
    }

    #[test]
    fn dragged_lengths_snap_to_note_values() {
        assert_eq!(MusicEditor::nearest_length(0.25), (4, false));
        assert_eq!(MusicEditor::nearest_length(0.37), (4, true));
        assert_eq!(MusicEditor::nearest_length(0.19), (8, true));
        assert_eq!(MusicEditor::nearest_length(0.7), (2, true));
        assert_eq!(MusicEditor::nearest_length(5.0), (1, true));
        assert_eq!(MusicEditor::nearest_length(-1.0), (64, false));
    }

    #[test]
    fn clicks_insert_at_the_nearer_end_of_a_note() {
        let events = [Event::note(48, 4, false), Event::rest(4, false), Event::note(50, 4, false)];
        let starts = [0.0, 0.25, 0.5];
        assert_eq!(insertion(&events, &starts, 0.75, 0.1), Some((0, 0.0)));
        assert_eq!(insertion(&events, &starts, 0.75, 0.2), Some((1, 0.25)));
        assert_eq!(insertion(&events, &starts, 0.75, 0.3), None, "a click on a rest fills it");
        assert_eq!(insertion(&events, &starts, 0.75, 0.9), Some((3, 0.75)));
        assert_eq!(insertion(&events, &starts, 0.75, -0.1), None);
    }

    #[test]
    fn a_selection_plays_alone_and_loops() {
        let mut editor = MusicEditor::from_text("T60 C D E F".into(), Introducer::SyncTerm);
        editor.select(Some(2), false);
        editor.select(Some(3), true);
        editor.looping = true;
        editor.play(0.0);
        assert_eq!(editor.playback.as_ref().unwrap().range, 2..4);
        let played = editor.player.played.last().unwrap().music_actions.clone();
        assert_eq!(played, ansi_music::to_ansi_music(&editor.events()[..4], 2).music_actions);
        let context = egui::Context::default();
        let _ = context
            .run_ui(
                egui::RawInput {
                    time: Some(2.5),
                    ..Default::default()
                },
                |ui| editor.show(ui, false),
            )
            .textures_delta
            .clear();
        assert!(editor.playing(), "loop playback starts over");
        assert_eq!(editor.player.played.len(), 2);
        assert_eq!(editor.playback.as_ref().unwrap().started, 2.5);
    }

    #[test]
    fn playback_plays_the_range_the_selection_or_from_the_cursor() {
        let mut editor = MusicEditor::from_text("T60 C D E F G".into(), Introducer::SyncTerm);
        assert_eq!(editor.playback_range(), 0..6);
        editor.select(Some(3), false);
        assert_eq!(editor.cursor, 0.5, "picking a note moves where playback starts");
        assert_eq!(editor.playback_range(), 3..6);
        editor.select(Some(2), true);
        assert_eq!(editor.playback_range(), 2..4, "a selected range plays alone");
        editor.range_from_selection();
        assert_eq!(editor.play_range, Some((0.25, 0.75)));
        editor.select(Some(5), false);
        assert_eq!(editor.playback_range(), 2..4, "the range wins over the selection");
        assert_eq!(editor.snap(0.3), 0.25);
        assert_eq!(editor.snap(9.0), 1.25);
    }

    #[test]
    fn positions_show_as_bar_and_beat() {
        let mut editor = MusicEditor::new();
        assert_eq!(editor.bar_beat(0.0), "1.1");
        assert_eq!(editor.bar_beat(1.25), "2.2");
        editor.meter = (3, 4);
        assert_eq!(editor.bar_beat(0.75), "2.1");
        editor.meter = (6, 8);
        assert_eq!(editor.bar_beat(0.375), "1.4");
        assert_eq!(clock(83_440.0), "1:23.4");
        assert_eq!(clock(59_960.0), "1:00.0");
        assert_eq!(clock(0.0), "0:00.0");
    }

    #[test]
    fn recovery_restores_text_and_introducer() {
        let mut editor = MusicEditor::from_text("T90 CDE\nMS G".into(), Introducer::Banana);
        editor.apply_text("T90 CDE\nMS GA".into());
        let restored = MusicEditor::from_recovery(&editor.recovery_snapshot()).unwrap();
        assert_eq!((restored.text(), restored.introducer()), ("T90 CDE\nMS GA", Introducer::Banana));
        assert!(restored.modified(), "restored music needs saving");
    }

    #[test]
    fn saving_needs_text_terminals_accept() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("song.ams");
        let mut editor = MusicEditor::from_text("LS CDE".into(), Introducer::Conflicting);
        editor.save(&path, false).unwrap();
        assert!(!editor.modified());
        let loaded = MusicEditor::load(&path).unwrap();
        assert_eq!(loaded.events(), editor.events());
        assert_eq!(loaded.introducer, Introducer::Conflicting);

        editor.text.push_str(" X");
        editor.reparse();
        assert!(editor.save(&path, true).unwrap_err().contains('X'));
        std::fs::write(directory.path().join("art.ans"), b"\x1b[31mart").unwrap();
        assert!(MusicEditor::load(&directory.path().join("art.ans")).is_err());
    }
}
