//! Shared paint helpers for tool handlers.

use icy_engine::{MouseButton, Position, TextPane};
use icy_engine_edit::brushes::{BrushMode as EngineBrushMode, ColorMode as EngineColorMode, DrawContext, PointRole};
use icy_engine_edit::{AtomicUndoGuard, AttributedChar, EditState};

use crate::ui::editor::ansi::widget::toolbar::top::BrushPrimaryMode;

#[derive(Clone, Copy, Debug)]
pub struct BrushSettings {
    pub primary: BrushPrimaryMode,
    pub paint_char: char,
    pub brush_size: usize,
    pub colorize_fg: bool,
    pub colorize_bg: bool,
}

impl Default for BrushSettings {
    fn default() -> Self {
        Self {
            primary: BrushPrimaryMode::Char,
            paint_char: ' ',
            brush_size: 1,
            colorize_fg: true,
            colorize_bg: true,
        }
    }
}

pub fn begin_paint_undo(state: &mut EditState, desc: String) -> AtomicUndoGuard {
    state.begin_atomic_undo(desc)
}

pub fn apply_stamp_at_doc_pos(state: &mut EditState, settings: BrushSettings, doc_pos: Position, half_block_is_top: bool, button: MouseButton) {
    let (offset, layer_w, layer_h) = if let Some(layer) = state.get_cur_layer() {
        (layer.offset(), layer.width(), layer.height())
    } else {
        return;
    };

    let use_selection = state.is_something_selected();

    let swap_colors = button == MouseButton::Right;

    let caret_attr = state.get_caret().attribute;

    // Shapes use Shift as erase/clear, so we don't use Shift-swap here.
    let shift_swap = false;

    let swap_for_colors = (swap_colors || shift_swap) && !matches!(settings.primary, BrushPrimaryMode::Shading | BrushPrimaryMode::Char);
    let (fg, bg) = if swap_for_colors {
        (caret_attr.background(), caret_attr.foreground())
    } else {
        (caret_attr.foreground(), caret_attr.background())
    };

    let effective_brush_size = if matches!(settings.primary, BrushPrimaryMode::HalfBlock) {
        1
    } else {
        settings.brush_size.max(1)
    };
    let brush_size_i = effective_brush_size as i32;

    let center = doc_pos - offset;
    let half = brush_size_i / 2;

    for dy in 0..brush_size_i {
        for dx in 0..brush_size_i {
            let layer_pos = Position::new(center.x + dx - half, center.y + dy - half);

            if layer_pos.x < 0 || layer_pos.y < 0 || layer_pos.x >= layer_w || layer_pos.y >= layer_h {
                continue;
            }

            if use_selection {
                let doc_cell = layer_pos + offset;
                if !state.is_selected(doc_cell) {
                    continue;
                }
            }

            let brush_mode = match settings.primary {
                BrushPrimaryMode::Char => {
                    if swap_colors {
                        EngineBrushMode::Char(' ')
                    } else {
                        EngineBrushMode::Char(settings.paint_char)
                    }
                }
                BrushPrimaryMode::HalfBlock => EngineBrushMode::HalfBlock,
                BrushPrimaryMode::Shading => {
                    if swap_colors {
                        EngineBrushMode::ShadeDown
                    } else {
                        EngineBrushMode::Shade
                    }
                }
                BrushPrimaryMode::Replace => EngineBrushMode::Replace(settings.paint_char),
                BrushPrimaryMode::Blink => EngineBrushMode::Blink(!swap_colors),
                BrushPrimaryMode::Colorize => EngineBrushMode::Colorize,
            };

            let color_mode = if matches!(settings.primary, BrushPrimaryMode::Colorize) {
                match (settings.colorize_fg, settings.colorize_bg) {
                    (true, true) => EngineColorMode::Both,
                    (true, false) => EngineColorMode::Foreground,
                    (false, true) => EngineColorMode::Background,
                    (false, false) => EngineColorMode::None,
                }
            } else {
                EngineColorMode::Both
            };

            let mut template = caret_attr;
            template.set_foreground(fg);
            template.set_background(bg);

            struct LayerTarget<'a> {
                state: &'a mut EditState,
                width: i32,
                height: i32,
            }
            impl<'a> icy_engine_edit::brushes::DrawTarget for LayerTarget<'a> {
                fn width(&self) -> i32 {
                    self.width
                }
                fn height(&self) -> i32 {
                    self.height
                }
                fn char_at(&self, pos: icy_engine_edit::Position) -> Option<icy_engine_edit::AttributedChar> {
                    self.state.get_cur_layer().map(|l| l.char_at(pos))
                }
                fn set_char(&mut self, pos: icy_engine_edit::Position, ch: icy_engine_edit::AttributedChar) {
                    let _ = self.state.set_char_in_atomic(pos, ch);
                }
            }

            let ctx = DrawContext::default()
                .with_brush_mode(brush_mode)
                .with_color_mode(color_mode)
                .with_foreground(fg)
                .with_background(bg)
                .with_template_attribute(template)
                .with_half_block_is_top(half_block_is_top);

            let mut target = LayerTarget {
                state,
                width: layer_w,
                height: layer_h,
            };

            ctx.plot_point(&mut target, layer_pos, PointRole::Fill);
        }
    }
}

pub fn clear_at_doc_pos(state: &mut EditState, doc_pos: Position) {
    let (offset, layer_w, layer_h) = if let Some(layer) = state.get_cur_layer() {
        (layer.offset(), layer.width(), layer.height())
    } else {
        return;
    };

    let use_selection = state.is_something_selected();

    let layer_pos = doc_pos - offset;
    if layer_pos.x < 0 || layer_pos.y < 0 || layer_pos.x >= layer_w || layer_pos.y >= layer_h {
        return;
    }

    if use_selection && !state.is_selected(doc_pos) {
        return;
    }

    let _ = state.set_char_in_atomic(layer_pos, AttributedChar::invisible());
}
