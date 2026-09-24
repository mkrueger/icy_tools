//! Navigator strip for art taller than the view: a scaled render of the whole file with the
//! visible part outlined. Click or drag to jump; long files scroll the strip along.

use super::thumbnails::Thumbnails;
use eframe::egui;
use icy_view::items::Item;

pub const WIDTH: f32 = 92.0;
const MARGIN: f32 = 8.0;

pub struct Minimap {
    thumbnails: Thumbnails,
}

/// Viewport of the preview in content fractions and the matching strip geometry.
#[derive(Debug, PartialEq)]
pub struct Geometry {
    pub strip_height: f32,
    pub strip_scroll: f32,
    pub view_top: f32,
    pub view_height: f32,
}

pub fn geometry(image: egui::Vec2, area_height: f32, offset: f32, max_offset: f32, view_height: f32) -> Geometry {
    let strip_height = image.y * WIDTH / image.x.max(1.0);
    let total = (max_offset + view_height).max(1.0);
    let progress = if max_offset > 0.0 { (offset / max_offset).clamp(0.0, 1.0) } else { 0.0 };
    let strip_scroll = (strip_height - area_height).max(0.0) * progress;
    Geometry {
        strip_height,
        strip_scroll,
        view_top: offset / total * strip_height - strip_scroll,
        view_height: (view_height / total * strip_height).max(6.0),
    }
}

impl Minimap {
    pub fn new(context: &egui::Context) -> Self {
        Self {
            thumbnails: Thumbnails::new(context),
        }
    }

    /// Returns the scroll offset to jump to when the strip was clicked or dragged.
    pub fn show(&mut self, ui: &mut egui::Ui, area: egui::Rect, item: &dyn Item, offset: egui::Vec2, max_offset: egui::Vec2) -> Option<f32> {
        if max_offset.y <= 0.0 || area.width() < WIDTH * 3.0 {
            return None;
        }
        self.thumbnails.request(item);
        self.thumbnails.poll(ui.ctx());
        self.thumbnails.trim();
        let entry = self.thumbnails.entries.get(&Thumbnails::key(item))?;
        let image = entry.images.first()?;
        let height = area.height() - MARGIN * 2.0;
        let rect = egui::Rect::from_min_size(egui::pos2(area.right() - WIDTH - MARGIN - 12.0, area.top() + MARGIN), egui::vec2(WIDTH, height));
        let view_height = area.height();
        let geometry = geometry(image.size, height, offset.y, max_offset.y, view_height);
        let strip = egui::Rect::from_min_size(rect.min, egui::vec2(WIDTH, geometry.strip_height.min(height)));
        let response = ui.interact(strip, ui.id().with("minimap"), egui::Sense::click_and_drag());
        let visuals = ui.visuals().clone();
        let painter = ui.painter().with_clip_rect(strip.expand(1.0));
        painter.rect_filled(
            strip.expand(2.0),
            4.0,
            egui::Color32::from_black_alpha(if response.hovered() { 200 } else { 150 }),
        );
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(strip));
        child.set_clip_rect(strip);
        child.set_opacity(if response.hovered() || response.dragged() { 1.0 } else { 0.8 });
        image.paint(
            &child,
            egui::Rect::from_min_size(strip.min - egui::vec2(0.0, geometry.strip_scroll), egui::vec2(WIDTH, geometry.strip_height)),
        );
        let view = egui::Rect::from_min_size(strip.min + egui::vec2(0.0, geometry.view_top), egui::vec2(WIDTH, geometry.view_height)).intersect(strip);
        painter.rect_filled(view, 2.0, visuals.selection.bg_fill.gamma_multiply(0.25));
        painter.rect_stroke(view, 2.0, egui::Stroke::new(1.5, visuals.selection.stroke.color), egui::StrokeKind::Inside);
        let pointer = response.interact_pointer_pos().filter(|_| response.clicked() || response.dragged())?;
        let fraction = ((pointer.y - strip.top() + geometry.strip_scroll) / geometry.strip_height.max(1.0)).clamp(0.0, 1.0);
        let total = max_offset.y + view_height;
        Some((fraction * total - view_height / 2.0).clamp(0.0, max_offset.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_follows_the_view_and_scrolls_for_long_files() {
        let short = geometry(egui::vec2(640.0, 800.0), 600.0, 0.0, 400.0, 400.0);
        assert_eq!(short.strip_scroll, 0.0);
        assert_eq!(short.view_top, 0.0);
        assert!((short.view_height - short.strip_height * 0.5).abs() < 0.01);
        let long_top = geometry(egui::vec2(640.0, 64000.0), 600.0, 0.0, 9600.0, 400.0);
        assert_eq!(long_top.strip_scroll, 0.0);
        let long_end = geometry(egui::vec2(640.0, 64000.0), 600.0, 9600.0, 9600.0, 400.0);
        assert!((long_end.strip_scroll - (long_end.strip_height - 600.0)).abs() < 0.01);
        assert!((long_end.view_top + long_end.view_height - 600.0).abs() < 1.0, "{long_end:?}");
    }
}
