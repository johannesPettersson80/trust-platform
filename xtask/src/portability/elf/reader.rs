//! The ELF reader of the chip image policies (specification 34, D.4 and D.5): it reads a linked ELF32
//! image into the facts a policy checks, its allocated sections, its loadable segments, its vector table,
//! its entry and its linker symbols. Each chip's policy (`stm32f407.rs`, `stm32u5a9.rs`) applies its own
//! memory map to those facts, so the reading is written once.

use anyhow::{bail, Context, Result};
use object::read::elf::ProgramHeader;
use object::{Object, ObjectSection, ObjectSymbol};

/// An allocated, non-empty section.
pub(super) struct Section {
    pub(super) name: String,
    pub(super) address: u64,
    pub(super) size: u64,
    /// Whether the section holds code (`SHF_EXECINSTR`).
    pub(super) executable: bool,
}

/// A loadable segment with file bytes.
pub(super) struct Load {
    /// The load (physical) address. The initial values of `.data` load in flash.
    pub(super) address: u64,
    pub(super) bytes: u64,
    /// Where the segment's bytes start in the ELF file.
    pub(super) offset: u64,
    /// Whether the segment is writable or executable (`PF_W`, `PF_X`).
    pub(super) writable_or_executable: bool,
}

/// What the policy reads from one linked ELF32 image.
pub(super) struct Image {
    /// ARM, 32-bit, little-endian.
    pub(super) arm32: bool,
    /// `e_flags` carries the hard-float ABI (`EF_ARM_ABI_FLOAT_HARD`).
    pub(super) hard_float: bool,
    pub(super) entry: u64,
    pub(super) sections: Vec<Section>,
    /// The bytes of `.vector_table`: the initial stack pointer, then the reset vector.
    pub(super) vector_table: Vec<u8>,
    pub(super) loads: Vec<Load>,
    pub(super) stack_start: Option<u64>,
    pub(super) stack_end: Option<u64>,
    /// `__firmware_load_end` from `profile.x`: the end of the last byte the firmware loads in flash.
    pub(super) load_end: Option<u64>,
    /// The size of the ELF file, which every segment's bytes must lie within.
    pub(super) file_bytes: u64,
}

impl Image {
    pub(super) fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|section| section.name == name)
    }
}

/// Reads the linked ELF32 image `bytes` into the facts the policies check.
///
/// # Errors
///
/// The refusal of an image that is not ELF, and of a section, segment or symbol that cannot be read.
pub(super) fn read(bytes: &[u8]) -> Result<Image> {
    let file = object::File::parse(bytes).context("decode firmware ELF")?;
    let object::FileFlags::Elf { e_flags, .. } = file.flags() else {
        bail!("firmware is not ELF");
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
    Ok(Image {
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
    })
}
