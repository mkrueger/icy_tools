use eframe::epaint::{FontId, Rounding, Vec2};
use egui::{FontFamily, RichText, Sense, WidgetText};
use i18n_embed_fl::fl;
use icy_engine::{Rectangle, Tag, TextAttribute};
use icy_engine_gui::TerminalCalc;

use crate::{medium_hover_button, AnsiEditor, Document, Event, Message};

use super::{Position, Tool};

#[derive(Default)]
pub struct TagTool {
    drag_started: bool,
    cur: Option<Position>,
    drag_start: Position,
    drag_offset: Position,
    drag: usize,
}

impl Tool for TagTool {
    fn get_icon(&self) -> &egui::Image<'static> {
        &super::icons::TAG_SVG
    }

    fn tool_name(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-tag_name")
    }

    fn tooltip(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-tag_tooltip")
    }

    fn use_caret(&self, _editor: &AnsiEditor) -> bool {
        false
    }

    fn use_selection(&self) -> bool {
        false
    }

    fn show_ui(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui, editor_opt: Option<&mut AnsiEditor>) -> Option<Message> {
        let mut result: Option<Message> = None;

        let Some(editor) = editor_opt else {
            return None;
        };

        let cur_tag = editor.buffer_view.lock().get_edit_state().get_current_tag().unwrap();
        if editor.has_tool_switched() {
            editor.set_tool_switch(false);
            if cur_tag < editor.buffer_view.lock().get_buffer().tags.len() {
                let attr = editor.buffer_view.lock().get_buffer().tags[cur_tag].attribute;
                editor.buffer_view.lock().get_edit_state_mut().get_caret_mut().set_attr(attr);
                return None;
            }
        } else {
            if cur_tag < editor.buffer_view.lock().get_buffer().tags.len() {
                let attr = editor.buffer_view.lock().get_edit_state().get_caret().get_attribute();
                if editor.buffer_view.lock().get_buffer().tags[cur_tag].attribute != attr {
                    let mut changed_tag = editor.buffer_view.lock().get_buffer().tags[cur_tag].clone();
                    changed_tag.attribute = attr;
                    return Some(Message::UpdateTag(changed_tag.into(), cur_tag));
                }
            }
        }

        ui.horizontal(|ui| {
            let r = medium_hover_button(ui, &crate::ADD_LAYER_SVG).on_hover_ui(|ui| {
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "add_tag_tooltip")).small());
            });

            if r.clicked() {
                result = Some(Message::ShowEditTagDialog(
                    Box::new(Tag {
                        is_enabled: true,
                        position: Position::default(),
                        length: 0,
                        preview: "New Tag".to_string(),
                        replacement_value: String::new(),
                        alignment: std::fmt::Alignment::Left,
                        attribute: TextAttribute::default(),
                        tag_placement: icy_engine::TagPlacement::InText,
                        tag_role: icy_engine::TagRole::Displaycode,
                    }),
                    -1,
                ));
            }

            let r = medium_hover_button(ui, &crate::DELETE_SVG).on_hover_ui(|ui| {
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "delete_tag_tooltip")).small());
            });
            if r.clicked() {
                if cur_tag < editor.buffer_view.lock().get_buffer().tags.len() {
                    result = Some(Message::RemoveTag(cur_tag));
                }
            }
        });
        egui::ScrollArea::vertical().id_source("tag_view_scroll_area").show(ui, |ui| {
            ui.vertical(|ui| {
                for (i, tag) in editor.buffer_view.lock().get_buffer().tags.iter().enumerate() {
                    let row_height = 22.0;
                    let (id, rect) = ui.allocate_space(Vec2::new(ui.available_width(), row_height));
                    let response = ui.interact(rect, id, Sense::click());
                    let is_selected = i == cur_tag;

                    let back_painter = ui.painter_at(rect);
                    if response.hovered() {
                        back_painter.rect_filled(rect, Rounding::ZERO, ui.style().visuals.widgets.active.bg_fill);
                    } else if is_selected {
                        back_painter.rect_filled(rect, Rounding::ZERO, ui.style().visuals.extreme_bg_color);
                    }

                    let text_color = if is_selected {
                        ui.style().visuals.strong_text_color()
                    } else {
                        ui.style().visuals.text_color()
                    };

                    let font_id = FontId::new(14.0, FontFamily::Proportional);
                    let text: WidgetText = tag.replacement_value.clone().into();
                    let galley = text.into_galley(ui, Some(egui::TextWrapMode::Truncate), f32::INFINITY, font_id);
                    ui.painter().galley_with_override_text_color(
                        egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), rect.shrink(4.0)).min,
                        galley,
                        text_color,
                    );

                    if response.clicked() {
                        result = Some(Message::SelectCurrentTag(i as usize));
                    }

                    if response.double_clicked() {
                        result = Some(Message::ShowEditTagDialog(tag.clone().into(), i as i32));
                    }
                }
            });
        });
        result
    }

    fn handle_hover(&mut self, _ui: &egui::Ui, response: egui::Response, editor: &mut AnsiEditor, cur: Position, _cur_abs: Position) -> egui::Response {
        let tags = editor.buffer_view.lock().get_buffer().tags.clone();
        update_tag_rects(editor);
        self.cur = Some(cur);

        for tag in tags {
            if tag.contains(cur) {
                return response.on_hover_cursor(egui::CursorIcon::Crosshair);
            }
        }

        response.on_hover_cursor(egui::CursorIcon::Default)
    }

    fn handle_no_hover(&mut self, editor: &mut AnsiEditor) {
        let lock = &mut editor.buffer_view.lock();
        let get_edit_state_mut = lock.get_edit_state_mut();
        if get_edit_state_mut.get_tool_overlay_mask_mut().is_empty() {
            return;
        }
        get_edit_state_mut.get_tool_overlay_mask_mut().clear();
        get_edit_state_mut.set_is_buffer_dirty();
    }

    fn handle_drag_begin(&mut self, editor: &mut AnsiEditor, _response: &egui::Response) -> Event {
        self.drag_started = false;
        let Some(cur) = self.cur else {
            return Event::None;
        };
        for (i, tag) in editor.buffer_view.lock().get_buffer().tags.iter().enumerate() {
            if tag.contains(cur) {
                self.drag_started = true;
                self.drag_start = tag.position;
                self.drag = i;
                break;
            }
        }
        Event::None
    }

    fn handle_drag(&mut self, _ui: &egui::Ui, response: egui::Response, editor: &mut AnsiEditor, _calc: &TerminalCalc) -> egui::Response {
        if !self.drag_started {
            return response;
        }

        self.drag_offset = self.drag_start + editor.drag_pos.cur_abs - editor.drag_pos.start_abs;

        let _ = editor.buffer_view.lock().get_edit_state_mut().move_tag(self.drag, self.drag_offset);
        update_tag_rects(editor);

        response.on_hover_cursor(egui::CursorIcon::Grabbing)
    }

    fn get_toolbar_location_text(&self, _editor: &AnsiEditor) -> String {
        if let Some(pos) = self.cur {
            fl!(crate::LANGUAGE_LOADER, "toolbar-position", line = (pos.y + 1), column = (pos.x + 1))
        } else {
            String::new()
        }
    }

    fn handle_click(&mut self, _editor: &mut AnsiEditor, button: i32, _pos: Position, _pos_abs: Position, _response: &egui::Response) -> Option<Message> {
        if button == 1 {}
        None
    }
}

fn update_tag_rects(editor: &mut AnsiEditor) {
    let tags = editor.buffer_view.lock().get_buffer().tags.clone();
    let lock = &mut editor.buffer_view.lock();
    let overlays = lock.get_edit_state_mut().get_tool_overlay_mask_mut();
    overlays.clear();
    for tag in tags {
        overlays.add_rectangle(Rectangle::new(tag.position, (tag.len() as i32, 1).into()));
    }

    lock.get_edit_state_mut().set_is_buffer_dirty();
}
