use std::sync::Arc;

use icy_engine::{AttributedChar, BufferType, EditableScreen, Position, Rectangle, RenderOptions, Screen, Size, TextAttribute, TextScreen};
use icy_engine_gui::{
    render_unicode_glyph_to_rgba, render_unicode_region_to_rgba, render_unicode_screen_to_rgba, CRTShaderProgram, CRTShaderState, MonitorSettings, ScalingMode,
    Terminal,
};
use parking_lot::Mutex;

fn unicode_screen(width: i32, height: i32) -> TextScreen {
    let mut screen = TextScreen::new((width, height));
    screen.buffer.buffer_type = BufferType::Unicode;
    screen.set_unicode_width(true);
    screen
}

fn render(screen: &dyn Screen, region: Rectangle) -> Vec<u8> {
    render_unicode_region_to_rgba(
        screen,
        region,
        &RenderOptions {
            blink_on: true,
            ..Default::default()
        },
    )
    .1
}

fn frame(terminal: &Terminal, state: &CRTShaderState) -> icy_engine_gui::TerminalShader {
    CRTShaderProgram::new(
        terminal,
        Arc::new(MonitorSettings {
            scaling_mode: ScalingMode::Manual(1.0),
            ..Default::default()
        }),
        None,
    )
    .frame(state, [640.0, 480.0], 1.0)
}

#[test]
fn unicode_is_rendered_by_the_real_crt_tile_path_and_dirty_edits_repaint() {
    let mut screen = unicode_screen(8, 2);
    screen.buffer.layers[0]
        .set_grapheme(Position::new(0, 0), "Ж", TextAttribute::default())
        .unwrap();
    let state = CRTShaderState::from_screen(&screen);
    let terminal = Terminal::new(Arc::new(Mutex::new(Box::new(screen))));
    let first = frame(&terminal, &state);
    let slice = &first.slices_blink_off[0];
    assert!(slice.rgba_data.chunks_exact(4).any(|pixel| pixel[0] != 0));
    let mut screen = terminal.screen.lock();
    screen
        .as_editable()
        .unwrap()
        .set_char(Position::new(0, 0), AttributedChar::new('Ω', TextAttribute::default()));
    drop(screen);
    let second = frame(&terminal, &state);
    assert_ne!(first.slices_blink_off[0].rgba_data, second.slices_blink_off[0].rgba_data);
}

#[test]
fn combining_clusters_and_wide_continuations_keep_following_cells_in_place() {
    let mut screen = unicode_screen(4, 1);
    let layer = &mut screen.buffer.layers[0];
    layer.set_grapheme(Position::new(0, 0), "e\u{301}", TextAttribute::default()).unwrap();
    layer.set_grapheme(Position::new(1, 0), "界", TextAttribute::default()).unwrap();
    layer.set_grapheme(Position::new(3, 0), "X", TextAttribute::default()).unwrap();
    let full = render(&screen, Rectangle::from(0, 0, 32, 16));
    for col in [0, 1, 2, 3] {
        assert!(
            (0..16).any(|y| (col * 8..(col + 1) * 8).any(|x| full[(y * 32 + x) * 4] > 0)),
            "cell {col} has no glyph pixels"
        );
    }
    let crop = render(&screen, Rectangle::from(16, 0, 16, 16));
    for y in 0..16 {
        assert_eq!(&crop[y * 64..(y + 1) * 64], &full[y * 128 + 64..(y + 1) * 128]);
    }
    let mut baseline = unicode_screen(1, 1);
    baseline.buffer.layers[0]
        .set_grapheme(Position::new(0, 0), "X", TextAttribute::default())
        .unwrap();
    let expected = render(&baseline, Rectangle::from(0, 0, 8, 16));
    for y in 0..16 {
        assert_eq!(&full[y * 128 + 96..(y + 1) * 128], &expected[y * 32..(y + 1) * 32]);
    }
}

#[test]
fn rgb_extended_colors_blink_and_half_wide_selection_are_preserved() {
    let mut screen = unicode_screen(2, 1);
    let mut attribute = TextAttribute::default();
    attribute.set_foreground_rgb(200, 100, 50);
    attribute.set_background_ext(21);
    attribute.set_is_blinking(true);
    screen.buffer.layers[0].set_grapheme(Position::new(0, 0), "界", attribute).unwrap();
    let region = Rectangle::from(0, 0, 16, 16);
    let hidden = render_unicode_region_to_rgba(&screen, region, &RenderOptions::default()).1;
    assert!(hidden.chunks_exact(4).all(|pixel| pixel == [0, 0, 255, 255]));
    let shown = render(&screen, region);
    assert!(shown.chunks_exact(4).any(|pixel| pixel[0] > 0));
    let selection = icy_engine::Selection::new((1, 0));
    let selected = render_unicode_region_to_rgba(
        &screen,
        region,
        &RenderOptions {
            blink_on: true,
            selection: Some(selection),
            selection_fg: Some(icy_engine::Color::new(255, 255, 255)),
            selection_bg: Some(icy_engine::Color::new(12, 34, 56)),
            ..Default::default()
        },
    )
    .1;
    for y in 0..16 {
        assert_eq!(&shown[y * 64..y * 64 + 32], &selected[y * 64..y * 64 + 32]);
    }
    assert_ne!(shown, selected);
}

#[test]
fn raw_vertical_crops_and_full_screen_aspect_ratio_agree() {
    let mut screen = unicode_screen(2, 2);
    screen.set_aspect_ratio(true);
    screen.buffer.layers[0]
        .set_grapheme(Position::new(0, 0), "╬", TextAttribute::default())
        .unwrap();
    screen.buffer.layers[0]
        .set_grapheme(Position::new(0, 1), "Ω", TextAttribute::default())
        .unwrap();
    let raw = render(&screen, Rectangle::from(0, 0, 16, 32));
    let crop = render(&screen, Rectangle::from(0, 13, 16, 7));
    assert_eq!(crop, raw[13 * 64..20 * 64]);
    let (size, full) = render_unicode_screen_to_rgba(
        &screen,
        &RenderOptions {
            blink_on: true,
            ..Default::default()
        },
    );
    assert_eq!(size.height, screen.font_dimensions().height * 2);
    assert_eq!(full.len(), (size.width * size.height * 4) as usize);
}

#[test]
fn cp437_crt_tiles_still_use_the_engine_bitmap_renderer() {
    let mut screen = TextScreen::new((8, 2));
    screen.buffer.layers[0].set_char((0, 0), AttributedChar::new('\u{db}', TextAttribute::default()));
    let expected = screen.render_text_region_to_rgba_raw(Rectangle::from(0, 0, 64, 32), &RenderOptions::default());
    let state = CRTShaderState::from_screen(&screen);
    let terminal = Terminal::new(Arc::new(Mutex::new(Box::new(screen))));
    let frame = frame(&terminal, &state);
    assert_eq!(frame.text_raster_scale, 1);
    assert_eq!((frame.slices_blink_off[0].width, frame.slices_blink_off[0].height), (64, 32));
    assert_eq!(&*frame.slices_blink_off[0].rgba_data, &expected.1);
}

#[test]
fn unicode_resolution_tracks_zoom_and_density_without_changing_cell_coordinates() {
    let mut screen = unicode_screen(8, 2);
    screen.buffer.layers[0]
        .set_grapheme(Position::new(0, 0), "X", TextAttribute::default())
        .unwrap();
    screen.caret.set_position(Position::new(3, 1));
    screen.caret.blinking = false;
    let state = CRTShaderState::from_screen(&screen);
    let terminal = Terminal::new(Arc::new(Mutex::new(Box::new(screen))));
    let mut previous: Option<icy_engine_gui::TerminalShader> = None;
    for (zoom, density, scale) in [(1.0, 1.0, 1), (2.0, 1.0, 2), (2.0, 2.0, 4), (1.0, 1.0, 1)] {
        let settings = Arc::new(MonitorSettings {
            scaling_mode: ScalingMode::Manual(zoom),
            ..Default::default()
        });
        let current = CRTShaderProgram::new(&terminal, settings, None).frame(&state, [640.0, 480.0], density);
        assert_eq!(current.text_raster_scale, scale);
        assert_eq!(
            (current.slices_blink_off[0].width, current.slices_blink_off[0].height),
            (64 * scale, 32 * scale)
        );
        assert_eq!(current.slice_heights, [32]);
        assert_eq!(current.texture_width, 64);
        assert_eq!((current.font_width, current.font_height), (8.0, 16.0));
        let graphics = &current.slices_blink_off[current.text_slice_count];
        assert_eq!((graphics.width, graphics.height), (64, 32));
        if let Some(previous) = previous {
            assert_ne!(previous.render_generation, current.render_generation);
            assert_eq!(previous.caret_pos, current.caret_pos);
            assert_eq!(previous.caret_size, current.caret_size);
        }
        previous = Some(current);
    }
}

#[test]
fn unicode_tiles_without_blinking_cells_share_one_texture_for_both_blink_states() {
    let mut screen = unicode_screen(8, 2);
    screen.buffer.layers[0]
        .set_grapheme(Position::new(0, 0), "X", TextAttribute::default())
        .unwrap();
    let state = CRTShaderState::from_screen(&screen);
    let terminal = Terminal::new(Arc::new(Mutex::new(Box::new(screen))));
    let steady = frame(&terminal, &state);
    assert!(Arc::ptr_eq(&steady.slices_blink_off[0].rgba_data, &steady.slices_blink_on[0].rgba_data));
    let mut attribute = TextAttribute::default();
    attribute.set_is_blinking(true);
    terminal
        .screen
        .lock()
        .as_editable()
        .unwrap()
        .set_char(Position::new(1, 1), AttributedChar::new('B', attribute));
    let blinking = frame(&terminal, &state);
    assert!(!Arc::ptr_eq(&blinking.slices_blink_off[0].rgba_data, &blinking.slices_blink_on[0].rgba_data));
    assert_ne!(blinking.slices_blink_off[0].rgba_data, blinking.slices_blink_on[0].rgba_data);
}

#[test]
fn unicode_high_resolution_tiles_preserve_crops_and_dirty_lines_at_tile_boundaries() {
    let mut screen = unicode_screen(2, 200);
    let mut attribute = TextAttribute::default();
    attribute.set_foreground_rgb(255, 255, 255);
    for row in [15, 16] {
        screen.buffer.layers[0].set_grapheme(Position::new(0, row), "X", attribute).unwrap();
    }
    let state = CRTShaderState::from_screen(&screen);
    let terminal = Terminal::new(Arc::new(Mutex::new(Box::new(screen))));
    let zoomed = |terminal: &Terminal| {
        let settings = Arc::new(MonitorSettings {
            scaling_mode: ScalingMode::Manual(2.0),
            ..Default::default()
        });
        CRTShaderProgram::new(terminal, settings, None).frame(&state, [640.0, 480.0], 1.0)
    };
    let before = zoomed(&terminal);
    assert_eq!(before.text_raster_scale, 2);
    assert!(before.text_slice_count >= 2);
    let (_, expected) = render_unicode_glyph_to_rgba("X", Size::new(16, 32), [255, 255, 255, 255]);
    for (tile, y_offset) in [(0, 15 * 32), (1, 0)] {
        let slice = &before.slices_blink_off[tile];
        assert_eq!(slice.height, 512);
        for y in 0..32 {
            for x in 0..16 {
                assert_eq!(slice.rgba_data[((y_offset + y) * slice.width as usize + x) * 4], expected[(y * 16 + x) * 4 + 3]);
            }
        }
    }
    terminal
        .screen
        .lock()
        .as_editable()
        .unwrap()
        .set_char(Position::new(0, 16), AttributedChar::new('Y', attribute));
    let after = zoomed(&terminal);
    assert!(Arc::ptr_eq(&before.slices_blink_off[0].rgba_data, &after.slices_blink_off[0].rgba_data));
    assert!(!Arc::ptr_eq(&before.slices_blink_off[1].rgba_data, &after.slices_blink_off[1].rgba_data));
}

#[test]
fn unicode_zoomed_out_views_remain_covered_when_tile_geometry_changes() {
    let screen = unicode_screen(8, 2000);
    let state = CRTShaderState::from_screen(&screen);
    let terminal = Terminal::new(Arc::new(Mutex::new(Box::new(screen))));
    let program = CRTShaderProgram::new(
        &terminal,
        Arc::new(MonitorSettings {
            scaling_mode: ScalingMode::Manual(0.1),
            use_integer_scaling: false,
            ..Default::default()
        }),
        None,
    );
    let mut previous_generation = None;
    for height in [1000.0, 400.0, 1000.0] {
        let current = program.frame(&state, [640.0, height], 1.0);
        assert_eq!(current.text_raster_scale, 1);
        let covered = current.slice_heights.iter().sum::<u32>() as f32 + current.first_slice_start_y;
        assert!(covered >= (current.scroll_offset_y + current.visible_height) / current.aspect_ratio_y);
        if let Some(previous) = previous_generation {
            assert_ne!(previous, current.render_generation);
        }
        previous_generation = Some(current.render_generation);
    }
}

#[test]
fn switching_buffer_type_invalidates_tiles_without_moving_the_caret() {
    let mut screen = TextScreen::new((8, 2));
    screen.buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::default()));
    screen.caret.set_position(Position::new(3, 1));
    screen.caret.blinking = false;
    let state = CRTShaderState::from_screen(&screen);
    let terminal = Terminal::new(Arc::new(Mutex::new(Box::new(screen))));
    let bitmap = frame(&terminal, &state);
    // Deliberately do not mark lines dirty: the screen type itself changes rendering.
    *terminal.screen.lock().as_editable().unwrap().buffer_type_mut() = BufferType::Unicode;
    let unicode = frame(&terminal, &state);
    assert_ne!(bitmap.slices_blink_off[0].rgba_data, unicode.slices_blink_off[0].rgba_data);
    assert_eq!(bitmap.caret_pos, unicode.caret_pos);
    assert_eq!(bitmap.caret_size, unicode.caret_size);
    *terminal.screen.lock().as_editable().unwrap().buffer_type_mut() = BufferType::CP437;
    let restored = frame(&terminal, &state);
    assert_eq!(bitmap.slices_blink_off[0].rgba_data, restored.slices_blink_off[0].rgba_data);
}

#[test]
fn scan_lines_duplicate_raw_rows_and_crop_in_doubled_pixel_space() {
    let mut screen = unicode_screen(2, 1);
    screen.scan_lines = true;
    screen.buffer.layers[0]
        .set_grapheme(Position::new(0, 0), "Ω", TextAttribute::default())
        .unwrap();
    let doubled = render(&screen, Rectangle::from(0, 0, 16, 32));
    screen.scan_lines = false;
    let normal = render(&screen, Rectangle::from(0, 0, 16, 16));
    for y in 0..32 {
        assert_eq!(&doubled[y * 64..(y + 1) * 64], &normal[(y / 2) * 64..(y / 2 + 1) * 64]);
    }
    screen.scan_lines = true;
    let crop = render(&screen, Rectangle::from(0, 13, 16, 7));
    assert_eq!(crop, doubled[13 * 64..20 * 64]);
}

#[test]
fn thumbnail_crop_is_applied_before_allocating_a_large_document() {
    let mut screen = unicode_screen(8, 200_000);
    screen.buffer.layers[0]
        .set_grapheme(Position::new(0, 0), "Ж", TextAttribute::default())
        .unwrap();
    let options = RenderOptions {
        rect: Rectangle::from(0, 0, 8, 80).into(),
        blink_on: true,
        ..Default::default()
    };
    let (size, pixels) = render_unicode_screen_to_rgba(&screen, &options);
    assert_eq!(size, Size::new(64, 1280));
    assert_eq!(pixels.len(), 64 * 1280 * 4);
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[0] > 0));
    let (size, pixels) = render_unicode_screen_to_rgba(&screen, &RenderOptions::default());
    assert_eq!(size, Size::new(0, 0), "uncropped document exceeds the allocation limit");
    assert!(pixels.is_empty());
}

#[test]
fn display_crop_honors_origin_one_cell_bounds_clamping_and_aspect_ratio() {
    let mut screen = unicode_screen(4, 3);
    screen.buffer.layers[0]
        .set_grapheme(Position::new(1, 1), "Ω", TextAttribute::default())
        .unwrap();
    let mut options = RenderOptions {
        rect: Rectangle::from(1, 1, 1, 1).into(),
        blink_on: true,
        ..Default::default()
    };
    let (size, pixels) = render_unicode_screen_to_rgba(&screen, &options);
    assert_eq!(size, Size::new(8, 16));
    assert_eq!(pixels, render(&screen, Rectangle::from(8, 16, 8, 16)));
    options.rect = Rectangle::from(0, 0, 1, 1).into();
    assert_eq!(render_unicode_screen_to_rgba(&screen, &options).0, Size::new(8, 16));
    options.rect = Rectangle::from(3, 2, 8, 80).into();
    screen.set_aspect_ratio(true);
    assert_eq!(
        render_unicode_screen_to_rgba(&screen, &options).0,
        Size::new(8, screen.font_dimensions().height)
    );
    options.rect = Rectangle::from(100, 100, 8, 80).into();
    let (size, pixels) = render_unicode_screen_to_rgba(&screen, &options);
    assert_eq!(size, Size::new(0, 0));
    assert!(pixels.is_empty());
}

#[test]
fn picker_preview_uses_the_same_grapheme_masks_as_screen_rendering() {
    for text in ["Ж", "e\u{301}", "界"] {
        let mut screen = unicode_screen(2, 1);
        let mut attribute = TextAttribute::default();
        attribute.set_foreground_rgb(255, 255, 255);
        attribute.set_background_rgb(0, 0, 0);
        screen.buffer.layers[0].set_grapheme(Position::new(0, 0), text, attribute).unwrap();
        let (size, preview) = render_unicode_glyph_to_rgba(text, Size::new(8, 16), [255, 255, 255, 255]);
        let expected = render(&screen, Rectangle::from(0, 0, size.width, size.height));
        for (actual, expected) in preview.chunks_exact(4).zip(expected.chunks_exact(4)) {
            assert_eq!(actual[3], expected[0]);
            if actual[3] > 0 {
                assert_eq!(&actual[..3], &[255, 255, 255]);
            }
        }
    }
}
