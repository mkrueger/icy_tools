use icy_engine::{Position, Size};
use icy_engine_gui::Viewport;

fn assert_approx(a: f32, b: f32, eps: f32) {
    assert!((a - b).abs() <= eps, "{a} != {b} (eps={eps})");
}

#[test]
fn visible_content_size_is_visible_div_zoom() {
    let mut vp = Viewport::new(Size::new(100, 80), Size::new(500, 400));
    vp.zoom = 2.0;
    assert_approx(vp.visible_content_width(), 50.0, 1e-6);
    assert_approx(vp.visible_content_height(), 40.0, 1e-6);
}

#[test]
fn max_scroll_is_clamped_to_non_negative() {
    // Content smaller than viewport => no scrolling.
    let vp = Viewport::new(Size::new(200, 200), Size::new(100, 100));
    assert_approx(vp.max_scroll_x(), 0.0, 1e-6);
    assert_approx(vp.max_scroll_y(), 0.0, 1e-6);
    assert!(!vp.is_scrollable_x());
    assert!(!vp.is_scrollable_y());
}

#[test]
fn clamp_scroll_limits_scroll_and_targets() {
    let mut vp = Viewport::new(Size::new(100, 100), Size::new(500, 500));
    vp.zoom = 1.0;

    vp.scroll_x = -10.0;
    vp.scroll_y = 9999.0;
    vp.target_scroll_x = 9999.0;
    vp.target_scroll_y = -10.0;
    vp.clamp_scroll();

    assert_approx(vp.scroll_x, 0.0, 1e-6);
    assert_approx(vp.scroll_y, vp.max_scroll_y(), 1e-6);
    assert_approx(vp.target_scroll_x, vp.max_scroll_x(), 1e-6);
    assert_approx(vp.target_scroll_y, 0.0, 1e-6);
}

#[test]
fn set_zoom_keeps_content_under_center_stable() {
    let mut vp = Viewport::new(Size::new(100, 100), Size::new(500, 500));
    vp.zoom = 1.0;
    vp.scroll_x = 50.0;
    vp.scroll_y = 20.0;
    vp.target_scroll_x = vp.scroll_x;
    vp.target_scroll_y = vp.scroll_y;

    let center_x = 25.0;
    let center_y = 40.0;

    let content_before_x = center_x / vp.zoom + vp.scroll_x;
    let content_before_y = center_y / vp.zoom + vp.scroll_y;

    vp.set_zoom(2.0, center_x, center_y);

    let content_after_x = center_x / vp.zoom + vp.scroll_x;
    let content_after_y = center_y / vp.zoom + vp.scroll_y;

    assert_approx(content_after_x, content_before_x, 1e-4);
    assert_approx(content_after_y, content_before_y, 1e-4);
    // set_zoom syncs targets to scroll.
    assert_approx(vp.target_scroll_x, vp.scroll_x, 1e-6);
    assert_approx(vp.target_scroll_y, vp.scroll_y, 1e-6);
}

#[test]
fn content_screen_mapping_roundtrips_for_integer_positions() {
    let mut vp = Viewport::new(Size::new(200, 200), Size::new(1000, 1000));
    vp.zoom = 2.0;
    vp.scroll_x = 10.0;
    vp.scroll_y = 20.0;

    let p = Position::new(100, 200);
    let (sx, sy) = vp.content_to_screen(p);
    assert_approx(sx, (100.0 - 10.0) * 2.0, 1e-6);
    assert_approx(sy, (200.0 - 20.0) * 2.0, 1e-6);

    let back = vp.screen_to_content(sx, sy);
    assert_eq!(back, p);
}

#[test]
fn visible_region_uses_ceil_for_fractional_visible_content() {
    let mut vp = Viewport::new(Size::new(100, 100), Size::new(1000, 1000));
    vp.zoom = 1.5;
    vp.scroll_x = 10.0;
    vp.scroll_y = 20.0;

    let r = vp.visible_region();
    // 100/1.5 = 66.666.. => ceil = 67
    assert_eq!(r.left(), 10);
    assert_eq!(r.top(), 20);
    assert_eq!(r.size.width, 67);
    assert_eq!(r.size.height, 67);
}
