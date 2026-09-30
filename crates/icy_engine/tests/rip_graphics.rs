use icy_engine::TerminalEmulation;
use icy_engine::{PaletteScreenBuffer, Screen, ScreenMode, ScreenSink};
use icy_parser_core::RipCommand;

fn parse_rip_commands(commands: Vec<RipCommand>) -> Box<dyn icy_engine::EditableScreen> {
    let data = format!("!{}\n", commands.into_iter().map(|command| command.to_string()).collect::<String>());
    let (mut screen, mut parser) = ScreenMode::Rip.create_screen(TerminalEmulation::Rip, None);
    let mut sink = ScreenSink::new(&mut *screen);
    parser.parse(data.as_bytes(), &mut sink);
    screen
}

#[test]
fn rip_viewport_includes_lower_right_pixel() {
    let mut screen = parse_rip_commands(vec![
        RipCommand::ViewPort { x0: 0, y0: 0, x1: 10, y1: 10 },
        RipCommand::Color { c: 15 },
        RipCommand::Pixel { x: 10, y: 10 },
    ]);
    let palette = screen
        .as_any_mut()
        .downcast_mut::<PaletteScreenBuffer>()
        .expect("RIP screen should downcast to PaletteScreenBuffer");
    let offset = 10 * palette.resolution().width as usize + 10;

    assert_eq!(palette.screen()[offset], 15);
}

#[test]
fn rip_get_image_uses_inclusive_coordinates() {
    let mut screen = parse_rip_commands(vec![RipCommand::GetImage {
        x0: 0,
        y0: 0,
        x1: 1,
        y1: 1,
        res: 0,
    }]);
    let palette = screen
        .as_any_mut()
        .downcast_mut::<PaletteScreenBuffer>()
        .expect("RIP screen should downcast to PaletteScreenBuffer");
    let image = palette.bgi.rip_image.as_ref().expect("RIP_GET_IMAGE should populate the clipboard");

    assert_eq!((image.width, image.height), (2, 2));
    assert_eq!(image.data.len(), 4);
}
