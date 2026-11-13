//! A click on a scaled terminal must hit RIP/IGS mouse fields by screen pixel, not by text cell.

use icy_engine::{KeyModifiers, MouseButton, ScreenMode, ScreenSink, TerminalEmulation, TerminalResolution};
use icy_engine_gui::{CRTShaderState, RenderInfo, TerminalMouseEvent};

fn render_info(terminal_width: f32, terminal_height: f32, scale: f32, scan_lines: bool) -> RenderInfo {
    RenderInfo {
        display_scale: scale,
        viewport_x: 20.0,
        viewport_y: 10.0,
        viewport_width: terminal_width * scale,
        viewport_height: terminal_height * scale,
        terminal_width,
        terminal_height,
        font_width: 8.0,
        font_height: 8.0,
        scan_lines,
        bounds_x: 0.0,
        bounds_y: 0.0,
        bounds_width: terminal_width * scale + 40.0,
        bounds_height: terminal_height * scale + 20.0,
    }
}

/// Builds the event the terminal widget publishes for a press at window position (mx, my).
fn press_at(info: &RenderInfo, mx: f32, my: f32) -> TerminalMouseEvent {
    let state = CRTShaderState::default();
    let cell = state.map_mouse_to_cell(info, mx, my, 0.0, 0.0);
    let pixel = state.map_mouse_to_pixel(info, mx, my, 0.0, 0.0);
    TerminalMouseEvent::new((mx, my), cell, MouseButton::Left, KeyModifiers::default()).with_terminal_pixel_position(pixel)
}

fn rip_screen() -> Box<dyn icy_engine::EditableScreen> {
    let (mut screen, mut parser) = ScreenMode::Rip.create_screen(TerminalEmulation::Rip, None);
    let mut sink = ScreenSink::new(&mut *screen);
    // RIP_MOUSE at pixels x 20-120, y 8-40 sending "a".
    let mouse = icy_parser_core::RipCommand::Mouse {
        num: 0,
        x0: 20,
        y0: 8,
        x1: 120,
        y1: 40,
        clk: 1,
        clr: 0,
        res: 0,
        text: "a".to_string(),
    };
    assert_eq!(mouse.to_string(), "|1M000K083C141000000a");
    parser.parse(format!("!{mouse}\n").as_bytes(), &mut sink);
    screen
}

#[test]
fn rip_click_maps_window_position_to_screen_pixels() {
    let screen = rip_screen();
    assert_eq!(screen.mouse_fields().len(), 1);
    let field = &screen.mouse_fields()[0];
    assert_eq!((field.x1, field.y1, field.x2, field.y2), (20, 8, 120, 40));

    // 640x350 RIP screen drawn at 2x, offset (20, 10) in the widget.
    let info = render_info(640.0, 350.0, 2.0, false);

    // Pixel (70, 24) is inside the region.
    let evt = press_at(&info, 20.0 + 70.0 * 2.0, 10.0 + 24.0 * 2.0);
    assert_eq!(evt.text_position, Some(icy_engine::Position::new(8, 3)));
    assert_eq!(evt.get_rip_field(&*screen), Some((false, "a".to_string())));

    // Text cell (50, 20) used to match (20 <= 50 <= 120, 8 <= 20 <= 40); it is pixel (400, 160).
    let evt = press_at(&info, 20.0 + 400.0 * 2.0 + 1.0, 10.0 + 160.0 * 2.0 + 1.0);
    assert_eq!(evt.text_position, Some(icy_engine::Position::new(50, 20)));
    assert_eq!(evt.get_rip_field(&*screen), None);

    // Region edges at this scale: x 120 is the last pixel in, 121 is out.
    assert!(press_at(&info, 20.0 + 120.0 * 2.0 + 1.5, 10.0 + 40.0 * 2.0 + 1.5)
        .get_rip_field(&*screen)
        .is_some());
    assert!(press_at(&info, 20.0 + 121.0 * 2.0, 10.0 + 24.0 * 2.0).get_rip_field(&*screen).is_none());
}

#[test]
fn igs_click_with_scan_lines_hits_zone_in_screen_pixels() {
    // Medium resolution: 640x200 pixels, drawn as a 640x400 texture with scan lines.
    let (mut screen, mut parser) = ScreenMode::AtariST(TerminalResolution::Medium, true).create_screen(TerminalEmulation::AtariST, None);
    let mut sink = ScreenSink::new(&mut *screen);
    parser.parse(b"G#X>4,1,26,48,215,53,1,a:", &mut sink);

    let info = render_info(640.0, 400.0, 1.5, true);

    // Screen pixel (100, 50) is texture row 100.
    let evt = press_at(&info, 20.0 + 100.0 * 1.5, 10.0 + 100.0 * 1.5);
    assert_eq!(evt.terminal_pixel_position, Some(icy_engine::Position::new(100, 50)));
    assert_eq!(evt.get_rip_field(&*screen), Some((false, "a".to_string())));

    // Screen pixel (100, 60) is below the zone.
    assert_eq!(press_at(&info, 20.0 + 100.0 * 1.5, 10.0 + 120.0 * 1.5).get_rip_field(&*screen), None);
}

#[test]
fn click_outside_the_terminal_hits_nothing() {
    let screen = rip_screen();
    let info = render_info(640.0, 350.0, 2.0, false);
    let evt = press_at(&info, 5.0, 5.0);
    assert_eq!(evt.terminal_pixel_position, None);
    assert_eq!(evt.get_rip_field(&*screen), None);
}
