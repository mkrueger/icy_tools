//! Tests for terminal centering logic
//!
//! These tests verify that documents are properly centered in the viewport
//! at various zoom levels, especially for small documents that don't fill
//! the entire viewport.

use icy_engine_gui::ScalingMode;

fn assert_near_integer(value: f32, label: &str) {
    let nearest = value.round();
    assert!(
        (value - nearest).abs() < 1e-6,
        "{} should be integer-like, got {} (nearest {})",
        label,
        value,
        nearest
    );
}

#[test]
fn centering_at_50_percent_zoom() {
    // Bug verification: At 50% zoom, the content should be centered in the window.
    // Scenario: Content is 800x600, viewport is 1000x800, zoom is 50%
    let content_w = 800.0;
    let content_h = 600.0;
    let viewport_w = 1000.0;
    let viewport_h = 800.0;
    let scaling_mode = ScalingMode::Manual(0.5);
    let use_integer_scaling = false;

    let rect = icy_engine_gui::compute_terminal_rect(content_w, content_h, viewport_w, viewport_h, &scaling_mode, use_integer_scaling);

    // At 50% zoom:
    // scaled_w = 800 * 0.5 = 400
    // scaled_h = 600 * 0.5 = 300
    // offset_x = (1000 - 400) / 2 = 300
    // offset_y = (800 - 300) / 2 = 250
    let expected_scaled_w = content_w * 0.5; // 400
    let expected_scaled_h = content_h * 0.5; // 300
    let expected_offset_x = (viewport_w - expected_scaled_w) / 2.0; // 300
    let expected_offset_y = (viewport_h - expected_scaled_h) / 2.0; // 250

    // Verify offsets for centering
    assert!(
        (rect.offset_x - expected_offset_x).abs() < 0.001,
        "offset_x should be {} but was {} (content not centered horizontally)",
        expected_offset_x,
        rect.offset_x
    );
    assert!(
        (rect.offset_y - expected_offset_y).abs() < 0.001,
        "offset_y should be {} but was {} (content not centered vertically)",
        expected_offset_y,
        rect.offset_y
    );

    // Verify normalized coordinates
    let expected_start_x = expected_offset_x / viewport_w; // 0.3
    let expected_start_y = expected_offset_y / viewport_h; // 0.3125
    let expected_width_n = expected_scaled_w / viewport_w; // 0.4
    let expected_height_n = expected_scaled_h / viewport_h; // 0.375

    assert!(
        (rect.start_x - expected_start_x).abs() < 0.001,
        "start_x should be {} but was {}",
        expected_start_x,
        rect.start_x
    );
    assert!(
        (rect.start_y - expected_start_y).abs() < 0.001,
        "start_y should be {} but was {}",
        expected_start_y,
        rect.start_y
    );
    assert!(
        (rect.width_n - expected_width_n).abs() < 0.001,
        "width_n should be {} but was {}",
        expected_width_n,
        rect.width_n
    );
    assert!(
        (rect.height_n - expected_height_n).abs() < 0.001,
        "height_n should be {} but was {}",
        expected_height_n,
        rect.height_n
    );

    // Verify that the centered rect covers the expected area:
    // start_x + width_n should give us the end, and 1 - (start_x + width_n) should equal start_x
    // for perfect horizontal centering
    let end_x = rect.start_x + rect.width_n;
    let margin_left = rect.start_x;
    let margin_right = 1.0 - end_x;
    assert!(
        (margin_left - margin_right).abs() < 0.001,
        "Horizontal margins should be equal for centering: left={}, right={}",
        margin_left,
        margin_right
    );

    let end_y = rect.start_y + rect.height_n;
    let margin_top = rect.start_y;
    let margin_bottom = 1.0 - end_y;
    assert!(
        (margin_top - margin_bottom).abs() < 0.001,
        "Vertical margins should be equal for centering: top={}, bottom={}",
        margin_top,
        margin_bottom
    );
}

/// This test shows the behavior when the document is larger than the viewport at current zoom.
/// In this case, no centering should happen - the content fills the viewport completely.
#[test]
fn no_centering_for_large_document_at_50_percent_zoom() {
    let viewport_w = 1000.0_f32;
    let viewport_h = 800.0_f32;
    let zoom = 0.5_f32;
    let document_w = 4000.0_f32;
    let document_h = 3000.0_f32;

    // Simulate how visible_width is computed in crt_shader_program.rs
    let visible_w = (viewport_w / zoom).min(document_w); // = 2000
    let visible_h = (viewport_h / zoom).min(document_h); // = 1600

    let scaling_mode = ScalingMode::Manual(zoom);
    let use_integer_scaling = false;

    let rect = icy_engine_gui::compute_terminal_rect(visible_w, visible_h, viewport_w, viewport_h, &scaling_mode, use_integer_scaling);

    // When the document is larger than what fits in the viewport at current zoom,
    // there's no need to center - the content fills the viewport completely.
    assert_eq!(rect.offset_x, 0.0, "Expected no centering for large document at 50% zoom");
    assert_eq!(rect.offset_y, 0.0, "Expected no centering for large document at 50% zoom");
}

/// Test the actual bug scenario: small document, large viewport, 50% zoom
/// Document (800x600) should be centered when displayed at 50% (400x300) in a 1000x800 viewport
#[test]
fn centering_small_document_at_50_percent_zoom() {
    let viewport_w = 1000.0_f32;
    let viewport_h = 800.0_f32;
    let zoom = 0.5_f32;
    let document_w = 800.0_f32; // Small document
    let document_h = 600.0_f32;

    // Simulate how visible_width is computed in crt_shader_program.rs
    // visible_width = (viewport_w / zoom).min(document_w)
    // = (1000 / 0.5).min(800) = 2000.min(800) = 800
    let visible_w = (viewport_w / zoom).min(document_w);
    let visible_h = (viewport_h / zoom).min(document_h);

    // visible_w = 800, visible_h = 600 (clamped to document size)
    assert_eq!(visible_w, 800.0);
    assert_eq!(visible_h, 600.0);

    let scaling_mode = ScalingMode::Manual(zoom);
    let use_integer_scaling = false;

    let rect = icy_engine_gui::compute_terminal_rect(visible_w, visible_h, viewport_w, viewport_h, &scaling_mode, use_integer_scaling);

    // At 50% zoom with document 800x600:
    // term_w = 800, term_h = 600
    // scaled_w = 800 * 0.5 = 400
    // scaled_h = 600 * 0.5 = 300
    // offset_x = (1000 - 400) / 2 = 300
    // offset_y = (800 - 300) / 2 = 250

    let expected_offset_x = 300.0;
    let expected_offset_y = 250.0;

    assert!(
        (rect.offset_x - expected_offset_x).abs() < 0.001,
        "Small document should be centered: offset_x expected {} but got {}",
        expected_offset_x,
        rect.offset_x
    );
    assert!(
        (rect.offset_y - expected_offset_y).abs() < 0.001,
        "Small document should be centered: offset_y expected {} but got {}",
        expected_offset_y,
        rect.offset_y
    );

    // Verify centering in normalized coordinates
    let expected_start_x = 0.3; // 300/1000
    let expected_start_y = 0.3125; // 250/800

    assert!(
        (rect.start_x - expected_start_x).abs() < 0.001,
        "start_x should be {} but was {}",
        expected_start_x,
        rect.start_x
    );
    assert!(
        (rect.start_y - expected_start_y).abs() < 0.001,
        "start_y should be {} but was {}",
        expected_start_y,
        rect.start_y
    );
}

#[test]
fn terminal_rect_offsets_are_floored_to_pixels() {
    // Construct a scenario where perfect centering would yield a half-pixel offset.
    // Example: avail=101px, content=100px at 100% -> (101-100)/2 = 0.5.
    // We want this floored to 0px to keep the terminal window pixel-aligned.
    let visible_w = 100.0_f32;
    let visible_h = 100.0_f32;
    let viewport_w = 101.0_f32;
    let viewport_h = 101.0_f32;

    let scaling_mode = ScalingMode::Manual(1.0);
    let rect = icy_engine_gui::compute_terminal_rect(visible_w, visible_h, viewport_w, viewport_h, &scaling_mode, false);

    assert_near_integer(rect.offset_x, "offset_x");
    assert_near_integer(rect.offset_y, "offset_y");

    // Also verify start_x/start_y map back to the same integer pixel offsets.
    assert_near_integer(rect.start_x * viewport_w, "start_x*viewport_w");
    assert_near_integer(rect.start_y * viewport_h, "start_y*viewport_h");

    assert!(rect.offset_x >= 0.0);
    assert!(rect.offset_y >= 0.0);
    assert!(rect.width_n > 0.0 && rect.width_n <= 1.0);
    assert!(rect.height_n > 0.0 && rect.height_n <= 1.0);
}

#[test]
fn centering_when_content_smaller_than_viewport_at_100_percent() {
    // When content is smaller than viewport at 100% zoom, it should be centered
    let content_w = 400.0;
    let content_h = 300.0;
    let viewport_w = 1000.0;
    let viewport_h = 800.0;
    let scaling_mode = ScalingMode::Manual(1.0);
    let use_integer_scaling = false;

    let rect = icy_engine_gui::compute_terminal_rect(content_w, content_h, viewport_w, viewport_h, &scaling_mode, use_integer_scaling);

    // At 100% zoom with content smaller than viewport:
    // scaled_w = 400 * 1.0 = 400
    // scaled_h = 300 * 1.0 = 300
    // offset_x = (1000 - 400) / 2 = 300
    // offset_y = (800 - 300) / 2 = 250
    let expected_offset_x = 300.0;
    let expected_offset_y = 250.0;

    assert!(
        (rect.offset_x - expected_offset_x).abs() < 0.001,
        "offset_x should be {} but was {}",
        expected_offset_x,
        rect.offset_x
    );
    assert!(
        (rect.offset_y - expected_offset_y).abs() < 0.001,
        "offset_y should be {} but was {}",
        expected_offset_y,
        rect.offset_y
    );
}

#[test]
fn no_centering_when_content_fills_viewport() {
    // When scaled content exactly fills viewport, no centering offset needed
    let content_w = 1000.0;
    let content_h = 800.0;
    let viewport_w = 1000.0;
    let viewport_h = 800.0;
    let scaling_mode = ScalingMode::Manual(1.0);
    let use_integer_scaling = false;

    let rect = icy_engine_gui::compute_terminal_rect(content_w, content_h, viewport_w, viewport_h, &scaling_mode, use_integer_scaling);

    assert!(
        rect.offset_x.abs() < 0.001,
        "offset_x should be 0 when content fills viewport, but was {}",
        rect.offset_x
    );
    assert!(
        rect.offset_y.abs() < 0.001,
        "offset_y should be 0 when content fills viewport, but was {}",
        rect.offset_y
    );
    assert!((rect.start_x).abs() < 0.001, "start_x should be 0, but was {}", rect.start_x);
    assert!((rect.start_y).abs() < 0.001, "start_y should be 0, but was {}", rect.start_y);
    assert!((rect.width_n - 1.0).abs() < 0.001, "width_n should be 1.0, but was {}", rect.width_n);
    assert!((rect.height_n - 1.0).abs() < 0.001, "height_n should be 1.0, but was {}", rect.height_n);
}

/// Reproduce the exact bug: 80x25 document at 100% zoom in a large window
/// The content should be centered, but without the fix it "sticks to the top"
#[test]
fn centering_80x25_document_at_100_percent_zoom() {
    // 80x25 document with 8x16 font = 640x400 pixels
    let doc_res_w = 80.0 * 8.0; // 640
    let doc_res_h = 25.0 * 16.0; // 400

    // Large viewport (typical window size)
    let viewport_w = 1200.0;
    let viewport_h = 700.0;

    let zoom = 1.0;
    let scaling_mode = ScalingMode::Manual(zoom);
    let use_integer_scaling = false;

    // Simulate how crt_shader_program.rs computes visible_width/height for Manual zoom:
    // visible_width = (bounds.width / zoom).min(res_w)
    // At 100% zoom: visible_width = (1200 / 1.0).min(640) = 640
    let visible_w = (viewport_w / zoom).min(doc_res_w);
    let visible_h = (viewport_h / zoom).min(doc_res_h);

    assert_eq!(visible_w, 640.0, "visible_width should be clamped to document resolution");
    assert_eq!(visible_h, 400.0, "visible_height should be clamped to document resolution");

    // Now run the centering logic
    let rect = icy_engine_gui::compute_terminal_rect(visible_w, visible_h, viewport_w, viewport_h, &scaling_mode, use_integer_scaling);

    // At 100% zoom with 640x400 visible content in 1200x700 viewport:
    // term_w = 640, term_h = 400
    // scaled_w = 640 * 1.0 = 640
    // scaled_h = 400 * 1.0 = 400
    // offset_x = (1200 - 640) / 2 = 280
    // offset_y = (700 - 400) / 2 = 150

    let expected_offset_x = (viewport_w - visible_w * zoom) / 2.0; // (1200-640)/2 = 280
    let expected_offset_y = (viewport_h - visible_h * zoom) / 2.0; // (700-400)/2 = 150

    assert!(
        (rect.offset_x - expected_offset_x).abs() < 0.001,
        "80x25 at 100%: offset_x should be {} but was {} (content not centered horizontally)",
        expected_offset_x,
        rect.offset_x
    );
    assert!(
        (rect.offset_y - expected_offset_y).abs() < 0.001,
        "80x25 at 100%: offset_y should be {} but was {} (content should be centered, not stuck at top!)",
        expected_offset_y,
        rect.offset_y
    );

    // Verify normalized coordinates
    let expected_start_x = expected_offset_x / viewport_w; // 280/1200 ≈ 0.233
    let expected_start_y = expected_offset_y / viewport_h; // 150/700 ≈ 0.214

    assert!(
        (rect.start_x - expected_start_x).abs() < 0.001,
        "start_x should be {} but was {}",
        expected_start_x,
        rect.start_x
    );
    assert!(
        (rect.start_y - expected_start_y).abs() < 0.001,
        "start_y should be {} but was {} (terminal_rect.y not set correctly for centering)",
        expected_start_y,
        rect.start_y
    );

    // Verify centering symmetry
    let margin_top = rect.start_y;
    let margin_bottom = 1.0 - (rect.start_y + rect.height_n);
    assert!(
        (margin_top - margin_bottom).abs() < 0.001,
        "Vertical margins should be equal: top={}, bottom={} (content stuck at top means top margin is wrong)",
        margin_top,
        margin_bottom
    );
}

/// Test that demonstrates the bug when fit_terminal_height_to_bounds inflates visible_height
/// to fill the entire viewport, eliminating vertical centering.
#[test]
fn bug_fit_terminal_height_inflates_visible_height() {
    // The bug: fit_terminal_height_to_bounds sets terminal height to fill viewport
    // For a 80x25 document in a 1527x855 viewport at 100% zoom:
    //   - Document resolution: 640x400 (80*8, 25*16)
    //   - Viewport: 1527x855
    //   - fit_terminal_height_to_bounds sets terminal rows to 855/16 ≈ 53 rows
    //   - New resolution becomes 640x848 (80*8, 53*16)
    //   - visible_height = min(855/1.0, 848) = 848 (almost fills viewport!)
    //   - offset_y = (855 - 848) / 2 ≈ 3.5 (almost no centering!)

    // This simulates what happens with the bug:
    let viewport_w = 1527.0_f32;
    let viewport_h = 855.0_f32;
    let font_h = 16.0_f32;

    // BUG: fit_terminal_height_to_bounds inflates terminal height
    let inflated_rows = (viewport_h / font_h).floor(); // 53 rows
    let inflated_res_h = inflated_rows * font_h; // 848 pixels

    // Document is 80x25, but resolution is now 80x53 due to the bug
    let doc_res_w = 640.0_f32;
    let buggy_res_h = inflated_res_h; // 848 instead of 400

    let zoom = 1.0_f32;

    // visible_height gets the inflated resolution
    let visible_w = (viewport_w / zoom).min(doc_res_w); // 640
    let visible_h = (viewport_h / zoom).min(buggy_res_h); // 848

    let scaling_mode = ScalingMode::Manual(zoom);
    let buggy_rect = icy_engine_gui::compute_terminal_rect(visible_w, visible_h, viewport_w, viewport_h, &scaling_mode, false);

    // With the bug, offset_y is tiny (almost no centering)
    assert!(
        buggy_rect.offset_y < 10.0,
        "Bug demonstration: with inflated height, offset_y={} should be very small (no centering)",
        buggy_rect.offset_y
    );

    // Correct behavior: visible_height should be clamped to actual document height (400)
    let correct_doc_res_h = 25.0 * 16.0; // 400
    let correct_visible_h = (viewport_h / zoom).min(correct_doc_res_h); // 400

    let correct_rect = icy_engine_gui::compute_terminal_rect(visible_w, correct_visible_h, viewport_w, viewport_h, &scaling_mode, false);

    // With correct behavior, content should be nicely centered
    let expected_offset_y = ((viewport_h - correct_doc_res_h) / 2.0).floor(); // (855-400)/2 = 227.5 -> 227 (pixel snapping)
    assert!(
        (correct_rect.offset_y - expected_offset_y).abs() < 0.001,
        "Correct behavior: offset_y should be {} for centering, got {}",
        expected_offset_y,
        correct_rect.offset_y
    );

    // Verify that the correct start_y gives proper centering
    let expected_start_y = expected_offset_y / viewport_h; // ≈ 0.266
    assert!(
        (correct_rect.start_y - expected_start_y).abs() < 0.001,
        "Correct start_y should be {} but was {}",
        expected_start_y,
        correct_rect.start_y
    );

    // Suppress unused variable warning
    let _ = buggy_rect.start_y;
}
