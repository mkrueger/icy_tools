use icy_engine::Position;
use icy_engine_gui::{CRTShaderState, RenderInfo};

fn make_render_info() -> RenderInfo {
    RenderInfo {
        display_scale: 1.0,
        viewport_x: 0.0,
        viewport_y: 0.0,
        viewport_width: 200.0,
        viewport_height: 150.0,
        terminal_width: 200.0,
        terminal_height: 150.0,
        font_width: 10.0,
        font_height: 20.0,
        scan_lines: false,
        bounds_x: 0.0,
        bounds_y: 0.0,
        bounds_width: 200.0,
        bounds_height: 150.0,
    }
}

#[test]
fn mouse_to_cell_accounts_for_fractional_scroll_x_carry() {
    let state = CRTShaderState::default();
    let render_info = make_render_info();

    let scroll_x = 8.0; // < font_width
    let scroll_y = 0.0;

    // term_x=8 -> (8 + 8)/10 = 1.6 => cell 1
    let pos = state.map_mouse_to_cell(&render_info, 8.0, 5.0, scroll_x, scroll_y);
    assert_eq!(pos, Some(Position::new(1, 0)));
}

#[test]
fn mouse_to_cell_accounts_for_fractional_scroll_y_carry() {
    let state = CRTShaderState::default();
    let render_info = make_render_info();

    let scroll_x = 0.0;
    let scroll_y = 19.0; // < font_height

    // term_y=19 -> (19 + 19)/20 = 1.9 => row 1
    let pos = state.map_mouse_to_cell(&render_info, 5.0, 19.0, scroll_x, scroll_y);
    assert_eq!(pos, Some(Position::new(0, 1)));
}

#[test]
fn mouse_to_cell_accounts_for_fractional_scroll_with_display_scale() {
    let state = CRTShaderState::default();
    let mut render_info = make_render_info();
    let scroll_x = 8.0;
    let scroll_y = 0.0;

    render_info.display_scale = 2.0;

    // screen mx=16 -> term_x=8 -> (8 + 8)/10 => cell 1
    let pos = state.map_mouse_to_cell(&render_info, 16.0, 0.0, scroll_x, scroll_y);
    assert_eq!(pos, Some(Position::new(1, 0)));
}

#[test]
fn mouse_to_cell_scanlines_accounts_for_fractional_scroll_y_carry() {
    let state = CRTShaderState::default();
    let mut render_info = make_render_info();
    let scroll_x = 0.0;
    let scroll_y = 19.0;

    render_info.scan_lines = true;

    // With scanlines: term_y is halved before mapping.
    // my=38 -> term_y=38 -> adjusted=19; (19 + 19)/20 => row 1
    let pos = state.map_mouse_to_cell(&render_info, 0.0, 38.0, scroll_x, scroll_y);
    assert_eq!(pos, Some(Position::new(0, 1)));
}

#[test]
fn mouse_to_cell_respects_viewport_offsets_and_bounds() {
    let state = CRTShaderState::default();
    let mut render_info = make_render_info();
    let scroll_x = 0.0;
    let scroll_y = 0.0;

    render_info.viewport_x = 5.0;
    render_info.viewport_y = 6.0;
    render_info.viewport_width = 100.0;
    render_info.viewport_height = 50.0;

    // Left of viewport => None
    assert_eq!(state.map_mouse_to_cell(&render_info, 4.0, 10.0, scroll_x, scroll_y), None);
    // Above viewport => None
    assert_eq!(state.map_mouse_to_cell(&render_info, 10.0, 5.0, scroll_x, scroll_y), None);

    // Exactly at viewport origin => first cell
    assert_eq!(state.map_mouse_to_cell(&render_info, 5.0, 6.0, scroll_x, scroll_y), Some(Position::new(0, 0)));
}
