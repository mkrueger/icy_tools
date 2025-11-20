use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use icy_parser_core::{CommandParser, CommandSink, TerminalCommand, VT52Mode, Vt52Parser};
use std::fs;
use std::hint::black_box;
use std::path::Path;

struct NullSink;
impl CommandSink for NullSink {
    #[inline]
    fn print(&mut self, _text: &[u8]) { /* discard */
    }

    #[inline]
    fn emit(&mut self, _cmd: TerminalCommand) { /* discard */
    }
}

fn load_vt52_files() -> Vec<u8> {
    let vt52_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/vt52_data");
    let mut combined = Vec::new();

    // Load all .vt52 files from the directory
    if let Ok(entries) = fs::read_dir(&vt52_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("vt52") {
                if let Ok(data) = fs::read(&path) {
                    combined.extend_from_slice(&data);
                } else {
                    eprintln!("Warning: Could not read {:?}", path);
                }
            }
        }
    }

    if combined.is_empty() {
        panic!("No VT52 files loaded from benches/vt52_data/");
    }

    combined
}

fn make_synthetic_inputs() -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    // 1. Pure text with minimal VT52 commands (realistic text output)
    let mut text_heavy = Vec::new();
    for i in 0..500 {
        text_heavy.extend_from_slice(format!("Line {} with some text content here\n", i).as_bytes());
    }
    // Add a few VT52 commands
    text_heavy.extend_from_slice(b"\x1BH"); // Cursor home

    // 2. Heavy cursor positioning (VT52 Y command)
    let mut cursor_heavy = Vec::new();
    for row in 0..24 {
        for col in 0..80 {
            // ESC Y row col (VT52 cursor position)
            cursor_heavy.push(0x1B);
            cursor_heavy.push(b'Y');
            cursor_heavy.push(32 + row); // Offset by 32
            cursor_heavy.push(32 + col);
            cursor_heavy.push(b'*'); // Draw a character
        }
    }

    // 3. Color-heavy (SGR attributes and inverse video)
    let mut color_heavy = Vec::new();
    for i in 0..200 {
        // ESC p (reverse video on)
        color_heavy.extend_from_slice(b"\x1Bp");
        color_heavy.extend_from_slice(format!("Text {}", i).as_bytes());
        // ESC q (reverse video off)
        color_heavy.extend_from_slice(b"\x1Bq");
        color_heavy.extend_from_slice(b"Normal text\n");
        // Various foreground colors
        color_heavy.extend_from_slice(format!("\x1B[3{}m", i % 8).as_bytes());
    }

    // 4. Mixed: text, cursor movements, colors
    let mut mixed = Vec::new();
    for i in 0..100 {
        mixed.extend_from_slice(format!("Text line {}\n", i).as_bytes());
        // Cursor position
        mixed.push(0x1B);
        mixed.push(b'Y');
        mixed.push(32 + (i % 24));
        mixed.push(32 + ((i * 3) % 80));
        // Color
        mixed.extend_from_slice(format!("\x1B[3{}m", i % 8).as_bytes());
        if i % 10 == 0 {
            mixed.extend_from_slice(b"\x1BH"); // Home
            mixed.extend_from_slice(b"\x1BJ"); // Clear to end of screen
        }
    }

    (text_heavy, cursor_heavy, color_heavy, mixed)
}

fn bench_vt52_parser(c: &mut Criterion) {
    let real_world_data = load_vt52_files();
    let (text_heavy, cursor_heavy, color_heavy, mixed) = make_synthetic_inputs();

    let mut group = c.benchmark_group("vt52_parser");

    // Benchmark combined real-world data
    group.throughput(Throughput::Bytes(real_world_data.len() as u64));
    group.bench_function("parse_real_world_combined", |b| {
        let mut parser = Vt52Parser::new(VT52Mode::Mixed);
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&real_world_data), &mut sink);
        });
    });

    // Synthetic benchmarks for specific patterns
    group.throughput(Throughput::Bytes(text_heavy.len() as u64));
    group.bench_function("parse_text_heavy", |b| {
        let mut parser = Vt52Parser::new(VT52Mode::Mixed);
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&text_heavy), &mut sink);
        });
    });

    group.throughput(Throughput::Bytes(cursor_heavy.len() as u64));
    group.bench_function("parse_cursor_heavy", |b| {
        let mut parser = Vt52Parser::new(VT52Mode::Mixed);
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&cursor_heavy), &mut sink);
        });
    });

    group.throughput(Throughput::Bytes(color_heavy.len() as u64));
    group.bench_function("parse_color_heavy", |b| {
        let mut parser = Vt52Parser::new(VT52Mode::Mixed);
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&color_heavy), &mut sink);
        });
    });

    group.throughput(Throughput::Bytes(mixed.len() as u64));
    group.bench_function("parse_mixed", |b| {
        let mut parser = Vt52Parser::new(VT52Mode::Mixed);
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&mixed), &mut sink);
        });
    });

    // Test parser reuse vs new instantiation on real-world data
    group.throughput(Throughput::Bytes(real_world_data.len() as u64));
    group.bench_function("parse_real_world_new_each_time", |b| {
        let mut sink = NullSink;
        b.iter(|| {
            let mut parser = Vt52Parser::new(VT52Mode::Mixed);
            parser.parse(black_box(&real_world_data), &mut sink);
        });
    });

    group.finish();
}

criterion_group!(name=vt52; config=Criterion::default().with_plots(); targets=bench_vt52_parser);
criterion_main!(vt52);
