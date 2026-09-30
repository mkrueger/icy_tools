//! The line type control, like IG's line "select type size" screen: a sample of the current
//! line that opens the line types, the width or user line pattern and the line ends.

use eframe::egui::{self, Color32, Stroke};
use icy_draw::fl;
use icy_parser_core::{user_line_mask, ArrowEnd, LineKind, USER_LINE_PATTERNS};

pub const KINDS: [LineKind; 7] = [
    LineKind::Solid,
    LineKind::LongDash,
    LineKind::Dotted,
    LineKind::DashDot,
    LineKind::Dashed,
    LineKind::DashDotDot,
    LineKind::UserDefined,
];
/// The line end pairs `T 2,n,50-64` can set, in the order of IG's line screen.
pub const ENDS: [(ArrowEnd, ArrowEnd); 9] = [
    (ArrowEnd::Square, ArrowEnd::Square),
    (ArrowEnd::Arrow, ArrowEnd::Arrow),
    (ArrowEnd::Rounded, ArrowEnd::Rounded),
    (ArrowEnd::Arrow, ArrowEnd::Square),
    (ArrowEnd::Square, ArrowEnd::Arrow),
    (ArrowEnd::Arrow, ArrowEnd::Rounded),
    (ArrowEnd::Rounded, ArrowEnd::Arrow),
    (ArrowEnd::Rounded, ArrowEnd::Square),
    (ArrowEnd::Square, ArrowEnd::Rounded),
];
pub const MAX_WIDTH: u8 = 41;
const SWATCH: egui::Vec2 = egui::vec2(64.0, 22.0);
/// Screen points per drawing pixel in the samples.
const ZOOM: f32 = 2.0;

/// A line type with everything that changes how it looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineStyle {
    pub kind: LineKind,
    /// The width of solid lines.
    pub width: u8,
    /// The user line pattern of user defined lines.
    pub pattern: u8,
    pub ends: (ArrowEnd, ArrowEnd),
}

impl LineStyle {
    /// The 16 pixel dash pattern the VDI repeats along the line.
    pub fn mask(&self, user: &[Vec<u16>; 8]) -> u16 {
        self.kind.mask(user_line_mask(user, self.pattern))
    }

    /// Only solid lines are drawn wide.
    pub fn drawn_width(&self) -> u8 {
        if self.kind == LineKind::Solid {
            self.width.max(1)
        } else {
            1
        }
    }
}

/// Paints a horizontal line in `style` across `rect`: its dashes pixel by pixel, its width
/// scaled down to fit, and its ends.
pub fn paint(painter: &egui::Painter, rect: egui::Rect, style: LineStyle, user: &[Vec<u16>; 8], color: Color32, background: Color32) {
    painter.rect_filled(rect, 2.0, background);
    let height = (f32::from(style.drawn_width()) * ZOOM).min(rect.height() - 6.0).max(ZOOM);
    let head = (height * 1.6).max(4.0 * ZOOM).min(rect.height() / 2.0 - 1.0);
    let (left, right) = (rect.left() + 3.0 + head, rect.right() - 3.0 - head);
    let y = rect.center().y;
    let mask = style.mask(user);
    let pixels = ((right - left) / ZOOM).floor() as u32;
    for index in 0..pixels {
        // The VDI rotates the mask before each pixel, starting from its highest bit.
        if mask.rotate_left(index + 1) & 1 != 0 {
            let x = left + index as f32 * ZOOM;
            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(x, y - height / 2.0), egui::vec2(ZOOM, height)), 0.0, color);
        }
    }
    for (end, x, outward) in [(style.ends.0, left, -1.0), (style.ends.1, right, 1.0)] {
        match end {
            ArrowEnd::Square => {}
            ArrowEnd::Arrow => {
                painter.add(egui::Shape::convex_polygon(
                    vec![egui::pos2(x + outward * head, y), egui::pos2(x, y - head * 0.7), egui::pos2(x, y + head * 0.7)],
                    color,
                    Stroke::NONE,
                ));
            }
            ArrowEnd::Rounded => {
                painter.circle_filled(egui::pos2(x, y), height / 2.0 + ZOOM * 0.5, color);
            }
        }
    }
    painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_gray(96)), egui::StrokeKind::Inside);
}

fn ends_name((start, end): (ArrowEnd, ArrowEnd)) -> String {
    format!("{} · {}", super::properties::line_end_name(start), super::properties::line_end_name(end))
}

/// The line type control: the current line in `colors` (line, background), opening the line
/// types, the width or user line pattern and the ends. Returns the style picked.
pub fn picker(ui: &mut egui::Ui, id: &str, style: LineStyle, user: &[Vec<u16>; 8], colors: (Color32, Color32), width: f32) -> Option<LineStyle> {
    let mut picked = style;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width.max(SWATCH.x + 18.0), SWATCH.y + 4.0), egui::Sense::click());
    let visuals = ui.style().interact(&response);
    ui.painter().rect(rect, 4.0, visuals.weak_bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
    let sample = egui::Rect::from_min_max(rect.min + egui::vec2(3.0, 3.0), egui::pos2(rect.right() - 16.0, rect.bottom() - 3.0));
    paint(ui.painter(), sample, style, user, colors.0, colors.1);
    let arrow = egui::pos2(rect.right() - 8.0, rect.center().y);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![arrow + egui::vec2(-4.0, -2.0), arrow + egui::vec2(4.0, -2.0), arrow + egui::vec2(0.0, 3.0)],
        visuals.text_color(),
        Stroke::NONE,
    ));
    let size = match style.kind {
        LineKind::Solid => format!(", {} {}", fl!("igs-thickness"), style.width),
        LineKind::UserDefined => format!(", {} {}", fl!("igs-line-pattern"), style.pattern),
        _ => String::new(),
    };
    let label = format!(
        "{}: {}{size}, {}",
        fl!("igs-line-type"),
        super::line_kind_name(style.kind),
        ends_name(style.ends)
    );
    let response = response.on_hover_text(&label);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, &label));
    egui::Popup::menu(&response)
        .id(egui::Id::new(id))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
            let row = |ui: &mut egui::Ui, sample: LineStyle, selected: bool, name: String| {
                ui.horizontal(|ui| {
                    let (swatch, _) = ui.allocate_exact_size(SWATCH, egui::Sense::hover());
                    paint(ui.painter(), swatch, sample, user, colors.0, colors.1);
                    if selected {
                        let accent = ui.visuals().selection.stroke.color;
                        ui.painter()
                            .rect_stroke(swatch.expand(2.0), 3.0, Stroke::new(2.0, accent), egui::StrokeKind::Inside);
                    }
                    ui.selectable_label(selected, name)
                })
                .inner
                .clicked()
            };
            // Fixed widths only: separators and previews would stretch the popup to the window.
            let columns = ui
                .horizontal(|ui| {
                    ui.vertical(|ui| {
                        for kind in KINDS {
                            let sample = LineStyle {
                                kind,
                                width: 1,
                                ends: (ArrowEnd::Square, ArrowEnd::Square),
                                ..picked
                            };
                            if row(ui, sample, picked.kind == kind, super::line_kind_name(kind)) {
                                picked.kind = kind;
                            }
                        }
                        ui.add_space(8.0);
                        // IG: only solid lines are sizable, user defined lines pick their pattern.
                        let (label, value, max) = match picked.kind {
                            LineKind::Solid => (fl!("igs-thickness"), &mut picked.width, MAX_WIDTH),
                            LineKind::UserDefined => (fl!("igs-line-pattern"), &mut picked.pattern, USER_LINE_PATTERNS),
                            _ => {
                                ui.weak(fl!("igs-line-only-solid-wide"));
                                return;
                            }
                        };
                        ui.horizontal(|ui| {
                            ui.label(label);
                            if ui.add_enabled(*value > 1, egui::Button::new("−")).clicked() {
                                *value -= 1;
                            }
                            ui.add(egui::DragValue::new(value).range(1..=max));
                            if ui.add_enabled(*value < max, egui::Button::new("+")).clicked() {
                                *value += 1;
                            }
                        });
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        for ends in ENDS {
                            let sample = LineStyle {
                                kind: LineKind::Solid,
                                width: 1,
                                ends,
                                ..picked
                            };
                            if row(ui, sample, picked.ends == ends, ends_name(ends)) {
                                picked.ends = ends;
                            }
                        }
                    });
                })
                .response
                .rect
                .width();
            ui.add_space(6.0);
            let (preview, _) = ui.allocate_exact_size(egui::vec2(columns, 48.0), egui::Sense::hover());
            paint(ui.painter(), preview, picked, user, colors.0, colors.1);
        });
    (picked != style).then_some(picked)
}
