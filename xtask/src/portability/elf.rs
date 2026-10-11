use anyhow::{ensure, Context, Result};
use object::read::elf::ProgramHeader;
use object::{Object, ObjectSection, ObjectSymbol};
use serde_json::{json, Value};

const VECTOR: std::ops::Range<u64> = 0x0800_0000..0x0800_4000;
const CODE: std::ops::Range<u64> = 0x0801_0000..0x0808_0000;
const RAM: std::ops::Range<u64> = 0x2000_0000..0x2001_4000;
const FLASH_MARGIN: u64 = 16 * 1024;

fn contained(start: u64, bytes: u64, region: &std::ops::Range<u64>) -> bool {
    start >= region.start
        && start
            .checked_add(bytes)
            .is_some_and(|end| end <= region.end)
}

fn add_flash_load(loads: &mut Vec<(u64, u64)>, start: u64, length: u64) -> Result<()> {
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

fn readonly_tail(
    start: u64,
    length: u64,
    flags: u64,
    vector_end: u64,
    alignment: u64,
) -> Result<()> {
    ensure!(
        start >= vector_end
            && start.is_multiple_of(32)
            && alignment.is_power_of_two()
            && start.is_multiple_of(alignment)
            && contained(start, length, &VECTOR),
        "immutable firmware data overlaps vectors or leaves sector 0"
    );
    ensure!(
        flags & u64::from(object::elf::SHF_ALLOC) != 0
            && flags & u64::from(object::elf::SHF_WRITE | object::elf::SHF_EXECINSTR) == 0,
        "sector-0 firmware data must be allocated read-only non-executable data"
    );
    Ok(())
}

fn upper_span(end: u64) -> Result<u64> {
    ensure!(
        (CODE.start..=CODE.end - FLASH_MARGIN).contains(&end),
        "firmware does not leave 16 KiB upper-region headroom"
    );
    Ok(end - CODE.start)
}

pub(super) fn inspect(bytes: &[u8]) -> Result<Value> {
    let file = object::File::parse(bytes).context("decode firmware ELF")?;
    ensure!(
        file.architecture() == object::Architecture::Arm
            && !file.is_64()
            && file.is_little_endian(),
        "firmware must be ARM32 little-endian ELF"
    );
    let object::FileFlags::Elf { e_flags, .. } = file.flags() else {
        anyhow::bail!("firmware is not ELF");
    };
    ensure!(
        e_flags & object::elf::EF_ARM_ABI_FLOAT_HARD != 0,
        "firmware lacks hard-float ABI flag"
    );
    ensure!(
        CODE.contains(&(file.entry() & !1)),
        "reset entry is outside executable partition"
    );
    let vectors = file
        .section_by_name(".vector_table")
        .context("missing vector table")?;
    ensure!(
        vectors.address() == VECTOR.start && contained(vectors.address(), vectors.size(), &VECTOR),
        "vector table exceeds sector 0"
    );
    let table = vectors.data()?;
    ensure!(table.len() >= 8, "truncated vector table");
    let initial_sp = u32::from_le_bytes(table[..4].try_into()?);
    ensure!(
        initial_sp == 0x2001_8000,
        "unexpected initial stack pointer"
    );
    let reset_vector = u32::from_le_bytes(table[4..8].try_into()?);
    ensure!(
        reset_vector & 1 == 1 && u64::from(reset_vector) == file.entry(),
        "reset vector differs from Thumb ELF entry"
    );
    let symbol = |name: &str| -> Result<u64> {
        file.symbols()
            .find(|s| s.name().ok() == Some(name))
            .map(|s| s.address())
            .with_context(|| format!("missing linker symbol {name}"))
    };
    ensure!(
        symbol("_stack_end")? == RAM.end && symbol("_stack_start")? == u64::from(initial_sp),
        "linker did not reserve the 16 KiB stack region"
    );
    let tail = file
        .section_by_name(".firmware_rodata")
        .context("missing immutable firmware data")?;
    let object::SectionFlags::Elf { sh_flags } = tail.flags() else {
        anyhow::bail!("immutable firmware data is not an ELF section");
    };
    readonly_tail(
        tail.address(),
        tail.size(),
        sh_flags,
        vectors.address() + vectors.size(),
        tail.align(),
    )?;
    ensure!(
        tail.address() == symbol("__firmware_rodata_start")?
            && tail.address() + tail.size() == symbol("__firmware_rodata_end")?,
        "immutable firmware data differs from linker bounds"
    );
    let code_end = symbol("__firmware_load_end")?;
    let code_span = upper_span(code_end)?;
    let code = file
        .section_by_name(".text")
        .context("missing executable firmware section")?;
    ensure!(
        code.address() == CODE.start
            && contained(code.address(), code.size(), &CODE)
            && (code.address()..code.address() + code.size()).contains(&(file.entry() & !1)),
        "reset handler is outside executable firmware section"
    );
    let object::SectionFlags::Elf { sh_flags } = code.flags() else {
        anyhow::bail!("firmware code is not an ELF section");
    };
    ensure!(
        sh_flags & u64::from(object::elf::SHF_EXECINSTR | object::elf::SHF_ALLOC)
            == u64::from(object::elf::SHF_EXECINSTR | object::elf::SHF_ALLOC),
        "firmware text is not allocated executable code"
    );
    let elf = object::read::elf::ElfFile32::<object::Endianness>::parse(bytes)?;
    let endian = elf.endian();
    let mut loads = Vec::new();
    let mut flash_bytes = 0_u64;
    for segment in elf.elf_program_headers() {
        if segment.p_type(endian) != object::elf::PT_LOAD || segment.p_filesz(endian) == 0 {
            continue;
        }
        let start = u64::from(segment.p_paddr(endian));
        let length = u64::from(segment.p_filesz(endian));
        add_flash_load(&mut loads, start, length)?;
        if VECTOR.contains(&start) {
            ensure!(
                segment.p_flags(endian) & (object::elf::PF_W | object::elf::PF_X) == 0,
                "sector-0 load segment is writable or executable"
            );
        } else {
            ensure!(
                start + length <= code_end,
                "firmware load exceeds recorded upper end"
            );
        }
        let offset = u64::from(segment.p_offset(endian));
        ensure!(
            offset
                .checked_add(length)
                .is_some_and(|end| end <= bytes.len() as u64),
            "ELF load extends beyond file"
        );
        flash_bytes += length;
    }
    ensure!(!loads.is_empty(), "firmware has no flash load segments");
    let mut sections = Vec::new();
    let mut ram_bytes = 0_u64;
    for section in file.sections() {
        let object::SectionFlags::Elf { sh_flags } = section.flags() else {
            continue;
        };
        if sh_flags & u64::from(object::elf::SHF_ALLOC) == 0 || section.size() == 0 {
            continue;
        }
        let start = section.address();
        let size = section.size();
        if (RAM.start..0x2001_8000).contains(&start) {
            ensure!(
                contained(start, size, &RAM),
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
                    matches!(section.name()?, ".vector_table" | ".firmware_rodata"),
                    "unexpected allocated section in vector/data sector"
                );
            } else {
                ensure!(
                    start + size <= code_end,
                    "section exceeds recorded upper end"
                );
            }
        }
        sections.push(json!({"name":section.name()?,"address":start,"bytes":size}));
    }
    ensure!(
        (72 * 1024..=80 * 1024).contains(&ram_bytes),
        "static RAM does not match heap/static reservation"
    );
    Ok(
        json!({"firmware_sha256":super::digest(bytes), "flash_load_bytes":flash_bytes,
        "static_ram_bytes_including_heap":ram_bytes,"other_static_ram_bytes":ram_bytes-72*1024,
        "heap_reserved_bytes":72*1024,"stack_reserved_bytes":16*1024,
        "application_sector_bytes":16*1024,"persistence_reserved_bytes":32*1024,"firmware_code_capacity_bytes":448*1024,
        "firmware_code_span_bytes":code_span,"firmware_code_free_bytes":CODE.end-code_end,
        "sector0_span_bytes":tail.address()+tail.size()-VECTOR.start,
        "sector0_free_bytes":VECTOR.end-tail.address()-tail.size(),
        "load_segments":loads,"sections":sections}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flash_and_ram_boundaries_do_not_admit_reserved_regions_or_overflow() {
        assert!(contained(VECTOR.start, 16 * 1024, &VECTOR));
        assert!(!contained(VECTOR.start, 16 * 1024 + 1, &VECTOR));
        assert!(!contained(0x0800_4000, 1, &CODE));
        assert!(!contained(0x0800_8000, 1, &CODE));
        assert!(!contained(0x0800_c000, 1, &CODE));
        assert!(!contained(0x0800_ffff, 2, &CODE));
        assert!(contained(CODE.start, 448 * 1024, &CODE));
        assert!(!contained(CODE.start, 448 * 1024 + 1, &CODE));
        assert!(!contained(0x2001_3fff, 2, &RAM));
        assert!(!contained(u64::MAX, 2, &RAM));
        assert!(inspect(b"not a linked firmware image").is_err());
    }

    #[test]
    fn discontiguous_loads_preserve_application_and_checkpoint_sectors() {
        let mut loads = Vec::new();
        add_flash_load(&mut loads, VECTOR.start, 16 * 1024).unwrap();
        add_flash_load(&mut loads, CODE.start, 432 * 1024).unwrap();
        assert!(add_flash_load(&mut loads, VECTOR.start + 404, 8).is_err());
        assert!(
            add_flash_load(&mut Vec::new(), VECTOR.start, CODE.start - VECTOR.start + 1).is_err()
        );
        assert!(add_flash_load(&mut Vec::new(), CODE.end - 4, 8).is_err());
        assert!(add_flash_load(&mut Vec::new(), u64::MAX, 2).is_err());
    }

    #[test]
    fn immutable_tail_rejects_vector_overlap_alignment_and_mutable_or_code_sections() {
        let flags = u64::from(object::elf::SHF_ALLOC);
        readonly_tail(VECTOR.start + 416, 15_968, flags, VECTOR.start + 404, 32).unwrap();
        assert!(readonly_tail(VECTOR.start + 416, 15_969, flags, VECTOR.start + 404, 32).is_err());
        assert!(readonly_tail(VECTOR.start + 400, 8, flags, VECTOR.start + 404, 32).is_err());
        assert!(readonly_tail(VECTOR.start + 404, 8, flags, VECTOR.start + 404, 32).is_err());
        assert!(readonly_tail(VECTOR.start + 416, 8, flags, VECTOR.start + 404, 64).is_err());
        for forbidden in [object::elf::SHF_WRITE, object::elf::SHF_EXECINSTR] {
            assert!(readonly_tail(
                VECTOR.start + 416,
                8,
                flags | u64::from(forbidden),
                VECTOR.start + 404,
                32
            )
            .is_err());
        }
    }

    #[test]
    fn upper_margin_rejects_one_byte_short_and_invalid_load_end() {
        assert_eq!(upper_span(CODE.end - FLASH_MARGIN).unwrap(), 432 * 1024);
        assert!(upper_span(CODE.end - FLASH_MARGIN + 1).is_err());
        assert!(upper_span(CODE.start - 1).is_err());
        assert!(upper_span(u64::MAX).is_err());
    }
}
