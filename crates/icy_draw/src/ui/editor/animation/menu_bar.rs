//! Animation Editor menu bar
//!
//! Menu structure is defined as data, then rendered to UI.
//! This allows hotkey handling and menu generation from a single source.

use iced::{Border, Element, Theme, border::Radius};
use iced_aw::menu::{self, Menu};
use iced_aw::style::{Status, menu_bar::primary};
use iced_aw::{menu_bar, menu_items};

use crate::MostRecentlyUsedFiles;
use crate::fl;
use crate::ui::main_window::Message;
use crate::ui::main_window::menu::{MenuItem, MenuState, menu_button, menu_items_to_iced};
use icy_engine_gui::commands::{Hotkey, cmd, hotkey_from_iced};

// ============================================================================
// AnimationMenu - Unified menu definition for Animation editor
// ============================================================================

/// Menu definition for the Animation editor
/// Single source of truth for both menu display and keyboard handling
pub struct AnimationMenu {
    pub file: Vec<MenuItem>,
    pub edit: Vec<MenuItem>,
    pub help: Vec<MenuItem>,
}

impl AnimationMenu {
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
                // Recent files submenu handled dynamically in view
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
                MenuItem::simple(
                    fl!("menu-export"),
                    "",
                    Message::AnimationEditor(super::AnimationEditorMessage::ShowExportDialog),
                ),
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
                MenuItem::cmd(&cmd::EDIT_SELECT_ALL, Message::SelectAll),
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
        for menu in [&self.file, &self.edit, &self.help] {
            for item in menu {
                if let Some(msg) = item.matches_hotkey(hotkey) {
                    return Some(msg);
                }
            }
        }
        None
    }
}

/// Handle keyboard event by checking all Animation menu commands
pub fn handle_command_event(event: &iced::Event, undo_desc: Option<&str>, redo_desc: Option<&str>) -> Option<Message> {
    let (key, modifiers) = match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => (key, *modifiers),
        _ => return None,
    };

    let hotkey = hotkey_from_iced(key, modifiers)?;
    let menu = AnimationMenu::new(undo_desc, redo_desc);
    menu.handle_hotkey(&hotkey)
}

// ============================================================================
// Menu View
// ============================================================================

/// Build the Animation editor menu bar from the menu data structure
pub fn view_animation_menu(recent_files: &MostRecentlyUsedFiles, undo_desc: Option<&str>, redo_desc: Option<&str>) -> Element<'static, Message> {
    let menu = AnimationMenu::new(undo_desc, redo_desc);

    let state = MenuState {
        recent_files,
        undo_description: undo_desc,
        redo_description: redo_desc,
    };

    let menu_template = |items: Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>>| Menu::new(items).width(300.0).offset(5.0);

    let file_items = menu_items_to_iced(&menu.file, &state);
    let edit_items = menu_items_to_iced(&menu.edit, &state);
    let help_items = menu_items_to_iced(&menu.help, &state);

    let mb = menu_bar!(
        (menu_button(fl!("menu-file")), menu_template(file_items)),
        (menu_button(fl!("menu-edit")), menu_template(edit_items)),
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
