//! The records of the image's tables (specification 44 §5.4 to §5.10), each a fixed number of
//! little-endian bytes.

use super::SectionId;

/// No symbol, no type, no parent: the "none" of an index field.
pub const NONE: u32 = u32::MAX;

/// The `N` bytes at `at`, or zeros past the end; a table hands each record exactly its bytes.
fn field<const N: usize>(bytes: &[u8], at: usize) -> [u8; N] {
    let mut out = [0u8; N];
    if let Some(source) = bytes.get(at..at + N) {
        out.copy_from_slice(source);
    }
    out
}

pub(super) fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(field(bytes, at))
}

pub(super) fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(field(bytes, at))
}

pub(super) fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(field(bytes, at))
}

fn put(out: &mut [u8], at: usize, bytes: &[u8]) {
    if let Some(target) = out.get_mut(at..at + bytes.len()) {
        target.copy_from_slice(bytes);
    }
}

/// A record of one of the image's tables.
pub trait Record: Sized + Copy {
    /// The section that holds the table.
    const SECTION: SectionId;
    /// The bytes of one record.
    const BYTES: usize;
    /// The record in `bytes`, which are exactly [`Self::BYTES`] long.
    fn read(bytes: &[u8]) -> Self;
    /// Writes the record into `out`, which is exactly [`Self::BYTES`] long.
    fn write(&self, out: &mut [u8]);
}

/// What an entry point does (specification 44 §5.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EntryKind {
    /// The body of one program instance; the index is the program's number in the task table's
    /// program lists.
    ProgramBody = 1,
    /// The initialization of a cold restart, after `INIT` is copied in (specification 44 §6.7).
    ColdInit = 2,
    /// The initialization of a warm restart.
    WarmInit = 3,
    /// A method the tools call.
    Method = 4,
}

impl EntryKind {
    /// The kind numbered `raw`.
    #[must_use]
    pub fn from_raw(raw: u32) -> Option<Self> {
        [
            Self::ProgramBody,
            Self::ColdInit,
            Self::WarmInit,
            Self::Method,
        ]
        .into_iter()
        .find(|kind| *kind as u32 == raw)
    }
}

/// An entry point: 16 bytes (specification 44 §5.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryRecord {
    /// What the entry does, an [`EntryKind`] number.
    pub kind: u32,
    /// The program's number for a program body, else 0.
    pub index: u32,
    /// The entry's first instruction relative to `CODE`, with the Thumb bit on a Thumb target.
    pub code_offset: u32,
}

impl Record for EntryRecord {
    const SECTION: SectionId = SectionId::Entry;
    const BYTES: usize = 16;
    fn read(bytes: &[u8]) -> Self {
        Self {
            kind: u32_at(bytes, 0),
            index: u32_at(bytes, 4),
            code_offset: u32_at(bytes, 8),
        }
    }
    fn write(&self, out: &mut [u8]) {
        put(out, 0, &self.kind.to_le_bytes());
        put(out, 4, &self.index.to_le_bytes());
        put(out, 8, &self.code_offset.to_le_bytes());
    }
}

/// An area's flag: the area is retained.
pub const AREA_RETAINED: u32 = 1;
/// An area's flag: the area takes its starting memory from `INIT`.
pub const AREA_HAS_INIT: u32 = 2;
/// An area's flag: the program only reads the area.
pub const AREA_READ_ONLY: u32 = 4;

/// One area the device provides: 24 bytes (specification 44 §5.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AreaRecord {
    /// The area's id, 1 to 7 (specification 44 §7.1).
    pub id: u32,
    /// The bytes the device provides.
    pub length: u64,
    /// The alignment of the area's first byte.
    pub alignment: u32,
    /// [`AREA_RETAINED`], [`AREA_HAS_INIT`], [`AREA_READ_ONLY`].
    pub flags: u32,
    /// Where the area's starting memory begins in `INIT`.
    pub init_offset: u32,
}

impl Record for AreaRecord {
    const SECTION: SectionId = SectionId::Areas;
    const BYTES: usize = 24;
    fn read(bytes: &[u8]) -> Self {
        Self {
            id: u32_at(bytes, 0),
            length: u64_at(bytes, 4),
            alignment: u32_at(bytes, 12),
            flags: u32_at(bytes, 16),
            init_offset: u32_at(bytes, 20),
        }
    }
    fn write(&self, out: &mut [u8]) {
        put(out, 0, &self.id.to_le_bytes());
        put(out, 4, &self.length.to_le_bytes());
        put(out, 12, &self.alignment.to_le_bytes());
        put(out, 16, &self.flags.to_le_bytes());
        put(out, 20, &self.init_offset.to_le_bytes());
    }
}

/// The priority of the record that holds the programs without a task (specification 10 §4).
pub const BACKGROUND_PRIORITY: u32 = u32::MAX;

/// One task: 40 bytes (specification 44 §5.6). Its programs are a range of the program list that
/// follows the task records in `TASKS`, each the index of a program body in `ENTRY`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskRecord {
    /// The task's name, a string offset.
    pub name: u32,
    /// The interval in nanoseconds; 0 for a task without one.
    pub interval_ns: u64,
    /// The priority, 0 the highest; [`BACKGROUND_PRIORITY`] for the programs without a task.
    pub priority: u32,
    /// The symbol of the `SINGLE` variable, or [`NONE`].
    pub single: u32,
    /// The number of programs.
    pub program_count: u32,
    /// The index of the first program in the program list.
    pub program_first: u32,
    /// Reserved, 0.
    pub flags: u32,
}

impl Record for TaskRecord {
    const SECTION: SectionId = SectionId::Tasks;
    const BYTES: usize = 40;
    fn read(bytes: &[u8]) -> Self {
        Self {
            name: u32_at(bytes, 0),
            interval_ns: u64_at(bytes, 4),
            priority: u32_at(bytes, 12),
            single: u32_at(bytes, 16),
            program_count: u32_at(bytes, 20),
            program_first: u32_at(bytes, 24),
            flags: u32_at(bytes, 28),
        }
    }
    fn write(&self, out: &mut [u8]) {
        put(out, 0, &self.name.to_le_bytes());
        put(out, 4, &self.interval_ns.to_le_bytes());
        put(out, 12, &self.priority.to_le_bytes());
        put(out, 16, &self.single.to_le_bytes());
        put(out, 20, &self.program_count.to_le_bytes());
        put(out, 24, &self.program_first.to_le_bytes());
        put(out, 28, &self.flags.to_le_bytes());
    }
}

/// An I/O binding: a variable with an address in a process image, 16 bytes (specification 44
/// §5.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IoBindRecord {
    /// The variable's symbol.
    pub symbol: u32,
    /// The image: 1 inputs, 2 outputs, 3 markers.
    pub area: u8,
    /// The width: 0 bit, 1 byte, 2 word, 3 double word, 4 long word.
    pub size: u8,
    /// The byte in the image.
    pub byte: u32,
    /// The bit of a bit address.
    pub bit: u8,
    /// [`BIND_FLAT`], or [`BIND_HIERARCHICAL`] for a hierarchical address, whose `byte` is the
    /// offset of its text in `STRINGS`.
    pub kind: u8,
    /// The codec in the low byte, [`BIND_RAW`] or [`BIND_TIME_MILLIS`], and above it, from bit
    /// [`BIND_OFFSET_SHIFT`], the offset of the bound leaf within the variable: an element or a
    /// member of a located array or structure (specification 10 §2.5).
    pub flags: u32,
}

/// An I/O binding's codec: the variable's bytes as they are, little-endian.
pub const BIND_RAW: u32 = 0;
/// An I/O binding's codec: a `TIME` in a double word as unsigned whole milliseconds, a negative
/// value or one of 2^32 ms or more refused (specification 10 §2.5).
pub const BIND_TIME_MILLIS: u32 = 1;
/// The shift of the bound leaf's offset within its variable in an I/O binding's flags.
pub const BIND_OFFSET_SHIFT: u32 = 8;
/// An I/O binding's kind: an address of a process image.
pub const BIND_FLAT: u8 = 0;
/// An I/O binding's kind: a hierarchical address (`%IX1.2.3`, IEC §6.5.5.2), whose value lies in
/// the kernel's hierarchical image, one slot per binding, and in no process image; the record's
/// `byte` is the offset of the address's text in `STRINGS` (specification 44 §5.6).
pub const BIND_HIERARCHICAL: u8 = 1;

impl Record for IoBindRecord {
    const SECTION: SectionId = SectionId::IoBind;
    const BYTES: usize = 16;
    fn read(bytes: &[u8]) -> Self {
        Self {
            symbol: u32_at(bytes, 0),
            area: field::<1>(bytes, 4)[0],
            size: field::<1>(bytes, 5)[0],
            byte: u32_at(bytes, 6),
            bit: field::<1>(bytes, 10)[0],
            kind: field::<1>(bytes, 11)[0],
            flags: u32_at(bytes, 12),
        }
    }
    fn write(&self, out: &mut [u8]) {
        put(out, 0, &self.symbol.to_le_bytes());
        put(out, 4, &[self.area, self.size]);
        put(out, 6, &self.byte.to_le_bytes());
        put(out, 10, &[self.bit, self.kind]);
        put(out, 12, &self.flags.to_le_bytes());
    }
}

impl IoBindRecord {
    /// Whether the binding's address is hierarchical ([`BIND_HIERARCHICAL`]).
    #[must_use]
    pub const fn is_hierarchical(&self) -> bool {
        self.kind == BIND_HIERARCHICAL
    }

    /// The bytes the binding spans in its image: 1 for a bit or a byte.
    #[must_use]
    pub fn width(&self) -> u32 {
        match self.size {
            2 => 2,
            3 => 4,
            4 => 8,
            _ => 1,
        }
    }
}

/// A symbol's flag: retained.
pub const SYMBOL_RETAINED: u8 = 1;
/// A symbol's flag: a constant.
pub const SYMBOL_CONSTANT: u8 = 2;
/// A symbol's flag: an input of its POU.
pub const SYMBOL_INPUT: u8 = 4;
/// A symbol's flag: an output of its POU.
pub const SYMBOL_OUTPUT: u8 = 8;
/// A symbol's flag: an in-out of its POU.
pub const SYMBOL_IN_OUT: u8 = 16;
/// A symbol's flag: a program instance.
pub const SYMBOL_PROGRAM: u8 = 32;
/// A symbol's flag: a function block or class instance.
pub const SYMBOL_INSTANCE: u8 = 64;
/// A symbol's flag: directly addressed (`AT %…`).
pub const SYMBOL_DIRECT: u8 = 128;

/// A named variable: 32 bytes, sorted by canonical name (specification 44 §5.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolRecord {
    /// The canonical full name, in capitals, a string offset.
    pub canonical: u32,
    /// The display name, as declared, a string offset.
    pub display: u32,
    /// The type's index in `TYPES`.
    pub type_index: u32,
    /// The area id; 0 for a symbol that holds no storage.
    pub area: u8,
    /// The `SYMBOL_` flags.
    pub flags: u8,
    /// The offset in the area.
    pub offset: u32,
    /// The bytes of the variable, padding excluded.
    pub size: u32,
    /// The parent symbol, or [`NONE`].
    pub parent: u32,
    /// The POU of a POU symbol, or the POU that declares a variable; [`NONE`] for a global.
    pub pou: u32,
}

impl Record for SymbolRecord {
    const SECTION: SectionId = SectionId::Symbols;
    const BYTES: usize = 32;
    fn read(bytes: &[u8]) -> Self {
        Self {
            canonical: u32_at(bytes, 0),
            display: u32_at(bytes, 4),
            type_index: u32_at(bytes, 8),
            area: field::<1>(bytes, 12)[0],
            flags: field::<1>(bytes, 13)[0],
            offset: u32_at(bytes, 16),
            size: u32_at(bytes, 20),
            parent: u32_at(bytes, 24),
            pou: u32_at(bytes, 28),
        }
    }
    fn write(&self, out: &mut [u8]) {
        put(out, 0, &self.canonical.to_le_bytes());
        put(out, 4, &self.display.to_le_bytes());
        put(out, 8, &self.type_index.to_le_bytes());
        put(out, 12, &[self.area, self.flags, 0, 0]);
        put(out, 16, &self.offset.to_le_bytes());
        put(out, 20, &self.size.to_le_bytes());
        put(out, 24, &self.parent.to_le_bytes());
        put(out, 28, &self.pou.to_le_bytes());
    }
}

/// The kinds of the type table (specification 44 §5.8): the elementary types first.
pub mod type_kind {
    /// `BOOL`.
    pub const BOOL: u32 = 1;
    /// `SINT`.
    pub const SINT: u32 = 2;
    /// `INT`.
    pub const INT: u32 = 3;
    /// `DINT`.
    pub const DINT: u32 = 4;
    /// `LINT`.
    pub const LINT: u32 = 5;
    /// `USINT`.
    pub const USINT: u32 = 6;
    /// `UINT`.
    pub const UINT: u32 = 7;
    /// `UDINT`.
    pub const UDINT: u32 = 8;
    /// `ULINT`.
    pub const ULINT: u32 = 9;
    /// `REAL`.
    pub const REAL: u32 = 10;
    /// `LREAL`.
    pub const LREAL: u32 = 11;
    /// `BYTE`.
    pub const BYTE: u32 = 12;
    /// `WORD`.
    pub const WORD: u32 = 13;
    /// `DWORD`.
    pub const DWORD: u32 = 14;
    /// `LWORD`.
    pub const LWORD: u32 = 15;
    /// `TIME`.
    pub const TIME: u32 = 16;
    /// `LTIME`.
    pub const LTIME: u32 = 17;
    /// `DATE`.
    pub const DATE: u32 = 18;
    /// `LDATE`.
    pub const LDATE: u32 = 19;
    /// `TIME_OF_DAY`.
    pub const TOD: u32 = 20;
    /// `LTIME_OF_DAY`.
    pub const LTOD: u32 = 21;
    /// `DATE_AND_TIME`.
    pub const DT: u32 = 22;
    /// `LDATE_AND_TIME`.
    pub const LDT: u32 = 23;
    /// `CHAR`.
    pub const CHAR: u32 = 24;
    /// `WCHAR`.
    pub const WCHAR: u32 = 25;
    /// An enumeration: its values are the detail, 16 bytes each (name, reserved, value i64).
    pub const ENUM: u32 = 100;
    /// A subrange: its base type and bounds are the detail (type u32, reserved, lower, upper).
    pub const SUBRANGE: u32 = 101;
    /// An array: its element type and its dimensions are the detail (type u32, reserved, then
    /// lower and upper i64 per dimension); the count is the number of dimensions.
    pub const ARRAY: u32 = 102;
    /// A structure: its members are the detail, 16 bytes each (name, type, offset, flags).
    pub const STRUCT: u32 = 103;
    /// A union: its variants are the detail, as a structure's members.
    pub const UNION: u32 = 104;
    /// `STRING[n]`: the count is the capacity.
    pub const STRING: u32 = 105;
    /// `WSTRING[n]`: the count is the capacity.
    pub const WSTRING: u32 = 106;
    /// `REF_TO`: the detail is the target type (u32).
    pub const REFERENCE: u32 = 107;
    /// `POINTER TO`: the detail is the target type (u32).
    pub const POINTER: u32 = 108;
    /// A function block: its variables are the detail, as a structure's members.
    pub const FUNCTION_BLOCK: u32 = 109;
    /// A class: its variables are the detail, as a structure's members.
    pub const CLASS: u32 = 110;
    /// An interface.
    pub const INTERFACE: u32 = 111;
    /// An alias: the detail is the base type (u32).
    pub const ALIAS: u32 = 112;
    /// A program: its variables are the detail, as a structure's members.
    pub const PROGRAM: u32 = 113;
}

/// One type: 24 bytes; its detail lies in the detail list that follows the type records in
/// `TYPES` (specification 44 §5.8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeRecord {
    /// A [`type_kind`].
    pub kind: u32,
    /// The type's name, a string offset, 0 for an anonymous type.
    pub name: u32,
    /// The bytes of a value.
    pub size: u32,
    /// The alignment of a value.
    pub alignment: u32,
    /// The detail's offset from the start of `TYPES`, 0 for none.
    pub detail: u32,
    /// Members, values, dimensions or the capacity, as the kind says.
    pub count: u32,
}

impl Record for TypeRecord {
    const SECTION: SectionId = SectionId::Types;
    const BYTES: usize = 24;
    fn read(bytes: &[u8]) -> Self {
        Self {
            kind: u32_at(bytes, 0),
            name: u32_at(bytes, 4),
            size: u32_at(bytes, 8),
            alignment: u32_at(bytes, 12),
            detail: u32_at(bytes, 16),
            count: u32_at(bytes, 20),
        }
    }
    fn write(&self, out: &mut [u8]) {
        put(out, 0, &self.kind.to_le_bytes());
        put(out, 4, &self.name.to_le_bytes());
        put(out, 8, &self.size.to_le_bytes());
        put(out, 12, &self.alignment.to_le_bytes());
        put(out, 16, &self.detail.to_le_bytes());
        put(out, 20, &self.count.to_le_bytes());
    }
}

/// The bytes of one member of a structure, union, function block, class or program in the detail
/// list: name, type, offset, flags (the symbol flags).
pub const MEMBER_BYTES: usize = 16;

/// One member in a type's detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemberDetail {
    /// The member's name, a string offset.
    pub name: u32,
    /// The member's type index.
    pub type_index: u32,
    /// The member's offset in its owner.
    pub offset: u32,
    /// The member's `SYMBOL_` flags.
    pub flags: u32,
}

impl MemberDetail {
    /// The member in `bytes`, [`MEMBER_BYTES`] long.
    #[must_use]
    pub fn read(bytes: &[u8]) -> Self {
        Self {
            name: u32_at(bytes, 0),
            type_index: u32_at(bytes, 4),
            offset: u32_at(bytes, 8),
            flags: u32_at(bytes, 12),
        }
    }

    /// The member's bytes.
    #[must_use]
    pub fn bytes(&self) -> [u8; MEMBER_BYTES] {
        let mut out = [0u8; MEMBER_BYTES];
        put(&mut out, 0, &self.name.to_le_bytes());
        put(&mut out, 4, &self.type_index.to_le_bytes());
        put(&mut out, 8, &self.offset.to_le_bytes());
        put(&mut out, 12, &self.flags.to_le_bytes());
        out
    }
}

/// One statement: 16 bytes; positions are 1-based (specification 44 §5.9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StmtRecord {
    /// The file's index in `FILES`.
    pub file: u16,
    /// The POU that holds the statement.
    pub pou: u16,
    /// The first line.
    pub start_line: u32,
    /// The first column.
    pub start_column: u16,
    /// The last line.
    pub end_line: u32,
    /// The column after the last character.
    pub end_column: u16,
}

impl Record for StmtRecord {
    const SECTION: SectionId = SectionId::Stmts;
    const BYTES: usize = 16;
    fn read(bytes: &[u8]) -> Self {
        Self {
            file: u16_at(bytes, 0),
            pou: u16_at(bytes, 2),
            start_line: u32_at(bytes, 4),
            start_column: u16_at(bytes, 8),
            end_line: u32_at(bytes, 10),
            end_column: u16_at(bytes, 14),
        }
    }
    fn write(&self, out: &mut [u8]) {
        put(out, 0, &self.file.to_le_bytes());
        put(out, 2, &self.pou.to_le_bytes());
        put(out, 4, &self.start_line.to_le_bytes());
        put(out, 8, &self.start_column.to_le_bytes());
        put(out, 10, &self.end_line.to_le_bytes());
        put(out, 14, &self.end_column.to_le_bytes());
    }
}

/// One retained variable: 16 bytes, in the order of specification 11 §6.7 (specification 44
/// §5.10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainRecord {
    /// Its checkpoint key, a string offset: a global's name, `@program/<program>/<variable>`, or
    /// the path of a variable of an instance.
    pub key: u32,
    /// The area it lies in: globals, retain or instances.
    pub area: u32,
    /// Its offset in the area.
    pub offset: u32,
    /// Its type's index.
    pub type_index: u32,
}

impl Record for RetainRecord {
    const SECTION: SectionId = SectionId::Retain;
    const BYTES: usize = 16;
    fn read(bytes: &[u8]) -> Self {
        Self {
            key: u32_at(bytes, 0),
            area: u32_at(bytes, 4),
            offset: u32_at(bytes, 8),
            type_index: u32_at(bytes, 12),
        }
    }
    fn write(&self, out: &mut [u8]) {
        put(out, 0, &self.key.to_le_bytes());
        put(out, 4, &self.area.to_le_bytes());
        put(out, 8, &self.offset.to_le_bytes());
        put(out, 12, &self.type_index.to_le_bytes());
    }
}

/// One source file: 16 bytes (specification 44 §5.3, `FILES`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileRecord {
    /// The file's label, a string offset.
    pub label: u32,
    /// Where its text begins in `SOURCE`, or [`NONE`] without one.
    pub source_offset: u32,
    /// The bytes of its text in `SOURCE`.
    pub source_length: u32,
}

impl Record for FileRecord {
    const SECTION: SectionId = SectionId::Files;
    const BYTES: usize = 16;
    fn read(bytes: &[u8]) -> Self {
        Self {
            label: u32_at(bytes, 0),
            source_offset: u32_at(bytes, 4),
            source_length: u32_at(bytes, 8),
        }
    }
    fn write(&self, out: &mut [u8]) {
        put(out, 0, &self.label.to_le_bytes());
        put(out, 4, &self.source_offset.to_le_bytes());
        put(out, 8, &self.source_length.to_le_bytes());
    }
}

/// The memory figure (specification 44 §10.4): what the program needs in RAM, computed by the
/// build, in bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemoryFigure {
    /// The areas' lengths.
    pub areas: u64,
    /// The stack bound of the deepest task (specification 44 §8.5).
    pub stack: u64,
    /// The kernel's bookkeeping for the program: tasks, breakpoint bits, Live Values queues.
    pub bookkeeping: u64,
    /// The retain capture buffer.
    pub retain_capture: u64,
    /// The sum of the four.
    pub total: u64,
    /// The module's code, which lies in flash on a chip.
    pub code: u64,
    /// The module's read-only data, in flash on a chip.
    pub rodata: u64,
    /// The starting memory, in flash on a chip.
    pub init: u64,
    /// The bytes of one call level of a recursion, which the total does not count: the most a
    /// recursive call adds to the stack until the next (specification 44 §8.5); 0 for a program
    /// without one.
    pub call_level: u64,
}

impl MemoryFigure {
    /// The bytes of `MEMFIG`.
    pub const BYTES: usize = 72;

    /// The figure in `bytes`.
    #[must_use]
    pub fn read(bytes: &[u8]) -> Self {
        Self {
            areas: u64_at(bytes, 0),
            stack: u64_at(bytes, 8),
            bookkeeping: u64_at(bytes, 16),
            retain_capture: u64_at(bytes, 24),
            total: u64_at(bytes, 32),
            code: u64_at(bytes, 40),
            rodata: u64_at(bytes, 48),
            init: u64_at(bytes, 56),
            call_level: u64_at(bytes, 64),
        }
    }

    /// The figure's bytes.
    #[must_use]
    pub fn bytes(&self) -> [u8; Self::BYTES] {
        let mut out = [0u8; Self::BYTES];
        for (index, value) in [
            self.areas,
            self.stack,
            self.bookkeeping,
            self.retain_capture,
            self.total,
            self.code,
            self.rodata,
            self.init,
            self.call_level,
        ]
        .into_iter()
        .enumerate()
        {
            put(&mut out, index * 8, &value.to_le_bytes());
        }
        out
    }
}
