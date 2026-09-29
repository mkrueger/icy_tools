//! Command-by-command playback shared by the RIP and IGS editors: the timing a terminal shows
//! a command stream with, the transport bar, and how the command list marks and follows the
//! animation frame.

use eframe::egui::{self, Color32};
use icy_draw::fl;
use icy_parser_core::BaudEmulation;

use super::widgets::{self, Icons};

/// The items of a command stream as a terminal receives them.
pub trait Timeline {
    fn len(&self) -> usize;

    /// The bytes a terminal receives for item `index`, which take time at a BPS rate.
    fn transmitted_bytes(&self, index: usize) -> usize;

    /// How many steps a terminal shows the item in, e.g. the iterations of a delayed loop.
    fn steps(&self, _index: usize) -> usize {
        1
    }

    /// The seconds a terminal waits after each step of the item.
    fn step_delay(&self, _index: usize) -> f64 {
        0.0
    }
}

/// A running animation: the canvas shows the items through `index`, the last of them through
/// its `step`-th of `steps` steps.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub index: usize,
    pub step: usize,
    pub steps: usize,
    pub next_at: f64,
}

/// A transport bar button.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Previous,
    Next,
    PlayPause,
    Stop,
    Seek(usize),
}

/// What an editor does after a transport action.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    /// The preview through the selection moves to this item instead of the animation.
    pub select: Option<usize>,
    /// The animation started at this item.
    pub started: Option<usize>,
    /// The paused animation stepped forward onto this item.
    pub stepped: Option<usize>,
    /// The running animation jumped to this item and continues after it.
    pub jumped: Option<usize>,
    /// The animation and the preview through the selection ended.
    pub stopped: bool,
    /// Sounds of the previous position end.
    pub silence: bool,
}

/// Playback state: running, paused at `playhead`, or stopped.
pub struct Transport {
    pub run: Option<Run>,
    pub playhead: Option<usize>,
    pub speed: BaudEmulation,
    /// The frame the selection last followed.
    followed: Option<usize>,
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            run: None,
            playhead: None,
            speed: BaudEmulation::Rate(1200),
            followed: None,
        }
    }
}

impl Transport {
    /// Whether the animation plays or is paused at a frame; editing waits until it stops.
    pub fn animating(&self) -> bool {
        self.run.is_some() || self.playhead.is_some()
    }

    pub fn playing(&self) -> bool {
        self.run.is_some()
    }

    /// The item the canvas shows the drawing through: the animation frame, or `preview`, the
    /// selection an editor previews through.
    pub fn frame(&self, preview: Option<usize>, len: usize) -> Option<usize> {
        self.run
            .as_ref()
            .map(|run| run.index)
            .or(self.playhead)
            .or(preview)
            .filter(|index| *index < len)
    }

    /// The steps of the frame drawn so far, while a multi-step item plays.
    pub fn steps_shown(&self) -> Option<usize> {
        self.run.as_ref().filter(|run| run.step < run.steps).map(|run| run.step)
    }

    pub fn transmission_seconds(&self, timeline: &dyn Timeline, index: usize) -> f64 {
        transmission_seconds(timeline, index, self.speed)
    }

    /// The wait after `step` of `steps` of item `index`; the last step also waits for the next
    /// item to be transmitted.
    fn step_seconds(&self, timeline: &dyn Timeline, index: usize, step: usize, steps: usize) -> f64 {
        let pause = timeline.step_delay(index);
        if step < steps || index + 1 >= timeline.len() {
            pause
        } else {
            pause + self.transmission_seconds(timeline, index + 1)
        }
    }

    /// Starts running at item `index`, which is entered now.
    pub fn play(&mut self, timeline: &dyn Timeline, index: usize, now: f64) {
        self.playhead = None;
        self.run = Some(self.enter(timeline, index, now));
    }

    fn enter(&self, timeline: &dyn Timeline, index: usize, now: f64) -> Run {
        let steps = timeline.steps(index).max(1);
        Run {
            index,
            step: 1,
            steps,
            next_at: now + self.step_seconds(timeline, index, 1, steps),
        }
    }

    /// Applies a transport action. `preview` is the selection the editor previews through,
    /// which steps and seeks move instead of an animation while none is shown.
    pub fn apply(&mut self, action: Action, timeline: &dyn Timeline, preview: Option<usize>, now: f64) -> Outcome {
        let len = timeline.len();
        let mut outcome = Outcome::default();
        let previewing = !self.animating() && preview.is_some();
        let Some(last) = len.checked_sub(1) else {
            if action == Action::Stop {
                self.stop();
                outcome.stopped = true;
            }
            return outcome;
        };
        match action {
            Action::PlayPause if self.playing() => {
                self.pause();
                outcome.silence = true;
            }
            Action::PlayPause => {
                let start = self.frame(preview, len).filter(|index| index + 1 < len).map_or(0, |index| index + 1);
                self.play(timeline, start, now);
                outcome.started = Some(start);
            }
            Action::Stop => {
                self.stop();
                outcome.stopped = true;
                outcome.silence = true;
            }
            Action::Previous | Action::Next => {
                let forward = action == Action::Next;
                let current = self.frame(preview, len);
                self.run = None;
                outcome.silence = true;
                let index = match (current, forward) {
                    (Some(index), true) => (index + 1).min(last),
                    (Some(index), false) => index.saturating_sub(1),
                    (None, _) => 0,
                };
                if previewing {
                    outcome.select = Some(index);
                } else {
                    if forward && current != Some(index) {
                        outcome.stepped = Some(index);
                    }
                    self.playhead = Some(index);
                }
            }
            Action::Seek(index) => {
                let index = index.min(last);
                if self.frame(preview, len) == Some(index) {
                    return outcome;
                }
                if previewing {
                    outcome.select = Some(index);
                    return outcome;
                }
                outcome.silence = true;
                if self.playing() {
                    outcome.jumped = Some(index);
                }
                self.seek(timeline, index, now);
            }
        }
        outcome
    }

    /// Ends the animation for a preview through its frame, which the editor selects.
    pub fn end_for_preview(&mut self, len: usize) -> Option<usize> {
        let frame = self.frame(None, len);
        self.stop();
        frame
    }

    /// Pauses at the running frame.
    pub fn pause(&mut self) {
        if let Some(run) = self.run.take() {
            self.playhead = Some(run.index);
        }
    }

    pub fn stop(&mut self) {
        self.run = None;
        self.playhead = None;
        self.followed = None;
    }

    /// Jumps to `index`: a running animation continues after it is shown complete, a paused
    /// one shows it.
    pub fn seek(&mut self, timeline: &dyn Timeline, index: usize, now: f64) {
        if self.run.is_some() {
            let steps = timeline.steps(index).max(1);
            self.run = Some(Run {
                index,
                step: steps,
                steps,
                next_at: now + self.step_seconds(timeline, index, steps, steps),
            });
        } else {
            self.playhead = Some(index);
        }
    }

    /// Advances to `now`, calling `entered` for every item reached; returns how long until the
    /// next change, or `None` once the animation ended at the last item.
    pub fn advance(&mut self, timeline: &dyn Timeline, now: f64, entered: &mut dyn FnMut(usize)) -> Option<f64> {
        let mut run = self.run.take()?;
        while now >= run.next_at {
            let due = run.next_at;
            if run.step < run.steps {
                run.step += 1;
                run.next_at = due + self.step_seconds(timeline, run.index, run.step, run.steps);
                continue;
            }
            if run.index + 1 >= timeline.len() {
                self.playhead = Some(run.index);
                return None;
            }
            run = self.enter(timeline, run.index + 1, due);
            entered(run.index);
        }
        let wait = (run.next_at - now).max(0.0);
        self.run = Some(run);
        Some(wait)
    }

    /// Keeps the running animation on schedule after the speed changed from `previous`.
    pub fn speed_changed(&mut self, timeline: &dyn Timeline, previous: BaudEmulation, now: f64) {
        let Some(run) = self.run.as_ref().filter(|run| run.step >= run.steps && run.index + 1 < timeline.len()) else {
            return;
        };
        let next = run.index + 1;
        let remaining = run.next_at - transmission_seconds(timeline, next, previous);
        let next_at = (remaining + self.transmission_seconds(timeline, next)).max(now);
        if let Some(run) = self.run.as_mut() {
            run.next_at = next_at;
        }
    }

    /// The frame the selection should move to because the animation moved, if it did.
    pub fn follow(&mut self, frame: Option<usize>) -> Option<usize> {
        if !self.animating() {
            self.followed = None;
            return None;
        }
        if frame == self.followed {
            return None;
        }
        self.followed = frame;
        frame
    }

    /// The transport bar: previous, play or pause, next, stop, a position slider, the item
    /// number and the BPS rate. `frame` is the item shown, `stoppable` whether Stop applies.
    pub fn ui(&mut self, ui: &mut egui::Ui, icons: &mut Icons, frame: Option<usize>, len: usize, timeline: &dyn Timeline) -> Option<Action> {
        let mut action = None;
        let playing = self.playing();
        ui.horizontal_centered(|ui| {
            ui.add_space(8.0);
            ui.label(icy_engine_gui::egui::appearance::bold(ui, fl!("playback-animation")));
            widgets::divider(ui);
            ui.add_enabled_ui(len > 0 && frame.is_some_and(|index| index > 0), |ui| {
                if icons.button_sized(ui, "skip_previous", &fl!("playback-previous"), false, 30.0).clicked() {
                    action = Some(Action::Previous);
                }
            });
            ui.add_enabled_ui(len > 0, |ui| {
                if icons
                    .button_sized(ui, if playing { "pause" } else { "play" }, &fl!("playback-play-pause"), playing, 34.0)
                    .clicked()
                {
                    action = Some(Action::PlayPause);
                }
            });
            ui.add_enabled_ui(len > 0 && frame.is_none_or(|index| index + 1 < len), |ui| {
                if icons.button_sized(ui, "skip_next", &fl!("playback-next"), false, 30.0).clicked() {
                    action = Some(Action::Next);
                }
            });
            ui.add_enabled_ui(frame.is_some(), |ui| {
                if ui.button(fl!("playback-stop")).clicked() {
                    action = Some(Action::Stop);
                }
            });
            if len > 0 {
                // Without a frame the whole drawing is shown, so the slider sits at the end.
                let mut position = frame.map_or(len, |index| index + 1);
                let slider = ui.scope(|ui| {
                    ui.spacing_mut().slider_width = 240.0;
                    ui.add(egui::Slider::new(&mut position, 1..=len).show_value(false))
                        .on_hover_text(fl!("playback-seek"))
                });
                if slider.inner.changed() {
                    action = Some(Action::Seek(position - 1));
                }
            }
            ui.label(fl!("playback-position", current = frame.map_or(0, |index| index + 1), total = len));
            widgets::divider(ui);
            ui.label(fl!("playback-speed"));
            let previous = self.speed;
            egui::ComboBox::from_id_salt("command-playback-speed")
                .selected_text(speed_name(self.speed))
                .show_ui(ui, |ui| {
                    for speed in BaudEmulation::OPTIONS {
                        ui.selectable_value(&mut self.speed, speed, speed_name(speed));
                    }
                });
            if previous != self.speed {
                self.speed_changed(timeline, previous, ui.input(|input| input.time));
            }
        });
        action
    }
}

fn transmission_seconds(timeline: &dyn Timeline, index: usize, speed: BaudEmulation) -> f64 {
    match speed {
        BaudEmulation::Off => 0.0,
        BaudEmulation::Rate(bps) => timeline.transmitted_bytes(index) as f64 / f64::from(bps),
    }
}

fn speed_name(speed: BaudEmulation) -> String {
    match speed {
        BaudEmulation::Off => fl!("playback-speed-max"),
        BaudEmulation::Rate(bps) => format!("{bps} BPS"),
    }
}

/// How a command list row relates to the animation frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RowMark {
    /// The item the canvas shows the drawing through.
    pub current: bool,
    /// After the current item, not drawn yet.
    pub pending: bool,
}

impl RowMark {
    pub fn new(index: usize, frame: Option<usize>) -> Self {
        Self {
            current: frame == Some(index),
            pending: frame.is_some_and(|frame| index > frame),
        }
    }

    /// Paints the background and bar of the current row, unless it is selected or hovered.
    pub fn paint_background(self, ui: &egui::Ui, rect: egui::Rect, highlighted: bool) {
        if !self.current {
            return;
        }
        let visuals = ui.visuals();
        let painter = ui.painter_at(rect);
        if !highlighted {
            painter.rect_filled(rect.shrink2(egui::vec2(2.0, 1.0)), 4, visuals.selection.bg_fill.gamma_multiply(0.35));
        }
        let bar = egui::Rect::from_min_size(rect.left_top() + egui::vec2(2.0, 3.0), egui::vec2(3.0, rect.height() - 6.0));
        painter.rect_filled(bar, 1.5, visuals.selection.stroke.color);
    }

    /// Dims text of items not drawn yet.
    pub fn text(self, color: Color32, selected: bool) -> Color32 {
        if self.pending && !selected {
            color.gamma_multiply(0.45)
        } else {
            color
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Items of `bytes` bytes, the second one a loop of three steps of 0.1 seconds.
    struct Items(Vec<usize>);

    impl Timeline for Items {
        fn len(&self) -> usize {
            self.0.len()
        }

        fn transmitted_bytes(&self, index: usize) -> usize {
            self.0[index]
        }

        fn steps(&self, index: usize) -> usize {
            if index == 1 {
                3
            } else {
                1
            }
        }

        fn step_delay(&self, index: usize) -> f64 {
            if index == 1 {
                0.1
            } else {
                0.0
            }
        }
    }

    #[test]
    fn runs_wait_for_transmission_and_steps() {
        let items = Items(vec![120, 60, 120]);
        let mut transport = Transport::default();
        transport.play(&items, 0, 0.0);
        // 60 bytes at 1200 BPS before the loop starts.
        assert_eq!(transport.run.as_ref().unwrap().next_at, 0.05);
        let mut entered = Vec::new();
        transport.advance(&items, 0.06, &mut |index| entered.push(index));
        assert_eq!((entered.clone(), transport.steps_shown()), (vec![1], Some(1)));
        transport.advance(&items, 0.16, &mut |index| entered.push(index));
        assert_eq!(transport.steps_shown(), Some(2));
        // The last step also waits for the third item: 0.1 + 0.1 seconds.
        transport.advance(&items, 0.26, &mut |index| entered.push(index));
        assert_eq!((transport.run.as_ref().unwrap().index, transport.steps_shown()), (1, None));
        transport.advance(&items, 0.36, &mut |index| entered.push(index));
        assert_eq!(transport.run.as_ref().unwrap().index, 1);
        assert_eq!(transport.advance(&items, 0.46, &mut |index| entered.push(index)), None);
        assert_eq!((entered, transport.playhead, transport.run.is_none()), (vec![1, 2], Some(2), true));
    }

    #[test]
    fn seeking_pausing_and_following() {
        let items = Items(vec![10, 10, 10, 10]);
        let mut transport = Transport::default();
        assert_eq!(transport.follow(Some(1)), None, "a stopped animation is not followed");
        transport.seek(&items, 2, 0.0);
        assert_eq!(transport.frame(None, 4), Some(2));
        assert_eq!(transport.follow(Some(2)), Some(2));
        assert_eq!(transport.follow(Some(2)), None);
        transport.play(&items, 1, 0.0);
        transport.seek(&items, 1, 0.0);
        let run = transport.run.clone().unwrap();
        assert_eq!((run.step, run.steps), (3, 3), "a seek shows the item complete");
        transport.pause();
        assert_eq!((transport.playhead, transport.playing()), (Some(1), false));
        assert_eq!(transport.frame(Some(3), 4), Some(1), "the animation frame wins over a preview");
        transport.stop();
        assert_eq!(transport.frame(Some(3), 4), Some(3));
        assert_eq!(RowMark::new(2, Some(1)), RowMark { current: false, pending: true });
    }

    #[test]
    fn actions_move_the_animation_or_the_preview() {
        let items = Items(vec![10, 10, 10, 10]);
        let mut transport = Transport::default();
        // While previewing through the selection, steps and seeks move the selection.
        let outcome = transport.apply(Action::Next, &items, Some(1), 0.0);
        assert_eq!((outcome.select, transport.animating()), (Some(2), false));
        assert_eq!(transport.apply(Action::Seek(9), &items, Some(1), 0.0).select, Some(3));
        // Play continues after the previewed item.
        assert_eq!(transport.apply(Action::PlayPause, &items, Some(1), 0.0).started, Some(2));
        let outcome = transport.apply(Action::Seek(0), &items, Some(1), 0.0);
        assert_eq!((outcome.jumped, outcome.silence), (Some(0), true));
        assert!(transport.apply(Action::PlayPause, &items, None, 0.0).silence);
        assert_eq!(transport.apply(Action::Next, &items, None, 0.0).stepped, Some(1));
        assert_eq!(transport.end_for_preview(4), Some(1));
        assert!(!transport.animating());
        assert!(transport.apply(Action::Stop, &items, None, 0.0).stopped);
    }
}
