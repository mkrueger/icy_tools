//! CharFont (TDF) Editor menu bar
//!
//! Menu structure is defined as data, then rendered to UI.
//! This allows hotkey handling and menu generation from a single source.

use iced::{Border, Element, Theme, border::Radius};
use iced_aw::menu::{self, Menu};
use iced_aw::style::{Status, menu_bar::primary};
use iced_aw::{menu_bar, menu_items};

use crate::MostRecentlyUsedFiles;
use crate::fl;
use crate::ui::editor::ansi::{AnsiEditorCoreMessage, AnsiEditorMessage};
use crate::ui::main_window::Message;
use crate::ui::main_window::menu::{
    MenuItem, UndoInfo, build_recent_files_menu, menu_button, menu_item, menu_item_redo, menu_item_simple, menu_item_simple_enabled, menu_item_submenu,
    menu_item_undo, separator,
};
use icy_engine_gui::commands::{Hotkey, cmd, hotkey_from_iced};

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
                MenuItem::submenu(
                    fl!("menu-import"),
                    vec![
                        MenuItem::simple(fl!("menu-import-font"), "", Message::ShowImportFontDialog),
                        MenuItem::simple(fl!("menu-import-fonts"), "", Message::CharFontEditor(super::CharFontEditorMessage::ImportFonts)),
                    ],
                ),
                MenuItem::submenu(
                    fl!("menu-export"),
                    vec![MenuItem::simple(
                        fl!("menu-export-font"),
                        "",
                        Message::CharFontEditor(super::CharFontEditorMessage::ExportFont),
                    )],
                ),
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
// Legacy view function (still used for rendering)
// ============================================================================

/// Build the CharFont (TDF) editor menu bar
pub fn view_charfont(recent_files: &MostRecentlyUsedFiles, undo_info: &UndoInfo) -> Element<'static, Message> {
    let menu_template = |items| Menu::new(items).width(300.0).offset(5.0);

    let close_editor_hotkey = cmd::WINDOW_CLOSE.primary_hotkey_display().unwrap_or_default();

    let mb = menu_bar!(
        // File menu
        (
            menu_button(fl!("menu-file")),
            menu_template(menu_items!(
                (menu_item(&cmd::FILE_NEW, Message::NewFile)),
                (menu_item(&cmd::FILE_OPEN, Message::OpenFile)),
                (menu_item_submenu(fl!("menu-open_recent")), build_recent_files_menu(recent_files)),
                (separator()),
                (menu_item(&cmd::FILE_SAVE, Message::SaveFile)),
                (menu_item(&cmd::FILE_SAVE_AS, Message::SaveFileAs)),
                (separator()),
                (
                    menu_item_submenu(fl!("menu-import")),
                    menu_template(menu_items!(
                        (menu_item_simple(fl!("menu-import-font"), Message::ShowImportFontDialog)),
                        (menu_item_simple(fl!("menu-import-fonts"), Message::CharFontEditor(super::CharFontEditorMessage::ImportFonts)))
                    ))
                ),
                (
                    menu_item_submenu(fl!("menu-export")),
                    menu_template(menu_items!(
                        (menu_item_simple(fl!("menu-export-font"), Message::CharFontEditor(super::CharFontEditorMessage::ExportFont)))
                    ))
                ),
                (separator()),
                (menu_item_simple(fl!("menu-connect-to-server"), Message::ShowConnectDialog)),
                (separator()),
                (menu_item(&cmd::SETTINGS_OPEN, Message::ShowSettings)),
                (separator()),
                (menu_item_simple_enabled(fl!("menu-close-editor"), close_editor_hotkey.as_str(), Message::CloseEditor, true)),
                (menu_item(&cmd::APP_QUIT, Message::QuitApp))
            ))
        ),
        // Edit menu (simplified - no SAUCE, no canvas size)
        (
            menu_button(fl!("menu-edit")),
            menu_template(menu_items!(
                (menu_item_undo(&cmd::EDIT_UNDO, Message::Undo, undo_info.undo_description.as_deref())),
                (menu_item_redo(&cmd::EDIT_REDO, Message::Redo, undo_info.redo_description.as_deref())),
                (separator()),
                (menu_item(&cmd::EDIT_CUT, Message::Cut)),
                (menu_item(&cmd::EDIT_COPY, Message::Copy)),
                (menu_item(&cmd::EDIT_PASTE, Message::Paste))
            ))
        ),
        // Colors menu (simplified - no palette selection)
        (
            menu_button(fl!("menu-colors")),
            menu_template(menu_items!(
                (menu_item_simple_enabled(
                    fl!("menu-next_fg_color"),
                    "Ctrl+Down",
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::NextFgColor)),
                    true
                )),
                (menu_item_simple_enabled(
                    fl!("menu-prev_fg_color"),
                    "Ctrl+Up",
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PrevFgColor)),
                    true
                )),
                (separator()),
                (menu_item_simple_enabled(
                    fl!("menu-next_bg_color"),
                    "Ctrl+Right",
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::NextBgColor)),
                    true
                )),
                (menu_item_simple_enabled(
                    fl!("menu-prev_bg_color"),
                    "Ctrl+Left",
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PrevBgColor)),
                    true
                )),
                (separator()),
                (menu_item_simple_enabled(
                    fl!("menu-toggle_color"),
                    "Alt+X",
                    Message::AnsiEditor(AnsiEditorMessage::ColorSwitcher(crate::ui::ColorSwitcherMessage::SwapColors)),
                    true
                )),
                (menu_item_simple(
                    fl!("menu-default_color"),
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SwitchToDefaultColor))
                ))
            ))
        ),
        // View menu (simplified - zoom and panels only)
        (
            menu_button(fl!("menu-view")),
            menu_template(menu_items!(
                (menu_item(&cmd::VIEW_ZOOM_RESET, Message::ZoomReset)),
                (menu_item(&cmd::VIEW_ZOOM_IN, Message::ZoomIn)),
                (menu_item(&cmd::VIEW_ZOOM_OUT, Message::ZoomOut)),
                (separator()),
                (menu_item_simple("4:1 400%".to_string(), Message::SetZoom(4.0))),
                (menu_item_simple("2:1 200%".to_string(), Message::SetZoom(2.0))),
                (menu_item_simple("1:1 100%".to_string(), Message::SetZoom(1.0))),
                (menu_item_simple("1:2 50%".to_string(), Message::SetZoom(0.5))),
                (menu_item_simple("1:4 25%".to_string(), Message::SetZoom(0.25))),
                (separator()),
                (menu_item(&cmd::VIEW_FULLSCREEN, Message::ToggleFullscreen))
            ))
        ),
        // Help menu
        (
            menu_button(fl!("menu-help")),
            menu_template(menu_items!(
                (menu_item_simple(fl!("menu-discuss"), Message::OpenDiscussions)),
                (menu_item_simple(fl!("menu-report-bug"), Message::ReportBug)),
                (separator()),
                (menu_item(&cmd::HELP_ABOUT, Message::ShowAbout))
            ))
        )
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
