use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use icy_parser_core::{CommandParser, CommandSink, IgsParser, TerminalCommand};
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

fn load_igs_files() -> Vec<u8> {
    let igs_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/igs_data");
    let mut combined = Vec::new();

    // Load all .IG files from the directory
    if let Ok(entries) = fs::read_dir(&igs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("IG") {
                if let Ok(data) = fs::read(&path) {
                    combined.extend_from_slice(&data);
                } else {
                    eprintln!("Warning: Could not read {:?}", path);
                }
            }
        }
    }

    if combined.is_empty() {
        panic!("No IGS files loaded from benches/igs_data/");
    }

    combined
}

fn make_synthetic_inputs() -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    // 1. Pure text with minimal IGS commands (realistic text output)
    let mut text_heavy = Vec::new();
    for i in 0..500 {
        text_heavy.extend_from_slice(format!("Line {} with some text content here\n", i).as_bytes());
    }
    // Add a few IGS commands
    text_heavy.extend_from_slice(b"\x1b[0;37m");

    // 2. Heavy IGS graphics commands (circles, lines, fills)
    let mut graphics_heavy = Vec::new();
    // Multiple circles with different parameters
    for i in 0..100 {
        graphics_heavy.extend_from_slice(format!("\x1b[C{},{},20,{}\r", 100 + i * 5, 100, i % 16).as_bytes());
    }
    // Lines
    for i in 0..100 {
        graphics_heavy.extend_from_slice(format!("\x1b[L{},{},{},{},{}\r", i * 3, 50, i * 3 + 50, 150, i % 16).as_bytes());
    }
    // Filled rectangles
    for i in 0..50 {
        graphics_heavy.extend_from_slice(format!("\x1b[R{},{},{},{},{}\r", i * 10, i * 5, 50, 40, i % 16).as_bytes());
    }

    // 3. Color-heavy (palette changes, pen/fill colors)
    let mut color_heavy = Vec::new();
    for i in 0..200 {
        // Set pen color
        color_heavy.extend_from_slice(format!("\x1b[P{}\r", i % 16).as_bytes());
        // Set fill color
        color_heavy.extend_from_slice(format!("\x1b[F{}\r", (i + 8) % 16).as_bytes());
        // Draw something
        color_heavy.extend_from_slice(format!("\x1b[R{},{},20,20,{}\r", (i % 20) * 30, (i / 20) * 30, i % 16).as_bytes());
    }

    // 4. Mixed: text, graphics, colors
    let mut mixed = Vec::new();
    for i in 0..100 {
        mixed.extend_from_slice(format!("Text line {}\n", i).as_bytes());
        mixed.extend_from_slice(format!("\x1b[P{}\r", i % 16).as_bytes());
        mixed.extend_from_slice(format!("\x1b[C{},{},15,{}\r", 50 + i * 2, 50 + i, i % 16).as_bytes());
        if i % 10 == 0 {
            mixed.extend_from_slice(b"\x1b[0;1;37m");
        }
    }

    (text_heavy, graphics_heavy, color_heavy, mixed)
}

fn bench_igs_parser(c: &mut Criterion) {
    let real_world_data = load_igs_files();
    let (text_heavy, graphics_heavy, color_heavy, mixed) = make_synthetic_inputs();

    let mut group = c.benchmark_group("igs_parser");

    // Benchmark combined real-world data
    group.throughput(Throughput::Bytes(real_world_data.len() as u64));
    group.bench_function("parse_real_world_combined", |b| {
        let mut parser = IgsParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&real_world_data), &mut sink);
        });
    });

    // Synthetic benchmarks for specific patterns
    group.throughput(Throughput::Bytes(text_heavy.len() as u64));
    group.bench_function("parse_text_heavy", |b| {
        let mut parser = IgsParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&text_heavy), &mut sink);
        });
    });

    group.throughput(Throughput::Bytes(graphics_heavy.len() as u64));
    group.bench_function("parse_graphics_heavy", |b| {
        let mut parser = IgsParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&graphics_heavy), &mut sink);
        });
    });

    group.throughput(Throughput::Bytes(color_heavy.len() as u64));
    group.bench_function("parse_color_heavy", |b| {
        let mut parser = IgsParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&color_heavy), &mut sink);
        });
    });

    group.throughput(Throughput::Bytes(mixed.len() as u64));
    group.bench_function("parse_mixed", |b| {
        let mut parser = IgsParser::new();
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
            let mut parser = IgsParser::new();
            parser.parse(black_box(&real_world_data), &mut sink);
        });
    });

    group.finish();
}

criterion_group!(name=igs; config=Criterion::default().with_plots(); targets=bench_igs_parser);
criterion_main!(igs);
