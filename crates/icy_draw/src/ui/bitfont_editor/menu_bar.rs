//! BitFont Editor menu bar
//!
//! Menu structure is defined as data, then rendered to UI.
//! This allows hotkey handling and menu generation from a single source.

use iced::{
    Border, Element, Length, Theme, alignment,
    border::Radius,
    widget::{button, row, text},
};
use iced_aw::menu::{self, Menu};
use iced_aw::menu_bar;
use iced_aw::menu_items;
use iced_aw::style::{Status, menu_bar::primary};

use crate::fl;
use crate::ui::MostRecentlyUsedFiles;
use crate::ui::commands::bitfont_cmd;
use crate::ui::main_window::Message;
use crate::ui::menu::{build_recent_files_menu, menu_button, menu_item_style, menu_item_submenu, separator};
use icy_engine_gui::commands::{CommandDef, Hotkey, cmd, hotkey_from_iced};

use super::BitFontEditorMessage;

// ============================================================================
// Menu Item Data Structure
// ============================================================================

/// A menu item that can be rendered and checked for hotkeys
#[derive(Clone)]
pub enum MenuItem {
    /// Command-based item with hotkey support
    Command {
        cmd: &'static CommandDef,
        message: Message,
        enabled: bool,
        /// Optional dynamic label override (e.g., "Undo Clear Glyph")
        label_override: Option<String>,
    },
    /// Simple item without command (no hotkey)
    Simple {
        label: String,
        hotkey_display: String,
        message: Message,
        enabled: bool,
    },
    /// Separator line
    Separator,
}

impl MenuItem {
    /// Create a command-based menu item
    pub fn cmd(cmd: &'static CommandDef, message: Message) -> Self {
        Self::Command {
            cmd,
            message,
            enabled: true,
            label_override: None,
        }
    }

    /// Create a command item with dynamic label (e.g., for Undo/Redo)
    pub fn cmd_with_label(cmd: &'static CommandDef, message: Message, label: impl Into<String>) -> Self {
        Self::Command {
            cmd,
            message,
            enabled: true,
            label_override: Some(label.into()),
        }
    }

    /// Create a simple menu item without command
    pub fn simple(label: impl Into<String>, hotkey: impl Into<String>, message: Message) -> Self {
        Self::Simple {
            label: label.into(),
            hotkey_display: hotkey.into(),
            message,
            enabled: true,
        }
    }

    /// Create a separator
    pub fn separator() -> Self {
        Self::Separator
    }

    /// Set enabled state
    pub fn enabled(mut self, enabled: bool) -> Self {
        match &mut self {
            Self::Command { enabled: e, .. } => *e = enabled,
            Self::Simple { enabled: e, .. } => *e = enabled,
            _ => {}
        }
        self
    }

    /// Check if this item's hotkey matches the given hotkey
    pub fn matches_hotkey(&self, hotkey: &Hotkey) -> Option<Message> {
        match self {
            Self::Command {
                cmd, message, enabled: true, ..
            } => {
                if cmd.active_hotkeys().iter().any(|hk| hk == hotkey) {
                    Some(message.clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Get the command definition if this is a command item
    #[allow(dead_code)]
    pub fn command(&self) -> Option<&'static CommandDef> {
        match self {
            Self::Command { cmd, .. } => Some(*cmd),
            _ => None,
        }
    }

    /// Get the message for this item
    #[allow(dead_code)]
    pub fn message(&self) -> Option<&Message> {
        match self {
            Self::Command { message, .. } | Self::Simple { message, .. } => Some(message),
            Self::Separator => None,
        }
    }

    /// Check if enabled
    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        match self {
            Self::Command { enabled, .. } | Self::Simple { enabled, .. } => *enabled,
            Self::Separator => true,
        }
    }

    /// Get label (with optional override)
    pub fn label(&self) -> String {
        match self {
            Self::Command { cmd, label_override, .. } => label_override
                .clone()
                .unwrap_or_else(|| if cmd.label_menu.is_empty() { cmd.id.clone() } else { cmd.label_menu.clone() }),
            Self::Simple { label, .. } => label.clone(),
            Self::Separator => String::new(),
        }
    }

    /// Get hotkey display string
    pub fn hotkey_display(&self) -> String {
        match self {
            Self::Command { cmd, .. } => cmd.primary_hotkey_display().unwrap_or_default(),
            Self::Simple { hotkey_display, .. } => hotkey_display.clone(),
            Self::Separator => String::new(),
        }
    }
}

/// Render a menu item button
fn render_menu_item_enabled(label: String, hotkey: String, message: Message, enabled: bool) -> Element<'static, Message> {
    let mut btn = button(
        row![
            text(label).size(14).width(Length::Fill),
            text(hotkey).size(12).style(|theme: &Theme| {
                iced::widget::text::Style {
                    color: Some(theme.palette().text.scale_alpha(0.6)),
                }
            }),
        ]
        .spacing(16)
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .padding([4, 8])
    .style(menu_item_style);

    if enabled {
        btn = btn.on_press(message);
    }

    btn.into()
}

// ============================================================================
// Menu Definition
// ============================================================================

/// Menu definition for the BitFont editor
pub struct BitFontMenu {
    pub file: Vec<MenuItem>,
    pub edit: Vec<MenuItem>,
    pub selection: Vec<MenuItem>,
    pub view: Vec<MenuItem>,
    pub help: Vec<MenuItem>,
}

impl BitFontMenu {
    /// Create the menu structure with current state
    pub fn new(undo_desc: Option<&str>, redo_desc: Option<&str>) -> Self {
        let undo_label = match undo_desc {
            Some(desc) => format!("{} {}", cmd::EDIT_UNDO.label_menu, desc),
            None => cmd::EDIT_UNDO.label_menu.clone(),
        };
        let redo_label = match redo_desc {
            Some(desc) => format!("{} {}", cmd::EDIT_REDO.label_menu, desc),
            None => cmd::EDIT_REDO.label_menu.clone(),
        };

        Self {
            file: vec![
                MenuItem::cmd(&cmd::FILE_NEW, Message::NewFile),
                MenuItem::cmd(&cmd::FILE_OPEN, Message::OpenFile),
                MenuItem::simple(fl!("menu-import-font"), "", Message::ShowImportFontDialog),
                MenuItem::simple(fl!("menu-export-font"), "", Message::ShowExportFontDialog),
                // Recent files submenu handled separately in view
                MenuItem::separator(),
                MenuItem::cmd(&cmd::FILE_SAVE, Message::SaveFile),
                MenuItem::cmd(&cmd::FILE_SAVE_AS, Message::SaveFileAs),
                MenuItem::separator(),
                MenuItem::cmd(&cmd::SETTINGS_OPEN, Message::ShowSettings),
                MenuItem::separator(),
                MenuItem::cmd(&cmd::FILE_CLOSE, Message::CloseFile),
            ],
            edit: vec![
                MenuItem::cmd_with_label(&cmd::EDIT_UNDO, Message::Undo, undo_label).enabled(undo_desc.is_some()),
                MenuItem::cmd_with_label(&cmd::EDIT_REDO, Message::Redo, redo_label).enabled(redo_desc.is_some()),
                MenuItem::separator(),
                MenuItem::cmd(&cmd::EDIT_CUT, Message::Cut),
                MenuItem::cmd(&cmd::EDIT_COPY, Message::Copy),
                MenuItem::cmd(&cmd::EDIT_PASTE, Message::Paste),
                MenuItem::separator(),
                MenuItem::cmd(&bitfont_cmd::BITFONT_SWAP_CHARS, Message::BitFontEditor(BitFontEditorMessage::SwapChars)),
                MenuItem::cmd(
                    &bitfont_cmd::BITFONT_DUPLICATE_LINE,
                    Message::BitFontEditor(BitFontEditorMessage::DuplicateLine),
                ),
                MenuItem::separator(),
                MenuItem::simple(fl!("menu-set-font-size"), "", Message::BitFontEditor(BitFontEditorMessage::ShowFontSizeDialog)),
            ],
            selection: vec![
                MenuItem::cmd(&cmd::EDIT_SELECT_ALL, Message::BitFontEditor(BitFontEditorMessage::SelectAll)),
                MenuItem::simple(
                    fl!("menu-select_nothing"),
                    "Ctrl+D",
                    Message::BitFontEditor(BitFontEditorMessage::ClearSelection),
                ),
                MenuItem::separator(),
                MenuItem::cmd(&bitfont_cmd::BITFONT_CLEAR, Message::BitFontEditor(BitFontEditorMessage::Clear)),
                MenuItem::cmd(&bitfont_cmd::BITFONT_FILL, Message::BitFontEditor(BitFontEditorMessage::FillSelection)),
                MenuItem::cmd(&bitfont_cmd::BITFONT_INVERSE, Message::BitFontEditor(BitFontEditorMessage::Inverse)),
                MenuItem::separator(),
                MenuItem::cmd(&bitfont_cmd::BITFONT_FLIP_X, Message::BitFontEditor(BitFontEditorMessage::FlipX)),
                MenuItem::cmd(&bitfont_cmd::BITFONT_FLIP_Y, Message::BitFontEditor(BitFontEditorMessage::FlipY)),
            ],
            view: vec![
                MenuItem::cmd(
                    &bitfont_cmd::BITFONT_TOGGLE_LETTER_SPACING,
                    Message::BitFontEditor(BitFontEditorMessage::ToggleLetterSpacing),
                ),
                MenuItem::cmd(&bitfont_cmd::BITFONT_SHOW_PREVIEW, Message::BitFontEditor(BitFontEditorMessage::ShowPreview)),
                MenuItem::separator(),
                MenuItem::cmd(&cmd::VIEW_FULLSCREEN, Message::ToggleFullscreen),
            ],
            help: vec![
                MenuItem::simple(fl!("menu-discuss"), "", Message::OpenDiscussions),
                MenuItem::simple(fl!("menu-report-bug"), "", Message::ReportBug),
                MenuItem::separator(),
                MenuItem::cmd(&cmd::HELP_ABOUT, Message::ShowAbout),
            ],
        }
    }

    /// Check if any menu item matches the given hotkey
    pub fn handle_hotkey(&self, hotkey: &Hotkey) -> Option<Message> {
        for menu in [&self.file, &self.edit, &self.selection, &self.view, &self.help] {
            for item in menu {
                if let Some(msg) = item.matches_hotkey(hotkey) {
                    return Some(msg);
                }
            }
        }
        None
    }
}

/// Handle keyboard event by checking all BitFont menu commands
pub fn handle_command_event(event: &iced::Event, undo_desc: Option<&str>, redo_desc: Option<&str>) -> Option<Message> {
    let (key, modifiers) = match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => (key, *modifiers),
        _ => return None,
    };

    let hotkey = hotkey_from_iced(key, modifiers)?;
    let menu = BitFontMenu::new(undo_desc, redo_desc);
    menu.handle_hotkey(&hotkey)
}

// ============================================================================
// Menu View - Helper macros/functions to render MenuItem to iced_aw items
// ============================================================================

/// Render a MenuItem as an Element for use in menu_items!
fn menu_item_view(item: &MenuItem) -> Element<'static, Message> {
    match item {
        MenuItem::Command { message, enabled, .. } | MenuItem::Simple { message, enabled, .. } => {
            render_menu_item_enabled(item.label(), item.hotkey_display(), message.clone(), *enabled)
        }
        MenuItem::Separator => separator().into(),
    }
}

fn menu_items_from_slice(items: &[MenuItem]) -> Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>> {
    items.iter().map(|item| iced_aw::menu::Item::new(menu_item_view(item))).collect()
}

/// Build the BitFont editor menu bar from the menu data structure
pub fn view_bitfont(recent_files: &MostRecentlyUsedFiles, undo_desc: Option<&str>, redo_desc: Option<&str>) -> Element<'static, Message> {
    let menu = BitFontMenu::new(undo_desc, redo_desc);
    let menu_template = |items: Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>>| Menu::new(items).width(300.0).offset(5.0);

    // File menu: insert "Open Recent" submenu before the first separator.
    let mut file_items: Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>> = Vec::new();
    let mut inserted_recent = false;
    for item in &menu.file {
        if !inserted_recent {
            if let MenuItem::Separator = item {
                file_items.push(iced_aw::menu::Item::with_menu(
                    menu_item_submenu(fl!("menu-open_recent")),
                    build_recent_files_menu(recent_files),
                ));
                inserted_recent = true;
            }
        }

        file_items.push(iced_aw::menu::Item::new(menu_item_view(item)));
    }
    if !inserted_recent {
        file_items.push(iced_aw::menu::Item::with_menu(
            menu_item_submenu(fl!("menu-open_recent")),
            build_recent_files_menu(recent_files),
        ));
    }

    let edit_items = menu_items_from_slice(&menu.edit);
    let selection_items = menu_items_from_slice(&menu.selection);
    let view_items = menu_items_from_slice(&menu.view);
    let help_items = menu_items_from_slice(&menu.help);

    let mb = menu_bar!(
        // File menu - with special handling for recent files submenu
        (menu_button(fl!("menu-file")), menu_template(file_items)),
        // Edit menu
        (menu_button(fl!("menu-edit")), menu_template(edit_items)),
        // Selection menu
        (menu_button(fl!("menu-selection")), menu_template(selection_items)),
        // View menu
        (menu_button(fl!("menu-view")), menu_template(view_items)),
        // Help menu
        (menu_button(fl!("menu-help")), menu_template(help_items))
    )
    .spacing(4.0)
    .padding([4, 8])
    .draw_path(menu::DrawPath::Backdrop)
    .close_on_item_click_global(true)
    .close_on_background_click_global(true)
    .style(|theme: &Theme, status: Status| {
        let palette = theme.extended_palette();
        menu::Style {
            path_border: Border {
                radius: Radius::new(6.0),
                ..Default::default()
            },
            path: palette.primary.weak.color.into(),
            ..primary(theme, status)
        }
    });

    mb.into()
}
