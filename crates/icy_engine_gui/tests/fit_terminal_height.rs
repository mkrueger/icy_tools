//! Tests for `clamp_terminal_height_to_viewport` function
//!
//! This function clamps the terminal height to: min(viewport_rows, document_height)
//! - Small documents stay small (enabling centering)
//! - Large documents shrink to viewport (using full screen)

use icy_engine::{Screen, Size, TextScreen};
use icy_engine_gui::clamp_terminal_height_to_viewport;

/// Helper to create a test screen with specific dimensions
fn create_test_screen(width: i32, height: i32) -> TextScreen {
    TextScreen::new(Size::new(width, height))
}

/// Test: Small document in large viewport stays at document size.
/// A 25-row document in a viewport that can show 50 rows should stay at 25.
#[test]
fn small_document_stays_small() {
    let mut screen = create_test_screen(80, 25);

    // Viewport can show 50 rows (50 * 16px = 800px)
    let bounds_height = 800.0;
    let scan_lines = false;
    let scale_factor = 1.0;
    let zoom = 1.0;

    let initial_height = screen.terminal_state().height();
    assert_eq!(initial_height, 25);

    let changed = clamp_terminal_height_to_viewport(&mut screen, bounds_height, scan_lines, scale_factor, zoom);

    // min(50, 25) = 25 -> no change
    assert!(!changed, "Small document should not change");
    assert_eq!(screen.terminal_state().height(), 25);
}

/// Test: Large document shrinks to viewport size.
/// A 100-row document in a viewport that can only show 30 rows should shrink to 30.
#[test]
fn large_document_shrinks_to_viewport() {
    let mut screen = create_test_screen(80, 100);

    // Viewport can only show 30 rows (30 * 16 = 480px)
    let bounds_height = 480.0;
    let scan_lines = false;
    let scale_factor = 1.0;
    let zoom = 1.0;

    let initial_height = screen.terminal_state().height();
    assert_eq!(initial_height, 100);

    let changed = clamp_terminal_height_to_viewport(&mut screen, bounds_height, scan_lines, scale_factor, zoom);

    // min(30, 100) = 30 -> shrinks to viewport
    assert!(changed, "Large document should shrink");
    assert_eq!(screen.terminal_state().height(), 30);
}

/// Test: When viewport matches document size, no change needed.
#[test]
fn no_change_when_viewport_matches_document() {
    let mut screen = create_test_screen(80, 25);

    // Viewport shows exactly 25 rows (25 * 16 = 400px)
    let bounds_height = 400.0;
    let scan_lines = false;
    let scale_factor = 1.0;
    let zoom = 1.0;

    let changed = clamp_terminal_height_to_viewport(&mut screen, bounds_height, scan_lines, scale_factor, zoom);

    assert!(!changed, "No change when viewport matches document");
    assert_eq!(screen.terminal_state().height(), 25);
}

/// Test: Scanlines reduce the effective viewport capacity.
#[test]
fn scanlines_reduce_viewport_capacity() {
    let mut screen = create_test_screen(80, 25);

    // Viewport height of 640px
    // Without scanlines: 640 / 16 = 40 rows
    // With scanlines: 640 / 32 = 20 rows
    let bounds_height = 640.0;
    let scale_factor = 1.0;
    let zoom = 1.0;

    // With scanlines: viewport can only show 20 rows, document has 25
    // min(20, 25) = 20 -> shrinks
    let changed = clamp_terminal_height_to_viewport(&mut screen, bounds_height, true, scale_factor, zoom);

    assert!(changed, "Should shrink to 20 rows with scanlines");
    assert_eq!(screen.terminal_state().height(), 20);
}

/// Test: Scale factor affects viewport capacity calculation.
#[test]
fn scale_factor_affects_calculation() {
    // With scale factor 2.0, more physical pixels available
    let scale_factor = 2.0;

    let mut screen = create_test_screen(80, 25);

    // Viewport 400 logical pixels * 2.0 scale = 800 physical pixels
    // 800 / 16 = 50 rows viewport capacity
    // min(50, 25) = 25 -> no change (small doc stays small)
    let bounds_height = 400.0;
    let scan_lines = false;
    let zoom = 1.0;

    let changed = clamp_terminal_height_to_viewport(&mut screen, bounds_height, scan_lines, scale_factor, zoom);

    assert!(!changed, "Small doc should stay at 25");
    assert_eq!(screen.terminal_state().height(), 25);
}

/// Test: Calling multiple times is idempotent.
#[test]
fn idempotent_when_called_multiple_times() {
    // Large document that will shrink
    let mut screen = create_test_screen(80, 100);
    let bounds_height = 480.0; // 30 rows
    let scan_lines = false;
    let scale_factor = 1.0;
    let zoom = 1.0;

    // First call - should shrink to 30
    let changed1 = clamp_terminal_height_to_viewport(&mut screen, bounds_height, scan_lines, scale_factor, zoom);
    assert!(changed1);
    assert_eq!(screen.terminal_state().height(), 30);

    // Second call - no change
    let changed2 = clamp_terminal_height_to_viewport(&mut screen, bounds_height, scan_lines, scale_factor, zoom);
    assert!(!changed2);
    assert_eq!(screen.terminal_state().height(), 30);
}

/// Test: Very small viewport clamps to minimum of 1 row.
#[test]
fn tiny_viewport_clamps_to_one_row() {
    let mut screen = create_test_screen(80, 25);

    // Tiny viewport - only 10 pixels (less than one row)
    let bounds_height = 10.0;
    let scan_lines = false;
    let scale_factor = 1.0;
    let zoom = 1.0;

    let changed = clamp_terminal_height_to_viewport(&mut screen, bounds_height, scan_lines, scale_factor, zoom);

    // viewport_rows = floor(10 / 16) = 0 -> clamped to 1
    // min(1, 25) = 1 -> shrinks to 1
    assert!(changed, "Should shrink to 1 row");
    assert_eq!(screen.terminal_state().height(), 1);
}

/// Test: Zero bounds_height handled gracefully.
#[test]
fn zero_bounds_handled_gracefully() {
    let mut screen = create_test_screen(80, 25);

    // Zero bounds - clamps to 1.0 internally, then 1 row
    let bounds_height = 0.0;
    let scan_lines = false;
    let scale_factor = 1.0;
    let zoom = 1.0;

    let changed = clamp_terminal_height_to_viewport(&mut screen, bounds_height, scan_lines, scale_factor, zoom);

    // min(1, 25) = 1
    assert!(changed);
    assert_eq!(screen.terminal_state().height(), 1);
}

/// Real-world scenario: 80x25 document in typical viewport.
/// This is the key bug test - a small document should NOT expand to fill viewport.
#[test]
fn realistic_80x25_in_large_viewport() {
    // Standard 80x25 terminal document
    let mut screen = create_test_screen(80, 25);

    // Large viewport (typical window size)
    // 855px / 16px = 53 rows viewport capacity
    let bounds_height = 855.0;
    let scan_lines = false;
    let scale_factor = 1.0;
    let zoom = 1.0;

    let initial_height = screen.terminal_state().height();
    assert_eq!(initial_height, 25);

    let changed = clamp_terminal_height_to_viewport(&mut screen, bounds_height, scan_lines, scale_factor, zoom);

    // min(53, 25) = 25 -> NO CHANGE! Document stays at 25.
    // This enables proper centering.
    assert!(!changed, "Small document must NOT expand");
    assert_eq!(screen.terminal_state().height(), 25);
}

/// Test: Zoom factor affects viewport capacity.
/// At 50% zoom, each row takes half the pixels, so double the rows fit.
#[test]
fn zoom_50_percent_doubles_viewport_capacity() {
    let mut screen = create_test_screen(80, 100);

    // At 100% zoom: 480px / 16px = 30 rows
    // At 50% zoom: 480px / 16px / 0.5 = 60 rows
    let bounds_height = 480.0;
    let scan_lines = false;
    let scale_factor = 1.0;
    let zoom = 0.5;

    let initial_height = screen.terminal_state().height();
    assert_eq!(initial_height, 100);

    let changed = clamp_terminal_height_to_viewport(&mut screen, bounds_height, scan_lines, scale_factor, zoom);

    // At 50% zoom: viewport can show 60 rows
    // min(60, 100) = 60 -> shrinks to 60
    assert!(changed, "Document should shrink to 60 at 50% zoom");
    assert_eq!(screen.terminal_state().height(), 60);
}

/// Test: At 200% zoom, rows take double the pixels, half the rows fit.
#[test]
fn zoom_200_percent_halves_viewport_capacity() {
    let mut screen = create_test_screen(80, 100);

    // At 100% zoom: 480px / 16px = 30 rows
    // At 200% zoom: 480px / 16px / 2.0 = 15 rows
    let bounds_height = 480.0;
    let scan_lines = false;
    let scale_factor = 1.0;
    let zoom = 2.0;

    let initial_height = screen.terminal_state().height();
    assert_eq!(initial_height, 100);

    let changed = clamp_terminal_height_to_viewport(&mut screen, bounds_height, scan_lines, scale_factor, zoom);

    // At 200% zoom: viewport can show 15 rows
    // min(15, 100) = 15 -> shrinks to 15
    assert!(changed, "Document should shrink to 15 at 200% zoom");
    assert_eq!(screen.terminal_state().height(), 15);
}

/// Test: Small document stays small regardless of zoom.
#[test]
fn small_document_stays_small_at_any_zoom() {
    let mut screen = create_test_screen(80, 25);

    // Large viewport at 50% zoom
    // At 50% zoom: 800px / 16px / 0.5 = 100 rows capacity
    let bounds_height = 800.0;
    let scan_lines = false;
    let scale_factor = 1.0;
    let zoom = 0.5;

    let initial_height = screen.terminal_state().height();
    assert_eq!(initial_height, 25);

    let changed = clamp_terminal_height_to_viewport(&mut screen, bounds_height, scan_lines, scale_factor, zoom);

    // min(100, 25) = 25 -> no change (small doc stays small)
    assert!(!changed, "Small document should not change at 50% zoom");
    assert_eq!(screen.terminal_state().height(), 25);
}
