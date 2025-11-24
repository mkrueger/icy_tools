#![allow(static_mut_refs)]
use crate::{MonitorSettings, Terminal, create_crt_shader};
use iced::Element;
use icy_engine::MouseEvent;

#[derive(Debug, Clone)]
pub enum Message {
    OpenLink(String),
    Copy,
    Paste,
    /// The bool indicates whether to clear the RIP screen before sending the command
    RipCommand(bool, String),

    SendMouseEvent(MouseEvent),
    ScrollViewport(f32, f32), // dx, dy in pixels
}

pub struct TerminalView<'a> {
    _term: &'a Terminal,
}

impl<'a> TerminalView<'a> {
    pub fn show_with_effects(term: &'a Terminal, settings: MonitorSettings) -> Element<'a, Message> {
        iced::widget::container(create_crt_shader(term, settings)).id(term.id.clone()).into()
    }
}
