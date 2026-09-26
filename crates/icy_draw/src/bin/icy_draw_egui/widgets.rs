use eframe::egui::{self, Color32, Response};
use icy_engine_gui::egui::appearance::PRIMARY;
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

    /// Frameless icon button: transparent until hovered, filled with the accent colour when selected.
    pub fn button_sized(&mut self, ui: &mut egui::Ui, name: &str, label: &str, selected: bool, size: f32) -> Response {
        self.paint_button(ui, name, label, selected, false, size)
    }

    /// Icon button drawn with a muted tint unless hovered, for secondary per-row toggles.
    pub fn subtle_button(&mut self, ui: &mut egui::Ui, name: &str, label: &str, muted: bool, size: f32) -> Response {
        self.paint_button(ui, name, label, false, muted, size)
    }

    fn paint_button(&mut self, ui: &mut egui::Ui, name: &str, label: &str, selected: bool, muted: bool, size: f32) -> Response {
        let icon = (size * 0.56).round();
        let image = self.image(ui, name, icon);
        let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::click());
        let enabled = ui.is_enabled();
        let visuals = ui.visuals();
        let fill = if selected {
            Some(PRIMARY)
        } else if enabled && response.is_pointer_button_down_on() {
            Some(visuals.widgets.active.weak_bg_fill)
        } else if enabled && response.hovered() {
            Some(visuals.widgets.hovered.weak_bg_fill)
        } else {
            None
        };
        if let Some(fill) = fill {
            ui.painter().rect_filled(rect, 6, fill);
        }
        let tint = if selected {
            Color32::WHITE
        } else if enabled && muted && !response.hovered() {
            visuals.weak_text_color().gamma_multiply(0.6)
        } else if enabled {
            visuals.text_color()
        } else {
            visuals.weak_text_color().gamma_multiply(0.5)
        };
        image
            .tint(tint)
            .paint_at(ui, egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(icon)));
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label));
        response.on_hover_text(label).on_disabled_hover_text(label)
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

/// Height shared by the controls in the tool options bar.
pub const CONTROL_HEIGHT: f32 = 30.0;

const OUTLINE_PATTERN: [u8; 48] = [
    69, 65, 65, 65, 65, 65, 65, 70, 67, 79, 71, 66, 66, 72, 79, 68, 67, 79, 73, 65, 65, 74, 79, 68, 67, 79, 71, 66, 66, 72, 79, 68, 67, 79, 68, 64, 64, 67, 79,
    68, 75, 66, 76, 64, 64, 75, 66, 76,
];

/// Visual picker for the 19 TheDraw outline styles.
pub fn outline_style_picker(ui: &mut egui::Ui, current: &mut usize) -> Response {
    let button = ui
        .add_sized([90.0, CONTROL_HEIGHT], egui::Button::new(format!("Style {}", *current + 1)))
        .on_hover_text("Choose an outline style");
    egui::Popup::menu(&button).show(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        egui::Grid::new("outline-style-grid").num_columns(5).show(ui, |ui| {
            for style in 0..19 {
                if outline_style_cell(ui, style, *current == style).clicked() {
                    *current = style;
                    ui.close();
                }
                if style % 5 == 4 {
                    ui.end_row();
                }
            }
        });
    });
    button
}

fn outline_style_cell(ui: &mut egui::Ui, style: usize, selected: bool) -> Response {
    const SCALE: f32 = 0.75;
    const COLUMNS: usize = 8;
    const ROWS: usize = 6;
    let font = icy_engine::BitFont::default();
    let dimensions = font.size();
    let preview_size = egui::vec2(dimensions.width as f32 * COLUMNS as f32 * SCALE, dimensions.height as f32 * ROWS as f32 * SCALE);
    let (rect, response) = ui.allocate_exact_size(preview_size + egui::vec2(28.0, 12.0), egui::Sense::click());
    let visuals = ui.visuals();
    let fill = if selected {
        visuals.selection.bg_fill
    } else if response.hovered() {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        visuals.faint_bg_color
    };
    ui.painter().rect_filled(rect, 5, fill);
    ui.painter().rect_stroke(
        rect,
        5,
        if selected {
            visuals.selection.stroke
        } else {
            visuals.widgets.noninteractive.bg_stroke
        },
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 8.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        char::from_u32(b'A' as u32 + style as u32).unwrap_or('?'),
        egui::FontId::monospace(11.0),
        visuals.weak_text_color(),
    );
    let origin = egui::pos2(rect.left() + 24.0, rect.top() + 6.0);
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            let unicode = retrofont::transform_outline(style, OUTLINE_PATTERN[column + row * COLUMNS]);
            let character = codepages::tables::UNICODE_TO_CP437.get(&unicode).copied().map(char::from).unwrap_or(unicode);
            for (pixel_y, pixels) in font.glyph(character).to_bitmap_pixels().iter().enumerate() {
                for (pixel_x, enabled) in pixels.iter().enumerate() {
                    if *enabled {
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                origin
                                    + egui::vec2(
                                        (column * dimensions.width as usize + pixel_x) as f32 * SCALE,
                                        (row * dimensions.height as usize + pixel_y) as f32 * SCALE,
                                    ),
                                egui::Vec2::splat(SCALE),
                            ),
                            0,
                            visuals.strong_text_color(),
                        );
                    }
                }
            }
        }
    }
    response.on_hover_text(format!("Style {} ({})", style + 1, char::from_u32(b'A' as u32 + style as u32).unwrap_or('?')))
}

/// Pill segmented control: an inset track with the current option filled with the accent colour.
/// Each option is `(value, label, tooltip)`. Returns true when the selection changed.
pub fn segmented<T: PartialEq + Copy>(ui: &mut egui::Ui, current: &mut T, options: &[(T, &str, &str)]) -> bool {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galleys: Vec<_> = options
        .iter()
        .map(|(_, label, _)| ui.fonts_mut(|fonts| fonts.layout_no_wrap((*label).to_owned(), font.clone(), Color32::PLACEHOLDER)))
        .collect();
    let widths: Vec<f32> = galleys.iter().map(|galley| (galley.size().x + 20.0).max(40.0)).collect();
    let inset = 2.0;
    let (track, _) = ui.allocate_exact_size(egui::vec2(widths.iter().sum::<f32>() + inset * 2.0, CONTROL_HEIGHT), egui::Sense::hover());
    let visuals = ui.visuals().clone();
    ui.painter().rect_filled(track, 7, visuals.extreme_bg_color);
    ui.painter()
        .rect_stroke(track, 7, visuals.widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
    let mut changed = false;
    let mut left = track.left() + inset;
    for (((value, label, tooltip), galley), width) in options.iter().zip(galleys).zip(widths) {
        let rect = egui::Rect::from_min_size(egui::pos2(left, track.top() + inset), egui::vec2(width, CONTROL_HEIGHT - inset * 2.0));
        left += width;
        let response = ui.interact(rect, ui.id().with(("segment", *label)), egui::Sense::click());
        let selected = *current == *value;
        let enabled = ui.is_enabled();
        let fill = if selected {
            Some(PRIMARY)
        } else if enabled && response.hovered() {
            Some(visuals.widgets.hovered.weak_bg_fill)
        } else {
            None
        };
        if let Some(fill) = fill {
            ui.painter().rect_filled(rect, 5, fill);
        }
        let color = if selected {
            Color32::WHITE
        } else if enabled {
            visuals.text_color()
        } else {
            visuals.weak_text_color()
        };
        ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, selected, *label));
        let response = if tooltip.is_empty() { response } else { response.on_hover_text(*tooltip) };
        if response.clicked() && !selected {
            *current = *value;
            changed = true;
        }
    }
    changed
}

/// On/off chip for boolean tool options, accent filled while on.
pub fn toggle(ui: &mut egui::Ui, label: &str, value: &mut bool, tooltip: &str) -> Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui.fonts_mut(|fonts| fonts.layout_no_wrap(label.to_owned(), font, Color32::PLACEHOLDER));
    let (rect, mut response) = ui.allocate_exact_size(egui::vec2((galley.size().x + 20.0).max(36.0), CONTROL_HEIGHT), egui::Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    let enabled = ui.is_enabled();
    let visuals = ui.visuals();
    let (fill, stroke, color) = if *value {
        (PRIMARY, egui::Stroke::NONE, Color32::WHITE)
    } else if enabled && response.hovered() {
        (
            visuals.widgets.hovered.weak_bg_fill,
            visuals.widgets.noninteractive.bg_stroke,
            visuals.text_color(),
        )
    } else {
        (
            visuals.widgets.inactive.weak_bg_fill,
            visuals.widgets.noninteractive.bg_stroke,
            if enabled { visuals.text_color() } else { visuals.weak_text_color() },
        )
    };
    ui.painter().rect_filled(rect, 6, fill);
    ui.painter().rect_stroke(rect, 6, stroke, egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, *value, label));
    response.on_hover_text(tooltip)
}

/// Short vertical rule between groups of controls in a horizontal bar.
pub fn divider(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(13.0, 20.0), egui::Sense::hover());
    ui.painter()
        .vline(rect.center().x, rect.y_range(), ui.visuals().widgets.noninteractive.bg_stroke);
}

/// Title row of a sidebar section with optional trailing controls, added right to left.
pub fn section_header(ui: &mut egui::Ui, title: &str, trailing: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 28.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.label(icy_engine_gui::egui::appearance::bold(ui, title).size(13.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            trailing(ui);
        });
    });
}

/// Small frameless status bar button, e.g. for the ICE and aspect ratio toggles.
pub fn status_button(ui: &mut egui::Ui, label: &str, tooltip: &str) -> Response {
    let font = egui::FontId::proportional(12.0);
    let galley = ui.fonts_mut(|fonts| fonts.layout_no_wrap(label.to_owned(), font, Color32::PLACEHOLDER));
    let (rect, response) = ui.allocate_exact_size(egui::vec2(galley.size().x + 14.0, 22.0), egui::Sense::click());
    let visuals = ui.visuals();
    if ui.is_enabled() && (response.hovered() || response.is_pointer_button_down_on()) {
        ui.painter().rect_filled(rect, 4, visuals.widgets.hovered.weak_bg_fill);
    }
    let color = if response.hovered() {
        visuals.text_color()
    } else {
        visuals.weak_text_color()
    };
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label));
    response.on_hover_text(tooltip)
}

pub fn fkey(ui: &mut egui::Ui, font: &icy_engine::BitFont, code: char, index: usize) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(30.0, 40.0), egui::Sense::click());
    let visuals = ui.visuals().clone();
    if ui.is_enabled() && response.hovered() {
        ui.painter().rect_filled(rect, 5, visuals.widgets.hovered.weak_bg_fill);
    }
    let glyph_rect = egui::Rect::from_center_size(rect.center_top() + egui::vec2(0.0, 13.0), egui::Vec2::splat(24.0));
    let glyph_response = ui
        .scope_builder(egui::UiBuilder::new().max_rect(glyph_rect), |ui| glyph(ui, font, code, false, 24.0))
        .inner;
    let label = format!("F{}", index + 1);
    ui.painter().text(
        rect.center_bottom() - egui::vec2(0.0, 1.0),
        egui::Align2::CENTER_BOTTOM,
        &label,
        egui::FontId::proportional(10.0),
        visuals.weak_text_color(),
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
    ui.painter().rect_filled(rect, 3, fill);
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

pub fn colored_glyph(ui: &mut egui::Ui, font: &icy_engine::BitFont, code: char, foreground: Color32, background: Color32, size: f32) -> Response {
    let old_extreme = ui.visuals().extreme_bg_color;
    let old_text = ui.visuals().override_text_color;
    ui.visuals_mut().extreme_bg_color = background;
    ui.visuals_mut().override_text_color = Some(foreground);
    let response = ui.add_enabled_ui(false, |ui| glyph(ui, font, code, false, size)).inner;
    ui.visuals_mut().extreme_bg_color = old_extreme;
    ui.visuals_mut().override_text_color = old_text;
    response
}

struct GlyphAtlas {
    font: icy_engine::BitFont,
    texture: egui::TextureHandle,
}
