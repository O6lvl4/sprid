//! Files read by mapping them, as ordinary `Vec<u8>`s.
//!
//! A font is several megabytes that sprid only reads. Read into the heap, it
//! counts in full against the process's memory; mapped, its pages are the
//! file's, clean, and the system shares and drops them as it likes — which
//! is how a terminal using Core Text holds its fonts. Almide's `Bytes` is a
//! `Vec<u8>`, so the mapping has to be one, and a `Vec` may only hold memory
//! its global allocator hands out and takes back. This module is that
//! allocator: the system's, which also owns the mappings made by `map_file`
//! — freeing one unmaps it, growing one moves it to the heap.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

#[global_allocator]
static ALLOCATOR: Mapped = Mapped;

/// Room for this many live mappings (fonts: a regular, a bold, fallbacks).
/// A file past it is read into the heap instead.
const SLOTS: usize = 16;

/// Start and length of each live mapping; a start of 0 is a free slot.
static STARTS: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
static LENS: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
/// Mappings made so far: while 0, freeing never looks at the slots.
static MADE: AtomicUsize = AtomicUsize::new(0);

struct Mapped;

/// The slot of the mapping that starts at `ptr`, if one does.
fn slot_of(ptr: *mut u8) -> Option<usize> {
    if MADE.load(Ordering::Acquire) == 0 {
        return None;
    }
    let p = ptr as usize;
    (0..SLOTS).find(|&i| STARTS[i].load(Ordering::Acquire) == p)
}

/// Unmap the mapping in slot `i` and free the slot.
unsafe fn unmap(i: usize) {
    let (start, len) = (STARTS[i].load(Ordering::Acquire), LENS[i].load(Ordering::Acquire));
    unsafe { libc::munmap(start as *mut libc::c_void, len) };
    STARTS[i].store(0, Ordering::Release);
}

unsafe impl GlobalAlloc for Mapped {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        match slot_of(ptr) {
            Some(i) => unsafe { unmap(i) },
            None => unsafe { System.dealloc(ptr, layout) },
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let Some(i) = slot_of(ptr) else {
            return unsafe { System.realloc(ptr, layout, new_size) };
        };
        // A mapping is never resized in place: its bytes move to the heap.
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        let new = unsafe { System.alloc(new_layout) };
        if !new.is_null() {
            unsafe {
                std::ptr::copy_nonoverlapping(ptr, new, layout.size().min(new_size));
                unmap(i);
            }
        }
        new
    }
}

/// The bytes of the file at `path`, mapped (copy-on-write: writing to them
/// changes this process's copy only); `None` when it can't be read.
pub fn map_file(path: &str) -> Option<Vec<u8>> {
    use std::os::fd::AsRawFd;
    let file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len() as usize;
    if len == 0 {
        return Some(Vec::new());
    }
    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE,
            file.as_raw_fd(),
            0,
        )
    };
    if ptr == libc::MAP_FAILED {
        return None;
    }
    let claimed = (0..SLOTS).find(|&i| {
        STARTS[i].compare_exchange(0, ptr as usize, Ordering::AcqRel, Ordering::Acquire).is_ok()
    });
    let Some(i) = claimed else {
        // Out of slots: an ordinary read.
        unsafe { libc::munmap(ptr, len) };
        return std::fs::read(path).ok();
    };
    LENS[i].store(len, Ordering::Release);
    MADE.fetch_add(1, Ordering::AcqRel);
    // SAFETY: the global allocator (`Mapped`) owns this memory from here on:
    // `dealloc` unmaps it and `realloc` moves it, for a `Vec<u8>` of exactly
    // this length and capacity, alignment 1 (a mapping is page-aligned).
    Some(unsafe { Vec::from_raw_parts(ptr as *mut u8, len, len) })
}
