use icy_engine::Screen;
use icy_engine_gui::terminal::Terminal;
use icy_ui::Rectangle;
use parking_lot::Mutex;
use std::sync::Arc;

#[test]
fn terminal_update_scroll_from_viewport_tracks_content_scroll_in_content_pixels() {
    // Use a real Screen implementation to keep this test focused on scroll state caching.
    let screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(icy_engine::TextScreen::new(icy_engine::Size::new(80, 25)))));

    let term = Terminal::new(screen);

    // `show_viewport` reports the viewport rectangle in *zoomed* pixels.
    // The terminal caches scroll in *content* pixels by dividing by the effective zoom.
    let zoom = 4.0;
    let viewport = Rectangle {
        x: 400.0,
        y: 200.0,
        width: 800.0,
        height: 500.0,
    };

    term.update_scroll_from_viewport(viewport, zoom);
    let state = term.scroll_state();

    assert!((state.scroll_x - 100.0).abs() < 1e-6);
    assert!((state.scroll_y - 50.0).abs() < 1e-6);
    assert!((state.viewport_width_px - 800.0).abs() < 1e-6);
    assert!((state.viewport_height_px - 500.0).abs() < 1e-6);
}
