//! The polymarker control, like IG's "select type size" screen: a swatch of the current marker
//! that opens the six marker types and the size to pick from.

use eframe::egui::{self, Color32, Stroke};
use icy_draw::fl;
use icy_parser_core::PolymarkerKind;

pub const KINDS: [PolymarkerKind; 6] = [
    PolymarkerKind::Point,
    PolymarkerKind::Plus,
    PolymarkerKind::Star,
    PolymarkerKind::Square,
    PolymarkerKind::DiagonalCross,
    PolymarkerKind::Diamond,
];
pub const MAX_SIZE: u8 = 8;
const SWATCH: egui::Vec2 = egui::vec2(36.0, 22.0);
/// Screen points per drawing pixel at most, so small markers stay recognizable.
const MAX_ZOOM: f32 = 2.0;

/// The pixels a marker of `kind` and `size` sets at `(x, y)`, the way the VDI draws its solid
/// lines, for a preview that matches the plotted marker.
pub fn pixels(kind: PolymarkerKind, size: u8, x: i32, y: i32) -> Vec<(i32, i32)> {
    let scale = i32::from(size.max(1));
    let points = kind.points();
    let mut pixels = Vec::new();
    let mut index = 1;
    for _ in 0..points[0] {
        let count = points[index] as usize;
        index += 1;
        let line: Vec<(i32, i32)> = (0..count)
            .map(|n| (scale * points[index + 2 * n] + x, scale * points[index + 2 * n + 1] + y))
            .collect();
        index += 2 * count;
        for pair in line.windows(2) {
            line_pixels(pair[0], pair[1], &mut pixels);
        }
    }
    pixels.sort_unstable();
    pixels.dedup();
    pixels
}

/// The solid line of `VdiPaint::draw_line`.
fn line_pixels((mut x0, mut y0): (i32, i32), (mut x1, mut y1): (i32, i32), pixels: &mut Vec<(i32, i32)>) {
    if x1 < x0 {
        std::mem::swap(&mut x0, &mut x1);
        std::mem::swap(&mut y0, &mut y1);
    }
    if x0 == x1 {
        pixels.extend((y0.min(y1)..=y0.max(y1)).map(|y| (x0, y)));
        return;
    }
    if y0 == y1 {
        pixels.extend((x0..=x1).map(|x| (x, y0)));
        return;
    }
    let (mut dx, mut dy) = (x1 - x0, y1 - y0);
    let y_step = if dy < 0 { -1 } else { 1 };
    dy = dy.abs();
    let (mut x, mut y) = (x0, y0);
    if dx >= dy {
        let mut eps = -dx;
        let (e1, e2) = (2 * dy, 2 * dx);
        while dx >= 0 {
            pixels.push((x, y));
            x += 1;
            eps += e1;
            if eps >= 0 {
                eps -= e2;
                y += y_step;
            }
            dx -= 1;
        }
    } else {
        let mut eps = -dy;
        let (e1, e2) = (2 * dx, 2 * dy);
        while dy >= 0 {
            pixels.push((x, y));
            y += y_step;
            eps += e1;
            if eps >= 0 {
                eps -= e2;
                x += 1;
            }
            dy -= 1;
        }
    }
}

/// Paints `kind` at `size` centered in `rect`, from the same lines the drawing uses, scaled
/// down when it does not fit.
pub fn paint(painter: &egui::Painter, rect: egui::Rect, kind: PolymarkerKind, size: u8, color: Color32, background: Color32) {
    painter.rect_filled(rect, 2.0, background);
    let scale = f32::from(size.max(1));
    // The largest marker reaches ±4 by ±3 units, plus the pixel it is drawn with.
    let zoom = ((rect.width() - 4.0) / (8.0 * scale + 1.0))
        .min((rect.height() - 4.0) / (6.0 * scale + 1.0))
        .min(MAX_ZOOM);
    let center = rect.center();
    let point = |x: i32, y: i32| center + egui::vec2(x as f32, y as f32) * scale * zoom;
    let stroke = Stroke::new(zoom.max(1.0), color);
    let points = kind.points();
    let mut index = 1;
    for _ in 0..points[0] {
        let count = points[index] as usize;
        index += 1;
        let line: Vec<egui::Pos2> = (0..count).map(|n| point(points[index + 2 * n], points[index + 2 * n + 1])).collect();
        index += 2 * count;
        if line.windows(2).all(|pair| pair[0] == pair[1]) {
            painter.rect_filled(egui::Rect::from_center_size(line[0], egui::Vec2::splat(zoom.max(1.0))), 0.0, color);
        } else {
            painter.add(egui::Shape::line(line, stroke));
        }
    }
    painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_gray(96)), egui::StrokeKind::Inside);
}

/// What the polymarker picker changed.
#[derive(Debug, Default, PartialEq)]
pub struct MarkerChange {
    pub kind: Option<PolymarkerKind>,
    pub size: Option<u8>,
}

/// The polymarker control: the current marker in `colors` (marker, background) with its size,
/// opening the marker types and the size.
pub fn picker(ui: &mut egui::Ui, id: &str, kind: PolymarkerKind, size: u8, colors: (Color32, Color32), width: f32) -> MarkerChange {
    let mut change = MarkerChange::default();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width.max(SWATCH.x + 40.0), SWATCH.y + 4.0), egui::Sense::click());
    let visuals = ui.style().interact(&response);
    ui.painter().rect(rect, 4.0, visuals.weak_bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
    let sample = egui::Rect::from_min_size(rect.min + egui::vec2(3.0, 3.0), egui::vec2(SWATCH.x, rect.height() - 6.0));
    paint(ui.painter(), sample, kind, size, colors.0, colors.1);
    ui.painter().text(
        egui::pos2(sample.right() + 6.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        format!("× {size}"),
        egui::TextStyle::Body.resolve(ui.style()),
        visuals.text_color(),
    );
    let arrow = egui::pos2(rect.right() - 8.0, rect.center().y);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![arrow + egui::vec2(-4.0, -2.0), arrow + egui::vec2(4.0, -2.0), arrow + egui::vec2(0.0, 3.0)],
        visuals.text_color(),
        Stroke::NONE,
    ));
    let label = format!("{}: {}, {} {size}", fl!("igs-marker"), super::marker_name(kind), fl!("igs-size"));
    let response = response.on_hover_text(&label);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, &label));
    egui::Popup::menu(&response)
        .id(egui::Id::new(id))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
            for candidate in KINDS {
                let row = ui
                    .horizontal(|ui| {
                        let (swatch, _) = ui.allocate_exact_size(SWATCH, egui::Sense::hover());
                        paint(ui.painter(), swatch, candidate, 1, colors.0, colors.1);
                        if candidate == kind {
                            let accent = ui.visuals().selection.stroke.color;
                            ui.painter()
                                .rect_stroke(swatch.expand(2.0), 3.0, Stroke::new(2.0, accent), egui::StrokeKind::Inside);
                        }
                        ui.selectable_label(candidate == kind, super::marker_name(candidate))
                    })
                    .inner;
                if row.clicked() {
                    change.kind = Some(candidate);
                }
            }
            ui.separator();
            let mut picked = size.clamp(1, MAX_SIZE);
            ui.horizontal(|ui| {
                ui.label(fl!("igs-size"));
                if ui
                    .add_enabled(picked > 1, egui::Button::new("−"))
                    .on_hover_text(fl!("igs-marker-size-down"))
                    .clicked()
                {
                    picked -= 1;
                }
                ui.add(egui::DragValue::new(&mut picked).range(1..=MAX_SIZE));
                if ui
                    .add_enabled(picked < MAX_SIZE, egui::Button::new("+"))
                    .on_hover_text(fl!("igs-marker-size-up"))
                    .clicked()
                {
                    picked += 1;
                }
            });
            if picked != size {
                change.size = Some(picked);
            }
            let shown = change.kind.unwrap_or(kind);
            let (preview, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(140.0), 60.0), egui::Sense::hover());
            paint(ui.painter(), preview, shown, picked, colors.0, colors.1);
            if shown == PolymarkerKind::Point {
                ui.weak(fl!("igs-marker-point-size"));
            }
            if change.kind.is_some() {
                ui.close();
            }
        });
    change
}
