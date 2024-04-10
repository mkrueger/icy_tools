use crate::{MainWindow, Message};
use eframe::{
    egui::{self, Sense},
    epaint::{Rect, Rounding, Vec2},
};
use egui::RichText;

pub fn add_tool_switcher(_ctx: &egui::Context, ui: &mut egui::Ui, arg: &MainWindow<'_>) -> Option<Message> {
    let mut msg = None;
    let spacing = 4.0;
    let icon_size = 28.0;

    let tools = arg.document_behavior.tools.lock();
    if tools[arg.document_behavior.get_selected_tool()].is_exclusive() {
        return msg;
    }
    let (id, back_rect) = ui.allocate_space(Vec2::new(230., 68.0));
    let mut pos = back_rect.min + Vec2::new(spacing, spacing);

    for i in 0..tools.len() {
        let t = &tools[i];
        if !t.is_visible() {
            continue;
        }

        let rect = Rect::from_min_size(pos.floor(), Vec2::new(icon_size, icon_size));
        let response = ui.interact(rect, id.with(i), Sense::click());
        if i == arg.document_behavior.get_selected_tool() {
            ui.painter()
                .rect_filled(rect.expand(2.0), Rounding::same(4.0), ui.style().visuals.extreme_bg_color);
            ui.painter()
                .rect_stroke(rect.expand(2.0), Rounding::same(4.0), ui.style().visuals.window_stroke);
        }

        if response.hovered() {
            ui.painter()
                .rect_filled(rect.expand(2.0), Rounding::same(4.0), ui.style().visuals.widgets.active.bg_fill);
            ui.painter()
                .rect_stroke(rect.expand(2.0), Rounding::same(4.0), ui.style().visuals.window_stroke);
        }

        let tint = if i == arg.document_behavior.get_selected_tool() {
            ui.visuals().widgets.active.fg_stroke.color
        } else {
            ui.visuals().widgets.inactive.fg_stroke.color
        };
        let mut image = t.get_icon().clone();
        image = image.tint(tint);
        image.paint_at(ui, rect);
        let response = response.on_hover_ui(|ui| {
            ui.strong(RichText::new(t.tool_name()).small());
            ui.label(RichText::new(t.tooltip()).small());
        });
        pos.x += icon_size + spacing;
        if pos.x - back_rect.min.x - spacing > back_rect.width() {
            pos.x = back_rect.min.x + spacing;
            pos.y += icon_size + spacing;
        }

        if response.clicked() {
            msg = Some(Message::SelectTool(i));
        }
    }
    msg
}
