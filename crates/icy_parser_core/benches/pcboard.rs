use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use icy_parser_core::{CommandParser, CommandSink, PcBoardParser, TerminalCommand};
use std::fs;
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

fn load_pcboard_file() -> Vec<u8> {
    let pcboard_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/data/Members01.pcb");

    fs::read(&pcboard_file).unwrap_or_else(|e| {
        panic!("Could not read Members01.pcb: {}", e);
    })
}

fn make_synthetic_inputs() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    // 1. Text heavy with minimal PCBoard commands
    let mut text_heavy = Vec::new();
    for i in 0..1000 {
        text_heavy.extend_from_slice(b"@X0F"); // White on black
        text_heavy.extend_from_slice(format!("Line {}: Some text content here\r\n", i).as_bytes());
    }

    // 2. PCBoard command heavy (lots of color changes)
    let mut command_heavy = Vec::new();
    for _ in 0..1000 {
        // Cycle through different color combinations
        for fg in 0..16 {
            for bg in 0..8 {
                let color = (bg << 4) | fg;
                command_heavy.extend_from_slice(format!("@X{:02X}*", color).as_bytes());
            }
        }
    }

    // 3. Mixed content with macros and escapes
    let mut mixed = Vec::new();
    for i in 0..500 {
        // Color code
        mixed.extend_from_slice(b"@X1F"); // White on blue
        mixed.extend_from_slice(format!("PCBoard Test {}\r\n", i).as_bytes());
        // Escaped @
        mixed.extend_from_slice(b"Email: user@@example.com\r\n");
        // Macro (ignored but parsed)
        mixed.extend_from_slice(b"@CLS@Screen cleared\r\n");
        // More color changes
        mixed.extend_from_slice(b"@X0E"); // Yellow on black
        mixed.extend_from_slice(b"Warning message\r\n");
        // Lowercase x variant
        mixed.extend_from_slice(b"@x07Normal text\r\n");
    }

    (text_heavy, command_heavy, mixed)
}

fn bench_pcboard_parser(c: &mut Criterion) {
    let real_world = load_pcboard_file();
    let (text_heavy, command_heavy, mixed) = make_synthetic_inputs();
    let mut group = c.benchmark_group("pcboard_parser");

    // Real-world file benchmark
    group.throughput(Throughput::Bytes(real_world.len() as u64));
    group.bench_function("parse_real_world_reuse", |b| {
        let mut parser = PcBoardParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&real_world), &mut sink);
        });
    });

    group.throughput(Throughput::Bytes(real_world.len() as u64));
    group.bench_function("parse_real_world_new_each_time", |b| {
        let mut sink = NullSink;
        b.iter(|| {
            let mut p = PcBoardParser::new();
            p.parse(black_box(&real_world), &mut sink);
        });
    });

    // Text heavy benchmark
    group.throughput(Throughput::Bytes(text_heavy.len() as u64));
    group.bench_function("parse_text_heavy", |b| {
        let mut parser = PcBoardParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&text_heavy), &mut sink);
        });
    });

    // Command heavy benchmark
    group.throughput(Throughput::Bytes(command_heavy.len() as u64));
    group.bench_function("parse_command_heavy", |b| {
        let mut parser = PcBoardParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&command_heavy), &mut sink);
        });
    });

    // Mixed content benchmark
    group.throughput(Throughput::Bytes(mixed.len() as u64));
    group.bench_function("parse_mixed", |b| {
        let mut parser = PcBoardParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&mixed), &mut sink);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_pcboard_parser);
criterion_main!(benches);
