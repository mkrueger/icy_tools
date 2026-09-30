use icy_engine::TerminalEmulation;
use icy_engine::{AttributedChar, PaletteScreenBuffer, Position, Screen, ScreenMode, ScreenSink, TextAttribute};
use icy_parser_core::RipCommand;

fn parse_rip_commands(commands: Vec<RipCommand>) -> Box<dyn icy_engine::EditableScreen> {
    let data = format!("!{}\n", commands.into_iter().map(|command| command.to_string()).collect::<String>());
    let (mut screen, mut parser) = ScreenMode::Rip.create_screen(TerminalEmulation::Rip, None);
    let mut sink = ScreenSink::new(&mut *screen);
    parser.parse(data.as_bytes(), &mut sink);
    screen
}

fn parse_rip_input(data: &[u8]) -> Box<dyn icy_engine::EditableScreen> {
    let (mut screen, mut parser) = ScreenMode::Rip.create_screen(TerminalEmulation::Rip, None);
    let mut sink = ScreenSink::new(&mut *screen);
    parser.parse(data, &mut sink);
    screen
}

#[test]
fn rip_stw_rtw_restores_text_window() {
    let screen = parse_rip_commands(vec![
        RipCommand::TextWindow {
            x0: 2,
            y0: 3,
            x1: 30,
            y1: 20,
            wrap: true,
            size: 0,
        },
        RipCommand::TextVariable { text: "STW".to_string() },
        RipCommand::TextWindow {
            x0: 8,
            y0: 9,
            x1: 40,
            y1: 25,
            wrap: true,
            size: 0,
        },
        RipCommand::TextVariable { text: "RTW".to_string() },
    ]);

    assert_eq!(screen.terminal_state().margins_left_right(), Some((2, 30)));
    assert_eq!(screen.terminal_state().margins_top_bottom(), Some((3, 20)));
    assert_eq!(screen.caret_position(), screen.upper_left_position());
}

#[test]
fn rip_gotoxy_is_relative_to_text_window() {
    let screen = parse_rip_commands(vec![
        RipCommand::TextWindow {
            x0: 2,
            y0: 3,
            x1: 30,
            y1: 20,
            wrap: true,
            size: 0,
        },
        RipCommand::GotoXY { x: 4, y: 5 },
    ]);

    assert_eq!(screen.caret_position(), Position::new(6, 8));
}

#[test]
fn rip_home_moves_to_text_window_origin() {
    let screen = parse_rip_commands(vec![
        RipCommand::TextWindow {
            x0: 2,
            y0: 3,
            x1: 30,
            y1: 20,
            wrap: true,
            size: 0,
        },
        RipCommand::GotoXY { x: 4, y: 5 },
        RipCommand::Home,
    ]);

    assert_eq!(screen.caret_position(), Position::new(2, 3));
}

#[test]
fn identical_rip_text_window_preserves_cursor() {
    let window = RipCommand::TextWindow {
        x0: 2,
        y0: 3,
        x1: 30,
        y1: 20,
        wrap: true,
        size: 0,
    };
    let screen = parse_rip_commands(vec![window.clone(), RipCommand::GotoXY { x: 4, y: 5 }, window]);

    assert_eq!(screen.caret_position(), Position::new(6, 8));
}

#[test]
fn rip_erase_window_clears_only_active_window() {
    let (mut screen, _) = ScreenMode::Rip.create_screen(TerminalEmulation::Rip, None);
    screen.handle_rip_command(RipCommand::TextWindow {
        x0: 2,
        y0: 3,
        x1: 30,
        y1: 20,
        wrap: true,
        size: 0,
    });

    let mut attribute = TextAttribute::default();
    attribute.set_background(15);
    screen.set_char(Position::new(1, 3), AttributedChar::new(' ', attribute));
    screen.set_char(Position::new(2, 3), AttributedChar::new(' ', attribute));
    let font_size = screen.font_dimensions();

    screen.handle_rip_command(RipCommand::EraseWindow);

    assert_eq!(screen.terminal_state().margins_left_right(), Some((2, 30)));
    assert_eq!(screen.terminal_state().margins_top_bottom(), Some((3, 20)));
    assert_eq!(screen.caret_position(), Position::new(2, 3));

    let palette = screen
        .as_any_mut()
        .downcast_mut::<PaletteScreenBuffer>()
        .expect("RIP screen should downcast to PaletteScreenBuffer");
    let width = palette.resolution().width as usize;
    let pixel = |x: i32, y: i32| palette.screen()[(y * font_size.height) as usize * width + (x * font_size.width) as usize];
    assert_eq!(pixel(1, 3), 15, "pixels outside the text window must remain unchanged");
    assert_eq!(pixel(2, 3), 0, "the text window must be cleared with the RIP background color");
}

#[test]
fn rip_erase_eol_stops_at_text_window_margin() {
    let (mut screen, _) = ScreenMode::Rip.create_screen(TerminalEmulation::Rip, None);
    screen.handle_rip_command(RipCommand::TextWindow {
        x0: 2,
        y0: 3,
        x1: 4,
        y1: 5,
        wrap: true,
        size: 0,
    });

    let mut attribute = TextAttribute::default();
    attribute.set_background(15);
    for x in 2..=5 {
        screen.set_char(Position::new(x, 3), AttributedChar::new(' ', attribute));
    }
    let font_size = screen.font_dimensions();
    screen.handle_rip_command(RipCommand::GotoXY { x: 1, y: 0 });
    screen.handle_rip_command(RipCommand::EraseEOL);

    let palette = screen
        .as_any_mut()
        .downcast_mut::<PaletteScreenBuffer>()
        .expect("RIP screen should downcast to PaletteScreenBuffer");
    let width = palette.resolution().width as usize;
    let pixel = |x: i32| palette.screen()[(3 * font_size.height) as usize * width + (x * font_size.width) as usize];
    assert_eq!(pixel(2), 15, "pixels before the cursor must remain unchanged");
    assert_eq!(pixel(3), 0);
    assert_eq!(pixel(4), 0);
    assert_eq!(pixel(5), 15, "pixels beyond the text window must remain unchanged");
}

#[test]
fn zero_rip_text_window_suppresses_tty_output_until_reenabled() {
    let mut disabled = parse_rip_input(b"!|w0000000000\nH");
    let disabled = disabled
        .as_any_mut()
        .downcast_mut::<PaletteScreenBuffer>()
        .expect("RIP screen should downcast to PaletteScreenBuffer");
    assert_eq!(count_nonzero_pixels(disabled, 0, 0, 640, 350), 0);

    let mut reenabled = parse_rip_input(b"!|w0000000000\nH\n!|w00000J0Z01\nH");
    let reenabled = reenabled
        .as_any_mut()
        .downcast_mut::<PaletteScreenBuffer>()
        .expect("RIP screen should downcast to PaletteScreenBuffer");
    assert!(count_nonzero_pixels(reenabled, 0, 0, 640, 350) > 0);
}

fn count_nonzero_pixels(screen: &PaletteScreenBuffer, left: i32, top: i32, right: i32, bottom: i32) -> usize {
    let resolution = screen.resolution();
    let left = left.clamp(0, resolution.width);
    let right = right.clamp(0, resolution.width);
    let top = top.clamp(0, resolution.height);
    let bottom = bottom.clamp(0, resolution.height);
    let width = resolution.width as usize;

    (top as usize..bottom as usize)
        .map(|y| {
            let row_start = y * width;
            (left as usize..right as usize).filter(|x| screen.screen()[row_start + *x] != 0).count()
        })
        .sum()
}

fn rightmost_nonzero_pixel(screen: &PaletteScreenBuffer, left: i32, top: i32, right: i32, bottom: i32) -> Option<i32> {
    let resolution = screen.resolution();
    let left = left.clamp(0, resolution.width);
    let right = right.clamp(0, resolution.width);
    let top = top.clamp(0, resolution.height);
    let bottom = bottom.clamp(0, resolution.height);
    let width = resolution.width as usize;

    let mut rightmost = None;
    for y in top as usize..bottom as usize {
        let row_start = y * width;
        for x in left as usize..right as usize {
            if screen.screen()[row_start + x] != 0 {
                rightmost = Some(x as i32);
            }
        }
    }
    rightmost
}

#[test]
fn region_text_renders_and_advances_lines() {
    let mut screen = parse_rip_commands(vec![
        RipCommand::FontStyle {
            font: 0,
            direction: 0,
            size: 1,
            res: 0,
        },
        RipCommand::Color { c: 15 },
        RipCommand::BeginText {
            x0: 10,
            y0: 10,
            x1: 160,
            y1: 60,
            res: 0,
        },
        RipCommand::RegionText {
            justify: false,
            text: "FIRST LINE".to_string(),
        },
        RipCommand::RegionText {
            justify: false,
            text: "SECOND LINE".to_string(),
        },
        RipCommand::EndText,
    ]);

    let palette = screen
        .as_any_mut()
        .downcast_mut::<PaletteScreenBuffer>()
        .expect("RIP screen should downcast to PaletteScreenBuffer");
    let line_height = palette.bgi.text_size("Ay").height.max(1);

    assert!(count_nonzero_pixels(palette, 10, 10, 160, 10 + line_height) > 0);
    assert!(count_nonzero_pixels(palette, 10, 10 + line_height, 160, 10 + 2 * line_height) > 0);
}

#[test]
fn justified_region_text_uses_available_width() {
    let commands = |justify| {
        vec![
            RipCommand::FontStyle {
                font: 0,
                direction: 0,
                size: 1,
                res: 0,
            },
            RipCommand::Color { c: 15 },
            RipCommand::BeginText {
                x0: 10,
                y0: 10,
                x1: 220,
                y1: 40,
                res: 0,
            },
            RipCommand::RegionText {
                justify,
                text: "ONE TWO THREE".to_string(),
            },
            RipCommand::EndText,
        ]
    };

    let mut left_screen = parse_rip_commands(commands(false));
    let left_palette = left_screen
        .as_any_mut()
        .downcast_mut::<PaletteScreenBuffer>()
        .expect("RIP screen should downcast to PaletteScreenBuffer");
    let line_height = left_palette.bgi.text_size("Ay").height.max(1);
    let left_rightmost = rightmost_nonzero_pixel(left_palette, 10, 10, 220, 10 + line_height).expect("left-justified text should draw pixels");

    let mut justified_screen = parse_rip_commands(commands(true));
    let justified_palette = justified_screen
        .as_any_mut()
        .downcast_mut::<PaletteScreenBuffer>()
        .expect("RIP screen should downcast to PaletteScreenBuffer");
    let justified_rightmost = rightmost_nonzero_pixel(justified_palette, 10, 10, 220, 10 + line_height).expect("justified text should draw pixels");

    assert!(justified_rightmost > left_rightmost + 10);
}
