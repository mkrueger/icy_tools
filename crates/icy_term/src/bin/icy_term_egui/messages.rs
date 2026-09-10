use std::collections::VecDeque;

use eframe::egui;

use super::appearance;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    Accept,
    Cancel,
    Save,
}

pub struct MessageBox<'a> {
    id: &'a str,
    title: &'a str,
    body: &'a str,
    details: &'a str,
    accept: &'a str,
    error: bool,
    destructive: bool,
    save: bool,
    enabled: bool,
}

impl<'a> MessageBox<'a> {
    pub fn question(id: &'a str, title: &'a str, body: &'a str, accept: &'a str) -> Self {
        Self {
            id,
            title,
            body,
            details: "",
            accept,
            error: false,
            destructive: false,
            save: false,
            enabled: true,
        }
    }

    pub fn error(id: &'a str, title: &'a str, body: &'a str) -> Self {
        Self {
            error: true,
            ..Self::question(id, title, body, "OK")
        }
    }

    pub fn details(mut self, details: &'a str) -> Self {
        self.details = details;
        self
    }
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }
    pub fn save_option(mut self) -> Self {
        self.save = true;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn show(self, context: &egui::Context) -> Option<Response> {
        let mut answer = None;
        let focus_id = egui::Id::new((self.id, "message-focus"));
        let first = !context.data(|data| data.get_temp::<bool>(focus_id).unwrap_or(false));
        let result = egui::Modal::new(egui::Id::new(self.id))
            .frame(appearance::dialog_frame(context))
            .show(context, |ui| {
                let width = (context.content_rect().width() - 48.0).clamp(220.0, 520.0);
                let text_height = |text: &str| {
                    ui.painter()
                        .layout(
                            text.to_owned(),
                            egui::TextStyle::Body.resolve(ui.style()),
                            ui.visuals().text_color(),
                            width - 44.0,
                        )
                        .size()
                        .y
                };
                let content_height = text_height(self.body) + if self.details.is_empty() { 0.0 } else { 8.0 + text_height(self.details) };
                let footer_space = if self.save && width < 400.0 { 142.0 } else { 112.0 };
                let height = (content_height + footer_space).clamp(140.0, (context.content_rect().height() - 64.0).clamp(140.0, 340.0));
                ui.set_width(width);
                ui.set_height(height);
                if appearance::dialog_header(ui, self.title) {
                    answer = Some(Response::Cancel);
                }
                egui::ScrollArea::vertical()
                    .id_salt((self.id, "body"))
                    .auto_shrink([false, false])
                    .min_scrolled_height(0.0)
                    .max_height((height - footer_space).max(0.0))
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            let color = if self.error {
                                ui.visuals().error_fg_color
                            } else if self.destructive {
                                ui.visuals().warn_fg_color
                            } else {
                                ui.visuals().selection.stroke.color
                            };
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
                            ui.painter().circle_stroke(rect.center(), 11.0, egui::Stroke::new(1.5, color));
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                if self.error || self.destructive { "!" } else { "?" },
                                egui::FontId::proportional(18.0),
                                color,
                            );
                            ui.vertical(|ui| {
                                ui.add(egui::Label::new(self.body).wrap().selectable(true));
                                if !self.details.is_empty() {
                                    ui.add_space(8.0);
                                    ui.add(egui::Label::new(egui::RichText::new(self.details).weak()).wrap().selectable(true));
                                }
                            });
                        });
                    });
                ui.separator();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center).with_main_wrap(true), |ui| {
                    if self.save {
                        if ui.add(appearance::primary_button(tr!("egui-save"))).clicked() {
                            answer = Some(Response::Save);
                        }
                    }
                    let accept = if self.destructive {
                        egui::Button::new(egui::RichText::new(self.accept).color(ui.visuals().error_fg_color))
                    } else {
                        appearance::primary_button(self.accept)
                    };
                    let accepted = ui.add_enabled(self.enabled, accept);
                    if accepted.clicked() {
                        answer = Some(Response::Accept);
                    }
                    if self.error {
                        if first {
                            accepted.request_focus();
                        }
                        if ui.button(tr!("terminal-menu-copy")).clicked() {
                            context.copy_text(format!("{}\n{}\n{}", self.title, self.body, self.details));
                        }
                    } else {
                        let cancel = ui.button(tr!("egui-cancel"));
                        if first {
                            cancel.request_focus();
                        }
                        if cancel.clicked() {
                            answer = Some(Response::Cancel);
                        }
                    }
                });
            });
        context.data_mut(|data| data.insert_temp(focus_id, true));
        if result.should_close() {
            answer = Some(Response::Cancel);
        }
        if answer.is_some() {
            context.data_mut(|data| data.remove::<bool>(focus_id));
        }
        if context.will_discard() {
            None
        } else {
            answer
        }
    }
}

#[derive(Default)]
pub struct Messages {
    errors: VecDeque<(String, String)>,
}

impl Messages {
    pub fn error(&mut self, title: String, detail: String) {
        if !self
            .errors
            .iter()
            .any(|(existing_title, existing_detail)| existing_title == &title && existing_detail == &detail)
        {
            self.errors.push_back((title, detail));
        }
    }

    pub fn is_open(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn show(&mut self, context: &egui::Context) {
        if let Some((title, detail)) = self.errors.front() {
            if MessageBox::error("application-message", title, detail).show(context).is_some() {
                self.errors.pop_front();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_errors_are_queued_once_without_losing_other_errors() {
        let mut messages = Messages::default();
        messages.error("Connection".into(), "Refused".into());
        messages.error("Connection".into(), "Refused".into());
        messages.error("Settings".into(), "Read-only".into());
        assert_eq!(messages.errors.len(), 2);
        assert!(messages.is_open());
    }
}
