# icy_parser_core

Minimal core crate providing parsing infrastructure decoupled from rendering. It defines:

- `TerminalCommand`: semantic commands (printable runs as byte slices + control events).
- `CommandSink`: consumer trait for emitted commands with error reporting.
- `CommandParser`: streaming parser interface.
- `AsciiParser`: initial implementation that batches printable ASCII runs and emits control commands.
- Type-safe enums for ANSI command parameters (e.g., `EraseInDisplayMode`, `EraseInLineMode`).

## Features

- **Type Safety**: Enums for command parameters instead of raw integers
  - `EraseInDisplayMode`: CursorToEnd, StartToCursor, All, AllAndScrollback
  - `EraseInLineMode`: CursorToEnd, StartToCursor, All
  - `DeviceStatusReport`: OperatingStatus, CursorPosition
  
- **Error Reporting**: `CommandSink::report_error()` for invalid parameters or malformed sequences

## Goals

1. Zero coupling to higher-level buffer / UI crates.
2. Efficient batch emission to reduce per-byte overhead.
3. Foundation for migrating ANSI and other format parsers.
4. Type-safe command parameters with error reporting.

## Example

```rust
use icy_parser_core::{AsciiParser, CommandParser, CommandSink, ParseError, TerminalCommand};

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

### Type-Safe Example

See `examples/type_safe_ansi.rs` for a complete example demonstrating:
- Type-safe enum usage for ANSI commands
- Error reporting for invalid parameters
- Fallback behavior on errors

Run it with:
```bash
cargo run --package icy_parser_core --example type_safe_ansi
```

## Testing

Run the crate tests:

```bash
cargo test -p icy_parser_core
```

## Performance Features

### SIMD Acceleration (Opt-in)

The parser includes an optional SIMD implementation using `portable_simd` that significantly accelerates parsing of text-heavy workloads:

- **5.3× faster** on pure text / UTF-8 content (20+ GiB/s throughput)
- **Comparable** performance on ASCII with line breaks
- **46% slower** on control-heavy input (frequent escape sequences, cursor movements)

**When to use SIMD:**
- Log file parsing
- Plain text viewing
- Document processing
- Any workload with long printable runs and sparse controls

**When to use default (LUT):**
- Interactive terminal emulation
- Control-heavy escape sequences
- Mixed workloads
- Stable Rust requirement

**Enable SIMD:**

Requires **nightly Rust** for `portable_simd` feature:

```bash
# Run tests with SIMD
cargo +nightly test -p icy_parser_core --features simd

# Run benchmarks with SIMD
cargo +nightly bench -p icy_parser_core --features simd
```

Add to your `Cargo.toml`:

```toml
[dependencies]
icy_parser_core = { version = "0.1", features = ["simd"] }
```

The SIMD implementation is portable across x86-64 (SSE/AVX) and ARM64 (NEON) architectures.

## Benchmarking

Compare parser implementations and workload patterns:

```bash
# ASCII parser benchmarks
cargo bench -p icy_parser_core --bench ascii

# ANSI parser benchmarks
cargo bench -p icy_parser_core --bench ansi

# SIMD comparison (requires nightly)
cargo +nightly bench -p icy_parser_core --features simd
```

### ASCII Parser Performance

- **Pure ASCII**: 3.0-3.4 GiB/s
- **UTF-8 mixed**: 3.2-3.5 GiB/s
- **Control heavy**: 2.8-3.0 GiB/s
- **SIMD (text-heavy)**: 20+ GiB/s (5.3× faster)

### ANSI Parser Performance

- **Real-world ANSI art**: ~271-273 MiB/s (6 combined ANSI files)
- **Text-heavy** (minimal ANSI): ~565-575 MiB/s
- **Mixed content**: ~531-546 MiB/s
- **CSI-heavy** (cursor movements): ~533-545 MiB/s
- **Color-heavy** (SGR sequences): ~384-388 MiB/s

The ANSI parser uses a nested match structure (state → byte) that allows the compiler to generate highly optimized code for each state's byte matching. It handles realistic terminal output efficiently, with real-world ANSI art files (containing complex cursor positioning, colors, and box-drawing) parsing at **~271 MiB/s**.

### Avatar Parser

The Avatar (Advanced Video Attribute Terminal Assembler and Recreator) parser handles the compact Avatar control language:

- **Commands**: `^V` followed by command byte (set color, cursor movement, clear screen)
- **Repetition**: `^Y{char}{count}` for efficient character runs
- **ANSI Integration**: Cursor movements map to ANSI equivalents; delegates to ANSI parser for standard control codes

Avatar commands:
- `^V^A{color}` - Set text attribute
- `^V^B` - Enable blinking
- `^V^C/^D/^E/^F` - Cursor movement (up/down/left/right) → maps to ANSI `CsiCursorUp/Down/Back/Forward`
- `^V^G` - Clear to end of line
- `^V^H{row}{col}` - Position cursor
- `^Y{char}{count}` - Repeat character
- `^L` - Clear screen

## Next Steps

- Adapter layer in `icy_engine` translating `TerminalCommand` to existing buffer operations.
- Benchmark harness for before/after performance comparisons.
- Additional format parsers (PETSCII, Viewdata, etc.)
