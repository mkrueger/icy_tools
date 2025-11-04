#![allow(static_mut_refs)]
use iced::Element;

use crate::{MonitorSettings, Terminal};

#[derive(Debug, Clone)]
pub enum Message {
    Scroll(i32),
    OpenLink(String),
    Copy,
    RipCommand(String),
}

pub struct TerminalView<'a> {
    _term: &'a Terminal,
}

impl<'a> TerminalView<'a> {
    pub fn show_with_effects(term: &'a Terminal, settings: MonitorSettings) -> Element<'a, Message> {
        iced::widget::container(crate::terminal_shader::create_crt_shader(term, settings))
            .id(term.id.clone())
            .into()
    }
}
