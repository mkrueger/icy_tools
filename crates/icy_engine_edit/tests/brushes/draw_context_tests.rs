//! Tests for DrawContext and brush operations

mod test_target;

use icy_engine::{AttributedChar, Position, TextAttribute};
use icy_engine_edit::brushes::{BrushMode, DrawContext, DrawTarget, HALF_BLOCKS, MirrorMode, PointRole, SHADE_GRADIENT};
use test_target::TestTarget;

#[test]
fn test_draw_block() {
    let mut target = TestTarget::new(80, 25);
    let ctx = DrawContext::default().with_brush_mode(BrushMode::Block);

    ctx.plot_point(&mut target, Position::new(10, 5), PointRole::Fill);

    let ch = target.get_at(10, 5);
    assert_eq!(ch.ch, HALF_BLOCKS.full);
}

#[test]
fn test_draw_char() {
    let mut target = TestTarget::new(80, 25);
    let ctx = DrawContext::default().with_brush_mode(BrushMode::Char('X'));

    ctx.plot_point(&mut target, Position::new(10, 5), PointRole::Fill);

    let ch = target.get_at(10, 5);
    assert_eq!(ch.ch, 'X');
}

#[test]
fn test_shade_gradient() {
    let mut target = TestTarget::new(80, 25);
    let ctx = DrawContext::default().with_brush_mode(BrushMode::Shade);
    let pos = Position::new(10, 5);

    // First stroke
    ctx.plot_point(&mut target, pos, PointRole::Fill);
    assert_eq!(target.get_at(10, 5).ch, SHADE_GRADIENT[0]);

    // Second stroke
    ctx.plot_point(&mut target, pos, PointRole::Fill);
    assert_eq!(target.get_at(10, 5).ch, SHADE_GRADIENT[1]);

    // Third stroke
    ctx.plot_point(&mut target, pos, PointRole::Fill);
    assert_eq!(target.get_at(10, 5).ch, SHADE_GRADIENT[2]);

    // Fourth stroke - max
    ctx.plot_point(&mut target, pos, PointRole::Fill);
    assert_eq!(target.get_at(10, 5).ch, SHADE_GRADIENT[3]);
}

#[test]
fn test_colorize() {
    let mut target = TestTarget::new(80, 25);
    let pos = Position::new(10, 5);

    // First, draw a character
    target.set_char(pos, AttributedChar::new('A', TextAttribute::default()));

    // Now colorize it
    let ctx = DrawContext::default()
        .with_brush_mode(BrushMode::Colorize)
        .with_foreground(4)
        .with_background(1);

    ctx.plot_point(&mut target, pos, PointRole::Fill);

    let ch = target.get_at(10, 5);
    assert_eq!(ch.ch, 'A'); // Character unchanged
    assert_eq!(ch.attribute.foreground(), 4);
    assert_eq!(ch.attribute.background(), 1);
}

#[test]
fn test_bounds_check() {
    let mut target = TestTarget::new(80, 25);
    let ctx = DrawContext::default().with_brush_mode(BrushMode::Block);

    // Should not panic on out-of-bounds
    ctx.plot_point(&mut target, Position::new(-1, 5), PointRole::Fill);
    ctx.plot_point(&mut target, Position::new(100, 5), PointRole::Fill);
    ctx.plot_point(&mut target, Position::new(10, -1), PointRole::Fill);
    ctx.plot_point(&mut target, Position::new(10, 100), PointRole::Fill);
}

#[test]
fn test_mirror_horizontal() {
    let mut target = TestTarget::new(80, 25);
    let ctx = DrawContext {
        brush_mode: BrushMode::Block,
        mirror_mode: MirrorMode::Horizontal,
        ..Default::default()
    };

    // Draw at x=10, should also draw at x=69 (mirror around center 40)
    ctx.plot_point(&mut target, Position::new(10, 5), PointRole::Fill);

    assert_eq!(target.get_at(10, 5).ch, HALF_BLOCKS.full);
    assert_eq!(target.get_at(69, 5).ch, HALF_BLOCKS.full);
}
