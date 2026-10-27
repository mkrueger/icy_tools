//! Macro expansion must check its budget before allocating a copy of the body.
//!
//! A separate test binary isolates the allocation counter from other tests.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use icy_parser_core::{AnsiParser, CommandParser, CommandSink, TerminalCommand};

struct CountingAllocator;

static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATED_BYTES.fetch_add(new_size, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Default)]
struct CountingSink {
    printed_bytes: usize,
}

impl CommandSink for CountingSink {
    fn print(&mut self, text: &[u8]) {
        self.printed_bytes += text.len();
    }

    fn emit(&mut self, _command: TerminalCommand) {}
}

#[test]
fn rejected_macro_invocations_do_not_allocate_bodies() {
    let mut parser = AnsiParser::new();
    let mut sink = CountingSink::default();
    let mut definition = b"\x1bP0;0;0!z".to_vec();
    definition.extend(std::iter::repeat_n(b'A', 100_000));
    definition.extend_from_slice(b"\x1b\\");
    parser.parse(&definition, &mut sink);
    parser.parse(b"\x1bP1;0;1!z!1000;1B5B302A7A;\x1b\\", &mut sink);

    // Each top-level call gets a fresh 4 MiB budget, not 100 MB of body copies.
    for _ in 0..2 {
        sink.printed_bytes = 0;
        ALLOCATED_BYTES.store(0, Ordering::Relaxed);
        parser.parse(b"\x1b[1*z", &mut sink);
        let allocated = ALLOCATED_BYTES.load(Ordering::Relaxed);

        assert!(sink.printed_bytes >= 100_000);
        assert!(sink.printed_bytes < 5 * 1024 * 1024);
        assert!(allocated < 5 * 1024 * 1024, "allocated {allocated} bytes despite the macro expansion budget");
    }
}
