use crate::{Message, SWAP_SVG};
use eframe::egui::{self, Sense};
use eframe::epaint::{Color32, Pos2, Rect, Stroke, Vec2};
use egui::CornerRadius;
use icy_engine::{Palette, TextAttribute};
use std::cmp::min;

pub fn palette_switcher(_ctx: &egui::Context, ui: &mut egui::Ui, caret_attr: &TextAttribute, palette: &Palette) -> Option<Message> {
    let mut result = None;

    let height = 62.0;
    let (id, rect) = ui.allocate_space(Vec2::new(height, height));
    let response = ui.interact(rect, id, Sense::click());
    let painter = ui.painter_at(rect);

    let rect_height = height * 0.618;

    painter.rect_filled(
        Rect::from_min_size(
            Pos2::new(height - rect_height, height - rect_height) + rect.left_top().to_vec2(),
            Vec2::new(rect_height, rect_height),
        ),
        CornerRadius::ZERO,
        Color32::BLACK,
    );

    painter.rect_filled(
        Rect::from_min_size(
            Pos2::new(height - rect_height + 1., height - rect_height + 1.) + rect.left_top().to_vec2(),
            Vec2::new(rect_height - 2., rect_height - 2.),
        ),
        CornerRadius::ZERO,
        Color32::WHITE,
    );

    let (r, g, b) = palette.rgb(caret_attr.background());
    painter.rect_filled(
        Rect::from_min_size(
            Pos2::new(height - rect_height + 2., height - rect_height + 2.) + rect.left_top().to_vec2(),
            Vec2::new(rect_height - 4., rect_height - 4.),
        ),
        CornerRadius::ZERO,
        Color32::from_rgb(r, g, b),
    );

    painter.rect_filled(
        Rect::from_min_size(Pos2::new(0., 0.) + rect.left_top().to_vec2(), Vec2::new(rect_height, rect_height)),
        CornerRadius::ZERO,
        Color32::BLACK,
    );

    painter.rect_filled(
        Rect::from_min_size(Pos2::new(1., 1.) + rect.left_top().to_vec2(), Vec2::new(rect_height - 2., rect_height - 2.)),
        CornerRadius::ZERO,
        Color32::WHITE,
    );

    let (r, g, b) = palette.rgb(caret_attr.foreground());
    painter.rect_filled(
        Rect::from_min_size(Pos2::new(2., 2.) + rect.left_top().to_vec2(), Vec2::new(rect_height - 4., rect_height - 4.)),
        CornerRadius::ZERO,
        Color32::from_rgb(r, g, b),
    );

    let s_rect_height = height * 0.382;
    let rh = s_rect_height / 1.8;
    let (r, g, b) = palette.rgb(7);

    let overlap = 2.0;

    painter.rect_filled(
        Rect::from_min_size(Pos2::new(rh - overlap, height - rh - overlap) + rect.left_top().to_vec2(), Vec2::new(rh, rh)),
        CornerRadius::ZERO,
        Color32::from_rgb(r ^ 0xFF, g ^ 0xFF, b ^ 0xFF),
    );

    painter.rect_filled(
        Rect::from_min_size(
            Pos2::new(rh - overlap + 1., height - rh - overlap + 1.) + rect.left_top().to_vec2(),
            Vec2::new(rh - 2., rh - 2.),
        ),
        CornerRadius::ZERO,
        Color32::from_rgb(r, g, b),
    );

    let (r, g, b) = palette.rgb(0);
    painter.rect_filled(
        Rect::from_min_size(
            Pos2::new(overlap, height - 2. * rh + 2. + overlap) + rect.left_top().to_vec2(),
            Vec2::new(rh, rh),
        ),
        CornerRadius::ZERO,
        Color32::from_rgb(r ^ 0xFF, g ^ 0xFF, b ^ 0xFF),
    );

    painter.rect_filled(
        Rect::from_min_size(
            Pos2::new(1. + overlap, height - 2. * rh + 3. + overlap) + rect.left_top().to_vec2(),
            Vec2::new(rh - 2., rh - 2.),
        ),
        CornerRadius::ZERO,
        Color32::from_rgb(r, g, b),
    );
    let mut tex_id = SWAP_SVG.clone();
    tex_id = tex_id.tint(Color32::WHITE);
    tex_id.paint_at(
        ui,
        Rect::from_min_size(
            Pos2::new(rect_height + 1., 0.) + rect.left_top().to_vec2(),
            Vec2::new(s_rect_height, s_rect_height),
        ),
    );

    if let Some(hp) = response.hover_pos() {
        let pos = hp.to_vec2() - rect.left_top().to_vec2();

        if response.clicked() {
            if pos.x > rect_height && pos.y < rect_height {
                result = Some(Message::ToggleColor);
            }

            if pos.x < rect_height && pos.y > rect_height {
                result = Some(Message::SwitchToDefaultColor);
            }
        }
    }
    result
}

pub fn palette_editor_16(
    ui: &mut egui::Ui,
    caret_attr: &TextAttribute,
    palette: &Palette,
    ice_mode: icy_engine::IceMode,
    font_mode: icy_engine::FontMode,
) -> Option<Message> {
    let mut result = None;

    ui.horizontal(|ui| {
        ui.add_space(4.0);
        let right_border = 4.0;
        let items_per_row = if palette.len() < 64 { 8 } else { 16 };

        let upper_limit = (palette.len() as f32 / items_per_row as f32).ceil() as usize * items_per_row;

        let height = (ui.available_width() - right_border) / items_per_row as f32;

        let (id, stroke_rect) = ui.allocate_space(Vec2::new(
            ui.available_width() - right_border,
            height * upper_limit as f32 / items_per_row as f32,
        ));

        let mut response = ui.interact(stroke_rect, id, Sense::click());
        let painter = ui.painter_at(stroke_rect);

        for i in 0..upper_limit {
            let (r, g, b) = palette.rgb(i as u32);
            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(
                        stroke_rect.left() + (i % items_per_row) as f32 * height,
                        stroke_rect.top() + (i / items_per_row) as f32 * height,
                    ),
                    Vec2::new(height, height),
                ),
                CornerRadius::ZERO,
                Color32::from_rgb(r, g, b),
            );
        }

        let marker_len = height / 3.;
        // paint fg marker
        let stroke = Stroke::new(1., Color32::WHITE);
        let origin = Pos2::new(
            stroke_rect.left() + (caret_attr.foreground() % items_per_row as u32) as f32 * height,
            stroke_rect.top() + (caret_attr.foreground() / items_per_row as u32) as f32 * height,
        );
        painter.line_segment([origin, origin + Vec2::new(marker_len, 0.)], stroke);
        painter.line_segment([origin, origin + Vec2::new(0., marker_len)], stroke);
        for i in 0..marker_len as usize {
            painter.line_segment([origin + Vec2::new(i as f32, 0.), origin + Vec2::new(0., i as f32)], stroke);
        }
        let stroke = Stroke::new(1., Color32::GRAY);
        painter.line_segment([origin, origin + Vec2::new(marker_len, 0.)], stroke);
        painter.line_segment([origin, origin + Vec2::new(0., marker_len)], stroke);
        painter.line_segment([origin + Vec2::new(marker_len, 0.), origin + Vec2::new(0., marker_len)], stroke);

        // paint bg marker
        let stroke = Stroke::new(1., Color32::WHITE);
        let origin = Pos2::new(
            stroke_rect.left() + (1 + caret_attr.background() % items_per_row as u32) as f32 * height,
            stroke_rect.top() + (1 + caret_attr.background() / items_per_row as u32) as f32 * height,
        );
        painter.line_segment([origin, origin - Vec2::new(marker_len, 0.)], stroke);
        painter.line_segment([origin, origin - Vec2::new(0., marker_len)], stroke);
        for i in 0..marker_len as usize {
            painter.line_segment([origin - Vec2::new(i as f32, 0.), origin - Vec2::new(0., i as f32)], stroke);
        }
        let stroke = Stroke::new(1., Color32::GRAY);
        painter.line_segment([origin, origin - Vec2::new(marker_len, 0.)], stroke);
        painter.line_segment([origin, origin - Vec2::new(0., marker_len)], stroke);
        painter.line_segment([origin - Vec2::new(marker_len, 0.), origin - Vec2::new(0., marker_len)], stroke);

        if let Some(hp) = response.hover_pos() {
            let pos = (hp.to_vec2() - stroke_rect.left_top().to_vec2()) / Vec2::new(height, height);
            let color = min(palette.len() as u32 - 1, pos.x as u32 + pos.y as u32 * items_per_row as u32);

            if response.hovered() {
                response = response.on_hover_ui(|ui| {
                    let col = palette.color(color);
                    let (r, g, b) = col.rgb();
                    if let Some(title) = &col.name {
                        ui.label(title);
                    }
                    ui.label(format!("#{:02X}{:02X}{:02X}", r, g, b));
                });
            }

            if response.clicked() {
                if color < 8 || font_mode.has_high_fg_colors() || palette.len() > 16 {
                    result = Some(Message::SetForeground(color));
                }
                response.mark_changed();
            }
            if response.secondary_clicked() {
                if color < 8 || ice_mode.has_high_bg_colors() || palette.len() > 16 {
                    result = Some(Message::SetBackground(color));
                }
                response.mark_changed();
            }
        }
    });
    result
}
