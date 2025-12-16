//! ANSI Editor menu bar

use iced::{
    Border, Element, Length, Theme,
    border::Radius,
    widget::{button, text},
};
use iced_aw::menu::{self, Menu};
use iced_aw::style::{Status, menu_bar::primary};
use iced_aw::{menu_bar, menu_items};

use crate::fl;
use crate::ui::MostRecentlyUsedFiles;
use crate::ui::main_window::Message;
use crate::ui::main_window::commands::area_cmd;
use crate::ui::main_window::commands::selection_cmd;
use crate::ui::main_window::menu::{
    UndoInfo, build_recent_files_menu, menu_button, menu_item, menu_item_checkbox, menu_item_redo, menu_item_simple, menu_item_style, menu_item_submenu,
    menu_item_undo, separator,
};
use crate::ui::widget::plugins::Plugin;
use icy_engine_gui::commands::cmd;

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
            .on_press(Message::ClearGuide),
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
                .on_press(Message::SetGuide(x, y)),
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
            .on_press(Message::ToggleGuides),
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
            .on_press(Message::ClearRaster),
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
                .on_press(Message::SetRaster(x, y)),
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
            .on_press(Message::ToggleRaster),
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

/// Build the area operations submenu
fn build_area_submenu() -> Menu<'static, Message, Theme, iced::Renderer> {
    let mut items: Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>> = Vec::new();

    // Line justify operations
    items.push(iced_aw::menu::Item::new(menu_item(&area_cmd::JUSTIFY_LINE_LEFT, Message::JustifyLineLeft)));
    items.push(iced_aw::menu::Item::new(menu_item(&area_cmd::JUSTIFY_LINE_CENTER, Message::JustifyLineCenter)));
    items.push(iced_aw::menu::Item::new(menu_item(&area_cmd::JUSTIFY_LINE_RIGHT, Message::JustifyLineRight)));
    items.push(iced_aw::menu::Item::new(separator()));

    // Row/column insert/delete
    items.push(iced_aw::menu::Item::new(menu_item(&area_cmd::INSERT_ROW, Message::InsertRow)));
    items.push(iced_aw::menu::Item::new(menu_item(&area_cmd::DELETE_ROW, Message::DeleteRow)));
    items.push(iced_aw::menu::Item::new(menu_item(&area_cmd::INSERT_COLUMN, Message::InsertColumn)));
    items.push(iced_aw::menu::Item::new(menu_item(&area_cmd::DELETE_COLUMN, Message::DeleteColumn)));
    items.push(iced_aw::menu::Item::new(separator()));

    // Erase operations
    items.push(iced_aw::menu::Item::new(menu_item(&area_cmd::ERASE_ROW, Message::EraseRow)));
    items.push(iced_aw::menu::Item::new(menu_item(&area_cmd::ERASE_ROW_TO_START, Message::EraseRowToStart)));
    items.push(iced_aw::menu::Item::new(menu_item(&area_cmd::ERASE_ROW_TO_END, Message::EraseRowToEnd)));
    items.push(iced_aw::menu::Item::new(menu_item(&area_cmd::ERASE_COLUMN, Message::EraseColumn)));
    items.push(iced_aw::menu::Item::new(menu_item(
        &area_cmd::ERASE_COLUMN_TO_START,
        Message::EraseColumnToStart,
    )));
    items.push(iced_aw::menu::Item::new(menu_item(&area_cmd::ERASE_COLUMN_TO_END, Message::EraseColumnToEnd)));
    items.push(iced_aw::menu::Item::new(separator()));

    // Scroll operations
    items.push(iced_aw::menu::Item::new(menu_item(&area_cmd::SCROLL_UP, Message::ScrollAreaUp)));
    items.push(iced_aw::menu::Item::new(menu_item(&area_cmd::SCROLL_DOWN, Message::ScrollAreaDown)));
    items.push(iced_aw::menu::Item::new(menu_item(&area_cmd::SCROLL_LEFT, Message::ScrollAreaLeft)));
    items.push(iced_aw::menu::Item::new(menu_item(&area_cmd::SCROLL_RIGHT, Message::ScrollAreaRight)));

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
                            .on_press(Message::RunPlugin(i)),
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
                            .on_press(Message::RunPlugin(i)),
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
) -> Element<'static, Message> {
    let menu_template = |items| Menu::new(items).width(300.0).offset(5.0);

    // Build submenus with current state
    let guides_submenu = build_guides_submenu(marker_state);
    let raster_submenu = build_raster_submenu(marker_state);

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
                    (menu_item_simple(fl!("menu-import-font"), "", Message::ShowImportFontDialog)),
                    (menu_item_submenu(fl!("menu-open_recent")), build_recent_files_menu(recent_files)),
                    (separator()),
                    (menu_item(&cmd::FILE_SAVE, Message::SaveFile)),
                    (menu_item(&cmd::FILE_SAVE_AS, Message::SaveFileAs)),
                    (menu_item_simple(fl!("menu-export"), "", Message::ExportFile)),
                    (separator()),
                    (menu_item(&cmd::SETTINGS_OPEN, Message::ShowSettings)),
                    (separator()),
                    (menu_item(&cmd::FILE_CLOSE, Message::CloseFile))
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
                    (separator()),
                    (menu_item_submenu(fl!("menu-area_operations")), build_area_submenu()),
                    (separator()),
                    (menu_item_simple(fl!("menu-open_font_selector"), "", Message::OpenFontSelector)),
                    (separator()),
                    (menu_item_checkbox(fl!("menu-mirror_mode"), "", mirror_mode, Message::ToggleMirrorMode)),
                    (separator()),
                    (menu_item_simple(fl!("menu-file-settings"), "", Message::ShowFileSettingsDialog))
                ))
            ),
            // Selection menu
            (
                menu_button(fl!("menu-selection")),
                menu_template(menu_items!(
                    (menu_item(&cmd::EDIT_SELECT_ALL, Message::SelectAll)),
                    (menu_item(&selection_cmd::SELECT_NONE, Message::Deselect)),
                    (menu_item(&selection_cmd::SELECT_INVERSE, Message::InverseSelection)),
                    (separator()),
                    (menu_item(&selection_cmd::SELECT_ERASE, Message::DeleteSelection)),
                    (menu_item(&selection_cmd::SELECT_FLIP_X, Message::FlipX)),
                    (menu_item(&selection_cmd::SELECT_FLIP_Y, Message::FlipY)),
                    (menu_item(&selection_cmd::SELECT_JUSTIFY_CENTER, Message::JustifyCenter)),
                    (menu_item(&selection_cmd::SELECT_JUSTIFY_LEFT, Message::JustifyLeft)),
                    (menu_item(&selection_cmd::SELECT_JUSTIFY_RIGHT, Message::JustifyRight)),
                    (menu_item(&selection_cmd::SELECT_CROP, Message::Crop))
                ))
            ),
            // Colors menu
            (
                menu_button(fl!("menu-colors")),
                menu_template(menu_items!(
                    (menu_item_simple(fl!("menu-edit_palette"), "", Message::EditPalette)),
                    (separator()),
                    (menu_item_simple(fl!("menu-next_fg_color"), "Ctrl+Down", Message::NextFgColor)),
                    (menu_item_simple(fl!("menu-prev_fg_color"), "Ctrl+Up", Message::PrevFgColor)),
                    (separator()),
                    (menu_item_simple(fl!("menu-next_bg_color"), "Ctrl+Right", Message::NextBgColor)),
                    (menu_item_simple(fl!("menu-prev_bg_color"), "Ctrl+Left", Message::PrevBgColor)),
                    (separator()),
                    (menu_item_simple(fl!("menu-pick_attribute_under_caret"), "Alt+U", Message::PickAttributeUnderCaret)),
                    (menu_item_simple(fl!("menu-toggle_color"), "Alt+X", Message::ToggleColor)),
                    (menu_item_simple(fl!("menu-default_color"), "", Message::SwitchToDefaultColor))
                ))
            ),
            // View menu
            (
                menu_button(fl!("menu-view")),
                menu_template(menu_items!(
                    (menu_item_submenu(fl!("menu-zoom")), build_zoom_submenu()),
                    (separator()),
                    (menu_item_submenu(fl!("menu-guides")), guides_submenu),
                    (menu_item_submenu(fl!("menu-raster")), raster_submenu),
                    (menu_item_checkbox(
                        fl!("menu-show_layer_borders"),
                        "",
                        marker_state.layer_borders_visible,
                        Message::ToggleLayerBorders
                    )),
                    (menu_item_checkbox(fl!("menu-show_line_numbers"), "", marker_state.line_numbers_visible, Message::ToggleLineNumbers)),
                    (separator()),
                    (menu_item(&cmd::VIEW_FULLSCREEN, Message::ToggleFullscreen)),
                    (separator()),
                    (menu_item_simple(fl!("menu-reference-image"), "Ctrl+Shift+O", Message::ShowReferenceImageDialog)),
                    (menu_item_simple(fl!("menu-toggle-reference-image"), "Ctrl+Tab", Message::ToggleReferenceImage))
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
                    (menu_item_simple(fl!("menu-import-font"), "", Message::ShowImportFontDialog)),
                    (menu_item_submenu(fl!("menu-open_recent")), build_recent_files_menu(recent_files)),
                    (separator()),
                    (menu_item(&cmd::FILE_SAVE, Message::SaveFile)),
                    (menu_item(&cmd::FILE_SAVE_AS, Message::SaveFileAs)),
                    (menu_item_simple(fl!("menu-export"), "", Message::ExportFile)),
                    (separator()),
                    (menu_item(&cmd::SETTINGS_OPEN, Message::ShowSettings)),
                    (separator()),
                    (menu_item(&cmd::FILE_CLOSE, Message::CloseFile))
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
                    (separator()),
                    (menu_item_submenu(fl!("menu-area_operations")), build_area_submenu()),
                    (separator()),
                    (menu_item_simple(fl!("menu-open_font_selector"), "", Message::OpenFontSelector)),
                    (separator()),
                    (menu_item_checkbox(fl!("menu-mirror_mode"), "", mirror_mode, Message::ToggleMirrorMode)),
                    (separator()),
                    (menu_item_simple(fl!("menu-file-settings"), "", Message::ShowFileSettingsDialog))
                ))
            ),
            // Selection menu
            (
                menu_button(fl!("menu-selection")),
                menu_template(menu_items!(
                    (menu_item(&cmd::EDIT_SELECT_ALL, Message::SelectAll)),
                    (menu_item(&selection_cmd::SELECT_NONE, Message::Deselect)),
                    (menu_item(&selection_cmd::SELECT_INVERSE, Message::InverseSelection)),
                    (separator()),
                    (menu_item(&selection_cmd::SELECT_ERASE, Message::DeleteSelection)),
                    (menu_item(&selection_cmd::SELECT_FLIP_X, Message::FlipX)),
                    (menu_item(&selection_cmd::SELECT_FLIP_Y, Message::FlipY)),
                    (menu_item(&selection_cmd::SELECT_JUSTIFY_CENTER, Message::JustifyCenter)),
                    (menu_item(&selection_cmd::SELECT_JUSTIFY_LEFT, Message::JustifyLeft)),
                    (menu_item(&selection_cmd::SELECT_JUSTIFY_RIGHT, Message::JustifyRight)),
                    (menu_item(&selection_cmd::SELECT_CROP, Message::Crop))
                ))
            ),
            // Colors menu
            (
                menu_button(fl!("menu-colors")),
                menu_template(menu_items!(
                    (menu_item_simple(fl!("menu-edit_palette"), "", Message::EditPalette)),
                    (separator()),
                    (menu_item_simple(fl!("menu-next_fg_color"), "Ctrl+Down", Message::NextFgColor)),
                    (menu_item_simple(fl!("menu-prev_fg_color"), "Ctrl+Up", Message::PrevFgColor)),
                    (separator()),
                    (menu_item_simple(fl!("menu-next_bg_color"), "Ctrl+Right", Message::NextBgColor)),
                    (menu_item_simple(fl!("menu-prev_bg_color"), "Ctrl+Left", Message::PrevBgColor)),
                    (separator()),
                    (menu_item_simple(fl!("menu-pick_attribute_under_caret"), "Alt+U", Message::PickAttributeUnderCaret)),
                    (menu_item_simple(fl!("menu-toggle_color"), "Alt+X", Message::ToggleColor)),
                    (menu_item_simple(fl!("menu-default_color"), "", Message::SwitchToDefaultColor))
                ))
            ),
            // View menu
            (
                menu_button(fl!("menu-view")),
                menu_template(menu_items!(
                    (menu_item_submenu(fl!("menu-zoom")), build_zoom_submenu()),
                    (separator()),
                    (menu_item_submenu(fl!("menu-guides")), guides_submenu),
                    (menu_item_submenu(fl!("menu-raster")), raster_submenu),
                    (menu_item_checkbox(
                        fl!("menu-show_layer_borders"),
                        "",
                        marker_state.layer_borders_visible,
                        Message::ToggleLayerBorders
                    )),
                    (menu_item_checkbox(fl!("menu-show_line_numbers"), "", marker_state.line_numbers_visible, Message::ToggleLineNumbers)),
                    (separator()),
                    (menu_item(&cmd::VIEW_FULLSCREEN, Message::ToggleFullscreen)),
                    (separator()),
                    (menu_item_simple(fl!("menu-reference-image"), "Ctrl+Shift+O", Message::ShowReferenceImageDialog)),
                    (menu_item_simple(fl!("menu-toggle-reference-image"), "Ctrl+Tab", Message::ToggleReferenceImage))
                ))
            ),
            // Plugins menu
            (menu_button(fl!("menu-plugins")), plugins_submenu),
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
        .style(style_fn)
        .into()
    }
}
