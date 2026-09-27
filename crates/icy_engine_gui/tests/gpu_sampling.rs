//! Headless GPU checks for how the terminal shader maps texels to screen pixels.
//!
//! Run with `cargo test -p icy_engine_gui --features egui --test gpu_sampling -- --ignored`.
#![cfg(feature = "egui")]

use std::sync::Arc;

use icy_engine::{AttributedChar, EditableScreen, Position, Screen, TextAttribute, TextScreen};
use icy_engine_gui::{CRTShaderProgram, CRTShaderState, MonitorSettings, ScalingMode, Terminal, TerminalShader, TerminalShaderRenderer};
use parking_lot::Mutex;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Gpu {
    fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        runtime.block_on(async {
            let instance = wgpu::Instance::default();
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .expect("GPU adapter required");
            let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default()).await.unwrap();
            Self { device, queue }
        })
    }

    /// Renders `frame` into a `size` physical pixel target and returns its RGBA pixels.
    fn render(&self, frame: &TerminalShader, renderer: &mut TerminalShaderRenderer, size: [u32; 2], pixels_per_point: f32) -> Vec<u8> {
        let logical = [size[0] as f32 / pixels_per_point, size[1] as f32 / pixels_per_point];
        frame.prepare_frame(renderer, &self.device, &self.queue, [0.0, 0.0, logical[0], logical[1]], pixels_per_point);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gpu sampling target"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let stride = (size[0] * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu sampling readback"),
            size: u64::from(stride * size[1]),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        frame.render_to_target(renderer, &mut encoder, &view, [0, 0, size[0], size[1]]);
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: None,
                },
            },
            texture.size(),
        );
        self.queue.submit([encoder.finish()]);
        buffer.slice(..).map_async(wgpu::MapMode::Read, |result| result.unwrap());
        self.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let pixels = buffer
            .slice(..)
            .get_mapped_range()
            .chunks(stride as usize)
            .flat_map(|row| row[..size[0] as usize * 4].to_vec())
            .collect();
        buffer.unmap();
        pixels
    }
}

fn terminal(configure: impl FnOnce(&mut TextScreen)) -> (Terminal, CRTShaderState) {
    let mut screen = TextScreen::new((80, 25));
    configure(&mut screen);
    let screen: Box<dyn Screen> = Box::new(screen);
    let state = CRTShaderState::from_screen(&*screen);
    let mut terminal = Terminal::new(Arc::new(Mutex::new(screen)));
    terminal.has_focus = false;
    (terminal, state)
}

fn fill(screen: &mut TextScreen, glyph: impl Fn(i32, i32) -> (char, u32, u32)) {
    for y in 0..25 {
        for x in 0..80 {
            let (ch, fg, bg) = glyph(x, y);
            let mut attribute = TextAttribute::default();
            attribute.set_foreground(fg);
            attribute.set_background(bg);
            screen.set_char(Position::new(x, y), AttributedChar::new(ch, attribute));
        }
    }
}

fn settings(scaling_mode: ScalingMode, use_integer_scaling: bool) -> MonitorSettings {
    MonitorSettings {
        scaling_mode,
        use_integer_scaling,
        ..Default::default()
    }
}

fn frame(terminal: &Terminal, state: &CRTShaderState, settings: MonitorSettings, size: [u32; 2], pixels_per_point: f32) -> TerminalShader {
    let logical = [size[0] as f32 / pixels_per_point, size[1] as f32 / pixels_per_point];
    CRTShaderProgram::new(terminal, Arc::new(settings), None).frame(state, logical, pixels_per_point)
}

fn pixel(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * width + x) * 4) as usize;
    pixels[offset..offset + 4].try_into().unwrap()
}

fn texel(frame: &TerminalShader, x: u32, y: u32) -> [u8; 4] {
    pixel(&frame.slices_blink_off[0].rgba_data, frame.slices_blink_off[0].width, x, y)
}

/// Runs of lit samples along a line, as (start, summed coverage in units of full intensity).
fn lit_runs(samples: impl Iterator<Item = u8>) -> Vec<(usize, f32)> {
    let mut runs = Vec::new();
    let mut current: Option<(usize, f32)> = None;
    for (index, value) in samples.enumerate() {
        match (&mut current, value > 0) {
            (Some((_, coverage)), true) => *coverage += value as f32 / 255.0,
            (None, true) => current = Some((index, value as f32 / 255.0)),
            (Some(_), false) => runs.push(current.take().unwrap()),
            (None, false) => {}
        }
    }
    runs.extend(current);
    runs
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn integer_scale_is_pixel_exact() {
    let gpu = Gpu::new();
    let mut renderer = TerminalShaderRenderer::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm);
    let (terminal, state) = terminal(|screen| {
        fill(screen, |x, y| {
            (char::from_u32(((x + y * 80) % 256) as u32).unwrap(), (x % 16) as u32, (y % 8) as u32)
        })
    });
    let size = [1320, 840];
    let frame = frame(&terminal, &state, settings(ScalingMode::Manual(2.0), true), size, 1.0);
    let pixels = gpu.render(&frame, &mut renderer, size, 1.0);
    let info = terminal.render_info.read().clone();
    assert_eq!(info.display_scale, 2.0);
    let (origin_x, origin_y) = (info.viewport_x as u32, info.viewport_y as u32);
    for y in 0..800 {
        for x in 0..1280 {
            let expected = texel(&frame, x / 2, y / 2);
            let actual = pixel(&pixels, size[0], origin_x + x, origin_y + y);
            assert_eq!(actual[..3], expected[..3], "pixel ({x}, {y}) differs from its texel");
        }
    }
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn aspect_ratio_draws_every_texel_row_with_equal_weight() {
    let gpu = Gpu::new();
    let mut renderer = TerminalShaderRenderer::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm);
    let (terminal, state) = terminal(|screen| {
        screen.set_aspect_ratio(true);
        fill(screen, |_, _| ('\u{C4}', 15, 0));
    });
    let size = [720, 560];
    let frame = frame(&terminal, &state, settings(ScalingMode::Manual(1.0), false), size, 1.0);
    let pixels = gpu.render(&frame, &mut renderer, size, 1.0);
    let info = terminal.render_info.read().clone();
    let pixels_per_texel = info.font_height / 16.0;
    assert!(pixels_per_texel > 1.1, "aspect ratio correction must stretch rows, got {pixels_per_texel}");

    // Middle of the fifth cell in the first column of glyphs.
    let column = 4 * 8 + 4;
    let lit_texel_rows = (0..400).filter(|&y| texel(&frame, column, y)[0] > 0).count();
    let screen_x = info.viewport_x as u32 + column;
    let terminal_rows = info.viewport_y as u32..(info.viewport_y + info.viewport_height) as u32;
    let runs = lit_runs(terminal_rows.map(|y| pixel(&pixels, size[0], screen_x, y)[0]));
    assert_eq!(runs.len(), lit_texel_rows, "every lit texel row must stay visible");
    for (start, coverage) in runs {
        assert!(
            (coverage - pixels_per_texel).abs() < 0.05,
            "row at {start} covers {coverage} pixels instead of {pixels_per_texel}"
        );
    }
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn fractional_dpi_draws_every_texel_column_with_equal_weight() {
    let gpu = Gpu::new();
    let mut renderer = TerminalShaderRenderer::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm);
    let (terminal, state) = terminal(|screen| fill(screen, |_, _| ('\u{B3}', 15, 0)));
    // 125% display scaling: integer scaling is integer in logical points only (2x = 2.5 pixels).
    let size = [1700, 1100];
    let frame = frame(&terminal, &state, settings(ScalingMode::Auto, true), size, 1.25);
    let pixels = gpu.render(&frame, &mut renderer, size, 1.25);
    let info = terminal.render_info.read().clone();
    assert_eq!(info.display_scale, 2.0);
    let pixels_per_texel = 2.5;

    let row = 8;
    let lit: Vec<_> = lit_runs((0..640).map(|x| texel(&frame, x, row)[0]))
        .into_iter()
        .map(|(_, width)| width)
        .collect();
    let screen_y = ((info.viewport_y + (row as f32 + 0.5) * 2.0) * 1.25) as u32;
    let terminal_columns = (info.viewport_x * 1.25) as u32..((info.viewport_x + info.viewport_width) * 1.25) as u32;
    let runs = lit_runs(terminal_columns.map(|x| pixel(&pixels, size[0], x, screen_y)[0]));
    assert_eq!(runs.len(), lit.len(), "every lit texel column must stay visible");
    for ((start, coverage), texels) in runs.into_iter().zip(lit) {
        let expected = texels * pixels_per_texel;
        assert!(
            (coverage - expected).abs() < 0.05,
            "column at {start} covers {coverage} pixels instead of {expected}"
        );
    }
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn blink_states_render_with_shared_and_separate_arrays() {
    let gpu = Gpu::new();
    for ice in [true, false] {
        let mut renderer = TerminalShaderRenderer::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm);
        let (terminal, state) = terminal(|screen| {
            screen.buffer.ice_mode = if ice { icy_engine::IceMode::Ice } else { icy_engine::IceMode::Blink };
            fill(screen, |x, _| ('\u{DB}', (x % 15 + 1) as u32, 0));
        });
        let size = [700, 460];
        let frame = frame(&terminal, &state, settings(ScalingMode::Manual(1.0), true), size, 1.0);
        assert_eq!(
            frame
                .slices_blink_off
                .iter()
                .zip(&frame.slices_blink_on)
                .all(|(off, on)| Arc::ptr_eq(&off.rgba_data, &on.rgba_data)),
            ice,
            "only iCE colors share the tiles of both blink states"
        );
        let pixels = gpu.render(&frame, &mut renderer, size, 1.0);
        let info = terminal.render_info.read().clone();
        let (origin_x, origin_y) = (info.viewport_x as u32, info.viewport_y as u32);
        for (x, y) in [(4, 8), (100, 200), (600, 390)] {
            assert_eq!(
                pixel(&pixels, size[0], origin_x + x, origin_y + y)[..3],
                texel(&frame, x, y)[..3],
                "ice={ice} ({x}, {y})"
            );
        }
    }
}
