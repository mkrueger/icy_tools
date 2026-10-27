//! Inline images must be size-checked before they are decoded.
//!
//! Its own test binary, since it counts allocations with a global allocator.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use icy_engine::Size;

struct LargestAllocation;

static LARGEST: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for LargestAllocation {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LARGEST.fetch_max(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        LARGEST.fetch_max(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LARGEST.fetch_max(new_size, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: LargestAllocation = LargestAllocation;

#[test]
fn test_oversized_inline_image_is_rejected_before_decoding() {
    // A header claiming 8000x8000 (64 MB of pixels) with no pixel data behind it.
    let image = b"P5\n8000 8000\n255\n\0\0\0\0";

    LARGEST.store(0, Ordering::Relaxed);
    let result = icy_engine::decode_image_blob(image, false, "", Size::new(8, 16), Size::new(80, 25));
    assert!(result.is_none());
    let largest = LARGEST.load(Ordering::Relaxed);
    assert!(largest < 1024 * 1024, "allocated {largest} bytes for a rejected image");
}
