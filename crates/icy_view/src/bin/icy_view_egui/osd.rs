//! Fading info panel shown when a file opens: title, artist and group, the SAUCE comment and a
//! row of attributes. Hovering keeps it open, the × hides it for the current file.

use super::{
    icons::{Icon, Icons},
    text,
};
use eframe::egui;
use std::time::{Duration, Instant};

const HOLD: Duration = Duration::from_millis(4500);
const FADE: Duration = Duration::from_millis(700);

pub struct Osd {
    opened: Instant,
    active: bool,
}

impl Default for Osd {
    fn default() -> Self {
        Self {
            opened: Instant::now(),
            active: false,
        }
    }
}

#[derive(Default)]
pub struct Info {
    pub title: String,
    pub byline: String,
    pub comment: Vec<String>,
    pub attributes: Vec<String>,
    pub rating: u8,
}

pub enum Action {
    Rate(u8),
    OpenSauce,
}

impl Osd {
    pub fn start(&mut self) {
        self.opened = Instant::now();
        self.active = true;
    }

    pub fn hide(&mut self) {
        self.active = false;
    }

    pub fn is_visible(&self) -> bool {
        self.active && self.opened.elapsed() < HOLD + FADE
    }

    fn opacity(&self) -> f32 {
        let elapsed = self.opened.elapsed();
        if elapsed <= HOLD {
            1.0
        } else {
            1.0 - ((elapsed - HOLD).as_secs_f32() / FADE.as_secs_f32()).clamp(0.0, 1.0)
        }
    }

    pub fn show(&mut self, ui: &egui::Ui, area: egui::Rect, info: &Info, icons: &mut Icons) -> Option<Action> {
        if !self.is_visible() {
            return None;
        }
        let context = ui.ctx().clone();
        let opacity = self.opacity();
        let width = (area.width() - 32.0).clamp(160.0, 460.0);
        let mut action = None;
        let response = egui::Area::new(egui::Id::new("viewer-osd"))
            .order(egui::Order::Middle)
            .pivot(egui::Align2::LEFT_BOTTOM)
            .fixed_pos(area.left_bottom() + egui::vec2(16.0, -16.0))
            .constrain_to(area)
            .show(&context, |ui| {
                ui.multiply_opacity(opacity);
                let visuals = ui.visuals().clone();
                egui::Frame::new()
                    .fill(visuals.window_fill.gamma_multiply(0.94))
                    .stroke(visuals.window_stroke)
                    .corner_radius(10)
                    .shadow(egui::Shadow {
                        offset: [0, 3],
                        blur: 14,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(if visuals.dark_mode { 120 } else { 45 }),
                    })
                    .inner_margin(egui::Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_width(width);
                        ui.spacing_mut().item_spacing.y = 3.0;
                        let palette = super::colors::Sauce::new(visuals.dark_mode);
                        let closed = ui
                            .horizontal(|ui| {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.spacing_mut().button_padding = egui::vec2(2.0, 2.0);
                                    let image = icons.image(ui.ctx(), Icon::Close, 14.0);
                                    let close = ui
                                        .add(super::icons::tool_button(image, false).min_size(egui::vec2(22.0, 22.0)))
                                        .on_hover_text(text("egui-osd-hide"));
                                    let title = egui::RichText::new(&info.title).size(17.0).strong().color(palette.title);
                                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                        if ui
                                            .add(egui::Label::new(title).truncate().sense(egui::Sense::click()))
                                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                                            .on_hover_text(text("cmd-dialog-sauce-action"))
                                            .clicked()
                                        {
                                            action = Some(Action::OpenSauce);
                                        }
                                    });
                                    close.clicked()
                                })
                                .inner
                            })
                            .inner;
                        if closed {
                            self.active = false;
                        }
                        if !info.byline.is_empty() {
                            ui.add(egui::Label::new(egui::RichText::new(&info.byline).size(13.0).color(palette.author)).truncate());
                        }
                        for line in info.comment.iter().take(3) {
                            ui.add(egui::Label::new(egui::RichText::new(line).size(12.0).italics().color(visuals.text_color())).truncate());
                        }
                        ui.add_space(3.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            if let Some(rating) = stars(ui, info.rating, icons) {
                                action = Some(Action::Rate(rating));
                            }
                            ui.add_space(10.0);
                            let attributes = info.attributes.join("  •  ");
                            ui.add(egui::Label::new(egui::RichText::new(attributes).size(12.0).color(palette.date)).truncate());
                        });
                    });
            })
            .response;
        let hovered = context.rect_contains_pointer(response.layer_id, response.rect);
        if hovered {
            self.opened = Instant::now();
        }
        context.request_repaint_after(if self.opened.elapsed() < HOLD {
            HOLD - self.opened.elapsed()
        } else {
            Duration::from_millis(16)
        });
        action
    }
}

/// Five clickable stars; clicking the current rating clears it.
pub fn stars(ui: &mut egui::Ui, rating: u8, icons: &mut Icons) -> Option<u8> {
    let color = super::colors::star(ui.visuals().dark_mode);
    let mut result = None;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(5.0 * 17.0, 16.0), egui::Sense::click());
    let hover = response
        .hover_pos()
        .map(|pointer| (((pointer.x - rect.left()) / 17.0).floor() as u8 + 1).min(5));
    let shown = hover.unwrap_or(rating);
    for index in 0..5u8 {
        let cell = egui::Rect::from_min_size(rect.min + egui::vec2(index as f32 * 17.0, 0.0), egui::Vec2::splat(16.0));
        let filled = index < shown;
        let icon = if filled { Icon::Star } else { Icon::StarOutline };
        let tint = if filled { color } else { ui.visuals().weak_text_color() };
        icons.image(ui.ctx(), icon, 16.0).tint(tint).paint_at(ui, cell);
    }
    if let Some(hover) = hover {
        if response.clicked() {
            result = Some(if hover == rating { 0 } else { hover });
        }
    }
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(text("egui-rate-tooltip"));
    result
}
