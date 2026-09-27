use std::collections::VecDeque;

use eframe::egui;

use super::appearance::{labels, DialogButton, MessageBox as SharedMessageBox, MessageKind};

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
            ..Self::question(id, title, body, "")
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
        let kind = if self.error {
            MessageKind::Error
        } else if self.destructive {
            MessageKind::Warning
        } else {
            MessageKind::Question
        };
        let message = SharedMessageBox::new(self.id, kind, self.title, self.body).details(self.details);
        let message = if self.error {
            message.copyable().buttons([DialogButton::primary(labels::ok(), Response::Accept).cancels()])
        } else {
            let accept = if self.destructive {
                DialogButton::destructive(self.accept, Response::Accept)
            } else {
                DialogButton::primary(self.accept, Response::Accept)
            };
            let accept = accept.enabled(self.enabled);
            if self.save {
                // Save dialogs follow "Don't Save" | Cancel, Save.
                message.buttons([
                    accept.leading(),
                    DialogButton::cancel(tr!("egui-cancel"), Response::Cancel),
                    DialogButton::primary(tr!("egui-save"), Response::Save),
                ])
            } else {
                message.buttons([DialogButton::cancel(tr!("egui-cancel"), Response::Cancel), accept])
            }
        };
        let response = message.show(context);
        response.action.or(response.dismissed.then_some(Response::Cancel))
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
