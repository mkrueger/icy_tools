use std::sync::Arc;

use eframe::{
    egui::{self, RichText, Sense, TextStyle, TopBottomPanel},
    emath::Align2,
    epaint::{Color32, Rect, Vec2},
};
use egui::{CornerRadius, Image, mutex::Mutex};
use i18n_embed_fl::fl;
use icy_engine_gui::BufferView;

use crate::{AnsiEditor, Document, INVISIBLE_SVG, Message, ToolWindow, VISIBLE_SVG};

pub struct LayerToolWindow {
    gl: Arc<glow::Context>,
    view_cache_id: usize,
    stack_len: usize,
    view_cache: Vec<Arc<eframe::epaint::mutex::Mutex<BufferView>>>,
}

impl LayerToolWindow {
    pub(crate) fn new(gl: Arc<glow::Context>) -> Self {
        Self {
            gl,
            view_cache: Vec::new(),
            view_cache_id: usize::MAX,
            stack_len: usize::MAX,
        }
    }

    pub fn get_buffer_view(&mut self, i: usize) -> Arc<eframe::epaint::mutex::Mutex<BufferView>> {
        while self.view_cache.len() <= i {
            let mut buffer_view = BufferView::new(&self.gl);
            buffer_view.interactive = false;
            buffer_view.get_buffer_mut().is_terminal_buffer = false;
            buffer_view.get_caret_mut().set_is_visible(false);
            self.view_cache.push(Arc::new(eframe::epaint::mutex::Mutex::new(buffer_view)));
        }

        self.view_cache[i].clone()
    }

    fn show_layer_view(&mut self, ui: &mut egui::Ui, editor: &AnsiEditor) -> Option<Message> {
        let row_height = 48.0;
        let mut result = None;

        let max = editor.buffer_view.lock().get_buffer().layers.len();
        let Ok(cur_layer) = editor.get_cur_layer_index() else {
            log::error!("Invalid layer index");
            return result;
        };

        let paste_mode = editor.buffer_view.lock().get_buffer().layers.iter().position(|layer| layer.role.is_paste());

        TopBottomPanel::bottom("layer_bottom").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.spacing_mut().item_spacing = eframe::epaint::Vec2::new(0.0, 0.0);

                if paste_mode.is_some() {
                    let r = medium_hover_button(ui, &crate::ADD_LAYER_SVG).on_hover_ui(|ui| {
                        ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "add_layer_tooltip")).small());
                    });

                    if r.clicked() {
                        result = Some(Message::AddFloatingLayer);
                    }

                    if let Some(layer) = editor.buffer_view.lock().get_edit_state().get_cur_layer() {
                        let role = layer.role;
                        if matches!(role, icy_engine::Role::PastePreview) {
                            let r = medium_hover_button(ui, &crate::ANCHOR_SVG).on_hover_ui(|ui| {
                                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "anchor_layer_tooltip")).small());
                            });

                            if r.clicked() && cur_layer < max {
                                result = Some(Message::AnchorLayer);
                            }
                        }
                    }

                    let r = medium_hover_button(ui, &crate::DELETE_SVG).on_hover_ui(|ui| {
                        ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "delete_layer_tooltip")).small());
                    });

                    if r.clicked() && cur_layer < max {
                        result = Some(Message::RemoveFloatingLayer);
                    }
                } else {
                    let r = medium_hover_button(ui, &crate::ADD_LAYER_SVG).on_hover_ui(|ui| {
                        ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "add_layer_tooltip")).small());
                    });

                    if r.clicked() {
                        result = Some(Message::AddNewLayer(cur_layer));
                    }

                    let r = medium_hover_button(ui, &crate::MOVE_UP_SVG).on_hover_ui(|ui| {
                        ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "move_layer_up_tooltip")).small());
                    });

                    if r.clicked() {
                        result = Some(Message::RaiseLayer(cur_layer));
                    }

                    let r = medium_hover_button(ui, &crate::MOVE_DOWN_SVG).on_hover_ui(|ui| {
                        ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "move_layer_down_tooltip")).small());
                    });

                    if r.clicked() {
                        result = Some(Message::LowerLayer(cur_layer));
                    }

                    let r = medium_hover_button(ui, &crate::DELETE_SVG).on_hover_ui(|ui| {
                        ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "delete_layer_tooltip")).small());
                    });

                    if r.clicked() && cur_layer < max {
                        result = Some(Message::RemoveLayer(cur_layer));
                    }
                }
            });
        });

        let redraw_layer_views = self.view_cache_id != editor.buffer_view.lock().id || editor.undo_stack_len() != self.stack_len;
        if redraw_layer_views {
            self.view_cache_id = editor.buffer_view.lock().id;
            self.stack_len = editor.undo_stack_len();
        }

        egui::ScrollArea::vertical().id_salt("layer_view_scroll_area").show(ui, |ui| {
            for i in (0..max).rev() {
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    let dims = editor.buffer_view.lock().get_buffer().get_font_dimensions();
                    let size = dims.height as f32 * 25.0;
                    let scale = row_height / size;

                    ui.allocate_ui(Vec2::new(scale * dims.width as f32 * 80.0, row_height), |ui| {
                        let opt = icy_engine_gui::TerminalOptions {
                            filter: glow::LINEAR as i32,
                            stick_to_bottom: false,
                            scale: Some(Vec2::new(scale, scale)),
                            use_terminal_height: false,
                            hide_scrollbars: true,
                            clip_rect: Some(ui.clip_rect()),
                            ..Default::default()
                        };
                        let view = self.get_buffer_view(i);
                        if redraw_layer_views {
                            view.lock().get_buffer_mut().layers.clear();
                            let width = editor.buffer_view.lock().get_width();
                            view.lock().get_buffer_mut().set_width(width);
                            let lock = &editor.buffer_view.lock();
                            if let Some(layer) = lock.get_buffer().layers.get(i) {
                                let mut l = layer.clone();
                                l.set_is_visible(true);
                                view.lock().get_buffer_mut().set_font_table(lock.get_buffer().get_font_table());
                                view.lock().get_buffer_mut().palette = lock.get_buffer().palette.clone();
                                view.lock().get_buffer_mut().layers.push(l);
                                view.lock().get_edit_state_mut().set_is_buffer_dirty();
                            }
                        }

                        let (_, _) = icy_engine_gui::show_terminal_area(ui, view, opt);
                    });

                    let (is_visible, title, color) = {
                        let lock = editor.buffer_view.lock();
                        let layer = &lock.get_buffer().layers[i];
                        (layer.get_is_visible(), layer.get_title().to_string(), layer.properties.color.clone())
                    };
                    let width = ui.available_width();

                    let (id, back_rect) = ui.allocate_space(Vec2::new(width, row_height));
                    let mut response = ui.interact(back_rect, id, Sense::click());

                    let back_painter = ui.painter_at(back_rect);

                    if response.hovered() {
                        back_painter.rect_filled(back_rect, CornerRadius::ZERO, ui.style().visuals.widgets.active.bg_fill);
                    } else if i == cur_layer {
                        back_painter.rect_filled(back_rect, CornerRadius::ZERO, ui.style().visuals.extreme_bg_color);
                    }

                    let stroke_rect = Rect::from_min_size(back_rect.min + Vec2::new(0.0, (row_height - 22.0) / 2.0), Vec2::new(22.0, 22.0));
                    let visible_icon_response = ui.interact(stroke_rect, id.with("visible"), Sense::click());

                    let painter = ui.painter_at(stroke_rect);

                    if let Some(color) = color {
                        let (r, g, b) = color.into();
                        painter.rect_filled(stroke_rect, CornerRadius::ZERO, Color32::from_rgb(r, g, b));
                    }

                    let image: Image<'static> = if is_visible { VISIBLE_SVG.clone() } else { INVISIBLE_SVG.clone() };

                    let tint = if i == cur_layer {
                        ui.visuals().widgets.active.fg_stroke.color
                    } else {
                        ui.visuals().widgets.inactive.fg_stroke.color
                    };
                    let image = image.tint(tint);
                    image.paint_at(ui, stroke_rect);

                    let color = if i == cur_layer {
                        ui.style().visuals.strong_text_color()
                    } else {
                        ui.style().visuals.text_color()
                    };
                    let font_id = TextStyle::Button.resolve(ui.style());

                    back_painter.text(stroke_rect.right_center() + Vec2::new(4., 0.), Align2::LEFT_CENTER, title, font_id, color);

                    if visible_icon_response.clicked() {
                        result = Some(Message::ToggleLayerVisibility(i));
                    }

                    if paste_mode.is_none() {
                        let response_opt = response.context_menu(|ui| {
                            ui.set_width(250.);
                            if ui.button(fl!(crate::LANGUAGE_LOADER, "layer_tool_menu_layer_properties")).clicked() {
                                result = Some(Message::EditLayer(i));
                                ui.close_kind(egui::UiKind::Menu);
                            }
                            if ui.button(fl!(crate::LANGUAGE_LOADER, "layer_tool_menu_resize_layer")).clicked() {
                                result = Some(Message::ResizeLayer(i));
                                ui.close_kind(egui::UiKind::Menu);
                            }
                            ui.separator();
                            if ui.button(fl!(crate::LANGUAGE_LOADER, "layer_tool_menu_new_layer")).clicked() {
                                result = Some(Message::AddNewLayer(i));
                                ui.close_kind(egui::UiKind::Menu);
                            }
                            if ui.button(fl!(crate::LANGUAGE_LOADER, "layer_tool_menu_duplicate_layer")).clicked() {
                                result = Some(Message::DuplicateLayer(i));
                                ui.close_kind(egui::UiKind::Menu);
                            }
                            if ui.button(fl!(crate::LANGUAGE_LOADER, "layer_tool_menu_merge_layer")).clicked() {
                                result = Some(Message::MergeLayerDown(i));
                                ui.close_kind(egui::UiKind::Menu);
                            }
                            if ui.button(fl!(crate::LANGUAGE_LOADER, "layer_tool_menu_delete_layer")).clicked() {
                                result = Some(Message::RemoveLayer(i));
                                ui.close_kind(egui::UiKind::Menu);
                            }
                            ui.separator();

                            if ui.button(fl!(crate::LANGUAGE_LOADER, "layer_tool_menu_clear_layer")).clicked() {
                                result = Some(Message::ClearLayer(i));
                                ui.close_kind(egui::UiKind::Menu);
                            }
                        });
                        if let Some(response_opt) = response_opt {
                            response = response_opt.response;
                        }
                    }

                    if paste_mode.is_none() && response.clicked() {
                        result = Some(Message::SelectLayer(i));
                    }

                    if paste_mode.is_none() && response.double_clicked() {
                        result = Some(Message::EditLayer(i));
                    }
                });
            }
        });

        result
    }
}

impl ToolWindow for LayerToolWindow {
    fn get_title(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "layer_tool_title")
    }

    fn show_ui(&mut self, ui: &mut egui::Ui, active_document: Option<Arc<Mutex<Box<dyn Document>>>>) -> Option<Message> {
        if let Some(doc) = active_document {
            if let Some(editor) = doc.lock().get_ansi_editor() {
                return self.show_layer_view(ui, editor);
            }
        }
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "no_document_selected")).small());
        });
        None
    }
}

pub fn medium_hover_button(ui: &mut egui::Ui, image: &Image<'_>) -> egui::Response {
    let size_points = egui::Vec2::splat(28.0);

    let (id, rect) = ui.allocate_space(size_points);
    let response = ui.interact(rect, id, Sense::click());

    let tint = if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(4), ui.style().visuals.extreme_bg_color);

        ui.visuals().widgets.active.fg_stroke.color
    } else {
        ui.visuals().widgets.inactive.fg_stroke.color
    };

    let image = image.clone().tint(tint);
    image.paint_at(ui, rect.shrink(4.0));

    response
}
