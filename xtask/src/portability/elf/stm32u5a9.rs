//! The STM32U5A9 (ARM Cortex-M33) image policy of `xtask portability inspect
//! riverdi-rvt50hqsnwc00-b` (specification 34, D.5; design section 4.4).
//!
//! [`inspect`] reads the linked ELF into an [`Image`], and [`check`] applies the policy to it, so
//! each rule is tested on a described image instead of a linked firmware. The layout is the chip's,
//! as `firmware/trust-stm32u5a9/memory.x` and `profile.x` state it: the vector table alone in the
//! first 8 KiB page of bank 1; the firmware from `0x0800_2000` to the end of bank 1; the installed
//! application and the two checkpoint slots in bank 2, which the firmware never loads; the statics in
//! the 640 KiB of SRAM3; and the main stack at the top of SRAM5.
use super::contained;
use super::reader::{read, Image};
use crate::portability::board::BoardProfile;
use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};
use std::ops::Range;

/// The start of bank 1, where the Cortex-M33 fetches its initial stack pointer and reset vector.
const FLASH_START: u64 = 0x0800_0000;
/// The vectors page: the first 8 KiB of bank 1, which holds the vector table alone.
const VECTORS: Range<u64> = FLASH_START..0x0800_2000;
/// The firmware: the rest of bank 1, from `_stext`, and the initial values of `.data` after its code.
const FIRMWARE: Range<u64> = 0x0800_2000..0x0820_0000;
/// The installed application: the rest of bank 2 below the checkpoint slots.
const APPLICATION: Range<u64> = 0x0820_0000..0x083F_8000;
/// The two checkpoint slots at the end of bank 2, 16 KiB each. Nothing is loaded in them.
const CHECKPOINT_A: Range<u64> = 0x083F_8000..0x083F_C000;
const CHECKPOINT_B: Range<u64> = 0x083F_C000..0x0840_0000;
/// The statics: the rest of SRAM3, 640 KiB, for `.data`, `.bss` and the runner (`memory.x`, `RAM`).
const STATICS: Range<u64> = 0x2010_0000..0x201A_0000;
/// The main stack: the top 78 KiB of SRAM5 (`memory.x`, `STACK`).
const MAIN_STACK: Range<u64> = 0x2025_C800..0x2027_0000;
/// The initial stack pointer: the top of the main stack, which is `_stack_start`.
const INITIAL_SP: u64 = MAIN_STACK.end;

/// Inspects a linked STM32U5A9 image.
pub(super) fn inspect(board: &BoardProfile, bytes: &[u8]) -> Result<Value> {
    let image = read(bytes)?;
    let mut report = check(board, &image)?;
    report["firmware_sha256"] = crate::portability::digest(bytes).into();
    Ok(report)
}

/// Whether the bytes `start` to `start + bytes` share an address with `region`.
fn overlaps(start: u64, bytes: u64, region: &Range<u64>) -> bool {
    start
        .checked_add(bytes)
        .is_some_and(|end| start < region.end && end > region.start)
}

/// Whether the bytes touch the installed application or a checkpoint slot.
fn in_application_or_checkpoint(start: u64, bytes: u64) -> bool {
    [APPLICATION, CHECKPOINT_A, CHECKPOINT_B]
        .iter()
        .any(|region| overlaps(start, bytes, region))
}

/// Applies the policy to one image and reports what it holds. The checks run in this order: the
/// file, the vector table, the stack, the recorded end of the firmware, the code, the flash loads,
/// then the sections.
fn check(board: &BoardProfile, image: &Image) -> Result<Value> {
    ensure!(image.arm32, "firmware must be ARM32 little-endian ELF");
    ensure!(image.hard_float, "firmware lacks hard-float ABI flag");
    ensure!(
        FIRMWARE.contains(&(image.entry & !1)),
        "reset entry {:#x} is outside the firmware region",
        image.entry
    );
    let vector_bytes = check_vector_table(image)?;
    check_stack(board, image)?;
    let code_end = firmware_end(image)?;
    check_code(image)?;
    let (placed, flash_bytes) = check_loads(image, code_end)?;
    let (ram_bytes, sections) = check_sections(image, code_end)?;
    Ok(json!({
        "initial_stack_pointer": INITIAL_SP,
        "flash_load_bytes": flash_bytes,
        "static_ram_bytes": ram_bytes,
        "statics_capacity_bytes": STATICS.end - STATICS.start,
        "stack_reserved_bytes": board.stack_window_bytes,
        "image_capacity_bytes": FIRMWARE.end - FIRMWARE.start,
        "firmware_span_bytes": code_end - FIRMWARE.start,
        "firmware_free_bytes": FIRMWARE.end - code_end,
        "vectors_span_bytes": vector_bytes,
        "vectors_free_bytes": VECTORS.end - VECTORS.start - vector_bytes,
        "application_bytes": APPLICATION.end - APPLICATION.start,
        "load_segments": placed,
        "sections": sections,
    }))
}

/// The vector table starts the vectors page and fills no more than it. Its first word is the initial
/// stack pointer, `INITIAL_SP`, and its second the Thumb reset vector of the ELF entry.
fn check_vector_table(image: &Image) -> Result<u64> {
    let vectors = image
        .section(".vector_table")
        .context("missing vector table")?;
    ensure!(
        vectors.address == FLASH_START,
        "vector table at {:#x} does not start the bank at {FLASH_START:#x}",
        vectors.address
    );
    ensure!(
        contained(vectors.address, vectors.size, &VECTORS),
        "vector table of {} bytes at {:#x} exceeds the vectors page",
        vectors.size,
        vectors.address
    );
    let table = &image.vector_table;
    ensure!(table.len() >= 8, "truncated vector table");
    let initial_sp = u32::from_le_bytes(table[..4].try_into()?);
    ensure!(
        u64::from(initial_sp) == INITIAL_SP,
        "unexpected initial stack pointer {initial_sp:#x}, expected {INITIAL_SP:#x}"
    );
    let reset_vector = u32::from_le_bytes(table[4..8].try_into()?);
    ensure!(
        reset_vector & 1 == 1 && u64::from(reset_vector) == image.entry,
        "reset vector {reset_vector:#x} differs from Thumb ELF entry {:#x}",
        image.entry
    );
    Ok(vectors.size)
}

/// The linker reserves the main stack window that the board profile budgets, from `_stack_end` up to
/// `_stack_start`, and the initial stack pointer is its top.
fn check_stack(board: &BoardProfile, image: &Image) -> Result<()> {
    let stack_start = image
        .stack_start
        .context("missing linker symbol _stack_start")?;
    let stack_end = image
        .stack_end
        .context("missing linker symbol _stack_end")?;
    ensure!(
        stack_end == MAIN_STACK.start
            && stack_start == INITIAL_SP
            && stack_start - stack_end == board.stack_window_bytes,
        "linker did not reserve the main stack region {:#x}..{INITIAL_SP:#x}",
        MAIN_STACK.start
    );
    Ok(())
}

/// The recorded end of the firmware's flash, `__firmware_load_end`, lies in the firmware region. It
/// may equal the region's end, the application's first byte.
fn firmware_end(image: &Image) -> Result<u64> {
    let end = image
        .load_end
        .context("missing linker symbol __firmware_load_end")?;
    ensure!(
        (FIRMWARE.start..=FIRMWARE.end).contains(&end),
        "firmware load end {end:#x} leaves the firmware region {:#x}..{:#x}",
        FIRMWARE.start,
        FIRMWARE.end
    );
    Ok(end)
}

/// The reset handler lies in `.text`, which is allocated executable code in the firmware region.
fn check_code(image: &Image) -> Result<()> {
    let code = image
        .section(".text")
        .context("missing executable firmware section")?;
    ensure!(
        contained(code.address, code.size, &FIRMWARE)
            && (code.address..code.address + code.size).contains(&(image.entry & !1)),
        "reset handler {:#x} is outside the executable firmware section at {:#x}",
        image.entry & !1,
        code.address
    );
    ensure!(
        code.executable,
        "firmware text at {:#x} is not allocated executable code",
        code.address
    );
    Ok(())
}

/// Each flash load segment lies in the vectors page, read-only, or in the firmware, ending by the
/// recorded end. None lies in the application or a checkpoint slot, and no two overlap. Returns the
/// segments and the bytes they load.
fn check_loads(image: &Image, code_end: u64) -> Result<(Vec<(u64, u64)>, u64)> {
    let mut placed: Vec<(u64, u64)> = Vec::new();
    let mut flash_bytes = 0_u64;
    for load in &image.loads {
        let (start, bytes) = (load.address, load.bytes);
        if contained(start, bytes, &VECTORS) {
            ensure!(
                !load.writable_or_executable,
                "vector load segment at {start:#x} is writable or executable"
            );
        } else if contained(start, bytes, &FIRMWARE) {
            ensure!(
                start + bytes <= code_end,
                "load segment at {start:#x} of {bytes} bytes exceeds the recorded firmware end {code_end:#x}"
            );
        } else if in_application_or_checkpoint(start, bytes) {
            bail!("load segment at {start:#x} lies in the application or checkpoint region");
        } else {
            bail!(
                "load segment at {start:#x} lies outside the vectors page and the firmware region"
            );
        }
        for &(previous, previous_bytes) in &placed {
            ensure!(
                start + bytes <= previous || previous + previous_bytes <= start,
                "overlapping flash load segments at {start:#x}"
            );
        }
        ensure!(
            load.offset
                .checked_add(bytes)
                .is_some_and(|end| end <= image.file_bytes),
            "ELF load at {start:#x} extends beyond file"
        );
        placed.push((start, bytes));
        flash_bytes += bytes;
    }
    ensure!(!placed.is_empty(), "firmware has no flash load segments");
    Ok((placed, flash_bytes))
}

/// Each allocated section lies in the statics; or in the vectors page, where `.vector_table` alone may
/// lie; or in the firmware, ending by the recorded end. A section anywhere else is refused with its
/// name and address. Returns the static RAM bytes and the sections.
fn check_sections(image: &Image, code_end: u64) -> Result<(u64, Vec<Value>)> {
    let mut ram_bytes = 0_u64;
    let mut sections = Vec::new();
    for section in &image.sections {
        let (name, start, size) = (section.name.as_str(), section.address, section.size);
        if contained(start, size, &STATICS) {
            ram_bytes += size;
        } else if contained(start, size, &VECTORS) {
            ensure!(
                name == ".vector_table",
                "unexpected allocated section {name} at {start:#x} in the vectors page"
            );
        } else if contained(start, size, &FIRMWARE) {
            ensure!(
                start + size <= code_end,
                "section {name} at {start:#x} exceeds the recorded firmware end {code_end:#x}"
            );
        } else if in_application_or_checkpoint(start, size) {
            bail!("allocated section {name} at {start:#x} lies in the application or checkpoint region");
        } else {
            bail!(
                "allocated section {name} at {start:#x} lies outside the vectors page, the firmware and the statics"
            );
        }
        sections.push(json!({"name": name, "address": start, "bytes": size}));
    }
    Ok((ram_bytes, sections))
}

#[cfg(test)]
mod tests {
    use super::super::reader::{Load, Section};
    use super::*;

    fn board() -> BoardProfile {
        BoardProfile::riverdi_rvt50hqsnwc00_b().unwrap()
    }

    fn section(name: &str, address: u64, size: u64, executable: bool) -> Section {
        Section {
            name: name.into(),
            address,
            size,
            executable,
        }
    }

    fn load(address: u64, bytes: u64, offset: u64, writable_or_executable: bool) -> Load {
        Load {
            address,
            bytes,
            offset,
            writable_or_executable,
        }
    }

    /// A 0x188-byte vector table with the initial stack pointer and reset vector in place.
    fn vector_table(initial_sp: u32, reset: u32) -> Vec<u8> {
        let mut table = vec![0; 0x188];
        table[..4].copy_from_slice(&initial_sp.to_le_bytes());
        table[4..8].copy_from_slice(&reset.to_le_bytes());
        table
    }

    /// A representative layout, not a linked build: the vector table in the vectors page; 256 KiB of
    /// code and 64 KiB of read-only data from `0x0800_2000`; the initial values of `.data` after them;
    /// 1 KiB of `.data` and 73 KiB of `.bss` in the statics; and the main stack at the top of SRAM5.
    fn image() -> Image {
        Image {
            arm32: true,
            hard_float: true,
            entry: 0x0800_2101,
            sections: vec![
                section(".vector_table", 0x0800_0000, 0x188, false),
                section(".text", 0x0800_2000, 0x4_0000, true),
                section(".rodata", 0x0804_2000, 0x1_0000, false),
                section(".data", 0x2010_0000, 0x400, false),
                section(".bss", 0x2010_0400, 0x1_2400, false),
            ],
            vector_table: vector_table(0x2027_0000, 0x0800_2101),
            loads: vec![
                load(0x0800_0000, 0x188, 0x1000, false),
                load(0x0800_2000, 0x5_0000, 0x2000, true),
                load(0x0805_2000, 0x400, 0x5_2000, true),
            ],
            stack_start: Some(0x2027_0000),
            stack_end: Some(0x2025_C800),
            load_end: Some(0x0805_2400),
            file_bytes: 0x5_2400,
        }
    }

    /// The report of the representative image after `mutate`.
    fn accepted(mutate: impl FnOnce(&mut Image)) -> Value {
        let mut image = image();
        mutate(&mut image);
        check(&board(), &image).unwrap()
    }

    /// The error text for the representative image after `mutate`, with its causes.
    fn rejected(mutate: impl FnOnce(&mut Image)) -> String {
        let mut image = image();
        mutate(&mut image);
        format!("{:#}", check(&board(), &image).unwrap_err())
    }

    #[test]
    fn riverdi_inspect_accepts_the_firmware_layout() {
        // Spec 34 D.5 (Flash and RAM rows) and design 4.4: the vector table at 0x0800_0000 in its
        // page, the firmware from 0x0800_2000, the statics in SRAM3, the stack at the top of SRAM5.
        // The sums are worked out by hand: loads 0x188 + 0x5_0000 + 0x400 bytes; statics 0x400 +
        // 0x1_2400 bytes; firmware 0x0800_2000 to 0x0805_2400, which leaves 0x1A_DC00 bytes of the
        // 2040 KiB. Catches a check that refuses the layout the spec places.
        let report = accepted(|_| {});
        assert_eq!(report["flash_load_bytes"], 0x5_0588);
        assert_eq!(report["static_ram_bytes"], 0x1_2800);
        assert_eq!(report["firmware_span_bytes"], 0x5_0400);
        assert_eq!(report["firmware_free_bytes"], 0x1A_DC00);
        assert_eq!(report["vectors_free_bytes"], 0x2000 - 0x188);
        assert_eq!(report["initial_stack_pointer"], 0x2027_0000);
        assert_eq!(report["sections"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn riverdi_inspect_refuses_a_section_in_the_application_region() {
        // Spec 34 D.5 (Flash row): the application and the two checkpoint slots are in bank 2 and
        // are never loaded by the firmware. A section in any of them, or one that runs into them
        // from below, is refused with its name and address. Catches an image that puts a static or
        // a stray section in bank 2, which would overwrite an installed application on the bench.
        for (address, size) in [
            (0x0820_0000, 4),
            (0x083F_7FFC, 8),
            (0x083F_8000, 4),
            (0x083F_C000, 4),
            (0x083F_FFFC, 4),
        ] {
            let error =
                rejected(|image| image.sections.push(section(".stray", address, size, false)));
            let address_text = format!("{address:#x}");
            assert!(error.contains(".stray"), "{address:#x}: {error}");
            assert!(
                error.contains(address_text.as_str()),
                "{address:#x}: {error}"
            );
            assert!(
                error.contains("application or checkpoint"),
                "{address:#x}: {error}"
            );
        }
        // A load into the application is refused the same way.
        let error = rejected(|image| image.loads.push(load(0x0820_0000, 4, 0x100, false)));
        assert!(error.contains("load segment at 0x8200000"), "{error}");
        assert!(error.contains("application or checkpoint"), "{error}");
    }

    #[test]
    fn riverdi_inspect_refuses_a_stack_pointer_off_the_top() {
        // Spec 34 D.5 (RAM row): the main stack is the top 32 KiB of SRAM5 and the initial stack
        // pointer is its top, 0x2027_0000. A pointer one word above it, at the bottom of the stack, or
        // at the top of the statics is refused with the pointer's value. Catches a vector table that
        // starts the stack at the wrong place.
        for initial_sp in [0x2027_0004, 0x2025_C800, 0x2010_0000] {
            let error =
                rejected(|image| image.vector_table = vector_table(initial_sp, 0x0800_2101));
            let value = format!("{initial_sp:#x}");
            assert!(
                error.contains("initial stack pointer"),
                "{initial_sp:#x}: {error}"
            );
            assert!(error.contains(value.as_str()), "{initial_sp:#x}: {error}");
        }
        // The linker's symbols must agree with the top too: a window that ends short of it, or one
        // that starts below it, is refused, and so is a missing symbol.
        let error = rejected(|image| image.stack_start = Some(0x2026_FFFC));
        assert!(
            error.contains("did not reserve the main stack region"),
            "{error}"
        );
        let error = rejected(|image| image.stack_end = Some(0x2026_0000));
        assert!(
            error.contains("did not reserve the main stack region"),
            "{error}"
        );
        let error = rejected(|image| image.stack_start = None);
        assert!(error.contains("_stack_start"), "{error}");
        // The exact top is accepted.
        accepted(|image| image.vector_table = vector_table(0x2027_0000, 0x0800_2101));
    }

    #[test]
    fn riverdi_inspect_keeps_the_vector_table_alone_in_its_page() {
        // Spec 34 D.5 (Flash row): the vector table is the first 8 KiB page of bank 1 and nothing
        // else lies there. A table that does not start the bank, or that runs past the page, is
        // refused; so is another section in the page, or a writable vector segment.
        let error = rejected(|image| image.sections[0].size = 0x2001);
        assert!(error.contains("exceeds the vectors page"), "{error}");
        let error = rejected(|image| image.sections[0].address = 0x0800_0100);
        assert!(error.contains("does not start the bank"), "{error}");
        let error = rejected(|image| {
            image
                .sections
                .push(section(".stray", 0x0800_1000, 4, false))
        });
        assert!(
            error.contains("unexpected allocated section .stray at 0x8001000"),
            "{error}"
        );
        let error = rejected(|image| image.loads[0].writable_or_executable = true);
        assert!(error.contains("writable or executable"), "{error}");
        // The table may fill the page exactly.
        let report = accepted(|image| image.sections[0].size = 0x2000);
        assert_eq!(report["vectors_free_bytes"], 0);
    }

    #[test]
    fn riverdi_inspect_ends_the_firmware_where_the_application_starts() {
        // Spec 34 D.5 (Flash row): the firmware and the initial values of `.data` end by 0x0820_0000,
        // the application's first byte. The recorded end may equal that address and not one byte
        // more. A recorded end below the firmware is refused, and so is a missing symbol.
        let report = accepted(|image| image.load_end = Some(0x0820_0000));
        assert_eq!(report["firmware_free_bytes"], 0);
        let error = rejected(|image| image.load_end = Some(0x0820_0001));
        assert!(error.contains("leaves the firmware region"), "{error}");
        let error = rejected(|image| image.load_end = Some(0x0800_1FFF));
        assert!(error.contains("leaves the firmware region"), "{error}");
        let error = rejected(|image| image.load_end = None);
        assert!(error.contains("__firmware_load_end"), "{error}");
        // A recorded end short of the initial values of `.data` leaves them outside the firmware's end.
        let error = rejected(|image| image.load_end = Some(0x0805_2000));
        assert!(
            error.contains("exceeds the recorded firmware end"),
            "{error}"
        );
    }

    #[test]
    fn riverdi_inspect_refuses_a_static_outside_the_statics_region() {
        // Spec 34 D.5 (RAM row): the statics are the rest of SRAM3, 0x2010_0000 to 0x201A_0000. Heap
        // region A below them, heap region B above them, the main stack, SRAM4 and the backup SRAM are
        // not statics. A section that starts outside the statics or runs past their end is refused.
        for (address, size) in [
            (0x2000_0000, 4),
            (0x2019_FFFC, 8),
            (0x201A_0000, 4),
            (0x2800_0000, 4),
            (0x4201_6400, 4),
        ] {
            let error =
                rejected(|image| image.sections.push(section(".late", address, size, false)));
            let address_text = format!("{address:#x}");
            assert!(
                error.contains("outside the vectors page, the firmware and the statics"),
                "{address:#x}: {error}"
            );
            assert!(
                error.contains(address_text.as_str()),
                "{address:#x}: {error}"
            );
        }
        // The statics fill the 640 KiB from `.data` to `0x201A_0000` and not one byte more.
        let report = accepted(|image| image.sections[4].size = 0x9_FC00);
        assert_eq!(report["static_ram_bytes"], 640 * 1024);
        let error = rejected(|image| image.sections[4].size = 0x9_FC01);
        assert!(error.contains("outside the vectors page"), "{error}");
    }

    #[test]
    fn riverdi_linker_scripts_state_the_layout_that_is_inspected() {
        let memory = include_str!("../../../../firmware/trust-stm32u5a9/memory.x");
        let profile = include_str!("../../../../firmware/trust-stm32u5a9/profile.x");
        for expected in [
            format!("FLASH : ORIGIN = {FLASH_START:#010X}, LENGTH = 2048K"),
            format!("RAM : ORIGIN = {:#010X}, LENGTH = 640K", STATICS.start),
            format!("STACK : ORIGIN = {:#010X}, LENGTH = 78K", MAIN_STACK.start),
            format!("_stext = {:#010X};", FIRMWARE.start),
            format!("_stack_start = {INITIAL_SP:#010X};"),
            format!("_stack_end = {:#010X};", MAIN_STACK.start),
        ] {
            assert!(memory.contains(&expected), "memory.x lacks {expected}");
        }
        for expected in [
            "SIZEOF(.vector_table) <= 8K".to_owned(),
            format!("ADDR(.vector_table) == {FLASH_START:#010X}"),
            format!("ADDR(.text) >= {:#010X}", FIRMWARE.start),
            "_stack_start - _stack_end == 78K".to_owned(),
            format!("__firmware_load_end <= {:#010X}", FIRMWARE.end),
        ] {
            assert!(profile.contains(&expected), "profile.x lacks {expected}");
        }
        assert_eq!(VECTORS.end - VECTORS.start, 8 * 1024);
        assert_eq!(STATICS.end - STATICS.start, 640 * 1024);
        assert_eq!(MAIN_STACK.end - MAIN_STACK.start, 78 * 1024);
        assert_eq!(board().stack_window_bytes, 78 * 1024);
    }

    #[test]
    fn the_inspected_regions_are_the_layout_of_the_chip_file() {
        let chip = crate::portability::board::chip("stm32u5a9").unwrap();
        let span = |region: trust_chips::Region| u64::from(region.start)..region.end();
        let layout = &chip.layout;
        assert_eq!(span(layout.vectors.unwrap()), VECTORS);
        assert_eq!(span(layout.firmware), FIRMWARE);
        assert_eq!(span(layout.application), APPLICATION);
        assert_eq!(span(layout.checkpoint_a), CHECKPOINT_A);
        assert_eq!(span(layout.checkpoint_b), CHECKPOINT_B);
        let flash = chip.memory.flash.as_ref().unwrap();
        assert_eq!(u64::from(flash.start), FLASH_START);
        assert_eq!(FLASH_START + u64::from(flash.kib) * 1024, CHECKPOINT_B.end);
        let ram = |name: &str| {
            chip.memory
                .ram
                .iter()
                .find(|region| region.name == name)
                .unwrap()
        };
        // SRAM3 holds the statics at its top; SRAM5 ends where the main stack does.
        assert!(u64::from(ram("SRAM3").start) <= STATICS.start);
        assert_eq!(ram("SRAM3").end(), STATICS.end);
        assert_eq!(ram("SRAM5").end(), MAIN_STACK.end);
        let board = board();
        assert_eq!(board.image_capacity_bytes, FIRMWARE.end - FIRMWARE.start);
        assert_eq!(board.application_bytes, APPLICATION.end - APPLICATION.start);
    }

    #[test]
    fn something_that_is_not_an_elf_is_refused() {
        assert!(inspect(&board(), b"not a linked firmware image").is_err());
        assert!(inspect(&board(), &[]).is_err());
    }
}
