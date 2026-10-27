//! An image written by hand that has a type of every kind a value is admitted to, variables of
//! some of them and process-image bindings, for the tests of the value grammar and of the requests
//! that type their values (specification 11, "Runtime control I/O and variable mutation contract").
//!
//! ```text
//! TYPE Color : (Red, Green, Blue) INT; END_TYPE
//! TYPE Percent : INT(0..100); END_TYPE
//! TYPE Point : STRUCT X : INT; Y : INT; END_STRUCT; END_TYPE
//! VAR_GLOBAL
//!     Setpoint AT %MD4 : REAL;           (* G 0 *)
//!     Level AT %MW2 : INT;               (* G 4 *)
//!     Lamp AT %MX0.1 : BOOL;             (* G 6 *)
//!     Delay AT %QD0 : TIME;              (* G 8, bound in whole milliseconds *)
//!     Mode : Color;                      (* G 16 *)
//!     Name : STRING[5];                  (* G 18 *)
//! END_VAR
//! VAR_GLOBAL CONSTANT Limit : INT := 7; END_VAR      (* G 26 *)
//! ```

use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::native::image::{
    type_kind, AreaRecord, CodeImage, EntryRecord, FileRecord, ImageWriter, IoBindRecord,
    MemberDetail, MemoryFigure, Record, RetainRecord, SectionId, StmtRecord, StringPool,
    SymbolRecord, Target, TaskRecord, TypeRecord, BIND_TIME_MILLIS, NONE, SYMBOL_CONSTANT,
};

/// The index of each type of the table, by its declaration.
pub(crate) mod types {
    pub(crate) const BOOL: u32 = 0;
    pub(crate) const SINT: u32 = 1;
    pub(crate) const INT: u32 = 2;
    pub(crate) const DINT: u32 = 3;
    pub(crate) const LINT: u32 = 4;
    pub(crate) const USINT: u32 = 5;
    pub(crate) const UINT: u32 = 6;
    pub(crate) const UDINT: u32 = 7;
    pub(crate) const ULINT: u32 = 8;
    pub(crate) const REAL: u32 = 9;
    pub(crate) const LREAL: u32 = 10;
    pub(crate) const BYTE: u32 = 11;
    pub(crate) const WORD: u32 = 12;
    pub(crate) const DWORD: u32 = 13;
    pub(crate) const LWORD: u32 = 14;
    pub(crate) const TIME: u32 = 15;
    pub(crate) const LTIME: u32 = 16;
    pub(crate) const DATE: u32 = 17;
    pub(crate) const LDATE: u32 = 18;
    pub(crate) const TOD: u32 = 19;
    pub(crate) const LTOD: u32 = 20;
    pub(crate) const DT: u32 = 21;
    pub(crate) const LDT: u32 = 22;
    pub(crate) const CHAR: u32 = 23;
    pub(crate) const WCHAR: u32 = 24;
    /// `STRING[5]`.
    pub(crate) const STRING5: u32 = 25;
    /// `WSTRING[3]`.
    pub(crate) const WSTRING3: u32 = 26;
    pub(crate) const COLOR: u32 = 27;
    pub(crate) const PERCENT: u32 = 28;
    pub(crate) const POINT: u32 = 29;
    /// `ARRAY[1..3] OF INT`.
    pub(crate) const INTS: u32 = 30;
}

/// The type table and its detail list.
fn type_table(strings: &mut StringPool) -> (Vec<TypeRecord>, Vec<u8>) {
    let count = 31usize;
    let base = TABLE_PREFIX + count * TypeRecord::BYTES;
    let mut details = Vec::new();
    let at = |details: &[u8]| u32::try_from(base + details.len()).expect("a small table");
    let color = at(&details);
    for (name, value) in [("Red", 0i64), ("Green", 1), ("Blue", 2)] {
        details.extend_from_slice(&strings.add(name).to_le_bytes());
        details.extend_from_slice(&0u32.to_le_bytes());
        details.extend_from_slice(&value.to_le_bytes());
    }
    let percent = at(&details);
    details.extend_from_slice(&type_kind::INT.to_le_bytes());
    details.extend_from_slice(&0u32.to_le_bytes());
    details.extend_from_slice(&0i64.to_le_bytes());
    details.extend_from_slice(&100i64.to_le_bytes());
    let point = at(&details);
    for (name, offset) in [("X", 0), ("Y", 2)] {
        let member = MemberDetail {
            name: strings.add(name),
            type_index: types::INT,
            offset,
            flags: 0,
        };
        details.extend_from_slice(&member.bytes());
    }
    let ints = at(&details);
    details.extend_from_slice(&types::INT.to_le_bytes());
    details.extend_from_slice(&0u32.to_le_bytes());
    details.extend_from_slice(&1i64.to_le_bytes());
    details.extend_from_slice(&3i64.to_le_bytes());
    let mut records = Vec::new();
    for (kind, name, size) in [
        (type_kind::BOOL, "BOOL", 1),
        (type_kind::SINT, "SINT", 1),
        (type_kind::INT, "INT", 2),
        (type_kind::DINT, "DINT", 4),
        (type_kind::LINT, "LINT", 8),
        (type_kind::USINT, "USINT", 1),
        (type_kind::UINT, "UINT", 2),
        (type_kind::UDINT, "UDINT", 4),
        (type_kind::ULINT, "ULINT", 8),
        (type_kind::REAL, "REAL", 4),
        (type_kind::LREAL, "LREAL", 8),
        (type_kind::BYTE, "BYTE", 1),
        (type_kind::WORD, "WORD", 2),
        (type_kind::DWORD, "DWORD", 4),
        (type_kind::LWORD, "LWORD", 8),
        (type_kind::TIME, "TIME", 8),
        (type_kind::LTIME, "LTIME", 8),
        (type_kind::DATE, "DATE", 8),
        (type_kind::LDATE, "LDATE", 8),
        (type_kind::TOD, "TIME_OF_DAY", 8),
        (type_kind::LTOD, "LTIME_OF_DAY", 8),
        (type_kind::DT, "DATE_AND_TIME", 8),
        (type_kind::LDT, "LDATE_AND_TIME", 8),
        (type_kind::CHAR, "CHAR", 1),
        (type_kind::WCHAR, "WCHAR", 2),
    ] {
        records.push(TypeRecord {
            kind,
            name: strings.add(name),
            size,
            alignment: size,
            detail: 0,
            count: 0,
        });
    }
    let record = |kind, name, size, detail, count| TypeRecord {
        kind,
        name,
        size,
        alignment: 2,
        detail,
        count,
    };
    records.extend([
        record(type_kind::STRING, strings.add("STRING"), 7, 0, 5),
        record(type_kind::WSTRING, strings.add("WSTRING"), 8, 0, 3),
        record(type_kind::ENUM, strings.add("Color"), 2, color, 3),
        record(type_kind::SUBRANGE, strings.add("Percent"), 2, percent, 0),
        record(type_kind::STRUCT, strings.add("Point"), 4, point, 2),
        record(type_kind::ARRAY, 0, 6, ints, 1),
    ]);
    assert_eq!(records.len(), count);
    (records, details)
}

/// The bytes before a table's records: its count.
const TABLE_PREFIX: usize = trust_runtime_core::native::image::TABLE_PREFIX_BYTES;

/// One symbol: canonical name, display name, type, flags and offset in the globals.
type SymbolRow = (&'static str, &'static str, u32, u8, u32, u32);

/// The globals, sorted by canonical name.
const SYMBOLS: [SymbolRow; 7] = [
    ("DELAY", "Delay", types::TIME, 0, 8, 8),
    ("LAMP", "Lamp", types::BOOL, 0, 6, 1),
    ("LEVEL", "Level", types::INT, 0, 4, 2),
    ("LIMIT", "Limit", types::INT, SYMBOL_CONSTANT, 26, 2),
    ("MODE", "Mode", types::COLOR, 0, 16, 2),
    ("NAME", "Name", types::STRING5, 0, 18, 7),
    ("SETPOINT", "Setpoint", types::REAL, 0, 0, 4),
];

/// The bindings: `Setpoint` at `%MD4`, `Level` at `%MW2`, `Lamp` at `%MX0.1` and `Delay` at
/// `%QD0` in whole milliseconds.
fn bindings() -> [IoBindRecord; 4] {
    let bind = |symbol, area, size, byte, bit, flags| IoBindRecord {
        symbol,
        area,
        size,
        byte,
        bit,
        kind: 0,
        flags,
    };
    [
        bind(6, 3, 3, 4, 0, 0),
        bind(2, 3, 2, 2, 0, 0),
        bind(1, 3, 0, 0, 1, 0),
        bind(0, 2, 3, 0, 0, BIND_TIME_MILLIS),
    ]
}

/// The image of the declarations above.
pub(crate) fn image() -> Vec<u8> {
    let mut strings = StringPool::new();
    let (types, details) = type_table(&mut strings);
    let symbols: Vec<SymbolRecord> = SYMBOLS
        .iter()
        .map(
            |&(canonical, display, type_index, flags, offset, size)| SymbolRecord {
                canonical: strings.add(canonical),
                display: strings.add(display),
                type_index,
                area: 4,
                flags,
                offset,
                size,
                parent: NONE,
                pou: NONE,
            },
        )
        .collect();
    let label = strings.add("main.st");
    let mut writer = ImageWriter::new(
        Target::this_cpu().unwrap_or(Target::X86_64),
        u16::try_from(trust_plc_abi::INTERFACE_VERSION).expect("a 16-bit interface version"),
    );
    writer.module(CodeImage {
        code: vec![0; 64],
        rodata: Vec::new(),
        rodata_distance: 64,
    });
    writer.table::<EntryRecord>(&[], &[]);
    let area = |id: u32, length: u64| AreaRecord {
        id,
        length,
        alignment: 8,
        flags: 0,
        init_offset: 0,
    };
    writer.table(
        &[
            area(1, 0),
            area(2, 4),
            area(3, 8),
            area(4, 28),
            area(5, 0),
            area(6, 0),
        ],
        &[],
    );
    writer.section(SectionId::Init, Vec::new());
    writer.table::<TaskRecord>(&[], &[]);
    writer.table(&bindings(), &[]);
    writer.table(&symbols, &[]);
    writer.table(&types, &details);
    writer.table::<StmtRecord>(&[], &[]);
    writer.table::<RetainRecord>(&[], &[]);
    writer.table(
        &[FileRecord {
            label,
            source_offset: NONE,
            source_length: 0,
        }],
        &[],
    );
    writer.section(SectionId::MemFig, MemoryFigure::default().bytes().to_vec());
    writer.section(SectionId::Strings, strings.into_bytes());
    writer.finish().expect("the image builds")
}
