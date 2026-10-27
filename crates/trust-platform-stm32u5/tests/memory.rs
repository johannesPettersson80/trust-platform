//! The STM32U5A9's memory map (specification 34, D.5; design §4.3) against the chip file and against itself.

use trust_chips::ChipFile;
use trust_platform_stm32u5::memory::{
    Region, BKPSRAM, HEAP_A, HEAP_B, MAIN_STACK, SRAM4, STACK_WINDOW, STATICS,
};

fn chip_file() -> ChipFile {
    ChipFile::parse("stm32u5a9", include_str!("../../../chips/stm32u5a9.toml"))
        .expect("the STM32U5A9 chip file is valid")
}

/// The span of a RAM block of the chip file, from its start and its size in KiB.
fn block(file: &ChipFile, name: &str) -> Region {
    let found = file
        .memory
        .ram
        .iter()
        .find(|region| region.name == name)
        .expect("the chip file names the block");
    Region {
        start: found.start,
        end: found.start + found.kib * 1024,
    }
}

#[test]
fn memory_regions_match_the_chip_and_do_not_overlap() {
    // Specification 34, D.5 (RAM) and design §4.3, worked out by hand: heap region A is 1 MiB from 0x2000_0000
    // (SRAM1, SRAM2 and 192 KiB of SRAM3); the statics are the other 640 KiB of SRAM3, to 0x201A_0000; heap
    // region B is 754 KiB from 0x201A_0000 to 0x2025_C800; the main stack is the top 78 KiB of SRAM5, from
    // 0x2025_C800 to 0x2027_0000 (0x2027_0000 - 78 x 1,024 = 0x2025_C800). SRAM4 and the backup SRAM are reported and assigned to nothing.
    // Catches a region one block off, a stack at the bottom of SRAM5, a window of the wrong size, and two
    // regions that share an address.
    assert_eq!(
        HEAP_A,
        Region {
            start: 0x2000_0000,
            end: 0x2010_0000
        }
    );
    assert_eq!(
        STATICS,
        Region {
            start: 0x2010_0000,
            end: 0x201A_0000
        }
    );
    assert_eq!(
        HEAP_B,
        Region {
            start: 0x201A_0000,
            end: 0x2025_C800
        }
    );
    assert_eq!(
        MAIN_STACK,
        Region {
            start: 0x2025_C800,
            end: 0x2027_0000
        }
    );
    assert_eq!(
        SRAM4,
        Region {
            start: 0x2800_0000,
            end: 0x2800_4000
        }
    );
    assert_eq!(
        BKPSRAM,
        Region {
            start: 0x4201_6400,
            end: 0x4201_6C00
        }
    );

    // The sizes: 1 MiB, 640 KiB, 754 KiB and 78 KiB.
    assert_eq!(HEAP_A.end - HEAP_A.start, 1024 * 1024);
    assert_eq!(STATICS.end - STATICS.start, 640 * 1024);
    assert_eq!(HEAP_B.end - HEAP_B.start, 754 * 1024);
    assert_eq!(MAIN_STACK.end - MAIN_STACK.start, 78 * 1024);

    // Against the chip file: SRAM1 is 768 KiB from 0x2000_0000, SRAM2 is 64 KiB, SRAM3 is 832 KiB from
    // 0x200D_0000, SRAM5 is 832 KiB from 0x201A_0000, and the backup SRAM and SRAM4 are as above.
    let file = chip_file();
    assert_eq!(
        block(&file, "SRAM1"),
        Region {
            start: 0x2000_0000,
            end: 0x200C_0000
        }
    );
    assert_eq!(
        block(&file, "SRAM3"),
        Region {
            start: 0x200D_0000,
            end: 0x201A_0000
        }
    );
    assert_eq!(
        block(&file, "SRAM5"),
        Region {
            start: 0x201A_0000,
            end: 0x2027_0000
        }
    );
    assert_eq!(block(&file, "SRAM4"), SRAM4);
    assert_eq!(block(&file, "BKPSRAM"), BKPSRAM);

    // The assigned regions tile SRAM1 to SRAM5 end to end: each starts where the one before it ends, so none
    // overlaps another and none is left out. The stack sits at the very top of SRAM5.
    assert_eq!(HEAP_A.start, block(&file, "SRAM1").start);
    assert_eq!(HEAP_A.end, STATICS.start);
    assert_eq!(STATICS.end, HEAP_B.start);
    assert_eq!(HEAP_B.end, MAIN_STACK.start);
    assert_eq!(MAIN_STACK.end, block(&file, "SRAM5").end);

    // The window the shared crate measures has the stack's bounds.
    assert_eq!(STACK_WINDOW.bottom(), 0x2025_C800);
    assert_eq!(STACK_WINDOW.top(), 0x2027_0000);
}
