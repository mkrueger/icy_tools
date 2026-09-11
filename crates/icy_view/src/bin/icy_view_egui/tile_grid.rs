use eframe::egui;
use icy_view::thumbnail_backend::masonry_layout::{calculate_masonry_layout, ItemSize, MasonryConfig, MasonryLayout};

use super::{
    browser::Browser,
    icons::{Icon, Icons},
    text,
    thumbnails::Thumbnails,
};

const TILE_WIDTH: f32 = 332.0;
const SPACING: f32 = 16.0;
const PADDING: f32 = 6.0;
const LABEL_HEIGHT: f32 = 30.0;

#[derive(Default)]
pub struct TileGrid {
    pub layout: Option<MasonryLayout>,
    signature: Option<(u64, usize, u64, u32, String, String, icy_view::sort_order::SortOrder)>,
    pub offset: f32,
    pub viewport_height: f32,
}

fn layout(width: f32, items: impl Iterator<Item = (usize, usize, egui::Vec2)>) -> MasonryLayout {
    let column_width = TILE_WIDTH.min((width - 4.0).max(1.0));
    let columns = MasonryConfig::columns_for_width(width, column_width, SPACING, 2.0);
    let config = MasonryConfig::new(column_width, SPACING, 2.0, columns);
    let sizes: Vec<_> = items
        .map(|(index, span, dimensions)| {
            let column_span = span.clamp(1, columns.min(3));
            let content_width = (column_width * column_span as f32 + SPACING * (column_span - 1) as f32 - PADDING * 2.0).max(1.0);
            ItemSize {
                index,
                column_span,
                height: content_width * dimensions.y / dimensions.x.max(1.0) + LABEL_HEIGHT + PADDING * 2.0,
            }
        })
        .collect();
    calculate_masonry_layout(&config, &sizes)
}

impl TileGrid {
    pub fn navigate(&self, selected: Option<usize>, key: egui::Key) -> Option<usize> {
        let items = &self.layout.as_ref()?.items;
        if key == egui::Key::Home {
            return items.first().map(|item| item.index);
        }
        if key == egui::Key::End {
            return items.last().map(|item| item.index);
        }
        let Some(current) = items.iter().find(|item| Some(item.index) == selected) else {
            return items.first().map(|item| item.index);
        };
        items
            .iter()
            .filter(|item| item.index != current.index)
            .filter_map(|item| {
                let horizontal = item.x + item.width * 0.5 - current.x - current.width * 0.5;
                let vertical = item.y - current.y;
                let score = match key {
                    egui::Key::ArrowLeft if horizontal < -1.0 => horizontal.abs() + vertical.abs() * 2.0,
                    egui::Key::ArrowRight if horizontal > 1.0 => horizontal.abs() + vertical.abs() * 2.0,
                    egui::Key::ArrowUp if vertical < -1.0 => vertical.abs() + horizontal.abs() * 2.0,
                    egui::Key::ArrowDown if vertical > 1.0 => vertical.abs() + horizontal.abs() * 2.0,
                    egui::Key::PageUp if vertical < -1.0 => (vertical + self.viewport_height).abs() + horizontal.abs(),
                    egui::Key::PageDown if vertical > 1.0 => (vertical - self.viewport_height).abs() + horizontal.abs(),
                    _ => return None,
                };
                Some((item.index, score))
            })
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(index, _)| index)
            .or(selected)
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        browser: &Browser,
        thumbnails: &mut Thumbnails,
        icons: &mut Icons,
        ensure_selected: bool,
    ) -> Option<(usize, bool)> {
        thumbnails.set_source(ui.ctx(), &browser.location.point.path, browser.revision());
        let mut activate = None;
        let source = (&browser.location.point.path, &browser.filter);
        let scroll = egui::ScrollArea::vertical().id_salt(("masonry", source)).auto_shrink([false, false]);
        scroll.show_viewport(ui, |ui, viewport| {
            let width = ui.available_width();
            let signature = (
                browser.revision(),
                browser.items.len(),
                thumbnails.revision,
                width.to_bits(),
                source.0.clone(),
                source.1.clone(),
                browser.sort,
            );
            if self.signature.as_ref() != Some(&signature) {
                self.layout = Some(layout(
                    width,
                    browser.visible().into_iter().map(|index| {
                        let entry = thumbnails.entries.get(&Thumbnails::key(&*browser.items[index]));
                        (
                            index,
                            entry.map_or(1, |entry| entry.width_multiplier),
                            entry.map_or(egui::vec2(320.0, 100.0), |entry| entry.dimensions),
                        )
                    }),
                ));
                self.signature = Some(signature);
            }
            let layout = self.layout.as_ref().unwrap();
            let origin = ui.cursor().min;
            ui.allocate_space(egui::vec2(width, layout.content_height));
            self.offset = viewport.top();
            self.viewport_height = viewport.height();
            for tile in &layout.items {
                let rect = egui::Rect::from_min_size(origin + egui::vec2(tile.x, tile.y), egui::vec2(tile.width, tile.height));
                let selected = browser.selected == Some(tile.index);
                if selected && ensure_selected {
                    ui.scroll_to_rect(
                        rect.intersect(egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), viewport.height()))),
                        None,
                    );
                }
                if tile.y > viewport.bottom() + 500.0 || tile.y + tile.height < viewport.top() - 500.0 {
                    continue;
                }
                let item = &browser.items[tile.index];
                thumbnails.request(&**item);
                if !ui.is_rect_visible(rect) {
                    continue;
                }
                let response = ui.interact(rect, ui.id().with(tile.index), egui::Sense::click());
                let visuals = ui.style().interact_selectable(&response, selected);
                ui.painter()
                    .rect_filled(rect.translate(egui::vec2(3.0, 4.0)), 6.0, egui::Color32::from_black_alpha(45));
                ui.painter().rect_filled(rect, 6.0, visuals.bg_fill);
                ui.painter().rect_stroke(
                    rect,
                    6.0,
                    egui::Stroke::new(if selected || response.hovered() { 2.0 } else { 1.0 }, visuals.bg_stroke.color),
                    egui::StrokeKind::Inside,
                );
                let art = egui::Rect::from_min_max(rect.min + egui::Vec2::splat(PADDING), rect.max - egui::vec2(PADDING, PADDING + LABEL_HEIGHT));
                let entry = &thumbnails.entries[&Thumbnails::key(&**item)];
                if entry.images.is_empty() {
                    icons
                        .image(ui.ctx(), Icon::File(item.get_file_icon()), 36.0)
                        .paint_at(ui, egui::Rect::from_center_size(art.center(), egui::Vec2::splat(36.0)));
                } else {
                    let frame = (ui.input(|input| input.time) * 2.0) as usize % entry.images.len();
                    entry.images[frame].paint(ui, art);
                    if entry.images.len() > 1 {
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
                    }
                }
                let label_rect = egui::Rect::from_min_max(egui::pos2(art.left(), art.bottom() + 4.0), rect.max - egui::Vec2::splat(PADDING));
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(label_rect));
                child.add(
                    egui::Label::new(egui::RichText::new(item.get_label()).monospace().color(visuals.text_color()))
                        .truncate()
                        .selectable(false),
                );
                if response.double_clicked() {
                    activate = Some((tile.index, true));
                } else if response.clicked() {
                    activate = Some((tile.index, false));
                }
                response.on_hover_ui(|ui| {
                    ui.label(item.get_label());
                    if let Some(sauce) = &entry.sauce {
                        for value in [sauce.title().to_string(), sauce.author().to_string(), sauce.group().to_string()] {
                            if !value.trim().is_empty() {
                                ui.label(value);
                            }
                        }
                    }
                    if let Some(error) = &entry.error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                });
            }
            if layout.items.is_empty() && !browser.loading {
                ui.label(text("folder-empty"));
            }
        });
        thumbnails.trim();
        activate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_uses_tile_positions_and_viewport_distance() {
        let grid = TileGrid {
            layout: Some(layout(
                720.0,
                [
                    (0, 1, egui::vec2(320.0, 600.0)),
                    (1, 1, egui::vec2(320.0, 100.0)),
                    (2, 1, egui::vec2(320.0, 100.0)),
                    (3, 1, egui::vec2(320.0, 100.0)),
                    (4, 1, egui::vec2(320.0, 100.0)),
                ]
                .into_iter(),
            )),
            viewport_height: 320.0,
            ..Default::default()
        };
        assert_eq!(grid.navigate(Some(0), egui::Key::ArrowRight), Some(1));
        assert_eq!(grid.navigate(Some(2), egui::Key::ArrowLeft), Some(0));
        assert_eq!(grid.navigate(Some(1), egui::Key::ArrowDown), Some(2));
        assert_eq!(grid.navigate(Some(3), egui::Key::ArrowUp), Some(2));
        assert_eq!(grid.navigate(Some(1), egui::Key::PageDown), Some(3));
        assert_eq!(grid.navigate(Some(4), egui::Key::Home), Some(0));
        assert_eq!(grid.navigate(Some(0), egui::Key::End), Some(4));
    }

    #[test]
    fn original_spans_heights_and_shortest_column_placement_are_preserved() {
        let result = layout(
            1100.0,
            [
                (0, 1, egui::vec2(320.0, 2000.0)),
                (1, 2, egui::vec2(640.0, 100.0)),
                (2, 1, egui::vec2(320.0, 200.0)),
                (3, 3, egui::vec2(960.0, 100.0)),
            ]
            .into_iter(),
        );
        assert_eq!(result.items[0].width, 332.0);
        assert_eq!(result.items[1].width, 680.0);
        assert_eq!(result.items[3].width, 1028.0);
        assert!(result.items[0].height > 2000.0);
        assert_eq!(result.items[2].column, 1);
        for (index, item) in result.items.iter().enumerate() {
            let rect = egui::Rect::from_min_size(egui::pos2(item.x, item.y), egui::vec2(item.width, item.height));
            for other in &result.items[index + 1..] {
                assert!(!rect.intersects(egui::Rect::from_min_size(egui::pos2(other.x, other.y), egui::vec2(other.width, other.height))));
            }
        }
        let narrow = layout(290.0, [(0, 3, egui::vec2(960.0, 480.0))].into_iter());
        assert!(narrow.items[0].x + narrow.items[0].width <= 290.0);
        assert!((narrow.items[0].height - (286.0 - 12.0) * 0.5 - LABEL_HEIGHT - 12.0).abs() < 0.1);
    }
}
