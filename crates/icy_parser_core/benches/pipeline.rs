//! Observable parser-only baseline. Filter with `pipeline_counting/ansi/64`, etc.
//! The checksum is intentionally part of the measured sink cost, not a null sink.
use std::{hint::black_box, time::Duration};

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use icy_parser_core::{AnsiParser, AsciiParser, CommandParser, CommandSink, RipParser, TerminalCommand};

#[derive(Default)]
struct CountingSink {
    print_calls: usize,
    printed_bytes: usize,
    commands: usize,
    checksum: u64,
}

impl CommandSink for CountingSink {
    fn print(&mut self, text: &[u8]) {
        self.print_calls += 1;
        self.printed_bytes += text.len();
        // Independent of print-call boundaries, unlike a per-call checksum.
        for &byte in text {
            self.checksum = self.checksum.rotate_left(5) ^ u64::from(byte);
        }
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        self.commands += 1;
        black_box(cmd);
    }
}

fn bench_parser<P: CommandParser>(c: &mut Criterion, name: &str, input: &[u8], new_parser: impl Fn() -> P) {
    let mut group = c.benchmark_group("pipeline_counting");
    group.throughput(Throughput::Bytes(input.len() as u64));

    let mut reference = CountingSink::default();
    new_parser().parse(input, &mut reference);
    assert!(reference.printed_bytes > 0);
    for (label, chunk_size) in [("1", 1), ("64", 64), ("4096", 4096), ("whole", input.len())] {
        // Untimed guard against dropped data or accidentally resetting the parser
        // at chunk boundaries. Print-call counts may legitimately differ.
        let mut parser = new_parser();
        let mut observed = CountingSink::default();
        for chunk in input.chunks(chunk_size) {
            parser.parse(chunk, &mut observed);
        }
        assert_eq!(
            (observed.printed_bytes, observed.commands, observed.checksum),
            (reference.printed_bytes, reference.commands, reference.checksum),
            "{name}/{label}"
        );

        group.bench_function(BenchmarkId::new(name, label), |b| {
            b.iter_batched_ref(
                || (new_parser(), CountingSink::default()),
                |(parser, sink)| {
                    for chunk in black_box(input).chunks(chunk_size) {
                        parser.parse(chunk, sink);
                    }
                    black_box((sink.print_calls, sink.printed_bytes, sink.commands, sink.checksum));
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn pipeline(c: &mut Criterion) {
    // Keep these deterministic fixtures identical to the GUI pipeline benchmark.
    let plain = b"The quick brown fox jumps over the lazy dog. 0123456789 ABCDEF\r\n".repeat(128);
    let ansi = b"\x1b[32mThe quick brown fox\x1b[0m jumps over \x1b[1;34mthe lazy dog\x1b[0m. 0123456789 ABCDEF\r\n".repeat(128);
    bench_parser(c, "ascii", &plain, AsciiParser::new);
    bench_parser(c, "ansi", &ansi, AnsiParser::new);
    // No graphics commands: isolates the RIP parser's ANSI/plain-text fallback.
    bench_parser(c, "rip_plain", &plain, RipParser::new);
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10)
        .warm_up_time(Duration::from_millis(500)).measurement_time(Duration::from_secs(1));
    targets = pipeline
}
criterion_main!(benches);
