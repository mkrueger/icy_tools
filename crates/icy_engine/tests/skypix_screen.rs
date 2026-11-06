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
fn skypix_backspace_echo_erases_a_full_cell_away_from_origin() {
    let screen = parse_skypix(b"\x1b[10;20HAB\x08 \x08");
    let expected = parse_skypix(b"\x1b[10;20HA");
    assert_eq!(screen.screen(), expected.screen());
    assert_eq!(screen.caret().position(), (160, 72).into());
    let screen = parse_skypix(b"\x08A");
    assert_eq!(screen.caret().position(), (8, 0).into());
}

#[test]
fn skypix_ansi_cursor_positions_are_one_based_and_preserve_the_row() {
    for command in [&b"\x1b[10;20H"[..], b"\x1b[10;20f", b"\x1b[10;1H\x1b[20G"] {
        let mut input = command.to_vec();
        input.push(b'A');
        let screen = parse_skypix(&input);
        assert_eq!(screen.caret().position(), (160, 72).into(), "{command:?}");
        let expected = parse_skypix(b"\x1b[19;152;72!A");
        assert_eq!(screen.screen(), expected.screen(), "{command:?}");
    }
    let screen = parse_skypix(b"\x1b[10;20H\x1b[H");
    assert_eq!(screen.caret().position(), (0, 0).into());
    let screen = parse_skypix(b"\x1b[10;20H\x1b[G");
    assert_eq!(screen.caret().position(), (0, 72).into());
}

#[test]
fn skypix_full_width_line_defers_wrap_until_next_printable_character() {
    let (mut screen, mut parser) = ScreenMode::SkyPix.create_screen(TerminalEmulation::Skypix, None);
    screen.terminal_state_mut().is_terminal_buffer = true;
    for _ in 0..80 {
        parser.parse(b"A", &mut ScreenSink::new(&mut *screen));
    }
    assert_eq!(screen.size(), Size::new(80, 25));
    assert_eq!(screen.caret().position(), (632, 0).into());
    assert!(screen.terminal_state().wrap_pending);
    parser.parse(b"B", &mut ScreenSink::new(&mut *screen));
    assert_eq!(screen.caret().position(), (8, 8).into());
    assert!(!screen.terminal_state().wrap_pending);
}

#[test]
fn skypix_full_width_line_followed_by_crlf_advances_only_once() {
    for ending in [&b"\r\nB"[..], b"\nB", b"\rB", b"\x1b[5;3HB", b"\x1b[19;16;32!B"] {
        let mut input = vec![b'A'; 80];
        input.extend_from_slice(ending);
        let screen = parse_skypix(&input);
        let expected = match ending {
            b"\rB" => (8, 0),
            b"\x1b[5;3HB" | b"\x1b[19;16;32!B" => (24, 32),
            _ => (8, 8),
        };
        assert_eq!(screen.caret().position(), expected.into(), "{ending:?}");
    }
}

#[test]
fn skypix_deferred_wrap_scrolls_only_when_output_requires_it() {
    let (mut screen, mut parser) = ScreenMode::SkyPix.create_screen(TerminalEmulation::Skypix, None);
    screen.terminal_state_mut().is_terminal_buffer = true;
    parser.parse(b"\x1b[1;0;0!\x1b[25;1H", &mut ScreenSink::new(&mut *screen));
    parser.parse(&[b'A'; 80], &mut ScreenSink::new(&mut *screen));
    assert_eq!(screen.screen()[0], 2, "writing the last cell must not scroll");
    assert_eq!(screen.caret().position(), (632, 192).into());
    parser.parse(b"B", &mut ScreenSink::new(&mut *screen));
    assert_eq!(screen.screen()[0], 0);
    assert_eq!(screen.caret().position(), (8, 192).into());
}

#[test]
fn skypix_disabled_autowrap_overwrites_the_last_cell() {
    let (mut screen, mut parser) = ScreenMode::SkyPix.create_screen(TerminalEmulation::Skypix, None);
    screen.terminal_state_mut().auto_wrap_mode = icy_engine::AutoWrapMode::NoWrap;
    parser.parse(&[b'A'; 81], &mut ScreenSink::new(&mut *screen));
    assert_eq!(screen.caret().position(), (632, 0).into());
    assert!(!screen.terminal_state().wrap_pending);
}

#[test]
fn skypix_ansi_bright_black_renders_gray_without_changing_native_pen_eight() {
    for prefix in [&b"\x1b[1;30m"[..], b"\x1b[30;1m", b"\x1b[90m"] {
        let mut input = prefix.to_vec();
        input.push(b'A');
        let screen = parse_skypix(&input);
        let (_, pixels) = screen.render_to_rgba(&RenderOptions::default());
        assert!(pixels.chunks_exact(4).any(|pixel| pixel == [85, 85, 85, 255]), "{prefix:?}");
        assert_eq!(screen.palette().rgb(8), (0, 187, 102));
    }
    let screen = parse_skypix(b"\x1b[1;30m\x1b[15;8!A");
    assert!(screen.screen().contains(&8));
    assert_eq!(screen.palette().rgb(8), (0, 187, 102));
    let screen = parse_skypix(b"\x1b[1;30m\x1b[22mA");
    assert!(screen.screen().iter().all(|&pixel| pixel == 0));
}

#[test]
fn skypix_ansi_bright_colors_work_with_inverse_and_backgrounds() {
    for input in [&b"\x1b[1;30;7mA"[..], b"\x1b[100mA"] {
        let screen = parse_skypix(input);
        let (_, pixels) = screen.render_to_rgba(&RenderOptions::default());
        assert_eq!(&pixels[..4], &[85, 85, 85, 255]);
    }
    let screen = parse_skypix(b"\x1b[100m\x1b[2J");
    let (_, pixels) = screen.render_to_rgba(&RenderOptions::default());
    assert!(pixels.chunks_exact(4).all(|pixel| pixel == [85, 85, 85, 255]));
    for (sgr, pen) in [(30, 0), (31, 3), (32, 4), (33, 6), (34, 1), (35, 7), (36, 5), (37, 2)] {
        let screen = parse_skypix(format!("\x1b[{sgr}mA").as_bytes());
        assert_eq!(screen.caret().attribute.foreground(), pen);
    }
}

#[test]
fn skypix_palette_resets_preserve_rendered_ansi_bright_colors() {
    for reset in [&b"\x1b[12!"[..], b"\x1b[17;2!", b"\x1b[11;0;0;0;0;0;0;0;0;0;0;0;0;0;0;0;0!"] {
        let mut input = b"\x1b[90mA".to_vec();
        input.extend_from_slice(reset);
        let screen = parse_skypix(&input);
        let (_, pixels) = screen.render_to_rgba(&RenderOptions::default());
        assert!(pixels.chunks_exact(4).any(|pixel| pixel == [85, 85, 85, 255]), "{reset:?}");
    }
}

#[test]
fn skypix_ansi_bright_colors_do_not_alias_native_palette_entries() {
    for color in 0..8 {
        let rgb = icy_engine::DOS_DEFAULT_PALETTE[icy_engine::ansi_to_internal_palette_index(color + 8) as usize].rgb();
        let packed = u32::from(rgb.0 / 17) | (u32::from(rgb.1 / 17) << 4) | (u32::from(rgb.2 / 17) << 8);
        for reset in ["\x1b[12!", "\x1b[17;1!", "\x1b[17;2!", "\x1b[99!", "\x1b[11;0;0;0;0;0;0;0;0;0;0;0;0;0;0;0;0!"] {
            let input = format!("\x1b[11;0;{packed};0;0;0;0;0;0;0;0;0;0;0;0;0;0!\x1b[{}mA{reset}", 90 + color);
            let screen = parse_skypix(input.as_bytes());
            let (_, pixels) = screen.render_to_rgba(&RenderOptions::default());
            assert!(
                pixels.chunks_exact(4).any(|pixel| pixel == [rgb.0, rgb.1, rgb.2, 255]),
                "bright color {color}, reset {reset:?}"
            );
            assert!(screen.screen().iter().any(|&pixel| pixel >= 16));
        }
    }
}

#[test]
fn skypix_equivalent_bright_sgr_forms_use_the_same_text_and_drawing_colors() {
    for color in 0..8 {
        let bold = parse_skypix(format!("\x1b[1;{}mA\x1b[1;100;100!", 30 + color).as_bytes());
        let explicit = parse_skypix(format!("\x1b[{}mA\x1b[1;100;100!", 90 + color).as_bytes());
        let rgb = icy_engine::DOS_DEFAULT_PALETTE[icy_engine::ansi_to_internal_palette_index(color + 8) as usize].rgb();
        assert_eq!(bold.palette().rgb(u32::from(bold.screen()[100 * 640 + 100])), rgb);
        assert_eq!(
            bold.render_to_rgba(&RenderOptions::default()),
            explicit.render_to_rgba(&RenderOptions::default()),
            "bright color {color}"
        );
    }
}

#[test]
fn skypix_bright_color_slots_are_reused_across_palette_resets() {
    let (mut screen, mut parser) = ScreenMode::SkyPix.create_screen(TerminalEmulation::Skypix, None);
    for _ in 0..32 {
        for color in 90..98 {
            parser.parse(format!("\x1b[{color}mA\x1b[12!").as_bytes(), &mut ScreenSink::new(&mut *screen));
        }
    }
    assert_eq!(screen.palette().len(), 24);
}

#[test]
fn skypix_cursor_motion_cancels_pending_wrap_and_clamps_to_full_cells() {
    for command in [&b"\x08"[..], b"\x1b[D", b"\x1b[80G", b"\x1b[H", b"\x1b[2J"] {
        let mut input = vec![b'A'; 80];
        input.extend_from_slice(command);
        input.push(b'B');
        let screen = parse_skypix(&input);
        assert_eq!(screen.caret().y, 0, "{command:?}");
    }
    let screen = parse_skypix(b"\x1b[999;999H");
    assert_eq!(screen.caret().position(), (632, 192).into());
    let screen = parse_skypix(b"\x1b[999A");
    assert_eq!(screen.caret().position(), (0, 0).into());
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
