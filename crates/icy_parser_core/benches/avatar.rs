use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use icy_parser_core::{AvatarParser, CommandParser, CommandSink, TerminalCommand};
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

fn load_avatar_file() -> Vec<u8> {
    let avatar_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/data/Members01.avt");

    fs::read(&avatar_file).unwrap_or_else(|e| {
        panic!("Could not read Members01.avt: {}", e);
    })
}

fn make_synthetic_inputs() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    // 1. Text heavy with minimal Avatar commands
    let mut text_heavy = Vec::new();
    for i in 0..1000 {
        text_heavy.extend_from_slice(b"\x16\x01\x1F"); // Set color to white on blue
        text_heavy.extend_from_slice(format!("Line {}: Some text content here\r\n", i).as_bytes());
    }

    // 2. Avatar command heavy (lots of color changes and cursor movements)
    let mut command_heavy = Vec::new();
    for y in 0..25 {
        for x in 0..80 {
            // Goto XY
            command_heavy.push(0x16); // AVT_CMD
            command_heavy.push(0x08); // Goto XY command
            command_heavy.push(y);
            command_heavy.push(x);
            // Set color
            command_heavy.push(0x16); // AVT_CMD
            command_heavy.push(0x01); // Set color
            command_heavy.push((y * x) as u8 & 0xFF);
            command_heavy.push(b'*');
        }
    }

    // 3. Mixed content with repetition
    let mut mixed = Vec::new();
    for _ in 0..500 {
        // Clear screen
        mixed.push(0x0C);
        // Goto position
        mixed.extend_from_slice(b"\x16\x08\x05\x0A"); // Row 5, Col 10
        // Set color
        mixed.extend_from_slice(b"\x16\x01\x1E"); // Yellow on blue
        mixed.extend_from_slice(b"Avatar Test ");
        // Repeat character
        mixed.extend_from_slice(b"\x19=\x14"); // Repeat '=' 20 times
        mixed.extend_from_slice(b"\r\n");
        // Blink on
        mixed.extend_from_slice(b"\x16\x02");
        mixed.extend_from_slice(b"Blinking text\r\n");
        // ANSI escape in Avatar
        mixed.extend_from_slice(b"\x1B[2J"); // ANSI clear screen
    }

    (text_heavy, command_heavy, mixed)
}

fn bench_avatar_parser(c: &mut Criterion) {
    let real_world = load_avatar_file();
    let (text_heavy, command_heavy, mixed) = make_synthetic_inputs();
    let mut group = c.benchmark_group("avatar_parser");

    // Real-world file benchmark
    group.throughput(Throughput::Bytes(real_world.len() as u64));
    group.bench_function("parse_real_world_reuse", |b| {
        let mut parser = AvatarParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&real_world), &mut sink);
        });
    });

    group.throughput(Throughput::Bytes(real_world.len() as u64));
    group.bench_function("parse_real_world_new_each_time", |b| {
        let mut sink = NullSink;
        b.iter(|| {
            let mut p = AvatarParser::new();
            p.parse(black_box(&real_world), &mut sink);
        });
    });

    // Text heavy benchmark
    group.throughput(Throughput::Bytes(text_heavy.len() as u64));
    group.bench_function("parse_text_heavy", |b| {
        let mut parser = AvatarParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&text_heavy), &mut sink);
        });
    });

    // Command heavy benchmark
    group.throughput(Throughput::Bytes(command_heavy.len() as u64));
    group.bench_function("parse_command_heavy", |b| {
        let mut parser = AvatarParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&command_heavy), &mut sink);
        });
    });

    // Mixed content benchmark
    group.throughput(Throughput::Bytes(mixed.len() as u64));
    group.bench_function("parse_mixed", |b| {
        let mut parser = AvatarParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&mixed), &mut sink);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_avatar_parser);
criterion_main!(benches);
