//! Shape Tools (Line, Rectangle, Ellipse - Outline and Filled)
//!
//! Draws geometric shapes between two drag points.
//! Supports half-block mode for higher resolution.
//!
//! This tool handles: Line, RectangleOutline, RectangleFilled, EllipseOutline, EllipseFilled

use iced::widget::{Space, button, row, svg, text, toggler};
use iced::{Element, Length, Theme};
use icy_engine::{MouseButton, Position, TextPane};
use icy_engine_edit::AttributedChar;
use icy_engine_gui::TerminalMessage;

use super::paint::{BrushSettings, apply_stamp_at_doc_pos, begin_paint_undo};
use super::{ToolContext, ToolHandler, ToolId, ToolMessage, ToolResult, ToolViewContext, UiAction};
use crate::ui::editor::ansi::shape_points::shape_points;
use crate::ui::editor::ansi::widget::segmented_control::gpu::{Segment, SegmentedControlMessage, ShaderSegmentedControl};
use crate::ui::editor::ansi::widget::toolbar::top::BrushPrimaryMode;
use crate::ui::editor::ansi::widget::toolbar::top::{ARROW_LEFT_SVG, ARROW_RIGHT_SVG};
use icy_engine_edit::tools::Tool;

/// Shape tool state
pub struct ShapeTool {
    /// Start position of the shape
    start_pos: Option<Position>,
    /// Current end position (during drag)
    current_pos: Option<Position>,

    /// Start position in layer-local half-block coordinates (Y has 2x resolution)
    start_half_block: Option<Position>,
    /// Current end position in layer-local half-block coordinates (during drag)
    current_half_block: Option<Position>,
    /// Whether currently dragging
    is_dragging: bool,
    /// Mouse button used for drawing
    draw_button: MouseButton,
    /// Whether to clear/erase instead of draw (Shift modifier)
    clear_mode: bool,

    tool: Tool,
    brush: BrushSettings,

    brush_mode_control: ShaderSegmentedControl,
    color_filter_control: ShaderSegmentedControl,

    undo: Option<icy_engine_edit::AtomicUndoGuard>,
}

impl Default for ShapeTool {
    fn default() -> Self {
        Self {
            start_pos: None,
            current_pos: None,
            start_half_block: None,
            current_half_block: None,
            is_dragging: false,
            draw_button: MouseButton::Left,
            clear_mode: false,
            tool: Tool::RectangleOutline,
            brush: BrushSettings::default(),

            brush_mode_control: ShaderSegmentedControl::new(),
            color_filter_control: ShaderSegmentedControl::new(),
            undo: None,
        }
    }
}

impl ShapeTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel_drag(&mut self) {
        self.start_pos = None;
        self.current_pos = None;
        self.start_half_block = None;
        self.current_half_block = None;
        self.is_dragging = false;
        self.draw_button = MouseButton::Left;
        self.clear_mode = false;
        self.undo = None;
    }

    pub fn is_dragging(&self) -> bool {
        self.is_dragging
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

    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
    }

    pub fn set_brush(&mut self, brush: BrushSettings) {
        self.brush = brush;
    }

    pub fn drag_snapshot(&self) -> Option<ShapeDragSnapshot> {
        Some(ShapeDragSnapshot {
            start_pos: self.start_pos?,
            current_pos: self.current_pos?,
            start_half_block: self.start_half_block,
            current_half_block: self.current_half_block,
            draw_button: self.draw_button,
            clear_mode: self.clear_mode,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ShapeDragSnapshot {
    pub start_pos: Position,
    pub current_pos: Position,
    pub start_half_block: Option<Position>,
    pub current_half_block: Option<Position>,
    pub draw_button: MouseButton,
    pub clear_mode: bool,
}

impl ToolHandler for ShapeTool {
    fn id(&self) -> ToolId {
        ToolId::Tool(self.tool)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
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
            ToolMessage::ToggleFilled(v) => {
                let new_tool = match self.tool {
                    Tool::RectangleOutline | Tool::RectangleFilled => {
                        if v {
                            Tool::RectangleFilled
                        } else {
                            Tool::RectangleOutline
                        }
                    }
                    Tool::EllipseOutline | Tool::EllipseFilled => {
                        if v {
                            Tool::EllipseFilled
                        } else {
                            Tool::EllipseOutline
                        }
                    }
                    _ => self.tool,
                };

                if new_tool != self.tool {
                    ToolResult::SwitchTool(super::ToolId::Tool(new_tool))
                } else {
                    ToolResult::None
                }
            }
            _ => ToolResult::None,
        }
    }

    fn view_toolbar(&self, ctx: &ToolViewContext) -> Element<'_, ToolMessage> {
        let primary = self.brush.primary;
        let segments = vec![
            Segment::text("Half Block", BrushPrimaryMode::HalfBlock),
            Segment::char(self.brush.paint_char, BrushPrimaryMode::Char),
            Segment::text("Shade", BrushPrimaryMode::Shading),
            Segment::text("Replace", BrushPrimaryMode::Replace),
            Segment::text("Blink", BrushPrimaryMode::Blink),
            Segment::text("Colorize", BrushPrimaryMode::Colorize),
        ];

        let font_for_color_filter = ctx.font.clone();
        let segmented_control = self
            .brush_mode_control
            .view_with_char_colors(segments, primary, ctx.font.clone(), &ctx.theme, ctx.caret_fg, ctx.caret_bg, &ctx.palette)
            .map(|msg| match msg {
                SegmentedControlMessage::Selected(m) | SegmentedControlMessage::Toggled(m) => ToolMessage::SetBrushPrimary(m),
                SegmentedControlMessage::CharClicked(_) => ToolMessage::BrushOpenCharSelector,
            });

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
            .view_multi_select(color_filter_segments, &selected_indices, font_for_color_filter, &ctx.theme)
            .map(|msg| match msg {
                SegmentedControlMessage::Toggled(0) => ToolMessage::ToggleForeground(!self.brush.colorize_fg),
                SegmentedControlMessage::Toggled(1) => ToolMessage::ToggleBackground(!self.brush.colorize_bg),
                _ => ToolMessage::ToggleForeground(self.brush.colorize_fg),
            });

        let show_filled_toggle = matches!(
            self.tool,
            Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled
        );
        let is_filled = matches!(self.tool, Tool::RectangleFilled | Tool::EllipseFilled);
        let filled_toggle: Element<'_, ToolMessage> = if show_filled_toggle {
            toggler(is_filled).label("Filled").on_toggle(ToolMessage::ToggleFilled).text_size(11).into()
        } else {
            Space::new().width(Length::Fixed(0.0)).into()
        };

        let secondary_color = ctx.theme.extended_palette().secondary.base.color;
        let base_color = ctx.theme.extended_palette().primary.base.color;
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
            filled_toggle,
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
    fn handle_terminal_message(&mut self, _ctx: &mut ToolContext, msg: &TerminalMessage) -> ToolResult {
        match msg {
            TerminalMessage::Press(evt) => {
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };

                self.start_pos = Some(pos);
                self.current_pos = Some(pos);
                self.is_dragging = true;
                self.draw_button = evt.button;
                self.clear_mode = evt.modifiers.shift;

                let desc = format!("{} drawn", self.tool.name());
                self.undo = Some(begin_paint_undo(_ctx.state, desc));

                if let Some(mapper) = _ctx.half_block_mapper {
                    let hb = mapper.pixel_to_layer_half_block(evt.pixel_position);
                    self.start_half_block = Some(hb);
                    self.current_half_block = Some(hb);
                } else {
                    self.start_half_block = None;
                    self.current_half_block = None;
                }

                ToolResult::StartCapture.and(ToolResult::Redraw)
            }

            TerminalMessage::Drag(evt) => {
                if self.is_dragging {
                    if let Some(pos) = evt.text_position {
                        self.current_pos = Some(pos);
                        if let Some(mapper) = _ctx.half_block_mapper {
                            self.current_half_block = Some(mapper.pixel_to_layer_half_block(evt.pixel_position));
                        }
                        return ToolResult::Redraw;
                    }
                }
                ToolResult::None
            }

            TerminalMessage::Release(evt) => {
                if self.is_dragging {
                    if let Some(pos) = evt.text_position {
                        self.current_pos = Some(pos);
                    }
                    if let Some(mapper) = _ctx.half_block_mapper {
                        self.current_half_block = Some(mapper.pixel_to_layer_half_block(evt.pixel_position));
                    }
                    self.is_dragging = false;

                    let Some(start) = self.start_pos else {
                        self.undo = None;
                        return ToolResult::EndCapture;
                    };
                    let Some(end) = self.current_pos else {
                        self.undo = None;
                        return ToolResult::EndCapture;
                    };

                    let primary = self.brush.primary;
                    let is_half_block_mode = matches!(primary, BrushPrimaryMode::HalfBlock);

                    if is_half_block_mode {
                        let (Some(start_hb), Some(end_hb)) = (self.start_half_block, self.current_half_block) else {
                            self.undo = None;
                            return ToolResult::EndCapture;
                        };

                        let pts_hb = shape_points(self.tool, start_hb, end_hb);
                        let offset = _ctx.state.get_cur_layer().map(|l| l.offset()).unwrap_or_default();
                        for p in pts_hb {
                            if p.y < 0 {
                                continue;
                            }
                            let cell_layer = Position::new(p.x, p.y / 2);
                            let is_top = (p.y % 2) == 0;
                            let cell_doc = cell_layer + offset;

                            if self.clear_mode {
                                let (layer_w, layer_h) = _ctx.state.get_cur_layer().map(|l| (l.width(), l.height())).unwrap_or((0, 0));
                                if cell_layer.x < 0 || cell_layer.y < 0 || cell_layer.x >= layer_w || cell_layer.y >= layer_h {
                                    continue;
                                }
                                if _ctx.state.is_something_selected() && !_ctx.state.is_selected(cell_doc) {
                                    continue;
                                }
                                let _ = _ctx.state.set_char_in_atomic(cell_layer, AttributedChar::invisible());
                            } else {
                                apply_stamp_at_doc_pos(_ctx.state, self.brush, cell_doc, is_top, self.draw_button);
                            }
                        }
                    } else {
                        let pts = shape_points(self.tool, start, end);
                        for p in pts {
                            if p.x < 0 || p.y < 0 {
                                continue;
                            }
                            if self.clear_mode {
                                let (offset, layer_w, layer_h) = if let Some(layer) = _ctx.state.get_cur_layer() {
                                    (layer.offset(), layer.width(), layer.height())
                                } else {
                                    continue;
                                };
                                let layer_pos = p - offset;
                                if layer_pos.x < 0 || layer_pos.y < 0 || layer_pos.x >= layer_w || layer_pos.y >= layer_h {
                                    continue;
                                }
                                if _ctx.state.is_something_selected() && !_ctx.state.is_selected(p) {
                                    continue;
                                }
                                let _ = _ctx.state.set_char_in_atomic(layer_pos, AttributedChar::invisible());
                            } else {
                                apply_stamp_at_doc_pos(_ctx.state, self.brush, p, true, self.draw_button);
                            }
                        }
                    }

                    let desc = format!("{} drawn", self.tool.name());

                    self.undo = None;
                    self.start_pos = None;
                    self.current_pos = None;
                    self.start_half_block = None;
                    self.current_half_block = None;

                    ToolResult::EndCapture.and(ToolResult::Commit(desc)).and(ToolResult::Redraw)
                } else {
                    ToolResult::None
                }
            }

            _ => ToolResult::None,
        }
    }

    fn view_status(&self, _ctx: &ToolViewContext) -> Element<'_, ToolMessage> {
        let status = if let (Some(start), Some(end)) = (self.start_pos, self.current_pos) {
            let w = (end.x - start.x).abs() + 1;
            let h = (end.y - start.y).abs() + 1;
            format!("Shape | ({},{}) → ({},{}) [{}x{}] | Shift=Erase", start.x, start.y, end.x, end.y, w, h)
        } else {
            "Shape | Click and drag to draw".to_string()
        };
        text(status).into()
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        iced::mouse::Interaction::Crosshair
    }

    fn show_caret(&self) -> bool {
        false
    }
}
