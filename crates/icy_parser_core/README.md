# icy_parser_core

Streaming terminal/BBS parsers without buffer, GUI, or network dependencies.

Supported parsers: ASCII, ANSI, Avatar, PCBoard, Ctrl-A, Renegade, ATASCII,
PETSCII, Viewdata, Mode 7, RIPscrip, VT52, IGS, and SkyPix.

## Model and contracts

- `CommandParser::parse(&[u8], &mut dyn CommandSink)` consumes input synchronously.
    Keep one parser across chunks; an empty chunk is **not** EOF or a flush.
- `CommandSink` emits ordered, typed operations. Implementations may execute,
    record, or queue them. Parsing must not require immediate execution feedback.
- `print(&[u8])` borrows bytes for the duration of the call. These are not
    necessarily UTF-8, and a call can end inside a codepoint. The consumer owns
    decoding state. A queue must retain its own copy.
- Adjacent print calls may be coalesced. Commands, requests, and graphics events
    must not be crossed when merging text. Chunking must preserve normalized
    event meaning and order, not exact print-call boundaries.
- `TerminalCommand` contains small, `Copy` operations; variable-size graphics,
    OSC/DCS payloads, music, and requests use separate callbacks.
- `report_error(ParseError, ErrorLevel)` reports recoverable diagnostics.
    `print` and `emit` are required; optional callbacks default to no-op and must
    be implemented by consumers that support their respective features.

### Viewdata execution state

`ViewdataParser` retains only escape-sequence syntax state. It emits `WriteCell`,
`Advance`, and `ResetAttributes` operations instead of asking the sink whether
the cursor wrapped. `emit_view_data` returns `()`.

The executor stores `ViewdataState` alongside its screen and resolves graphics,
hold, and row-wrap behavior **when the command is executed**. In `icy_engine`,
this state lives in `TerminalState`, not in the short-lived `ScreenSink` adapter.
This makes immediate execution and `icy_engine_gui::QueueingSink` equivalent,
even when geometry or cursor position changes between parsing and execution.

**API migration:** custom sinks must change `emit_view_data` from `-> bool` to
`-> ()`. Recorders/queues should retain the new operations unchanged. Executors
must handle the new variants with persistent display state, rather than treating
`WriteCell` as already-mapped text. Existing low-level operations such as
`SetChar`, `MoveCaret`, and `DoubleHeight` remain available for execution helpers
and Mode 7.

`QueueingSink` coalesces neighboring text operations into blocks of at most
16 KiB. This limits the size of an individual text operation between screen-lock
budget checks; it is not a total queue-size or parser-work limit.

## Example

```rust
use icy_parser_core::{AsciiParser, CommandParser, CommandSink, ErrorLevel, ParseError, TerminalCommand};

struct PrintSink;
impl CommandSink for PrintSink {
    fn print(&mut self, text: &[u8]) {
        println!("PRINT {:?}", std::str::from_utf8(text).unwrap_or("<non-utf8>"));
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        println!("EVENT {:?}", cmd);
    }
    
    fn report_error(&mut self, error: ParseError, _level: ErrorLevel) {
        eprintln!("Parse error: {:?}", error);
    }
}

let mut parser = AsciiParser::new();
let mut sink = PrintSink;
parser.parse(b"Hello World\n", &mut sink);
```

## Testing

The streaming-contract tests record all sink callbacks and normalize adjacent
text. They compare whole input, every two-way split, bytewise input, deterministic
random partitions, and empty chunks for all parser families. The fixed/random
crash-smoke tests are useful but are not coverage-guided fuzzing.

```bash
cargo test -p icy_parser_core
cargo test -p icy_parser_core --test streaming_contract
cargo test -p icy_engine_gui --test parser_pipeline --test queue_batching
```

## Benchmarking

There is currently **no `simd` feature** and no nightly requirement. Older
README SIMD figures described a different implementation and do not apply.

The workspace release profile optimizes for size (`opt-level = 'z'`, LTO).
Do not assume changing it to `3` improves every parser. Compare on the actual
target machine, retaining the same profile and workload between runs.

The existing format-specific benchmarks use null sinks and remain useful as
parser microbenchmarks. The pipeline benchmarks additionally measure:

| Benchmark group | Work included |
| --- | --- |
| `pipeline_counting` | Parser, observable print/command counts, byte checksum |
| `pipeline_queue` | Parser, actual production queue, queue statistics traversal |
| `pipeline_screen` | Parser and direct screen execution |
| `pipeline_queued_screen` | Parser, queue, statistics, drain and screen execution |
| `no_screen/print_calls` | Actual queue with identical data in different print sizes |

ASCII, ANSI, and RIP plain-text passthrough use chunks of 1, 64, 4096 bytes and
the whole fixture. The core and GUI pipeline fixtures match. Fresh parsers and
bounded screens are prepared outside timed execution; screen tests deliberately
exclude scrolling, UI/GPU rendering, network I/O, music, and graphics decoding.
Thus these numbers are not application-wide FPS or network-throughput claims.

```bash
# Existing microbenchmarks
cargo bench -p icy_parser_core --bench ascii
cargo bench -p icy_parser_core --bench ansi

# Observable parser baseline
cargo bench -p icy_parser_core --bench pipeline

# Production sinks, no window or GPU required
cargo bench -p icy_engine_gui --bench parser_pipeline

# Focused comparison: capture before changing code, then compare after
cargo bench -p icy_engine_gui --bench parser_pipeline -- 'pipeline_queued_screen/rip_plain/whole' --save-baseline before
cargo bench -p icy_engine_gui --bench parser_pipeline -- 'pipeline_queued_screen/rip_plain/whole' --baseline before
```

### Local batching comparison (2026-09-06)

Linux x86-64, Ryzen 9 9950X3D, rustc 1.96.0, unchanged workspace bench profile
(`opt-level = 'z'`, LTO). Criterion: 10 samples, 0.5 s warmup, 1 s measurement.
Approximate central estimates, before/after the text-batching changes:

| Fixture | Before | After |
| --- | ---: | ---: |
| RIP plain, observable parser, whole input | 36.9 µs | 11.2 µs |
| RIP plain, parser + queue, whole input | 289 µs | 19.1 µs |
| RIP plain, parser + queue + screen, whole input | 408 µs | 113 µs |
| ANSI, parser + queue + screen, whole input | 148 µs | 150 µs |
| Queue only, 8192 one-byte prints | 258 µs | 33.0 µs |
| Queue only, one 8192-byte print | 1.14 µs | 1.17 µs |

The tiny-print case changes from 8192 text entries to **one**, deterministically.
The last comparison found no significant change for ANSI or an already-large
print. Runs on this shared workstation varied, so these are local observations,
not portable performance guarantees. Queue statistics traversal is included and
also becomes cheaper when there are fewer entries. Each benchmark measures its
own stage; do not subtract the rows to estimate individual costs.

Trade-off: the added RIP batching checks increased the bytewise parser-only
fixture from about 44.4 to 46.9 µs (roughly 6%). Larger blocks are the intended
fast path; single-byte latency remains a candidate for the parser-specific pass.

## Remaining architecture work

- No shared EOF/finalization or pause/resume API yet. Incomplete input remains
    buffered; do not treat `parse(b"")` as a completion check.
- Buffer limits exist in specific parsers, not as a uniform per-stream budget.
    The bounded queue **text block** size does not bound total queue memory,
    macro expansion, or execution time.
- Optional IGS loop execution still exists in the parser. A fully separate,
    budgeted interpreter is a follow-up, not part of the text-batching change.
- More malformed-input, resource-limit, and coverage-guided fuzz tests are
    still needed. Passing the chunk fixtures is not a proof for every byte stream.
