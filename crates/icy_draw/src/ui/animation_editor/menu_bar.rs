//! Animation Editor menu bar
//!
//! Simple menu bar with File, Edit, and Help menus.

use iced::{Border, Element, Theme, border::Radius};
use iced_aw::Menu;
use iced_aw::style::{Status, menu_bar::primary};
use iced_aw::{menu_bar, menu_items};

use crate::fl;
use crate::ui::MostRecentlyUsedFiles;
use crate::ui::main_window::Message;
use crate::ui::menu::{UndoInfo, build_recent_files_menu, menu_button, menu_item, menu_item_simple, menu_item_submenu, separator};
use icy_engine_gui::commands::cmd;

/// Build the Animation editor menu bar
pub fn view_animation_menu(recent_files: &MostRecentlyUsedFiles, undo_info: &UndoInfo) -> Element<'static, Message> {
    let menu_template = |items| Menu::new(items).width(280.0).offset(5.0);

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
                (menu_item_simple(fl!("menu-export"), "", Message::ShowAnimationExportDialog)),
                (separator()),
                (menu_item(&cmd::FILE_CLOSE, Message::CloseFile))
            ))
        ),
        // Edit menu
        (
            menu_button(fl!("menu-edit")),
            menu_template(menu_items!(
                (menu_item(&cmd::EDIT_UNDO, Message::Undo)),
                (menu_item(&cmd::EDIT_REDO, Message::Redo)),
                (separator()),
                (menu_item(&cmd::EDIT_CUT, Message::Cut)),
                (menu_item(&cmd::EDIT_COPY, Message::Copy)),
                (menu_item(&cmd::EDIT_PASTE, Message::Paste)),
                (separator()),
                (menu_item(&cmd::EDIT_SELECT_ALL, Message::SelectAll))
            ))
        ),
        // Help menu
        (
            menu_button(fl!("menu-help")),
            menu_template(menu_items!(
                (menu_item_simple(fl!("menu-discuss"), "", Message::OpenDiscussions)),
                (menu_item_simple(fl!("menu-report-bug"), "", Message::ReportBug)),
                (separator()),
                (menu_item_simple(fl!("menu-about"), "", Message::ShowAbout))
            ))
        )
    )
    .style(|theme: &Theme, status: Status| {
        let palette = theme.extended_palette();
        let mut style = primary(theme, status);
        style.bar_background = iced::Background::Color(palette.background.weak.color);
        style.menu_background = iced::Background::Color(palette.background.base.color);
        style.menu_border = Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: Radius::new(4),
        };
        style
    });

    mb.into()
}
