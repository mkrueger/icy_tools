use eframe::egui::{self, Color32, Response, Stroke, StrokeKind};
use icy_draw::fl;
use icy_engine_gui::egui::{appearance::PRIMARY, dialog::DANGER};
use std::collections::HashMap;

#[derive(rust_embed::RustEmbed)]
#[folder = "data/icons"]
struct Assets;

#[derive(Default)]
pub struct Icons(HashMap<String, egui::TextureHandle>);

impl Icons {
    /// Whether the icon `name` was drawn at some point.
    #[cfg(test)]
    pub fn loaded(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }

    pub fn button(&mut self, ui: &mut egui::Ui, name: &str, label: &str, selected: bool) -> Response {
        self.button_sized(ui, name, label, selected, 30.0)
    }

    /// Frameless icon button: transparent until hovered, filled with the accent colour when selected.
    pub fn button_sized(&mut self, ui: &mut egui::Ui, name: &str, label: &str, selected: bool, size: f32) -> Response {
        self.paint_button(ui, name, label, selected, false, egui::Vec2::splat(size))
    }

    /// Like [`Self::button_sized`], but wider than high so a row of them fills its width.
    pub fn button_rect(&mut self, ui: &mut egui::Ui, name: &str, label: &str, selected: bool, size: egui::Vec2) -> Response {
        self.paint_button(ui, name, label, selected, false, size)
    }

    /// Icon button drawn with a muted tint unless hovered, for secondary per-row toggles.
    pub fn subtle_button(&mut self, ui: &mut egui::Ui, name: &str, label: &str, muted: bool, size: f32) -> Response {
        self.paint_button(ui, name, label, false, muted, egui::Vec2::splat(size))
    }

    fn paint_button(&mut self, ui: &mut egui::Ui, name: &str, label: &str, selected: bool, muted: bool, size: egui::Vec2) -> Response {
        let icon = (size.min_elem() * 0.56).round();
        let image = self.image(ui, name, icon);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
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

/// Number of TheDraw outline styles.
pub const OUTLINE_STYLES: usize = 19;

/// Visual picker for the 19 TheDraw outline styles.
pub fn outline_style_picker(ui: &mut egui::Ui, current: &mut usize) -> Response {
    let button = ui
        .add_sized([90.0, CONTROL_HEIGHT], egui::Button::new(fl!("outline-style-label", style = (*current + 1))))
        .on_hover_text(fl!("outline-style-choose"));
    egui::Popup::menu(&button).show(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        egui::Grid::new("outline-style-grid").num_columns(5).show(ui, |ui| {
            for style in 0..OUTLINE_STYLES {
                if outline_style_cell(ui, style, *current == style, 0.75, true).clicked() {
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

/// All outline styles as clickable previews filling the available width; returns true when the style changed.
pub fn outline_style_grid(ui: &mut egui::Ui, current: &mut usize) -> bool {
    const COLUMNS: usize = 7;
    const SPACING: f32 = 4.0;
    let font = icy_engine::BitFont::default();
    let pattern_width = font.size().width as f32 * 8.0;
    let cell_width = ((ui.available_width() - SPACING * (COLUMNS - 1) as f32) / COLUMNS as f32).floor();
    let scale = ((cell_width - 8.0) / pattern_width).clamp(0.25, 1.0);
    let mut changed = false;
    ui.spacing_mut().item_spacing = egui::Vec2::splat(SPACING);
    for row in 0..OUTLINE_STYLES.div_ceil(COLUMNS) {
        ui.horizontal(|ui| {
            for style in row * COLUMNS..((row + 1) * COLUMNS).min(OUTLINE_STYLES) {
                if outline_style_cell(ui, style, *current == style, scale, false).clicked() && *current != style {
                    *current = style;
                    changed = true;
                }
            }
        });
    }
    changed
}

fn outline_style_cell(ui: &mut egui::Ui, style: usize, selected: bool, scale: f32, letter: bool) -> Response {
    const COLUMNS: usize = 8;
    const ROWS: usize = 6;
    let font = icy_engine::BitFont::default();
    let dimensions = font.size();
    let glyph_size = egui::vec2(dimensions.width as f32, dimensions.height as f32) * scale;
    let preview_size = egui::vec2(glyph_size.x * COLUMNS as f32, glyph_size.y * ROWS as f32);
    let label_width = if letter { 20.0 } else { 0.0 };
    let (rect, response) = ui.allocate_exact_size(preview_size + egui::vec2(8.0 + label_width, 8.0), egui::Sense::click());
    let visuals = ui.visuals().clone();
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
    if letter {
        ui.painter().text(
            egui::pos2(rect.left() + 8.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            char::from_u32(b'A' as u32 + style as u32).unwrap_or('?'),
            egui::FontId::monospace(11.0),
            visuals.weak_text_color(),
        );
    }
    let texture = outline_style_texture(ui.ctx(), &font, style);
    let target = egui::Rect::from_min_size(egui::pos2(rect.left() + 4.0 + label_width, rect.top() + 4.0), preview_size);
    ui.painter().image(
        texture.id(),
        target,
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        visuals.strong_text_color(),
    );
    response.on_hover_text(fl!(
        "outline-style-tooltip",
        style = (style + 1),
        key = char::from_u32(b'A' as u32 + style as u32).unwrap_or('?').to_string()
    ))
}

/// Cached white-on-transparent rendering of the outline sample pattern in `style`, filtered linearly
/// so the downscaled previews keep their thin lines.
fn outline_style_texture(context: &egui::Context, font: &icy_engine::BitFont, style: usize) -> egui::TextureHandle {
    const COLUMNS: usize = 8;
    const ROWS: usize = 6;
    let key = egui::Id::new(("outline-style-texture", style));
    if let Some(texture) = context.data(|data| data.get_temp::<egui::TextureHandle>(key)) {
        return texture;
    }
    let width = font.size().width.max(1) as usize;
    let height = font.size().height.max(1) as usize;
    let mut image = egui::ColorImage::filled([width * COLUMNS, height * ROWS], Color32::TRANSPARENT);
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            let character = outline_result(style, OUTLINE_PATTERN[column + row * COLUMNS]);
            for (y, pixels) in font.glyph(character).to_bitmap_pixels().iter().take(height).enumerate() {
                for (x, &enabled) in pixels.iter().take(width).enumerate() {
                    if enabled {
                        image[(column * width + x, row * height + y)] = Color32::WHITE;
                    }
                }
            }
        }
    }
    let options = egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear));
    let texture = context.load_texture(format!("outline-style-{style}"), image, options);
    context.data_mut(|data| data.insert_temp(key, texture.clone()));
    texture
}

/// CP437 character an outline placeholder code (`A`..`Q`) becomes in `style`.
pub fn outline_result(style: usize, code: u8) -> char {
    let unicode = retrofont::transform_outline(style, code);
    codepages::tables::UNICODE_TO_CP437.get(&unicode).copied().map(char::from).unwrap_or(unicode)
}

/// Row captions of the outline cheat sheet (key, placeholder code, result).
pub fn outline_sheet_captions(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(44.0, OUTLINE_KEY_HEIGHT), egui::Sense::hover());
    let color = ui.visuals().weak_text_color();
    let font = egui::FontId::proportional(10.0);
    for (row, label) in [
        fl!("tdf-editor-cheat_sheet_key"),
        fl!("tdf-editor-cheat_sheet_code"),
        fl!("tdf-editor-cheat_sheet_res"),
    ]
    .into_iter()
    .enumerate()
    {
        ui.painter().text(
            egui::pos2(rect.right() - 2.0, rect.top() + OUTLINE_ROWS[row]),
            egui::Align2::RIGHT_CENTER,
            format!("{label}:"),
            font.clone(),
            color,
        );
    }
}

const OUTLINE_KEY_HEIGHT: f32 = 42.0;
/// Vertical centers of the key, code and result rows inside an outline cheat sheet column.
const OUTLINE_ROWS: [f32; 3] = [5.0, 16.0, 32.0];

/// One outline cheat sheet column: the key, the placeholder code it types and the resulting character.
pub fn outline_key(ui: &mut egui::Ui, font: &icy_engine::BitFont, key: &str, code: &str, result: Option<char>) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(26.0, OUTLINE_KEY_HEIGHT), egui::Sense::click());
    let visuals = ui.visuals().clone();
    if ui.is_enabled() && response.hovered() {
        ui.painter().rect_filled(rect, 5, visuals.widgets.hovered.weak_bg_fill);
    }
    let center = rect.center().x;
    ui.painter().text(
        egui::pos2(center, rect.top() + OUTLINE_ROWS[0]),
        egui::Align2::CENTER_CENTER,
        key,
        egui::FontId::proportional(10.0),
        visuals.weak_text_color(),
    );
    ui.painter().text(
        egui::pos2(center, rect.top() + OUTLINE_ROWS[1]),
        egui::Align2::CENTER_CENTER,
        code,
        egui::FontId::monospace(11.0),
        visuals.text_color(),
    );
    let result_center = egui::pos2(center, rect.top() + OUTLINE_ROWS[2]);
    match result {
        Some(character) if character != ' ' => {
            let dimensions = font.size();
            let scale = (16.0 / dimensions.height.max(1) as f32).min(1.0);
            let size = egui::vec2(dimensions.width as f32, dimensions.height as f32) * scale;
            paint_glyph(
                ui,
                font,
                character,
                egui::Rect::from_center_size(result_center, size),
                visuals.strong_text_color(),
            );
        }
        _ => {
            ui.painter().text(
                result_center,
                egui::Align2::CENTER_CENTER,
                "SP",
                egui::FontId::proportional(9.0),
                visuals.weak_text_color(),
            );
        }
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), key));
    response
}

/// Pill segmented control: an inset track with the current option filled with the accent colour.
/// Each option is `(value, label, tooltip)`. Returns true when the selection changed.
pub fn segmented<T: PartialEq + Copy, S: AsRef<str>>(ui: &mut egui::Ui, current: &mut T, options: &[(T, S, S)]) -> bool {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galleys: Vec<_> = options
        .iter()
        .map(|(_, label, _)| ui.fonts_mut(|fonts| fonts.layout_no_wrap(label.as_ref().to_owned(), font.clone(), Color32::PLACEHOLDER)))
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
        let response = ui.interact(rect, ui.id().with(("segment", label.as_ref())), egui::Sense::click());
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
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, selected, label.as_ref()));
        let response = if tooltip.as_ref().is_empty() {
            response
        } else {
            response.on_hover_text(tooltip.as_ref())
        };
        if response.clicked() && !selected {
            *current = *value;
            changed = true;
        }
    }
    changed
}

/// [`segmented`] control that fills the available width with equally wide options, wrapping
/// into rows of `columns`. Returns true when the selection changed.
pub fn segmented_grid<T: PartialEq + Copy, S: AsRef<str>>(ui: &mut egui::Ui, current: &mut T, options: &[(T, S, S)], columns: usize) -> bool {
    let columns = columns.clamp(1, options.len().max(1));
    let rows = options.len().div_ceil(columns);
    let inset = 2.0;
    let cell_height = CONTROL_HEIGHT - inset * 2.0;
    let (track, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), cell_height * rows as f32 + inset * 2.0),
        egui::Sense::hover(),
    );
    let visuals = ui.visuals().clone();
    ui.painter().rect_filled(track, 7, visuals.extreme_bg_color);
    ui.painter()
        .rect_stroke(track, 7, visuals.widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
    let font = egui::TextStyle::Button.resolve(ui.style());
    let cell_width = (track.width() - inset * 2.0) / columns as f32;
    let enabled = ui.is_enabled();
    let mut changed = false;
    for (index, (value, label, tooltip)) in options.iter().enumerate() {
        let min = track.min + egui::vec2(inset + (index % columns) as f32 * cell_width, inset + (index / columns) as f32 * cell_height);
        let rect = egui::Rect::from_min_size(min, egui::vec2(cell_width, cell_height));
        let response = ui.interact(rect, ui.id().with(("segment", label.as_ref())), egui::Sense::click());
        let selected = *current == *value;
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
        let galley = ui.fonts_mut(|fonts| fonts.layout_no_wrap(label.as_ref().to_owned(), font.clone(), color));
        ui.painter()
            .with_clip_rect(rect.shrink(2.0))
            .galley(rect.center() - galley.size() / 2.0, galley, color);
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, selected, label.as_ref()));
        let response = if tooltip.as_ref().is_empty() {
            response
        } else {
            response.on_hover_text(tooltip.as_ref())
        };
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
    let target = egui::Rect::from_min_size(origin, egui::vec2(dimensions.width as f32, dimensions.height as f32) * scale);
    paint_glyph(ui, font, code, target, ui.visuals().text_color());
    let code = (code as u32).min(255);
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

/// Paints `code` of `font` into `target` using a cached 16×16 glyph atlas texture.
pub fn paint_glyph(ui: &egui::Ui, font: &icy_engine::BitFont, code: char, target: egui::Rect, color: Color32) {
    paint_glyph_on(ui.painter(), font, code, target, color);
}

/// [`paint_glyph`] with a given painter, e.g. one clipped to the canvas.
pub fn paint_glyph_on(painter: &egui::Painter, font: &icy_engine::BitFont, code: char, target: egui::Rect, color: Color32) {
    let context = painter.ctx();
    let dimensions = font.size();
    // Keyed per font so previews in the default font and buffer glyphs do not evict each other.
    let key = egui::Id::new(("glyph-atlas", font.name().to_string(), dimensions.width, dimensions.height));
    let existing = context.data(|data| data.get_temp::<std::sync::Arc<GlyphAtlas>>(key));
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
            texture: context.load_texture("glyph-atlas", image, egui::TextureOptions::NEAREST),
        });
        context.data_mut(|data| data.insert_temp(key, atlas.clone()));
        atlas
    };
    let code = (code as u32).min(255);
    let uv = egui::Rect::from_min_size(egui::pos2((code % 16) as f32 / 16.0, (code / 16) as f32 / 16.0), egui::Vec2::splat(1.0 / 16.0));
    painter.image(atlas.texture.id(), target, uv, color);
}

/// The size of the square buttons of the transport bars.
pub const TRANSPORT_BUTTON: f32 = 30.0;
/// The size of the round play button between them.
pub const PLAY_BUTTON: f32 = 40.0;
pub const PLAY: Color32 = Color32::from_rgb(46, 160, 67);

/// Framed square transport button, filled with the accent color while `active`.
pub fn transport_button(icons: &mut Icons, ui: &mut egui::Ui, icon: &str, tooltip: &str, enabled: bool, active: bool) -> egui::Response {
    let image = icons.image(ui, icon, 18.0);
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(TRANSPORT_BUTTON), sense);
    let visuals = ui.visuals();
    let (fill, stroke, tint) = if active && enabled {
        (PRIMARY, PRIMARY, Color32::WHITE)
    } else if !enabled {
        (
            visuals.widgets.inactive.weak_bg_fill.gamma_multiply(0.5),
            visuals.widgets.noninteractive.bg_stroke.color,
            visuals.weak_text_color().gamma_multiply(0.5),
        )
    } else if response.is_pointer_button_down_on() {
        (
            visuals.widgets.active.weak_bg_fill,
            visuals.widgets.active.bg_stroke.color,
            visuals.text_color(),
        )
    } else if response.hovered() {
        (
            visuals.widgets.hovered.weak_bg_fill,
            visuals.widgets.hovered.bg_stroke.color,
            visuals.text_color(),
        )
    } else {
        (
            visuals.widgets.inactive.weak_bg_fill,
            visuals.widgets.noninteractive.bg_stroke.color,
            visuals.text_color(),
        )
    };
    ui.painter().rect(rect, 6, fill, Stroke::new(1.0, stroke), StrokeKind::Inside);
    image
        .tint(tint)
        .paint_at(ui, egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(18.0)));
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, active, tooltip));
    response.on_hover_text(tooltip)
}

/// Round play/pause button: green to start playback, red while playing.
pub fn play_button(icons: &mut Icons, ui: &mut egui::Ui, playing: bool, enabled: bool, label: &str) -> egui::Response {
    let image = icons.image(ui, if playing { "pause" } else { "play" }, 22.0);
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(PLAY_BUTTON), sense);
    let base = if !enabled {
        ui.visuals().widgets.inactive.weak_bg_fill
    } else if playing {
        DANGER
    } else {
        PLAY
    };
    let fill = if enabled && response.hovered() { base.gamma_multiply(1.15) } else { base };
    let painter = ui.painter();
    painter.circle_filled(rect.center() + egui::vec2(0.0, 2.0), PLAY_BUTTON / 2.0, Color32::from_black_alpha(60));
    painter.circle_filled(rect.center(), PLAY_BUTTON / 2.0, fill);
    let tint = if enabled { Color32::WHITE } else { ui.visuals().weak_text_color() };
    // The play triangle looks centred when nudged right a little.
    let offset = if playing { 0.0 } else { 1.5 };
    image.tint(tint).paint_at(
        ui,
        egui::Rect::from_center_size(rect.center() + egui::vec2(offset, 0.0), egui::Vec2::splat(22.0)),
    );
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, playing, label));
    response.on_hover_text(label)
}

/// Marks the corners placed so far of a polyline or polygon being drawn.
pub fn paint_vertices(ui: &egui::Ui, points: impl IntoIterator<Item = egui::Pos2>, accent: Color32) {
    for point in points {
        let handle = egui::Rect::from_center_size(point, egui::Vec2::splat(7.0));
        ui.painter().rect_filled(handle, 1.0, Color32::WHITE);
        ui.painter().rect_stroke(handle, 1.0, Stroke::new(1.5, accent), egui::StrokeKind::Middle);
    }
}
