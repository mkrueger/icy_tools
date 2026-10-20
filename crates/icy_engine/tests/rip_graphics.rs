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

#[test]
fn rip_pixel_mutations_report_dirty_lines() {
    let (mut screen, mut parser) = ScreenMode::Rip.create_screen(TerminalEmulation::Rip, None);
    let dirty = screen.get_dirty_lines();
    assert!(dirty.is_some());
    screen.clear_dirty_lines();
    assert_eq!(screen.get_dirty_lines(), None);
    parser.parse(b"Hello RIP", &mut ScreenSink::new(&mut *screen));
    assert_eq!(screen.get_dirty_lines(), dirty, "text must invalidate cached pixels");

    for mutate in [
        |screen: &mut dyn icy_engine::EditableScreen| screen.clear_screen(),
        |screen: &mut dyn icy_engine::EditableScreen| screen.scroll_up(),
        |screen: &mut dyn icy_engine::EditableScreen| screen.scroll_down(),
        |screen: &mut dyn icy_engine::EditableScreen| screen.scroll_left(),
        |screen: &mut dyn icy_engine::EditableScreen| screen.scroll_right(),
        |screen: &mut dyn icy_engine::EditableScreen| screen.screen_mut()[0] = 15,
        |screen: &mut dyn icy_engine::EditableScreen| screen.palette_mut().set_color(15, icy_engine::Color::new(255, 0, 0)),
        |screen: &mut dyn icy_engine::EditableScreen| screen.reset_resolution(),
        |screen: &mut dyn icy_engine::EditableScreen| screen.handle_rip_command(RipCommand::Pixel { x: 639, y: 349 }),
    ] {
        screen.clear_dirty_lines();
        mutate(&mut *screen);
        assert_eq!(screen.get_dirty_lines(), dirty);
    }
}

#[test]
fn rip_rendered_size_and_tiles_match_screen_resolution() {
    let screen = parse_rip_commands(vec![RipCommand::Color { c: 15 }, RipCommand::Pixel { x: 639, y: 349 }]);
    let options = icy_engine::RenderOptions::default();
    let (size, pixels) = screen.render_to_rgba(&options);
    assert_eq!(size, icy_engine::Size::new(640, 350));
    assert_eq!(screen.resolution(), size);
    assert_eq!(screen.virtual_size(), size);
    assert_eq!(pixels.len(), 640 * 350 * 4);
    assert!(!screen.scan_lines());

    let mut tiled = Vec::new();
    for y in (0..350).step_by(63) {
        let height = 63.min(350 - y);
        let (tile_size, tile) = screen.render_text_region_to_rgba_raw(icy_engine::Rectangle::from(0, y, 640, height), &options);
        assert_eq!(tile_size, icy_engine::Size::new(640, height));
        tiled.extend(tile);
    }
    assert_eq!(tiled, pixels);
    assert_eq!(&pixels[pixels.len() - 4..], &[255, 255, 255, 255]);
}
