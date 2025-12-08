//! Menu system for icy_draw
//!
//! Uses iced_aw's menu widget for dropdown menus.
//! Ported from the egui version in src_egui/ui/top_bar.rs

use iced::{
    Border, Color, Element, Length, Theme, alignment,
    border::Radius,
    widget::{button, row, text},
};
use iced_aw::menu::{self, Menu};
use iced_aw::style::{Status, menu_bar::primary};
use iced_aw::{menu_bar, menu_items, quad, widget::InnerBounds};

use super::MostRecentlyUsedFiles;
use super::main_window::{EditMode, Message};
use crate::fl;
use icy_engine_gui::commands::cmd;

/// Information about current undo/redo state for menu display
#[derive(Default, Clone)]
pub struct UndoInfo {
    /// Description of the next undo operation (None if nothing to undo)
    pub undo_description: Option<String>,
    /// Description of the next redo operation (None if nothing to redo)
    pub redo_description: Option<String>,
}

impl UndoInfo {
    pub fn new(undo_description: Option<String>, redo_description: Option<String>) -> Self {
        Self { undo_description, redo_description }
    }
}

/// Menu bar state
#[derive(Default)]
pub struct MenuBarState;

impl MenuBarState {
    pub fn new() -> Self {
        Self
    }

    /// Build the menu bar view based on the current edit mode
    pub fn view(&self, mode: &EditMode, recent_files: &MostRecentlyUsedFiles, undo_info: &UndoInfo) -> Element<'_, Message> {
        match mode {
            EditMode::Ansi => self.view_ansi(recent_files, undo_info),
            EditMode::BitFont => self.view_bitfont(recent_files, undo_info),
            EditMode::CharFont => self.view_charfont(recent_files, undo_info),
        }
    }

    /// Build the ANSI editor menu bar (full menu)
    fn view_ansi(&self, recent_files: &MostRecentlyUsedFiles, undo_info: &UndoInfo) -> Element<'_, Message> {
        let menu_template = |items| Menu::new(items).width(300.0).offset(5.0);

        let mb = menu_bar!(
            // ═══════════════════════════════════════════════════════════════════
            // File menu
            // ═══════════════════════════════════════════════════════════════════
            (
                menu_button(fl!("menu-file")),
                menu_template(menu_items!(
                    (menu_item(&cmd::FILE_NEW, Message::NewFile)),
                    (menu_item(&cmd::FILE_OPEN, Message::OpenFile)),
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
            // ═══════════════════════════════════════════════════════════════════
            // Edit menu
            // ═══════════════════════════════════════════════════════════════════
            (
                menu_button(fl!("menu-edit")),
                menu_template(menu_items!(
                    (menu_item_undo(&cmd::EDIT_UNDO, Message::Undo, undo_info.undo_description.as_deref())),
                    (menu_item_redo(&cmd::EDIT_REDO, Message::Redo, undo_info.redo_description.as_deref())),
                    (separator()),
                    (menu_item(&cmd::EDIT_CUT, Message::Cut)),
                    (menu_item(&cmd::EDIT_COPY, Message::Copy)),
                    (menu_item(&cmd::EDIT_PASTE, Message::Paste)),
                    // TODO: Paste As submenu (New Image, Brush)
                    (separator()),
                    (menu_item_simple(fl!("menu-mirror_mode"), "", Message::ToggleMirrorMode)),
                    (separator()),
                    (menu_item_simple(fl!("menu-edit-sauce"), "", Message::EditSauce)),
                    (menu_item_simple(fl!("menu-9px-font"), "Ctrl+F", Message::ToggleLGAFont)),
                    (menu_item_simple(fl!("menu-aspect-ratio"), "", Message::ToggleAspectRatio)),
                    (separator()),
                    (menu_item_simple(fl!("menu-set-canvas-size"), "", Message::SetCanvasSize))
                ))
            ),
            // ═══════════════════════════════════════════════════════════════════
            // Selection menu
            // ═══════════════════════════════════════════════════════════════════
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
            // ═══════════════════════════════════════════════════════════════════
            // Colors menu
            // ═══════════════════════════════════════════════════════════════════
            (
                menu_button(fl!("menu-colors")),
                menu_template(menu_items!(
                    // TODO: ICE Mode submenu
                    // TODO: Palette Mode submenu
                    (menu_item_simple(fl!("menu-select_palette"), "", Message::SelectPalette)),
                    (menu_item_simple(fl!("menu-open_palettes_directoy"), "", Message::OpenPalettesDirectory)),
                    (separator()),
                    (menu_item_simple(fl!("menu-next_fg_color"), "Ctrl+↓", Message::NextFgColor)),
                    (menu_item_simple(fl!("menu-prev_fg_color"), "Ctrl+↑", Message::PrevFgColor)),
                    (separator()),
                    (menu_item_simple(fl!("menu-next_bg_color"), "Ctrl+→", Message::NextBgColor)),
                    (menu_item_simple(fl!("menu-prev_bg_color"), "Ctrl+←", Message::PrevBgColor)),
                    (separator()),
                    (menu_item_simple(fl!("menu-pick_attribute_under_caret"), "Alt+U", Message::PickAttributeUnderCaret)),
                    (menu_item_simple(fl!("menu-toggle_color"), "Alt+X", Message::ToggleColor)),
                    (menu_item_simple(fl!("menu-default_color"), "", Message::SwitchToDefaultColor))
                ))
            ),
            // ═══════════════════════════════════════════════════════════════════
            // Fonts menu
            // ═══════════════════════════════════════════════════════════════════
            (
                menu_button(fl!("menu-fonts")),
                menu_template(menu_items!(
                    // TODO: Font Mode submenu
                    (menu_item_simple(fl!("menu-open_font_selector"), "", Message::OpenFontSelector)),
                    (menu_item_simple(fl!("menu-add_fonts"), "", Message::AddFonts)),
                    (menu_item_simple(fl!("menu-open_font_manager"), "", Message::OpenFontManager)),
                    (separator()),
                    (menu_item_simple(fl!("menu-open_font_directoy"), "", Message::OpenFontDirectory))
                ))
            ),
            // ═══════════════════════════════════════════════════════════════════
            // View menu
            // ═══════════════════════════════════════════════════════════════════
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
                    (menu_item_simple(fl!("menu-toggle_raster"), "Ctrl+'", Message::ToggleRaster)),
                    (menu_item_simple(fl!("menu-toggle_guide"), "Ctrl+;", Message::ToggleGuides)),
                    (menu_item_simple(fl!("menu-show_layer_borders"), "", Message::ToggleLayerBorders)),
                    (menu_item_simple(fl!("menu-show_line_numbers"), "", Message::ToggleLineNumbers)),
                    (separator()),
                    (menu_item_simple(fl!("menu-toggle_left_pane"), "F11", Message::ToggleLeftPanel)),
                    (menu_item_simple(fl!("menu-toggle_right_pane"), "F12", Message::ToggleRightPanel)),
                    (menu_item(&cmd::VIEW_FULLSCREEN, Message::ToggleFullscreen)),
                    (separator()),
                    (menu_item_simple(fl!("menu-reference-image"), "Ctrl+Shift+O", Message::SetReferenceImage)),
                    (menu_item_simple(fl!("menu-toggle-reference-image"), "Ctrl+Tab", Message::ToggleReferenceImage)),
                    (menu_item_simple(fl!("menu-clear-reference-image"), "", Message::ClearReferenceImage))
                ))
            ),
            // ═══════════════════════════════════════════════════════════════════
            // Plugins menu
            // ═══════════════════════════════════════════════════════════════════
            (
                menu_button(fl!("menu-plugins")),
                menu_template(menu_items!(
                    // TODO: Dynamic plugin list
                    (menu_item_simple(fl!("menu-open_plugin_directory"), "", Message::OpenPluginDirectory))
                ))
            ),
            // ═══════════════════════════════════════════════════════════════════
            // Help menu
            // ═══════════════════════════════════════════════════════════════════
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

    /// Build the BitFont editor menu bar (reduced: File, Edit, Help)
    fn view_bitfont(&self, recent_files: &MostRecentlyUsedFiles, undo_info: &UndoInfo) -> Element<'_, Message> {
        let menu_template = |items| Menu::new(items).width(300.0).offset(5.0);

        let mb = menu_bar!(
            // ═══════════════════════════════════════════════════════════════════
            // File menu (reduced for BitFont)
            // ═══════════════════════════════════════════════════════════════════
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
            // ═══════════════════════════════════════════════════════════════════
            // Edit menu (BitFont specific)
            // ═══════════════════════════════════════════════════════════════════
            (
                menu_button(fl!("menu-edit")),
                menu_template(menu_items!(
                    (menu_item_undo(&cmd::EDIT_UNDO, Message::Undo, undo_info.undo_description.as_deref())),
                    (menu_item_redo(&cmd::EDIT_REDO, Message::Redo, undo_info.redo_description.as_deref())),
                    (separator()),
                    (menu_item_simple(fl!("menu-select-all"), "Ctrl+A", Message::BitFontSelectAll)),
                    (menu_item_simple(fl!("menu-deselect"), "Esc", Message::BitFontClearSelection)),
                    (separator()),
                    (menu_item_simple(fl!("menu-clear-glyph"), "E", Message::BitFontClearGlyph)),
                    (menu_item_simple(fl!("menu-fill-glyph"), "F", Message::BitFontFillSelection)),
                    (menu_item_simple(fl!("menu-inverse-glyph"), "I", Message::BitFontInverseGlyph)),
                    (separator()),
                    (menu_item_simple(fl!("menu-flip-x"), "X", Message::BitFontFlipX)),
                    (menu_item_simple(fl!("menu-flip-y"), "Y", Message::BitFontFlipY))
                ))
            ),
            // ═══════════════════════════════════════════════════════════════════
            // Tools menu (BitFont specific)
            // ═══════════════════════════════════════════════════════════════════
            (
                menu_button(fl!("menu-tools")),
                menu_template(menu_items!(
                    (menu_item_simple(fl!("menu-tool-click"), "C", Message::BitFontSelectToolClick)),
                    (menu_item_simple(fl!("menu-tool-select"), "S", Message::BitFontSelectToolSelect)),
                    (menu_item_simple(fl!("menu-tool-rectangle"), "R", Message::BitFontSelectToolRect)),
                    (menu_item_simple(fl!("menu-tool-fill"), "G", Message::BitFontSelectToolFill)),
                    (separator()),
                    (menu_item_simple(fl!("menu-next-char"), "+", Message::BitFontNextChar)),
                    (menu_item_simple(fl!("menu-prev-char"), "-", Message::BitFontPrevChar))
                ))
            ),
            // ═══════════════════════════════════════════════════════════════════
            // Help menu
            // ═══════════════════════════════════════════════════════════════════
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

    /// Build the CharFont (TDF) editor menu bar (placeholder - same as BitFont for now)
    fn view_charfont(&self, recent_files: &MostRecentlyUsedFiles, undo_info: &UndoInfo) -> Element<'_, Message> {
        // For now, use the same reduced menu as BitFont
        self.view_bitfont(recent_files, undo_info)
    }
}

/// Create a menu bar button
fn menu_button(label: String) -> Element<'static, Message> {
    button(text(label).size(14))
        .padding([4, 8])
        .style(menu_button_style)
        .on_press(Message::Tick) // Dummy message - menu handles the interaction
        .into()
}

/// Create a menu item from a command definition
fn menu_item(cmd: &icy_engine_gui::commands::CommandDef, msg: Message) -> Element<'static, Message> {
    let label = if cmd.label_menu.is_empty() { cmd.id.clone() } else { cmd.label_menu.clone() };

    let hotkey = cmd.primary_hotkey_display().unwrap_or_default();

    button(
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
    .style(menu_item_style)
    .on_press(msg)
    .into()
}

/// Create an Undo menu item with optional operation description
fn menu_item_undo(cmd: &icy_engine_gui::commands::CommandDef, msg: Message, description: Option<&str>) -> Element<'static, Message> {
    let base_label = if cmd.label_menu.is_empty() { cmd.id.clone() } else { cmd.label_menu.clone() };
    let label = match description {
        Some(desc) => format!("{} {}", base_label, desc),
        None => base_label,
    };
    let hotkey = cmd.primary_hotkey_display().unwrap_or_default();
    let is_enabled = description.is_some();

    let btn = button(
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
    .style(if is_enabled { menu_item_style } else { menu_item_disabled_style });

    if is_enabled {
        btn.on_press(msg).into()
    } else {
        btn.into()
    }
}

/// Create a Redo menu item with optional operation description
fn menu_item_redo(cmd: &icy_engine_gui::commands::CommandDef, msg: Message, description: Option<&str>) -> Element<'static, Message> {
    let base_label = if cmd.label_menu.is_empty() { cmd.id.clone() } else { cmd.label_menu.clone() };
    let label = match description {
        Some(desc) => format!("{} {}", base_label, desc),
        None => base_label,
    };
    let hotkey = cmd.primary_hotkey_display().unwrap_or_default();
    let is_enabled = description.is_some();

    let btn = button(
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
    .style(if is_enabled { menu_item_style } else { menu_item_disabled_style });

    if is_enabled {
        btn.on_press(msg).into()
    } else {
        btn.into()
    }
}

/// Create a menu item with direct label and hotkey (without CommandDef)
fn menu_item_simple(label: String, hotkey: &str, msg: Message) -> Element<'static, Message> {
    let hotkey_text = hotkey.to_string();

    button(
        row![
            text(label).size(14).width(Length::Fill),
            text(hotkey_text).size(12).style(|theme: &Theme| {
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
    .style(menu_item_style)
    .on_press(msg)
    .into()
}

/// Create a separator line
/*fn separator() -> Element<'static, Message> {
    container(rule::horizontal(1))
        .padding([4, 0])
        .width(Length::Fill)
        .into()
}*/
fn separator() -> quad::Quad {
    quad::Quad {
        quad_color: Color::from([0.5; 3]).into(),
        quad_border: Border {
            radius: Radius::new(5.0),
            ..Default::default()
        },
        inner_bounds: InnerBounds::Ratio(0.98, 0.2),
        height: Length::Fixed(4.0),
        ..Default::default()
    }
}

/// Create a submenu item (with arrow indicator)
fn menu_item_submenu(label: String) -> Element<'static, Message> {
    button(
        row![
            text(label).size(14).width(Length::Fill),
            text("▶").size(12).style(|theme: &Theme| {
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
    .style(menu_item_style)
    .on_press(Message::Tick) // Dummy - submenu handles interaction
    .into()
}

/// Build the recent files submenu
fn build_recent_files_menu(recent_files: &MostRecentlyUsedFiles) -> Menu<'static, Message, Theme, iced::Renderer> {
    let files = recent_files.files();
    
    let mut items: Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>> = Vec::new();
    
    if files.is_empty() {
        // Show "No recent files" when empty
        items.push(iced_aw::menu::Item::new(
            button(text(fl!("menu-no_recent_files")).size(14))
                .width(Length::Fill)
                .padding([4, 8])
                .style(menu_item_disabled_style)
        ));
    } else {
        // Show files in reverse order (most recent first)
        for file in files.iter().rev() {
            let file_name = file.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| file.display().to_string());
            let path = file.clone();
            
            items.push(iced_aw::menu::Item::new(
                button(text(file_name).size(14))
                    .width(Length::Fill)
                    .padding([4, 8])
                    .style(menu_item_style)
                    .on_press(Message::OpenRecentFile(path))
            ));
        }
        
        // Add separator and clear option
        items.push(iced_aw::menu::Item::new(separator()));
        items.push(iced_aw::menu::Item::new(
            button(text(fl!("menu-clear_recent_files")).size(14))
                .width(Length::Fill)
                .padding([4, 8])
                .style(menu_item_style)
                .on_press(Message::ClearRecentFiles)
        ));
    }
    
    Menu::new(items).width(350.0).offset(0.0)
}

// ============================================================================
// Styles
// ============================================================================

fn menu_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        text_color: palette.background.base.text,
        border: Border::default().rounded(6.0),
        ..Default::default()
    };
    match status {
        button::Status::Active => base.with_background(Color::TRANSPARENT),
        button::Status::Hovered => base.with_background(Color::from_rgb(
            palette.primary.weak.color.r * 1.2,
            palette.primary.weak.color.g * 1.2,
            palette.primary.weak.color.b * 1.2,
        )),
        button::Status::Pressed => base.with_background(palette.primary.weak.color),
        button::Status::Disabled => base.with_background(Color::from_rgb(0.5, 0.5, 0.5)),
    }
}

fn menu_item_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        text_color: palette.background.base.text,
        border: Border::default().rounded(6.0),
        ..Default::default()
    };

    match status {
        button::Status::Active => base.with_background(Color::TRANSPARENT),
        button::Status::Hovered => base.with_background(Color::from_rgb(
            palette.primary.weak.color.r * 1.2,
            palette.primary.weak.color.g * 1.2,
            palette.primary.weak.color.b * 1.2,
        )),
        button::Status::Pressed => base.with_background(palette.primary.weak.color),
        button::Status::Disabled => base.with_background(Color::from_rgb(0.5, 0.5, 0.5)),
    }
}

fn menu_item_disabled_style(theme: &Theme, _status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    button::Style {
        text_color: palette.background.base.text.scale_alpha(0.5),
        border: Border::default().rounded(6.0),
        background: Some(Color::TRANSPARENT.into()),
        ..Default::default()
    }
}
