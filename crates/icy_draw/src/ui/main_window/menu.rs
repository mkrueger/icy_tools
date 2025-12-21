//! Menu system for icy_draw
//!
//! Uses iced_aw's menu widget for dropdown menus.
//! Ported from the egui version in src_egui/ui/top_bar.rs

use iced::{
    Border, Color, Element, Length, Theme, alignment,
    border::Radius,
    widget::{button, row, text},
};
use iced_aw::menu::Menu;
use iced_aw::{quad, widget::InnerBounds};

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::main_window::{EditMode, Message};
use crate::Settings;
use crate::{
    MostRecentlyUsedFiles, Plugin, fl,
    ui::editor::{animation, ansi, bitfont, charfont},
};
pub use ansi::widget::toolbar::menu_bar::MarkerMenuState;
use icy_engine_gui::commands::{CommandDef, Hotkey};

// ============================================================================
// MenuItem - Unified menu item data structure
// ============================================================================

/// State passed to dynamic menu builders
#[allow(dead_code)]
pub struct MenuState<'a> {
    pub recent_files: &'a MostRecentlyUsedFiles,
    pub undo_description: Option<&'a str>,
    pub redo_description: Option<&'a str>,
}

/// A menu item that can be rendered and checked for hotkeys.
/// Single source of truth for both menu display and keyboard handling.
#[derive(Clone)]
#[allow(dead_code)]
pub enum MenuItem {
    /// Command-based item with hotkey support from CommandDef
    Command {
        cmd: &'static CommandDef,
        message: Message,
        enabled: bool,
        /// Optional dynamic label override (e.g., "Undo Clear Glyph")
        label_override: Option<String>,
    },
    /// Simple item without command definition (manual hotkey display)
    Simple {
        label: String,
        hotkey_display: String,
        message: Message,
        enabled: bool,
    },
    /// Toggle item with checked state (checkbox/toggle)
    Toggle {
        cmd: Option<&'static CommandDef>,
        label: String,
        hotkey_display: String,
        message: Message,
        checked: bool,
        enabled: bool,
    },
    /// Static submenu with nested items
    SubMenu { label: String, items: Vec<MenuItem> },
    /// Dynamic submenu - builder is called each time menu opens
    DynamicSubMenu {
        label: String,
        builder: Arc<dyn Fn(&MenuState<'_>) -> Vec<MenuItem> + Send + Sync>,
    },
    /// Separator line
    Separator,
}

#[allow(dead_code)]
impl MenuItem {
    // ========================================================================
    // Constructors
    // ========================================================================

    /// Create a command-based menu item (enabled by default)
    pub fn cmd(cmd: &'static CommandDef, message: Message) -> Self {
        Self::Command {
            cmd,
            message,
            enabled: true,
            label_override: None,
        }
    }

    /// Create a command item with dynamic label (e.g., for Undo/Redo)
    pub fn cmd_with_label(cmd: &'static CommandDef, message: Message, label: impl Into<String>) -> Self {
        Self::Command {
            cmd,
            message,
            enabled: true,
            label_override: Some(label.into()),
        }
    }

    /// Create a simple menu item without command definition
    pub fn simple(label: impl Into<String>, hotkey: impl Into<String>, message: Message) -> Self {
        Self::Simple {
            label: label.into(),
            hotkey_display: hotkey.into(),
            message,
            enabled: true,
        }
    }

    /// Create a toggle/checkbox item with command
    pub fn toggle_cmd(cmd: &'static CommandDef, message: Message, checked: bool) -> Self {
        Self::Toggle {
            cmd: Some(cmd),
            label: if cmd.label_menu.is_empty() { cmd.id.clone() } else { cmd.label_menu.clone() },
            hotkey_display: cmd.primary_hotkey_display().unwrap_or_default(),
            message,
            checked,
            enabled: true,
        }
    }

    /// Create a toggle/checkbox item without command
    pub fn toggle(label: impl Into<String>, hotkey: impl Into<String>, message: Message, checked: bool) -> Self {
        Self::Toggle {
            cmd: None,
            label: label.into(),
            hotkey_display: hotkey.into(),
            message,
            checked,
            enabled: true,
        }
    }

    /// Create a static submenu
    pub fn submenu(label: impl Into<String>, items: Vec<MenuItem>) -> Self {
        Self::SubMenu { label: label.into(), items }
    }

    /// Create a dynamic submenu with builder callback
    pub fn dynamic_submenu<F>(label: impl Into<String>, builder: F) -> Self
    where
        F: Fn(&MenuState<'_>) -> Vec<MenuItem> + Send + Sync + 'static,
    {
        Self::DynamicSubMenu {
            label: label.into(),
            builder: Arc::new(builder),
        }
    }

    /// Create a separator
    pub fn separator() -> Self {
        Self::Separator
    }

    // ========================================================================
    // Builder methods (chainable)
    // ========================================================================

    /// Set enabled state (builder pattern)
    pub fn enabled(mut self, enabled: bool) -> Self {
        match &mut self {
            Self::Command { enabled: e, .. } => *e = enabled,
            Self::Simple { enabled: e, .. } => *e = enabled,
            Self::Toggle { enabled: e, .. } => *e = enabled,
            _ => {}
        }
        self
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Get the display label
    pub fn label(&self) -> String {
        match self {
            Self::Command { cmd, label_override, .. } => label_override
                .clone()
                .unwrap_or_else(|| if cmd.label_menu.is_empty() { cmd.id.clone() } else { cmd.label_menu.clone() }),
            Self::Simple { label, .. } => label.clone(),
            Self::Toggle { label, .. } => label.clone(),
            Self::SubMenu { label, .. } => label.clone(),
            Self::DynamicSubMenu { label, .. } => label.clone(),
            Self::Separator => String::new(),
        }
    }

    /// Get the hotkey display string
    pub fn hotkey_display(&self) -> String {
        match self {
            Self::Command { cmd, .. } => cmd.primary_hotkey_display().unwrap_or_default(),
            Self::Simple { hotkey_display, .. } => hotkey_display.clone(),
            Self::Toggle { hotkey_display, .. } => hotkey_display.clone(),
            _ => String::new(),
        }
    }

    /// Check if this item is enabled
    pub fn is_enabled(&self) -> bool {
        match self {
            Self::Command { enabled, .. } => *enabled,
            Self::Simple { enabled, .. } => *enabled,
            Self::Toggle { enabled, .. } => *enabled,
            Self::SubMenu { .. } => true,
            Self::DynamicSubMenu { .. } => true,
            Self::Separator => true,
        }
    }

    // ========================================================================
    // Hotkey matching
    // ========================================================================

    /// Check if this item's hotkey matches (returns message if match and enabled)
    pub fn matches_hotkey(&self, hotkey: &Hotkey) -> Option<Message> {
        match self {
            Self::Command {
                cmd, message, enabled: true, ..
            } => {
                if cmd.active_hotkeys().iter().any(|hk| hk == hotkey) {
                    Some(message.clone())
                } else {
                    None
                }
            }
            Self::Toggle {
                cmd: Some(cmd),
                message,
                enabled: true,
                ..
            } => {
                if cmd.active_hotkeys().iter().any(|hk| hk == hotkey) {
                    Some(message.clone())
                } else {
                    None
                }
            }
            Self::SubMenu { items, .. } => {
                // Recursively check submenu items
                for item in items {
                    if let Some(msg) = item.matches_hotkey(hotkey) {
                        return Some(msg);
                    }
                }
                None
            }
            // DynamicSubMenu hotkeys are not checked here (would need state)
            // Simple items don't have CommandDef hotkeys
            _ => None,
        }
    }

    /// Recursively check hotkeys including dynamic submenus (needs state)
    pub fn matches_hotkey_with_state(&self, hotkey: &Hotkey, state: &MenuState<'_>) -> Option<Message> {
        match self {
            Self::DynamicSubMenu { builder, .. } => {
                let items = builder(state);
                for item in &items {
                    if let Some(msg) = item.matches_hotkey_with_state(hotkey, state) {
                        return Some(msg);
                    }
                }
                None
            }
            Self::SubMenu { items, .. } => {
                for item in items {
                    if let Some(msg) = item.matches_hotkey_with_state(hotkey, state) {
                        return Some(msg);
                    }
                }
                None
            }
            _ => self.matches_hotkey(hotkey),
        }
    }
}

// ============================================================================
// MenuItem Rendering
// ============================================================================

/// Render a single MenuItem to an iced Element
pub fn render_menu_item(item: &MenuItem) -> Element<'static, Message> {
    match item {
        MenuItem::Command { message, enabled, .. } | MenuItem::Simple { message, enabled, .. } => {
            render_menu_item_button(item.label(), item.hotkey_display(), message.clone(), *enabled)
        }
        MenuItem::Toggle { message, checked, enabled, .. } => render_menu_item_toggle(item.label(), item.hotkey_display(), message.clone(), *checked, *enabled),
        MenuItem::SubMenu { label, .. } => render_menu_item_submenu_button(label.clone()),
        MenuItem::DynamicSubMenu { label, .. } => render_menu_item_submenu_button(label.clone()),
        MenuItem::Separator => separator().into(),
    }
}

fn render_menu_item_button(label: String, hotkey: String, message: Message, enabled: bool) -> Element<'static, Message> {
    let mut btn = button(
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
    .style(if enabled { menu_item_style } else { menu_item_disabled_style });

    if enabled {
        btn = btn.on_press(message);
    }

    btn.into()
}

fn render_menu_item_toggle(label: String, hotkey: String, message: Message, checked: bool, enabled: bool) -> Element<'static, Message> {
    let indicator = if checked { "☑" } else { "☐" };

    let mut btn = button(
        row![
            text(indicator).size(14).width(Length::Fixed(20.0)),
            text(label).size(14).width(Length::Fill),
            text(hotkey).size(12).style(|theme: &Theme| {
                iced::widget::text::Style {
                    color: Some(theme.palette().text.scale_alpha(0.6)),
                }
            }),
        ]
        .spacing(8)
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .padding([4, 8])
    .style(if enabled { menu_item_style } else { menu_item_disabled_style });

    if enabled {
        btn = btn.on_press(message);
    }

    btn.into()
}

fn render_menu_item_submenu_button(label: String) -> Element<'static, Message> {
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
    .on_press(Message::Noop)
    .into()
}

/// Convert a slice of MenuItems to iced_aw menu items (for rendering)
pub fn menu_items_to_iced(items: &[MenuItem], state: &MenuState<'_>) -> Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>> {
    items
        .iter()
        .map(|item| match item {
            MenuItem::SubMenu { items: sub_items, .. } => {
                let sub_menu = Menu::new(menu_items_to_iced(sub_items, state)).width(300.0).offset(5.0);
                iced_aw::menu::Item::with_menu(render_menu_item(item), sub_menu)
            }
            MenuItem::DynamicSubMenu { builder, .. } => {
                let dynamic_items = builder(state);
                let sub_menu = Menu::new(menu_items_to_iced(&dynamic_items, state)).width(300.0).offset(5.0);
                iced_aw::menu::Item::with_menu(render_menu_item(item), sub_menu)
            }
            _ => iced_aw::menu::Item::new(render_menu_item(item)),
        })
        .collect()
}

/// Check all items in a menu for hotkey matches
#[allow(dead_code)]
pub fn check_menu_hotkeys(items: &[MenuItem], hotkey: &Hotkey, state: &MenuState<'_>) -> Option<Message> {
    for item in items {
        if let Some(msg) = item.matches_hotkey_with_state(hotkey, state) {
            return Some(msg);
        }
    }
    None
}

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
        Self {
            undo_description,
            redo_description,
        }
    }
}

/// Menu bar state
#[derive(Default)]
pub struct MenuBarState;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct MenuBarCacheKey {
    // Use a primitive mode tag to avoid requiring `EditMode: Hash`.
    mode_tag: u8,
    undo_description: Option<String>,
    redo_description: Option<String>,

    // Flattened `MarkerMenuState` to avoid requiring it to implement Hash/Eq.
    guide: Option<(u32, u32)>,
    guide_visible: bool,
    raster: Option<(u32, u32)>,
    raster_visible: bool,
    line_numbers_visible: bool,
    layer_borders_visible: bool,

    mirror_mode: bool,
    recent_files_hash: u64,
    plugins_hash: u64,
    is_connected: bool,
    has_image_clipboard: bool,
}

fn hash_recent_files(recent_files: &MostRecentlyUsedFiles) -> u64 {
    let mut hasher = DefaultHasher::new();
    recent_files.files().len().hash(&mut hasher);
    for path in recent_files.files() {
        path.hash(&mut hasher);
    }
    hasher.finish()
}

fn hash_plugins(plugins: &[Plugin]) -> u64 {
    let mut hasher = DefaultHasher::new();
    plugins.len().hash(&mut hasher);
    for plugin in plugins {
        plugin.title.hash(&mut hasher);
        plugin.path.hash(&mut hasher);
        plugin.description.hash(&mut hasher);
        plugin.author.hash(&mut hasher);
    }
    hasher.finish()
}

impl MenuBarState {
    pub fn new() -> Self {
        Self
    }

    /// Build the menu bar view based on the current edit mode
    pub fn view(
        &self,
        mode: &EditMode,
        options: std::sync::Arc<parking_lot::RwLock<Settings>>,
        undo_info: &UndoInfo,
        marker_state: &MarkerMenuState,
        plugins: std::sync::Arc<Vec<Plugin>>,
        mirror_mode: bool,
        is_connected: bool,
        has_image_clipboard: bool,
    ) -> Element<'_, Message> {
        let recent_files_hash = {
            let options_guard = options.read();
            hash_recent_files(&options_guard.recent_files)
        };

        let plugins_hash = hash_plugins(plugins.as_ref());

        let mode_tag = match mode {
            EditMode::Ansi => 0,
            EditMode::BitFont => 1,
            EditMode::CharFont => 2,
            EditMode::Animation => 3,
        };

        let key = MenuBarCacheKey {
            mode_tag,
            undo_description: undo_info.undo_description.clone(),
            redo_description: undo_info.redo_description.clone(),
            guide: marker_state.guide,
            guide_visible: marker_state.guide_visible,
            raster: marker_state.raster,
            raster_visible: marker_state.raster_visible,
            line_numbers_visible: marker_state.line_numbers_visible,
            layer_borders_visible: marker_state.layer_borders_visible,
            mirror_mode,
            recent_files_hash,
            plugins_hash,
            is_connected,
            has_image_clipboard,
        };

        // Cache the whole menu subtree. During resize, this avoids rebuilding all menu widgets and
        // regenerating strings/translations.
        iced::widget::lazy(key, move |key: &MenuBarCacheKey| {
            let undo_info = UndoInfo {
                undo_description: key.undo_description.clone(),
                redo_description: key.redo_description.clone(),
            };

            let marker_state = MarkerMenuState {
                guide: key.guide,
                guide_visible: key.guide_visible,
                raster: key.raster,
                raster_visible: key.raster_visible,
                line_numbers_visible: key.line_numbers_visible,
                layer_borders_visible: key.layer_borders_visible,
            };

            let options_guard = options.read();
            let recent_files = &options_guard.recent_files;

            match key.mode_tag {
                0 => ansi::widget::toolbar::menu_bar::view_ansi(
                    recent_files,
                    &undo_info,
                    &marker_state,
                    plugins.as_ref(),
                    key.mirror_mode,
                    key.is_connected,
                    key.has_image_clipboard,
                ),
                1 => bitfont::menu_bar::view_bitfont(recent_files, undo_info.undo_description.as_deref(), undo_info.redo_description.as_deref()),
                2 => charfont::menu_bar::view_charfont(recent_files, &undo_info),
                3 => animation::menu_bar::view_animation_menu(recent_files, undo_info.undo_description.as_deref(), undo_info.redo_description.as_deref()),
                _ => ansi::widget::toolbar::menu_bar::view_ansi(
                    recent_files,
                    &undo_info,
                    &marker_state,
                    plugins.as_ref(),
                    key.mirror_mode,
                    key.is_connected,
                    key.has_image_clipboard,
                ),
            }
        })
        .into()
    }
}

// ============================================================================
// Public helper functions for editor menu modules
// ============================================================================

/// Create a menu bar button
pub fn menu_button(label: String) -> Element<'static, Message> {
    button(text(label).size(14))
        .padding([4, 8])
        .style(menu_button_style)
        .on_press(Message::Noop) // Dummy message - menu handles the interaction
        .into()
}

/// Create a menu item from a command definition
pub fn menu_item(cmd: &icy_engine_gui::commands::CommandDef, msg: Message) -> Element<'static, Message> {
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
pub fn menu_item_undo(cmd: &icy_engine_gui::commands::CommandDef, msg: Message, description: Option<&str>) -> Element<'static, Message> {
    let base_label = if cmd.label_menu.is_empty() { cmd.id.clone() } else { cmd.label_menu.clone() };
    let label = match description {
        Some(desc) => format!("{} {}", base_label, desc),
        None => base_label,
    };
    let hotkey = cmd.primary_hotkey_display().unwrap_or_default();
    let is_enabled = description.is_some();

    // Always set on_press to avoid iced_aw menu overlay issues
    // The message handler will ignore the action if nothing to undo
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
    .style(if is_enabled { menu_item_style } else { menu_item_disabled_style })
    .on_press(msg)
    .into()
}

/// Create a Redo menu item with optional operation description
pub fn menu_item_redo(cmd: &icy_engine_gui::commands::CommandDef, msg: Message, description: Option<&str>) -> Element<'static, Message> {
    let base_label = if cmd.label_menu.is_empty() { cmd.id.clone() } else { cmd.label_menu.clone() };
    let label = match description {
        Some(desc) => format!("{} {}", base_label, desc),
        None => base_label,
    };
    let hotkey = cmd.primary_hotkey_display().unwrap_or_default();
    let is_enabled = description.is_some();

    // Always set on_press to avoid iced_aw menu overlay issues
    // The message handler will ignore the action if nothing to redo
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
    .style(if is_enabled { menu_item_style } else { menu_item_disabled_style })
    .on_press(msg)
    .into()
}

/// Create a menu item with direct label and hotkey (without CommandDef)
pub fn menu_item_simple(label: String, hotkey: &str, msg: Message) -> Element<'static, Message> {
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

/// Create a menu item with direct label and hotkey that can be enabled/disabled
pub fn menu_item_simple_enabled(label: String, hotkey: &str, msg: Message, enabled: bool) -> Element<'static, Message> {
    let hotkey_text = hotkey.to_string();

    let btn = button(
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
    .style(if enabled { menu_item_style } else { menu_item_disabled_style });

    if enabled { btn.on_press(msg).into() } else { btn.into() }
}

/// Create a menu item with checkbox indicator
pub fn menu_item_checkbox(label: String, hotkey: &str, checked: bool, msg: Message) -> Element<'static, Message> {
    let hotkey_text = hotkey.to_string();
    let checkbox_indicator = if checked { "☑" } else { "☐" };

    button(
        row![
            text(checkbox_indicator).size(14).width(Length::Fixed(16.0)),
            text(label).size(14).width(Length::Fill),
            text(hotkey_text).size(12).style(|theme: &Theme| {
                iced::widget::text::Style {
                    color: Some(theme.palette().text.scale_alpha(0.6)),
                }
            }),
        ]
        .spacing(8)
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .padding([4, 8])
    .style(menu_item_style)
    .on_press(msg)
    .into()
}

/// Create a separator line
pub fn separator() -> quad::Quad {
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
pub fn menu_item_submenu(label: String) -> Element<'static, Message> {
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
    .on_press(Message::Noop) // Dummy - submenu handles interaction
    .into()
}

/// Build the recent files submenu
pub fn build_recent_files_menu(recent_files: &MostRecentlyUsedFiles) -> Menu<'static, Message, Theme, iced::Renderer> {
    let files = recent_files.files();

    let mut items: Vec<iced_aw::menu::Item<'static, Message, Theme, iced::Renderer>> = Vec::new();

    if files.is_empty() {
        // Show "No recent files" when empty
        items.push(iced_aw::menu::Item::new(
            button(text(fl!("menu-no_recent_files")).size(14))
                .width(Length::Fill)
                .padding([4, 8])
                .style(menu_item_disabled_style),
        ));
    } else {
        // Show files in reverse order (most recent first)
        for file in files.iter().rev() {
            let file_name = file
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| file.display().to_string());
            let path = file.clone();

            items.push(iced_aw::menu::Item::new(
                button(text(file_name).size(14))
                    .width(Length::Fill)
                    .padding([4, 8])
                    .style(menu_item_style)
                    .on_press(Message::OpenRecentFile(path)),
            ));
        }

        // Add separator and clear option
        items.push(iced_aw::menu::Item::new(separator()));
        items.push(iced_aw::menu::Item::new(
            button(text(fl!("menu-clear_recent_files")).size(14))
                .width(Length::Fill)
                .padding([4, 8])
                .style(menu_item_style)
                .on_press(Message::ClearRecentFiles),
        ));
    }

    Menu::new(items).width(350.0).offset(0.0)
}

// ============================================================================
// Styles
// ============================================================================

pub fn menu_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        text_color: palette.background.base.text,
        border: Border::default().rounded(6.0),
        ..Default::default()
    };
    match status {
        button::Status::Active => base.with_background(Color::TRANSPARENT),
        button::Status::Hovered => base.with_background(palette.primary.weak.color),
        button::Status::Pressed => base.with_background(palette.primary.strong.color),
        button::Status::Disabled => base.with_background(Color::from_rgb(0.5, 0.5, 0.5)),
    }
}

pub fn menu_item_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        text_color: palette.background.base.text,
        border: Border::default().rounded(6.0),
        ..Default::default()
    };

    match status {
        button::Status::Active => base.with_background(Color::TRANSPARENT),
        button::Status::Hovered => base.with_background(palette.primary.weak.color),
        button::Status::Pressed => base.with_background(palette.primary.weak.color),
        button::Status::Disabled => base.with_background(Color::from_rgb(0.5, 0.5, 0.5)),
    }
}

pub fn menu_item_disabled_style(theme: &Theme, _status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    button::Style {
        text_color: palette.background.base.text.scale_alpha(0.5),
        border: Border::default().rounded(6.0),
        background: Some(Color::TRANSPARENT.into()),
        ..Default::default()
    }
}
