//! The program region's placement (specification 44 §10.3, §12.3). The region only computes
//! addresses, so the tests give it an address range and touch no memory.

use super::{ProgramRegion, BLOCK_ALIGN};

/// The C6's program region: 192 KiB at an aligned address.
const START: usize = 0x4083_0000;
const SIZE: usize = 192 * 1024;

fn region() -> ProgramRegion {
    let region = ProgramRegion::empty();
    region.init(START, SIZE);
    region
}

#[test]
fn a_module_goes_to_the_top_and_an_upload_to_the_bottom_on_block_boundaries() {
    // Worked by hand: a module of 43,810 bytes asked high ends at the region's end, its start
    // rounded down to 64: (START + 196,608 - 43,810) & !63 = START + 152,768; an upload of 90,200
    // bytes asked low starts at START. Catches a block off its 64-byte boundary, which a module's
    // code needs, and a module placed where the next upload would go.
    let region = region();
    let module = region.allocate(43_810, true).expect("the module fits");
    assert_eq!(module, START + 152_768);
    assert!(module.is_multiple_of(BLOCK_ALIGN));
    let upload = region.allocate(90_200, false).expect("the upload fits");
    assert_eq!(upload, START);
    assert_eq!(region.used(), 43_810 + 90_200);
}

#[test]
fn repeated_changes_of_the_pasteurizers_debug_image_alternate_in_the_region() {
    // Worked by hand with the pasteurizer's debug image (90,200 bytes) and module (43,810 bytes)
    // in 196,608 bytes: the installed module at the top, the first upload at 0; the commit frees
    // the module, so the second upload goes after the first image, at 90,240 (90,200 rounded up
    // to 64); the next commit frees the first image, so the third upload takes 0 again, and the
    // fourth 90,240. Catches placement that leaves only gaps smaller than an image after the
    // installed module is freed (first fit from the bottom for the module too), which refuses the
    // second change of a program the region holds twice.
    let region = region();
    let module = region.allocate(43_810, true).expect("the installed module");
    let first = region.allocate(90_200, false).expect("the first change");
    assert_eq!(first, START);
    assert!(region.free(module));
    let second = region.allocate(90_200, false).expect("the second change");
    assert_eq!(second, START + 90_240);
    assert!(region.free(first));
    let third = region.allocate(90_200, false).expect("the third change");
    assert_eq!(third, START);
    assert!(region.free(second));
    let fourth = region.allocate(90_200, false).expect("the fourth change");
    assert_eq!(fourth, START + 90_240);
}

#[test]
fn a_block_that_fits_no_gap_and_a_full_list_are_refused() {
    // 196,608 bytes hold two images of 90,200 but not three; the region keeps at most eight blocks;
    // a free of an address where no block starts changes nothing. Catches an overlap of blocks
    // and a list that grows past its bound.
    let region = region();
    assert!(region.allocate(90_200, false).is_some());
    assert!(region.allocate(90_200, false).is_some());
    assert_eq!(region.allocate(90_200, false), None);
    assert!(!region.free(START + 1));
    let small = ProgramRegion::empty();
    small.init(START, 64 * 16);
    for _ in 0..8 {
        assert!(small.allocate(64, false).is_some());
    }
    assert_eq!(small.allocate(64, false), None);
}

#[test]
fn a_region_without_memory_holds_nothing() {
    // A heap whose composition gives no program region places no block there and owns no
    // address. Catches an allocation from an uninitialized region.
    let region = ProgramRegion::empty();
    assert!(!region.is_set());
    assert_eq!(region.allocate(64, false), None);
    assert!(!region.contains(START));
    let set = super::ProgramRegion::empty();
    set.init(START, SIZE);
    assert!(set.contains(START) && set.contains(START + SIZE - 1) && !set.contains(START + SIZE));
}
