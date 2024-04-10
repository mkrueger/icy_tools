use eframe::{
    egui::{self},
    epaint::FontId,
};
use egui_modal::Modal;
use i18n_embed_fl::fl;

pub struct HelpDialog {}

pub enum Message {
    CloseDialog,
}

impl HelpDialog {
    pub fn new() -> Self {
        Self {}
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<Message> {
        let mut message = None;
        let modal = Modal::new(ctx, "protocol_modal");
        modal.show(|ui| {
            modal.title(ui, fl!(crate::LANGUAGE_LOADER, "help-dialog-title"));

            modal.frame(ui, |ui: &mut egui::Ui| {
                let help = fl!(crate::LANGUAGE_LOADER, "help-dialog-text");
                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut help.as_str())
                            .font(FontId::new(18.0, egui::FontFamily::Proportional))
                            .desired_rows(6)
                            .desired_width(f32::INFINITY),
                    );
                });
            });

            modal.buttons(ui, |ui| {
                if modal.button(ui, fl!(crate::LANGUAGE_LOADER, "button-ok")).clicked() {
                    message = Some(Message::CloseDialog);
                }
            });
        });
        modal.open();

        message
    }
}
