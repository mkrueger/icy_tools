//! Start screen shown when Icy Draw is launched without a file, and the document type rows
//! shared with the New dialog.

use super::super::widgets::Icons;
use super::{menus, DrawApp, FileAction, NewKind};
use eframe::egui::{self, Color32};
use icy_draw::fl;
use icy_engine::{FileFormat, Rectangle, RenderOptions, Size, TextPane};
use icy_engine_gui::egui::appearance::{self, PRIMARY};
use std::path::Path;

fn compact_folder(path: &Path, home: Option<&Path>) -> String {
    if let Some(relative) = home.and_then(|home| path.strip_prefix(home).ok()) {
        if relative.as_os_str().is_empty() {
            return "~".to_owned();
        }
        return Path::new("~").join(relative).display().to_string();
    }
    path.display().to_string()
}

/// Canvas size presets offered by the New and Canvas Size dialogs.
pub(super) fn size_presets() -> [(i32, i32, String); 5] {
    [
        (80, 25, fl!("size-preset-standard")),
        (80, 50, fl!("size-preset-vga50")),
        (132, 25, fl!("size-preset-wide")),
        (132, 50, fl!("size-preset-wide50")),
        (40, 25, fl!("size-preset-40columns")),
    ]
}

const TILE_SIZE: egui::Vec2 = egui::vec2(204.0, 60.0);
const TILE_SPACING: f32 = 8.0;
const MAX_COLUMNS: usize = 3;
const TILE_SUBTITLE_ROWS: usize = 2;
const RECENT_ROW_HEIGHT: f32 = 44.0;
const RECENT_GAP: f32 = 32.0;
const RECENT_MIN_WIDTH: f32 = 240.0;
const RECENT_MAX_WIDTH: f32 = 520.0;

/// Selectable row with icon, name and description of a document kind.
pub(super) fn kind_row(icons: &mut Icons, ui: &mut egui::Ui, kind: NewKind, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 44.0), egui::Sense::click());
    let visuals = ui.visuals().clone();
    if selected {
        ui.painter().rect_filled(rect, 6, PRIMARY);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 6, visuals.widgets.hovered.weak_bg_fill);
    }
    let (strong, weak) = if selected {
        (Color32::WHITE, Color32::from_white_alpha(200))
    } else {
        (visuals.strong_text_color(), visuals.weak_text_color())
    };
    let icon = egui::Rect::from_center_size(egui::pos2(rect.left() + 22.0, rect.center().y), egui::Vec2::splat(20.0));
    icons.image(ui, kind.icon(), 20.0).tint(strong).paint_at(ui, icon);
    let painter = ui.painter().with_clip_rect(rect);
    painter.text(
        egui::pos2(rect.left() + 44.0, rect.center().y - 1.0),
        egui::Align2::LEFT_BOTTOM,
        kind.name(),
        egui::FontId::proportional(14.0),
        strong,
    );
    painter.text(
        egui::pos2(rect.left() + 44.0, rect.center().y + 1.0),
        egui::Align2::LEFT_TOP,
        kind.description(),
        egui::FontId::proportional(12.0),
        weak,
    );
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, ui.is_enabled(), selected, kind.name()));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Card with an icon, title and subtitle used on the start screen.
fn tile(icons: &mut Icons, ui: &mut egui::Ui, icon: &str, title: &str, subtitle: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(TILE_SIZE, egui::Sense::click());
    let visuals = ui.visuals().clone();
    let fill = if response.is_pointer_button_down_on() {
        visuals.widgets.active.weak_bg_fill
    } else if response.hovered() {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        visuals.widgets.inactive.weak_bg_fill
    };
    let stroke = if response.hovered() {
        egui::Stroke::new(1.0, PRIMARY)
    } else {
        visuals.widgets.noninteractive.bg_stroke
    };
    ui.painter().rect(rect, 8, fill, stroke, egui::StrokeKind::Inside);
    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.left() + 20.0, rect.center().y), egui::Vec2::splat(18.0));
    icons.image(ui, icon, 18.0).tint(PRIMARY).paint_at(ui, icon_rect);
    let text_left = rect.left() + 38.0;
    let text_width = rect.right() - 8.0 - text_left;
    let layout = |text: &str, size: f32, color: Color32, rows: usize| {
        let mut job = egui::text::LayoutJob::simple(text.to_owned(), egui::FontId::proportional(size), color, text_width);
        job.wrap.max_rows = rows;
        ui.painter().layout_job(job)
    };
    let title_galley = layout(title, 14.0, visuals.strong_text_color(), 1);
    let subtitle_galley = layout(subtitle, 12.0, visuals.weak_text_color(), TILE_SUBTITLE_ROWS);
    let top = rect.center().y - (title_galley.size().y + 2.0 + subtitle_galley.size().y) / 2.0;
    let elided = title_galley.elided || subtitle_galley.elided;
    let subtitle_top = top + title_galley.size().y + 2.0;
    let painter = ui.painter().with_clip_rect(rect.shrink(4.0));
    painter.galley(egui::pos2(text_left, top), title_galley, visuals.strong_text_color());
    painter.galley(egui::pos2(text_left, subtitle_top), subtitle_galley, visuals.weak_text_color());
    let response = if elided {
        response.on_hover_text(format!("{title}\n{subtitle}"))
    } else {
        response
    };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), title));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Document kind (`None` = custom size dialog), icon, title and subtitle of a start screen tile.
type StartTile = (Option<NewKind>, &'static str, String, String);

fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text.to_uppercase()).size(11.0).color(ui.visuals().weak_text_color()));
    ui.add_space(2.0);
}

impl DrawApp {
    pub(super) fn start_screen(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let recent: Vec<_> = self.settings.recent_files.files().into_iter().rev().take(8).collect();
        let groups: Vec<_> = NewKind::groups()
            .into_iter()
            .map(|(title, kinds)| {
                let mut tiles = Vec::new();
                for &kind in kinds {
                    let subtitle = match kind {
                        NewKind::Ansi => fl!("start-ansi-subtitle"),
                        NewKind::TheDraw => fl!("start-tdf-subtitle"),
                        _ => kind.description(),
                    };
                    tiles.push((Some(kind), kind.icon(), kind.name(), subtitle));
                    if kind == NewKind::Ansi {
                        tiles.push((None, "measure", fl!("start-custom"), fl!("start-custom-subtitle")));
                    }
                }
                (title, tiles)
            })
            .collect();
        let mut create = None;
        let mut open = None;
        egui::ScrollArea::vertical().id_salt("start").auto_shrink([false, false]).show(ui, |ui| {
            let available = ui.available_width() - 48.0;
            let columns_for = |space: f32| ((space + TILE_SPACING) / (TILE_SIZE.x + TILE_SPACING)).floor().clamp(1.0, MAX_COLUMNS as f32) as usize;
            let tiles_width = |columns: usize| columns as f32 * TILE_SIZE.x + (columns - 1) as f32 * TILE_SPACING;
            // On wide windows the recent files go into a column next to the tiles instead of below them.
            let side_by_side = !recent.is_empty() && available >= tiles_width(3) + RECENT_GAP + RECENT_MIN_WIDTH;
            let columns = columns_for(if side_by_side { available - RECENT_GAP - RECENT_MIN_WIDTH } else { available });
            let tiles_width = tiles_width(columns);
            let recent_width = if side_by_side {
                (available - tiles_width - RECENT_GAP).min(RECENT_MAX_WIDTH)
            } else {
                tiles_width
            };
            let width = if side_by_side { tiles_width + RECENT_GAP + recent_width } else { tiles_width };

            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
                ui.vertical(|ui| {
                    ui.set_width(width);
                    ui.vertical_centered(|ui| {
                        if !logo(ui) {
                            ui.label(appearance::bold(ui, "Icy Draw").size(26.0));
                        }
                    });
                    ui.add_space(16.0);
                    if side_by_side {
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(tiles_width);
                                self.start_new_section(ui, &context, &groups, columns, &mut create);
                            });
                            ui.add_space(RECENT_GAP - ui.spacing().item_spacing.x);
                            ui.vertical(|ui| {
                                ui.set_width(recent_width);
                                self.start_recent_section(ui, &recent, recent_width, &mut open);
                            });
                        });
                    } else {
                        self.start_new_section(ui, &context, &groups, columns, &mut create);
                        if !recent.is_empty() {
                            ui.add_space(18.0);
                            self.start_recent_section(ui, &recent, width, &mut open);
                        }
                    }
                });
            });
        });
        match create {
            Some(Some(NewKind::TheDraw)) => {
                self.new_kind = NewKind::TheDraw;
                self.request_new();
            }
            Some(Some(kind)) => self.create(kind, Size::new(80, 25)),
            Some(None) => self.request_new(),
            None => {}
        }
        if let Some(path) = open {
            self.open(path);
        }
    }

    fn start_new_section(
        &mut self,
        ui: &mut egui::Ui,
        context: &egui::Context,
        groups: &[(String, Vec<StartTile>)],
        columns: usize,
        create: &mut Option<Option<NewKind>>,
    ) {
        for (index, (title, tiles)) in groups.iter().enumerate() {
            if index > 0 {
                ui.add_space(6.0);
            }
            caption(ui, title);
            for row in tiles.chunks(columns) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = TILE_SPACING;
                    for (kind, icon, title, subtitle) in row {
                        if tile(&mut self.icons, ui, icon, title, subtitle).clicked() {
                            *create = Some(*kind);
                        }
                    }
                });
                ui.add_space(TILE_SPACING - ui.spacing().item_spacing.y);
            }
        }
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            let open_button = appearance::primary_button(fl!("menu-open"))
                .shortcut_text(egui::RichText::new(context.format_shortcut(&menus::OPEN)).color(Color32::from_white_alpha(190)))
                .min_size(egui::vec2(150.0, 30.0));
            if ui.add_enabled(!self.picker, open_button).clicked() {
                self.choose(context, FileAction::Open);
            }
            let connect_button = egui::Button::new(fl!("menu-connect-to-server")).min_size(egui::vec2(150.0, 30.0));
            if ui
                .add_enabled(!self.picker && !self.collab.in_session(), connect_button)
                .on_hover_text(fl!("start-connect-tooltip"))
                .clicked()
            {
                self.open_connect_dialog();
            }
            if ui
                .add_enabled(!self.picker, egui::Button::new(fl!("menu-show_settings")).min_size(egui::vec2(0.0, 30.0)))
                .clicked()
            {
                self.open_settings();
            }
        });
    }

    fn start_recent_section(&mut self, ui: &mut egui::Ui, recent: &[std::path::PathBuf], width: f32, open: &mut Option<std::path::PathBuf>) {
        caption(ui, &fl!("start-recent"));
        let home = directories::UserDirs::new();
        let text_width = (width - 38.0).max(0.0);
        let spacing = ui.spacing().item_spacing.y;
        ui.spacing_mut().item_spacing.y = 0.0;
        for path in recent {
            let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let folder = path
                .parent()
                .map(|parent| compact_folder(parent, home.as_ref().map(|dirs| dirs.home_dir())))
                .unwrap_or_default();
            let (rect, response) = ui.allocate_exact_size(egui::vec2(width, RECENT_ROW_HEIGHT), egui::Sense::click());
            if response.hovered() {
                ui.painter().rect_filled(rect, 6, ui.visuals().widgets.hovered.weak_bg_fill);
            }
            let icon = egui::Rect::from_center_size(egui::pos2(rect.left() + 16.0, rect.center().y), egui::Vec2::splat(16.0));
            self.icons.image(ui, "file_copy", 16.0).tint(ui.visuals().weak_text_color()).paint_at(ui, icon);
            let painter = ui.painter().with_clip_rect(rect.shrink2(egui::vec2(6.0, 0.0)));
            for (text, is_name, font_size, color) in [
                (name.as_str(), true, 14.0, ui.visuals().strong_text_color()),
                (folder.as_str(), false, 12.0, ui.visuals().weak_text_color()),
            ] {
                let mut job = egui::text::LayoutJob::simple(text.to_owned(), egui::FontId::proportional(font_size), color, text_width);
                job.wrap.max_rows = 1;
                let galley = painter.layout_job(job);
                let top = if is_name {
                    rect.center().y - 1.0 - galley.size().y
                } else {
                    rect.center().y + 1.0
                };
                let position = egui::pos2(rect.left() + 32.0, top);
                painter.galley(position, galley, color);
            }
            response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &name));
            let response = response
                .on_hover_text(path.display().to_string())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if response.clicked() {
                *open = Some(path.clone());
            }
        }
        ui.spacing_mut().item_spacing.y = spacing;
    }
}

/// The Icy Draw logo with the version filled in, on a black screen like the other Icy tools.
/// Returns false when the logo could not be rendered.
fn logo(ui: &mut egui::Ui) -> bool {
    let id = egui::Id::new("start-logo");
    let texture = ui.ctx().data(|data| data.get_temp::<egui::TextureHandle>(id)).or_else(|| {
        let texture = ui
            .ctx()
            .load_texture("start-logo", render_logo(&logo_buffer()?)?, egui::TextureOptions::NEAREST);
        ui.ctx().data_mut(|data| data.insert_temp(id, texture.clone()));
        Some(texture)
    });
    let Some(texture) = texture else {
        return false;
    };
    // The dark start screen is black itself, so the logo needs no frame there.
    let padding = if ui.visuals().dark_mode { 0.0 } else { 12.0 };
    let size = texture.size_vec2();
    let (rect, _) = ui.allocate_exact_size(size + egui::Vec2::splat(padding * 2.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 8.0, Color32::BLACK);
    let image = egui::Rect::from_center_size(rect.center(), size);
    ui.painter().image(
        texture.id(),
        image,
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    true
}

/// `data/welcome.xb` is the wide variant of `gj-icydraw.xb` with a `@` version marker.
fn logo_buffer() -> Option<icy_engine::TextBuffer> {
    let mut buffer = FileFormat::XBin
        .from_bytes(include_bytes!("../../../data/welcome.xb"), None)
        .ok()?
        .screen
        .buffer;
    icy_engine_gui::version_helper::replace_version_marker(&mut buffer, &icy_draw::VERSION, None);
    Some(buffer)
}

fn render_logo(buffer: &icy_engine::TextBuffer) -> Option<egui::ColorImage> {
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
    fn folder_display_abbreviates_only_the_home_directory() {
        let home = Path::new("/home/alice");
        assert_eq!(compact_folder(home, Some(home)), "~");
        assert_eq!(
            compact_folder(&home.join("Downloads/art"), Some(home)),
            Path::new("~").join("Downloads/art").display().to_string()
        );
        assert_eq!(compact_folder(Path::new("/home/alice-other/art"), Some(home)), "/home/alice-other/art");
        assert_eq!(compact_folder(Path::new("/tmp/art"), Some(home)), "/tmp/art");
        assert_eq!(compact_folder(home, None), "/home/alice");
    }

    #[test]
    fn recent_folders_are_below_names_and_long_names_are_elided() {
        let context = egui::Context::default();
        let mut app = DrawApp::new();
        let recent = [
            Path::new("/one/a.ans").to_path_buf(),
            Path::new("/two/a-very-long-document-name-that-must-be-truncated.ans").to_path_buf(),
        ];
        let output = context.run(Default::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| {
                app.start_recent_section(ui, &recent, RECENT_MIN_WIDTH, &mut None);
            });
        });
        let text: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text),
                _ => None,
            })
            .collect();
        let first = text.iter().find(|text| text.galley.text() == "/one").unwrap();
        let second = text.iter().find(|text| text.galley.text() == "/two").unwrap();
        assert_eq!(first.pos.x, second.pos.x);
        let short = text.iter().find(|text| text.galley.text() == "a.ans").unwrap();
        let long = text.iter().find(|text| text.galley.text().starts_with("a-very-long")).unwrap();
        assert!(long.galley.elided);
        for (name, folder) in [(short, first), (long, second)] {
            assert_eq!(name.pos.x, folder.pos.x);
            assert!(name.pos.y + name.galley.size().y < folder.pos.y);
        }
        assert!(first.pos.y + first.galley.size().y < long.pos.y);
    }

    #[test]
    fn skypix_is_offered_on_the_start_screen_and_new_dialog() {
        let groups = NewKind::groups();
        assert!(groups[1].1.contains(&NewKind::Skypix));
        assert_eq!(
            groups
                .iter()
                .flat_map(|(_, kinds)| kinds.iter())
                .filter(|&&kind| kind == NewKind::Skypix)
                .count(),
            1
        );
        assert_eq!(NewKind::Skypix.name(), fl!("skypix-editor-title"));
        assert_eq!(NewKind::Skypix.description(), fl!("skypix-editor-description"));
        assert_eq!(NewKind::Skypix.icon(), "paint_brush");
        assert!(!NewKind::Skypix.has_size());
    }

    #[test]
    fn logo_shows_the_current_version_without_blinking() {
        let buffer = logo_buffer().unwrap();
        let rows: Vec<String> = (0..buffer.height())
            .map(|y| (0..buffer.width()).map(|x| buffer.char_at((x, y).into()).ch).collect())
            .collect();
        let version = format!("v{}", *icy_draw::VERSION);
        assert!(rows.iter().any(|row| row.contains(&version)), "{rows:#?}");
        assert!(!rows.iter().any(|row| row.contains('@')));
        let blinking = (0..buffer.height()).any(|y| (0..buffer.width()).any(|x| buffer.char_at((x, y).into()).attribute.is_blinking()));
        assert!(!blinking);
        let image = render_logo(&buffer).unwrap();
        assert_eq!(image.size, [buffer.width() as usize * 8, buffer.height() as usize * 16]);
    }
}
