//! Start screen shown when Icy Draw is launched without a file, and the document type rows
//! shared with the New dialog.

use super::super::widgets::Icons;
use super::{menus, DrawApp, FileAction, NewKind};
use eframe::egui::{self, Color32};
use icy_engine::Size;
use icy_engine_gui::egui::appearance::{self, PRIMARY};

/// Canvas size presets offered by the New and Canvas Size dialogs.
pub(super) const SIZE_PRESETS: [(i32, i32, &str); 5] = [
    (80, 25, "Standard"),
    (80, 50, "VGA 50 lines"),
    (132, 25, "Wide"),
    (132, 50, "Wide, 50 lines"),
    (40, 25, "40 columns"),
];

const TILE_SIZE: egui::Vec2 = egui::vec2(170.0, 92.0);
const TILE_SPACING: f32 = 10.0;

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
    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.left() + 26.0, rect.top() + 26.0), egui::Vec2::splat(22.0));
    icons.image(ui, icon, 22.0).tint(PRIMARY).paint_at(ui, icon_rect);
    let painter = ui.painter().with_clip_rect(rect.shrink(4.0));
    painter.text(
        egui::pos2(rect.left() + 14.0, rect.bottom() - 30.0),
        egui::Align2::LEFT_BOTTOM,
        title,
        egui::FontId::proportional(14.0),
        visuals.strong_text_color(),
    );
    painter.text(
        egui::pos2(rect.left() + 14.0, rect.bottom() - 27.0),
        egui::Align2::LEFT_TOP,
        subtitle,
        egui::FontId::proportional(12.0),
        visuals.weak_text_color(),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), title));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text.to_uppercase()).size(11.0).color(ui.visuals().weak_text_color()));
    ui.add_space(2.0);
}

impl DrawApp {
    pub(super) fn start_screen(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let recent: Vec<_> = self.settings.recent_files.files().into_iter().rev().take(8).collect();
        let mut tiles: Vec<(Option<NewKind>, &str, &str, &str)> = vec![
            (Some(NewKind::Ansi), "pencil", "ANSI Art", "80 × 25 canvas"),
            (None, "measure", "Custom…", "Choose type and size"),
        ];
        for kind in NewKind::ALL.into_iter().skip(1) {
            let name = kind.name().trim_start_matches("TheDraw ");
            let subtitle = if matches!(kind, NewKind::TheDraw(_)) { "TheDraw font" } else { kind.description() };
            tiles.push((Some(kind), kind.icon(), name, subtitle));
        }
        let mut create = None;
        let mut open = None;
        egui::ScrollArea::vertical().id_salt("start").auto_shrink([false, false]).show(ui, |ui| {
            let columns = ((ui.available_width() - 48.0 + TILE_SPACING) / (TILE_SIZE.x + TILE_SPACING)).floor().clamp(1.0, 4.0) as usize;
            let width = columns as f32 * TILE_SIZE.x + (columns - 1) as f32 * TILE_SPACING;
            let rows = tiles.len().div_ceil(columns) as f32;
            let content_height =
                170.0 + rows * (TILE_SIZE.y + TILE_SPACING) + if recent.is_empty() { 0.0 } else { 44.0 + recent.len() as f32 * 40.0 };
            ui.add_space(((ui.available_height() - content_height) / 2.0).max(24.0));
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
                ui.vertical(|ui| {
                    ui.set_width(width);
                    ui.label(appearance::bold(ui, "Icy Draw").size(26.0));
                    ui.label(egui::RichText::new("Create ANSI art, TheDraw fonts, bitmap fonts and animations.").weak());
                    ui.add_space(20.0);
                    caption(ui, "New");
                    for row in tiles.chunks(columns) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = TILE_SPACING;
                            for (kind, icon, title, subtitle) in row {
                                if tile(&mut self.icons, ui, icon, title, subtitle).clicked() {
                                    create = Some(*kind);
                                }
                            }
                        });
                        ui.add_space(TILE_SPACING - ui.spacing().item_spacing.y);
                    }
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        let open_button = appearance::primary_button("Open…")
                            .shortcut_text(egui::RichText::new(context.format_shortcut(&menus::OPEN)).color(Color32::from_white_alpha(190)))
                            .min_size(egui::vec2(150.0, 32.0));
                        if ui.add_enabled(!self.picker, open_button).clicked() {
                            self.choose(&context, FileAction::Open);
                        }
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("or drop a file anywhere in this window").weak().size(12.0));
                    });
                    if recent.is_empty() {
                        return;
                    }
                    ui.add_space(24.0);
                    caption(ui, "Recent");
                    for path in &recent {
                        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        let folder = path.parent().map(|parent| parent.display().to_string()).unwrap_or_default();
                        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 40.0), egui::Sense::click());
                        if response.hovered() {
                            ui.painter().rect_filled(rect, 6, ui.visuals().widgets.hovered.weak_bg_fill);
                        }
                        let icon = egui::Rect::from_center_size(egui::pos2(rect.left() + 18.0, rect.center().y), egui::Vec2::splat(18.0));
                        self.icons
                            .image(ui, "file_copy", 18.0)
                            .tint(ui.visuals().weak_text_color())
                            .paint_at(ui, icon);
                        let painter = ui.painter().with_clip_rect(rect);
                        painter.text(
                            egui::pos2(rect.left() + 38.0, rect.center().y - 1.0),
                            egui::Align2::LEFT_BOTTOM,
                            &name,
                            egui::FontId::proportional(14.0),
                            ui.visuals().strong_text_color(),
                        );
                        painter.text(
                            egui::pos2(rect.left() + 38.0, rect.center().y + 1.0),
                            egui::Align2::LEFT_TOP,
                            &folder,
                            egui::FontId::proportional(12.0),
                            ui.visuals().weak_text_color(),
                        );
                        let response = response
                            .on_hover_text(path.display().to_string())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if response.clicked() {
                            open = Some(path.clone());
                        }
                    }
                });
            });
            ui.add_space(24.0);
        });
        match create {
            Some(Some(kind)) => self.create(kind, Size::new(80, 25)),
            Some(None) => self.request_new(),
            None => {}
        }
        if let Some(path) = open {
            self.open(path);
        }
    }
}
