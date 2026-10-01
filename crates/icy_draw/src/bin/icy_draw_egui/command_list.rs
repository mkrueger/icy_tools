use eframe::egui::{self, Color32, Stroke};

use super::{playback::RowMark, widgets::Icons};

pub const ROW_HEIGHT: f32 = 24.0;

#[derive(Clone, Copy, Default)]
pub enum Tone {
    #[default]
    Normal,
    Muted,
    Warning,
}

#[derive(Clone, Copy, Default)]
pub enum SwatchLayout {
    #[default]
    ReplaceIcon,
    AfterIcon,
}

pub struct CommandRow<'a> {
    pub index: usize,
    pub name: &'a str,
    pub summary: &'a str,
    pub icon: Option<&'a str>,
    pub swatch: Option<Color32>,
    pub swatch_layout: SwatchLayout,
    pub selected: bool,
    pub related: bool,
    pub tone: Tone,
    pub mark: RowMark,
}

impl CommandRow<'_> {
    pub fn show(self, ui: &mut egui::Ui, icons: &mut Icons) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_HEIGHT), egui::Sense::click());
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, ui.is_enabled(), self.selected, self.name));
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let visuals = ui.visuals().clone();
        let painter = ui.painter_at(rect);
        let background = if self.selected {
            Some(visuals.selection.bg_fill)
        } else if self.related {
            Some(visuals.selection.bg_fill.gamma_multiply(0.4))
        } else if response.hovered() {
            Some(visuals.widgets.hovered.weak_bg_fill)
        } else {
            None
        };
        if let Some(fill) = background {
            painter.rect_filled(rect.shrink2(egui::vec2(2.0, 1.0)), 4, fill);
        }
        self.mark.paint_background(ui, rect, background.is_some());
        let text = if self.selected {
            visuals.selection.stroke.color
        } else {
            match self.tone {
                Tone::Normal => visuals.text_color(),
                Tone::Muted => visuals.weak_text_color(),
                Tone::Warning => visuals.warn_fg_color,
            }
        };
        let text = self.mark.text(text, self.selected);
        let weak = if self.selected {
            text.gamma_multiply(0.7)
        } else {
            self.mark.text(visuals.weak_text_color(), false)
        };
        let center = rect.center().y;
        painter.text(
            egui::pos2(rect.left() + 34.0, center),
            egui::Align2::RIGHT_CENTER,
            (self.index + 1).to_string(),
            egui::FontId::monospace(11.0),
            weak,
        );
        let mut x = rect.left() + 42.0;
        let swatch = |cell: egui::Rect, color| {
            painter.rect_filled(cell, 2, color);
            painter.rect_stroke(cell, 2, Stroke::new(1.0, weak), egui::StrokeKind::Inside);
        };
        let cell = egui::Rect::from_center_size(egui::pos2(x + 7.0, center), egui::Vec2::splat(14.0));
        match (self.swatch_layout, self.swatch) {
            (SwatchLayout::ReplaceIcon, Some(color)) => swatch(cell.shrink(1.0), color),
            _ => {
                if let Some(icon) = self.icon {
                    icons.image(ui, icon, 14.0).tint(text).paint_at(ui, cell);
                }
            }
        }
        x += 22.0;
        if let (SwatchLayout::AfterIcon, Some(color)) = (self.swatch_layout, self.swatch) {
            swatch(egui::Rect::from_min_size(egui::pos2(x, center - 6.0), egui::Vec2::splat(12.0)), color);
            x += 18.0;
        }
        let font = egui::TextStyle::Body.resolve(ui.style());
        let mut job = egui::text::LayoutJob::default();
        job.append(
            self.name,
            0.0,
            egui::TextFormat {
                font_id: font.clone(),
                color: text,
                ..Default::default()
            },
        );
        if !self.summary.is_empty() {
            job.append(
                self.summary,
                8.0,
                egui::TextFormat {
                    font_id: font,
                    color: weak,
                    ..Default::default()
                },
            );
        }
        job.wrap = egui::text::TextWrapping::truncate_at_width((rect.right() - 6.0 - x).max(0.0));
        let galley = painter.layout_job(job);
        painter.galley(egui::pos2(x, center - galley.size().y / 2.0), galley, text);
        response.on_hover_text(if self.summary.is_empty() {
            self.name.to_owned()
        } else {
            format!("{}  {}", self.name, self.summary)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_command_rows_stay_single_line_and_inside_narrow_panels() {
        let context = egui::Context::default();
        let output = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(220.0, 300.0))),
                ..Default::default()
            },
            |context| {
                egui::CentralPanel::default().show(context, |ui| {
                    let mut icons = Icons::default();
                    for layout in [SwatchLayout::ReplaceIcon, SwatchLayout::AfterIcon] {
                        let width = ui.available_width();
                        let response = CommandRow {
                            index: 123,
                            name: "Line",
                            summary: "117, 16 -> 302, 86, followed by a deliberately very long description",
                            icon: Some("line"),
                            swatch: Some(Color32::RED),
                            swatch_layout: layout,
                            selected: true,
                            related: false,
                            tone: Tone::Normal,
                            mark: RowMark::default(),
                        }
                        .show(ui, &mut icons);
                        assert_eq!(response.rect.width(), width);
                        assert_eq!(response.rect.height(), ROW_HEIGHT);
                    }
                });
            },
        );
        for shape in output.shapes {
            if let egui::Shape::Text(text) = shape.shape {
                assert_eq!(text.galley.rows.len(), 1);
                assert!(text.galley.size().x <= 204.0);
            }
        }
    }
}
