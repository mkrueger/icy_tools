//! Top toolbar component
//!
//! Shows tool-specific options in a horizontal bar above the canvas.
//! Inspired by Moebius toolbar design.

use iced::{
    Element, Length, Task, Theme,
    widget::{Space, button, container, row, svg, text, toggler},
};

use super::segmented_control_gpu::{Segment, SegmentedControlMessage, ShaderSegmentedControl};
use super::tools::Tool;
use crate::ui::FKeySets;
use icy_engine::{BitFont, BufferType, Palette};

// Navigation icons for F-key set chooser
const NAV_PREV_SVG: &[u8] = include_bytes!("../../../data/icons/navigate_prev.svg");
const NAV_NEXT_SVG: &[u8] = include_bytes!("../../../data/icons/navigate_next.svg");

// Arrow icons for brush size selector
const ARROW_LEFT_SVG: &[u8] = include_bytes!("../../../data/icons/arrow_left.svg");
const ARROW_RIGHT_SVG: &[u8] = include_bytes!("../../../data/icons/arrow_right.svg");

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
    /// Set the primary brush mode (exclusive)
    SetBrushPrimary(BrushPrimaryMode),
    /// Clicked the current brush character button.
    /// If Char mode is already active, this requests opening the char table.
    BrushCharButton,
    /// Request to open the brush character table
    OpenBrushCharTable,
    /// Set the current brush character
    SetBrushChar(char),
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
    /// Type the character assigned to the given F-key slot
    TypeFKey(usize),
    /// Navigate F-key page
    NextFKeyPage,
    /// Navigate F-key page
    PrevFKeyPage,
    /// Set selection mode
    SetSelectionMode(SelectionMode),
}

/// Primary brush mode (exclusive).
///
/// Note: only `colorize_fg`/`colorize_bg` are additive flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BrushPrimaryMode {
    /// Paint with a chosen character
    #[default]
    Char,
    /// Half-block drawing mode
    HalfBlock,
    /// Shade up/down drawing mode
    Shading,
    /// Replace-color mode
    Replace,
    /// Blink attribute mode
    Blink,
    /// Colorize mode (only affects attributes)
    Colorize,
}

/// Brush mode options
#[derive(Clone, Debug, Default)]
pub struct BrushOptions {
    /// Primary brush mode (exclusive)
    pub primary: BrushPrimaryMode,
    /// Character used when `primary == BrushPrimaryMode::Char`
    pub paint_char: char,
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
    /// GPU Segmented control for selection mode
    pub selection_mode_control: ShaderSegmentedControl,
    /// GPU Segmented control for brush mode
    pub brush_mode_control: ShaderSegmentedControl,
    /// GPU Segmented control for color filter (FG/BG toggles)
    pub color_filter_control: ShaderSegmentedControl,
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
                primary: BrushPrimaryMode::Char,
                paint_char: '\u{00B0}',
                brush_size: 1,
                colorize_fg: true,
                colorize_bg: true,
                ..Default::default()
            },
            select_options: SelectOptions::default(),
            filled: false,
            selection_mode_control: ShaderSegmentedControl::new(),
            brush_mode_control: ShaderSegmentedControl::new(),
            color_filter_control: ShaderSegmentedControl::new(),
        }
    }

    /// Update the top toolbar state
    pub fn update(&mut self, message: TopToolbarMessage) -> Task<TopToolbarMessage> {
        match message {
            TopToolbarMessage::SetBrushPrimary(mode) => self.brush_options.primary = mode,
            TopToolbarMessage::BrushCharButton => {
                // If Char is already active, request opening the char table.
                if self.brush_options.primary == BrushPrimaryMode::Char {
                    return Task::done(TopToolbarMessage::OpenBrushCharTable);
                }
                self.brush_options.primary = BrushPrimaryMode::Char;
            }
            TopToolbarMessage::OpenBrushCharTable => {
                // handled at a higher level (AnsiEditor)
            }
            TopToolbarMessage::SetBrushChar(ch) => self.brush_options.paint_char = ch,
            TopToolbarMessage::ToggleColorizeFg(v) => self.brush_options.colorize_fg = v,
            TopToolbarMessage::ToggleColorizeBg(v) => self.brush_options.colorize_bg = v,
            TopToolbarMessage::SetBrushSize(s) => self.brush_options.brush_size = s.max(1).min(9),
            TopToolbarMessage::IncrementBrushSize => {
                self.brush_options.brush_size = (self.brush_options.brush_size + 1).min(9);
            }
            TopToolbarMessage::DecrementBrushSize => {
                self.brush_options.brush_size = self.brush_options.brush_size.saturating_sub(1).max(1);
            }
            TopToolbarMessage::ToggleFilled(v) => self.filled = v,
            TopToolbarMessage::SelectFKeySlot(slot) => self.select_options.selected_fkey = slot,
            TopToolbarMessage::TypeFKey(_) => {
                // handled at a higher level (AnsiEditor)
            }
            TopToolbarMessage::NextFKeyPage => {
                // handled at a higher level (AnsiEditor)
            }
            TopToolbarMessage::PrevFKeyPage => {
                // handled at a higher level (AnsiEditor)
            }
            TopToolbarMessage::SetSelectionMode(mode) => {
                self.select_options.selection_mode = mode;
            }
        }
        Task::none()
    }

    /// Render the top toolbar based on current tool
    pub fn view(
        &self,
        current_tool: Tool,
        fkeys: &FKeySets,
        buffer_type: BufferType,
        font: Option<BitFont>,
        theme: &Theme,
        caret_fg: u32,
        caret_bg: u32,
        palette: &Palette,
    ) -> Element<'_, TopToolbarMessage> {
        let content: Element<'_, TopToolbarMessage> = match current_tool {
            Tool::Click => self.view_click_panel(fkeys, buffer_type),
            Tool::Select => self.view_select_panel(font.clone(), theme),
            Tool::Pencil | Tool::Brush | Tool::Erase => {
                self.view_brush_panel(font, theme, caret_fg, caret_bg, palette)
            }
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
            .height(Length::Fill)
            // Keep horizontal spacing, but don't eat vertical height.
            // Vertical padding here shrinks children (e.g. SegmentedControl) from 54px to 46px.
            .padding([0, 4])
            .center_y(Length::Fill)
            .style(container::rounded_box)
            .into()
    }

    /// Selection tool panel with segmented control for mode selection
    fn view_select_panel(&self, font: Option<BitFont>, theme: &Theme) -> Element<'_, TopToolbarMessage> {
        let mode = self.select_options.selection_mode;

        // Build segments for the segmented control
        let segments = vec![
            Segment::text("Rect", SelectionMode::Normal),
            Segment::text("Char", SelectionMode::Character),
            Segment::text("Attr", SelectionMode::Attribute),
            Segment::text("Fg", SelectionMode::Foreground),
            Segment::text("Bg", SelectionMode::Background),
        ];

        // Convert SegmentedControlMessage to TopToolbarMessage
        let segmented_control = self.selection_mode_control.view(segments, mode, font, theme).map(|msg| match msg {
            SegmentedControlMessage::Selected(m) => TopToolbarMessage::SetSelectionMode(m),
            SegmentedControlMessage::Toggled(m) => TopToolbarMessage::SetSelectionMode(m),
            SegmentedControlMessage::CharClicked(m) => TopToolbarMessage::SetSelectionMode(m),
        });

        // Center the control with flexible space on both sides
        row![
            Space::new().width(Length::Fill),
            segmented_control,
            Space::new().width(Length::Fixed(24.0)),
            text("⇧: add   ⌃/Ctrl: remove").size(14),
            Space::new().width(Length::Fill),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .into()
    }

    fn view_click_panel(&self, fkeys: &FKeySets, buffer_type: BufferType) -> Element<'_, TopToolbarMessage> {
        let set_idx = self.select_options.current_fkey_page;
        let set_count = fkeys.set_count();

        let mut keys = row![].spacing(8).align_y(iced::Alignment::Center);

        for slot in 0..12usize {
            let code = fkeys.code_at(set_idx, slot);
            let raw = char::from_u32(code as u32).unwrap_or(' ');

            // Interpret stored code as CP437, then map to the current buffer type for display.
            let unicode_cp437 = BufferType::CP437.convert_to_unicode(raw);
            let target = buffer_type.convert_from_unicode(unicode_cp437);
            let display = buffer_type.convert_to_unicode(target);

            // Label + clickable char (no button backdrop)
            let fkey_label = text(format!("F{}:", slot + 1)).size(12);
            let char_text = button(text(display.to_string()).size(14))
                .padding([1, 4])
                .style(button::text)
                .on_press(TopToolbarMessage::TypeFKey(slot));

            keys = keys.push(row![fkey_label, char_text].spacing(1).align_y(iced::Alignment::Center));
        }

        // SVG navigation arrows (no button backdrop)
        let prev_icon = svg(svg::Handle::from_memory(NAV_PREV_SVG))
            .width(Length::Fixed(16.0))
            .height(Length::Fixed(16.0));
        let next_icon = svg(svg::Handle::from_memory(NAV_NEXT_SVG))
            .width(Length::Fixed(16.0))
            .height(Length::Fixed(16.0));

        let chooser = row![
            button(prev_icon).padding(2).style(button::text).on_press(TopToolbarMessage::PrevFKeyPage),
            text(format!("{}", set_idx.saturating_add(1).min(set_count).max(1))).size(14),
            button(next_icon).padding(2).style(button::text).on_press(TopToolbarMessage::NextFKeyPage),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);

        let content = row![keys, Space::new().width(Length::Fixed(16.0)), chooser]
            .spacing(8)
            .align_y(iced::Alignment::Center);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    /// Brush tool panel
    fn view_brush_panel(
        &self,
        font: Option<BitFont>,
        theme: &Theme,
        caret_fg: u32,
        caret_bg: u32,
        palette: &Palette,
    ) -> Element<'_, TopToolbarMessage> {
        let primary = self.brush_options.primary;

        // Build segments for the brush mode segmented control
        // First segment shows the current paint char - clicking when selected opens char picker
        let segments = vec![
            Segment::text("Half Block", BrushPrimaryMode::HalfBlock),
            Segment::char(self.brush_options.paint_char, BrushPrimaryMode::Char),
            Segment::text("Shade", BrushPrimaryMode::Shading),
            Segment::text("Replace", BrushPrimaryMode::Replace),
            Segment::text("Blink", BrushPrimaryMode::Blink),
            Segment::text("Colorize", BrushPrimaryMode::Colorize),
        ];

        // Convert SegmentedControlMessage to TopToolbarMessage
        // Use view_with_char_colors to render Char segments with caret colors
        let font_for_color_filter = font.clone();
        let segmented_control = self
            .brush_mode_control
            .view_with_char_colors(segments, primary, font, theme, caret_fg, caret_bg, palette)
            .map(|msg| match msg {
                SegmentedControlMessage::Selected(m) => TopToolbarMessage::SetBrushPrimary(m),
                SegmentedControlMessage::Toggled(m) => TopToolbarMessage::SetBrushPrimary(m),
                SegmentedControlMessage::CharClicked(_) => TopToolbarMessage::OpenBrushCharTable,
            });

        // FG/BG color filter toggles - always visible as a pill-pair
        // Index 0 = FG, Index 1 = BG
        let color_filter_segments = vec![Segment::text("FG", 0usize), Segment::text("BG", 1usize)];
        let mut selected_indices = Vec::new();
        if self.brush_options.colorize_fg {
            selected_indices.push(0);
        }
        if self.brush_options.colorize_bg {
            selected_indices.push(1);
        }
        let color_filter = self
            .color_filter_control
            .view_multi_select(color_filter_segments, &selected_indices, font_for_color_filter, theme)
            .map(|msg| match msg {
                SegmentedControlMessage::Toggled(0) => TopToolbarMessage::ToggleColorizeFg(!self.brush_options.colorize_fg),
                SegmentedControlMessage::Toggled(1) => TopToolbarMessage::ToggleColorizeBg(!self.brush_options.colorize_bg),
                _ => TopToolbarMessage::ToggleColorizeFg(self.brush_options.colorize_fg), // no-op fallback
            });

        // Brush size selector with SVG arrow icons
        let secondary_color = theme.extended_palette().secondary.base.color;
        let base_color = theme.extended_palette().primary.base.color;
        let left_arrow = svg(svg::Handle::from_memory(ARROW_LEFT_SVG))
            .width(Length::Fixed(32.0))
            .height(Length::Fixed(32.0))
            .style(move |_theme, status| {
                let color = match status {
                    svg::Status::Hovered => base_color,
                    _ => secondary_color,
                };
                svg::Style { color: Some(color) }
            });
        let right_arrow = svg(svg::Handle::from_memory(ARROW_RIGHT_SVG))
            .width(Length::Fixed(32.0))
            .height(Length::Fixed(32.0))
            .style(move |_theme, status| {
                let color = match status {
                    svg::Status::Hovered => base_color,
                    _ => secondary_color,
                };
                svg::Style { color: Some(color) }
            });

        // Size number in secondary color, monospace 14pt
        let size_text = text(format!("{}", self.brush_options.brush_size))
            .size(14)
            .font(iced::Font::MONOSPACE)
            .style(|theme: &Theme| {
                text::Style {
                    color: Some(theme.extended_palette().secondary.base.color),
                }
            });

        // Center the control with flexible space on both sides
        row![
            Space::new().width(Length::Fill),
            segmented_control,
            Space::new().width(Length::Fixed(16.0)),
            color_filter,
            Space::new().width(Length::Fixed(16.0)),
            button(left_arrow)
                .on_press(TopToolbarMessage::DecrementBrushSize)
                .padding(2)
                .style(|theme: &Theme, status| {
                    let secondary = theme.extended_palette().secondary.base.color;
                    let base = theme.extended_palette().primary.base.color;
                    let text_color = match status {
                        button::Status::Hovered | button::Status::Pressed => base,
                        _ => secondary,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
                        border: iced::Border {
                            color: iced::Color::TRANSPARENT,
                            width: 0.0,
                            radius: 0.0.into(),
                        },
                        text_color,
                        ..Default::default()
                    }
                }),
            size_text,
            button(right_arrow)
                .on_press(TopToolbarMessage::IncrementBrushSize)
                .padding(2)
                .style(|theme: &Theme, status| {
                    let secondary = theme.extended_palette().secondary.base.color;
                    let base = theme.extended_palette().primary.base.color;
                    let text_color = match status {
                        button::Status::Hovered | button::Status::Pressed => base,
                        _ => secondary,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
                        border: iced::Border {
                            color: iced::Color::TRANSPARENT,
                            width: 0.0,
                            radius: 0.0.into(),
                        },
                        text_color,
                        ..Default::default()
                    }
                }),
            Space::new().width(Length::Fill),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center)
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
