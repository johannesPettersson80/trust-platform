//! The STM32F407 (ARM Cortex-M4F) image policy of `xtask portability inspect`.
//!
//! [`inspect`] reads the linked ELF into an [`Image`], and [`check`] applies the policy to it, so
//! each rule is tested on a described image instead of a linked firmware. The layout is the
//! F407's flash sector map (RM0090, specification 34 D.4): the vector table alone in sector 0, the
//! application bundle in sector 1, the checkpoint slots in sectors 2 and 3, and the firmware from
//! sector 4 through sector 11. The 64 KiB of core-coupled memory is unassigned, so a section
//! there fails.
use super::{contained, meets_flash_growth_target, FLASH_GROWTH_TARGET};
use crate::portability::board::BoardProfile;
use anyhow::{ensure, Context, Result};
use object::read::elf::ProgramHeader;
use object::{Object, ObjectSection, ObjectSymbol};
use serde_json::{json, Value};
use std::ops::Range;

const FLASH_START: u64 = 0x0800_0000;
/// The flash sectors in order, in KiB (RM0090, flash organization): four of 16, one of 64 and
/// seven of 128.
const SECTOR_KIB: [u64; 12] = [16, 16, 16, 16, 64, 128, 128, 128, 128, 128, 128, 128];

/// The address of the first byte of `sector`; the sector count gives the end of flash.
const fn sector_address(sector: usize) -> u64 {
    let mut address = FLASH_START;
    let mut index = 0;
    while index < sector {
        address += SECTOR_KIB[index] * 1024;
        index += 1;
    }
    address
}

/// Sector 0: the vector table, alone.
const VECTOR: Range<u64> = sector_address(0)..sector_address(1);
/// Sector 1, where the separately installed application bundle goes. Nothing is loaded here.
const APPLICATION: Range<u64> = sector_address(1)..sector_address(2);
/// Sectors 2 and 3, the checkpoint slots. Nothing is loaded here.
const PERSISTENCE: Range<u64> = sector_address(2)..sector_address(4);
/// Sectors 4 to 11: the firmware, and the initial values of `.data` after its code.
const CODE: Range<u64> = sector_address(4)..sector_address(12);

/// SRAM1 and SRAM2 are one block of 128 KiB. The CCM at 0x1000_0000 is not part of it.
const SRAM: Range<u64> = 0x2000_0000..0x2002_0000;
/// The initial stack pointer: the top of SRAM, where the main stack starts.
const INITIAL_SP: u64 = SRAM.end;
/// The bottom of the main stack, which is the top of the statics and the allocator arena.
const STACK_END: u64 = 0x2001_C000;
/// The part of SRAM below the main stack, for statics and the allocator arena.
const STATIC_RAM: Range<u64> = SRAM.start..STACK_END;

/// An allocated, non-empty section.
struct Section {
    name: String,
    address: u64,
    size: u64,
    /// Whether the section holds code (`SHF_EXECINSTR`).
    executable: bool,
}

/// A loadable segment with file bytes.
struct Load {
    /// The load (physical) address. The initial values of `.data` load in flash.
    address: u64,
    bytes: u64,
    /// Where the segment's bytes start in the ELF file.
    offset: u64,
    /// Whether the segment is writable or executable (`PF_W`, `PF_X`).
    writable_or_executable: bool,
}

/// What the policy reads from one linked ELF32 image.
struct Image {
    /// ARM, 32-bit, little-endian.
    arm32: bool,
    /// `e_flags` carries the hard-float ABI (`EF_ARM_ABI_FLOAT_HARD`).
    hard_float: bool,
    entry: u64,
    sections: Vec<Section>,
    /// The bytes of `.vector_table`: the initial stack pointer, then the reset vector.
    vector_table: Vec<u8>,
    loads: Vec<Load>,
    stack_start: Option<u64>,
    stack_end: Option<u64>,
    /// `__firmware_load_end` from profile.x: the end of the last byte the firmware loads in flash.
    load_end: Option<u64>,
    /// The size of the ELF file, which every segment's bytes must lie within.
    file_bytes: u64,
}

impl Image {
    fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|section| section.name == name)
    }
}

/// Inspects a linked F407 image.
pub(super) fn inspect(board: &BoardProfile, bytes: &[u8]) -> Result<Value> {
    let file = object::File::parse(bytes).context("decode firmware ELF")?;
    let object::FileFlags::Elf { e_flags, .. } = file.flags() else {
        anyhow::bail!("firmware is not ELF");
    };
    let mut sections = Vec::new();
    for section in file.sections() {
        let object::SectionFlags::Elf { sh_flags, .. } = section.flags() else {
            continue;
        };
        if sh_flags.contains(object::elf::SHF_ALLOC) && section.size() != 0 {
            sections.push(Section {
                name: section.name()?.to_owned(),
                address: section.address(),
                size: section.size(),
                executable: sh_flags.contains(object::elf::SHF_EXECINSTR),
            });
        }
    }
    let vector_table = match file.section_by_name(".vector_table") {
        Some(section) => section.data()?.to_vec(),
        None => Vec::new(),
    };
    let elf = object::read::elf::ElfFile32::<object::Endianness>::parse(bytes)?;
    let endian = elf.endian();
    let mut loads = Vec::new();
    for segment in elf.elf_program_headers() {
        if segment.p_type(endian) == object::elf::PT_LOAD && segment.p_filesz(endian) != 0 {
            loads.push(Load {
                address: u64::from(segment.p_paddr(endian)),
                bytes: u64::from(segment.p_filesz(endian)),
                offset: u64::from(segment.p_offset(endian)),
                writable_or_executable: segment
                    .p_flags(endian)
                    .intersects(object::elf::PF_W | object::elf::PF_X),
            });
        }
    }
    let symbol = |name: &str| {
        file.symbols()
            .find(|s| s.name().ok() == Some(name))
            .map(|s| s.address())
    };
    let image = Image {
        arm32: file.architecture() == object::Architecture::Arm
            && !file.is_64()
            && file.is_little_endian(),
        hard_float: e_flags.contains(object::elf::FileFlags(object::elf::EF_ARM_ABI_FLOAT_HARD)),
        entry: file.entry(),
        sections,
        vector_table,
        loads,
        stack_start: symbol("_stack_start"),
        stack_end: symbol("_stack_end"),
        load_end: symbol("__firmware_load_end"),
        file_bytes: bytes.len() as u64,
    };
    let mut report = check(board, &image)?;
    report["firmware_sha256"] = crate::portability::digest(bytes).into();
    Ok(report)
}

/// Records a flash load. It lies in sector 0 or in the firmware sectors, and no other load
/// overlaps it.
fn flash_load(loads: &mut Vec<(u64, u64)>, start: u64, length: u64) -> Result<()> {
    ensure!(
        contained(start, length, &VECTOR) || contained(start, length, &CODE),
        "ELF load at {start:#x} + {length} overlaps application/persistence or leaves flash"
    );
    for &(previous, previous_length) in loads.iter() {
        ensure!(
            start + length <= previous || previous + previous_length <= start,
            "overlapping flash load segments"
        );
    }
    loads.push((start, length));
    Ok(())
}

/// The bytes of the firmware sectors that a load ending at `end` spans.
fn firmware_span(end: u64) -> Result<u64> {
    ensure!(
        (CODE.start..=CODE.end).contains(&end),
        "firmware load end leaves the executable region"
    );
    Ok(end - CODE.start)
}

/// Applies the policy to one image and reports what it holds. The checks run in this order: the
/// file, the vector table and stack, the code, the flash loads, then the sections.
fn check(board: &BoardProfile, image: &Image) -> Result<Value> {
    ensure!(image.arm32, "firmware must be ARM32 little-endian ELF");
    ensure!(image.hard_float, "firmware lacks hard-float ABI flag");
    ensure!(
        CODE.contains(&(image.entry & !1)),
        "reset entry is outside executable partition"
    );
    let vectors = image
        .section(".vector_table")
        .context("missing vector table")?;
    ensure!(
        vectors.address == VECTOR.start && contained(vectors.address, vectors.size, &VECTOR),
        "vector table exceeds sector 0"
    );
    let table = &image.vector_table;
    ensure!(table.len() >= 8, "truncated vector table");
    let initial_sp = u32::from_le_bytes(table[..4].try_into()?);
    ensure!(
        u64::from(initial_sp) == INITIAL_SP,
        "unexpected initial stack pointer"
    );
    let reset_vector = u32::from_le_bytes(table[4..8].try_into()?);
    ensure!(
        reset_vector & 1 == 1 && u64::from(reset_vector) == image.entry,
        "reset vector differs from Thumb ELF entry"
    );
    let stack_start = image
        .stack_start
        .context("missing linker symbol _stack_start")?;
    let stack_end = image
        .stack_end
        .context("missing linker symbol _stack_end")?;
    ensure!(
        stack_end == STACK_END
            && stack_start == INITIAL_SP
            && stack_start - stack_end == board.stack_window_bytes,
        "linker did not reserve the main stack region {STACK_END:#x}..{INITIAL_SP:#x}"
    );
    let code_end = image
        .load_end
        .context("missing linker symbol __firmware_load_end")?;
    let code_span = firmware_span(code_end)?;
    let code_free_bytes = CODE.end - code_end;
    let code = image
        .section(".text")
        .context("missing executable firmware section")?;
    ensure!(
        code.address == CODE.start
            && contained(code.address, code.size, &CODE)
            && (code.address..code.address + code.size).contains(&(image.entry & !1)),
        "reset handler is outside executable firmware section"
    );
    ensure!(
        code.executable,
        "firmware text is not allocated executable code"
    );
    let mut placed = Vec::new();
    let mut flash_bytes = 0_u64;
    for load in &image.loads {
        flash_load(&mut placed, load.address, load.bytes)?;
        if VECTOR.contains(&load.address) {
            ensure!(
                !load.writable_or_executable,
                "sector-0 load segment is writable or executable"
            );
        } else {
            ensure!(
                load.address + load.bytes <= code_end,
                "firmware load exceeds recorded upper end"
            );
        }
        ensure!(
            load.offset
                .checked_add(load.bytes)
                .is_some_and(|end| end <= image.file_bytes),
            "ELF load extends beyond file"
        );
        flash_bytes += load.bytes;
    }
    ensure!(!placed.is_empty(), "firmware has no flash load segments");
    let mut sections = Vec::new();
    let mut ram_bytes = 0_u64;
    for section in &image.sections {
        let (start, size) = (section.address, section.size);
        if SRAM.contains(&start) {
            ensure!(
                contained(start, size, &STATIC_RAM),
                "static section overlaps MSP reserve"
            );
            ram_bytes += size;
        } else {
            ensure!(
                contained(start, size, &VECTOR) || contained(start, size, &CODE),
                "allocated section outside firmware regions"
            );
            if VECTOR.contains(&start) {
                ensure!(
                    section.name == ".vector_table",
                    "unexpected allocated section in vector/data sector"
                );
            } else {
                ensure!(
                    start + size <= code_end,
                    "section exceeds recorded upper end"
                );
            }
        }
        sections.push(json!({"name":section.name,"address":start,"bytes":size}));
    }
    ensure!(
        (board.heap_bytes..=STATIC_RAM.end - STATIC_RAM.start).contains(&ram_bytes),
        "static RAM does not match heap/static reservation"
    );
    Ok(json!({
        "flash_load_bytes": flash_bytes,
        "static_ram_bytes_including_heap": ram_bytes,
        "other_static_ram_bytes": ram_bytes - board.heap_bytes,
        "heap_reserved_bytes": board.heap_bytes,
        "stack_reserved_bytes": board.stack_window_bytes,
        "application_sector_bytes": APPLICATION.end - APPLICATION.start,
        "persistence_reserved_bytes": PERSISTENCE.end - PERSISTENCE.start,
        "firmware_code_capacity_bytes": CODE.end - CODE.start,
        "firmware_code_span_bytes": code_span,
        "firmware_code_free_bytes": code_free_bytes,
        "firmware_code_growth_target_bytes": FLASH_GROWTH_TARGET,
        "firmware_code_growth_target_met": meets_flash_growth_target(code_free_bytes),
        "sector0_span_bytes": vectors.size,
        "sector0_free_bytes": VECTOR.end - vectors.address - vectors.size,
        "load_segments": placed,
        "sections": sections,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> BoardProfile {
        BoardProfile::stm32f407g_disc1().unwrap()
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

    /// A representative layout, not a linked build: the vector table in sector 0; 256 KiB of code
    /// and 64 KiB of read-only data from sector 4; the initial values of `.data` after them; and
    /// 1 KiB of `.data` with the 72 KiB allocator arena and 1 KiB of other `.bss` in SRAM.
    fn image() -> Image {
        Image {
            arm32: true,
            hard_float: true,
            entry: 0x0801_0101,
            sections: vec![
                section(".vector_table", 0x0800_0000, 0x188, false),
                section(".text", 0x0801_0000, 0x4_0000, true),
                section(".rodata", 0x0805_0000, 0x1_0000, false),
                section(".data", 0x2000_0000, 0x400, false),
                section(".bss", 0x2000_0400, 0x1_2400, false),
            ],
            vector_table: vector_table(0x2002_0000, 0x0801_0101),
            loads: vec![
                load(0x0800_0000, 0x188, 0x1000, false),
                load(0x0801_0000, 0x5_0000, 0x2000, true),
                load(0x0806_0000, 0x400, 0x5_2000, true),
            ],
            stack_start: Some(0x2002_0000),
            stack_end: Some(0x2001_C000),
            load_end: Some(0x0806_0400),
            file_bytes: 0x6_0000,
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
    fn the_sector_map_gives_the_regions_and_the_capacities_of_the_profile() {
        assert_eq!(SECTOR_KIB.iter().sum::<u64>(), 1024);
        let starts = [
            0x0800_0000,
            0x0800_4000,
            0x0800_8000,
            0x0800_C000,
            0x0801_0000,
            0x0802_0000,
            0x0804_0000,
            0x0806_0000,
            0x0808_0000,
            0x080A_0000,
            0x080C_0000,
            0x080E_0000,
            0x0810_0000,
        ];
        for (sector, start) in starts.into_iter().enumerate() {
            assert_eq!(sector_address(sector), start, "sector {sector}");
        }
        assert_eq!(VECTOR, 0x0800_0000..0x0800_4000);
        assert_eq!(APPLICATION, 0x0800_4000..0x0800_8000);
        assert_eq!(PERSISTENCE, 0x0800_8000..0x0801_0000);
        assert_eq!(CODE, 0x0801_0000..0x0810_0000);
        let board = board();
        assert_eq!(CODE.end - CODE.start, board.image_capacity_bytes);
        assert_eq!(APPLICATION.end - APPLICATION.start, board.application_bytes);
        assert_eq!(PERSISTENCE.end - PERSISTENCE.start, 32 * 1024);
        assert_eq!(CODE.end - FLASH_START, board.flash_kib * 1024);
        assert_eq!(SRAM.end - SRAM.start, 128 * 1024);
        assert_eq!(SRAM.end - STACK_END, board.stack_window_bytes);
        assert_eq!(INITIAL_SP, 0x2002_0000);
    }

    #[test]
    fn the_inspected_regions_are_the_layout_and_memory_of_the_chip_file() {
        let chip = crate::portability::board::chip("stm32f407").unwrap();
        let span = |region: trust_chips::Region| u64::from(region.start)..region.end();
        let layout = &chip.layout;
        assert_eq!(span(layout.vectors.unwrap()), VECTOR);
        assert_eq!(span(layout.application), APPLICATION);
        assert_eq!(
            u64::from(layout.checkpoint_a.start)..layout.checkpoint_b.end(),
            PERSISTENCE
        );
        assert_eq!(span(layout.firmware), CODE);
        let flash = chip.memory.flash.as_ref().unwrap();
        assert_eq!(u64::from(flash.start), FLASH_START);
        let sectors: Vec<u64> = flash
            .sectors
            .iter()
            .flat_map(|run| {
                std::iter::repeat_n(u64::from(run.kib), usize::try_from(run.count).unwrap())
            })
            .collect();
        assert_eq!(sectors, SECTOR_KIB);
        // SRAM1 and SRAM2 are the one block of 128 KiB; the CCM is the third region.
        let ram = &chip.memory.ram;
        assert_eq!(u64::from(ram[0].start)..ram[1].end(), SRAM);
        assert_eq!((ram[2].name.as_str(), ram[2].dma), ("CCM", false));
    }

    #[test]
    fn the_linker_scripts_state_the_layout_that_is_inspected() {
        let memory = include_str!("../../../../firmware/trust-stm32f407/memory.x");
        let profile = include_str!("../../../../firmware/trust-stm32f407/profile.x");
        for expected in [
            format!("FLASH : ORIGIN = {FLASH_START:#010X}, LENGTH = 1024K"),
            format!("RAM : ORIGIN = {:#010X}, LENGTH = 128K", SRAM.start),
            format!("_stext = {:#010X};", CODE.start),
            format!("_stack_start = {INITIAL_SP:#010X};"),
            format!("_stack_end = {STACK_END:#010X};"),
        ] {
            assert!(memory.contains(&expected), "memory.x lacks {expected}");
        }
        for expected in [
            "SIZEOF(.vector_table) <= 16K".to_owned(),
            format!("ADDR(.text) >= {:#010X}", CODE.start),
            "_stack_start - _stack_end == 16K".to_owned(),
            format!("__firmware_load_end <= {:#010X}", CODE.end),
        ] {
            assert!(profile.contains(&expected), "profile.x lacks {expected}");
        }
        assert_eq!(VECTOR.end - VECTOR.start, 16 * 1024);
        assert_eq!(board().stack_window_bytes, 16 * 1024);
        // No firmware data in sector 0, where the retired F401 firmware placed its constants.
        assert!(!memory.contains(".firmware_rodata"));
        assert!(!profile.contains("__firmware_rodata"));
    }

    #[test]
    fn a_representative_layout_is_accepted_and_every_region_is_reported() {
        let report = accepted(|_| {});
        assert_eq!(report["flash_load_bytes"], 0x188 + 0x5_0000 + 0x400);
        assert_eq!(report["static_ram_bytes_including_heap"], 75_776);
        assert_eq!(report["other_static_ram_bytes"], 2048);
        assert_eq!(report["heap_reserved_bytes"], 72 * 1024);
        assert_eq!(report["stack_reserved_bytes"], 16 * 1024);
        assert_eq!(report["application_sector_bytes"], 16 * 1024);
        assert_eq!(report["persistence_reserved_bytes"], 32 * 1024);
        assert_eq!(report["firmware_code_capacity_bytes"], 960 * 1024);
        assert_eq!(report["firmware_code_span_bytes"], 0x5_0400);
        assert_eq!(report["firmware_code_free_bytes"], 960 * 1024 - 0x5_0400);
        assert_eq!(report["firmware_code_growth_target_bytes"], 16 * 1024);
        assert_eq!(report["firmware_code_growth_target_met"], true);
        assert_eq!(report["sector0_span_bytes"], 0x188);
        assert_eq!(report["sector0_free_bytes"], 16 * 1024 - 0x188);
        assert_eq!(
            report["load_segments"],
            serde_json::json!([
                [0x0800_0000, 0x188],
                [0x0801_0000, 0x5_0000],
                [0x0806_0000, 0x400]
            ])
        );
        assert_eq!(report["sections"].as_array().unwrap().len(), 5);
        assert_eq!(report["sections"][1]["name"], ".text");
        assert_eq!(report["sections"][1]["address"], 0x0801_0000);
    }

    #[test]
    fn an_image_must_be_arm32_with_the_hard_float_abi() {
        assert!(rejected(|image| image.arm32 = false).contains("ARM32"));
        assert!(rejected(|image| image.hard_float = false).contains("hard-float"));
    }

    #[test]
    fn the_reset_entry_must_lie_in_the_firmware_sectors() {
        // Sector 2 holds a checkpoint slot; 0x0810_0000 is the first byte past the flash.
        for entry in [0x0800_0101, 0x0800_8001, 0x0810_0001] {
            let error = rejected(|image| image.entry = entry);
            assert!(error.contains("reset entry"), "{entry:#x}: {error}");
        }
        // The first and the last Thumb addresses of the code.
        for entry in [0x0801_0001_u32, 0x0804_FFFF] {
            accepted(|image| {
                image.entry = u64::from(entry);
                image.vector_table = vector_table(0x2002_0000, entry);
            });
        }
    }

    #[test]
    fn the_vector_table_must_fill_no_more_than_sector_0() {
        let error = rejected(|image| image.sections.retain(|s| s.name != ".vector_table"));
        assert!(error.contains("missing vector table"), "{error}");
        let error = rejected(|image| image.sections[0].address = 0x0800_0100);
        assert!(error.contains("exceeds sector 0"), "{error}");
        let error = rejected(|image| image.sections[0].size = 0x4001);
        assert!(error.contains("exceeds sector 0"), "{error}");
        let report = accepted(|image| image.sections[0].size = 0x4000);
        assert_eq!(report["sector0_free_bytes"], 0);
        let error = rejected(|image| image.vector_table.truncate(7));
        assert!(error.contains("truncated"), "{error}");
    }

    #[test]
    fn the_initial_stack_pointer_and_the_reset_vector_are_pinned() {
        // The F401's initial stack pointer is the top of its 96 KiB of SRAM.
        let error = rejected(|image| image.vector_table = vector_table(0x2001_8000, 0x0801_0101));
        assert!(error.contains("initial stack pointer"), "{error}");
        let error = rejected(|image| image.vector_table = vector_table(0x2002_0004, 0x0801_0101));
        assert!(error.contains("initial stack pointer"), "{error}");
        // The reset vector must be a Thumb address, and the ELF entry.
        let error = rejected(|image| image.vector_table = vector_table(0x2002_0000, 0x0801_0100));
        assert!(error.contains("reset vector"), "{error}");
        let error = rejected(|image| image.vector_table = vector_table(0x2002_0000, 0x0801_0201));
        assert!(error.contains("reset vector"), "{error}");
    }

    #[test]
    fn the_main_stack_is_the_reserved_window_at_the_top_of_sram() {
        let error = rejected(|image| image.stack_start = None);
        assert!(error.contains("_stack_start"), "{error}");
        let error = rejected(|image| image.stack_end = None);
        assert!(error.contains("_stack_end"), "{error}");
        // The F401's window, a window of 8 KiB, and a start that is not the initial stack pointer.
        for (start, end) in [
            (0x2001_8000, 0x2001_4000),
            (0x2002_0000, 0x2001_E000),
            (0x2001_F000, 0x2001_C000),
        ] {
            let error = rejected(|image| {
                image.stack_start = Some(start);
                image.stack_end = Some(end);
            });
            assert!(error.contains("did not reserve"), "{start:#x}: {error}");
        }
    }

    #[test]
    fn the_load_end_stays_inside_the_firmware_sectors_and_the_growth_target_is_report_only() {
        let error = rejected(|image| image.load_end = None);
        assert!(error.contains("__firmware_load_end"), "{error}");
        for end in [CODE.start - 1, CODE.end + 1, u64::MAX] {
            let error = rejected(|image| image.load_end = Some(end));
            assert!(
                error.contains("leaves the executable region"),
                "{end:#x}: {error}"
            );
        }
        let full = accepted(|image| image.load_end = Some(CODE.end));
        assert_eq!(full["firmware_code_free_bytes"], 0);
        assert_eq!(full["firmware_code_growth_target_met"], false);
        let target = accepted(|image| image.load_end = Some(CODE.end - FLASH_GROWTH_TARGET));
        assert_eq!(target["firmware_code_growth_target_met"], true);
        let below = accepted(|image| image.load_end = Some(CODE.end - FLASH_GROWTH_TARGET + 1));
        assert_eq!(below["firmware_code_free_bytes"], FLASH_GROWTH_TARGET - 1);
        assert_eq!(below["firmware_code_growth_target_met"], false);
    }

    #[test]
    fn the_code_starts_the_firmware_sectors_and_holds_the_reset_handler() {
        let error = rejected(|image| image.sections.retain(|s| s.name != ".text"));
        assert!(error.contains("missing executable"), "{error}");
        let error = rejected(|image| image.sections[1].address = 0x0801_0020);
        assert!(
            error.contains("outside executable firmware section"),
            "{error}"
        );
        // The entry is in the firmware sectors but in the read-only data, not in the code.
        let error = rejected(|image| {
            image.entry = 0x0805_0001;
            image.vector_table = vector_table(0x2002_0000, 0x0805_0001);
        });
        assert!(
            error.contains("outside executable firmware section"),
            "{error}"
        );
        let error = rejected(|image| image.sections[1].executable = false);
        assert!(error.contains("not allocated executable code"), "{error}");
    }

    #[test]
    fn flash_loads_stay_in_sector_0_and_the_firmware_sectors_without_overlap() {
        // The application sector, a checkpoint slot, a load that spans a sector boundary into
        // them, one past the end of flash, and sector 0 grown past its 16 KiB.
        for (address, bytes) in [
            (0x0800_4000, 16),
            (0x0800_8000, 16),
            (0x0800_FFF0, 0x20),
            (0x0800_3FF0, 0x20),
            (0x080F_FFFC, 8),
            (0x0800_0000, 0x4001),
            (u64::MAX, 2),
        ] {
            let error = rejected(|image| image.loads.push(load(address, bytes, 0x100, false)));
            assert!(
                error.contains("overlaps application/persistence or leaves flash"),
                "{address:#x} + {bytes}: {error}"
            );
        }
        let error = rejected(|image| image.loads.push(load(0x0801_0100, 16, 0x100, true)));
        assert!(error.contains("overlapping flash load segments"), "{error}");
        let error = rejected(|image| image.loads.clear());
        assert!(error.contains("no flash load segments"), "{error}");
    }

    #[test]
    fn the_vector_segment_is_read_only_and_the_other_loads_end_at_the_recorded_end() {
        let error = rejected(|image| image.loads[0].writable_or_executable = true);
        assert!(error.contains("writable or executable"), "{error}");
        // The data's initial values load after the code, so the recorded end must cover them.
        let error = rejected(|image| image.load_end = Some(0x0806_0000));
        assert!(error.contains("exceeds recorded upper end"), "{error}");
        let error = rejected(|image| image.file_bytes = 0x5_2000);
        assert!(error.contains("beyond file"), "{error}");
        accepted(|image| image.file_bytes = 0x5_2400);
    }

    #[test]
    fn sections_outside_the_firmware_regions_and_the_static_ram_are_refused() {
        // The CCM is unassigned, the application sector and a checkpoint slot are never loaded.
        for (name, address) in [
            (".ccmram", 0x1000_0000),
            (".stray", 0x0800_4000),
            (".stray", 0x0800_8000),
            (".stray", 0x0810_0000),
            (".stray", 0x4000_0000),
        ] {
            let error = rejected(|image| image.sections.push(section(name, address, 4, false)));
            assert!(
                error.contains("outside firmware regions"),
                "{address:#x}: {error}"
            );
        }
        // Sector 0 holds the vector table alone; the retired F401 kept its constants there.
        let error = rejected(|image| {
            image
                .sections
                .push(section(".firmware_rodata", 0x0800_0200, 0x100, false));
        });
        assert!(error.contains("unexpected allocated section"), "{error}");
        let error = rejected(|image| {
            image
                .sections
                .push(section(".rodata2", 0x0806_0000, 0x1000, false));
        });
        assert!(error.contains("exceeds recorded upper end"), "{error}");
    }

    #[test]
    fn statics_stay_below_the_main_stack_window() {
        // A section that starts in the window, and one that runs into it.
        for (address, size) in [(0x2001_C000, 4), (0x2001_BFF0, 0x20), (0x2001_FFFC, 4)] {
            let error = rejected(|image| {
                image.sections.push(section(".late", address, size, false));
            });
            assert!(
                error.contains("overlaps MSP reserve"),
                "{address:#x}: {error}"
            );
        }
        // The statics may fill the whole 112 KiB below it and not one byte more.
        let report = accepted(|image| image.sections[4].size = 0x1_BC00);
        assert_eq!(report["static_ram_bytes_including_heap"], 112 * 1024);
        let error = rejected(|image| image.sections[4].size = 0x1_BC01);
        assert!(error.contains("overlaps MSP reserve"), "{error}");
    }

    #[test]
    fn the_static_ram_holds_at_least_the_allocator_arena() {
        let error = rejected(|image| image.sections.retain(|s| s.name != ".bss"));
        assert!(error.contains("static RAM does not match"), "{error}");
        let report = accepted(|image| image.sections[4].size = 0x1_1C00);
        assert_eq!(report["static_ram_bytes_including_heap"], 72 * 1024);
        assert_eq!(report["other_static_ram_bytes"], 0);
        let error = rejected(|image| image.sections[4].size = 0x1_1BFF);
        assert!(error.contains("static RAM does not match"), "{error}");
    }

    #[test]
    fn something_that_is_not_an_elf_is_refused() {
        assert!(inspect(&board(), b"not a linked firmware image").is_err());
        assert!(inspect(&board(), &[]).is_err());
    }
}
