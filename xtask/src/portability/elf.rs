use super::board::{Board, BoardProfile};
use anyhow::{ensure, Context, Result};
use object::read::elf::ProgramHeader;
use object::{Object, ObjectSection, ObjectSymbol};
use serde_json::{json, Value};

mod stm32f407;

const FLASH_GROWTH_TARGET: u64 = 16 * 1024;

fn contained(start: u64, bytes: u64, region: &std::ops::Range<u64>) -> bool {
    start >= region.start
        && start
            .checked_add(bytes)
            .is_some_and(|end| end <= region.end)
}

fn meets_flash_growth_target(free_bytes: u64) -> bool {
    free_bytes >= FLASH_GROWTH_TARGET
}

/// Inspects a linked image with the policy of its board, and records the board in the report.
pub(super) fn inspect(board: &BoardProfile, bytes: &[u8]) -> Result<Value> {
    let mut report = match board.board {
        Board::Stm32f407gDisc1 => stm32f407::inspect(board, bytes)?,
        Board::Esp32c6Devkitc1 => inspect_esp32c6(board, bytes)?,
    };
    report["board"] = board.name.into();
    Ok(report)
}

// ESP32-C6 windows from esp-hal 1.2.2 (`ld/esp32c6/memory.x`): flash-mapped code and read-only
// data, HP SRAM, and RTC fast memory. Every allocated section and load segment must lie in one.
const C6_FLASH: std::ops::Range<u64> = 0x4200_0000..0x4280_0000;
const C6_HP_SRAM: std::ops::Range<u64> = 0x4080_0000..0x4088_0000;
/// `_stack_start` in esp-hal 1.2.2's C6 memory map; the composition's fixed stack window ends here.
const C6_STACK_TOP: u64 = 0x4086_E610;
/// The C6's main-stack window: the 16 KiB the composition budgets at the top of the esp-hal stack
/// region (specification 34, D.2); `firmware/trust-esp32c6-reference/src/memory.rs` pins the same.
pub(super) const C6_STACK_WINDOW_BYTES: u64 = 16 * 1024;
/// The margin above the window the C6 build requires. Below it the boot-time stack paint reaches
/// static data, as the S1b-5c firmware did (specification 34, D.2, the S1b-5d guard).
pub(super) const C6_STACK_MARGIN_MINIMUM_BYTES: u64 = 4 * 1024;
const C6_RTC_FAST: std::ops::Range<u64> = 0x5000_0000..0x5000_4000;
// e_machine and e_flags values of the RISC-V ELF psABI.
const EM_RISCV: u16 = 243;
const EF_RISCV_RVC: u32 = 0x1;
const EF_RISCV_FLOAT_ABI: u32 = 0x6;
const EF_RISCV_RVE: u32 = 0x8;

/// What the ESP32-C6 policy reads from one linked ELF32 image.
struct Esp32c6Image {
    riscv32: bool,
    e_flags: u32,
    entry: u64,
    /// Allocated, non-empty sections: name, address, size.
    sections: Vec<(String, u64, u64)>,
    /// Loadable segments with file bytes: load address, file size.
    loads: Vec<(u64, u64)>,
    stack_start: Option<u64>,
    stack_end: Option<u64>,
    /// Size of the firmware's allocator arena, when the symbol is present.
    heap_arena_bytes: Option<u64>,
    /// Addresses of every copy of the VM dispatch loop (spec 34 D.2: it executes from HP SRAM).
    dispatch_addresses: Vec<u64>,
}

/// The VM dispatch loop's v0-mangled path; the length prefixes keep the match exact.
const C6_DISPATCH_SYMBOL: &str = "18trust_runtime_core2vm8dispatch20execute_with_buffers";

fn c6_window(start: u64, bytes: u64) -> Option<&'static str> {
    [
        ("flash", C6_FLASH),
        ("hp_sram", C6_HP_SRAM),
        ("rtc_fast", C6_RTC_FAST),
    ]
    .into_iter()
    .find(|(_, window)| contained(start, bytes, window))
    .map(|(name, _)| name)
}

/// The allocated, non-empty sections of a linked image: name, address and byte length, in the
/// image's own order.
pub(super) fn allocated_sections(bytes: &[u8]) -> Result<Vec<(String, u64, u64)>> {
    let file = object::File::parse(bytes).context("decode firmware ELF")?;
    let mut sections = Vec::new();
    for section in file.sections() {
        let object::SectionFlags::Elf { sh_flags, .. } = section.flags() else {
            continue;
        };
        if sh_flags.contains(object::elf::SHF_ALLOC) && section.size() != 0 {
            sections.push((
                section.name()?.to_owned(),
                section.address(),
                section.size(),
            ));
        }
    }
    Ok(sections)
}

/// The ESP32-C6 stack region from a linked image's section table, and its margin above the window.
///
/// The region is `.stack`, which runs from `_bss_end` to the composition's fixed stack top. The
/// build requires it to hold the 16 KiB window plus at least 4 KiB of margin, the same rule the
/// linker guard asserts (specification 34, D.2); a smaller region lets the boot-time stack paint
/// reach static data, as the S1b-5c firmware did. `cargo xtask portability build-firmware esp32c6`
/// refuses the image here, so the check catches a regression even if the linker script is bypassed.
pub(super) fn c6_stack_region(sections: &[(String, u64, u64)]) -> Result<Value> {
    let (_, start, bytes) = sections
        .iter()
        .find(|(name, ..)| name == ".stack")
        .context("linked image has no .stack region")?;
    let end = start
        .checked_add(*bytes)
        .context("stack region overflows the address space")?;
    ensure!(
        end == C6_STACK_TOP,
        "the .stack region ends at {end:#x}, not the composition's fixed stack top {C6_STACK_TOP:#x}"
    );
    let margin_bytes = bytes.saturating_sub(C6_STACK_WINDOW_BYTES);
    ensure!(
        margin_bytes >= C6_STACK_MARGIN_MINIMUM_BYTES,
        "the stack region of {bytes} bytes leaves {margin_bytes} bytes above the {C6_STACK_WINDOW_BYTES} byte stack window, less than the {C6_STACK_MARGIN_MINIMUM_BYTES} byte margin the build requires"
    );
    Ok(json!({
        "region_start": start,
        "region_end": end,
        "region_bytes": bytes,
        "window_bytes": C6_STACK_WINDOW_BYTES,
        "margin_bytes": margin_bytes,
        "margin_minimum_bytes": C6_STACK_MARGIN_MINIMUM_BYTES,
    }))
}

/// The ESP32-C6 image policy. The ELF is read into `Esp32c6Image`; the checks are in
/// `check_esp32c6`, so each rule can be tested on its own.
fn inspect_esp32c6(board: &BoardProfile, bytes: &[u8]) -> Result<Value> {
    let file = object::File::parse(bytes).context("decode firmware ELF")?;
    // ELF32 header fields by byte offset: e_machine at 18 and e_flags at 36.
    let e_machine = u16::from_le_bytes(
        bytes
            .get(18..20)
            .context("truncated ELF header")?
            .try_into()?,
    );
    let e_flags = u32::from_le_bytes(
        bytes
            .get(36..40)
            .context("truncated ELF header")?
            .try_into()?,
    );
    let sections = allocated_sections(bytes)?;
    let elf = object::read::elf::ElfFile32::<object::Endianness>::parse(bytes)?;
    let endian = elf.endian();
    let mut loads = Vec::new();
    for segment in elf.elf_program_headers() {
        if segment.p_type(endian) == object::elf::PT_LOAD && segment.p_filesz(endian) != 0 {
            loads.push((
                u64::from(segment.p_paddr(endian)),
                u64::from(segment.p_filesz(endian)),
            ));
        }
    }
    let symbol = |name: &str| {
        file.symbols()
            .find(|s| s.name().ok() == Some(name))
            .map(|s| s.address())
    };
    let image = Esp32c6Image {
        riscv32: e_machine == EM_RISCV
            && file.architecture() == object::Architecture::Riscv32
            && !file.is_64()
            && file.is_little_endian(),
        e_flags,
        entry: file.entry(),
        sections,
        loads,
        stack_start: symbol("_stack_start"),
        stack_end: symbol("_stack_end"),
        heap_arena_bytes: file
            .symbols()
            .find(|s| s.name().is_ok_and(|name| name.contains("HEAP_MEMORY")))
            .map(|s| s.size()),
        dispatch_addresses: file
            .symbols()
            .filter(|s| s.name().is_ok_and(|name| name.contains(C6_DISPATCH_SYMBOL)))
            .map(|s| s.address())
            .collect(),
    };
    let mut report = check_esp32c6(board, &image)?;
    report["firmware_sha256"] = super::digest(bytes).into();
    Ok(report)
}

fn check_esp32c6(board: &BoardProfile, image: &Esp32c6Image) -> Result<Value> {
    ensure!(
        image.riscv32,
        "firmware must be a 32-bit little-endian RISC-V ELF"
    );
    ensure!(
        image.e_flags & EF_RISCV_RVC != 0,
        "firmware lacks the RVC flag"
    );
    ensure!(
        image.e_flags & EF_RISCV_FLOAT_ABI == 0,
        "firmware is not built for the soft-float ABI"
    );
    ensure!(
        image.e_flags & EF_RISCV_RVE == 0,
        "firmware is RV32E, not RV32I"
    );
    let mut hp_sram_static_bytes = 0_u64;
    let mut rtc_fast_bytes = 0_u64;
    let mut sections = Vec::new();
    for (name, address, size) in &image.sections {
        let window = c6_window(*address, *size).with_context(|| {
            format!("allocated section {name} at {address:#x} lies outside every ESP32-C6 window")
        })?;
        // The stack is reported as its own region below, not as static RAM.
        if window == "hp_sram" && name.as_str() != ".stack" {
            hp_sram_static_bytes += *size;
        }
        if window == "rtc_fast" {
            rtc_fast_bytes += *size;
        }
        sections.push(json!({
            "name": name, "address": *address, "bytes": *size, "window": window
        }));
    }
    let mut image_load_bytes = 0_u64;
    let mut flash_mapped_bytes = 0_u64;
    let mut segments = Vec::new();
    for (address, bytes) in &image.loads {
        let window = c6_window(*address, *bytes).with_context(|| {
            format!("load segment at {address:#x} lies outside every ESP32-C6 window")
        })?;
        image_load_bytes += *bytes;
        if window == "flash" {
            flash_mapped_bytes += *bytes;
        }
        segments.push(json!({"address": *address, "bytes": *bytes, "window": window}));
    }
    let mut ordered = image.loads.clone();
    ordered.sort_unstable();
    ensure!(
        ordered
            .windows(2)
            .all(|pair| pair[0].0 + pair[0].1 <= pair[1].0),
        "load segments overlap"
    );
    ensure!(
        image_load_bytes <= board.image_capacity_bytes,
        "firmware image of {image_load_bytes} bytes exceeds the {} byte factory partition",
        board.image_capacity_bytes
    );
    let start = image
        .stack_start
        .context("missing linker symbol _stack_start")?;
    let end = image
        .stack_end
        .context("missing linker symbol _stack_end")?;
    ensure!(end < start, "stack symbols are not ordered");
    ensure!(
        start == C6_STACK_TOP,
        "_stack_start {start:#x} is not the composition's fixed stack top {C6_STACK_TOP:#x}"
    );
    let stack_region_bytes = start - end;
    ensure!(
        stack_region_bytes >= board.stack_window_bytes + board.stack_headroom_bytes,
        "stack region of {stack_region_bytes} bytes is smaller than the {} byte window plus {} byte headroom",
        board.stack_window_bytes,
        board.stack_headroom_bytes
    );
    ensure!(
        c6_window(end, stack_region_bytes) == Some("hp_sram"),
        "stack region lies outside HP SRAM"
    );
    // From flash the interpreter stalled on code fetches; scan-time code must execute from RAM.
    ensure!(
        !image.dispatch_addresses.is_empty(),
        "the VM dispatch loop symbol is missing from the image"
    );
    ensure!(
        image
            .dispatch_addresses
            .iter()
            .all(|&address| contained(address, 1, &C6_HP_SRAM)),
        "the VM dispatch loop does not execute from HP SRAM"
    );
    let ram_code_bytes = image
        .sections
        .iter()
        .find(|(name, ..)| name == ".rwtext")
        .map_or(0, |&(_, _, size)| size);
    Ok(json!({
        "entry": image.entry,
        "e_flags": image.e_flags,
        "image_load_bytes": image_load_bytes,
        "factory_partition_bytes": board.image_capacity_bytes,
        "factory_free_bytes": board.image_capacity_bytes - image_load_bytes,
        "flash_mapped_bytes": flash_mapped_bytes,
        "hp_sram_static_bytes": hp_sram_static_bytes,
        "rtc_fast_bytes": rtc_fast_bytes,
        "stack_start": start,
        "stack_end": end,
        "stack_region_bytes": stack_region_bytes,
        "stack_window_bytes": board.stack_window_bytes,
        "stack_headroom_minimum_bytes": board.stack_headroom_bytes,
        "heap_arena_bytes": image.heap_arena_bytes,
        "ram_code_bytes": ram_code_bytes,
        "dispatch_loop_in_hp_sram": true,
        "application_partition_bytes": board.application_bytes,
        "load_segments": segments,
        "sections": sections,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The measured layout of the C6 probe image: flash code and data, RAM segments loaded from
    /// the image, and the esp-hal stack region that ends at `_stack_start`.
    fn c6_image() -> Esp32c6Image {
        Esp32c6Image {
            riscv32: true,
            e_flags: EF_RISCV_RVC,
            entry: 0x4201_0072,
            sections: vec![
                (".flash.appdesc".into(), 0x4200_0020, 0x100),
                (".rodata".into(), 0x4200_0120, 0x508),
                (".text_gap".into(), 0x4200_0628, 0xf9f8),
                (".text".into(), 0x4201_0020, 0x55a6),
                (".trap".into(), 0x4080_0000, 0x63c),
                (".data".into(), 0x4080_0be8, 0x29c),
                (".bss".into(), 0x4080_0e84, 0x4254),
                (".stack".into(), 0x4080_50d8, 0x69538),
                (".rtc_fast.persistent".into(), 0x5000_0000, 8),
            ],
            loads: vec![
                (0x4080_0000, 0xbe8),
                (0x4080_0be8, 0x29c),
                (0x4200_0020, 0x608),
                (0x4201_0020, 0x55a6),
            ],
            stack_start: Some(0x4086_e610),
            stack_end: Some(0x4080_50d8),
            heap_arena_bytes: None,
            dispatch_addresses: vec![0x4080_0100],
        }
    }

    #[test]
    fn c6_requires_the_fixed_stack_top_the_composition_measures() {
        let board = BoardProfile::esp32c6_devkitc_1().unwrap();
        let mut image = c6_image();
        image.stack_start = Some(C6_STACK_TOP + 0x100);
        let error = check_esp32c6(&board, &image).unwrap_err().to_string();
        assert!(error.contains("fixed stack top"), "{error}");
    }

    #[test]
    fn c6_requires_the_dispatch_loop_in_hp_sram() {
        let board = BoardProfile::esp32c6_devkitc_1().unwrap();
        let mut image = c6_image();
        image.dispatch_addresses = vec![0x4080_0100, 0x4201_0400];
        let error = check_esp32c6(&board, &image).unwrap_err().to_string();
        assert!(error.contains("does not execute from HP SRAM"), "{error}");
        image.dispatch_addresses.clear();
        let error = check_esp32c6(&board, &image).unwrap_err().to_string();
        assert!(error.contains("missing"), "{error}");
        image.dispatch_addresses = vec![0x4080_0100];
        let report = check_esp32c6(&board, &image).unwrap();
        assert_eq!(report["dispatch_loop_in_hp_sram"], true);
    }

    #[test]
    fn c6_accepts_the_measured_layout_and_reports_each_region() {
        let board = BoardProfile::esp32c6_devkitc_1().unwrap();
        let report = check_esp32c6(&board, &c6_image()).unwrap();
        assert_eq!(report["image_load_bytes"], 27_186);
        assert_eq!(report["flash_mapped_bytes"], 23_470);
        assert_eq!(report["hp_sram_static_bytes"], 0x63c + 0x29c + 0x4254);
        assert_eq!(report["rtc_fast_bytes"], 8);
        assert_eq!(report["stack_region_bytes"], 431_416);
        assert_eq!(report["factory_free_bytes"], 0x1F0000 - 27_186);
        assert_eq!(report["entry"], 0x4201_0072_u64);
        assert!(report["heap_arena_bytes"].is_null());
    }

    #[test]
    fn c6_rejects_a_section_or_load_outside_every_window() {
        let board = BoardProfile::esp32c6_devkitc_1().unwrap();
        let mut stray = c6_image();
        stray.sections.push((".stray".into(), 0x3000_0000, 4));
        assert!(check_esp32c6(&board, &stray).is_err());
        let mut straddling = c6_image();
        straddling
            .sections
            .push((".straddle".into(), 0x4087_FFFC, 8));
        assert!(check_esp32c6(&board, &straddling).is_err());
        let mut stray_load = c6_image();
        stray_load.loads.push((0x4400_0000, 4));
        assert!(check_esp32c6(&board, &stray_load).is_err());
    }

    #[test]
    fn c6_windows_do_not_admit_straddling_or_neighbouring_ranges() {
        assert_eq!(c6_window(0x4087_FFFC, 4), Some("hp_sram"));
        assert_eq!(c6_window(0x4087_FFFC, 8), None);
        assert_eq!(c6_window(0x4280_0000, 1), None);
        assert_eq!(c6_window(0x5000_3FFC, 4), Some("rtc_fast"));
        assert_eq!(c6_window(0x5000_3FFC, 8), None);
    }

    #[test]
    fn c6_rejects_an_arm_image_and_hard_float_or_rve_flags() {
        let board = BoardProfile::esp32c6_devkitc_1().unwrap();
        let mut arm = c6_image();
        arm.riscv32 = false;
        assert!(check_esp32c6(&board, &arm).is_err());
        // No RVC; single, double and quad float ABIs; RV32E with RVC.
        for e_flags in [0x0, 0x3, 0x5, 0x7, 0x9] {
            let mut image = c6_image();
            image.e_flags = e_flags;
            assert!(
                check_esp32c6(&board, &image).is_err(),
                "e_flags {e_flags:#x}"
            );
        }
        assert!(inspect_esp32c6(&board, b"not a linked firmware image").is_err());
    }

    #[test]
    fn c6_requires_the_stack_region_to_hold_the_window_and_headroom() {
        let board = BoardProfile::esp32c6_devkitc_1().unwrap();
        let mut short = c6_image();
        short.stack_end = Some(0x4086_e610 - (18 * 1024 - 1));
        assert!(check_esp32c6(&board, &short).is_err());
        let mut exact = c6_image();
        exact.stack_end = Some(0x4086_e610 - 18 * 1024);
        assert!(check_esp32c6(&board, &exact).is_ok());
        let mut missing = c6_image();
        missing.stack_start = None;
        assert!(check_esp32c6(&board, &missing).is_err());
    }

    #[test]
    fn c6_build_reports_the_stack_margin_and_refuses_a_region_below_the_window_and_margin() {
        // Specification 34 D.2: the region holds the 16 KiB window plus at least a 4 KiB margin.
        let table = |start: u64, bytes: u64| vec![(".stack".to_owned(), start, bytes)];
        // S1b-5c's measured region, 0x3F20 = 16,160 bytes: below the window, so outside the margin.
        let error = c6_stack_region(&table(C6_STACK_TOP - 0x3F20, 0x3F20))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("16160") && error.contains("16384") && error.contains("4096"),
            "{error}"
        );
        // The window plus exactly the minimum margin passes and reports its margin.
        let report = c6_stack_region(&table(C6_STACK_TOP - 20 * 1024, 20 * 1024)).unwrap();
        assert_eq!(report["region_bytes"], 20_480);
        assert_eq!(report["window_bytes"], 16_384);
        assert_eq!(report["margin_bytes"], 4_096);
        assert_eq!(report["margin_minimum_bytes"], 4_096);
        // One byte under the minimum margin is refused.
        assert!(c6_stack_region(&table(C6_STACK_TOP - (20 * 1024 - 1), 20 * 1024 - 1)).is_err());
        // The 8 KiB margin S1b-5d gives back is reported.
        let report = c6_stack_region(&table(C6_STACK_TOP - 24 * 1024, 24 * 1024)).unwrap();
        assert_eq!(report["margin_bytes"], 8_192);
        // A missing region and one that does not end at the fixed stack top are refused.
        assert!(c6_stack_region(&[]).is_err());
        assert!(c6_stack_region(&table(C6_STACK_TOP - 20 * 1024 - 4, 20 * 1024)).is_err());
    }

    #[test]
    fn c6_rejects_an_image_larger_than_the_factory_partition() {
        let board = BoardProfile::esp32c6_devkitc_1().unwrap();
        let mut too_large = c6_image();
        too_large.loads.push((0x4240_0000, 0x1F0000));
        assert!(check_esp32c6(&board, &too_large).is_err());
        let mut fits = c6_image();
        fits.loads.push((0x4240_0000, 0x1F0000 - 27_186));
        assert!(check_esp32c6(&board, &fits).is_ok());
    }

    #[test]
    fn c6_reports_the_heap_arena_when_the_firmware_exports_one() {
        let board = BoardProfile::esp32c6_devkitc_1().unwrap();
        let mut image = c6_image();
        image.heap_arena_bytes = Some(72 * 1024);
        let report = check_esp32c6(&board, &image).unwrap();
        assert_eq!(report["heap_arena_bytes"], 73_728);
    }

    #[test]
    fn c6_ram_windows_are_the_ram_regions_of_the_chip_file() {
        // HP SRAM and the RTC fast memory (LP SRAM), as chips/esp32c6.toml records them.
        let chip = crate::portability::board::chip("esp32c6").unwrap();
        let regions: Vec<_> = chip
            .memory
            .ram
            .iter()
            .map(|region| u64::from(region.start)..region.end())
            .collect();
        assert_eq!(regions, [C6_HP_SRAM, C6_RTC_FAST]);
        assert!(contained(C6_STACK_TOP, 0, &C6_HP_SRAM));
    }
}
