use icy_engine::{PaletteScreenBuffer, Screen, ScreenMode, ScreenSink};
use icy_net::telnet::TerminalEmulation;
use icy_parser_core::RipCommand;

fn parse_rip_commands(commands: Vec<RipCommand>) -> Box<dyn icy_engine::EditableScreen> {
    let data = format!("!{}\n", commands.into_iter().map(|command| command.to_string()).collect::<String>());
    let (mut screen, mut parser) = ScreenMode::Rip.create_screen(TerminalEmulation::Rip, None);
    let mut sink = ScreenSink::new(&mut *screen);
    parser.parse(data.as_bytes(), &mut sink);
    screen
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
