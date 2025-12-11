use std::sync::Arc;

use eframe::egui::{self, WidgetText};
use egui::mutex::Mutex;

use crate::{Document, Message};

pub struct ToolTab {
    pub doc: Box<dyn ToolWindow>,
}
impl ToolTab {
    pub(crate) fn new<T: 'static + ToolWindow>(tool_window: T) -> Self {
        Self { doc: Box::new(tool_window) }
    }
}

#[derive(Default)]
pub struct ToolBehavior {
    pub active_document: Option<Arc<Mutex<Box<dyn Document>>>>,
    pub message: Option<Message>,
}

impl egui_tiles::Behavior<ToolTab> for ToolBehavior {
    fn tab_title_for_pane(&mut self, pane: &ToolTab) -> egui::WidgetText {
        let title = pane.doc.title();

        WidgetText::RichText(Arc::new(egui::RichText::new(title).small()))
    }
    fn pane_ui(&mut self, ui: &mut egui::Ui, _tile_id: egui_tiles::TileId, pane: &mut ToolTab) -> egui_tiles::UiResponse {
        let message = pane.doc.show_ui(ui, self.active_document.clone());
        if self.message.is_none() {
            self.message = message;
        }
        egui_tiles::UiResponse::None
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            all_panes_must_have_tabs: true,
            ..Default::default()
        }
    }
}

pub trait ToolWindow {
    fn title(&self) -> String;

    fn show_ui(&mut self, ui: &mut egui::Ui, active_document: Option<Arc<Mutex<Box<dyn Document>>>>) -> Option<Message>;
}
