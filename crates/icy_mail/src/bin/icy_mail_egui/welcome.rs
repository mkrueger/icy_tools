use eframe::egui;
use i18n_embed_fl::fl;
use icy_engine::{FileFormat, Rectangle, RenderOptions, TextPane};
use icy_engine_gui::egui::appearance;
use icy_mail::LANGUAGE_LOADER;

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
                ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "welcome-opening", name = name)).weak());
            });
            return;
        }
        let recent = self.recent.as_ref().map(|recent| recent.packets.clone()).unwrap_or_default();
        let mut remove = None;
        let mut open = None;
        egui::ScrollArea::vertical().id_salt("welcome").auto_shrink([false, false]).show(ui, |ui| {
            let width = ui.available_width().min(460.0);
            let content_height = 330.0 + if recent.is_empty() { 0.0 } else { 40.0 + recent.len() as f32 * 44.0 };
            ui.add_space(((ui.available_height() - content_height) / 2.0).max(16.0));
            ui.vertical_centered(|ui| {
                ui.set_max_width(width);
                if !logo(ui) {
                    ui.label(appearance::bold(ui, "Icy Mail").size(24.0));
                }
                ui.add_space(10.0);
                ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "welcome-tagline")).weak());
                ui.add_space(18.0);
                let button = appearance::primary_button(fl!(LANGUAGE_LOADER, "welcome-open-packet")).min_size(egui::vec2(180.0, 34.0));
                if ui
                    .add_enabled(!self.loader.picking, button)
                    .on_hover_text(fl!(LANGUAGE_LOADER, "welcome-open-packet-tooltip"))
                    .clicked()
                {
                    self.loader.pick(&context);
                }
                ui.add_space(6.0);
                ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "welcome-drop-hint")).weak().size(12.0));
                if recent.is_empty() {
                    return;
                }
                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(fl!(LANGUAGE_LOADER, "welcome-recent-packets").to_uppercase())
                            .size(11.0)
                            .color(ui.visuals().weak_text_color()),
                    );
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
                    let size = std::fs::metadata(path).ok().filter(|metadata| metadata.is_file()).map(|metadata| format_size(metadata.len()));
                    let size_font = egui::FontId::proportional(11.5);
                    let size_width = size.as_ref().map_or(0.0, |size| {
                        ui.painter().layout_no_wrap(size.clone(), size_font.clone(), ui.visuals().weak_text_color()).size().x + 10.0
                    });
                    if let Some(size) = &size {
                        widgets::paint_text(ui, top, size, size_font, ui.visuals().weak_text_color(), egui::Align::Max);
                    }
                    widgets::paint_text(
                        ui,
                        egui::Rect::from_min_max(top.min, egui::pos2(top.right() - size_width, top.bottom())),
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
                    if close_response.on_hover_text(fl!(LANGUAGE_LOADER, "welcome-forget-recent")).clicked() {
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

/// The Icy Mail logo with the version filled in, on a black screen like Icy View's welcome page.
/// Returns false when the logo could not be rendered.
fn logo(ui: &mut egui::Ui) -> bool {
    let id = egui::Id::new("welcome-logo");
    let texture = ui.ctx().data(|data| data.get_temp::<egui::TextureHandle>(id)).or_else(|| {
        let texture = ui.ctx().load_texture("welcome-logo", render_logo()?, egui::TextureOptions::NEAREST);
        ui.ctx().data_mut(|data| data.insert_temp(id, texture.clone()));
        Some(texture)
    });
    let Some(texture) = texture else {
        return false;
    };
    const PADDING: f32 = 12.0;
    let size = texture.size_vec2();
    let (rect, _) = ui.allocate_exact_size(size + egui::Vec2::splat(PADDING * 2.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 8.0, egui::Color32::BLACK);
    let image = egui::Rect::from_center_size(rect.center(), size);
    ui.painter().image(
        texture.id(),
        image,
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );
    true
}

fn format_size(size: u64) -> String {
    if size >= 1024 * 1024 {
        format!("{:.1} MiB", size as f64 / 1048576.0)
    } else if size >= 1024 {
        format!("{:.1} KiB", size as f64 / 1024.0)
    } else {
        format!("{size} B")
    }
}

/// `data/welcome.xb` is the stacked variant of `gj-icymail.xb` with a `@` version marker.
fn logo_buffer() -> Option<icy_engine::TextBuffer> {
    let mut buffer = FileFormat::XBin
        .from_bytes(include_bytes!("../../../data/welcome.xb"), None)
        .ok()?
        .screen
        .buffer;
    icy_engine_gui::version_helper::replace_version_marker(&mut buffer, &icy_mail::VERSION, None);
    Some(buffer)
}

fn render_logo() -> Option<egui::ColorImage> {
    let buffer = logo_buffer()?;
    let size = buffer.size();
    let options: RenderOptions = Rectangle::from(0, 0, size.width, size.height).into();
    let (pixels, rgba) = buffer.render_to_rgba(&options, false);
    let (width, height) = (pixels.width.max(0) as usize, pixels.height.max(0) as usize);
    (width > 0 && rgba.len() >= width * height * 4).then(|| egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba[..width * height * 4]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_sizes_are_human_readable() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2048), "2.0 KiB");
        assert_eq!(format_size(1536 * 1024), "1.5 MiB");
    }

    #[test]
    fn logo_shows_the_current_version() {
        let buffer = logo_buffer().unwrap();
        let rows: Vec<String> = (0..buffer.height())
            .map(|y| (0..buffer.width()).map(|x| buffer.char_at((x, y).into()).ch).collect())
            .collect();
        let version = format!("v{}", *icy_mail::VERSION);
        assert!(rows.iter().any(|row| row.contains(&version)), "{rows:#?}");
        assert!(!rows.iter().any(|row| row.contains('@')));
        let image = render_logo().unwrap();
        assert_eq!(image.size, [buffer.width() as usize * 8, buffer.height() as usize * 16]);
    }
}
