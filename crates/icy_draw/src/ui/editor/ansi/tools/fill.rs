//! Fill Tool (Bucket Fill / Flood Fill)
//!
//! Fills connected regions with color/character.

use icy_engine::{AttributedChar, MouseButton, Position, TextAttribute, TextPane};
use icy_engine_gui::TerminalMessage;
use icy_ui::widget::{row, text, toggler, Space};
use icy_ui::{Element, Length};

use super::paint::SharedBrush;
use super::{BrushSettings, ToolContext, ToolHandler, ToolId, ToolMessage, ToolResult, ToolViewContext, UiAction};
use crate::ui::editor::ansi::widget::segmented_control::gpu::{Segment, SegmentedControlMessage, ShaderSegmentedControl};
use crate::ui::editor::ansi::widget::toolbar::top::BrushPrimaryMode;
use icy_engine_edit::tools::Tool;

/// Fill tool state
pub struct FillTool {
    /// Last fill position (for status display)
    last_fill_pos: Option<Position>,

    /// Shared brush state (owned by the editor, cloned across tools and toolbar).
    brush: SharedBrush,

    brush_mode_control: ShaderSegmentedControl,
    color_filter_control: ShaderSegmentedControl,
}

impl FillTool {
    pub fn new(brush: SharedBrush) -> Self {
        Self {
            last_fill_pos: None,
            brush,
            brush_mode_control: ShaderSegmentedControl::new(),
            color_filter_control: ShaderSegmentedControl::new(),
        }
    }

    pub fn set_brush_settings(&mut self, settings: BrushSettings) {
        *self.brush.write() = settings;
    }

    pub fn brush_settings(&self) -> BrushSettings {
        *self.brush.read()
    }

    pub(crate) fn paint_char(&self) -> char {
        self.brush.read().paint_char
    }

    pub(crate) fn brush_primary(&self) -> BrushPrimaryMode {
        self.brush.read().primary
    }
}

impl ToolHandler for FillTool {
    fn id(&self) -> ToolId {
        ToolId::Fill
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
                self.brush.write().primary = primary;
                ToolResult::None
            }
            ToolMessage::BrushOpenCharSelector => ToolResult::Ui(UiAction::OpenCharSelectorForBrush),
            ToolMessage::SetBrushChar(ch) => {
                self.brush.write().paint_char = ch;
                ToolResult::None
            }
            ToolMessage::ToggleForeground(v) => {
                self.brush.write().colorize_fg = v;
                ToolResult::None
            }
            ToolMessage::ToggleBackground(v) => {
                self.brush.write().colorize_bg = v;
                ToolResult::None
            }
            ToolMessage::FillToggleExact(v) => {
                self.brush.write().exact = v;
                ToolResult::None
            }
            _ => ToolResult::None,
        }
    }
    fn handle_terminal_message(&mut self, ctx: &mut ToolContext, msg: &TerminalMessage) -> ToolResult {
        match msg {
            TerminalMessage::Press(evt) => {
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };

                self.last_fill_pos = Some(pos);

                // Snapshot brush settings for the duration of this fill operation
                // (BrushSettings is Copy, so this releases the lock immediately).
                let settings = *self.brush.read();

                // Fill only supports HalfBlock / Char / Colorize.
                let primary = match settings.primary {
                    BrushPrimaryMode::HalfBlock | BrushPrimaryMode::Char | BrushPrimaryMode::Colorize => settings.primary,
                    _ => BrushPrimaryMode::Char,
                };

                // If Colorize mode is selected but no channels are enabled, do nothing.
                if matches!(primary, BrushPrimaryMode::Colorize) && !settings.colorize_fg && !settings.colorize_bg {
                    return ToolResult::None;
                }

                use std::collections::HashSet;

                let swap_colors = evt.button == MouseButton::Right;
                let shift_swap = evt.modifiers.shift;

                // Begin atomic undo for the entire fill.
                let _undo = ctx.state.begin_atomic_undo("Bucket fill".to_string());

                if matches!(primary, BrushPrimaryMode::HalfBlock) {
                    let Some(mapper) = ctx.half_block_mapper else {
                        return ToolResult::None;
                    };

                    let start_hb = mapper.pixel_to_layer_half_block(evt.pixel_position);

                    let (offset, width, height) = if let Some(layer) = ctx.state.get_cur_layer() {
                        (layer.offset(), layer.width(), layer.height())
                    } else {
                        return ToolResult::None;
                    };
                    let use_selection = ctx.state.is_something_selected();

                    let caret_attr = ctx.state.get_caret().attribute;
                    let (fg, bg) = if swap_colors || shift_swap {
                        (caret_attr.background_color(), caret_attr.foreground_color())
                    } else {
                        (caret_attr.foreground_color(), caret_attr.background_color())
                    };

                    // Determine the target color at the start position.
                    let start_cell = icy_engine::Position::new(start_hb.x, start_hb.y / 2);
                    if start_cell.x < 0 || start_hb.y < 0 || start_cell.x >= width || start_cell.y >= height {
                        return ToolResult::Commit("Bucket fill".to_string());
                    }

                    let start_char = { ctx.state.get_cur_layer().unwrap().char_at(start_cell) };
                    let start_block = icy_engine::paint::HalfBlock::from_char(start_char, start_hb);
                    if !start_block.is_blocky() {
                        return ToolResult::Commit("Bucket fill".to_string());
                    }
                    let target_color = if start_block.is_top {
                        start_block.upper_block_color
                    } else {
                        start_block.lower_block_color
                    };
                    // Don't fill if target color is the same as fill color
                    if target_color == fg {
                        return ToolResult::Commit("Bucket fill".to_string());
                    }

                    let mut visited: HashSet<icy_engine::Position> = HashSet::new();
                    let mut stack: Vec<(icy_engine::Position, icy_engine::Position)> = vec![(start_hb, start_hb)];

                    while let Some((from, to)) = stack.pop() {
                        let text_pos = icy_engine::Position::new(to.x, to.y / 2);
                        if to.x < 0 || to.y < 0 || to.x >= width || text_pos.y >= height || !visited.insert(to) {
                            continue;
                        }

                        if use_selection {
                            let doc_cell = text_pos + offset;
                            if !ctx.state.is_selected(doc_cell) {
                                continue;
                            }
                        }

                        let cur = { ctx.state.get_cur_layer().unwrap().char_at(text_pos) };
                        let block = icy_engine::paint::HalfBlock::from_char(cur, to);

                        if block.is_blocky()
                            && ((block.is_top && block.upper_block_color == target_color) || (!block.is_top && block.lower_block_color == target_color))
                        {
                            let ch = block.get_half_block_char(fg, true);
                            let _ = ctx.state.set_char_in_atomic(text_pos, ch);

                            stack.push((to, to + icy_engine::Position::new(-1, 0)));
                            stack.push((to, to + icy_engine::Position::new(1, 0)));
                            stack.push((to, to + icy_engine::Position::new(0, -1)));
                            stack.push((to, to + icy_engine::Position::new(0, 1)));
                        } else if block.is_vertically_blocky() {
                            // Vertikale Half-Blocks (links/rechts geteilt)
                            // Prüfe die Seite basierend auf der Richtung, aus der wir kommen
                            let ch = if from.x == to.x - 1 && block.left_block_color == target_color {
                                // Kommen von links, linke Seite hat target_color
                                Some(AttributedChar::new(221 as char, TextAttribute::from_colors(fg, block.right_block_color)))
                            } else if from.x == to.x + 1 && block.right_block_color == target_color {
                                // Kommen von rechts, rechte Seite hat target_color
                                Some(AttributedChar::new(222 as char, TextAttribute::from_colors(fg, block.left_block_color)))
                            } else if from.y != to.y {
                                // Kommen von oben oder unten - prüfe beide Seiten
                                if block.left_block_color == target_color {
                                    Some(AttributedChar::new(221 as char, TextAttribute::from_colors(fg, block.right_block_color)))
                                } else if block.right_block_color == target_color {
                                    Some(AttributedChar::new(222 as char, TextAttribute::from_colors(fg, block.left_block_color)))
                                } else {
                                    None
                                }
                            } else if from == to {
                                // Startpunkt - prüfe beide Seiten
                                if block.left_block_color == target_color {
                                    Some(AttributedChar::new(221 as char, TextAttribute::from_colors(fg, block.right_block_color)))
                                } else if block.right_block_color == target_color {
                                    Some(AttributedChar::new(222 as char, TextAttribute::from_colors(fg, block.left_block_color)))
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            if let Some(ch) = ch {
                                let _ = ctx.state.set_char_in_atomic(text_pos, ch);

                                // WICHTIG: Nachbarn auf den Stack pushen, damit Fill weitergeht!
                                stack.push((to, to + icy_engine::Position::new(-1, 0)));
                                stack.push((to, to + icy_engine::Position::new(1, 0)));
                                stack.push((to, to + icy_engine::Position::new(0, -1)));
                                stack.push((to, to + icy_engine::Position::new(0, 1)));
                            }
                        }
                    }

                    let _ = bg; // keep symmetry with other tools; currently unused for half-block fill
                    return ToolResult::Commit("Bucket fill".to_string());
                }

                let (offset, width, height) = if let Some(layer) = ctx.state.get_cur_layer() {
                    (layer.offset(), layer.width(), layer.height())
                } else {
                    return ToolResult::None;
                };
                let use_selection = ctx.state.is_something_selected();

                let start_cell_layer = pos - offset;
                if start_cell_layer.x < 0 || start_cell_layer.y < 0 || start_cell_layer.x >= width || start_cell_layer.y >= height {
                    return ToolResult::Commit("Bucket fill".to_string());
                }

                let base_char = { ctx.state.get_cur_layer().unwrap().char_at(start_cell_layer) };

                let caret_attr = ctx.state.get_caret().attribute;
                let (fg, bg) = if swap_colors || shift_swap {
                    (caret_attr.background(), caret_attr.foreground())
                } else {
                    (caret_attr.foreground(), caret_attr.background())
                };
                let caret_font_page = caret_attr.font_page();

                let mut visited: HashSet<icy_engine::Position> = HashSet::new();
                let mut stack: Vec<icy_engine::Position> = vec![start_cell_layer];

                while let Some(p) = stack.pop() {
                    if p.x < 0 || p.y < 0 || p.x >= width || p.y >= height || !visited.insert(p) {
                        continue;
                    }

                    if use_selection {
                        let doc_cell = p + offset;
                        if !ctx.state.is_selected(doc_cell) {
                            continue;
                        }
                    }

                    let cur = { ctx.state.get_cur_layer().unwrap().char_at(p) };

                    // Determine if this cell matches (like src_egui FillOperation).
                    match primary {
                        BrushPrimaryMode::Char => {
                            if (settings.exact && cur != base_char) || (!settings.exact && cur.ch != base_char.ch) {
                                continue;
                            }
                        }
                        BrushPrimaryMode::Colorize => {
                            if (settings.exact && cur != base_char) || (!settings.exact && cur.attribute != base_char.attribute) {
                                continue;
                            }
                        }
                        _ => {}
                    }

                    let mut repl = cur;

                    if matches!(primary, BrushPrimaryMode::Char) {
                        repl.ch = settings.paint_char;
                    }

                    if settings.colorize_fg {
                        repl.attribute.set_foreground(fg);
                        repl.attribute.set_is_bold(caret_attr.is_bold());
                    }
                    if settings.colorize_bg {
                        repl.attribute.set_background(bg);
                    }

                    repl.set_font_page(caret_font_page);
                    repl.attribute.attr &= !icy_engine::attribute::INVISIBLE;

                    let _ = ctx.state.set_char_in_atomic(p, repl);

                    stack.push(p + icy_engine::Position::new(-1, 0));
                    stack.push(p + icy_engine::Position::new(1, 0));
                    stack.push(p + icy_engine::Position::new(0, -1));
                    stack.push(p + icy_engine::Position::new(0, 1));
                }

                ToolResult::Commit("Bucket fill".to_string())
            }

            TerminalMessage::Move(evt) | TerminalMessage::Drag(evt) => {
                // Update hover position for status display
                if let Some(pos) = evt.text_position {
                    self.last_fill_pos = Some(pos);
                }
                ToolResult::None
            }

            _ => ToolResult::None,
        }
    }

    fn view_toolbar(&self, ctx: &ToolViewContext) -> Element<'_, ToolMessage> {
        let settings = *self.brush.read();
        let primary = match settings.primary {
            BrushPrimaryMode::HalfBlock | BrushPrimaryMode::Char | BrushPrimaryMode::Colorize => settings.primary,
            _ => BrushPrimaryMode::Char,
        };

        let segments = vec![
            Segment::text("Half Block", BrushPrimaryMode::HalfBlock),
            Segment::char(settings.paint_char, BrushPrimaryMode::Char),
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
        if settings.colorize_fg {
            selected_indices.push(0);
        }
        if settings.colorize_bg {
            selected_indices.push(1);
        }
        let color_filter = self
            .color_filter_control
            .view_multi_select(color_filter_segments, &selected_indices, font_for_color_filter, &ctx.theme)
            .map(move |msg| match msg {
                SegmentedControlMessage::Toggled(0) => ToolMessage::ToggleForeground(!settings.colorize_fg),
                SegmentedControlMessage::Toggled(1) => ToolMessage::ToggleBackground(!settings.colorize_bg),
                _ => ToolMessage::ToggleForeground(settings.colorize_fg),
            });

        let exact_toggle: Element<'_, ToolMessage> = toggler(settings.exact)
            .label("Exact")
            .on_toggle(ToolMessage::FillToggleExact)
            .text_size(11)
            .into();

        row![
            Space::new().width(Length::Fill),
            segmented_control,
            Space::new().width(Length::Fixed(16.0)),
            exact_toggle,
            Space::new().width(Length::Fixed(16.0)),
            color_filter,
            Space::new().width(Length::Fill),
        ]
        .spacing(4)
        .align_y(icy_ui::Alignment::Center)
        .into()
    }

    fn cursor(&self) -> icy_ui::mouse::Interaction {
        icy_ui::mouse::Interaction::Crosshair
    }

    fn show_caret(&self) -> bool {
        false
    }

    fn show_selection(&self) -> bool {
        true
    }
}
