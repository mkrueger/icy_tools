//! Panels shared by the editors of home computer text screens (ATASCII, VT52): the tool rail,
//! glyphs drawn in the screen's font, function key tiles and the character map.

use eframe::egui::{self, Color32, Key};
use icy_draw::fl;
use icy_engine_edit::tools::{Tool, ToolPair};

use super::{chrome, widgets, DrawApp};

pub const FKEYS: [Key; 12] = [
    Key::F1,
    Key::F2,
    Key::F3,
    Key::F4,
    Key::F5,
    Key::F6,
    Key::F7,
    Key::F8,
    Key::F9,
    Key::F10,
    Key::F11,
    Key::F12,
];

/// The largest cell of the character map.
const GLYPH_CELL: f32 = 20.0;

/// The foreground and background a glyph is drawn in.
pub type GlyphColors = (Color32, Color32);

/// Which tool the pipette returns to: the tool of the last frame and the one before the pipette.
#[derive(Clone, Copy)]
pub struct PipetteReturn {
    last: Tool,
    before: Tool,
}

impl Default for PipetteReturn {
    fn default() -> Self {
        Self {
            last: Tool::Click,
            before: Tool::Pencil,
        }
    }
}

impl PipetteReturn {
    /// Notes the current tool; call once a frame.
    pub fn track(&mut self, tool: Tool) {
        if tool == Tool::Pipette && self.last != Tool::Pipette {
            self.before = self.last;
        }
        self.last = tool;
    }

    pub fn tool(self) -> Tool {
        self.before
    }
}

impl DrawApp {
    /// Paints `code` in the document's font into `rect`, scaled by whole pixels.
    pub(super) fn paint_screen_glyph(&self, painter: &egui::Painter, rect: egui::Rect, code: u8, (foreground, background): GlyphColors) {
        // The caret's character set: the VDC types in one of two.
        let font = self.document.with_state(|state| {
            let buffer = state.get_buffer();
            buffer.font(state.get_caret().attribute.font_page()).or_else(|| buffer.font(0)).cloned()
        });
        painter.rect_filled(rect, 2, background);
        if let Some(font) = &font {
            let glyph = font.size();
            let scale = (rect.width() / glyph.width as f32).min(rect.height() / glyph.height as f32).floor().max(1.0);
            let target = egui::Rect::from_center_size(rect.center(), egui::vec2(glyph.width as f32 * scale, glyph.height as f32 * scale));
            widgets::paint_glyph_on(painter, font, char::from(code), target, foreground);
        }
    }

    pub(super) fn screen_glyph(&self, ui: &mut egui::Ui, code: u8, size: egui::Vec2, colors: GlyphColors) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        self.paint_screen_glyph(ui.painter(), rect, code, colors);
        response
    }

    /// A function key of the toolbar: its character over the key name, like the ANSI editor's.
    pub(super) fn screen_fkey(&self, ui: &mut egui::Ui, code: u8, index: usize, colors: GlyphColors) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(30.0, 40.0), egui::Sense::click());
        if ui.is_enabled() && response.hovered() {
            ui.painter().rect_filled(rect, 5, ui.visuals().widgets.hovered.weak_bg_fill);
        }
        let glyph = egui::Rect::from_center_size(rect.center_top() + egui::vec2(0.0, 13.0), egui::Vec2::splat(24.0));
        self.paint_screen_glyph(ui.painter(), glyph, code, colors);
        let label = format!("F{}", index + 1);
        ui.painter().text(
            rect.center_bottom() - egui::vec2(0.0, 1.0),
            egui::Align2::CENTER_BOTTOM,
            &label,
            egui::FontId::proportional(10.0),
            ui.visuals().weak_text_color(),
        );
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &label));
        response.on_hover_text(format!("{label} · #{code:02X}"))
    }

    /// The function key tiles of `codes` between buttons for the previous and next set, with the
    /// set's number. Returns the picked code and the set step.
    pub(super) fn screen_fkey_bar(&mut self, ui: &mut egui::Ui, codes: &[u8; 12], (set, count): (usize, usize), colors: GlyphColors) -> (Option<u8>, i32) {
        let mut picked = None;
        let mut step = 0;
        ui.spacing_mut().item_spacing.x = 2.0;
        if self.icons.button(ui, "navigate_prev", &fl!("atascii-fkeys-previous"), false).clicked() {
            step = -1;
        }
        for (index, &code) in codes.iter().enumerate() {
            if self.screen_fkey(ui, code, index, colors).clicked() {
                picked = Some(code);
            }
        }
        if self.icons.button(ui, "navigate_next", &fl!("atascii-fkeys-next"), false).clicked() {
            step = 1;
        }
        ui.add_space(4.0);
        ui.weak(fl!("atascii-fkeys-set", set = (set + 1), count = count));
        (picked, step)
    }

    /// `codes` 16 to a row as the screen shows them, `selected` framed. Returns the clicked code.
    pub(super) fn character_map(&self, ui: &mut egui::Ui, codes: &[u8], selected: Option<u8>, colors: GlyphColors) -> Option<u8> {
        let glyph = self.document.with_state(|state| state.get_buffer().font(0).map(icy_engine::BitFont::size));
        let glyph = glyph.unwrap_or(icy_engine::Size::new(8, 8));
        // Two pixels between the characters, which are scaled by whole pixels.
        let pitch = (ui.available_width() / 16.0).floor().min(GLYPH_CELL);
        // Tall glyphs (8 × 16) stay at their size, so the map keeps to about square cells.
        let scale = ((pitch - 2.0) / glyph.width as f32).min((pitch + 6.0) / glyph.height as f32).floor().max(1.0);
        let cell = egui::vec2(pitch, glyph.height as f32 * scale + 2.0);
        let rows = codes.len().div_ceil(16);
        let (area, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), cell.y * rows as f32), egui::Sense::click());
        let origin = egui::pos2(area.center().x - pitch * 8.0, area.top());
        let cell_rect = |index: usize| {
            let (column, row) = ((index % 16) as f32, (index / 16) as f32);
            egui::Rect::from_min_size(origin + egui::vec2(column * cell.x, row * cell.y), cell).shrink(1.0)
        };
        let painter = ui.painter();
        for (index, &code) in codes.iter().enumerate() {
            self.paint_screen_glyph(painter, cell_rect(index), code, colors);
        }
        let hovered = response.hover_pos().and_then(|point| {
            let offset = point - origin;
            let (column, row) = ((offset.x / cell.x).floor(), (offset.y / cell.y).floor());
            let index = row as usize * 16 + column as usize;
            ((0.0..16.0).contains(&column) && row >= 0.0 && index < codes.len()).then_some(index)
        });
        if let Some(index) = hovered {
            painter.rect_stroke(
                cell_rect(index),
                2,
                egui::Stroke::new(1.0, Color32::from_white_alpha(160)),
                egui::StrokeKind::Outside,
            );
        }
        if let Some(index) = selected.and_then(|selected| codes.iter().position(|&code| code == selected)) {
            // A dark and a white ring, visible on any screen color.
            let rect = cell_rect(index).expand(1.0);
            painter.rect_stroke(rect, 3, egui::Stroke::new(3.0, Color32::from_black_alpha(200)), egui::StrokeKind::Outside);
            painter.rect_stroke(rect, 3, egui::Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Inside);
        }
        let code = hovered.map(|index| codes[index]);
        let response = match code {
            Some(code) => response.on_hover_text(format!("#{code:02X}")),
            None => response,
        };
        response.clicked().then_some(code).flatten()
    }

    /// The left rail with `groups` of tools.
    pub(super) fn screen_tool_rail(&mut self, context: &egui::Context, groups: &[&[ToolPair]], blocked: bool) {
        egui::SidePanel::left("sidebar")
            .exact_width(chrome::SIDEBAR_WIDTH)
            .frame(egui::Frame::new().fill(context.style().visuals.panel_fill))
            .resizable(false)
            .show(context, |ui| {
                if blocked || self.document.paste_active() {
                    ui.disable();
                }
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.add_space(6.0);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for (index, group) in groups.iter().enumerate() {
                        if index > 0 {
                            chrome::rail_divider(ui);
                        }
                        for &pair in *group {
                            self.tool_button(ui, pair);
                        }
                    }
                });
            });
    }
}
