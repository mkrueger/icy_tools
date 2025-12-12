//! ANSI Editor menu bar

use iced::{Border, Color, Element, Length, Theme, border::Radius, widget::button};
use iced_aw::menu::{self, Menu};
use iced_aw::style::{Status, menu_bar::primary};
use iced_aw::{menu_bar, menu_items};

use crate::fl;
use crate::ui::MostRecentlyUsedFiles;
use crate::ui::main_window::Message;
use crate::ui::menu::{
    UndoInfo, build_recent_files_menu, menu_button, menu_item, menu_item_checkbox, menu_item_redo, menu_item_simple, menu_item_style, menu_item_submenu,
    menu_item_undo, separator,
};
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

/// Build the ANSI editor menu bar (full menu)
pub fn view_ansi(recent_files: &MostRecentlyUsedFiles, undo_info: &UndoInfo, marker_state: &MarkerMenuState) -> Element<'static, Message> {
    let menu_template = |items| Menu::new(items).width(300.0).offset(5.0);

    // Build submenus with current state
    let guides_submenu = build_guides_submenu(marker_state);
    let raster_submenu = build_raster_submenu(marker_state);

    let mb = menu_bar!(
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
                (menu_item_simple(fl!("menu-mirror_mode"), "", Message::ToggleMirrorMode)),
                (separator()),
                (menu_item_simple(fl!("menu-file-settings"), "", Message::ShowFileSettingsDialog))
            ))
        ),
        // Selection menu
        (
            menu_button(fl!("menu-selection")),
            menu_template(menu_items!(
                (menu_item(&cmd::EDIT_SELECT_ALL, Message::SelectAll)),
                (menu_item_simple(fl!("menu-select_nothing"), "Ctrl+D", Message::Deselect)),
                (menu_item_simple(fl!("menu-inverse_selection"), "Ctrl+Shift+I", Message::InverseSelection)),
                (separator()),
                (menu_item_simple(fl!("menu-erase"), "Del", Message::DeleteSelection)),
                (menu_item_simple(fl!("menu-flipx"), "", Message::FlipX)),
                (menu_item_simple(fl!("menu-flipy"), "", Message::FlipY)),
                (menu_item_simple(fl!("menu-justifycenter"), "", Message::JustifyCenter)),
                (menu_item_simple(fl!("menu-justifyleft"), "", Message::JustifyLeft)),
                (menu_item_simple(fl!("menu-justifyright"), "", Message::JustifyRight)),
                (menu_item_simple(fl!("menu-crop"), "", Message::Crop))
            ))
        ),
        // Colors menu
        (
            menu_button(fl!("menu-colors")),
            menu_template(menu_items!(
                (menu_item_simple(fl!("menu-select_palette"), "", Message::SelectPalette)),
                (menu_item_simple(fl!("menu-open_palettes_directoy"), "", Message::OpenPalettesDirectory)),
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
        // Fonts menu
        (
            menu_button(fl!("menu-fonts")),
            menu_template(menu_items!(
                (menu_item_simple(fl!("menu-open_font_selector"), "", Message::OpenFontSelector)),
                (menu_item_simple(fl!("menu-add_fonts"), "", Message::AddFonts)),
                (menu_item_simple(fl!("menu-open_font_manager"), "", Message::OpenFontManager)),
                (separator()),
                (menu_item_simple(fl!("menu-open_font_directoy"), "", Message::OpenFontDirectory))
            ))
        ),
        // View menu
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
                (menu_item_submenu(fl!("menu-guides")), guides_submenu),
                (menu_item_submenu(fl!("menu-raster")), raster_submenu),
                (menu_item_simple(fl!("menu-show_layer_borders"), "", Message::ToggleLayerBorders)),
                (menu_item_checkbox(fl!("menu-show_line_numbers"), "", marker_state.line_numbers_visible, Message::ToggleLineNumbers)),
                (separator()),
                (menu_item_simple(fl!("menu-toggle_left_pane"), "F11", Message::ToggleLeftPanel)),
                (menu_item_simple(fl!("menu-toggle_right_pane"), "F12", Message::ToggleRightPanel)),
                (menu_item(&cmd::VIEW_FULLSCREEN, Message::ToggleFullscreen)),
                (separator()),
                (menu_item_simple(fl!("menu-reference-image"), "Ctrl+Shift+O", Message::ShowReferenceImageDialog)),
                (menu_item_simple(fl!("menu-toggle-reference-image"), "Ctrl+Tab", Message::ToggleReferenceImage))
            ))
        ),
        // Plugins menu
        (
            menu_button(fl!("menu-plugins")),
            menu_template(menu_items!(
                (menu_item_simple(fl!("menu-open_plugin_directory"), "", Message::OpenPluginDirectory))
            ))
        ),
        // Help menu
        (
            menu_button(fl!("menu-help")),
            menu_template(menu_items!(
                (menu_item_simple(fl!("menu-discuss"), "", Message::OpenDiscussions)),
                (menu_item_simple(fl!("menu-report-bug"), "", Message::ReportBug)),
                (menu_item_simple(fl!("menu-open_log_file"), "", Message::OpenLogFile)),
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
            path: Color::from_rgb(
                palette.primary.weak.color.r * 1.2,
                palette.primary.weak.color.g * 1.2,
                palette.primary.weak.color.b * 1.2,
            )
            .into(),
            ..primary(theme, status)
        }
    });

    mb.into()
}
