//! The program image `TRSTI001` (specification 44 §5): one file per resource and target, read
//! where it lies.
//!
//! The image is little-endian. A 128-byte header and a section table of 16-byte entries come first;
//! `CODE` follows at a 64-byte boundary with `RODATA` at its linked distance after it, then the
//! other sections at 8-byte boundaries, `SIGN` last. A table section starts with its record count and
//! a reserved word, holds that many fixed-size records ([`Record`]) and may end with data its records
//! name by offsets from the section's start (the task table's program list, the type table's
//! details). Strings are NUL-terminated UTF-8 in `STRINGS`, which starts with the empty string.
//!
//! [`Image::parse`] checks the header and the section table, [`Image::check_tables`] every record
//! (specification 44 §10.2, steps 2, 3 and 5); neither copies or allocates.

mod records;
mod writer;

use core::fmt;
use core::marker::PhantomData;

use trust_plc_abi as abi;

pub use records::{
    type_kind, AreaRecord, EntryKind, EntryRecord, FileRecord, IoBindRecord, MemberDetail,
    MemoryFigure, Record, RetainRecord, StmtRecord, SymbolRecord, TaskRecord, TypeRecord,
    AREA_HAS_INIT, AREA_READ_ONLY, AREA_RETAINED, BACKGROUND_PRIORITY, BIND_FLAT,
    BIND_HIERARCHICAL, BIND_OFFSET_SHIFT, BIND_RAW, BIND_TIME_MILLIS, MEMBER_BYTES, NONE,
    SYMBOL_CONSTANT, SYMBOL_DIRECT, SYMBOL_INPUT, SYMBOL_INSTANCE, SYMBOL_IN_OUT, SYMBOL_OUTPUT,
    SYMBOL_PROGRAM, SYMBOL_RETAINED,
};
use records::{u16_at, u32_at, u64_at};
pub use writer::{CodeImage, ImageWriter, StringPool, WriteError};

/// The image's magic.
pub const MAGIC: [u8; 8] = *b"TRSTI001";
/// The format version this reader reads.
pub const FORMAT_VERSION: u16 = 2;
/// The bytes of the header.
pub const HEADER_BYTES: usize = 128;

/// Bit 0 of the header's flags: the image has no statement hooks, a release image built with
/// `[build] debug = false` (specification 44 §5.2, §13.2).
pub const FLAG_RELEASE: u32 = 1;
/// Bit 1 of the header's flags: the program reads the platform's wall clock, `CURRENT_DT`, so its
/// deployment record requires a device that has one (specification 44 §5.2; specification 34
/// §5.2).
pub const FLAG_WALL_CLOCK: u32 = 2;
/// The bytes of a section table entry.
pub const SECTION_ENTRY_BYTES: usize = 16;
/// The bytes before a table's records: its count and a reserved word.
pub const TABLE_PREFIX_BYTES: usize = 8;
/// The alignment of `CODE` in the image, which a container keeps for the image in a slot: the
/// largest alignment a module's code or read-only data needs on a target of specification 44 §4.4.
pub const CODE_ALIGNMENT: usize = 64;
/// The alignment of every other section.
pub const SECTION_ALIGNMENT: usize = 8;

/// The sections (specification 44 §5.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SectionId {
    /// The module's code.
    Code = 1,
    /// The module's read-only data, at its linked distance after `CODE`.
    Rodata = 2,
    /// The entry table.
    Entry = 3,
    /// The area table.
    Areas = 4,
    /// The starting memory of the areas.
    Init = 5,
    /// The task table and its program list.
    Tasks = 6,
    /// The I/O bindings.
    IoBind = 7,
    /// The symbol table.
    Symbols = 8,
    /// The type table and its details.
    Types = 9,
    /// The string pool.
    Strings = 10,
    /// The statement table.
    Stmts = 11,
    /// The retain map.
    Retain = 12,
    /// The file table.
    Files = 13,
    /// The embedded sources, optional.
    Source = 14,
    /// The memory figure.
    MemFig = 15,
    /// The signature, optional, always last.
    Sign = 16,
}

impl SectionId {
    /// Every section, in the order of their ids.
    pub const ALL: [Self; 16] = [
        Self::Code,
        Self::Rodata,
        Self::Entry,
        Self::Areas,
        Self::Init,
        Self::Tasks,
        Self::IoBind,
        Self::Symbols,
        Self::Types,
        Self::Strings,
        Self::Stmts,
        Self::Retain,
        Self::Files,
        Self::Source,
        Self::MemFig,
        Self::Sign,
    ];

    /// The section numbered `raw`.
    #[must_use]
    pub fn from_raw(raw: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|id| *id as u32 == raw)
    }

    /// Whether an image without the section is refused.
    #[must_use]
    pub fn required(self) -> bool {
        !matches!(self, Self::Source | Self::Sign)
    }

    /// The section's name as the specification writes it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Code => "CODE",
            Self::Rodata => "RODATA",
            Self::Entry => "ENTRY",
            Self::Areas => "AREAS",
            Self::Init => "INIT",
            Self::Tasks => "TASKS",
            Self::IoBind => "IOBIND",
            Self::Symbols => "SYMBOLS",
            Self::Types => "TYPES",
            Self::Strings => "STRINGS",
            Self::Stmts => "STMTS",
            Self::Retain => "RETAIN",
            Self::Files => "FILES",
            Self::Source => "SOURCE",
            Self::MemFig => "MEMFIG",
            Self::Sign => "SIGN",
        }
    }

    fn index(self) -> usize {
        self as usize - 1
    }
}

/// The CPU a module is built for (specification 44 §4.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Target {
    /// RV32IMAC, the ESP32-C6.
    Rv32imac = 1,
    /// Cortex-M33 with single-precision floating point, the STM32U5A9.
    ThumbV8mMainHf = 2,
    /// x86-64, every x86-64 computer.
    X86_64 = 3,
    /// AArch64, every 64-bit ARM computer.
    Aarch64 = 4,
}

impl Target {
    /// The target numbered `raw`.
    #[must_use]
    pub fn from_raw(raw: u32) -> Option<Self> {
        [
            Self::Rv32imac,
            Self::ThumbV8mMainHf,
            Self::X86_64,
            Self::Aarch64,
        ]
        .into_iter()
        .find(|target| *target as u32 == raw)
    }

    /// The target of the CPU this code is compiled for, if it is one.
    #[must_use]
    pub fn this_cpu() -> Option<Self> {
        if cfg!(target_arch = "riscv32") {
            Some(Self::Rv32imac)
        } else if cfg!(target_arch = "arm") {
            Some(Self::ThumbV8mMainHf)
        } else if cfg!(target_arch = "x86_64") {
            Some(Self::X86_64)
        } else if cfg!(target_arch = "aarch64") {
            Some(Self::Aarch64)
        } else {
            None
        }
    }

    /// Whether entry addresses carry the Thumb bit.
    #[must_use]
    pub fn thumb(self) -> bool {
        self == Self::ThumbV8mMainHf
    }

    /// Whether the target is a computer's CPU, whose modules link no C library and take the
    /// memory functions from the toolchain (specification 44 §4.5), where a chip's target builtins
    /// hold them.
    #[must_use]
    pub fn is_computer(self) -> bool {
        matches!(self, Self::X86_64 | Self::Aarch64)
    }

    /// The bytes of an address on the target, which a reference or a pointer of a program built
    /// for it takes (specification 29 §1): 4 on the 32-bit chips, 8 on a 64-bit computer.
    #[must_use]
    pub fn pointer_bytes(self) -> u64 {
        if self.is_computer() {
            8
        } else {
            4
        }
    }
}

/// The header (specification 44 §5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// The CRC-32 of bytes 12 to the end of the section table.
    pub crc: u32,
    /// The image's total length.
    pub length: u32,
    /// The format version.
    pub format_version: u16,
    /// The kernel interface version the module was built against.
    pub interface_version: u16,
    /// The [`Target`] number.
    pub target: u32,
    /// The build time, Unix seconds.
    pub build_time: u64,
    /// The SHA-256 of the project's sources and settings.
    pub source_hash: [u8; 32],
    /// The Rust compiler's version, NUL-padded.
    pub rustc: [u8; 32],
    /// The program library's version and flags hash.
    pub library: [u8; 16],
    /// The number of sections.
    pub section_count: u32,
    /// The offset of the section table.
    pub section_table: u32,
    /// The flags: [`FLAG_RELEASE`] for an image without statement hooks.
    pub flags: u32,
    /// The offset plus one in `STRINGS` of the `RESOURCE` name the program's `CONFIGURATION`
    /// declares; zero when it declares none.
    pub resource: u32,
}

impl Header {
    fn read(bytes: &[u8]) -> Self {
        let array = |at: usize| {
            let mut out = [0u8; 32];
            if let Some(source) = bytes.get(at..at + 32) {
                out.copy_from_slice(source);
            }
            out
        };
        let mut library = [0u8; 16];
        if let Some(source) = bytes.get(96..112) {
            library.copy_from_slice(source);
        }
        Self {
            crc: u32_at(bytes, 8),
            length: u32_at(bytes, 12),
            format_version: u16_at(bytes, 16),
            interface_version: u16_at(bytes, 18),
            target: u32_at(bytes, 20),
            build_time: u64_at(bytes, 24),
            source_hash: array(32),
            rustc: array(64),
            library,
            section_count: u32_at(bytes, 112),
            section_table: u32_at(bytes, 116),
            flags: u32_at(bytes, 120),
            resource: u32_at(bytes, 124),
        }
    }

    /// Whether the image is a release image, built without statement hooks: it is not debuggable
    /// (specification 44 §13.2).
    #[must_use]
    pub const fn is_release(&self) -> bool {
        self.flags & FLAG_RELEASE != 0
    }

    /// Whether the program reads the platform's wall clock (`CURRENT_DT`, specification 34 §5.2).
    #[must_use]
    pub const fn reads_wall_clock(&self) -> bool {
        self.flags & FLAG_WALL_CLOCK != 0
    }

    /// The compiler version as text, without its padding.
    #[must_use]
    pub fn rustc_version(&self) -> &str {
        let end = self
            .rustc
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(self.rustc.len());
        core::str::from_utf8(self.rustc.get(..end).unwrap_or(&[])).unwrap_or("")
    }
}

/// One section of the section table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Section {
    /// Which section.
    pub id: SectionId,
    /// Its flags, 0 in format version 1.
    pub flags: u32,
    /// Its first byte in the image.
    pub offset: u32,
    /// Its bytes.
    pub length: u32,
}

impl Section {
    fn end(&self) -> u64 {
        u64::from(self.offset) + u64::from(self.length)
    }
}

/// Why an image is refused; the first failure of the checks names its reason (specification 44
/// §10.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageError {
    /// Fewer bytes than a header.
    TooShort {
        /// The bytes there are.
        length: usize,
    },
    /// Not a `TRSTI001` image.
    Magic,
    /// The header's length is not the image's.
    Length {
        /// The header's length.
        declared: u32,
        /// The image's bytes.
        actual: usize,
    },
    /// The header's CRC does not match.
    Crc {
        /// The header's CRC.
        stored: u32,
        /// The CRC of the bytes.
        computed: u32,
    },
    /// Another format version.
    FormatVersion(u16),
    /// The section table lies outside the image.
    SectionTable,
    /// A section id this reader does not know.
    UnknownSection(u32),
    /// A section listed twice.
    DuplicateSection(SectionId),
    /// A required section is absent.
    MissingSection(SectionId),
    /// A section lies outside the image.
    SectionBounds(SectionId),
    /// A section is not aligned.
    SectionAlignment(SectionId),
    /// Two sections overlap.
    SectionOverlap(SectionId, SectionId),
    /// `SIGN` is not the last section.
    SignNotLast,
    /// A table's records do not fit its section.
    Table(SectionId),
    /// A record names something outside its section, area or table.
    Record {
        /// The record's section.
        section: SectionId,
        /// The record's index.
        index: u32,
        /// What is wrong.
        reason: &'static str,
    },
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { length } => {
                write!(f, "image of {length} bytes is shorter than its header")
            }
            Self::Magic => f.write_str("not a TRSTI001 program image"),
            Self::Length { declared, actual } => {
                write!(
                    f,
                    "image header gives {declared} bytes, the image has {actual}"
                )
            }
            Self::Crc { stored, computed } => write!(
                f,
                "image header CRC {stored:#010x} does not match {computed:#010x}"
            ),
            Self::FormatVersion(version) => {
                write!(
                    f,
                    "image format version {version} is not 1: rebuild the project"
                )
            }
            Self::SectionTable => f.write_str("image section table lies outside the image"),
            Self::UnknownSection(id) => write!(f, "image has unknown section {id}"),
            Self::DuplicateSection(id) => write!(f, "image has section {} twice", id.name()),
            Self::MissingSection(id) => write!(f, "image has no {} section", id.name()),
            Self::SectionBounds(id) => {
                write!(f, "image section {} lies outside the image", id.name())
            }
            Self::SectionAlignment(id) => write!(f, "image section {} is not aligned", id.name()),
            Self::SectionOverlap(a, b) => {
                write!(f, "image sections {} and {} overlap", a.name(), b.name())
            }
            Self::SignNotLast => f.write_str("image section SIGN is not the last section"),
            Self::Table(id) => write!(f, "image table {} does not fit its section", id.name()),
            Self::Record {
                section,
                index,
                reason,
            } => write!(f, "image {} record {index}: {reason}", section.name()),
        }
    }
}

/// The records of one table, read where they lie.
#[derive(Clone, Copy)]
pub struct Table<'a, R> {
    records: &'a [u8],
    count: u32,
    record: PhantomData<R>,
}

impl<'a, R: Record + 'a> Table<'a, R> {
    /// The number of records.
    #[must_use]
    pub fn len(&self) -> u32 {
        self.count
    }

    /// Whether the table has no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The record at `index`.
    #[must_use]
    pub fn get(&self, index: u32) -> Option<R> {
        if index >= self.count {
            return None;
        }
        let start = usize::try_from(index).ok()?.checked_mul(R::BYTES)?;
        self.records.get(start..start + R::BYTES).map(R::read)
    }

    /// The records in order.
    pub fn iter(&self) -> impl Iterator<Item = R> + 'a {
        let records = self.records;
        records.chunks_exact(R::BYTES).map(R::read)
    }
}

/// An image whose header and section table are checked.
#[derive(Clone, Copy)]
pub struct Image<'a> {
    bytes: &'a [u8],
    header: Header,
    sections: [Option<Section>; 16],
}

/// What reading an image's header and section table found, so that the image is read again over
/// its bytes without repeating [`Image::parse`]'s checks.
#[derive(Clone, Copy)]
pub(crate) struct Shape {
    header: Header,
    sections: [Option<Section>; 16],
}

impl<'a> Image<'a> {
    /// What this image's header and section table hold.
    pub(crate) fn shape(&self) -> Shape {
        Shape {
            header: self.header,
            sections: self.sections,
        }
    }

    /// The image in `bytes` as `shape` describes it: `bytes` are the bytes `shape` was read from.
    pub(crate) fn over(bytes: &'a [u8], shape: &Shape) -> Self {
        Self {
            bytes,
            header: shape.header,
            sections: shape.sections,
        }
    }
}

impl<'a> Image<'a> {
    /// The image in `bytes`, its header and section table checked (specification 44 §10.2, steps 2
    /// and 3, without the device's own numbers).
    ///
    /// # Errors
    ///
    /// The first [`ImageError`] of the checks.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, ImageError> {
        if bytes.len() < HEADER_BYTES {
            return Err(ImageError::TooShort {
                length: bytes.len(),
            });
        }
        if bytes.get(..8) != Some(&MAGIC[..]) {
            return Err(ImageError::Magic);
        }
        let header = Header::read(bytes);
        if usize::try_from(header.length).ok() != Some(bytes.len()) {
            return Err(ImageError::Length {
                declared: header.length,
                actual: bytes.len(),
            });
        }
        if header.format_version != FORMAT_VERSION {
            return Err(ImageError::FormatVersion(header.format_version));
        }
        let table_start =
            usize::try_from(header.section_table).map_err(|_| ImageError::SectionTable)?;
        let table_end = usize::try_from(header.section_count)
            .ok()
            .and_then(|count| count.checked_mul(SECTION_ENTRY_BYTES))
            .and_then(|bytes| bytes.checked_add(table_start))
            .ok_or(ImageError::SectionTable)?;
        if table_start < HEADER_BYTES || table_end > bytes.len() {
            return Err(ImageError::SectionTable);
        }
        let computed = crate::crc32::checksum(bytes.get(12..table_end).unwrap_or(&[]));
        if computed != header.crc {
            return Err(ImageError::Crc {
                stored: header.crc,
                computed,
            });
        }
        let mut sections = [None; 16];
        for entry in bytes
            .get(table_start..table_end)
            .unwrap_or(&[])
            .as_chunks::<SECTION_ENTRY_BYTES>()
            .0
        {
            let raw = u32_at(entry, 0);
            let id = SectionId::from_raw(raw).ok_or(ImageError::UnknownSection(raw))?;
            let section = Section {
                id,
                flags: u32_at(entry, 4),
                offset: u32_at(entry, 8),
                length: u32_at(entry, 12),
            };
            if section.end() > bytes.len() as u64 || u64::from(section.offset) < table_end as u64 {
                return Err(ImageError::SectionBounds(id));
            }
            let alignment = if id == SectionId::Code {
                CODE_ALIGNMENT
            } else {
                SECTION_ALIGNMENT
            };
            if !usize::try_from(section.offset).is_ok_and(|offset| offset % alignment == 0) {
                return Err(ImageError::SectionAlignment(id));
            }
            let slot = &mut sections[id.index()];
            if slot.is_some() {
                return Err(ImageError::DuplicateSection(id));
            }
            *slot = Some(section);
        }
        for id in SectionId::ALL {
            if id.required() && sections[id.index()].is_none() {
                return Err(ImageError::MissingSection(id));
            }
        }
        // No two sections share a byte: sorted by offset, each ends before the next begins.
        let mut ordered: [Option<Section>; 16] = sections;
        ordered.sort_unstable_by_key(|section| {
            section.map_or(u64::MAX, |section| u64::from(section.offset))
        });
        for pair in ordered.windows(2) {
            if let [Some(first), Some(second)] = pair
                && first.length > 0
                && second.length > 0
                && first.end() > u64::from(second.offset)
            {
                return Err(ImageError::SectionOverlap(first.id, second.id));
            }
        }
        if let Some(sign) = sections[SectionId::Sign.index()]
            && sections
                .iter()
                .flatten()
                .any(|section| section.id != SectionId::Sign && section.offset > sign.offset)
        {
            return Err(ImageError::SignNotLast);
        }
        Ok(Self {
            bytes,
            header,
            sections,
        })
    }

    /// The image's bytes.
    #[must_use]
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// The header.
    #[must_use]
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The section `id`, when the image has it.
    #[must_use]
    pub fn section(&self, id: SectionId) -> Option<Section> {
        self.sections[id.index()]
    }

    /// The bytes of section `id`; none for an absent section.
    #[must_use]
    pub fn section_bytes(&self, id: SectionId) -> &'a [u8] {
        self.section(id)
            .and_then(|section| {
                let start = usize::try_from(section.offset).ok()?;
                let end = start.checked_add(usize::try_from(section.length).ok()?)?;
                self.bytes.get(start..end)
            })
            .unwrap_or(&[])
    }

    /// The bytes the kernel maps executable: `CODE` through the end of `RODATA`, with `RODATA` at
    /// its linked distance, and the offset of `CODE` in the image (specification 44 §10.3).
    #[must_use]
    pub fn module(&self) -> (u32, &'a [u8]) {
        let code = self.section(SectionId::Code);
        let rodata = self.section(SectionId::Rodata);
        let (Some(code), Some(rodata)) = (code, rodata) else {
            return (0, &[]);
        };
        let end = code.end().max(rodata.end());
        let bytes = usize::try_from(code.offset)
            .ok()
            .zip(usize::try_from(end).ok())
            .and_then(|(start, end)| self.bytes.get(start..end))
            .unwrap_or(&[]);
        (code.offset, bytes)
    }

    /// The table of `R`.
    ///
    /// # Errors
    ///
    /// [`ImageError::Table`] when the count's records do not fit the section.
    pub fn table<R: Record>(&self) -> Result<Table<'a, R>, ImageError> {
        let bytes = self.section_bytes(R::SECTION);
        let count = u32_at(bytes, 0);
        let end = usize::try_from(count)
            .ok()
            .and_then(|count| count.checked_mul(R::BYTES))
            .and_then(|records| records.checked_add(TABLE_PREFIX_BYTES))
            .ok_or(ImageError::Table(R::SECTION))?;
        if bytes.len() < TABLE_PREFIX_BYTES || end > bytes.len() {
            return Err(ImageError::Table(R::SECTION));
        }
        Ok(Table {
            records: bytes.get(TABLE_PREFIX_BYTES..end).unwrap_or(&[]),
            count,
            record: PhantomData,
        })
    }

    /// The `RESOURCE` name the program's `CONFIGURATION` declares; `None` when it declares none
    /// (§5.2).
    #[must_use]
    pub fn resource_name(&self) -> Option<&'a str> {
        let offset = self.header().resource.checked_sub(1)?;
        self.string(offset)
    }

    /// The string at `offset` in `STRINGS`; `None` past the pool, without its NUL or not UTF-8.
    #[must_use]
    pub fn string(&self, offset: u32) -> Option<&'a str> {
        let pool = self.section_bytes(SectionId::Strings);
        let start = usize::try_from(offset).ok()?;
        let rest = pool.get(start..)?;
        let end = rest.iter().position(|byte| *byte == 0)?;
        core::str::from_utf8(rest.get(..end)?).ok()
    }

    /// The program list of the task table: the entry index of each program, in order.
    pub fn task_programs(&self, task: &TaskRecord) -> impl Iterator<Item = u32> + 'a {
        let bytes = self.section_bytes(SectionId::Tasks);
        let records = usize::try_from(u32_at(bytes, 0))
            .unwrap_or(0)
            .saturating_mul(TaskRecord::BYTES)
            .saturating_add(TABLE_PREFIX_BYTES);
        let first = usize::try_from(task.program_first).unwrap_or(usize::MAX);
        let count = usize::try_from(task.program_count).unwrap_or(0);
        let start = first.saturating_mul(4).saturating_add(records);
        bytes
            .get(start..start.saturating_add(count.saturating_mul(4)))
            .unwrap_or(&[])
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| u32::from_le_bytes(*chunk))
    }

    /// The memory figure.
    #[must_use]
    pub fn memory_figure(&self) -> MemoryFigure {
        MemoryFigure::read(self.section_bytes(SectionId::MemFig))
    }

    /// The symbol whose canonical name is `canonical`, by one binary search, with its index.
    #[must_use]
    pub fn find_symbol(&self, canonical: &str) -> Option<(u32, SymbolRecord)> {
        let symbols = self.table::<SymbolRecord>().ok()?;
        let (mut low, mut high) = (0u32, symbols.len());
        while low < high {
            let middle = low + (high - low) / 2;
            let symbol = symbols.get(middle)?;
            match self.string(symbol.canonical)?.cmp(canonical) {
                core::cmp::Ordering::Less => low = middle + 1,
                core::cmp::Ordering::Greater => high = middle,
                core::cmp::Ordering::Equal => return Some((middle, symbol)),
            }
        }
        None
    }

    /// The length of area `id`, or `None` when the image has no such area.
    #[must_use]
    pub fn area_length(&self, id: u8) -> Option<u64> {
        self.table::<AreaRecord>()
            .ok()?
            .iter()
            .find(|area| area.id == u32::from(id))
            .map(|area| area.length)
    }

    /// Checks every record against its section, its area and the other tables (specification 44
    /// §10.2, step 5, without the device's process image).
    ///
    /// # Errors
    ///
    /// The first [`ImageError`].
    pub fn check_tables(&self) -> Result<(), ImageError> {
        let strings = self.section_bytes(SectionId::Strings);
        if strings.first() != Some(&0) || strings.last() != Some(&0) {
            return Err(ImageError::Table(SectionId::Strings));
        }
        let entries = self.check_entries()?;
        self.check_areas()?;
        let types = self.check_types()?;
        let symbols = self.check_symbols(types)?;
        self.check_tasks(entries, symbols)?;
        self.check_bindings(symbols)?;
        self.check_files_and_statements()?;
        self.check_retain()?;
        if self.section_bytes(SectionId::MemFig).len() < MemoryFigure::BYTES {
            return Err(ImageError::Table(SectionId::MemFig));
        }
        Ok(())
    }

    /// Every entry of a known kind and inside `CODE`; returns the entry table.
    fn check_entries(&self) -> Result<Table<'a, EntryRecord>, ImageError> {
        let entries = self.table::<EntryRecord>()?;
        let code = self.section(SectionId::Code).map_or(0, |code| code.length);
        let thumb = Target::from_raw(self.header.target).is_some_and(Target::thumb);
        for (index, entry) in (0u32..).zip(entries.iter()) {
            if EntryKind::from_raw(entry.kind).is_none() {
                return Err(record(SectionId::Entry, index, "unknown entry kind"));
            }
            let address = if thumb {
                entry.code_offset & !1
            } else {
                entry.code_offset
            };
            if address >= code {
                return Err(record(SectionId::Entry, index, "entry outside CODE"));
            }
        }
        Ok(entries)
    }

    /// Every area once, at most 2^32 - 1 bytes, aligned 1 to 8, its starting memory inside `INIT`.
    fn check_areas(&self) -> Result<(), ImageError> {
        let areas = self.table::<AreaRecord>()?;
        let init = self
            .section(SectionId::Init)
            .map_or(0, |init| u64::from(init.length));
        let mut seen = [false; 8];
        for (index, area) in (0u32..).zip(areas.iter()) {
            let id = usize::try_from(area.id).unwrap_or(0);
            if !(1..=7).contains(&id) || seen[id] {
                return Err(record(
                    SectionId::Areas,
                    index,
                    "area id not 1 to 7 or repeated",
                ));
            }
            seen[id] = true;
            if area.length > u64::from(u32::MAX) {
                return Err(record(
                    SectionId::Areas,
                    index,
                    "area longer than 2^32 - 1 bytes",
                ));
            }
            if !area.alignment.is_power_of_two() || area.alignment > 8 {
                return Err(record(SectionId::Areas, index, "area alignment not 1 to 8"));
            }
            if area.flags & AREA_HAS_INIT != 0 && u64::from(area.init_offset) + area.length > init {
                return Err(record(
                    SectionId::Areas,
                    index,
                    "starting memory outside INIT",
                ));
            }
        }
        Ok(())
    }

    /// Every type's name and detail inside their sections; returns the number of types.
    fn check_types(&self) -> Result<u32, ImageError> {
        let types = self.table::<TypeRecord>()?;
        let types_bytes = self.section_bytes(SectionId::Types).len() as u64;
        for (index, ty) in (0u32..).zip(types.iter()) {
            if ty.name != 0 && self.string(ty.name).is_none() {
                return Err(record(SectionId::Types, index, "name outside STRINGS"));
            }
            if u64::from(ty.detail) > types_bytes {
                return Err(record(SectionId::Types, index, "detail outside TYPES"));
            }
        }
        Ok(types.len())
    }

    /// The symbols sorted by canonical name, their names, types and parents in their tables and
    /// their variables inside their areas; returns the number of symbols.
    fn check_symbols(&self, types: u32) -> Result<u32, ImageError> {
        let symbols = self.table::<SymbolRecord>()?;
        let mut previous: Option<&str> = None;
        for (index, symbol) in (0u32..).zip(symbols.iter()) {
            let Some(name) = self.string(symbol.canonical) else {
                return Err(record(SectionId::Symbols, index, "name outside STRINGS"));
            };
            if previous.is_some_and(|previous| previous >= name) {
                return Err(record(
                    SectionId::Symbols,
                    index,
                    "not sorted by canonical name",
                ));
            }
            previous = Some(name);
            if self.string(symbol.display).is_none() {
                return Err(record(
                    SectionId::Symbols,
                    index,
                    "display name outside STRINGS",
                ));
            }
            // A function's POU symbol runs on no instance and has no type (§5.7).
            let typeless_pou = symbol.area == 0 && symbol.type_index == NONE;
            if symbol.type_index >= types && !typeless_pou {
                return Err(record(SectionId::Symbols, index, "type outside TYPES"));
            }
            if symbol.parent != NONE && symbol.parent >= symbols.len() {
                return Err(record(SectionId::Symbols, index, "parent outside SYMBOLS"));
            }
            match symbol.area {
                0 | 7 => {}
                1..=6 => {
                    let length = self.area_length(symbol.area).unwrap_or(0);
                    if u64::from(symbol.offset) + u64::from(symbol.size) > length {
                        return Err(record(
                            SectionId::Symbols,
                            index,
                            "variable outside its area",
                        ));
                    }
                }
                _ => return Err(record(SectionId::Symbols, index, "area id not 0 to 7")),
            }
        }
        Ok(symbols.len())
    }

    /// Every task's name and `SINGLE` in their tables, its program list inside `TASKS` and each
    /// program a program body.
    fn check_tasks(&self, entries: Table<'a, EntryRecord>, symbols: u32) -> Result<(), ImageError> {
        let tasks = self.table::<TaskRecord>()?;
        for (index, task) in (0u32..).zip(tasks.iter()) {
            if self.string(task.name).is_none() {
                return Err(record(SectionId::Tasks, index, "name outside STRINGS"));
            }
            if task.single != NONE && task.single >= symbols {
                return Err(record(SectionId::Tasks, index, "SINGLE outside SYMBOLS"));
            }
            let listed = self.task_programs(&task).count();
            if listed != usize::try_from(task.program_count).unwrap_or(usize::MAX) {
                return Err(record(
                    SectionId::Tasks,
                    index,
                    "program list outside TASKS",
                ));
            }
            for program in self.task_programs(&task) {
                let is_body = entries
                    .get(program)
                    .is_some_and(|entry| entry.kind == EntryKind::ProgramBody as u32);
                if !is_body {
                    return Err(record(
                        SectionId::Tasks,
                        index,
                        "program is no program body",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Every binding to a symbol and an image address, a hierarchical one to the text of a
    /// hierarchical address of the same area and size.
    fn check_bindings(&self, symbols: u32) -> Result<(), ImageError> {
        let bindings = self.table::<IoBindRecord>()?;
        for (index, binding) in (0u32..).zip(bindings.iter()) {
            if binding.symbol >= symbols {
                return Err(record(SectionId::IoBind, index, "symbol outside SYMBOLS"));
            }
            if !(1..=3).contains(&binding.area)
                || binding.size > 4
                || binding.bit > 7
                || binding.kind > BIND_HIERARCHICAL
                || self.binding_address(&binding).is_none()
            {
                return Err(record(
                    SectionId::IoBind,
                    index,
                    "address not an image address",
                ));
            }
        }
        Ok(())
    }

    /// The address of `binding`: its process image's, or the hierarchical address its text spells
    /// (§5.6); `None` for a hierarchical binding whose text is no hierarchical address of the
    /// record's area and size.
    #[must_use]
    pub fn binding_address(&self, binding: &IoBindRecord) -> Option<crate::io_address::IoAddress> {
        use crate::io_address::{IoAddress, IoSize};
        use crate::memory::IoArea;
        let area = match binding.area {
            1 => IoArea::Input,
            2 => IoArea::Output,
            _ => IoArea::Memory,
        };
        let size = match binding.size {
            0 => IoSize::Bit,
            1 => IoSize::Byte,
            2 => IoSize::Word,
            3 => IoSize::DWord,
            _ => IoSize::LWord,
        };
        if !binding.is_hierarchical() {
            // A flat address keeps its byte as its one path component, as the parser makes it.
            return Some(IoAddress {
                area,
                size,
                byte: binding.byte,
                bit: binding.bit,
                path: alloc::vec![binding.byte],
                wildcard: false,
            });
        }
        let address = IoAddress::parse(self.string(binding.byte)?).ok()?;
        (address.area == area && address.size == size && address.path.len() > 1).then_some(address)
    }

    /// Every file's label and text inside their sections, every statement's file in `FILES`.
    fn check_files_and_statements(&self) -> Result<(), ImageError> {
        let files = self.table::<FileRecord>()?;
        let source = self
            .section(SectionId::Source)
            .map_or(0, |source| u64::from(source.length));
        for (index, file) in (0u32..).zip(files.iter()) {
            if self.string(file.label).is_none() {
                return Err(record(SectionId::Files, index, "label outside STRINGS"));
            }
            if file.source_offset != NONE
                && u64::from(file.source_offset) + u64::from(file.source_length) > source
            {
                return Err(record(SectionId::Files, index, "text outside SOURCE"));
            }
        }
        let statements = self.table::<StmtRecord>()?;
        for (index, statement) in (0u32..).zip(statements.iter()) {
            if u32::from(statement.file) >= files.len() {
                return Err(record(SectionId::Stmts, index, "file outside FILES"));
            }
        }
        Ok(())
    }

    /// Every retained variable a key in the string pool, of a type of the type table, inside the
    /// globals, retain or instance area (specification 44 §5.10).
    fn check_retain(&self) -> Result<(), ImageError> {
        let retain = self.table::<RetainRecord>()?;
        let types = self.table::<TypeRecord>()?;
        for (index, retained) in (0u32..).zip(retain.iter()) {
            if self.string(retained.key).is_none() {
                return Err(record(
                    SectionId::Retain,
                    index,
                    "key outside the string pool",
                ));
            }
            let Some(ty) = types.get(retained.type_index) else {
                return Err(record(SectionId::Retain, index, "type outside TYPES"));
            };
            let area = u8::try_from(retained.area)
                .ok()
                .filter(|area| {
                    matches!(
                        *area,
                        abi::AREA_GLOBALS | abi::AREA_RETAIN | abi::AREA_INSTANCES
                    )
                })
                .ok_or_else(|| record(SectionId::Retain, index, "area of no variable"))?;
            let length = self.area_length(area).unwrap_or(0);
            if u64::from(retained.offset) + u64::from(ty.size) > length {
                return Err(record(
                    SectionId::Retain,
                    index,
                    "variable outside its area",
                ));
            }
        }
        Ok(())
    }
}

/// The failure of record `index` of `section`.
fn record(section: SectionId, index: u32, reason: &'static str) -> ImageError {
    ImageError::Record {
        section,
        index,
        reason,
    }
}

#[cfg(test)]
mod tests;
