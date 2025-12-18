//! CharFont (TDF) Editor menu bar
//!
//! Simplified menu bar for TDF font editing - no palette changes, no SAUCE, no canvas resize.

use iced::{Border, Element, Theme, border::Radius};
use iced_aw::menu::{self, Menu};
use iced_aw::style::{Status, menu_bar::primary};
use iced_aw::{menu_bar, menu_items};

use crate::fl;
use crate::ui::MostRecentlyUsedFiles;
use crate::ui::editor::ansi::{AnsiEditorCoreMessage, AnsiEditorMessage};
use crate::ui::main_window::Message;
use crate::ui::main_window::menu::{
    UndoInfo, build_recent_files_menu, menu_button, menu_item, menu_item_redo, menu_item_simple, menu_item_submenu, menu_item_undo, separator,
};
use icy_engine_gui::commands::cmd;

/// Build the CharFont (TDF) editor menu bar
pub fn view_charfont(recent_files: &MostRecentlyUsedFiles, undo_info: &UndoInfo) -> Element<'static, Message> {
    let menu_template = |items| Menu::new(items).width(300.0).offset(5.0);

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
                (menu_item(&cmd::SETTINGS_OPEN, Message::ShowSettings)),
                (separator()),
                (menu_item(&cmd::FILE_CLOSE, Message::CloseFile))
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
                (menu_item_simple(
                    fl!("menu-next_fg_color"),
                    "Ctrl+Down",
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::NextFgColor))
                )),
                (menu_item_simple(
                    fl!("menu-prev_fg_color"),
                    "Ctrl+Up",
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PrevFgColor))
                )),
                (separator()),
                (menu_item_simple(
                    fl!("menu-next_bg_color"),
                    "Ctrl+Right",
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::NextBgColor))
                )),
                (menu_item_simple(
                    fl!("menu-prev_bg_color"),
                    "Ctrl+Left",
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::PrevBgColor))
                )),
                (separator()),
                (menu_item_simple(
                    fl!("menu-toggle_color"),
                    "Alt+X",
                    Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToggleColor))
                )),
                (menu_item_simple(
                    fl!("menu-default_color"),
                    "",
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
                (menu_item_simple("4:1 400%".to_string(), "", Message::SetZoom(4.0))),
                (menu_item_simple("2:1 200%".to_string(), "", Message::SetZoom(2.0))),
                (menu_item_simple("1:1 100%".to_string(), "", Message::SetZoom(1.0))),
                (menu_item_simple("1:2 50%".to_string(), "", Message::SetZoom(0.5))),
                (menu_item_simple("1:4 25%".to_string(), "", Message::SetZoom(0.25))),
                (separator()),
                (menu_item(&cmd::VIEW_FULLSCREEN, Message::ToggleFullscreen))
            ))
        ),
        // Help menu
        (
            menu_button(fl!("menu-help")),
            menu_template(menu_items!(
                (menu_item_simple(fl!("menu-discuss"), "", Message::OpenDiscussions)),
                (menu_item_simple(fl!("menu-report-bug"), "", Message::ReportBug)),
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
