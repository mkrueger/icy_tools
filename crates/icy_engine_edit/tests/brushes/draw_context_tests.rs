//! Tests for DrawContext and brush operations

mod test_target;

use icy_engine::{AttributedChar, Position, TextAttribute};
use icy_engine_edit::brushes::{BrushMode, DrawContext, DrawTarget, MirrorMode, PointRole, HALF_BLOCKS, SHADE_GRADIENT};
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

// =============================================================================
// ShadeDown tests - verifying Moebius-style behavior
// =============================================================================

/// Test that ShadeDown reduces shade characters step by step
#[test]
fn test_shade_down_reduces_shade_chars() {
    let mut target = TestTarget::new(80, 25);
    let ctx = DrawContext::default().with_brush_mode(BrushMode::ShadeDown);
    let pos = Position::new(10, 5);

    // Start with full block (219 / SHADE_GRADIENT[3])
    target.set_char(pos, AttributedChar::new(SHADE_GRADIENT[3], TextAttribute::default()));

    // First stroke: full block -> dark shade
    ctx.plot_point(&mut target, pos, PointRole::Fill);
    assert_eq!(target.get_at(10, 5).ch, SHADE_GRADIENT[2], "Full block should reduce to dark shade");

    // Second stroke: dark shade -> medium shade
    ctx.plot_point(&mut target, pos, PointRole::Fill);
    assert_eq!(target.get_at(10, 5).ch, SHADE_GRADIENT[1], "Dark shade should reduce to medium shade");

    // Third stroke: medium shade -> light shade
    ctx.plot_point(&mut target, pos, PointRole::Fill);
    assert_eq!(target.get_at(10, 5).ch, SHADE_GRADIENT[0], "Medium shade should reduce to light shade");

    // Fourth stroke: light shade -> empty space
    ctx.plot_point(&mut target, pos, PointRole::Fill);
    assert_eq!(target.get_at(10, 5).ch, ' ', "Light shade should reduce to empty space");
}

/// Test that ShadeDown does NOT affect normal characters (Moebius behavior)
/// This is the key difference from the current behavior:
/// In Moebius, right-click with shade tool only affects shade chars (176, 177, 178, 219),
/// leaving all other characters untouched.
#[test]
fn test_shade_down_ignores_non_shade_chars() {
    let mut target = TestTarget::new(80, 25);
    let ctx = DrawContext::default().with_brush_mode(BrushMode::ShadeDown);
    let pos = Position::new(10, 5);

    // Set a normal character (letter 'A')
    target.set_char(pos, AttributedChar::new('A', TextAttribute::default()));

    // Apply ShadeDown - should NOT change the character
    ctx.plot_point(&mut target, pos, PointRole::Fill);

    // Character should remain 'A' (Moebius behavior)
    assert_eq!(target.get_at(10, 5).ch, 'A', "ShadeDown should not affect non-shade characters");
}

/// Test that ShadeDown ignores various non-shade characters
#[test]
fn test_shade_down_ignores_various_chars() {
    let mut target = TestTarget::new(80, 25);
    let ctx = DrawContext::default().with_brush_mode(BrushMode::ShadeDown);

    // Test various non-shade characters
    let test_chars = ['A', 'Z', '0', '!', '#', '\u{00C4}', '\u{00DC}']; // Various CP437 chars

    for (i, ch) in test_chars.iter().enumerate() {
        let pos = Position::new(i as i32, 0);
        target.set_char(pos, AttributedChar::new(*ch, TextAttribute::default()));

        // Apply ShadeDown
        ctx.plot_point(&mut target, pos, PointRole::Fill);

        // Character should remain unchanged
        assert_eq!(target.get_at(i as i32, 0).ch, *ch, "ShadeDown should not affect character {:?}", ch);
    }
}

/// Test that ShadeDown correctly identifies shade characters
#[test]
fn test_shade_down_recognizes_all_shade_chars() {
    let mut target = TestTarget::new(80, 25);
    let ctx = DrawContext::default().with_brush_mode(BrushMode::ShadeDown);

    // Test each shade character reduces correctly
    for (i, shade_char) in SHADE_GRADIENT.iter().enumerate() {
        let pos = Position::new(i as i32, 0);
        target.set_char(pos, AttributedChar::new(*shade_char, TextAttribute::default()));

        // Apply ShadeDown
        ctx.plot_point(&mut target, pos, PointRole::Fill);

        // Should reduce by one level
        let expected = if i == 0 {
            ' ' // Light shade becomes empty
        } else {
            SHADE_GRADIENT[i - 1]
        };
        assert_eq!(
            target.get_at(i as i32, 0).ch,
            expected,
            "Shade char {:?} at level {} should reduce to {:?}",
            shade_char,
            i,
            expected
        );
    }
}

/// Test that empty space is not further affected by ShadeDown
#[test]
fn test_shade_down_on_empty_space() {
    let mut target = TestTarget::new(80, 25);
    let ctx = DrawContext::default().with_brush_mode(BrushMode::ShadeDown);
    let pos = Position::new(10, 5);

    // Target starts with empty space by default
    assert_eq!(target.get_at(10, 5).ch, ' ');

    // Apply ShadeDown multiple times
    ctx.plot_point(&mut target, pos, PointRole::Fill);
    ctx.plot_point(&mut target, pos, PointRole::Fill);
    ctx.plot_point(&mut target, pos, PointRole::Fill);

    // Should still be empty space
    assert_eq!(target.get_at(10, 5).ch, ' ', "Empty space should remain empty");
}
