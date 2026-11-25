//! The measured heap's program region (specification 44 §10.3, §12.3), on the host.

extern crate std;

use core::alloc::{GlobalAlloc, Layout};
use std::boxed::Box;
use std::vec;

use super::{MeasuredHeap, PROGRAM_BLOCK_BYTES};
use crate::program_region::BLOCK_ALIGN;

/// A heap with one data region of 4,096 bytes and a program region of 192 KiB.
fn heap() -> &'static MeasuredHeap {
    let heap: &'static MeasuredHeap = Box::leak(Box::new(MeasuredHeap::empty()));
    let data: &'static mut [u8] = Box::leak(vec![0u8; 4096].into_boxed_slice());
    let program: &'static mut [u8] = Box::leak(vec![0u8; 192 * 1024].into_boxed_slice());
    // SAFETY: both regions are leaked, exclusive and valid for the test's lifetime; nothing else
    // allocates from this heap.
    unsafe {
        heap.init(data, None, None);
        heap.init_program_region(program);
    }
    heap
}

#[test]
fn a_programs_blocks_go_to_the_program_region_and_the_rest_to_the_data_regions() {
    // Specification 44 §12.3 on the C6: a block asked with the module alignment (64) is a module
    // copy and goes to the top of the program region; a block of 32 KiB or more asked as a
    // program's, an upload, goes to its bottom; a small block goes to a data region; each comes back
    // where it came from, and `remaining` counts the data regions only. Catches an upload in the
    // data heap, whose module the CPU cannot run in place, small data in the executable region, and
    // a program block counted against a program's data.
    let heap = heap();
    let region = heap.program_region().expect("the program region");
    let module_layout = Layout::from_size_align(1000, BLOCK_ALIGN).unwrap();
    let upload_layout = Layout::from_size_align(PROGRAM_BLOCK_BYTES, 1).unwrap();
    let small_layout = Layout::from_size_align(100, 8).unwrap();
    let remaining = heap.remaining();
    // SAFETY: valid layouts of non-zero size; each block is freed with its own layout.
    unsafe {
        let module = heap.alloc(module_layout);
        let upload = heap.with_program_allocations(|| heap.alloc(upload_layout));
        let small = heap.alloc(small_layout);
        assert!(region.contains(module as usize) && (module as usize).is_multiple_of(BLOCK_ALIGN));
        assert!(region.contains(upload as usize) && (upload as usize) < module as usize);
        assert!(!region.contains(small as usize));
        assert_eq!(heap.remaining(), remaining - heap.inner[0].used());
        heap.dealloc(module, module_layout);
        heap.dealloc(upload, upload_layout);
        heap.dealloc(small, small_layout);
    }
    assert_eq!(region.used(), 0);
    assert_eq!(heap.remaining(), remaining);
}

#[test]
fn a_heap_without_a_program_region_serves_every_block_from_its_data_regions() {
    // The F407 and the U5A9 give no program region: a block asked with alignment 64 comes from a
    // data region. Catches a heap that refuses an aligned block when it has no program region.
    let heap: &'static MeasuredHeap = Box::leak(Box::new(MeasuredHeap::empty()));
    let data: &'static mut [u8] = Box::leak(vec![0u8; 4096].into_boxed_slice());
    // SAFETY: the region is leaked, exclusive and valid for the test's lifetime.
    unsafe { heap.init(data, None, None) };
    assert!(heap.program_region().is_none());
    let layout = Layout::from_size_align(256, BLOCK_ALIGN).unwrap();
    // SAFETY: a valid layout of non-zero size, freed with the same layout.
    unsafe {
        let block = heap.alloc(layout);
        assert!(!block.is_null());
        heap.dealloc(block, layout);
    }
}

#[test]
fn a_large_block_of_data_takes_the_data_regions_and_only_an_uploads_the_program_region() {
    // Specification 34 MEM-12 with specification 44 §12.3: a block of 32 KiB asked as data, such
    // as a recording's room, goes to a data region, and `used` counts it; the same block asked
    // inside `with_program_allocations`, as an upload, goes to the program region, which counts it
    // while `used` does not. Worked by hand on a heap of a 64 KiB data region and a 192 KiB
    // program region: `used` is 32,768 or more after the data block (the allocator's header on
    // top) and 0 again once it is freed, and the program region's `used` is 32,768 or more for
    // the upload. Catches a large block of data in the memory the next program's code is
    // installed in, and a `used` that mixes the program region with the data it is compared to.
    let heap: &'static MeasuredHeap = Box::leak(Box::new(MeasuredHeap::empty()));
    let data: &'static mut [u8] = Box::leak(vec![0u8; 64 * 1024].into_boxed_slice());
    let program: &'static mut [u8] = Box::leak(vec![0u8; 192 * 1024].into_boxed_slice());
    // SAFETY: both regions are leaked, exclusive and valid for the test's lifetime; nothing else
    // allocates from this heap.
    unsafe {
        heap.init(data, None, None);
        heap.init_program_region(program);
    }
    let region = heap.program_region().expect("the program region");
    let layout = Layout::from_size_align(PROGRAM_BLOCK_BYTES, 8).unwrap();
    // SAFETY: valid layouts of non-zero size; each block is freed with its own layout.
    unsafe {
        let data_block = heap.alloc(layout);
        assert!(!data_block.is_null());
        assert!(!region.contains(data_block as usize));
        assert!(heap.used() >= PROGRAM_BLOCK_BYTES, "used {}", heap.used());
        assert_eq!(region.used(), 0);
        heap.dealloc(data_block, layout);
        assert_eq!(heap.used(), 0);

        let upload = heap.with_program_allocations(|| heap.alloc(layout));
        assert!(region.contains(upload as usize));
        assert!(
            region.used() >= PROGRAM_BLOCK_BYTES,
            "region used {}",
            region.used()
        );
        assert_eq!(heap.used(), 0);
        heap.dealloc(upload, layout);
    }
    assert_eq!(region.used(), 0);
}
