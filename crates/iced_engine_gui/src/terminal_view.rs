#![allow(static_mut_refs)]
use crate::{MonitorSettings, Terminal};
use iced::Element;

#[derive(Debug, Clone)]
pub enum Message {
    Scroll(i32),
    OpenLink(String),
    Copy,
    Paste,
    RipCommand(String),

    SendMouseEvent(icy_engine::ansi::mouse_event::MouseEvent),
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
