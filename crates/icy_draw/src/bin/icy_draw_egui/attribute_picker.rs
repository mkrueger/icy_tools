//! Moebius' attribute picker: Escape without a selection shows the foreground colors as a column
//! and the background colors as a row, crossing at the current pair. The arrow keys change the
//! colors at once (up/down the foreground, left/right the background), a click picks a color and
//! closes the picker, and Escape, Enter or a click outside close it.

use eframe::egui::{self, Color32, Key};
use icy_draw::fl;
use icy_engine::Palette;

/// Size of one color cell; the current pair takes two cells each way.
const CELL: f32 = 22.0;
/// Gap between two swatches.
const GAP: f32 = 2.0;
/// Space below the grid for the colors' numbers and the keys.
const FOOTER: f32 = 42.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Foreground(u32),
    Background(u32),
}

/// What the picker shows and where.
pub struct Colors<'a> {
    pub palette: &'a Palette,
    pub foreground: u32,
    pub background: u32,
    /// Foreground colors to choose from, usually 16.
    pub foregrounds: u32,
    /// Background colors to choose from: 16 with iCE colors, 8 while the high ones blink.
    pub backgrounds: u32,
}

/// Where a click at `point` (relative to the picker) lands: on the foreground column or the
/// background row, with the current pair's cell left out of both.
pub fn hit(point: egui::Vec2, colors: &Colors) -> Option<Pick> {
    let (foreground, background) = (colors.foreground as i32, colors.background as i32);
    let column = ((point.x - CELL / 2.0) / CELL).floor() as i32;
    let row = ((point.y - CELL / 2.0) / CELL).floor() as i32;
    if column == background {
        let mut index = (point.y / CELL).floor() as i32;
        if index > foreground {
            index = ((point.y - CELL) / CELL).floor() as i32;
        }
        if (0..colors.foregrounds as i32).contains(&index) {
            return Some(Pick::Foreground(index as u32));
        }
    }
    if row == foreground {
        let mut index = (point.x / CELL).floor() as i32;
        if index > background {
            index = ((point.x - CELL) / CELL).floor() as i32;
        }
        if (0..colors.backgrounds as i32).contains(&index) {
            return Some(Pick::Background(index as u32));
        }
    }
    None
}

/// The next color when stepping through `count` colors, wrapping around.
pub fn step(color: u32, count: u32, forward: bool) -> u32 {
    let count = count.max(1);
    let color = color.min(count - 1);
    if forward {
        (color + 1) % count
    } else {
        (color + count - 1) % count
    }
}

/// Shows the picker centered on `center`. Returns the colors picked this frame and whether it
/// stays open. `keys` is false on the frame that opened it, so its Escape does not close it again.
pub fn show(context: &egui::Context, center: egui::Pos2, colors: &Colors, keys: bool) -> (Vec<Pick>, bool) {
    let mut picks = Vec::new();
    let mut open = true;
    let (mut foreground, mut background) = (
        colors.foreground.min(colors.foregrounds.max(1) - 1),
        colors.background.min(colors.backgrounds.max(1) - 1),
    );
    if keys {
        let pressed = |key| context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key));
        if pressed(Key::ArrowUp) {
            foreground = step(foreground, colors.foregrounds, false);
            picks.push(Pick::Foreground(foreground));
        }
        if pressed(Key::ArrowDown) {
            foreground = step(foreground, colors.foregrounds, true);
            picks.push(Pick::Foreground(foreground));
        }
        if pressed(Key::ArrowLeft) {
            background = step(background, colors.backgrounds, false);
            picks.push(Pick::Background(background));
        }
        if pressed(Key::ArrowRight) {
            background = step(background, colors.backgrounds, true);
            picks.push(Pick::Background(background));
        }
        if pressed(Key::Escape) || pressed(Key::Enter) {
            open = false;
        }
    }
    let shown = Colors {
        foreground,
        background,
        ..*colors
    };
    let grid = egui::vec2((colors.backgrounds + 1) as f32 * CELL, (colors.foregrounds + 1) as f32 * CELL);
    let size = grid + egui::vec2(0.0, FOOTER);
    let area = egui::Area::new(egui::Id::new("attribute-picker"))
        .order(egui::Order::Foreground)
        .fixed_pos(center - size / 2.0)
        .constrain(true)
        .show(context, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(12).corner_radius(10).show(ui, |ui| {
                ui.set_width(grid.x);
                let visuals = ui.visuals().clone();
                let (rect, response) = ui.allocate_exact_size(grid, egui::Sense::click());
                let hovered = response.hover_pos().and_then(|point| hit(point - rect.min, &shown));
                paint(ui, rect, &shown, hovered);
                if response.clicked() {
                    if let Some(pick) = response.interact_pointer_pos().and_then(|point| hit(point - rect.min, &shown)) {
                        picks.push(pick);
                        open = false;
                    }
                }
                if hovered.is_some() {
                    response.on_hover_cursor(egui::CursorIcon::PointingHand);
                }
                let (footer, _) = ui.allocate_exact_size(egui::vec2(grid.x, FOOTER), egui::Sense::hover());
                let description = match hovered {
                    Some(Pick::Foreground(index)) => fl!("attribute-picker-foreground", color = index),
                    Some(Pick::Background(index)) => fl!("attribute-picker-background", color = index),
                    None => fl!("attribute-picker-pair", foreground = foreground, background = background),
                };
                ui.painter().text(
                    footer.left_top() + egui::vec2(0.0, 8.0),
                    egui::Align2::LEFT_TOP,
                    description,
                    egui::FontId::proportional(13.0),
                    visuals.text_color(),
                );
                ui.painter().text(
                    footer.left_bottom(),
                    egui::Align2::LEFT_BOTTOM,
                    fl!("attribute-picker-keys"),
                    egui::FontId::proportional(11.5),
                    visuals.weak_text_color(),
                );
            });
        });
    let outside = context.input(|input| input.pointer.any_pressed() && input.pointer.interact_pos().is_some_and(|point| !area.response.rect.contains(point)));
    if outside {
        open = false;
    }
    (picks, open)
}

fn color(palette: &Palette, index: u32) -> Color32 {
    let (red, green, blue) = palette.rgb(index);
    Color32::from_rgb(red, green, blue)
}

/// A swatch with rounded corners and a hairline, so dark colors stand out on the popup.
fn swatch(painter: &egui::Painter, rect: egui::Rect, fill: Color32, edge: Color32) {
    painter.rect_filled(rect, 4.0, fill);
    painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, edge), egui::StrokeKind::Inside);
}

fn paint(ui: &egui::Ui, rect: egui::Rect, colors: &Colors, hovered: Option<Pick>) {
    let painter = ui.painter();
    let visuals = ui.visuals();
    let edge = visuals.widgets.noninteractive.bg_stroke.color.gamma_multiply(0.8);
    let (foreground, background) = (colors.foreground as f32, colors.background as f32);
    let origin = rect.min;
    let highlight = |cell: egui::Rect| {
        painter.rect_stroke(
            cell.expand(1.5),
            5.0,
            egui::Stroke::new(2.0, visuals.strong_text_color()),
            egui::StrokeKind::Outside,
        );
    };
    // The foreground column runs through the current background, skipping the current pair.
    let mut y = 0.0;
    for index in 0..colors.foregrounds {
        if index == colors.foreground {
            y += CELL;
        } else {
            let cell = egui::Rect::from_min_size(origin + egui::vec2(background * CELL + CELL / 2.0, y), egui::Vec2::splat(CELL)).shrink(GAP / 2.0);
            swatch(painter, cell, color(colors.palette, index), edge);
            if hovered == Some(Pick::Foreground(index)) {
                highlight(cell);
            }
        }
        y += CELL;
    }
    // The background row runs through the current foreground.
    let mut x = 0.0;
    for index in 0..colors.backgrounds {
        if index == colors.background {
            x += CELL;
        } else {
            let cell = egui::Rect::from_min_size(origin + egui::vec2(x, foreground * CELL + CELL / 2.0), egui::Vec2::splat(CELL)).shrink(GAP / 2.0);
            swatch(painter, cell, color(colors.palette, index), edge);
            if hovered == Some(Pick::Background(index)) {
                highlight(cell);
            }
        }
        x += CELL;
    }
    // The current pair: a sample character in the foreground color on the background.
    let pair = egui::Rect::from_min_size(origin + egui::vec2(background * CELL, foreground * CELL), egui::Vec2::splat(CELL * 2.0)).shrink(GAP / 2.0);
    painter.rect_filled(pair.expand(2.0), 8.0, visuals.selection.bg_fill);
    painter.rect_filled(pair, 6.0, color(colors.palette, colors.background));
    painter.text(
        pair.center(),
        egui::Align2::CENTER_CENTER,
        "Aa",
        egui::FontId::monospace(CELL * 0.8),
        color(colors.palette, colors.foreground),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors(palette: &Palette, foreground: u32, background: u32) -> Colors<'_> {
        Colors {
            palette,
            foreground,
            background,
            foregrounds: 16,
            backgrounds: 16,
        }
    }

    #[test]
    fn clicks_pick_from_the_column_or_the_row_around_the_current_pair() {
        let palette = Palette::dos_default();
        let colors = colors(&palette, 12, 0);
        let column = CELL / 2.0 + 1.0;
        assert_eq!(hit(egui::vec2(column, 1.0), &colors), Some(Pick::Foreground(0)));
        assert_eq!(hit(egui::vec2(column, 11.0 * CELL + 1.0), &colors), Some(Pick::Foreground(11)));
        // Below the current pair's two cells the column continues with the next color.
        assert_eq!(hit(egui::vec2(column, 14.0 * CELL + 1.0), &colors), Some(Pick::Foreground(13)));
        let row = 12.0 * CELL + CELL / 2.0 + 1.0;
        assert_eq!(hit(egui::vec2(2.0 * CELL + 1.0, row), &colors), Some(Pick::Background(1)));
        assert_eq!(hit(egui::vec2(16.0 * CELL + 1.0, row), &colors), Some(Pick::Background(15)));
        assert_eq!(hit(egui::vec2(8.0 * CELL, 3.0 * CELL), &colors), None, "beside the cross");
    }

    #[test]
    fn arrows_wrap_around_the_colors() {
        assert_eq!(step(15, 16, true), 0);
        assert_eq!(step(0, 16, false), 15);
        assert_eq!(step(7, 8, true), 0, "only the colors that can be chosen");
        assert_eq!(step(12, 8, false), 6, "a color out of range starts from the last one");
    }
}
