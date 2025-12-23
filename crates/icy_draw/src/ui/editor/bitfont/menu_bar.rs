//! BitFont Editor menu bar
//!
//! Menu structure is defined as data, then rendered to UI.
//! This allows hotkey handling and menu generation from a single source.

use iced::{Border, Element, Theme};
use iced_aw::menu::{self, Menu};
use iced_aw::style::{Status, menu_bar::primary};
use iced_aw::{menu_bar, menu_items};

use crate::MostRecentlyUsedFiles;
use crate::fl;
use crate::ui::main_window::Message;
use crate::ui::main_window::commands::bitfont_cmd;
use crate::ui::main_window::menu::{MenuItem, MenuState, menu_button, menu_items_to_iced};
use icy_engine_gui::commands::{Hotkey, cmd, hotkey_from_iced};

use super::BitFontEditorMessage;

// ============================================================================
// Recent Files Submenu Builder
// ============================================================================

fn build_recent_files_items(state: &MenuState<'_>) -> Vec<MenuItem> {
    let files = state.recent_files.files();

    if files.is_empty() {
        vec![MenuItem::simple(fl!("menu-no_recent_files"), "", Message::Noop).enabled(false)]
    } else {
        let mut items: Vec<MenuItem> = files
            .iter()
            .rev()
            .map(|file| {
                let file_name = file
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| file.display().to_string());
                MenuItem::simple(file_name, "", Message::OpenRecentFile(file.clone()))
            })
            .collect();

        items.push(MenuItem::separator());
        items.push(MenuItem::simple(fl!("menu-clear_recent_files"), "", Message::ClearRecentFiles));
        items
    }
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
                MenuItem::dynamic_submenu(fl!("menu-open_recent"), build_recent_files_items),
                MenuItem::separator(),
                MenuItem::cmd(&cmd::FILE_SAVE, Message::SaveFile),
                MenuItem::cmd(&cmd::FILE_SAVE_AS, Message::SaveFileAs),
                MenuItem::separator(),
                MenuItem::simple(fl!("menu-export-font"), "", Message::BitFontEditor(BitFontEditorMessage::ShowExportFontDialog)),
                MenuItem::simple(fl!("menu-import-font"), "", Message::ShowImportFontDialog),
                MenuItem::separator(),
                MenuItem::simple(fl!("menu-connect-to-server"), "", Message::ShowConnectDialog),
                MenuItem::separator(),
                MenuItem::cmd(&cmd::SETTINGS_OPEN, Message::ShowSettings),
                MenuItem::separator(),
                MenuItem::cmd_with_label(&cmd::WINDOW_CLOSE, Message::CloseEditor, fl!("menu-close-editor")),
                MenuItem::cmd(&cmd::APP_QUIT, Message::QuitApp),
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
// Menu View
// ============================================================================

use iced::border::Radius;

/// Build the BitFont editor menu bar from the menu data structure
pub fn view_bitfont(recent_files: &MostRecentlyUsedFiles, undo_desc: Option<&str>, redo_desc: Option<&str>) -> Element<'static, Message> {
    let menu = BitFontMenu::new(undo_desc, redo_desc);

    let state = MenuState {
        recent_files,
        undo_description: undo_desc,
        redo_description: redo_desc,
    };

    let menu_template = |items: Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>>| Menu::new(items).width(300.0).offset(5.0);

    let file_items = menu_items_to_iced(&menu.file, &state);
    let edit_items = menu_items_to_iced(&menu.edit, &state);
    let selection_items = menu_items_to_iced(&menu.selection, &state);
    let view_items = menu_items_to_iced(&menu.view, &state);
    let help_items = menu_items_to_iced(&menu.help, &state);

    let mb = menu_bar!(
        (menu_button(fl!("menu-file")), menu_template(file_items)),
        (menu_button(fl!("menu-edit")), menu_template(edit_items)),
        (menu_button(fl!("menu-selection")), menu_template(selection_items)),
        (menu_button(fl!("menu-view")), menu_template(view_items)),
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
