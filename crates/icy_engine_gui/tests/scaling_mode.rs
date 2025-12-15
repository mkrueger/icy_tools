use icy_engine_gui::ScalingMode;

fn assert_approx(a: f32, b: f32, eps: f32) {
    assert!((a - b).abs() <= eps, "{a} != {b} (eps={eps})");
}

#[test]
fn auto_compute_zoom_uses_uniform_scale() {
    // Content: 100x200, viewport: 300x300 -> scale_x=3.0, scale_y=1.5 => min=1.5
    let z = ScalingMode::Auto.compute_zoom(100.0, 200.0, 300.0, 300.0, false);
    assert_approx(z, 1.5, 1e-6);

    // Content: 200x100, viewport: 300x300 -> scale_x=1.5, scale_y=3.0 => min=1.5
    let z2 = ScalingMode::Auto.compute_zoom(200.0, 100.0, 300.0, 300.0, false);
    assert_approx(z2, 1.5, 1e-6);
}

#[test]
fn auto_compute_zoom_integer_scaling_never_below_one() {
    // Integer scaling cannot downscale below 1.0.
    let z = ScalingMode::Auto.compute_zoom(100.0, 100.0, 90.0, 90.0, true);
    assert_approx(z, 1.0, 1e-6);

    // If it fits at >1, it floors to the largest integer that fits.
    let z2 = ScalingMode::Auto.compute_zoom(100.0, 100.0, 350.0, 350.0, true);
    assert_approx(z2, 3.0, 1e-6);
}

#[test]
fn manual_compute_zoom_respects_integer_rounding() {
    let z = ScalingMode::Manual(1.6).compute_zoom(100.0, 100.0, 200.0, 200.0, true);
    assert_approx(z, 2.0, 1e-6);

    let z2 = ScalingMode::Manual(1.4).compute_zoom(100.0, 100.0, 200.0, 200.0, true);
    assert_approx(z2, 1.0, 1e-6);

    // Integer scaling: never below 1.0.
    let z3 = ScalingMode::Manual(0.6).compute_zoom(100.0, 100.0, 200.0, 200.0, true);
    assert_approx(z3, 1.0, 1e-6);

    // Non-integer scaling: value is passed through.
    let z4 = ScalingMode::Manual(0.6).compute_zoom(100.0, 100.0, 200.0, 200.0, false);
    assert_approx(z4, 0.6, 1e-6);
}
