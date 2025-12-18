use icy_engine::Screen;
use icy_engine_gui::terminal::Terminal;
use parking_lot::Mutex;
use std::sync::Arc;

#[test]
fn terminal_sync_scrollbar_updates_viewport_scrollbar_positions() {
    // Use a real Screen implementation to keep this test focused on scrollbar sync.
    let screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(icy_engine::TextScreen::new(icy_engine::Size::new(80, 25)))));

    let mut term = Terminal::new(screen);

    // Simulate a 400% zoom view with a small visible area.
    {
        let mut vp = term.viewport.write();
        vp.zoom = 4.0;
        vp.set_visible_size(800.0, 500.0);
        vp.set_content_size(2000.0, 1500.0);

        // Force scroll to bottom-right (max).
        let max_x = vp.max_scroll_x();
        let max_y = vp.max_scroll_y();
        vp.scroll_x = max_x;
        vp.scroll_y = max_y;
        vp.target_scroll_x = max_x;
        vp.target_scroll_y = max_y;

        // Intentionally desync viewport scrollbar to reproduce the bug.
        vp.scrollbar.set_scroll_position_x(0.0);
        vp.scrollbar.set_scroll_position(0.0);
    }

    term.sync_scrollbar_with_viewport();

    let vp = term.viewport.read();
    assert!((vp.scrollbar.scroll_position_x - 1.0).abs() < 1e-6, "expected horizontal scrollbar at end");
    assert!((vp.scrollbar.scroll_position - 1.0).abs() < 1e-6, "expected vertical scrollbar at end");
}
