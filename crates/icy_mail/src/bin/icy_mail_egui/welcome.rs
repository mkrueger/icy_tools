use eframe::egui;
use i18n_embed_fl::fl;
use icy_engine::{FileFormat, Rectangle, RenderOptions, TextPane};
use icy_engine_gui::egui::appearance;
use std::path::Path;

use icy_mail::{state::PacketSummary, LANGUAGE_LOADER};

use super::{
    app::MailApp,
    list,
    widgets::{self, Icon},
};

/// Widest the start page grows; wider windows center it.
const PAGE_WIDTH: f32 = 560.0;
const DROP_HEIGHT: f32 = 132.0;
/// Card height with and without a stored summary of the packet.
const CARD_HEIGHT: f32 = 76.0;
const PLAIN_CARD_HEIGHT: f32 = 56.0;
const CARD_GAP: f32 = 8.0;

impl MailApp {
    /// Start page while no packet is open: a drop zone and cards for the recent packets.
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
        let (recent, summaries) = self
            .recent
            .as_ref()
            .map(|recent| (recent.packets.clone(), recent.summaries.clone()))
            .unwrap_or_default();
        let dragging = context.input(|input| !input.raw.hovered_files.is_empty());
        let height_id = egui::Id::new("welcome-content-height");
        let mut remove = None;
        let mut open = None;
        egui::ScrollArea::vertical().id_salt("welcome").auto_shrink([false, false]).show(ui, |ui| {
            let width = ui.available_width().min(PAGE_WIDTH);
            // Centered with the height measured in the previous frame.
            let previous = context.data(|data| data.get_temp::<f32>(height_id)).unwrap_or(0.0);
            ui.add_space(((ui.available_height() - previous) / 2.0).max(16.0));
            let top = ui.cursor().top();
            ui.vertical_centered(|ui| {
                ui.set_max_width(width);
                if !logo(ui) {
                    ui.label(appearance::bold(ui, "Icy Mail").size(24.0));
                }
                ui.add_space(10.0);
                ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "welcome-tagline")).weak());
                ui.add_space(20.0);
                self.drop_zone(ui, width, dragging);
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
                ui.add_space(4.0);
                let today = list::today();
                for path in &recent {
                    let summary = summaries.iter().find(|summary| summary.path == *path);
                    match self.packet_card(ui, width, path, summary, today) {
                        Some(CardAction::Open) => open = Some(path.clone()),
                        Some(CardAction::Forget) => remove = Some(path.clone()),
                        None => {}
                    }
                    ui.add_space(CARD_GAP);
                }
            });
            let height = ui.cursor().top() - top;
            if (height - previous).abs() > 0.5 {
                context.data_mut(|data| data.insert_temp(height_id, height));
                context.request_repaint();
            }
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

    /// Dashed area inviting to drop a packet, with the button to pick one; highlighted while files are dragged over the window.
    fn drop_zone(&mut self, ui: &mut egui::Ui, width: f32, dragging: bool) {
        let context = ui.ctx().clone();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, DROP_HEIGHT), egui::Sense::hover());
        let accent = widgets::accent(ui);
        let (fill, stroke) = if dragging {
            (accent.gamma_multiply(0.14), accent)
        } else {
            (ui.visuals().faint_bg_color, ui.visuals().weak_text_color().gamma_multiply(0.6))
        };
        ui.painter().rect_filled(rect, 10.0, fill);
        let edge = rect.shrink(0.5);
        let corners = [edge.left_top(), edge.right_top(), edge.right_bottom(), edge.left_bottom(), edge.left_top()];
        ui.painter()
            .extend(egui::Shape::dashed_line(&corners, egui::Stroke::new(1.2, stroke), 6.0, 4.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(12.0)), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(4.0);
                let tint = if dragging { accent } else { ui.visuals().weak_text_color() };
                ui.add(self.icons.image(&context, Icon::Open, 28.0).tint(tint));
                ui.add_space(4.0);
                let title = if dragging {
                    fl!(LANGUAGE_LOADER, "welcome-drop-release")
                } else {
                    fl!(LANGUAGE_LOADER, "welcome-drop-title")
                };
                ui.label(appearance::bold(ui, title).size(14.0));
                ui.add_space(8.0);
                let button = appearance::primary_button(fl!(LANGUAGE_LOADER, "welcome-open-packet")).min_size(egui::vec2(160.0, 30.0));
                if ui
                    .add_enabled(!self.loader.picking && !dragging, button)
                    .on_hover_text(fl!(LANGUAGE_LOADER, "welcome-open-packet-tooltip"))
                    .clicked()
                {
                    self.loader.pick(&context);
                }
            });
        });
    }

    /// One recent packet: the BBS with its unread count, the file and when it was packed, and what is waiting in it.
    fn packet_card(&mut self, ui: &mut egui::Ui, width: f32, path: &Path, summary: Option<&PacketSummary>, today: chrono::NaiveDate) -> Option<CardAction> {
        let height = if summary.is_some() { CARD_HEIGHT } else { PLAIN_CARD_HEIGHT };
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
        let visuals = ui.visuals().clone();
        let exists = path.is_file();
        let hovered = response.hovered();
        let fill = if hovered {
            visuals.widgets.hovered.weak_bg_fill
        } else {
            visuals.faint_bg_color
        };
        ui.painter()
            .rect(rect, 8.0, fill, visuals.widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
        let file = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let title = summary
            .map(|summary| summary.bbs_name.trim())
            .filter(|name| !name.is_empty())
            .unwrap_or(&file)
            .to_string();
        let dim = |color: egui::Color32| if exists { color } else { color.gamma_multiply(0.55) };
        let avatar = egui::Rect::from_center_size(egui::pos2(rect.left() + 30.0, rect.center().y), egui::Vec2::splat(36.0));
        widgets::paint_avatar(ui, avatar, &title);
        if !exists {
            ui.painter().circle_filled(avatar.center(), 18.0, fill.gamma_multiply(0.5));
        }

        let close = egui::Rect::from_center_size(egui::pos2(rect.right() - 20.0, rect.center().y), egui::vec2(26.0, 26.0));
        let close_response = ui.interact(close, ui.id().with(("forget", path)), egui::Sense::click());
        let text_left = rect.left() + 60.0;
        let text_right = rect.right() - 44.0;
        let lines = if summary.is_some() { 3 } else { 2 };
        let line_height = 19.0;
        let first = rect.center().y - line_height * (lines as f32 - 1.0) / 2.0;
        let line = |index: usize, right: f32| {
            let center = first + index as f32 * line_height;
            egui::Rect::from_min_max(egui::pos2(text_left, center - line_height / 2.0), egui::pos2(right, center + line_height / 2.0))
        };

        // Unread count on the right of the title line.
        let mut title_right = text_right;
        if let Some(summary) = summary.filter(|_| exists) {
            let (text, filled) = if summary.unread > 0 {
                (fl!(LANGUAGE_LOADER, "welcome-recent-unread", count = summary.unread), true)
            } else {
                (fl!(LANGUAGE_LOADER, "welcome-recent-all-read"), false)
            };
            let color = if filled { egui::Color32::WHITE } else { visuals.weak_text_color() };
            let galley = ui.painter().layout_no_wrap(text, egui::FontId::proportional(11.5), color);
            let pill = egui::Rect::from_min_size(
                egui::pos2(text_right - galley.size().x - 16.0, line(0, text_right).center().y - 9.0),
                egui::vec2(galley.size().x + 16.0, 18.0),
            );
            if filled {
                ui.painter().rect_filled(pill, 9.0, widgets::accent(ui));
            }
            ui.painter().galley(pill.center() - galley.size() / 2.0, galley, color);
            title_right = pill.left() - 8.0;
        }
        widgets::paint_text(
            ui,
            line(0, title_right),
            &title,
            egui::FontId::new(14.0, appearance::bold_family(ui)),
            dim(visuals.text_color()),
            egui::Align::Min,
        );

        let weak = dim(visuals.weak_text_color());
        let small = egui::FontId::proportional(12.0);
        let details = if !exists {
            fl!(LANGUAGE_LOADER, "welcome-recent-missing", file = file.as_str())
        } else if let Some(summary) = summary {
            let mut parts = vec![file.clone()];
            if let Ok(metadata) = std::fs::metadata(path) {
                parts.push(format_size(metadata.len()));
            }
            if let Some(created) = created_date(&summary.created, today) {
                parts.push(fl!(LANGUAGE_LOADER, "welcome-recent-created", date = created));
            }
            parts.join("  \u{00b7}  ")
        } else {
            let folder = path.parent().map(|parent| parent.display().to_string()).unwrap_or_default();
            let size = std::fs::metadata(path).map(|metadata| format_size(metadata.len())).unwrap_or_default();
            [file.clone(), size, folder]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("  \u{00b7}  ")
        };
        let details_color = if exists { weak } else { widgets::warning(ui) };
        widgets::paint_text(ui, line(1, text_right), &details, small.clone(), details_color, egui::Align::Min);

        if let Some(summary) = summary.filter(|_| exists) {
            let mut job = egui::text::LayoutJob::default();
            let format = |color| egui::TextFormat {
                font_id: small.clone(),
                color,
                ..Default::default()
            };
            job.append(&fl!(LANGUAGE_LOADER, "welcome-recent-messages", count = summary.messages), 0.0, format(weak));
            if summary.starred > 0 {
                job.append("  \u{00b7}  ", 0.0, format(weak));
                job.append("\u{2605} ", 0.0, format(widgets::star(ui)));
                job.append(&fl!(LANGUAGE_LOADER, "welcome-recent-starred", count = summary.starred), 0.0, format(weak));
            }
            if summary.drafts > 0 {
                job.append("  \u{00b7}  ", 0.0, format(weak));
                job.append(
                    &fl!(LANGUAGE_LOADER, "welcome-recent-drafts", count = summary.drafts),
                    0.0,
                    format(widgets::accent(ui)),
                );
            }
            job.wrap = egui::text::TextWrapping::truncate_at_width(text_right - text_left);
            let galley = ui.painter().layout_job(job);
            let area = line(2, text_right);
            ui.painter()
                .galley(egui::pos2(area.left(), area.center().y - galley.size().y / 2.0), galley, weak);
        }

        if hovered || close_response.hovered() {
            if close_response.hovered() {
                ui.painter().rect_filled(close, 4.0, visuals.widgets.active.weak_bg_fill);
            }
            self.icons.paint(ui, Icon::Close, close.shrink(6.0), visuals.text_color());
        }
        if close_response.on_hover_text(fl!(LANGUAGE_LOADER, "welcome-forget-recent")).clicked() {
            return Some(CardAction::Forget);
        }
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(path.display().to_string())
            .clicked()
            .then_some(CardAction::Open)
    }
}

enum CardAction {
    Open,
    Forget,
}

/// The packet's creation time as a list date. BBSes write `MM-DD-YYYY,HH:MM:SS`, some with a
/// two-digit year or without the comma; anything else is shown as written.
fn created_date(created: &str, today: chrono::NaiveDate) -> Option<String> {
    let created = created.trim();
    let is_date = |text: &str, year: usize| {
        let bytes = text.as_bytes();
        bytes.len() == 6 + year
            && bytes[2] == b'-'
            && bytes[5] == b'-'
            && bytes.iter().enumerate().all(|(index, byte)| index == 2 || index == 5 || byte.is_ascii_digit())
    };
    let parsed = [(4, "%m-%d-%Y"), (2, "%m-%d-%y")].into_iter().find_map(|(year, format)| {
        let date = created.get(..6 + year).filter(|date| is_date(date, year))?;
        let date = chrono::NaiveDate::parse_from_str(date, format).ok()?;
        let time = created[6 + year..].trim_start_matches([',', ' ']);
        let time = ["%H:%M:%S", "%H:%M"]
            .iter()
            .find_map(|format| chrono::NaiveTime::parse_from_str(time, format).ok())
            .unwrap_or_default();
        Some(date.and_time(time))
    });
    match parsed {
        Some(date) => Some(list::friendly_date(date, &date.format("%Y-%m-%d %H:%M").to_string(), today)),
        None => (!created.is_empty()).then(|| created.to_string()),
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

/// `data/welcome.xb` is the wide variant of `gj-icymail.xb` with a `@` version marker.
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
    fn packet_creation_times_read_like_list_dates() {
        crate::use_english();
        let today = chrono::NaiveDate::from_ymd_opt(2026, 9, 29).unwrap();
        assert_eq!(created_date("09-28-2026,08:15:00", today).as_deref(), Some("Yesterday 08:15"));
        assert_eq!(created_date("01-02-20,10:00:00", today).as_deref(), Some("2020-01-02"));
        assert_eq!(created_date("09-29-202607:05", today).as_deref(), Some("Today 07:05"));
        assert_eq!(created_date(" odd ", today).as_deref(), Some("odd"));
        assert_eq!(created_date("", today), None);
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
