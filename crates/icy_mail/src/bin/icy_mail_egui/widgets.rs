use std::collections::HashMap;

use eframe::egui::{self, Color32, FontId, Rect, Response, Sense};
use icy_mail::reader::SortDirection;

pub const ROW_HEIGHT: f32 = 20.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Open,
    Refresh,
    Copy,
    Up,
    Down,
    Menu,
}

#[derive(Default)]
pub struct Icons(HashMap<Icon, egui::TextureHandle>);

impl Icons {
    pub fn button(&mut self, ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool) -> Response {
        let texture = self.0.entry(icon).or_insert_with(|| {
            let bytes: &[u8] = match icon {
                Icon::Open => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_folder.svg"),
                Icon::Refresh => include_bytes!("../../../../icy_engine_gui/src/ui/icons/rotate_right.svg"),
                Icon::Copy => include_bytes!("../../../../icy_engine_gui/src/ui/icons/content_copy.svg"),
                Icon::Up => include_bytes!("../../../../icy_engine_gui/src/ui/icons/arrow_upward.svg"),
                Icon::Down => include_bytes!("../../../../icy_engine_gui/src/ui/icons/arrow_downward.svg"),
                Icon::Menu => include_bytes!("../../../../icy_engine_gui/data/icons/menu.svg"),
            };
            let tree = resvg::usvg::Tree::from_data(bytes, &Default::default()).expect("bundled icon");
            let mut pixels = resvg::tiny_skia::Pixmap::new(48, 48).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::from_scale(48.0 / tree.size().width(), 48.0 / tree.size().height()),
                &mut pixels.as_mut(),
            );
            ui.ctx().load_texture(
                label,
                egui::ColorImage::from_rgba_premultiplied([48, 48], pixels.data()),
                egui::TextureOptions::LINEAR,
            )
        });
        let image = egui::Image::new((texture.id(), egui::Vec2::splat(18.0))).tint(ui.visuals().text_color());
        ui.add_enabled_ui(enabled, |ui| {
            ui.spacing_mut().button_padding = egui::vec2(4.0, 4.0);
            ui.add_sized([30.0, 30.0], egui::Button::image(image))
        })
        .inner
        .on_hover_text(label)
    }
}

pub fn header(ui: &mut egui::Ui, widths: &[f32], labels: &[&str], active: Option<(usize, SortDirection)>, enabled: bool) -> Option<usize> {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(widths.iter().sum(), 24.0), Sense::hover());
    ui.painter().rect_filled(rect, 0, ui.visuals().faint_bg_color);
    let mut left = rect.left();
    let mut clicked = None;
    for (index, (&width, label)) in widths.iter().zip(labels).enumerate() {
        let cell = Rect::from_min_size(egui::pos2(left, rect.top()), egui::vec2(width, rect.height()));
        let text = if let Some((_, direction)) = active.filter(|(column, _)| *column == index) {
            format!("{label} {}", direction.arrow())
        } else {
            label.to_string()
        };
        let response = ui.interact(cell, ui.id().with(("header", index)), if enabled { Sense::click() } else { Sense::hover() });
        let color = if active.is_some_and(|(column, _)| column == index) {
            ui.visuals().hyperlink_color
        } else {
            ui.visuals().text_color()
        };
        paint_cell(ui, cell, &text, color, 0.0);
        if response.clicked() {
            clicked = Some(index);
        }
        left += width;
    }
    clicked
}

pub fn row(ui: &mut egui::Ui, widths: &[f32], values: &[&str], selected: bool, focused: bool, depth: u16) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(widths.iter().sum(), ROW_HEIGHT), Sense::click());
    let fill = if selected && focused {
        ui.visuals().selection.bg_fill
    } else if selected {
        ui.visuals().widgets.active.weak_bg_fill
    } else if response.hovered() {
        ui.visuals().widgets.hovered.weak_bg_fill
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 0, fill);
    let color = if selected && focused {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals().text_color()
    };
    let mut left = rect.left();
    for (index, (&width, value)) in widths.iter().zip(values).enumerate() {
        let cell = Rect::from_min_size(egui::pos2(left, rect.top()), egui::vec2(width, ROW_HEIGHT));
        paint_cell(ui, cell, value, color, if index == 2 { f32::from(depth) * 14.0 } else { 0.0 });
        left += width;
    }
    response.on_hover_text(values.join("\n"))
}

fn paint_cell(ui: &egui::Ui, rect: Rect, text: &str, color: Color32, indent: f32) {
    let clipped = rect.shrink2(egui::vec2(4.0, 0.0));
    ui.painter().with_clip_rect(ui.clip_rect().intersect(clipped)).text(
        egui::pos2(clipped.left() + indent, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        FontId::monospace(12.0),
        color,
    );
}

pub fn reveal(position: usize, offset: f32, height: f32) -> f32 {
    let top = position as f32 * ROW_HEIGHT;
    let bottom = top + ROW_HEIGHT;
    if top < offset || height <= 0.0 {
        top
    } else if bottom > offset + height {
        (bottom - height).max(0.0)
    } else {
        offset
    }
}
