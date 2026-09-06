//! Production sinks, without a GUI/window/GPU. Named filters:
//! `pipeline_queue`, `pipeline_screen`, `pipeline_queued_screen`, `no_screen`.
//! Fixtures match icy_parser_core's pipeline benchmark. Throughput is input bytes.
//! Queue statistics traversal is timed; batched setup and teardown are not.
//! Queued-screen execution includes draining and freeing the queued events.
use std::{collections::VecDeque, hint::black_box, time::Duration};

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use icy_engine::{ScreenSink, Size, TextScreen};
use icy_engine_gui::util::{QueuedCommand, QueueingSink};
use icy_parser_core::{AnsiParser, AsciiParser, CommandParser, CommandSink, RipParser};

#[derive(Debug, Default)]
struct QueueStats {
    events: usize,
    print_events: usize,
    printed_bytes: usize,
}

fn queue_stats(queue: &VecDeque<QueuedCommand>) -> QueueStats {
    let mut stats = QueueStats {
        events: queue.len(),
        ..QueueStats::default()
    };
    for event in queue {
        if let QueuedCommand::Print(bytes) = event {
            stats.print_events += 1;
            stats.printed_bytes += bytes.len();
        }
    }
    stats
}

fn observe_queue(queue: &VecDeque<QueuedCommand>) {
    let stats = queue_stats(queue);
    black_box((stats.events, stats.print_events, stats.printed_bytes));
    // Make payloads observable too, without adding a byte-wise checksum to timing.
    black_box(queue);
}

fn new_screen() -> TextScreen {
    // 128 short lines fit without scrollback rendering, growth, or UI state.
    // Fresh for every iteration; this measures text execution, not scrollback.
    TextScreen::new(Size::new(80, 160))
}

fn parse_chunks(parser: &mut impl CommandParser, input: &[u8], chunk_size: usize, sink: &mut dyn CommandSink) {
    for chunk in black_box(input).chunks(chunk_size) {
        parser.parse(chunk, sink);
    }
}

fn bench_parser<P: CommandParser>(c: &mut Criterion, name: &str, input: &[u8], new_parser: impl Fn() -> P) {
    for stage in ["pipeline_queue", "pipeline_screen", "pipeline_queued_screen"] {
        let mut group = c.benchmark_group(stage);
        group.throughput(Throughput::Bytes(input.len() as u64));
        for (label, chunk_size) in [("1", 1), ("64", 64), ("4096", 4096), ("whole", input.len())] {
            group.bench_function(BenchmarkId::new(name, label), |b| match stage {
                "pipeline_queue" => b.iter_batched_ref(
                    || (new_parser(), VecDeque::new()),
                    |(parser, queue)| {
                        parse_chunks(parser, input, chunk_size, &mut QueueingSink::new(queue));
                        observe_queue(queue);
                    },
                    BatchSize::SmallInput,
                ),
                "pipeline_screen" => b.iter_batched_ref(
                    || (new_parser(), new_screen()),
                    |(parser, screen)| {
                        parse_chunks(parser, input, chunk_size, &mut ScreenSink::new(screen));
                        black_box(&*screen);
                    },
                    // Bound peak live screen memory even for fast iterations.
                    BatchSize::PerIteration,
                ),
                "pipeline_queued_screen" => b.iter_batched_ref(
                    || (new_parser(), VecDeque::new(), new_screen()),
                    |(parser, queue, screen)| {
                        parse_chunks(parser, input, chunk_size, &mut QueueingSink::new(queue));
                        observe_queue(queue);
                        let mut sink = ScreenSink::new(screen);
                        while let Some(command) = queue.pop_front() {
                            black_box(command.process_screen_command(&mut sink));
                        }
                        black_box(&*screen);
                    },
                    BatchSize::PerIteration,
                ),
                _ => unreachable!(),
            });
        }
        group.finish();
    }
}

fn parser_pipeline(c: &mut Criterion) {
    let plain = b"The quick brown fox jumps over the lazy dog. 0123456789 ABCDEF\r\n".repeat(128);
    let ansi = b"\x1b[32mThe quick brown fox\x1b[0m jumps over \x1b[1;34mthe lazy dog\x1b[0m. 0123456789 ABCDEF\r\n".repeat(128);
    bench_parser(c, "ascii", &plain, AsciiParser::new);
    bench_parser(c, "ansi", &ansi, AnsiParser::new);
    bench_parser(c, "rip_plain", &plain, RipParser::new);
}

fn tiny_prints(c: &mut Criterion) {
    // Same bytes, different call granularity. No parser or screen to obscure
    // allocations/event coalescing in the actual production QueueingSink.
    let input = b"0123456789ABCDEF".repeat(512);
    let mut group = c.benchmark_group("no_screen");
    group.throughput(Throughput::Bytes(input.len() as u64));
    for (label, chunk_size) in [("1", 1), ("64", 64), ("4096", 4096), ("whole", input.len())] {
        // Also report actual event counts outside timing, making batching changes
        // visible alongside Criterion throughput. Do not assert a coalescing policy.
        let mut queue = VecDeque::new();
        for chunk in input.chunks(chunk_size) {
            QueueingSink::new(&mut queue).print(chunk);
        }
        let stats = queue_stats(&queue);
        assert_eq!(stats.printed_bytes, input.len());
        eprintln!("no_screen/print_calls/{label}: {stats:?}");

        group.bench_function(BenchmarkId::new("print_calls", label), |b| {
            b.iter_batched_ref(
                VecDeque::new,
                |queue| {
                    let mut sink = QueueingSink::new(queue);
                    for chunk in black_box(input.as_slice()).chunks(chunk_size) {
                        sink.print(chunk);
                    }
                    observe_queue(queue);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10)
        .warm_up_time(Duration::from_millis(500)).measurement_time(Duration::from_secs(1));
    targets = parser_pipeline, tiny_prints
}
criterion_main!(benches);
