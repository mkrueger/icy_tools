use std::sync::Arc;

use eframe::egui::{self, RichText};
use egui::mutex::Mutex;
use i18n_embed_fl::fl;

use crate::{Document, Message, ToolWindow};

#[derive(Default)]
pub struct ChannelToolWindow {}

impl ToolWindow for ChannelToolWindow {
    fn title(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "channel_tool_title")
    }

    fn show_ui(&mut self, ui: &mut egui::Ui, active_document: Option<Arc<Mutex<Box<dyn Document>>>>) -> Option<Message> {
        if let Some(doc) = active_document {
            if let Some(editor) = doc.lock().get_ansi_editor() {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    if ui
                        .checkbox(&mut editor.buffer_view.lock().use_fg, fl!(crate::LANGUAGE_LOADER, "channel_tool_fg"))
                        .changed()
                    {
                        editor.buffer_view.lock().redraw_view();
                    }
                });
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    if ui
                        .checkbox(&mut editor.buffer_view.lock().use_bg, fl!(crate::LANGUAGE_LOADER, "channel_tool_bg"))
                        .changed()
                    {
                        editor.buffer_view.lock().redraw_view();
                    }
                });
            }
        } else {
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "no_document_selected")).small());
            });
        }
        None
    }
}
