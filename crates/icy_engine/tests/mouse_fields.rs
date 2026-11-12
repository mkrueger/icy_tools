//! RIP buttons / mouse regions and IGS zones are stored in screen pixels and must be
//! hit-tested against the clicked pixel, not the clicked text cell.

use icy_engine::{clickable_mouse_field_at, Position, Screen, ScreenMode, ScreenSink, TerminalEmulation, TerminalResolution};
use icy_parser_core::RipCommand;

fn parse_rip_commands(commands: Vec<RipCommand>) -> Box<dyn icy_engine::EditableScreen> {
    let data = format!("!{}\n", commands.into_iter().map(|command| command.to_string()).collect::<String>());
    let (mut screen, mut parser) = ScreenMode::Rip.create_screen(TerminalEmulation::Rip, None);
    let mut sink = ScreenSink::new(&mut *screen);
    parser.parse(data.as_bytes(), &mut sink);
    screen
}

fn mouse_region(x0: u16, y0: u16, x1: u16, y1: u16, text: &str) -> RipCommand {
    RipCommand::Mouse {
        num: 0,
        x0,
        y0,
        x1,
        y1,
        clk: 1,
        clr: 0,
        res: 0,
        text: text.to_string(),
    }
}

/// A RIP button at pixels x 20-120, y 8-40 (the shape from the manual repro).
fn button_screen() -> Box<dyn icy_engine::EditableScreen> {
    parse_rip_commands(vec![
        RipCommand::ButtonStyle {
            wid: 0,
            hgt: 0,
            orient: 2,
            flags: 1024, // mouse button
            bevsize: 2,
            dfore: 15,
            dback: 0,
            bright: 15,
            dark: 8,
            surface: 7,
            grp_no: 0,
            flags2: 0,
            uline_col: 0,
            corner_col: 0,
            res: 0,
        },
        RipCommand::Button {
            x0: 20,
            y0: 8,
            x1: 120,
            y1: 40,
            hotkey: 0,
            flags: 0,
            res: 0,
            text: "<>A<>a".to_string(),
        },
    ])
}

fn command_at(screen: &dyn Screen, x: i32, y: i32) -> Option<String> {
    clickable_mouse_field_at(screen.mouse_fields(), Position::new(x, y)).and_then(|field| field.host_command.clone())
}

#[test]
fn rip_button_is_hit_inside_its_pixel_area() {
    let screen = button_screen();
    assert_eq!(command_at(&*screen, 70, 24).as_deref(), Some("a"));
    // Text cell (3, 2) of the 8x8 RIP font covers pixels 24..32 x 16..24, inside the button.
    assert_eq!(command_at(&*screen, 3 * 8, 2 * 8).as_deref(), Some("a"));
}

#[test]
fn rip_button_is_not_hit_at_its_coordinates_read_as_a_cell() {
    let screen = button_screen();
    // Clicking text cell (50, 20) used to match because 20 <= 50 <= 120 and 8 <= 20 <= 40.
    // That cell is at pixel (400, 160), well outside the button.
    assert_eq!(command_at(&*screen, 50 * 8, 20 * 8), None);
    assert_eq!(command_at(&*screen, 50 * 8 + 7, 20 * 8 + 7), None);
}

#[test]
fn rip_button_edges_are_inclusive() {
    let screen = button_screen();
    for (x, y) in [(20, 8), (120, 8), (20, 40), (120, 40)] {
        assert_eq!(command_at(&*screen, x, y).as_deref(), Some("a"), "corner ({x}, {y})");
    }
    for (x, y) in [(19, 8), (121, 8), (20, 7), (20, 41), (121, 41)] {
        assert_eq!(command_at(&*screen, x, y), None, "outside ({x}, {y})");
    }
}

#[test]
fn rip_mouse_region_is_hit_in_pixels() {
    let screen = parse_rip_commands(vec![mouse_region(300, 200, 400, 260, "m")]);
    assert_eq!(command_at(&*screen, 300, 200).as_deref(), Some("m"));
    assert_eq!(command_at(&*screen, 400, 260).as_deref(), Some("m"));
    assert_eq!(command_at(&*screen, 299, 230), None);
    assert_eq!(command_at(&*screen, 350, 261), None);
    // Tested against text cells (at most column 79 / row 43) this region could never be hit.
    assert_eq!(command_at(&*screen, 79 * 8, 43 * 8), None);
}

#[test]
fn first_defined_mouse_field_wins_where_fields_overlap() {
    let screen = parse_rip_commands(vec![mouse_region(100, 100, 200, 200, "first"), mouse_region(150, 150, 250, 250, "second")]);
    assert_eq!(command_at(&*screen, 175, 175).as_deref(), Some("first"));
    assert_eq!(command_at(&*screen, 225, 225).as_deref(), Some("second"));
}

#[test]
fn mouse_field_without_host_command_is_not_clickable() {
    let screen = parse_rip_commands(vec![mouse_region(100, 100, 200, 200, ""), mouse_region(100, 100, 200, 200, "under")]);
    assert_eq!(command_at(&*screen, 150, 150).as_deref(), Some("under"));
}

#[test]
fn igs_zone_is_stored_and_hit_in_pixels() {
    // Medium resolution: 640x200 pixels with scan lines, 80x25 text cells.
    let (mut screen, mut parser) = ScreenMode::AtariST(TerminalResolution::Medium, true).create_screen(TerminalEmulation::AtariST, None);
    let mut sink = ScreenSink::new(&mut *screen);
    parser.parse(b"G#X>4,1,26,48,215,53,1,a:", &mut sink);

    let fields = screen.mouse_fields();
    assert_eq!(fields.len(), 1);
    assert_eq!((fields[0].x1, fields[0].y1, fields[0].x2, fields[0].y2), (26, 48, 215, 53));

    assert_eq!(command_at(&*screen, 100, 50).as_deref(), Some("a"));
    assert_eq!(command_at(&*screen, 100, 54), None);
    // Text cell (30, 50) used to match; it is not even on the 25-row screen.
    assert_eq!(command_at(&*screen, 30 * 8, 50 * 8), None);
}
