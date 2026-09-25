use eframe::egui;
use icy_engine_gui::egui::appearance;

use super::{
    app::MailApp,
    widgets::{self, Icon},
};

impl MailApp {
    /// Start page while no packet is open.
    pub fn welcome(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        if let Some(path) = &self.loading {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            ui.vertical_centered(|ui| {
                ui.add_space((ui.available_height() / 2.0 - 30.0).max(8.0));
                ui.spinner();
                ui.add_space(6.0);
                ui.label(egui::RichText::new(format!("Opening {name}\u{2026}")).weak());
            });
            return;
        }
        let recent = self.recent.as_ref().map(|recent| recent.packets.clone()).unwrap_or_default();
        let mut remove = None;
        let mut open = None;
        egui::ScrollArea::vertical().id_salt("welcome").auto_shrink([false, false]).show(ui, |ui| {
            let width = ui.available_width().min(460.0);
            let content_height = 250.0 + if recent.is_empty() { 0.0 } else { 40.0 + recent.len() as f32 * 44.0 };
            ui.add_space(((ui.available_height() - content_height) / 2.0).max(16.0));
            ui.vertical_centered(|ui| {
                ui.set_max_width(width);
                let accent = widgets::accent(ui);
                ui.add(self.icons.image(&context, Icon::Mailbox, 56.0).tint(accent));
                ui.add_space(6.0);
                ui.label(appearance::bold(ui, "Icy Mail").size(24.0));
                ui.label(egui::RichText::new("Read and answer your BBS mail offline.").weak());
                ui.add_space(18.0);
                let button = appearance::primary_button("Open Packet\u{2026}").min_size(egui::vec2(180.0, 34.0));
                if ui
                    .add_enabled(!self.loader.picking, button)
                    .on_hover_text("Open a QWK packet (Ctrl+O)")
                    .clicked()
                {
                    self.loader.pick(&context);
                }
                ui.add_space(6.0);
                ui.label(egui::RichText::new("or drop a .QWK packet onto this window").weak().size(12.0));
                if recent.is_empty() {
                    return;
                }
                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("RECENT PACKETS").size(11.0).color(ui.visuals().weak_text_color()));
                });
                ui.add_space(2.0);
                for path in &recent {
                    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    let folder = path.parent().map(|parent| parent.display().to_string()).unwrap_or_default();
                    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 42.0), egui::Sense::click());
                    let hovered = response.hovered();
                    if hovered {
                        ui.painter().rect_filled(rect, 6.0, ui.visuals().widgets.hovered.weak_bg_fill);
                    }
                    let icon = egui::Rect::from_center_size(egui::pos2(rect.left() + 20.0, rect.center().y), egui::Vec2::splat(20.0));
                    self.icons.paint(ui, Icon::Inbox, icon, ui.visuals().weak_text_color());
                    let text = egui::Rect::from_min_max(
                        egui::pos2(rect.left() + 40.0, rect.top() + 3.0),
                        egui::pos2(rect.right() - 36.0, rect.bottom() - 3.0),
                    );
                    let (top, bottom) = text.split_top_bottom_at_fraction(0.5);
                    widgets::paint_text(
                        ui,
                        top,
                        &name,
                        egui::FontId::new(13.5, appearance::bold_family(ui)),
                        ui.visuals().text_color(),
                        egui::Align::Min,
                    );
                    widgets::paint_text(
                        ui,
                        bottom,
                        &folder,
                        egui::FontId::proportional(11.5),
                        ui.visuals().weak_text_color(),
                        egui::Align::Min,
                    );
                    let close = egui::Rect::from_center_size(egui::pos2(rect.right() - 18.0, rect.center().y), egui::vec2(26.0, 26.0));
                    let close_response = ui.interact(close, ui.id().with(("forget", path)), egui::Sense::click());
                    if hovered || close_response.hovered() {
                        if close_response.hovered() {
                            ui.painter().rect_filled(close, 4.0, ui.visuals().widgets.active.weak_bg_fill);
                        }
                        self.icons.paint(ui, Icon::Close, close.shrink(6.0), ui.visuals().text_color());
                    }
                    if close_response.on_hover_text("Remove from the list").clicked() {
                        remove = Some(path.clone());
                    } else if response.on_hover_text(path.display().to_string()).clicked() {
                        open = Some(path.clone());
                    }
                }
            });
        });
        if let Some(path) = remove {
            if let Some(recent) = &mut self.recent {
                let _ = recent.remove(&path);
            }
        }
        if let Some(path) = open {
            self.open(path, &context);
        }
    }
}
