//! Pencil (Freehand Drawing) Tool
//!
//! Allows freehand drawing with various brush modes:
//! - Character mode: Stamps characters
//! - Half-block mode: 2x vertical resolution drawing
//! - Colorize mode: Changes only colors
//! - Shade mode: Lightens/darkens existing content

use super::{ToolContext, ToolHandler, ToolId, ToolMessage, ToolResult, ToolViewContext, UiAction};
use iced::widget::{Space, button, column, row, svg, text};
use iced::{Element, Length, Theme};
use icy_engine::{MouseButton, Position};
use icy_engine_edit::brushes;
use icy_engine_edit::tools::Tool;
use icy_engine_gui::TerminalMessage;

use super::paint::{BrushSettings, apply_stamp_at_doc_pos, begin_paint_undo};
use crate::ui::editor::ansi::widget::segmented_control::gpu::{Segment, SegmentedControlMessage, ShaderSegmentedControl};
use crate::ui::editor::ansi::widget::toolbar::top::BrushPrimaryMode;
use crate::ui::editor::ansi::widget::toolbar::top::{ARROW_LEFT_SVG, ARROW_RIGHT_SVG};

/// State for freehand pencil drawing
pub struct PencilTool {
    /// Whether a stroke is in progress
    is_drawing: bool,
    /// Last position for interpolation
    last_pos: Option<Position>,

    /// Last half-block position (layer-local, Y doubled)
    last_half_block: Option<Position>,

    /// Mouse button used for current stroke
    stroke_button: MouseButton,

    brush: BrushSettings,
    undo: Option<icy_engine_edit::AtomicUndoGuard>,

    brush_mode_control: ShaderSegmentedControl,
    color_filter_control: ShaderSegmentedControl,
}

impl Default for PencilTool {
    fn default() -> Self {
        Self {
            is_drawing: false,
            last_pos: None,
            last_half_block: None,
            stroke_button: MouseButton::Left,
            brush: BrushSettings::default(),
            undo: None,

            brush_mode_control: ShaderSegmentedControl::new(),
            color_filter_control: ShaderSegmentedControl::new(),
        }
    }
}

impl PencilTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_dragging(&self) -> bool {
        self.is_drawing
    }

    pub fn cancel_drag(&mut self) {
        self.is_drawing = false;
        self.last_pos = None;
        self.last_half_block = None;
        self.stroke_button = MouseButton::Left;
        self.undo = None;
    }

    pub fn set_brush(&mut self, brush: BrushSettings) {
        self.brush = brush;
    }

    pub fn brush_settings(&self) -> BrushSettings {
        self.brush
    }

    pub(crate) fn paint_char(&self) -> char {
        self.brush.paint_char
    }

    pub(crate) fn brush_primary(&self) -> BrushPrimaryMode {
        self.brush.primary
    }

    pub(crate) fn brush_size(&self) -> usize {
        self.brush.brush_size
    }

    fn apply_half_block_with_brush_size(&self, ctx: &mut ToolContext<'_>, half_block_layer: Position, button: MouseButton) {
        let brush_size = self.brush.brush_size.max(1) as i32;
        let half = brush_size / 2;

        let offset = ctx.state.get_cur_layer().map(|l| l.offset()).unwrap_or_default();

        for dy in 0..brush_size {
            for dx in 0..brush_size {
                let hb_x = half_block_layer.x + dx - half;
                let hb_y = half_block_layer.y + dy - half;

                if hb_y < 0 {
                    continue;
                }

                let cell_layer = Position::new(hb_x, hb_y / 2);
                let is_top = (hb_y % 2) == 0;
                let cell_doc = cell_layer + offset;

                apply_stamp_at_doc_pos(ctx.state, self.brush, cell_doc, is_top, button);
            }
        }
    }
}

impl ToolHandler for PencilTool {
    fn id(&self) -> ToolId {
        ToolId::Tool(Tool::Pencil)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn cancel_capture(&mut self) {
        self.cancel_drag();
    }

    fn handle_event(&mut self, _ctx: &mut ToolContext, event: &iced::Event) -> ToolResult {
        match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                use iced::keyboard::key::Named;
                use iced::keyboard::Key;

                // Moebius: Brush size shortcuts
                // - Alt+= increase
                // - Alt+- decrease
                // - Alt+0 reset to 1
                if modifiers.alt() && !modifiers.control() {
                    let mut changed = false;

                    match key {
                        Key::Character(ch) if ch == "=" || ch == "+" => {
                            let new_size = (self.brush.brush_size + 1).min(9);
                            if new_size != self.brush.brush_size {
                                self.brush.brush_size = new_size;
                                changed = true;
                            }
                        }
                        Key::Character(ch) if ch == "-" => {
                            let new_size = self.brush.brush_size.saturating_sub(1).max(1);
                            if new_size != self.brush.brush_size {
                                self.brush.brush_size = new_size;
                                changed = true;
                            }
                        }
                        Key::Character(ch) if ch == "0" => {
                            if self.brush.brush_size != 1 {
                                self.brush.brush_size = 1;
                                changed = true;
                            }
                        }
                        Key::Named(Named::Digit0) => {
                            if self.brush.brush_size != 1 {
                                self.brush.brush_size = 1;
                                changed = true;
                            }
                        }
                        _ => {}
                    }

                    if changed {
                        return ToolResult::Redraw;
                    }
                }

                ToolResult::None
            }
            _ => ToolResult::None,
        }
    }

    fn handle_terminal_message(&mut self, ctx: &mut ToolContext<'_>, msg: &TerminalMessage) -> ToolResult {
        match msg {
            TerminalMessage::Press(evt) => {
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };

                self.is_drawing = true;
                self.last_pos = Some(pos);
                self.stroke_button = evt.button;

                if self.undo.is_none() {
                    self.undo = Some(begin_paint_undo(ctx.state, "Pencil stroke".to_string()));
                }

                let primary = self.brush.primary;
                if matches!(primary, BrushPrimaryMode::HalfBlock) {
                    let Some(mapper) = ctx.half_block_mapper else {
                        return ToolResult::None;
                    };
                    let hb = mapper.pixel_to_layer_half_block(evt.pixel_position);
                    self.last_half_block = Some(hb);
                    self.apply_half_block_with_brush_size(ctx, hb, self.stroke_button);
                } else {
                    self.last_half_block = None;
                    apply_stamp_at_doc_pos(ctx.state, self.brush, pos, true, self.stroke_button);
                }
                ToolResult::Multi(vec![ToolResult::StartCapture, ToolResult::Redraw])
            }

            TerminalMessage::Drag(evt) => {
                if !self.is_drawing {
                    return ToolResult::None;
                }

                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };

                let primary = self.brush.primary;
                if matches!(primary, BrushPrimaryMode::HalfBlock) {
                    let Some(mapper) = ctx.half_block_mapper else {
                        return ToolResult::None;
                    };
                    let new_hb = mapper.pixel_to_layer_half_block(evt.pixel_position);

                    let mut cur = self.last_half_block.unwrap_or(new_hb);
                    while cur != new_hb {
                        let s = (new_hb - cur).signum();
                        cur = cur + s;
                        self.apply_half_block_with_brush_size(ctx, cur, self.stroke_button);
                    }
                    self.last_half_block = Some(new_hb);
                } else {
                    let Some(last) = self.last_pos else {
                        self.last_pos = Some(pos);
                        return ToolResult::Redraw;
                    };

                    let pts = brushes::get_line_points(last, pos);
                    for p in pts {
                        apply_stamp_at_doc_pos(ctx.state, self.brush, p, true, self.stroke_button);
                    }
                    self.last_pos = Some(pos);
                }

                ToolResult::Redraw
            }

            TerminalMessage::Release(_evt) => {
                if !self.is_drawing {
                    return ToolResult::None;
                }

                self.is_drawing = false;
                self.last_pos = None;
                self.last_half_block = None;

                // Drop guard to finish atomic undo entry.
                self.undo = None;

                ToolResult::Multi(vec![
                    ToolResult::EndCapture,
                    ToolResult::Commit("Pencil stroke".to_string()),
                    ToolResult::Redraw,
                ])
            }

            _ => ToolResult::None,
        }
    }

    fn handle_message(&mut self, _ctx: &mut ToolContext<'_>, msg: &ToolMessage) -> ToolResult {
        match *msg {
            ToolMessage::SetBrushPrimary(primary) => {
                self.brush.primary = primary;
                ToolResult::None
            }
            ToolMessage::BrushOpenCharSelector => ToolResult::Ui(UiAction::OpenCharSelectorForBrush),
            ToolMessage::SetBrushChar(ch) => {
                self.brush.paint_char = ch;
                ToolResult::None
            }
            ToolMessage::SetBrushSize(size) => {
                self.brush.brush_size = (size.max(1).min(9)) as usize;
                ToolResult::None
            }
            ToolMessage::ToggleForeground(v) => {
                self.brush.colorize_fg = v;
                ToolResult::None
            }
            ToolMessage::ToggleBackground(v) => {
                self.brush.colorize_bg = v;
                ToolResult::None
            }
            _ => ToolResult::None,
        }
    }

    fn view_toolbar(&self, _ctx: &ToolViewContext) -> Element<'_, ToolMessage> {
        let primary = self.brush.primary;
        let segments = vec![
            Segment::text("Half Block", BrushPrimaryMode::HalfBlock),
            Segment::char(self.brush.paint_char, BrushPrimaryMode::Char),
            Segment::text("Shade", BrushPrimaryMode::Shading),
            Segment::text("Replace", BrushPrimaryMode::Replace),
            Segment::text("Blink", BrushPrimaryMode::Blink),
            Segment::text("Colorize", BrushPrimaryMode::Colorize),
        ];

        let font_for_color_filter = _ctx.font.clone();
        let segmented_control = self
            .brush_mode_control
            .view_with_char_colors(segments, primary, _ctx.font.clone(), &_ctx.theme, _ctx.caret_fg, _ctx.caret_bg, &_ctx.palette)
            .map(|msg| match msg {
                SegmentedControlMessage::Selected(m) | SegmentedControlMessage::Toggled(m) => ToolMessage::SetBrushPrimary(m),
                SegmentedControlMessage::CharClicked(_) => ToolMessage::BrushOpenCharSelector,
            });

        // FG/BG filter
        let color_filter_segments = vec![Segment::text("FG", 0usize), Segment::text("BG", 1usize)];
        let mut selected_indices = Vec::new();
        if self.brush.colorize_fg {
            selected_indices.push(0);
        }
        if self.brush.colorize_bg {
            selected_indices.push(1);
        }
        let color_filter = self
            .color_filter_control
            .view_multi_select(color_filter_segments, &selected_indices, font_for_color_filter, &_ctx.theme)
            .map(|msg| match msg {
                SegmentedControlMessage::Toggled(0) => ToolMessage::ToggleForeground(!self.brush.colorize_fg),
                SegmentedControlMessage::Toggled(1) => ToolMessage::ToggleBackground(!self.brush.colorize_bg),
                _ => ToolMessage::ToggleForeground(self.brush.colorize_fg),
            });

        // Brush size arrows
        let secondary_color = _ctx.theme.extended_palette().secondary.base.color;
        let base_color = _ctx.theme.extended_palette().primary.base.color;
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

        let size_text = text(format!("{}", self.brush.brush_size))
            .size(14)
            .font(iced::Font::MONOSPACE)
            .style(|theme: &Theme| text::Style {
                color: Some(theme.extended_palette().secondary.base.color),
            });

        let dec_size = self.brush.brush_size.saturating_sub(1).max(1);
        let inc_size = (self.brush.brush_size + 1).min(9);

        row![
            Space::new().width(Length::Fill),
            segmented_control,
            Space::new().width(Length::Fixed(16.0)),
            color_filter,
            Space::new().width(Length::Fixed(16.0)),
            button(left_arrow)
                .on_press(ToolMessage::SetBrushSize(dec_size as u8))
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
                .on_press(ToolMessage::SetBrushSize(inc_size as u8))
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

    fn view_options(&self, _ctx: &ToolViewContext) -> Element<'_, ToolMessage> {
        // Brush options are rendered by AnsiEditor for now
        column![].into()
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        iced::mouse::Interaction::Crosshair
    }

    fn show_caret(&self) -> bool {
        false
    }

    fn show_selection(&self) -> bool {
        false
    }
}
