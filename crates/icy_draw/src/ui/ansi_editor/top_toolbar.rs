//! Top toolbar component
//!
//! Shows tool-specific options in a horizontal bar above the canvas.
//! Inspired by Moebius toolbar design.

use iced::{
    Element, Length, Task,
    widget::{Space, button, container, radio, row, text, toggler},
};

use super::tools::Tool;

/// Selection mode for the select tool
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionMode {
    /// Normal rectangle selection
    #[default]
    Normal,
    /// Select all cells with the same character
    Character,
    /// Select all cells with the same attribute
    Attribute,
    /// Select all cells with the same foreground color
    Foreground,
    /// Select all cells with the same background color
    Background,
}

/// Selection modifier based on keyboard modifiers
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionModifier {
    /// Replace the selection
    #[default]
    Replace,
    /// Add to the selection (Shift)
    Add,
    /// Remove from the selection (Ctrl/Cmd)
    Remove,
}

impl SelectionModifier {
    /// Get the response for a selection check
    /// Returns Some(true) to select, Some(false) to deselect, None to keep
    pub fn get_response(&self, matches: bool) -> Option<bool> {
        match self {
            SelectionModifier::Replace => Some(matches),
            SelectionModifier::Add => {
                if matches {
                    Some(true)
                } else {
                    None
                }
            }
            SelectionModifier::Remove => {
                if matches {
                    Some(false)
                } else {
                    None
                }
            }
        }
    }
}

/// Messages from the top toolbar
#[derive(Clone, Debug)]
pub enum TopToolbarMessage {
    /// Toggle half-block mode
    ToggleHalfBlock(bool),
    /// Toggle shading mode
    ToggleShading(bool),
    /// Toggle replace color mode
    ToggleReplaceColor(bool),
    /// Toggle blink mode
    ToggleBlink(bool),
    /// Toggle colorize mode
    ToggleColorize(bool),
    /// Toggle colorize foreground only
    ToggleColorizeFg(bool),
    /// Toggle colorize background only
    ToggleColorizeBg(bool),
    /// Change brush size
    SetBrushSize(u32),
    /// Increment brush size
    IncrementBrushSize,
    /// Decrement brush size
    DecrementBrushSize,
    /// Toggle filled shapes
    ToggleFilled(bool),
    /// Select F-key slot
    SelectFKeySlot(usize),
    /// Navigate F-key page
    NextFKeyPage,
    /// Navigate F-key page
    PrevFKeyPage,
    /// Set selection mode
    SetSelectionMode(SelectionMode),
}

/// Brush mode options
#[derive(Clone, Debug, Default)]
pub struct BrushOptions {
    pub half_block: bool,
    pub shading: bool,
    pub replace_color: bool,
    pub blink: bool,
    pub colorize: bool,
    pub colorize_fg: bool,
    pub colorize_bg: bool,
    pub brush_size: u32,
}

/// Selection mode options
#[derive(Clone, Debug, Default)]
pub struct SelectOptions {
    pub current_fkey_page: usize,
    pub selected_fkey: usize,
    pub selection_mode: SelectionMode,
}

/// Top toolbar state
pub struct TopToolbar {
    /// Brush options
    pub brush_options: BrushOptions,
    /// Selection options
    pub select_options: SelectOptions,
    /// Shape filled toggle
    pub filled: bool,
}

impl Default for TopToolbar {
    fn default() -> Self {
        Self::new()
    }
}

impl TopToolbar {
    pub fn new() -> Self {
        Self {
            brush_options: BrushOptions {
                brush_size: 1,
                ..Default::default()
            },
            select_options: SelectOptions::default(),
            filled: false,
        }
    }

    /// Update the top toolbar state
    pub fn update(&mut self, message: TopToolbarMessage) -> Task<TopToolbarMessage> {
        match message {
            TopToolbarMessage::ToggleHalfBlock(v) => self.brush_options.half_block = v,
            TopToolbarMessage::ToggleShading(v) => self.brush_options.shading = v,
            TopToolbarMessage::ToggleReplaceColor(v) => self.brush_options.replace_color = v,
            TopToolbarMessage::ToggleBlink(v) => self.brush_options.blink = v,
            TopToolbarMessage::ToggleColorize(v) => self.brush_options.colorize = v,
            TopToolbarMessage::ToggleColorizeFg(v) => self.brush_options.colorize_fg = v,
            TopToolbarMessage::ToggleColorizeBg(v) => self.brush_options.colorize_bg = v,
            TopToolbarMessage::SetBrushSize(s) => self.brush_options.brush_size = s.max(1).min(10),
            TopToolbarMessage::IncrementBrushSize => {
                self.brush_options.brush_size = (self.brush_options.brush_size + 1).min(10);
            }
            TopToolbarMessage::DecrementBrushSize => {
                self.brush_options.brush_size = self.brush_options.brush_size.saturating_sub(1).max(1);
            }
            TopToolbarMessage::ToggleFilled(v) => self.filled = v,
            TopToolbarMessage::SelectFKeySlot(slot) => self.select_options.selected_fkey = slot,
            TopToolbarMessage::NextFKeyPage => {
                self.select_options.current_fkey_page = (self.select_options.current_fkey_page + 1) % 10;
            }
            TopToolbarMessage::PrevFKeyPage => {
                self.select_options.current_fkey_page = (self.select_options.current_fkey_page + 9) % 10;
            }
            TopToolbarMessage::SetSelectionMode(mode) => {
                self.select_options.selection_mode = mode;
            }
        }
        Task::none()
    }

    /// Render the top toolbar based on current tool
    pub fn view(&self, current_tool: Tool) -> Element<'_, TopToolbarMessage> {
        let content: Element<'_, TopToolbarMessage> = match current_tool {
            Tool::Click | Tool::Select => self.view_select_panel(),
            Tool::Pencil | Tool::Brush | Tool::Erase => self.view_brush_panel(),
            Tool::Line => self.view_line_panel(),
            Tool::RectangleOutline | Tool::RectangleFilled => self.view_shape_panel("Rectangle"),
            Tool::EllipseOutline | Tool::EllipseFilled => self.view_shape_panel("Ellipse"),
            Tool::Fill => self.view_fill_panel(),
            Tool::Pipette => self.view_sample_panel(),
            Tool::Shifter => self.view_shifter_panel(),
            Tool::Font => self.view_font_panel(),
            Tool::Tag => self.view_tag_panel(),
        };

        container(content)
            .width(Length::Fill)
            .height(Length::Fixed(40.0))
            .padding(4)
            .style(container::rounded_box)
            .into()
    }

    /// Selection tool panel with selection mode options
    fn view_select_panel(&self) -> Element<'_, TopToolbarMessage> {
        let mode = self.select_options.selection_mode;

        row![
            text("Mode:").size(11),
            radio("Rectangle", SelectionMode::Normal, Some(mode), TopToolbarMessage::SetSelectionMode)
                .size(14)
                .text_size(11),
            radio("Character", SelectionMode::Character, Some(mode), TopToolbarMessage::SetSelectionMode)
                .size(14)
                .text_size(11),
            radio("Attribute", SelectionMode::Attribute, Some(mode), TopToolbarMessage::SetSelectionMode)
                .size(14)
                .text_size(11),
            radio("Foreground", SelectionMode::Foreground, Some(mode), TopToolbarMessage::SetSelectionMode)
                .size(14)
                .text_size(11),
            radio("Background", SelectionMode::Background, Some(mode), TopToolbarMessage::SetSelectionMode)
                .size(14)
                .text_size(11),
            Space::new().width(Length::Fixed(16.0)),
            text("⇧: add  ⌘/Ctrl: remove").size(10),
        ]
        .spacing(8)
        .into()
    }

    /// Brush tool panel
    fn view_brush_panel(&self) -> Element<'_, TopToolbarMessage> {
        row![
            toggler(self.brush_options.half_block)
                .label("Half Block")
                .on_toggle(TopToolbarMessage::ToggleHalfBlock)
                .text_size(11),
            toggler(self.brush_options.shading)
                .label("Shading")
                .on_toggle(TopToolbarMessage::ToggleShading)
                .text_size(11),
            toggler(self.brush_options.replace_color)
                .label("Replace")
                .on_toggle(TopToolbarMessage::ToggleReplaceColor)
                .text_size(11),
            toggler(self.brush_options.blink)
                .label("Blink")
                .on_toggle(TopToolbarMessage::ToggleBlink)
                .text_size(11),
            toggler(self.brush_options.colorize)
                .label("Colorize")
                .on_toggle(TopToolbarMessage::ToggleColorize)
                .text_size(11),
            Space::new().width(Length::Fixed(16.0)),
            text("Size:").size(11),
            button(text("-").size(12)).on_press(TopToolbarMessage::DecrementBrushSize).padding(2),
            text(format!("{}", self.brush_options.brush_size)).size(12),
            button(text("+").size(12)).on_press(TopToolbarMessage::IncrementBrushSize).padding(2),
        ]
        .spacing(8)
        .into()
    }

    /// Line tool panel
    fn view_line_panel(&self) -> Element<'_, TopToolbarMessage> {
        row![
            text("Line Tool").size(12),
            Space::new().width(Length::Fixed(16.0)),
            text("Shift: Constrain to 45°").size(10),
        ]
        .spacing(8)
        .into()
    }

    /// Shape tool panel (rectangle, ellipse)
    fn view_shape_panel<'a>(&self, shape_name: &'a str) -> Element<'a, TopToolbarMessage> {
        row![
            text(shape_name).size(12),
            Space::new().width(Length::Fixed(16.0)),
            toggler(self.filled).label("Filled").on_toggle(TopToolbarMessage::ToggleFilled).text_size(11),
            Space::new().width(Length::Fixed(16.0)),
            text("Shift: Square/Circle").size(10),
        ]
        .spacing(8)
        .into()
    }

    /// Fill tool panel
    fn view_fill_panel(&self) -> Element<'_, TopToolbarMessage> {
        row![
            text("Fill Tool").size(12),
            Space::new().width(Length::Fixed(16.0)),
            text("Click to flood fill area").size(10),
        ]
        .spacing(8)
        .into()
    }

    /// Sample/Pipette tool panel
    fn view_sample_panel(&self) -> Element<'_, TopToolbarMessage> {
        row![
            text("Pipette").size(12),
            Space::new().width(Length::Fixed(16.0)),
            text("Click: Pick color | Shift+Click: Pick character").size(10),
        ]
        .spacing(8)
        .into()
    }

    /// Shifter tool panel
    fn view_shifter_panel(&self) -> Element<'_, TopToolbarMessage> {
        row![
            text("Shifter").size(12),
            Space::new().width(Length::Fixed(16.0)),
            text("Drag to shift characters").size(10),
        ]
        .spacing(8)
        .into()
    }

    /// Font tool panel
    fn view_font_panel(&self) -> Element<'_, TopToolbarMessage> {
        row![
            text("Font Tool").size(12),
            Space::new().width(Length::Fixed(16.0)),
            text("Type with TDF fonts").size(10),
        ]
        .spacing(8)
        .into()
    }

    /// Tag tool panel
    fn view_tag_panel(&self) -> Element<'_, TopToolbarMessage> {
        row![
            text("Tag Tool").size(12),
            Space::new().width(Length::Fixed(16.0)),
            text("Click to add annotation tag").size(10),
        ]
        .spacing(8)
        .into()
    }
}
