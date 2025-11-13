use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use icy_parser_core::{AnsiParser, CommandParser, CommandSink, TerminalCommand};
use std::fs;
use std::path::Path;

struct NullSink;
impl CommandSink for NullSink {
    #[inline]
    fn emit(&mut self, _cmd: TerminalCommand<'_>) { /* discard */
    }
}

fn load_ansi_files() -> Vec<u8> {
    let ansi_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/ansi_data");
    let mut combined = Vec::new();

    let files = [
        "APAM-EXOTICAADD.ANS",
        "Members01.ans",
        "NAUWH-VN.ANS",
        "anst-rorschach.ans",
        "fuel25-mem.ans",
        "k1-bombq.ans",
    ];

    for filename in &files {
        let path = ansi_dir.join(filename);
        if let Ok(data) = fs::read(&path) {
            combined.extend_from_slice(&data);
        } else {
            eprintln!("Warning: Could not read {}", filename);
        }
    }

    if combined.is_empty() {
        panic!("No ANSI files loaded from benches/ansi_data/");
    }

    combined
}

fn make_synthetic_inputs() -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    // 1. Pure text with minimal ANSI (realistic terminal output)
    let mut text_heavy = Vec::new();
    for i in 0..1000 {
        text_heavy.extend_from_slice(b"\x1B[32mLine ");
        text_heavy.extend_from_slice(i.to_string().as_bytes());
        text_heavy.extend_from_slice(b": Some text content here\x1B[0m\n");
    }

    // 2. CSI sequence heavy (lots of cursor movements)
    let mut csi_heavy = Vec::new();
    for y in 0..100 {
        for x in 0..80 {
            csi_heavy.extend_from_slice(format!("\x1B[{};{}H*", y, x).as_bytes());
        }
    }

    // 3. SGR color-heavy (typical colorized output)
    let mut color_heavy = Vec::new();
    for _ in 0..1000 {
        color_heavy.extend_from_slice(b"\x1B[31mRed\x1B[0m \x1B[32mGreen\x1B[0m \x1B[34mBlue\x1B[0m ");
        color_heavy.extend_from_slice(b"\x1B[1;33mBold Yellow\x1B[0m ");
        color_heavy.extend_from_slice(b"\x1B[38;5;208mOrange\x1B[0m\n");
    }

    // 4. Mixed content (text, controls, CSI, OSC)
    let mut mixed = Vec::new();
    for i in 0..500 {
        mixed.extend_from_slice(b"\x1B]0;Window Title\x07");
        mixed.extend_from_slice(format!("\x1B[{};1H", i % 24 + 1).as_bytes());
        mixed.extend_from_slice(b"\x1B[2KClearing line and writing text\n");
        mixed.extend_from_slice(b"Normal text with \x08backspace\t and tab\r\n");
        mixed.extend_from_slice(b"\x1B[1;32mColored text\x1B[0m");
    }

    (text_heavy, csi_heavy, color_heavy, mixed)
}

fn bench_ansi_parser(c: &mut Criterion) {
    // Load real-world ANSI files
    let real_world_data = load_ansi_files();

    // Generate synthetic test patterns
    let (text_heavy, csi_heavy, color_heavy, mixed) = make_synthetic_inputs();
    let mut group = c.benchmark_group("ansi_parser");

    // Benchmark real-world ANSI art files
    group.throughput(Throughput::Bytes(real_world_data.len() as u64));
    group.bench_function("parse_real_world_ansi_files", |b| {
        let mut parser = AnsiParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&real_world_data), &mut sink);
        });
    });

    // Synthetic benchmarks for specific patterns
    group.throughput(Throughput::Bytes(text_heavy.len() as u64));
    group.bench_function("parse_text_heavy", |b| {
        let mut parser = AnsiParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&text_heavy), &mut sink);
        });
    });

    group.throughput(Throughput::Bytes(csi_heavy.len() as u64));
    group.bench_function("parse_csi_heavy", |b| {
        let mut parser = AnsiParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&csi_heavy), &mut sink);
        });
    });

    group.throughput(Throughput::Bytes(color_heavy.len() as u64));
    group.bench_function("parse_color_heavy", |b| {
        let mut parser = AnsiParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&color_heavy), &mut sink);
        });
    });

    group.throughput(Throughput::Bytes(mixed.len() as u64));
    group.bench_function("parse_mixed", |b| {
        let mut parser = AnsiParser::new();
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
            let mut parser = AnsiParser::new();
            parser.parse(black_box(&real_world_data), &mut sink);
        });
    });

    group.finish();
}

criterion_group!(name=ansi; config=Criterion::default().with_plots(); targets=bench_ansi_parser);
criterion_main!(ansi);
