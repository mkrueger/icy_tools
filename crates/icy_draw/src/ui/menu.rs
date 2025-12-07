//! Menu system for icy_draw
//!
//! Temporary simple menu bar until iced_aw version conflicts are resolved.
//! Will use iced_aw's MenuBar when available.

use iced::{Element, widget::{button, row, text}};

use super::main_window::Message;
use super::commands::create_draw_commands;
use icy_engine_gui::commands::CommandSet;

/// Menu builder that creates menu bars from commands
#[allow(dead_code)]
pub struct MenuBuilder {
    commands: CommandSet,
}

impl MenuBuilder {
    pub fn new() -> Self {
        Self {
            commands: create_draw_commands(),
        }
    }
    
    /// Get the hotkey display string for a command
    #[allow(dead_code)]
    fn hotkey_for(&self, cmd_id: &str) -> Option<String> {
        self.commands.get(cmd_id)?.primary_hotkey_display()
    }
    
    /// Build a simple button-based menu bar (temporary until iced_aw works)
    pub fn build(&self) -> Element<'_, Message> {
        row![
            button(text("New")).on_press(Message::NewFile).padding([4, 8]),
            button(text("Open")).on_press(Message::OpenFile).padding([4, 8]),
            button(text("Save")).on_press(Message::SaveFile).padding([4, 8]),
            text(" | ").size(16),
            button(text("Undo")).on_press(Message::Undo).padding([4, 8]),
            button(text("Redo")).on_press(Message::Redo).padding([4, 8]),
            text(" | ").size(16),
            button(text("Zoom +")).on_press(Message::ZoomIn).padding([4, 8]),
            button(text("Zoom -")).on_press(Message::ZoomOut).padding([4, 8]),
            button(text("Zoom 1:1")).on_press(Message::ZoomReset).padding([4, 8]),
        ]
        .spacing(4)
        .padding(4)
        .into()
    }
}

// TODO: Full iced_aw based menu when version conflicts are resolved:
// 
// use iced_aw::menu::{Item, Menu, MenuBar};
// 
// fn menu_item<'a>(label: &'a str, hotkey: Option<&'a str>, action: Message) -> Element<'a, Message> {
//     let content: Element<'a, Message> = if let Some(hk) = hotkey {
//         row![
//             text(label),
//             iced::widget::horizontal_space(),
//             text(hk).size(12),
//         ]
//         .spacing(20)
//         .into()
//     } else {
//         text(label).into()
//     };
//     
//     button(content)
//         .on_press(action)
//         .padding([4, 12])
//         .width(iced::Length::Fill)
//         .into()
// }

