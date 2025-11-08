use std::sync::Arc;

use eframe::{
    egui::{self, RichText},
    epaint::{Color32, Rect, Stroke, Vec2},
};
use egui::mutex::Mutex;
use i18n_embed_fl::fl;
use icy_engine::TextPane;
use icy_engine_gui::BufferView;

use crate::{AnsiEditor, Document, Message, SETTINGS, ToolWindow};

pub struct MinimapToolWindow {
    buffer_view: Arc<eframe::epaint::mutex::Mutex<BufferView>>,
    undo_size: i32,
    id: u64,
    palette_hash: u32,
    next_scroll_pos: Option<Vec2>,
}

impl ToolWindow for MinimapToolWindow {
    fn get_title(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "minimap_tool_title")
    }

    fn show_ui(&mut self, ui: &mut egui::Ui, active_document: Option<Arc<Mutex<Box<dyn Document>>>>) -> Option<Message> {
        if let Some(doc) = active_document {
            if let Some(editor) = doc.lock().get_ansi_editor_mut() {
                return self.show_minimap(ui, editor);
            }
        }
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "no_document_selected")).small());
        });
        None
    }
}

impl MinimapToolWindow {
    pub fn show_minimap(&mut self, ui: &mut egui::Ui, editor: &mut AnsiEditor) -> Option<Message> {
        let undo_stack = editor.buffer_view.lock().get_edit_state().undo_stack_len() as i32;
        let cur_palette_hash = editor.buffer_view.lock().get_buffer_mut().palette.get_checksum();

        if editor.id != self.id || undo_stack != self.undo_size || self.palette_hash != cur_palette_hash {
            self.id = editor.id;
            self.undo_size = undo_stack;
            let bv = editor.buffer_view.lock();
            let buffer = bv.get_buffer();
            self.buffer_view.lock().get_buffer_mut().set_size(buffer.get_size());
            self.buffer_view.lock().get_buffer_mut().layers = buffer.layers.clone();
            self.buffer_view.lock().get_buffer_mut().buffer_type = buffer.buffer_type;
            self.buffer_view.lock().get_buffer_mut().palette = buffer.palette.clone();
            self.buffer_view.lock().get_buffer_mut().set_font_table(buffer.get_font_table());
            self.palette_hash = cur_palette_hash;
            self.buffer_view.lock().redraw_font();
            self.buffer_view.lock().redraw_view();
        }
        self.buffer_view.lock().use_fg = editor.buffer_view.lock().use_fg;
        self.buffer_view.lock().use_bg = editor.buffer_view.lock().use_bg;
        let w = (ui.available_width() / self.buffer_view.lock().get_buffer().get_font_dimensions().width as f32).floor();

        let scalex = (w / self.buffer_view.lock().get_width() as f32).min(2.0);
        let scaley = if self.buffer_view.lock().get_buffer_mut().use_aspect_ratio() {
            scalex * 1.35
        } else {
            scalex
        };

        let mut opt = icy_engine_gui::TerminalOptions {
            filter: glow::LINEAR as i32,
            monitor_settings: unsafe { SETTINGS.monitor_settings.clone() },
            stick_to_bottom: false,
            scale: Some(Vec2::new(scalex, scaley)),
            use_terminal_height: false,
            hide_scrollbars: true,

            ..Default::default()
        };

        let next_scroll_pos = self.next_scroll_pos.take();

        if let Some(next_scroll_pos) = next_scroll_pos {
            opt.scroll_offset_x = Some(next_scroll_pos.x);
            opt.scroll_offset_y = Some(next_scroll_pos.y);
        }

        let (response, ours) = icy_engine_gui::show_terminal_area(ui, self.buffer_view.clone(), opt);

        let theirs: icy_engine_gui::TerminalCalc = editor.buffer_view.lock().calc.clone();

        let their_total_size = Vec2::new(theirs.char_width, theirs.char_height) * theirs.char_size;
        let their_buffer_size = Vec2::new(theirs.buffer_char_width, theirs.buffer_char_height) * theirs.char_size;

        let our_total_size = Vec2::new(ours.char_width, ours.char_height) * ours.char_size;

        let tmax_y: f32 = theirs.font_height * (theirs.char_height - theirs.buffer_char_height).max(0.0);
        let tmax_x: f32 = theirs.font_width * (theirs.real_width as f32 - theirs.buffer_char_width).max(0.0);

        let mut size = our_total_size * their_buffer_size / their_total_size;
        let tx = theirs.char_scroll_position.x / tmax_x.max(1.0);
        let ty = theirs.char_scroll_position.y / tmax_y.max(1.0);

        let pos = (our_total_size - size - Vec2::new(1.0, 1.0)) * Vec2::new(tx, ty);

        let pos = pos - ours.char_scroll_position * ours.scale;
        let min = (ours.buffer_rect.min + pos).floor() + Vec2::new(0.5, 0.5);

        let corr = ours.buffer_rect.min.y + ours.terminal_rect.height() - (size.y + min.y);
        let view_size = Vec2::new(size.x, size.y + corr.min(0.0)).floor();

        ui.painter().rect_stroke(
            Rect::from_min_size(min, view_size),
            0.0,
            Stroke::new(1.0, Color32::from_rgba_premultiplied(157, 157, 157, 220)),
            egui::StrokeKind::Outside,
        );

        if pos.x < 0.0 || pos.y < 0.0 {
            self.next_scroll_pos = Some(ours.char_scroll_position + pos / ours.scale);
            ui.ctx().request_repaint();
        }

        size.x = size.x.min(ours.terminal_rect.size().x);
        size.y = size.y.min(ours.terminal_rect.size().y);

        if pos.x + size.x > ours.terminal_rect.size().x || pos.y + size.y > ours.terminal_rect.size().y {
            let p = pos + size - ours.terminal_rect.size();
            self.next_scroll_pos = Some(ours.char_scroll_position + p / ours.scale);
            ui.ctx().request_repaint();
        }

        if response.dragged() {
            if let Some(pos) = response.hover_pos() {
                let pos = (pos - ours.buffer_rect.min) / ours.scale + ours.char_scroll_position;
                editor.next_scroll_x_position = Some(pos.x - theirs.buffer_char_width * theirs.font_width / 2.0);
                editor.next_scroll_y_position = Some(pos.y - theirs.buffer_char_height * theirs.font_height / 2.0);
                ui.ctx().request_repaint();
            }
        }

        None
    }

    pub(crate) fn new(gl: Arc<glow::Context>) -> Self {
        let mut buffer_view = BufferView::new(&gl);
        buffer_view.interactive = false;
        buffer_view.get_buffer_mut().terminal_state.is_terminal_buffer = false;
        buffer_view.get_caret_mut().set_is_visible(false);
        Self {
            id: 0,
            buffer_view: Arc::new(eframe::epaint::mutex::Mutex::new(buffer_view)),
            undo_size: -1,
            palette_hash: 0,
            next_scroll_pos: None,
        }
    }
}
