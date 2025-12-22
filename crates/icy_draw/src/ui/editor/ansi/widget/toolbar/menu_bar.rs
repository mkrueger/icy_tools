//! ANSI Editor menu bar
//!
//! Menu structure is defined as data, then rendered to UI.
//! This allows hotkey handling and menu generation from a single source.

use iced::{
    Border, Element, Length, Theme,
    border::Radius,
    widget::{button, text},
};
use iced_aw::menu::{self, Menu};
use iced_aw::style::{Status, menu_bar::primary};
use iced_aw::{menu_bar, menu_items};

use crate::Plugin;
use crate::fl;
use crate::ui::editor::ansi::{AnsiEditorCoreMessage, AnsiEditorMessage};
use crate::ui::main_window::Message;
use crate::ui::main_window::commands::area_cmd;
use crate::ui::main_window::commands::selection_cmd;
use crate::ui::main_window::menu::{
    MenuItem, UndoInfo, build_recent_files_menu, menu_button, menu_item, menu_item_checkbox, menu_item_redo, menu_item_simple, menu_item_simple_enabled,
    menu_item_style, menu_item_submenu, menu_item_undo, separator,
};
use crate::{MostRecentlyUsedFiles, ui::PaletteGridMessage};
use icy_engine_gui::commands::{Hotkey, cmd, hotkey_from_iced};

/// Current state of guides/raster for menu display
#[derive(Clone, Debug, Default)]
pub struct MarkerMenuState {
    /// Currently selected guide (column, row), or None if off
    pub guide: Option<(u32, u32)>,
    /// Is guide visibility enabled
    pub guide_visible: bool,
    /// Currently selected raster (width, height), or None if off  
    pub raster: Option<(u32, u32)>,
    /// Is raster visibility enabled
    pub raster_visible: bool,
    /// Is line numbers visible
    pub line_numbers_visible: bool,
    /// Is layer borders visible
    pub layer_borders_visible: bool,
}

// ============================================================================
// AnsiMenu - Unified menu definition for ANSI editor
// ============================================================================

/// Menu definition for the ANSI editor
/// Single source of truth for both menu display and keyboard handling
pub struct AnsiMenu {
    pub file: Vec<MenuItem>,
    pub edit: Vec<MenuItem>,
    pub selection: Vec<MenuItem>,
    pub colors: Vec<MenuItem>,
    pub view: Vec<MenuItem>,
    pub help: Vec<MenuItem>,
}

impl AnsiMenu {
    /// Create the menu structure with current state
    pub fn new(undo_desc: Option<&str>, redo_desc: Option<&str>, mirror_mode: bool) -> Self {
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
                MenuItem::cmd(&cmd::FILE_EXPORT, Message::ExportFile),
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
                MenuItem::submenu(
                    fl!("menu-paste-as"),
                    vec![MenuItem::simple(fl!("menu-paste-as-new-image"), "", Message::PasteAsNewImage)],
                ),
                MenuItem::simple(fl!("menu-insert-sixel-from-file"), "", Message::InsertSixelFromFile),
                MenuItem::separator(),
                // Area operations submenu
                MenuItem::submenu(
                    fl!("menu-area_operations"),
                    vec![
                        MenuItem::cmd(
                            &area_cmd::JUSTIFY_LINE_LEFT,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLineLeft)),
                        ),
                        MenuItem::cmd(
                            &area_cmd::JUSTIFY_LINE_CENTER,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLineCenter)),
                        ),
                        MenuItem::cmd(
                            &area_cmd::JUSTIFY_LINE_RIGHT,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLineRight)),
                        ),
                        MenuItem::separator(),
                        MenuItem::cmd(
                            &area_cmd::INSERT_ROW,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::InsertRow)),
                        ),
                        MenuItem::cmd(
                            &area_cmd::DELETE_ROW,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::DeleteRow)),
                        ),
                        MenuItem::cmd(
                            &area_cmd::INSERT_COLUMN,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::InsertColumn)),
                        ),
                        MenuItem::cmd(
                            &area_cmd::DELETE_COLUMN,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::DeleteColumn)),
                        ),
                        MenuItem::separator(),
                        MenuItem::cmd(
                            &area_cmd::ERASE_ROW,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseRow)),
                        ),
                        MenuItem::cmd(
                            &area_cmd::ERASE_ROW_TO_START,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseRowToStart)),
                        ),
                        MenuItem::cmd(
                            &area_cmd::ERASE_ROW_TO_END,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseRowToEnd)),
                        ),
                        MenuItem::cmd(
                            &area_cmd::ERASE_COLUMN,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseColumn)),
                        ),
                        MenuItem::cmd(
                            &area_cmd::ERASE_COLUMN_TO_START,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseColumnToStart)),
                        ),
                        MenuItem::cmd(
                            &area_cmd::ERASE_COLUMN_TO_END,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseColumnToEnd)),
                        ),
                        MenuItem::separator(),
                        MenuItem::cmd(
                            &area_cmd::SCROLL_UP,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ScrollAreaUp)),
                        ),
                        MenuItem::cmd(
                            &area_cmd::SCROLL_DOWN,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ScrollAreaDown)),
                        ),
                        MenuItem::cmd(
                            &area_cmd::SCROLL_LEFT,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ScrollAreaLeft)),
                        ),
                        MenuItem::cmd(
                            &area_cmd::SCROLL_RIGHT,
                            Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ScrollAreaRight)),
                        ),
                    ],
                ),
                MenuItem::separator(),
                MenuItem::simple(fl!("menu-open_font_selector"), "", Message::AnsiEditor(AnsiEditorMessage::OpenFontSelector)),
                MenuItem::separator(),
                MenuItem::toggle(
                    fl!("menu-mirror_mode"),
                    "",
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleMirrorMode)),
                    mirror_mode,
                ),
                MenuItem::separator(),
                MenuItem::simple(fl!("menu-file-settings"), "", Message::ShowFileSettingsDialog),
            ],
            selection: vec![
                MenuItem::cmd(&cmd::EDIT_SELECT_ALL, Message::SelectAll),
                MenuItem::cmd(&selection_cmd::SELECT_NONE, Message::Deselect),
                MenuItem::cmd(&selection_cmd::SELECT_INVERSE, Message::AnsiEditor(AnsiEditorMessage::InverseSelection)),
                MenuItem::separator(),
                MenuItem::cmd(
                    &selection_cmd::SELECT_FLIP_X,
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::FlipX)),
                ),
                MenuItem::cmd(
                    &selection_cmd::SELECT_FLIP_Y,
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::FlipY)),
                ),
                MenuItem::cmd(
                    &selection_cmd::SELECT_JUSTIFY_CENTER,
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyCenter)),
                ),
                MenuItem::cmd(
                    &selection_cmd::SELECT_JUSTIFY_LEFT,
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLeft)),
                ),
                MenuItem::cmd(
                    &selection_cmd::SELECT_JUSTIFY_RIGHT,
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyRight)),
                ),
                MenuItem::cmd(
                    &selection_cmd::SELECT_CROP,
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::Crop)),
                ),
            ],
            colors: vec![
                MenuItem::simple(fl!("menu-edit_palette"), "", Message::AnsiEditor(AnsiEditorMessage::EditPalette)),
                MenuItem::separator(),
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
                    fl!("menu-pick_attribute_under_caret"),
                    "Alt+U",
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PickAttributeUnderCaret)),
                ),
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
                // View items are dynamic based on marker state, handled separately
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
        for menu in [&self.file, &self.edit, &self.selection, &self.colors, &self.view, &self.help] {
            for item in menu {
                if let Some(msg) = item.matches_hotkey(hotkey) {
                    return Some(msg);
                }
            }
        }
        None
    }
}

/// Handle keyboard event by checking all ANSI menu commands
pub fn handle_command_event(event: &iced::Event, undo_desc: Option<&str>, redo_desc: Option<&str>, mirror_mode: bool) -> Option<Message> {
    let (key, modifiers) = match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => (key, *modifiers),
        _ => return None,
    };

    let hotkey = hotkey_from_iced(key, modifiers)?;
    let menu = AnsiMenu::new(undo_desc, redo_desc, mirror_mode);
    menu.handle_hotkey(&hotkey)
}

// ============================================================================
// Legacy view functions (still used for rendering, but hotkeys now handled by AnsiMenu)
// ============================================================================

/// Build the guides submenu with predefined guide sizes
fn build_guides_submenu(state: &MarkerMenuState) -> Menu<'static, Message, Theme, iced::Renderer> {
    use iced::widget::text;

    // Predefined guide sizes (common ANSI art formats)
    let guides: [(&str, i32, i32); 4] = [
        ("Smallscale 80x25", 80, 25),
        ("Square 80x40", 80, 40),
        ("Instagram 80x50", 80, 50),
        ("File_ID.DIZ 44x22", 44, 22),
    ];

    let mut items: Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>> = Vec::new();

    // Off option
    let off_selected = state.guide.is_none();
    let off_label = if off_selected { "● Off" } else { "   Off" };
    items.push(iced_aw::menu::Item::new(
        button(text(off_label).size(14))
            .width(Length::Fill)
            .padding([4, 8])
            .style(menu_item_style)
            .on_press(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ClearGuide))),
    ));

    items.push(iced_aw::menu::Item::new(separator()));

    for (name, x, y) in guides {
        let is_selected = state.guide == Some((x as u32, y as u32));
        let label = if is_selected { format!("● {}", name) } else { format!("   {}", name) };
        items.push(iced_aw::menu::Item::new(
            button(text(label).size(14))
                .width(Length::Fill)
                .padding([4, 8])
                .style(menu_item_style)
                .on_press(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SetGuide(x, y)))),
        ));
    }

    // Add separator and visibility toggle
    items.push(iced_aw::menu::Item::new(separator()));

    let visibility_label = if state.guide_visible {
        format!("☑ {}", fl!("menu-toggle_guide"))
    } else {
        format!("☐ {}", fl!("menu-toggle_guide"))
    };
    items.push(iced_aw::menu::Item::new(
        button(text(visibility_label).size(14))
            .width(Length::Fill)
            .padding([4, 8])
            .style(menu_item_style)
            .on_press(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleGuide))),
    ));

    Menu::new(items).width(200.0).offset(5.0)
}

/// Build the raster/grid submenu with predefined grid sizes
fn build_raster_submenu(state: &MarkerMenuState) -> Menu<'static, Message, Theme, iced::Renderer> {
    use iced::widget::text;

    // Predefined raster sizes
    let rasters: [(i32, i32); 10] = [(1, 1), (2, 2), (4, 2), (4, 4), (8, 2), (8, 4), (8, 8), (16, 4), (16, 8), (16, 16)];

    let mut items: Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>> = Vec::new();

    // Off option
    let off_selected = state.raster.is_none();
    let off_label = if off_selected { "● Off" } else { "   Off" };
    items.push(iced_aw::menu::Item::new(
        button(text(off_label).size(14))
            .width(Length::Fill)
            .padding([4, 8])
            .style(menu_item_style)
            .on_press(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ClearRaster))),
    ));

    items.push(iced_aw::menu::Item::new(separator()));

    for (x, y) in rasters {
        let is_selected = state.raster == Some((x as u32, y as u32));
        let label = if is_selected { format!("● {}x{}", x, y) } else { format!("   {}x{}", x, y) };
        items.push(iced_aw::menu::Item::new(
            button(text(label).size(14))
                .width(Length::Fill)
                .padding([4, 8])
                .style(menu_item_style)
                .on_press(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::SetRaster(x, y)))),
        ));
    }

    // Add separator and visibility toggle
    items.push(iced_aw::menu::Item::new(separator()));

    let visibility_label = if state.raster_visible {
        format!("☑ {}", fl!("menu-toggle_raster"))
    } else {
        format!("☐ {}", fl!("menu-toggle_raster"))
    };
    items.push(iced_aw::menu::Item::new(
        button(text(visibility_label).size(14))
            .width(Length::Fill)
            .padding([4, 8])
            .style(menu_item_style)
            .on_press(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleRaster))),
    ));

    Menu::new(items).width(150.0).offset(5.0)
}

/// Build the zoom submenu with zoom levels
fn build_zoom_submenu() -> Menu<'static, Message, Theme, iced::Renderer> {
    use icy_engine_gui::commands::cmd;

    let mut items: Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>> = Vec::new();

    // Zoom commands
    items.push(iced_aw::menu::Item::new(menu_item(&cmd::VIEW_ZOOM_RESET, Message::ZoomReset)));
    items.push(iced_aw::menu::Item::new(menu_item(&cmd::VIEW_ZOOM_IN, Message::ZoomIn)));
    items.push(iced_aw::menu::Item::new(menu_item(&cmd::VIEW_ZOOM_OUT, Message::ZoomOut)));
    items.push(iced_aw::menu::Item::new(separator()));

    // Preset zoom levels
    for (label, zoom) in [("4:1 400%", 4.0), ("2:1 200%", 2.0), ("1:1 100%", 1.0), ("1:2 50%", 0.5), ("1:4 25%", 0.25)] {
        items.push(iced_aw::menu::Item::new(
            button(text(label).size(14))
                .width(Length::Fill)
                .padding([4, 8])
                .style(menu_item_style)
                .on_press(Message::SetZoom(zoom)),
        ));
    }

    Menu::new(items).width(200.0).offset(5.0)
}

/// Build the view menu with conditional chat panel visibility
fn build_view_menu(
    marker_state: &MarkerMenuState,
    guides_submenu: Menu<'static, Message, Theme, iced::Renderer>,
    raster_submenu: Menu<'static, Message, Theme, iced::Renderer>,
    is_connected: bool,
) -> Menu<'static, Message, Theme, iced::Renderer> {
    let mut items: Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>> = Vec::new();

    // Zoom submenu
    items.push(iced_aw::menu::Item::with_menu(menu_item_submenu(fl!("menu-zoom")), build_zoom_submenu()));

    items.push(iced_aw::menu::Item::new(separator()));

    // Guide and raster submenus
    items.push(iced_aw::menu::Item::with_menu(menu_item_submenu(fl!("menu-guides")), guides_submenu));
    items.push(iced_aw::menu::Item::with_menu(menu_item_submenu(fl!("menu-raster")), raster_submenu));

    // Layer borders checkbox
    items.push(iced_aw::menu::Item::new(menu_item_checkbox(
        fl!("menu-show_layer_borders"),
        "",
        marker_state.layer_borders_visible,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleLayerBorders)),
    )));

    // Line numbers checkbox
    items.push(iced_aw::menu::Item::new(menu_item_checkbox(
        fl!("menu-show_line_numbers"),
        "",
        marker_state.line_numbers_visible,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleLineNumbers)),
    )));

    items.push(iced_aw::menu::Item::new(separator()));

    // Fullscreen
    items.push(iced_aw::menu::Item::new(menu_item(&cmd::VIEW_FULLSCREEN, Message::ToggleFullscreen)));

    // Only show chat panel toggle if connected
    if is_connected {
        items.push(iced_aw::menu::Item::new(separator()));
        items.push(iced_aw::menu::Item::new(menu_item_simple(fl!("menu-toggle-chat"), Message::ToggleChatPanel)));
    }

    items.push(iced_aw::menu::Item::new(separator()));

    // Reference image
    items.push(iced_aw::menu::Item::new(menu_item_simple_enabled(
        fl!("menu-reference-image"),
        "Ctrl+Shift+O",
        Message::AnsiEditor(AnsiEditorMessage::ShowReferenceImageDialog),
        true,
    )));
    items.push(iced_aw::menu::Item::new(menu_item_simple_enabled(
        fl!("menu-toggle-reference-image"),
        "Ctrl+Tab",
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleReferenceImage)),
        true,
    )));

    Menu::new(items).width(300.0).offset(5.0)
}

/// Build the area operations submenu
fn build_area_submenu() -> Menu<'static, Message, Theme, iced::Renderer> {
    use crate::ui::editor::ansi::AnsiEditorMessage;

    let mut items: Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>> = Vec::new();

    // Line justify operations
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::JUSTIFY_LINE_LEFT,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLineLeft)),
    )));
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::JUSTIFY_LINE_CENTER,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLineCenter)),
    )));
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::JUSTIFY_LINE_RIGHT,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLineRight)),
    )));
    items.push(iced_aw::menu::Item::new(separator()));

    // Row/column insert/delete
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::INSERT_ROW,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::InsertRow)),
    )));
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::DELETE_ROW,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::DeleteRow)),
    )));
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::INSERT_COLUMN,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::InsertColumn)),
    )));
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::DELETE_COLUMN,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::DeleteColumn)),
    )));
    items.push(iced_aw::menu::Item::new(separator()));

    // Erase operations
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::ERASE_ROW,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseRow)),
    )));
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::ERASE_ROW_TO_START,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseRowToStart)),
    )));
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::ERASE_ROW_TO_END,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseRowToEnd)),
    )));
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::ERASE_COLUMN,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseColumn)),
    )));
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::ERASE_COLUMN_TO_START,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseColumnToStart)),
    )));
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::ERASE_COLUMN_TO_END,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::EraseColumnToEnd)),
    )));
    items.push(iced_aw::menu::Item::new(separator()));

    // Scroll operations
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::SCROLL_UP,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ScrollAreaUp)),
    )));
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::SCROLL_DOWN,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ScrollAreaDown)),
    )));
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::SCROLL_LEFT,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ScrollAreaLeft)),
    )));
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::SCROLL_RIGHT,
        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ScrollAreaRight)),
    )));

    Menu::new(items).width(250.0).offset(5.0)
}

/// Build the plugins submenu from loaded plugins
fn build_plugins_menu(plugins: &[Plugin]) -> Menu<'static, Message, Theme, iced::Renderer> {
    let mut items: Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>> = Vec::new();

    if plugins.is_empty() {
        // Show "No plugins" when empty
        items.push(iced_aw::menu::Item::new(
            button(text(fl!("menu-no_plugins")).size(14))
                .width(Length::Fill)
                .padding([4, 8])
                .style(menu_item_style),
        ));
    } else {
        // Group plugins by path
        let grouped = Plugin::group_by_path(plugins);

        for (menu_path, plugin_items) in grouped {
            if menu_path.is_empty() {
                // Top-level plugins
                for (i, p) in plugin_items {
                    items.push(iced_aw::menu::Item::new(
                        button(text(p.title.clone()).size(14))
                            .width(Length::Fill)
                            .padding([4, 8])
                            .style(menu_item_style)
                            .on_press(Message::AnsiEditor(AnsiEditorMessage::RunPlugin(i))),
                    ));
                }
            } else {
                // Submenu for grouped plugins
                let mut sub_items: Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>> = Vec::new();
                for (i, p) in plugin_items {
                    sub_items.push(iced_aw::menu::Item::new(
                        button(text(p.title.clone()).size(14))
                            .width(Length::Fill)
                            .padding([4, 8])
                            .style(menu_item_style)
                            .on_press(Message::AnsiEditor(AnsiEditorMessage::RunPlugin(i))),
                    ));
                }
                let submenu = Menu::new(sub_items).width(250.0).offset(5.0);
                items.push(iced_aw::menu::Item::with_menu(menu_item_submenu(menu_path), submenu));
            }
        }
    }

    Menu::new(items).width(250.0).offset(5.0)
}

/// Build the ANSI editor menu bar (full menu)
pub fn view_ansi(
    recent_files: &MostRecentlyUsedFiles,
    undo_info: &UndoInfo,
    marker_state: &MarkerMenuState,
    plugins: &[Plugin],
    mirror_mode: bool,
    is_connected: bool,
) -> Element<'static, Message> {
    let menu_template = |items| Menu::new(items).width(300.0).offset(5.0);

    let close_editor_hotkey = cmd::WINDOW_CLOSE.primary_hotkey_display().unwrap_or_default();

    let style_fn = |theme: &Theme, status: Status| {
        let palette = theme.extended_palette();
        menu::Style {
            path_border: Border {
                radius: Radius::new(6.0),
                ..Default::default()
            },
            path: palette.primary.weak.color.into(),
            ..primary(theme, status)
        }
    };

    if plugins.is_empty() {
        menu_bar!(
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
                    (menu_item(&cmd::FILE_EXPORT, Message::ExportFile)),
                    (menu_item_simple(fl!("menu-import-font"), Message::ShowImportFontDialog)),
                    (separator()),
                    (menu_item_simple(fl!("menu-connect-to-server"), Message::ShowConnectDialog)),
                    (separator()),
                    (menu_item(&cmd::SETTINGS_OPEN, Message::ShowSettings)),
                    (separator()),
                    (menu_item_simple_enabled(fl!("menu-close-editor"), close_editor_hotkey.as_str(), Message::CloseEditor, true)),
                    (menu_item(&cmd::APP_QUIT, Message::QuitApp))
                ))
            ),
            // Edit menu
            (
                menu_button(fl!("menu-edit")),
                menu_template(menu_items!(
                    (menu_item_undo(&cmd::EDIT_UNDO, Message::Undo, undo_info.undo_description.as_deref())),
                    (menu_item_redo(&cmd::EDIT_REDO, Message::Redo, undo_info.redo_description.as_deref())),
                    (separator()),
                    (menu_item(&cmd::EDIT_CUT, Message::Cut)),
                    (menu_item(&cmd::EDIT_COPY, Message::Copy)),
                    (menu_item(&cmd::EDIT_PASTE, Message::Paste)),
                    (
                        menu_item_submenu(fl!("menu-paste-as")),
                        menu_template(menu_items!((menu_item_simple(fl!("menu-paste-as-new-image"), Message::PasteAsNewImage))))
                    ),
                    (menu_item_simple(fl!("menu-insert-sixel-from-file"), Message::InsertSixelFromFile)),
                    (separator()),
                    (menu_item_submenu(fl!("menu-area_operations")), build_area_submenu()),
                    (separator()),
                    (menu_item_simple(fl!("menu-open_font_selector"), Message::AnsiEditor(AnsiEditorMessage::OpenFontSelector))),
                    (separator()),
                    (menu_item_checkbox(
                        fl!("menu-mirror_mode"),
                        "",
                        mirror_mode,
                        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleMirrorMode))
                    )),
                    (separator()),
                    (menu_item_simple(fl!("menu-file-settings"), Message::ShowFileSettingsDialog))
                ))
            ),
            // Selection menu
            (
                menu_button(fl!("menu-selection")),
                menu_template(menu_items!(
                    (menu_item(&cmd::EDIT_SELECT_ALL, Message::SelectAll)),
                    (menu_item(&selection_cmd::SELECT_NONE, Message::Deselect)),
                    (menu_item(&selection_cmd::SELECT_INVERSE, Message::AnsiEditor(AnsiEditorMessage::InverseSelection))),
                    (separator()),
                    (menu_item(
                        &selection_cmd::SELECT_FLIP_X,
                        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::FlipX))
                    )),
                    (menu_item(
                        &selection_cmd::SELECT_FLIP_Y,
                        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::FlipY))
                    )),
                    (menu_item(
                        &selection_cmd::SELECT_JUSTIFY_CENTER,
                        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyCenter))
                    )),
                    (menu_item(
                        &selection_cmd::SELECT_JUSTIFY_LEFT,
                        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLeft))
                    )),
                    (menu_item(
                        &selection_cmd::SELECT_JUSTIFY_RIGHT,
                        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyRight))
                    )),
                    (menu_item(
                        &selection_cmd::SELECT_CROP,
                        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::Crop))
                    ))
                ))
            ),
            // Colors menu
            (
                menu_button(fl!("menu-colors")),
                menu_template(menu_items!(
                    (menu_item_simple(fl!("menu-edit_palette"), Message::AnsiEditor(AnsiEditorMessage::EditPalette))),
                    (separator()),
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
                        fl!("menu-pick_attribute_under_caret"),
                        "Alt+U",
                        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PickAttributeUnderCaret)),
                        true
                    )),
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
            // View menu
            (
                menu_button(fl!("menu-view")),
                build_view_menu(
                    marker_state,
                    build_guides_submenu(marker_state),
                    build_raster_submenu(marker_state),
                    is_connected
                )
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
        .style(style_fn)
        .into()
    } else {
        let plugins_submenu = build_plugins_menu(plugins);
        menu_bar!(
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
                    (menu_item(&cmd::FILE_EXPORT, Message::ExportFile)),
                    (menu_item_simple(fl!("menu-import-font"), Message::ShowImportFontDialog)),
                    (separator()),
                    (menu_item_simple(fl!("menu-connect-to-server"), Message::ShowConnectDialog)),
                    (separator()),
                    (menu_item(&cmd::SETTINGS_OPEN, Message::ShowSettings)),
                    (separator()),
                    (menu_item_simple_enabled(fl!("menu-close-editor"), close_editor_hotkey.as_str(), Message::CloseEditor, true)),
                    (menu_item(&cmd::APP_QUIT, Message::QuitApp))
                ))
            ),
            // Edit menu
            (
                menu_button(fl!("menu-edit")),
                menu_template(menu_items!(
                    (menu_item_undo(&cmd::EDIT_UNDO, Message::Undo, undo_info.undo_description.as_deref())),
                    (menu_item_redo(&cmd::EDIT_REDO, Message::Redo, undo_info.redo_description.as_deref())),
                    (separator()),
                    (menu_item(&cmd::EDIT_CUT, Message::Cut)),
                    (menu_item(&cmd::EDIT_COPY, Message::Copy)),
                    (menu_item(&cmd::EDIT_PASTE, Message::Paste)),
                    (
                        menu_item_submenu(fl!("menu-paste-as")),
                        menu_template(menu_items!((menu_item_simple(fl!("menu-paste-as-new-image"), Message::PasteAsNewImage))))
                    ),
                    (menu_item_simple(fl!("menu-insert-sixel-from-file"), Message::InsertSixelFromFile)),
                    (separator()),
                    (menu_item_submenu(fl!("menu-area_operations")), build_area_submenu()),
                    (separator()),
                    (menu_item_simple(fl!("menu-open_font_selector"), Message::AnsiEditor(AnsiEditorMessage::OpenFontSelector))),
                    (separator()),
                    (menu_item_checkbox(
                        fl!("menu-mirror_mode"),
                        "",
                        mirror_mode,
                        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleMirrorMode))
                    )),
                    (separator()),
                    (menu_item_simple(fl!("menu-file-settings"), Message::ShowFileSettingsDialog))
                ))
            ),
            // Selection menu
            (
                menu_button(fl!("menu-selection")),
                menu_template(menu_items!(
                    (menu_item(&cmd::EDIT_SELECT_ALL, Message::SelectAll)),
                    (menu_item(&selection_cmd::SELECT_NONE, Message::Deselect)),
                    (menu_item(&selection_cmd::SELECT_INVERSE, Message::AnsiEditor(AnsiEditorMessage::InverseSelection))),
                    (separator()),
                    (menu_item(
                        &selection_cmd::SELECT_FLIP_X,
                        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::FlipX))
                    )),
                    (menu_item(
                        &selection_cmd::SELECT_FLIP_Y,
                        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::FlipY))
                    )),
                    (menu_item(
                        &selection_cmd::SELECT_JUSTIFY_CENTER,
                        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyCenter))
                    )),
                    (menu_item(
                        &selection_cmd::SELECT_JUSTIFY_LEFT,
                        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyLeft))
                    )),
                    (menu_item(
                        &selection_cmd::SELECT_JUSTIFY_RIGHT,
                        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::JustifyRight))
                    )),
                    (menu_item(
                        &selection_cmd::SELECT_CROP,
                        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::Crop))
                    ))
                ))
            ),
            // Colors menu
            (
                menu_button(fl!("menu-colors")),
                menu_template(menu_items!(
                    (menu_item_simple(fl!("menu-edit_palette"), Message::AnsiEditor(AnsiEditorMessage::EditPalette))),
                    (separator()),
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
                        fl!("menu-pick_attribute_under_caret"),
                        "Alt+U",
                        Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PickAttributeUnderCaret)),
                        true
                    )),
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
            // View menu
            (
                menu_button(fl!("menu-view")),
                build_view_menu(
                    marker_state,
                    build_guides_submenu(marker_state),
                    build_raster_submenu(marker_state),
                    is_connected
                )
            ),
            // Plugins menu
            (menu_button(fl!("menu-plugins")), plugins_submenu),
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
        .style(style_fn)
        .into()
    }
}
