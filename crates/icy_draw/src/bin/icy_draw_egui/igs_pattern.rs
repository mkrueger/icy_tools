//! The fill pattern dialog: one of the eight 16 × 16 user patterns that `X 7` commands load
//! and `A 4,n` fills use.

use eframe::egui::{self, Color32, Stroke};
use icy_draw::fl;
use icy_engine_gui::egui::appearance::{self, labels, DialogButton, DialogSize};
use icy_parser_core::{IgsCommand, PatternType};

const CELL: f32 = 16.0;
/// A picker swatch in screen points, and the screen points per pattern pixel in it.
const SWATCH: egui::Vec2 = egui::vec2(36.0, 22.0);
const SWATCH_PIXEL: f32 = 2.0;

pub enum PatternResult {
    Open,
    Cancel,
    Apply,
}

pub struct PatternDialog {
    slot: u8,
    data: [u16; 16],
    /// The pattern command edited in place.
    pub target: Option<usize>,
    /// The value cells are set to while the pointer is held down.
    painting: Option<bool>,
}

/// The user pattern of `slot` in effect before the end of `commands`.
pub fn pattern_before<'a>(commands: impl Iterator<Item = &'a IgsCommand>, slot: u8) -> [u16; 16] {
    let mut pattern = [0; 16];
    for command in commands {
        if let IgsCommand::LoadFillPattern { pattern: loaded, data } = command {
            if *loaded == slot {
                pattern = [0; 16];
                for (row, word) in pattern.iter_mut().zip(data) {
                    *row = *word;
                }
            }
        }
    }
    pattern
}

/// The user patterns of all eight slots in effect before the end of `commands`, as fills read
/// them.
pub fn user_patterns<'a>(commands: impl Iterator<Item = &'a IgsCommand> + Clone) -> [Vec<u16>; 8] {
    std::array::from_fn(|slot| pattern_before(commands.clone(), slot as u8).to_vec())
}

/// Paints `pattern` tiled into `rect`, in `color` on `background`, the way it fills.
pub fn paint_swatch(painter: &egui::Painter, rect: egui::Rect, pattern: PatternType, user: &[Vec<u16>; 8], color: Color32, background: Color32) {
    painter.rect_filled(rect, 2.0, background);
    if pattern != PatternType::Hollow {
        let rows = pattern.fill_pattern(user);
        let (columns, lines) = ((rect.width() / SWATCH_PIXEL) as usize, (rect.height() / SWATCH_PIXEL) as usize);
        for y in 0..lines {
            let row = rows.get(y % rows.len().max(1)).copied().unwrap_or(0);
            for x in 0..columns {
                if row & (0x8000 >> (x % 16)) != 0 {
                    let pixel = egui::Rect::from_min_size(rect.min + egui::vec2(x as f32, y as f32) * SWATCH_PIXEL, egui::Vec2::splat(SWATCH_PIXEL));
                    painter.rect_filled(pixel, 0.0, color);
                }
            }
        }
    }
    painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_gray(96)), egui::StrokeKind::Inside);
}

/// A clickable swatch of `pattern`, framed with the accent while it is the current one.
fn swatch(ui: &mut egui::Ui, pattern: PatternType, current: PatternType, user: &[Vec<u16>; 8], colors: (Color32, Color32)) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(SWATCH, egui::Sense::click());
    paint_swatch(ui.painter(), rect, pattern, user, colors.0, colors.1);
    let accent = ui.visuals().selection.stroke.color;
    if pattern == current {
        ui.painter()
            .rect_stroke(rect.expand(2.0), 3.0, Stroke::new(2.0, accent), egui::StrokeKind::Inside);
    } else if response.hovered() {
        ui.painter()
            .rect_stroke(rect.expand(2.0), 3.0, Stroke::new(1.0, ui.visuals().text_color()), egui::StrokeKind::Inside);
    }
    response.on_hover_text(super::pattern_name(pattern))
}

/// What the fill pattern picker changed.
#[derive(Debug, Default, PartialEq)]
pub struct PickerChange {
    pub pattern: Option<PatternType>,
    pub border: Option<bool>,
    /// The user pattern slot to draw in the pattern dialog.
    pub edit: Option<u8>,
}

/// The fill pattern control, like IG's pattern screen: a swatch of the current pattern that
/// opens all patterns to pick from, hatches and patterns, hollow and solid, the eight user
/// patterns and the border.
pub fn picker(ui: &mut egui::Ui, id: &str, current: PatternType, border: bool, user: &[Vec<u16>; 8], colors: (Color32, Color32), width: f32) -> PickerChange {
    let mut change = PickerChange::default();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width.max(SWATCH.x + 18.0), SWATCH.y + 4.0), egui::Sense::click());
    let visuals = ui.style().interact(&response);
    ui.painter().rect(rect, 4.0, visuals.weak_bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
    let sample = egui::Rect::from_min_max(rect.min + egui::vec2(3.0, 3.0), egui::pos2(rect.right() - 16.0, rect.bottom() - 3.0));
    paint_swatch(ui.painter(), sample, current, user, colors.0, colors.1);
    let arrow = egui::pos2(rect.right() - 8.0, rect.center().y);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![arrow + egui::vec2(-4.0, -2.0), arrow + egui::vec2(4.0, -2.0), arrow + egui::vec2(0.0, 3.0)],
        visuals.text_color(),
        Stroke::NONE,
    ));
    let label = format!("{}: {}", fl!("igs-fill"), super::pattern_name(current));
    let response = response.on_hover_text(&label);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, &label));
    egui::Popup::menu(&response)
        .id(egui::Id::new(id))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            let mut pick = |ui: &mut egui::Ui, pattern: PatternType| {
                if swatch(ui, pattern, current, user, colors).clicked() {
                    change.pattern = Some(pattern);
                }
            };
            ui.horizontal(|ui| {
                pick(ui, PatternType::Hollow);
                ui.label(fl!("igs-fill-hollow"));
                ui.add_space(12.0);
                pick(ui, PatternType::Solid);
                ui.label(fl!("igs-fill-solid"));
            });
            ui.weak(fl!("igs-fill-kind-pattern"));
            egui::Grid::new((id, "patterns")).spacing(egui::vec2(6.0, 6.0)).show(ui, |ui| {
                for index in 1..=24 {
                    pick(ui, PatternType::Pattern(index));
                    if index % 8 == 0 {
                        ui.end_row();
                    }
                }
            });
            ui.weak(fl!("igs-fill-kind-hatch"));
            egui::Grid::new((id, "hatches")).spacing(egui::vec2(6.0, 6.0)).show(ui, |ui| {
                for index in 1..=12 {
                    pick(ui, PatternType::Hatch(index));
                    if index % 6 == 0 {
                        ui.end_row();
                    }
                }
            });
            ui.weak(fl!("igs-fill-kind-user"));
            ui.horizontal(|ui| {
                for slot in 0..8 {
                    pick(ui, PatternType::UserDefined(slot));
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                let mut framed = border;
                if ui.checkbox(&mut framed, fl!("igs-fill-border")).changed() {
                    change.border = Some(framed);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(fl!("igs-pattern-draw-user")).on_hover_text(fl!("igs-pattern-edit-tooltip")).clicked() {
                        change.edit = Some(match current {
                            PatternType::UserDefined(slot) => slot.min(7),
                            _ => 0,
                        });
                    }
                });
            });
            if change.pattern.is_some() || change.edit.is_some() {
                ui.close();
            }
        });
    change
}

impl PatternDialog {
    pub fn new(slot: u8, data: [u16; 16], target: Option<usize>) -> Self {
        Self {
            slot: slot.min(7),
            data,
            target,
            painting: None,
        }
    }

    pub fn slot(&self) -> u8 {
        self.slot
    }

    pub fn command(&self) -> IgsCommand {
        IgsCommand::LoadFillPattern {
            pattern: self.slot,
            data: self.data.to_vec(),
        }
    }

    fn cell(&self, x: usize, y: usize) -> bool {
        self.data[y] & (0x8000 >> x) != 0
    }

    fn set_cell(&mut self, x: usize, y: usize, on: bool) {
        if on {
            self.data[y] |= 0x8000 >> x;
        } else {
            self.data[y] &= !(0x8000 >> x);
        }
    }

    #[cfg(test)]
    pub fn toggle(&mut self, x: usize, y: usize) {
        let on = !self.cell(x, y);
        self.set_cell(x, y, on);
    }

    /// The 16 × 16 cells, drawn black on white like IG's pattern editor whatever the pen
    /// colors are, with a stronger line through the middle for orientation.
    fn grid(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(CELL * 16.0), egui::Sense::click_and_drag());
        let cell_at = |pos: egui::Pos2| {
            let offset = (pos - rect.min) / CELL;
            ((0.0..16.0).contains(&offset.x) && (0.0..16.0).contains(&offset.y)).then_some((offset.x as usize, offset.y as usize))
        };
        if let Some((x, y)) = response.interact_pointer_pos().and_then(cell_at) {
            let value = *self.painting.get_or_insert(!self.cell(x, y));
            self.set_cell(x, y, value);
        }
        if !ui.input(|input| input.pointer.any_down()) {
            self.painting = None;
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::WHITE);
        for y in 0..16 {
            for x in 0..16 {
                if self.cell(x, y) {
                    let cell = egui::Rect::from_min_size(rect.min + egui::vec2(x as f32, y as f32) * CELL, egui::Vec2::splat(CELL));
                    painter.rect_filled(cell, 0.0, Color32::BLACK);
                }
            }
        }
        for line in 1..16 {
            let stroke = Stroke::new(1.0, if line == 8 { Color32::from_gray(110) } else { Color32::from_gray(200) });
            let offset = line as f32 * CELL;
            painter.line_segment(
                [rect.left_top() + egui::vec2(offset, 0.0), rect.left_bottom() + egui::vec2(offset, 0.0)],
                stroke,
            );
            painter.line_segment([rect.left_top() + egui::vec2(0.0, offset), rect.right_top() + egui::vec2(0.0, offset)], stroke);
        }
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, Color32::from_gray(110)), egui::StrokeKind::Inside);
        if let Some((x, y)) = response.hover_pos().and_then(cell_at) {
            let cell = egui::Rect::from_min_size(rect.min + egui::vec2(x as f32, y as f32) * CELL, egui::Vec2::splat(CELL));
            painter.rect_stroke(cell, 0.0, Stroke::new(2.0, ui.visuals().selection.stroke.color), egui::StrokeKind::Inside);
        }
    }

    /// The pattern tiled three times in each direction as fills draw it: the fill pen on the
    /// background color, at three screen points per pixel.
    fn tiled(&self, ui: &mut egui::Ui, colors: (Color32, Color32)) {
        const SCALE: f32 = 3.0;
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(SCALE * 48.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, colors.1);
        for y in 0..48 {
            for x in 0..48 {
                if self.cell(x % 16, y % 16) {
                    painter.rect_filled(
                        egui::Rect::from_min_size(rect.min + egui::vec2(x as f32, y as f32) * SCALE, egui::Vec2::splat(SCALE)),
                        0.0,
                        colors.0,
                    );
                }
            }
        }
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, Color32::from_gray(110)), egui::StrokeKind::Outside);
    }

    /// Moves the pattern one cell, wrapping around like the tiles do.
    pub fn shift(&mut self, dx: i32, dy: i32) {
        self.data = self
            .data
            .map(|row| if dx > 0 { row.rotate_right(dx as u32) } else { row.rotate_left((-dx) as u32) });
        if dy > 0 {
            self.data.rotate_right(dy as usize);
        } else {
            self.data.rotate_left((-dy) as usize);
        }
    }

    /// `load` gives the pattern a slot has in the drawing, used when the slot changes; `colors`
    /// are the fill pen and background the preview shows the pattern in.
    pub fn show(&mut self, context: &egui::Context, colors: (Color32, Color32), load: impl Fn(u8) -> [u16; 16]) -> PatternResult {
        #[derive(Clone, Copy)]
        enum Action {
            Clear,
            Invert,
            Cancel,
            Apply,
        }
        let response = appearance::Dialog::new("igs-pattern")
            .title(fl!("igs-pattern-title"))
            .subtitle(fl!("igs-pattern-subtitle"))
            .size(DialogSize::Width(560.0))
            .show(context, |dialog| {
                dialog.content(|ui| {
                    ui.horizontal(|ui| {
                        // The eight slots as the patterns they hold; a pattern command being
                        // edited keeps its slot.
                        ui.label(fl!("igs-pattern-slot"));
                        ui.add_space(8.0);
                        ui.spacing_mut().item_spacing.x = 6.0;
                        for slot in 0..8u8 {
                            let rows = if slot == self.slot { self.data } else { load(slot) };
                            let user = std::array::from_fn(|index| if index == usize::from(slot) { rows.to_vec() } else { Vec::new() });
                            let (rect, response) = ui.allocate_exact_size(egui::vec2(30.0, 22.0), egui::Sense::click());
                            paint_swatch(ui.painter(), rect, PatternType::UserDefined(slot), &user, colors.0, colors.1);
                            if slot == self.slot {
                                ui.painter().rect_stroke(
                                    rect.expand(2.0),
                                    3.0,
                                    Stroke::new(2.0, ui.visuals().selection.stroke.color),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            let response = response.on_hover_text(fl!("igs-pattern-slot-summary", slot = slot));
                            if response.clicked() && self.target.is_none() && slot != self.slot {
                                self.slot = slot;
                                self.data = load(slot);
                            }
                        }
                    });
                    ui.add_space(4.0);
                    ui.horizontal_top(|ui| {
                        self.grid(ui);
                        ui.add_space(16.0);
                        ui.vertical(|ui| {
                            ui.weak(fl!("igs-pattern-preview"));
                            self.tiled(ui, colors);
                            ui.add_space(12.0);
                            ui.weak(fl!("igs-pattern-shift"));
                            ui.horizontal(|ui| {
                                for (label, tooltip, dx, dy) in [
                                    ("←", fl!("igs-pattern-shift-left"), -1, 0),
                                    ("→", fl!("igs-pattern-shift-right"), 1, 0),
                                    ("↑", fl!("igs-pattern-shift-up"), 0, -1),
                                    ("↓", fl!("igs-pattern-shift-down"), 0, 1),
                                ] {
                                    if ui
                                        .add(egui::Button::new(label).min_size(egui::vec2(28.0, 24.0)))
                                        .on_hover_text(tooltip)
                                        .clicked()
                                    {
                                        self.shift(dx, dy);
                                    }
                                }
                            });
                        });
                    });
                });
                dialog.buttons([
                    DialogButton::secondary(fl!("igs-pattern-clear"), Action::Clear).leading(),
                    DialogButton::secondary(fl!("igs-pattern-invert"), Action::Invert).leading(),
                    DialogButton::cancel(labels::cancel(), Action::Cancel),
                    DialogButton::primary(fl!("button-apply"), Action::Apply),
                ]);
            });
        match response.action {
            Some(Action::Clear) => {
                self.data = [0; 16];
                PatternResult::Open
            }
            Some(Action::Invert) => {
                self.data = self.data.map(|row| !row);
                PatternResult::Open
            }
            Some(Action::Apply) => PatternResult::Apply,
            Some(Action::Cancel) => PatternResult::Cancel,
            None if response.dismissed => PatternResult::Cancel,
            None => PatternResult::Open,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_follow_the_last_load_of_their_slot() {
        let commands = [
            IgsCommand::LoadFillPattern {
                pattern: 1,
                data: vec![0xFFFF; 16],
            },
            IgsCommand::LoadFillPattern {
                pattern: 2,
                data: vec![0x00FF; 16],
            },
            IgsCommand::LoadFillPattern {
                pattern: 1,
                data: vec![0x8000; 16],
            },
        ];
        assert_eq!(pattern_before(commands.iter(), 1), [0x8000; 16]);
        assert_eq!(pattern_before(commands.iter(), 2), [0x00FF; 16]);
        assert_eq!(pattern_before(commands.iter(), 3), [0; 16]);
        let mut dialog = PatternDialog::new(3, [0; 16], None);
        dialog.toggle(0, 0);
        dialog.toggle(15, 15);
        let IgsCommand::LoadFillPattern { pattern: 3, data } = dialog.command() else {
            panic!("expected a pattern")
        };
        assert_eq!((data[0], data[15]), (0x8000, 0x0001));
    }

    #[test]
    fn patterns_move_one_cell_and_wrap_around() {
        let mut rows = [0; 16];
        rows[0] = 0x8001;
        let mut dialog = PatternDialog::new(0, rows, None);
        dialog.shift(1, 0);
        assert_eq!(dialog.data[0], 0xC000, "the rightmost pixel wraps to the left");
        dialog.shift(-1, 0);
        assert_eq!(dialog.data[0], 0x8001);
        dialog.shift(0, -1);
        assert_eq!((dialog.data[0], dialog.data[15]), (0, 0x8001), "the top row wraps to the bottom");
        dialog.shift(0, 1);
        assert_eq!(dialog.data[0], 0x8001);
    }
}
