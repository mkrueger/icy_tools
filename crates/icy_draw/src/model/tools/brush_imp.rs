use eframe::egui::Response;
use egui::{load::SizedTexture, Image, TextureHandle, Widget};
use i18n_embed_fl::fl;
use icy_engine::{editor::AtomicUndoGuard, AttributedChar, Layer, TextPane};
use icy_engine_gui::TerminalCalc;
use std::{cell::RefCell, rc::Rc};

use crate::{
    create_image,
    paint::{BrushMode, ColorMode},
    AnsiEditor, DragMode, Event, Message,
};

use super::{Position, Tool};

pub static mut CUSTOM_BRUSH: Option<Layer> = None;

pub struct BrushTool {
    color_mode: ColorMode,
    size: i32,
    char_code: Rc<RefCell<char>>,

    undo_op: Option<AtomicUndoGuard>,
    cur_pos: Position,
    custom_brush: Option<Layer>,
    image: Option<TextureHandle>,
    brush_mode: BrushMode,
}

impl Default for BrushTool {
    fn default() -> Self {
        Self {
            size: 3,
            color_mode: ColorMode::Both,
            undo_op: None,
            custom_brush: None,
            image: None,
            brush_mode: BrushMode::Shade,
            char_code: Rc::new(RefCell::new('\u{00B0}')),
            cur_pos: Position::default(),
        }
    }
}
impl BrushTool {
    fn paint_brush(&self, editor: &mut AnsiEditor, pos: Position, swap_colors: bool) {
        let mid = Position::new(-(self.size / 2), -(self.size / 2));

        let center = pos + mid;
        let gradient = ['\u{00B0}', '\u{00B1}', '\u{00B2}', '\u{00DB}'];
        let caret_attr = editor.buffer_view.lock().get_caret().get_attribute();
        if matches!(self.brush_mode, BrushMode::Custom) {
            editor.join_overlay("brush");
            return;
        }

        let use_selection = editor.buffer_view.lock().get_edit_state().is_something_selected();
        editor.buffer_view.lock().get_edit_state_mut().set_is_buffer_dirty();

        let offset = if let Some(layer) = editor.buffer_view.lock().get_edit_state().get_cur_layer() {
            layer.get_offset()
        } else {
            Position::default()
        };

        for y in 0..self.size {
            for x in 0..self.size {
                let pos = center + Position::new(x, y);
                if use_selection && !editor.buffer_view.lock().get_edit_state().get_is_selected(pos + offset) {
                    continue;
                }
                let ch = editor.get_char_from_cur_layer(pos);
                let mut attribute = ch.attribute;
                attribute.attr &= !icy_engine::attribute::INVISIBLE;

                if self.color_mode.use_fore() {
                    attribute.set_foreground(if swap_colors {
                        caret_attr.get_background()
                    } else {
                        caret_attr.get_foreground()
                    });
                }
                if self.color_mode.use_back() {
                    attribute.set_background(if swap_colors {
                        caret_attr.get_foreground()
                    } else {
                        caret_attr.get_background()
                    });
                }

                match &self.brush_mode {
                    BrushMode::Shade => {
                        let mut char_code = gradient[0];
                        if ch.ch == gradient[gradient.len() - 1] {
                            char_code = gradient[gradient.len() - 1];
                        } else {
                            for i in 0..gradient.len() - 1 {
                                if ch.ch == gradient[i] {
                                    char_code = gradient[i + 1];
                                    break;
                                }
                            }
                        }
                        editor.set_char(pos, AttributedChar::new(char_code, attribute));
                    }
                    BrushMode::Char(ch) => {
                        attribute.set_font_page(caret_attr.get_font_page());
                        editor.set_char(center + Position::new(x, y), AttributedChar::new(*ch.borrow(), attribute));
                    }
                    BrushMode::Colorize => {
                        editor.set_char(pos, AttributedChar::new(ch.ch, attribute));
                    }
                    _ => {}
                }
            }
        }
    }
}

impl Tool for BrushTool {
    fn get_icon(&self) -> &egui::Image<'static> {
        &super::icons::BRUSH_SVG
    }

    fn tool_name(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-paint_brush_name")
    }

    fn tooltip(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-paint_brush_tooltip")
    }

    fn use_caret(&self, _editor: &AnsiEditor) -> bool {
        false
    }

    fn show_ui(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, editor_opt: Option<&mut AnsiEditor>) -> Option<Message> {
        self.color_mode.show_ui(ui);

        ui.horizontal(|ui| {
            ui.label(fl!(crate::LANGUAGE_LOADER, "tool-size-label"));
            ui.add(egui::DragValue::new(&mut self.size).range(1..=20).speed(1));
        });
        /*
                ui.radio_value(&mut self.brush_type, BrushType::Shade, fl!(crate::LANGUAGE_LOADER, "tool-shade"));
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.brush_type, BrushType::Solid, fl!(crate::LANGUAGE_LOADER, "tool-character"));
                    if let Some(buffer_opt) = buffer_opt {
                        result = draw_glyph(ui, buffer_opt, &self.char_code);
                    }
                });
                ui.radio_value(&mut self.brush_type, BrushType::Color, fl!(crate::LANGUAGE_LOADER, "tool-colorize"));
        */
        let result = self.brush_mode.show_ui(ui, editor_opt, self.char_code.clone(), crate::paint::BrushUi::Brush);

        unsafe {
            if CUSTOM_BRUSH.is_some() {
                self.custom_brush = CUSTOM_BRUSH.take();
            }
        }

        if let Some(custom_brush) = &self.custom_brush {
            let mut layer = custom_brush.clone();
            layer.set_offset((0, 0));
            layer.role = icy_engine::Role::Normal;
            let mut buf = icy_engine::Buffer::new(layer.get_size());
            layer.set_title(buf.layers[0].get_title());
            buf.layers.clear();
            buf.layers.push(layer);
            self.image = Some(create_image(ctx, &buf));

            ui.radio_value(&mut self.brush_mode, BrushMode::Custom, fl!(crate::LANGUAGE_LOADER, "tool-custom-brush"));
            if let Some(image) = &self.image {
                let sized_texture: SizedTexture = image.into();
                let w = ui.available_width() - 16.0;
                let scale = w / sized_texture.size.x;
                let image = Image::from_texture(sized_texture).fit_to_original_size(scale);
                image.ui(ui);
            }
        }
        result
    }

    fn handle_no_hover(&mut self, editor: &mut AnsiEditor) {
        if matches!(self.brush_mode, BrushMode::Custom) {
            editor.clear_overlay_layer();
        }
        let lock = &mut editor.buffer_view.lock();
        let get_edit_state_mut = lock.get_edit_state_mut();
        if get_edit_state_mut.get_tool_overlay_mask_mut().is_empty() {
            return;
        }
        get_edit_state_mut.get_tool_overlay_mask_mut().clear();
        get_edit_state_mut.set_is_buffer_dirty();
    }

    fn handle_hover(&mut self, _ui: &egui::Ui, response: egui::Response, editor: &mut AnsiEditor, cur: Position, cur_abs: Position) -> egui::Response {
        if matches!(self.brush_mode, BrushMode::Custom) {
            editor.clear_overlay_layer();
            let lock = &mut editor.buffer_view.lock();
            if let Some(layer) = lock.get_edit_state_mut().get_overlay_layer() {
                if let Some(brush) = &self.custom_brush {
                    let mid = Position::new(-(brush.get_width() / 2), -(brush.get_height() / 2));
                    self.cur_pos = cur + mid;
                    for y in 0..brush.get_height() {
                        for x in 0..brush.get_width() {
                            let pos = Position::new(x, y);
                            let ch = brush.get_char(pos);
                            layer.set_char(cur + pos + mid, AttributedChar::new(ch.ch, ch.attribute));
                        }
                    }
                    lock.get_edit_state_mut().set_is_buffer_dirty();
                }
            }
        } else {
            let mid = Position::new(-(self.size / 2), -(self.size / 2));

            if self.cur_pos != cur + mid {
                self.cur_pos = cur + mid;
                let lock = &mut editor.buffer_view.lock();
                let get_tool_overlay_mask_mut = lock.get_edit_state_mut().get_tool_overlay_mask_mut();
                get_tool_overlay_mask_mut.clear();
                for y in 0..self.size {
                    for x in 0..self.size {
                        let pos = cur_abs + Position::new(x, y) + mid;
                        get_tool_overlay_mask_mut.set_is_selected(pos, true);
                    }
                }
                lock.get_edit_state_mut().set_is_buffer_dirty();
            }
            editor.buffer_view.lock().get_buffer_mut().remove_overlay();
        }
        response.on_hover_cursor(egui::CursorIcon::Crosshair)
    }

    fn handle_click(&mut self, editor: &mut AnsiEditor, button: i32, pos: Position, _pos_abs: Position, _response: &Response) -> Option<Message> {
        let _op: AtomicUndoGuard = editor.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-paint-brush"));
        self.paint_brush(editor, pos, button == 2);
        None
    }

    fn handle_drag(&mut self, _ui: &egui::Ui, response: egui::Response, editor: &mut AnsiEditor, _calc: &TerminalCalc) -> egui::Response {
        self.paint_brush(editor, editor.drag_pos.cur, matches!(editor.drag_started, DragMode::Secondary));
        response
    }

    fn handle_drag_begin(&mut self, editor: &mut AnsiEditor, _response: &egui::Response) -> Event {
        self.undo_op = Some(editor.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-paint-brush")));
        self.paint_brush(editor, editor.drag_pos.cur, matches!(editor.drag_started, DragMode::Secondary));
        Event::None
    }

    fn handle_drag_end(&mut self, _editor: &mut AnsiEditor) -> Option<Message> {
        self.undo_op = None;
        None
    }

    fn get_toolbar_location_text(&self, _editor: &AnsiEditor) -> String {
        let pos = self.cur_pos;
        fl!(crate::LANGUAGE_LOADER, "toolbar-position", line = (pos.y + 1), column = (pos.x + 1))
    }
}
