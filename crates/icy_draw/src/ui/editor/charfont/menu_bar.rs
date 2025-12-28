//! CharFont (TDF) Editor menu bar
//!
//! Menu structure is defined as data, then rendered to UI.
//! This allows hotkey handling and menu generation from a single source.

use iced::Element;
use iced::widget::menu::{bar as menu_bar_fn, root as menu_root, Tree as MenuTree};

use crate::fl;
use crate::ui::editor::ansi::{AnsiEditorCoreMessage, AnsiEditorMessage};
use crate::ui::main_window::menu::{menu_items_to_iced, MenuItem, MenuState};
use crate::ui::main_window::Message;
use crate::MostRecentlyUsedFiles;
use icy_engine_gui::commands::{cmd, hotkey_from_iced, Hotkey};

// ============================================================================
// CharFontMenu - Unified menu definition for CharFont editor
// ============================================================================

/// Menu definition for the CharFont (TDF) editor
/// Single source of truth for both menu display and keyboard handling
pub struct CharFontMenu {
    pub file: Vec<MenuItem>,
    pub edit: Vec<MenuItem>,
    pub colors: Vec<MenuItem>,
    pub view: Vec<MenuItem>,
    pub help: Vec<MenuItem>,
}

impl CharFontMenu {
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
                MenuItem::dynamic_submenu(fl!("menu-open_recent"), |state| {
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
                }),
                MenuItem::separator(),
                MenuItem::cmd(&cmd::FILE_SAVE, Message::SaveFile),
                MenuItem::cmd(&cmd::FILE_SAVE_AS, Message::SaveFileAs),
                MenuItem::separator(),
                MenuItem::simple(fl!("menu-export-font"), "", Message::CharFontEditor(super::CharFontEditorMessage::ExportFont)),
                MenuItem::simple(fl!("menu-import-font"), "", Message::ShowImportFontDialog),
                MenuItem::simple(fl!("menu-import-fonts"), "", Message::CharFontEditor(super::CharFontEditorMessage::ImportFonts)),
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
            ],
            colors: vec![
                MenuItem::simple(
                    fl!("menu-next_fg_color"),
                    "Ctrl+Down",
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::NextFgColor)),
                ),
                MenuItem::simple(
                    fl!("menu-prev_fg_color"),
                    "Ctrl+Up",
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PrevFgColor)),
                ),
                MenuItem::separator(),
                MenuItem::simple(
                    fl!("menu-next_bg_color"),
                    "Ctrl+Right",
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::NextBgColor)),
                ),
                MenuItem::simple(
                    fl!("menu-prev_bg_color"),
                    "Ctrl+Left",
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PrevBgColor)),
                ),
                MenuItem::separator(),
                MenuItem::simple(
                    fl!("menu-toggle_color"),
                    "Alt+X",
                    Message::AnsiEditor(AnsiEditorMessage::ColorSwitcher(crate::ui::ColorSwitcherMessage::SwapColors)),
                ),
                MenuItem::simple(
                    fl!("menu-default_color"),
                    "",
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SwitchToDefaultColor)),
                ),
            ],
            view: vec![
                MenuItem::cmd(&cmd::VIEW_ZOOM_RESET, Message::ZoomReset),
                MenuItem::cmd(&cmd::VIEW_ZOOM_IN, Message::ZoomIn),
                MenuItem::cmd(&cmd::VIEW_ZOOM_OUT, Message::ZoomOut),
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
        for menu in [&self.file, &self.edit, &self.colors, &self.view, &self.help] {
            for item in menu {
                if let Some(msg) = item.matches_hotkey(hotkey) {
                    return Some(msg);
                }
            }
        }
        None
    }
}

/// Handle keyboard event by checking all CharFont menu commands
pub fn handle_command_event(event: &iced::Event, undo_desc: Option<&str>, redo_desc: Option<&str>) -> Option<Message> {
    let (key, modifiers) = match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => (key, *modifiers),
        _ => return None,
    };

    let hotkey = hotkey_from_iced(key, modifiers)?;
    let menu = CharFontMenu::new(undo_desc, redo_desc);
    menu.handle_hotkey(&hotkey)
}

// ============================================================================
// Menu View
// ============================================================================

/// Build the CharFont (TDF) editor menu bar
pub fn view_charfont(recent_files: &MostRecentlyUsedFiles, undo_desc: Option<&str>, redo_desc: Option<&str>) -> Element<'static, Message> {
    let menu = CharFontMenu::new(undo_desc, redo_desc);

    let state = MenuState {
        recent_files,
        undo_description: undo_desc,
        redo_description: redo_desc,
    };

    let file_items = menu_items_to_iced(&menu.file, &state);
    let edit_items = menu_items_to_iced(&menu.edit, &state);
    let colors_items = menu_items_to_iced(&menu.colors, &state);
    let view_items = menu_items_to_iced(&menu.view, &state);
    let help_items = menu_items_to_iced(&menu.help, &state);

    let menu_roots = vec![
        MenuTree::with_children(menu_root(fl!("menu-file"), Message::Noop), file_items),
        MenuTree::with_children(menu_root(fl!("menu-edit"), Message::Noop), edit_items),
        MenuTree::with_children(menu_root(fl!("menu-colors"), Message::Noop), colors_items),
        MenuTree::with_children(menu_root(fl!("menu-view"), Message::Noop), view_items),
        MenuTree::with_children(menu_root(fl!("menu-help"), Message::Noop), help_items),
    ];

    menu_bar_fn(menu_roots)
        .spacing(4.0)
        .padding([4, 8])
        .into()
}
