use icy_engine::{EditableScreen, GraphicsType, Rectangle, RenderOptions, ScreenMode, ScreenSink, Size, TerminalEmulation};
use icy_parser_core::{CommandSink, SkypixCommand};

fn skypix_screen() -> Box<dyn EditableScreen> {
    ScreenMode::SkyPix.create_screen(TerminalEmulation::Skypix, None).0
}

#[test]
fn skypix_reports_dirty_lines_after_streamed_output() {
    let (mut screen, mut parser) = ScreenMode::SkyPix.create_screen(TerminalEmulation::Skypix, None);
    assert_eq!(screen.get_dirty_lines(), Some((0, 25)));
    screen.clear_dirty_lines();
    assert_eq!(screen.get_dirty_lines(), None);

    {
        let mut sink = ScreenSink::new(&mut *screen);
        parser.parse(b"Hello SkyPix", &mut sink);
    }
    assert!(screen.screen().iter().any(|&pixel| pixel != 0));
    assert_eq!(screen.get_dirty_lines(), Some((0, 25)));
    screen.clear_dirty_lines();

    {
        let mut sink = ScreenSink::new(&mut *screen);
        sink.emit_skypix(SkypixCommand::SetPixel { x: 639, y: 199 });
    }
    assert_eq!(screen.get_dirty_lines(), Some((0, 25)));
    screen.clear_dirty_lines();
    assert_eq!(screen.get_dirty_lines(), None);
}

#[test]
fn skypix_display_size_matches_rendered_size() {
    let screen = skypix_screen();
    let (size, pixels) = screen.render_to_rgba(&RenderOptions::default());
    assert_eq!(size, Size::new(640, 400));
    assert_eq!(screen.resolution(), size);
    assert_eq!(screen.virtual_size(), size);
    assert_eq!(pixels.len(), 640 * 400 * 4);
    assert_eq!(screen.screen().len(), 640 * 200);
}

#[test]
fn skypix_tiles_use_doubled_scanline_coordinates() {
    let mut screen = skypix_screen();
    {
        let mut sink = ScreenSink::new(&mut *screen);
        sink.emit_skypix(SkypixCommand::SetPixel { x: 639, y: 199 });
    }
    let options = RenderOptions::default();
    let (full_size, full) = screen.render_to_rgba(&options);
    let mut tiled = Vec::new();
    for y in (0..400).step_by(63) {
        let height = 63.min(400 - y);
        let region = Rectangle::from(0, y, 640, height);
        let (size, pixels) = screen.render_text_region_to_rgba_raw(region, &options);
        assert_eq!(size, Size::new(640, height));
        tiled.extend(pixels);
    }
    assert_eq!(tiled, full);
    let stride = full_size.width as usize * 4;
    let white = [221, 221, 221, 255];
    assert_eq!(&full[398 * stride + 639 * 4..399 * stride], &white);
    assert_eq!(&full[399 * stride + 639 * 4..400 * stride], &white);
}

#[test]
fn skypix_scanlines_can_be_disabled_for_export() {
    let screen = skypix_screen();
    let options = RenderOptions {
        override_scan_lines: Some(false),
        ..Default::default()
    };
    let (size, pixels) = screen.render_to_rgba(&options);
    assert_eq!(size, Size::new(640, 200));
    assert_eq!(pixels.len(), 640 * 200 * 4);
}

#[test]
fn skypix_resolution_reset_preserves_native_dimensions() {
    let mut screen = skypix_screen();
    screen.reset_resolution();
    assert_eq!(screen.resolution(), Size::new(640, 400));
    assert_eq!(screen.screen().len(), 640 * 200);
    assert_eq!(screen.graphics_type(), GraphicsType::Skypix);
}

#[test]
fn skypix_fill_uses_native_pixel_coordinates() {
    let mut screen = skypix_screen();
    {
        let mut sink = ScreenSink::new(&mut *screen);
        sink.emit_skypix(SkypixCommand::AreaFill {
            mode: icy_parser_core::FillMode::Color,
            x: 639,
            y: 199,
        });
    }
    assert!(screen.screen().iter().all(|&pixel| pixel == 2));
}

#[test]
fn skypix_palette_scroll_and_erase_invalidate_cached_pixels() {
    let mut screen = skypix_screen();
    for command in [&b"\x1b[2J"[..], b"\x1b[K", b"\x1b[1K", b"\x1b[2K", b"\x1b[J", b"\x1b[1J"] {
        let (_, mut parser) = ScreenMode::SkyPix.create_screen(TerminalEmulation::Skypix, None);
        screen.clear_dirty_lines();
        parser.parse(command, &mut ScreenSink::new(&mut *screen));
        assert_eq!(screen.get_dirty_lines(), Some((0, 25)), "{command:?}");
    }
    screen.clear_dirty_lines();
    screen.scroll_up();
    assert_eq!(screen.get_dirty_lines(), Some((0, 25)));
    screen.clear_dirty_lines();
    screen.scroll_down();
    assert_eq!(screen.get_dirty_lines(), Some((0, 25)));
    screen.clear_dirty_lines();
    screen.palette_mut().set_color(2, icy_engine::Color::new(255, 255, 255));
    assert_eq!(screen.get_dirty_lines(), Some((0, 25)));
}

fn parse_skypix(input: &[u8]) -> Box<dyn EditableScreen> {
    let (mut screen, mut parser) = ScreenMode::SkyPix.create_screen(TerminalEmulation::Skypix, None);
    let mut sink = ScreenSink::new(&mut *screen);
    parser.parse(input, &mut sink);
    assert!(sink.diagnostics().is_empty(), "{:?}", sink.diagnostics());
    screen
}

#[test]
fn skypix_reign_of_fire_title_matches_original_amiga_glyphs() {
    let screen = parse_skypix(b"\x1b[10;19!Sapphire.font!\x1b[15;6!\x1b[19;25;25!Reign \x1b[15;2!of \x1b[15;3!Fire \x1b[15;2!BBS");
    let reference = image::open("tests/output/skypix/files/rof/rof_main.png").unwrap().to_rgba8();
    let mut expected_count = 0;
    // The original's angled border crosses the area below the last title glyph.
    for y in 29..47 {
        for x in 25..230 {
            let expected = reference.get_pixel(x as u32, y as u32 * 2).0;
            let expected_ink = expected[..3].iter().any(|&value| value > 0);
            expected_count += usize::from(expected_ink);
            assert_eq!(screen.screen()[y * 640 + x] != 0, expected_ink, "title pixel ({x}, {y})");
        }
    }
    assert!(expected_count > 100);
}

#[test]
fn skypix_custom_font_uses_proportional_advances_and_space_bearings() {
    let screen = parse_skypix(b"\x1b[10;19!Sapphire.font!\x1b[19;25;25!Reign ");
    // Sapphire 19 advances: R=16, e=9, i=5, g=13, n=12, space=13.
    assert_eq!(screen.caret().position(), (93, 25).into());
    assert_eq!(screen.font_dimensions(), Size::new(20, 19));
}

#[test]
fn skypix_courier_36_preserves_full_width_height_and_bearings() {
    let screen = parse_skypix(b"\x1b[10;36!Courier.font!\x1b[19;10;10!MW");
    assert_eq!(screen.font_dimensions(), Size::new(22, 36));
    assert_eq!(screen.caret().position(), (54, 10).into());
    assert!(screen.screen()[..640 * 200]
        .chunks_exact(640)
        .skip(10)
        .take(45)
        .any(|row| row[18..32].iter().any(|&pixel| pixel != 0)));
    assert!(screen
        .screen()
        .chunks_exact(640)
        .skip(42)
        .take(12)
        .any(|row| row[10..54].iter().any(|&pixel| pixel != 0)));
}

#[test]
fn skypix_custom_font_cr_advances_line_and_lf_is_ignored() {
    let screen = parse_skypix(b"\x1b[10;19!Sapphire.font!\x1b[19;25;25!R\r\n");
    assert_eq!(screen.caret().position(), (0, 44).into());
}

#[test]
fn skypix_graphics_commands_preserve_default_text_mode() {
    let screen = parse_skypix(b"\x1b[15;6!A\r\nB");
    assert_eq!(screen.caret().position(), (8, 8).into());
    assert_eq!(screen.caret().font_page(), 0);
}

#[test]
fn skypix_font_reset_restores_default_cell_and_text_mode() {
    for reset in [&b"\x1b[10;0!"[..], b"\x1b[10;12!Missing.font!", b"\x1b[99!", b"\x1b[m", b"\x1b[0m"] {
        let mut input = b"\x1b[10;36!Courier.font!\x1b[19;10;10!M".to_vec();
        input.extend_from_slice(reset);
        let screen = parse_skypix(&input);
        assert_eq!(screen.caret().position(), (32, 38).into());
        assert_eq!(screen.font_dimensions(), Size::new(8, 8));
        assert!(screen.caret().visible);
        assert_eq!(screen.caret().font_page(), 0);
    }
}

#[test]
fn skypix_custom_font_clips_at_edges_without_wrapping_or_scrolling() {
    let screen = parse_skypix(b"\x1b[1;0;0!\x1b[10;36!Courier.font!\x1b[19;630;190!MW");
    assert_eq!(screen.caret().position(), (674, 190).into());
    assert_eq!(screen.screen()[0], 2, "clipped graphics text must not scroll the screen");
}

#[test]
fn skypix_all_bundled_fonts_preserve_native_bitmaps_and_metrics() {
    for (name, height, data, _) in icy_engine::skypix::load_amiga_fonts() {
        let font = libyaff::YaffFont::from_bytes(data.as_bytes()).unwrap();
        for code in [b' ', b'.', b'i', b'M', b'W', b'g', 0xa9] {
            let glyph = font
                .glyphs
                .iter()
                .find(|glyph| {
                    glyph
                        .labels
                        .iter()
                        .any(|label| matches!(label, libyaff::Label::Codepoint(codes) if codes.contains(&u16::from(code))))
                })
                .or_else(|| {
                    font.glyphs.iter().find(|glyph| {
                        glyph
                            .labels
                            .iter()
                            .any(|label| matches!(label, libyaff::Label::Unicode(codes) if codes.contains(&u32::from(code))))
                    })
                })
                .or_else(|| {
                    font.glyphs
                        .iter()
                        .find(|glyph| glyph.labels.iter().any(|label| matches!(label, libyaff::Label::Tag(tag) if tag == "default")))
                })
                .unwrap_or_else(|| panic!("{name} {height}: no glyph or default for byte {code}"));
            let left = glyph.left_bearing.or(font.global_left_bearing).unwrap_or(0);
            let right = glyph.right_bearing.or(font.global_right_bearing).unwrap_or(0);
            let shift = glyph.shift_up.or(font.global_shift_up).unwrap_or(0);
            let mut input = format!("\x1b[10;{height}!{name}!\x1b[19;100;50!").into_bytes();
            input.push(code);
            let screen = parse_skypix(&input);
            let mut expected = vec![0; 640 * 200];
            for (y, row) in glyph.bitmap.pixels.iter().enumerate() {
                for (x, &pixel) in row.iter().enumerate() {
                    if pixel {
                        expected[((50 + y as i32 - shift) * 640 + 100 + left + x as i32) as usize] = 2;
                    }
                }
            }
            assert_eq!(screen.screen(), expected, "{name} {height}, byte {code}");
            assert_eq!(
                screen.caret().position(),
                (100 + left + glyph.bitmap.width as i32 + right, 50).into(),
                "{name} {height}, byte {code} advance"
            );
        }
    }
}

#[test]
fn skypix_font_demo_matches_bundled_reference_layout() {
    let screen = parse_skypix(include_bytes!("output/skypix/files/rof/rof_intro.ans"));
    let reference = image::open("tests/output/skypix/files/rof/rof_intro.png").unwrap().to_rgba8();
    let (size, pixels) = screen.render_to_rgba(&RenderOptions::default());
    assert_eq!((size.width as u32, size.height as u32), reference.dimensions());
    for (index, (actual, expected)) in pixels.chunks_exact(4).zip(reference.pixels()).enumerate() {
        // The old reference omitted the ellipsis's left bearing; the native
        // period bitmap and bearings are checked for every font above.
        if index % size.width as usize >= 360 && index / size.width as usize >= 200 {
            continue;
        }
        assert_eq!(
            actual[..3].iter().any(|&value| value > 0),
            expected.0[..3].iter().any(|&value| value > 0),
            "font demo pixel ({}, {})",
            index % size.width as usize,
            index / size.width as usize
        );
    }
}
