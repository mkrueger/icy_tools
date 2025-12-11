//! Benchmarks for render_region_to_rgba across different Screen implementations
//!
//! Tests TextScreen, PaletteScreenBuffer, and ScrollbackBuffer performance
//! for various region sizes and offsets.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use icy_engine::{
    AttributedChar, GraphicsType, PaletteScreenBuffer, Position, Rectangle, RenderOptions, Screen, ScrollbackBuffer, ScrollbackChunk, Size, TextScreen,
};
use std::hint::black_box;

// ============================================================================
// Test data creation helpers
// ============================================================================

/// Create a test TextScreen wrapping a TextBuffer
fn create_text_screen(width: i32, height: i32, scan_lines: bool) -> TextScreen {
    let mut screen = TextScreen::new(Size::new(width, height));
    screen.scan_lines = scan_lines;

    // Fill with mixed content to simulate realistic usage
    for y in 0..height {
        for x in 0..width {
            let ch = match (x + y) % 4 {
                0 => 'A',
                1 => '#',
                2 => '*',
                _ => ' ',
            };

            let attr_char = AttributedChar::new(ch, Default::default());
            screen.buffer.layers[0].set_char(Position::new(x, y), attr_char);
        }
    }

    screen
}

/// Create a test PaletteScreenBuffer with RIP graphics type
fn create_palette_screen() -> PaletteScreenBuffer {
    // RIP graphics is 640x350
    PaletteScreenBuffer::new(GraphicsType::Rip)
}

/// Create a test ScrollbackBuffer with some chunks
fn create_scrollback_buffer(chunk_count: usize, chunk_width: i32, chunk_height: i32) -> ScrollbackBuffer {
    let mut buffer = ScrollbackBuffer::new();
    buffer.font_dimensions = Size::new(8, 16);

    // Add some test chunks
    for i in 0..chunk_count {
        let size = Size::new(chunk_width, chunk_height);
        // Create RGBA data (4 bytes per pixel)
        let pixel_count = (chunk_width * chunk_height) as usize;
        let mut rgba_data = vec![0u8; pixel_count * 4];

        // Fill with pattern
        for y in 0..chunk_height {
            for x in 0..chunk_width {
                let idx = ((y * chunk_width + x) as usize) * 4;
                let color = ((x + y + i as i32) % 256) as u8;
                rgba_data[idx] = color; // R
                rgba_data[idx + 1] = color; // G
                rgba_data[idx + 2] = color; // B
                rgba_data[idx + 3] = 255; // A
            }
        }

        buffer.add_chunk(rgba_data, size);
    }

    // Also set a current screen
    let cur_size = Size::new(chunk_width, chunk_height);
    let cur_pixels = (chunk_width * chunk_height) as usize;
    let cur_rgba = vec![128u8; cur_pixels * 4];
    buffer.cur_screen = ScrollbackChunk {
        rgba_data: cur_rgba,
        size: cur_size,
    };

    buffer
}

// ============================================================================
// Generic benchmark helpers using Screen trait
// ============================================================================

/// Benchmark different region sizes for any Screen implementation
fn bench_screen_full_region<S: Screen>(group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>, screen: &S, screen_name: &str) {
    let resolution = screen.resolution();
    let options = RenderOptions::default();

    // Full screen
    let full_region = Rectangle::from_coords(0, 0, resolution.width, resolution.height);
    let full_pixels = (resolution.width * resolution.height * 4) as u64;
    group.throughput(Throughput::Bytes(full_pixels));
    group.bench_with_input(
        BenchmarkId::new(screen_name, format!("full_{}x{}", resolution.width, resolution.height)),
        &full_region,
        |b, r| {
            b.iter(|| black_box(screen.render_region_to_rgba(*r, &options)));
        },
    );
}

fn bench_screen_half_region<S: Screen>(group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>, screen: &S, screen_name: &str) {
    let resolution = screen.resolution();
    let options = RenderOptions::default();

    // Half screen (typical viewport)
    let half_w = resolution.width / 2;
    let half_h = resolution.height / 2;
    if half_w > 0 && half_h > 0 {
        let half_region = Rectangle::from_coords(0, 0, half_w, half_h);
        let half_pixels = (half_w * half_h * 4) as u64;
        group.throughput(Throughput::Bytes(half_pixels));
        group.bench_with_input(BenchmarkId::new(screen_name, format!("half_{}x{}", half_w, half_h)), &half_region, |b, r| {
            b.iter(|| black_box(screen.render_region_to_rgba(*r, &options)));
        });
    }
}

fn bench_screen_quarter_region<S: Screen>(group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>, screen: &S, screen_name: &str) {
    let resolution = screen.resolution();
    let options = RenderOptions::default();

    // Quarter screen (scrolling scenario)
    let quarter_w = resolution.width / 4;
    let quarter_h = resolution.height / 4;
    if quarter_w > 0 && quarter_h > 0 {
        let quarter_region = Rectangle::from_coords(0, 0, quarter_w, quarter_h);
        let quarter_pixels = (quarter_w * quarter_h * 4) as u64;
        group.throughput(Throughput::Bytes(quarter_pixels));
        group.bench_with_input(
            BenchmarkId::new(screen_name, format!("quarter_{}x{}", quarter_w, quarter_h)),
            &quarter_region,
            |b, r| {
                b.iter(|| black_box(screen.render_region_to_rgba(*r, &options)));
            },
        );
    }
}

fn bench_screen_offset_region<S: Screen>(group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>, screen: &S, screen_name: &str) {
    let resolution = screen.resolution();
    let options = RenderOptions::default();

    // Offset region (middle of screen)
    let offset_x = resolution.width / 4;
    let offset_y = resolution.height / 4;
    let offset_w = resolution.width / 2;
    let offset_h = resolution.height / 2;
    if offset_w > 0 && offset_h > 0 && offset_x + offset_w <= resolution.width && offset_y + offset_h <= resolution.height {
        let offset_region = Rectangle::from_coords(offset_x, offset_y, offset_x + offset_w, offset_y + offset_h);
        let offset_pixels = (offset_w * offset_h * 4) as u64;
        group.throughput(Throughput::Bytes(offset_pixels));
        group.bench_with_input(
            BenchmarkId::new(screen_name, format!("offset_{}+{}_{}x{}", offset_x, offset_y, offset_w, offset_h)),
            &offset_region,
            |b, r| {
                b.iter(|| black_box(screen.render_region_to_rgba(*r, &options)));
            },
        );
    }
}

/// Run all region benchmarks for a Screen implementation
fn bench_all_regions<S: Screen>(group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>, screen: &S, screen_name: &str) {
    bench_screen_full_region(group, screen, screen_name);
    bench_screen_half_region(group, screen, screen_name);
    bench_screen_quarter_region(group, screen, screen_name);
    bench_screen_offset_region(group, screen, screen_name);
}

// ============================================================================
// Benchmark functions
// ============================================================================

fn bench_text_screen(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_screen");

    // Test different buffer sizes
    let sizes = [(80, 25, "80x25"), (200, 100, "200x100")];

    for (width, height, name) in sizes {
        let screen = create_text_screen(width, height, false);
        bench_all_regions(&mut group, &screen, name);
    }

    group.finish();
}

fn bench_palette_screen(c: &mut Criterion) {
    let mut group = c.benchmark_group("palette_screen");

    // RIP graphics (640x350)
    let rip_screen = create_palette_screen();
    bench_all_regions(&mut group, &rip_screen, "rip");

    group.finish();
}

fn bench_scrollback_buffer(c: &mut Criterion) {
    let mut group = c.benchmark_group("scrollback_buffer");

    // Test with different chunk counts
    let configs = [(5, 640, 400, "5_chunks"), (20, 640, 400, "20_chunks"), (100, 640, 400, "100_chunks")];

    for (chunk_count, width, height, name) in configs {
        let buffer = create_scrollback_buffer(chunk_count, width, height);
        bench_all_regions(&mut group, &buffer, name);
    }

    group.finish();
}

fn bench_scanlines_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("scanlines");

    let options = RenderOptions::default();

    // TextScreen without scanlines
    let text_no_scan = create_text_screen(80, 25, false);
    let text_resolution = text_no_scan.resolution();
    let text_region = Rectangle::from_coords(0, 0, text_resolution.width, text_resolution.height);

    group.throughput(Throughput::Bytes((text_resolution.width * text_resolution.height * 4) as u64));
    group.bench_function("text_no_scanlines", |b| {
        b.iter(|| black_box(text_no_scan.render_region_to_rgba(text_region, &options)));
    });

    // TextScreen with scanlines (height doubled in output)
    let text_with_scan = create_text_screen(80, 25, true);
    group.throughput(Throughput::Bytes((text_resolution.width * text_resolution.height * 2 * 4) as u64));
    group.bench_function("text_with_scanlines", |b| {
        b.iter(|| black_box(text_with_scan.render_region_to_rgba(text_region, &options)));
    });

    // PaletteScreen
    let palette_screen = create_palette_screen();
    let palette_resolution = palette_screen.resolution();
    let palette_region = Rectangle::from_coords(0, 0, palette_resolution.width, palette_resolution.height);
    group.throughput(Throughput::Bytes((palette_resolution.width * palette_resolution.height * 4) as u64));
    group.bench_function("palette_rip", |b| {
        b.iter(|| black_box(palette_screen.render_region_to_rgba(palette_region, &options)));
    });

    group.finish();
}

fn bench_small_regions(c: &mut Criterion) {
    let mut group = c.benchmark_group("small_regions");

    let options = RenderOptions::default();

    // Very small regions (cursor area, single character updates)
    let text_screen = create_text_screen(80, 25, false);
    let font_size = text_screen.font_dimensions();

    // Single character region
    let single_char = Rectangle::from_coords(0, 0, font_size.width, font_size.height);
    group.throughput(Throughput::Bytes((font_size.width * font_size.height * 4) as u64));
    group.bench_function("text_single_char", |b| {
        b.iter(|| black_box(text_screen.render_region_to_rgba(single_char, &options)));
    });

    // 5x5 character region
    let small_region = Rectangle::from_coords(0, 0, font_size.width * 5, font_size.height * 5);
    group.throughput(Throughput::Bytes((font_size.width * 5 * font_size.height * 5 * 4) as u64));
    group.bench_function("text_5x5_chars", |b| {
        b.iter(|| black_box(text_screen.render_region_to_rgba(small_region, &options)));
    });

    // 10x10 character region
    let medium_region = Rectangle::from_coords(0, 0, font_size.width * 10, font_size.height * 10);
    group.throughput(Throughput::Bytes((font_size.width * 10 * font_size.height * 10 * 4) as u64));
    group.bench_function("text_10x10_chars", |b| {
        b.iter(|| black_box(text_screen.render_region_to_rgba(medium_region, &options)));
    });

    // Palette screen small regions
    let palette_screen = create_palette_screen();

    // 32x32 pixel region
    let small_px = Rectangle::from_coords(0, 0, 32, 32);
    group.throughput(Throughput::Bytes((32 * 32 * 4) as u64));
    group.bench_function("palette_32x32", |b| {
        b.iter(|| black_box(palette_screen.render_region_to_rgba(small_px, &options)));
    });

    // 64x64 pixel region
    let medium_px = Rectangle::from_coords(0, 0, 64, 64);
    group.throughput(Throughput::Bytes((64 * 64 * 4) as u64));
    group.bench_function("palette_64x64", |b| {
        b.iter(|| black_box(palette_screen.render_region_to_rgba(medium_px, &options)));
    });

    // 128x128 pixel region
    let large_px = Rectangle::from_coords(0, 0, 128, 128);
    group.throughput(Throughput::Bytes((128 * 128 * 4) as u64));
    group.bench_function("palette_128x128", |b| {
        b.iter(|| black_box(palette_screen.render_region_to_rgba(large_px, &options)));
    });

    group.finish();
}

fn bench_scrolling_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("scrolling");

    let options = RenderOptions::default();

    // Simulate scrolling by rendering different offset regions
    let text_screen = create_text_screen(80, 50, false);
    let resolution = text_screen.resolution();
    let viewport_h = resolution.height / 2; // Half-screen viewport

    // Scroll positions
    let scroll_positions = [(0, "top"), (resolution.height / 4, "quarter"), (resolution.height / 2, "middle")];

    for (scroll_y, name) in scroll_positions {
        if scroll_y + viewport_h <= resolution.height {
            let region = Rectangle::from_coords(0, scroll_y, resolution.width, scroll_y + viewport_h);
            let pixels = (resolution.width * viewport_h * 4) as u64;
            group.throughput(Throughput::Bytes(pixels));
            group.bench_with_input(BenchmarkId::new("text_scroll", name), &region, |b, r| {
                b.iter(|| black_box(text_screen.render_region_to_rgba(*r, &options)));
            });
        }
    }

    // ScrollbackBuffer scrolling (main use case)
    let scrollback = create_scrollback_buffer(50, 640, 400);
    let sb_resolution = scrollback.resolution();
    let sb_viewport_h = sb_resolution.height / 2;

    for (scroll_y, name) in scroll_positions {
        if scroll_y + sb_viewport_h <= sb_resolution.height {
            let region = Rectangle::from_coords(0, scroll_y, sb_resolution.width, scroll_y + sb_viewport_h);
            let pixels = (sb_resolution.width * sb_viewport_h * 4) as u64;
            group.throughput(Throughput::Bytes(pixels));
            group.bench_with_input(BenchmarkId::new("scrollback_scroll", name), &region, |b, r| {
                b.iter(|| black_box(scrollback.render_region_to_rgba(*r, &options)));
            });
        }
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_text_screen,
    bench_palette_screen,
    bench_scrollback_buffer,
    bench_scanlines_comparison,
    bench_small_regions,
    bench_scrolling_simulation,
);

criterion_main!(benches);
