use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use icy_parser_core::{CommandParser, CommandSink, RenegadeParser, TerminalCommand};
use std::fs;
use std::path::Path;

struct NullSink;
impl CommandSink for NullSink {
    #[inline]
    fn emit(&mut self, _cmd: TerminalCommand<'_>) { /* discard */
    }
}

fn load_renegade_file() -> Vec<u8> {
    let renegade_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/data/Members01.an1");

    fs::read(&renegade_file).unwrap_or_else(|e| {
        panic!("Could not read Members01.an1: {}", e);
    })
}

fn make_synthetic_inputs() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    // 1. Text heavy with minimal Renegade commands
    let mut text_heavy = Vec::new();
    for i in 0..1000 {
        text_heavy.extend_from_slice(b"|15"); // White foreground
        text_heavy.extend_from_slice(format!("Line {}: Some text content here\r\n", i).as_bytes());
    }

    // 2. Renegade command heavy (lots of color changes)
    let mut command_heavy = Vec::new();
    for _ in 0..1000 {
        // Cycle through all 16 foreground colors
        for fg in 0..16 {
            command_heavy.extend_from_slice(format!("|{:02}*", fg).as_bytes());
        }
        // Cycle through all 8 background colors
        for bg in 16..24 {
            command_heavy.extend_from_slice(format!("|{:02}#", bg).as_bytes());
        }
    }

    // 3. Mixed content with various color combinations
    let mut mixed = Vec::new();
    for i in 0..500 {
        // White on blue
        mixed.extend_from_slice(b"|15|17");
        mixed.extend_from_slice(format!("Renegade Test {}\r\n", i).as_bytes());
        // Bright yellow on black
        mixed.extend_from_slice(b"|14|16");
        mixed.extend_from_slice(b"Warning message\r\n");
        // Bright white on red
        mixed.extend_from_slice(b"|15|20");
        mixed.extend_from_slice(b"Error: Something went wrong\r\n");
        // Green on black
        mixed.extend_from_slice(b"|10|16");
        mixed.extend_from_slice(b"Success!\r\n");
        // Literal pipe (invalid code)
        mixed.extend_from_slice(b"|Hello world\r\n");
        // Incomplete sequence
        mixed.extend_from_slice(b"|5X\r\n");
    }

    (text_heavy, command_heavy, mixed)
}

fn bench_renegade_parser(c: &mut Criterion) {
    let real_world = load_renegade_file();
    let (text_heavy, command_heavy, mixed) = make_synthetic_inputs();
    let mut group = c.benchmark_group("renegade_parser");

    // Real-world file benchmark
    group.throughput(Throughput::Bytes(real_world.len() as u64));
    group.bench_function("parse_real_world_reuse", |b| {
        let mut parser = RenegadeParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&real_world), &mut sink);
        });
    });

    group.throughput(Throughput::Bytes(real_world.len() as u64));
    group.bench_function("parse_real_world_new_each_time", |b| {
        let mut sink = NullSink;
        b.iter(|| {
            let mut p = RenegadeParser::new();
            p.parse(black_box(&real_world), &mut sink);
        });
    });

    // Text heavy benchmark
    group.throughput(Throughput::Bytes(text_heavy.len() as u64));
    group.bench_function("parse_text_heavy", |b| {
        let mut parser = RenegadeParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&text_heavy), &mut sink);
        });
    });

    // Command heavy benchmark
    group.throughput(Throughput::Bytes(command_heavy.len() as u64));
    group.bench_function("parse_command_heavy", |b| {
        let mut parser = RenegadeParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&command_heavy), &mut sink);
        });
    });

    // Mixed content benchmark
    group.throughput(Throughput::Bytes(mixed.len() as u64));
    group.bench_function("parse_mixed", |b| {
        let mut parser = RenegadeParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&mixed), &mut sink);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_renegade_parser);
criterion_main!(benches);
