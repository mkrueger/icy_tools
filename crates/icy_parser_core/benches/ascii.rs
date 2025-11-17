use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use icy_parser_core::{AsciiParser, CommandParser, CommandSink, TerminalCommand};
use std::hint::black_box;

struct NullSink;
impl CommandSink for NullSink {
    #[inline]
    fn print(&mut self, _text: &[u8]) { /* discard */
    }

    #[inline]
    fn emit(&mut self, _cmd: TerminalCommand) { /* discard */
    }
}

fn make_inputs() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    // 1. Pure ASCII with newlines
    let ascii = "Hello World\n".repeat(10_000).into_bytes();
    // 2. Mixed UTF-8 multi-byte sequences
    let utf_mixed_pattern = "Aäα😀"; // 1 + 2 + 2 + 4 bytes
    let utf_mixed = utf_mixed_pattern.repeat(5_000).into_bytes();
    // 3. Control heavy data
    let control_pattern = b"\x07\x08\t\n\x0C\rHello"; // bell, backspace, tab, lf, formfeed, cr then text
    let mut control_heavy = Vec::with_capacity(control_pattern.len() * 8_000);
    for _ in 0..8_000 {
        control_heavy.extend_from_slice(control_pattern);
    }
    (ascii, utf_mixed, control_heavy)
}

fn bench_ascii_parser(c: &mut Criterion) {
    let (ascii, utf_mixed, control_heavy) = make_inputs();
    let mut group = c.benchmark_group("ascii_parser");

    group.throughput(Throughput::Bytes(ascii.len() as u64));
    group.bench_function("parse_ascii_run_reuse", |b| {
        let mut parser = AsciiParser::new();
        let mut sink = NullSink;
        b.iter(|| {
            parser.parse(black_box(&ascii), &mut sink);
        });
    });

    group.throughput(Throughput::Bytes(ascii.len() as u64));
    group.bench_function("parse_ascii_run_new_each_time", |b| {
        let mut sink = NullSink;
        b.iter(|| {
            let mut p = AsciiParser::new();
            p.parse(black_box(&ascii), &mut sink);
        });
    });

    group.throughput(Throughput::Bytes(utf_mixed.len() as u64));
    group.bench_function("parse_utf_mixed", |b| {
        let mut parser = AsciiParser::new();
        let mut sink = NullSink;
        b.iter(|| parser.parse(black_box(&utf_mixed), &mut sink));
    });

    group.throughput(Throughput::Bytes(control_heavy.len() as u64));
    group.bench_function("parse_control_heavy", |b| {
        let mut parser = AsciiParser::new();
        let mut sink = NullSink;
        b.iter(|| parser.parse(black_box(&control_heavy), &mut sink));
    });

    group.finish();
}

criterion_group!(name=ascii; config=Criterion::default().with_plots(); targets=bench_ascii_parser);
criterion_main!(ascii);
