use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use icy_parser_core::{CommandParser, CommandSink, CtrlAParser, TerminalCommand};
use std::fs;
use std::path::Path;

struct NullSink;
impl CommandSink for NullSink {
    #[inline]
    fn emit(&mut self, _cmd: TerminalCommand<'_>) { /* discard */
    }
}

fn load_ctrla_file() -> Vec<u8> {
    let ctrla_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/data/Members01.msg");

    fs::read(&ctrla_file).unwrap_or_else(|e| {
        panic!("Could not read Members01.msg: {}", e);
    })
}

fn make_synthetic_inputs() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    // 1. Text heavy with minimal CTRL-A commands
    let mut text_heavy = Vec::new();
    for i in 0..1000 {
        text_heavy.push(0x01); // CTRL-A
        text_heavy.push(b'W'); // White
        text_heavy.extend_from_slice(format!("Line {}: Some text content here\r\n", i).as_bytes());
    }

    // 2. CTRL-A command heavy (lots of color changes and attributes)
    let mut command_heavy = Vec::new();
    let fg_codes = b"KBGCRMYW"; // Foreground colors
    let bg_codes = b"04261537"; // Background colors
    for _ in 0..500 {
        // Cycle through foreground colors
        for &fg in fg_codes.iter() {
            command_heavy.push(0x01);
            command_heavy.push(fg);
            command_heavy.push(b'*');
        }
        // Cycle through background colors
        for &bg in bg_codes.iter() {
            command_heavy.push(0x01);
            command_heavy.push(bg);
            command_heavy.push(b'#');
        }
        // Bold on/off
        command_heavy.extend_from_slice(b"\x01H\x01RRed Bold\x01N");
    }

    // 3. Mixed content with cursor movement and attributes
    let mut mixed = Vec::new();
    for i in 0..500 {
        // Clear screen
        mixed.push(0x01);
        mixed.push(b'L');
        // Home cursor
        mixed.push(0x01);
        mixed.push(b'\'');
        // Set color
        mixed.push(0x01);
        mixed.push(b'Y'); // Yellow
        mixed.push(0x01);
        mixed.push(b'1'); // Blue background
        mixed.extend_from_slice(format!("CTRL-A Test {}\r\n", i).as_bytes());
        // Bold text
        mixed.extend_from_slice(b"\x01H\x01WBold White\x01N\r\n");
        // Cursor movement
        mixed.extend_from_slice(b"\x01<\x01]");
        // iCE colors mode
        mixed.extend_from_slice(b"\x01E\x01C\x01NHigh intensity BG\r\n");
        // Literal CTRL-A
        mixed.extend_from_slice(b"\x01AThis has a literal ^A character\r\n");
    }

    (text_heavy, command_heavy, mixed)
}

fn bench_ctrla_parser(c: &mut Criterion) {
    let real_world = load_ctrla_file();
    let (text_heavy, command_heavy, mixed) = make_synthetic_inputs();
    let mut group = c.benchmark_group("ctrla_parser");

    // Real-world file benchmark
    group.throughput(Throughput::Bytes(real_world.len() as u64));
    group.bench_function("parse_real_world_reuse", |b| {
        let mut parser = CtrlAParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&real_world), &mut sink);
        });
    });

    group.throughput(Throughput::Bytes(real_world.len() as u64));
    group.bench_function("parse_real_world_new_each_time", |b| {
        let mut sink = NullSink;
        b.iter(|| {
            let mut p = CtrlAParser::new();
            p.parse(black_box(&real_world), &mut sink);
        });
    });

    // Text heavy benchmark
    group.throughput(Throughput::Bytes(text_heavy.len() as u64));
    group.bench_function("parse_text_heavy", |b| {
        let mut parser = CtrlAParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&text_heavy), &mut sink);
        });
    });

    // Command heavy benchmark
    group.throughput(Throughput::Bytes(command_heavy.len() as u64));
    group.bench_function("parse_command_heavy", |b| {
        let mut parser = CtrlAParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&command_heavy), &mut sink);
        });
    });

    // Mixed content benchmark
    group.throughput(Throughput::Bytes(mixed.len() as u64));
    group.bench_function("parse_mixed", |b| {
        let mut parser = CtrlAParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&mixed), &mut sink);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_ctrla_parser);
criterion_main!(benches);
