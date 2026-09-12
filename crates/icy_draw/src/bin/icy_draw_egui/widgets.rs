use eframe::egui::{self, Color32, Response};
use std::collections::HashMap;

#[derive(rust_embed::RustEmbed)]
#[folder = "data/icons"]
struct Assets;

#[derive(Default)]
pub struct Icons(HashMap<String, egui::TextureHandle>);

impl Icons {
    pub fn button(&mut self, ui: &mut egui::Ui, name: &str, label: &str, selected: bool) -> Response {
        self.button_sized(ui, name, label, selected, 30.0)
    }

    pub fn button_sized(&mut self, ui: &mut egui::Ui, name: &str, label: &str, selected: bool, size: f32) -> Response {
        let image = self.image(ui, name, size * 2.0 / 3.0);
        ui.scope(|ui| {
            ui.spacing_mut().button_padding = egui::vec2(4.0, 4.0);
            ui.add_sized([size, size], egui::Button::image(image).selected(selected))
        })
        .inner
        .on_hover_text(label)
    }

    pub fn image(&mut self, ui: &egui::Ui, name: &str, size: f32) -> egui::Image<'static> {
        let texture = self.0.entry(name.to_owned()).or_insert_with(|| {
            let data = Assets::get(&format!("{name}.svg")).expect("bundled draw icon");
            let tree = resvg::usvg::Tree::from_data(&data.data, &Default::default()).expect("valid draw icon");
            let mut pixels = resvg::tiny_skia::Pixmap::new(48, 48).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::from_scale(48.0 / tree.size().width(), 48.0 / tree.size().height()),
                &mut pixels.as_mut(),
            );
            ui.ctx().load_texture(
                name,
                egui::ColorImage::from_rgba_premultiplied([48, 48], pixels.data()),
                egui::TextureOptions::LINEAR,
            )
        });
        egui::Image::new((texture.id(), egui::Vec2::splat(size))).tint(ui.visuals().text_color())
    }
}

pub fn segment(ui: &mut egui::Ui, label: &str, selected: bool, width: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 36.0), egui::Sense::click());
    let visuals = ui.style().interact_selectable(&response, selected);
    ui.painter().rect_filled(rect, 0, visuals.bg_fill);
    ui.painter().rect_stroke(rect, 0, visuals.bg_stroke, egui::StrokeKind::Inside);
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.left_bottom() - egui::vec2(0.0, 2.0), egui::vec2(rect.width(), 2.0)),
            0,
            ui.visuals().selection.stroke.color,
        );
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::TextStyle::Body.resolve(ui.style()),
        visuals.text_color(),
    );
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, ui.is_enabled(), selected, label));
    response
}

pub fn fkey(ui: &mut egui::Ui, font: &icy_engine::BitFont, code: char, index: usize) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(32.0, 44.0), egui::Sense::click());
    let visuals = *ui.style().interact(&response);
    ui.painter().rect_filled(rect, 2, visuals.bg_fill);
    let glyph_rect = egui::Rect::from_center_size(rect.center_top() + egui::vec2(0.0, 14.0), egui::Vec2::splat(26.0));
    let glyph_response = ui
        .scope_builder(egui::UiBuilder::new().max_rect(glyph_rect), |ui| glyph(ui, font, code, false, 26.0))
        .inner;
    let label = format!("F{}", index + 1);
    ui.painter().text(
        rect.center_bottom() - egui::vec2(0.0, 3.0),
        egui::Align2::CENTER_BOTTOM,
        &label,
        egui::TextStyle::Small.resolve(ui.style()),
        visuals.text_color(),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &label));
    response.union(glyph_response).on_hover_text(format!("{label}: character {}", code as u32))
}

pub fn swatch(ui: &mut egui::Ui, color: Color32, selected: bool, size: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    ui.painter().rect_filled(rect.shrink(2.0), 1, color);
    if selected || response.hovered() {
        ui.painter()
            .rect_stroke(rect.shrink(0.5), 1, egui::Stroke::new(1.0, ui.visuals().text_color()), egui::StrokeKind::Inside);
    }
    response
}

pub fn glyph(ui: &mut egui::Ui, font: &icy_engine::BitFont, code: char, selected: bool, size: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let fill = if selected {
        ui.visuals().selection.bg_fill
    } else if response.hovered() {
        ui.visuals().widgets.hovered.bg_fill
    } else {
        ui.visuals().extreme_bg_color
    };
    ui.painter().rect_filled(rect, 0, fill);
    let dimensions = font.size();
    let scale = ((size - 4.0) / dimensions.width.max(dimensions.height).max(1) as f32).max(0.1);
    let origin = rect.center() - egui::vec2(dimensions.width as f32, dimensions.height as f32) * scale * 0.5;
    let key = ui.id().with("glyph-atlas");
    let existing = ui.ctx().data(|data| data.get_temp::<std::sync::Arc<GlyphAtlas>>(key));
    let atlas = if let Some(atlas) = existing.filter(|atlas| atlas.font == *font) {
        atlas
    } else {
        let width = dimensions.width.max(1) as usize;
        let height = dimensions.height.max(1) as usize;
        let mut image = egui::ColorImage::filled([width * 16, height * 16], Color32::TRANSPARENT);
        for code in 0..256 {
            for (row, pixels) in font.glyph(char::from_u32(code).unwrap()).to_bitmap_pixels().iter().take(height).enumerate() {
                for (column, &enabled) in pixels.iter().take(width).enumerate() {
                    if enabled {
                        image[((code as usize % 16) * width + column, (code as usize / 16) * height + row)] = Color32::WHITE;
                    }
                }
            }
        }
        let atlas = std::sync::Arc::new(GlyphAtlas {
            font: font.clone(),
            texture: ui.ctx().load_texture("glyph-atlas", image, egui::TextureOptions::NEAREST),
        });
        ui.ctx().data_mut(|data| data.insert_temp(key, atlas.clone()));
        atlas
    };
    let code = (code as u32).min(255);
    let uv = egui::Rect::from_min_size(egui::pos2((code % 16) as f32 / 16.0, (code / 16) as f32 / 16.0), egui::Vec2::splat(1.0 / 16.0));
    let target = egui::Rect::from_min_size(origin, egui::vec2(dimensions.width as f32, dimensions.height as f32) * scale);
    ui.painter().image(atlas.texture.id(), target, uv, ui.visuals().text_color());
    response.on_hover_text(format!("{} (0x{:02X})", code as u32, code as u32))
}

struct GlyphAtlas {
    font: icy_engine::BitFont,
    texture: egui::TextureHandle,
}
