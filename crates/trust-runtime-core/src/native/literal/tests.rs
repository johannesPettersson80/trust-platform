//! Integers that Live Values writes into a compiled program (specification 11, "An admitted
//! integer is an untyped literal"), over an image written by hand whose type table holds `INT`
//! (0), `USINT` (1), `BOOL` (2), `INT(0..10)` (3) and an alias of `WORD` (4), and whose one
//! binding puts the `INT` variable `MAIN.REG` at `%MW0`.

use alloc::vec;
use alloc::vec::Vec;

use super::{for_address, for_variable};
use crate::error::RuntimeError;
use crate::io_address::IoAddress;
use crate::native::image::{
    type_kind, AreaRecord, CodeImage, EntryRecord, FileRecord, Image, ImageWriter, IoBindRecord,
    MemoryFigure, Record, RetainRecord, SectionId, StmtRecord, StringPool, SymbolRecord, Target,
    TaskRecord, TypeRecord, NONE, TABLE_PREFIX_BYTES,
};
use crate::value::Value;

const INT: u32 = 0;
const USINT: u32 = 1;
const BOOL: u32 = 2;
const SMALL: u32 = 3;
const WORD_ALIAS: u32 = 4;

fn image() -> Vec<u8> {
    let mut strings = StringPool::new();
    let reg = strings.add("MAIN.REG");
    let label = strings.add("main.st");
    let mut writer = ImageWriter::new(
        Target::this_cpu().unwrap_or(Target::X86_64),
        u16::try_from(trust_plc_abi::INTERFACE_VERSION).unwrap(),
    );
    writer.module(CodeImage {
        code: vec![0; 64],
        rodata: Vec::new(),
        rodata_distance: 64,
    });
    writer.table::<EntryRecord>(&[], &[]);
    writer.section(SectionId::Init, Vec::new());
    let area = |id: u8, length: u64| AreaRecord {
        id: u32::from(id),
        length,
        alignment: 8,
        flags: 0,
        init_offset: 0,
    };
    writer.table(
        &[
            area(1, 0),
            area(2, 0),
            area(3, 2),
            area(4, 0),
            area(5, 0),
            area(6, 2),
        ],
        &[],
    );
    writer.table::<TaskRecord>(&[], &[]);
    writer.table(
        &[IoBindRecord {
            symbol: 0,
            area: 3,
            size: 2,
            byte: 0,
            bit: 0,
            flags: 0,
        }],
        &[],
    );
    writer.table(
        &[SymbolRecord {
            canonical: reg,
            display: reg,
            type_index: INT,
            area: 6,
            flags: 0,
            offset: 0,
            size: 2,
            parent: NONE,
            pou: NONE,
        }],
        &[],
    );
    // The details: the subrange's base kind and bounds (24 bytes), then the alias's base (4).
    let mut details = Vec::new();
    details.extend_from_slice(&type_kind::INT.to_le_bytes());
    details.extend_from_slice(&[0; 4]);
    details.extend_from_slice(&0i64.to_le_bytes());
    details.extend_from_slice(&10i64.to_le_bytes());
    details.extend_from_slice(&5u32.to_le_bytes());
    // A detail offset counts from the start of the TYPES section, past its prefix and its records.
    let base = u32::try_from(TABLE_PREFIX_BYTES + 6 * TypeRecord::BYTES).unwrap();
    let elementary = |kind: u32, size: u32| TypeRecord {
        kind,
        name: 0,
        size,
        alignment: size,
        detail: 0,
        count: 0,
    };
    writer.table(
        &[
            elementary(type_kind::INT, 2),
            elementary(type_kind::USINT, 1),
            elementary(type_kind::BOOL, 1),
            TypeRecord {
                detail: base,
                ..elementary(type_kind::SUBRANGE, 2)
            },
            TypeRecord {
                detail: base + 24,
                ..elementary(type_kind::ALIAS, 2)
            },
            elementary(type_kind::WORD, 2),
        ],
        &details,
    );
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
    writer.section(
        SectionId::MemFig,
        MemoryFigure {
            total: 100,
            ..MemoryFigure::default()
        }
        .bytes()
        .to_vec(),
    );
    writer.section(SectionId::Strings, strings.into_bytes());
    writer.finish().expect("the image builds")
}

#[test]
fn an_integer_takes_the_declared_type_of_its_variable() {
    // Specification 11, "An admitted integer is an untyped literal", worked by hand: 40 into a
    // USINT is the USINT 40; 300 overflows it; 5 into INT(0..10) is the INT 5 (the variable's own
    // bounds check follows); 7 into an alias of WORD is the WORD 7; a literal into a BOOL and a
    // value that is no literal pass through, for the variable's type to refuse or accept. Catches
    // an integer kept as LINT, which every narrower variable refused on the C6 bench (24 October
    // 2026: `set` of 2 into an INT), and a literal wrapped into a narrower type.
    let bytes = image();
    let image = Image::parse(&bytes).expect("the image parses");
    assert_eq!(
        for_variable(&image, USINT, &Value::LInt(40)),
        Ok(Some(Value::USInt(40)))
    );
    assert_eq!(
        for_variable(&image, USINT, &Value::LInt(300)),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        for_variable(&image, SMALL, &Value::LInt(5)),
        Ok(Some(Value::Int(5)))
    );
    assert_eq!(
        for_variable(&image, WORD_ALIAS, &Value::LInt(7)),
        Ok(Some(Value::Word(7)))
    );
    assert_eq!(for_variable(&image, BOOL, &Value::LInt(1)), Ok(None));
    assert_eq!(for_variable(&image, INT, &Value::Bool(true)), Ok(None));
}

#[test]
fn an_integer_at_an_address_takes_the_bound_variables_type_or_the_addresss_width() {
    // Worked by hand: %MW0 holds the INT `MAIN.REG`, so -5 is the INT -5 and the image holds its
    // bits, the WORD 0xFFFB, and 40,000 overflows the INT; %MW2 has no variable, so 40,000 is the
    // WORD 40,000 and -1 overflows the WORD; %MB4 takes the BYTE 200; a literal at a bit address
    // passes through. Catches an image write of a literal that the image refuses, a signed
    // variable's literal refused as a bit string, and a literal wrapped into the width.
    let bytes = image();
    let image = Image::parse(&bytes).expect("the image parses");
    let at = |text: &str| IoAddress::parse(text).expect("an address");
    assert_eq!(
        for_address(&image, &at("%MW0"), Value::LInt(-5)),
        Ok(Value::Word(0xFFFB))
    );
    assert_eq!(
        for_address(&image, &at("%MW0"), Value::LInt(40_000)),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        for_address(&image, &at("%MW2"), Value::LInt(40_000)),
        Ok(Value::Word(40_000))
    );
    assert_eq!(
        for_address(&image, &at("%MW2"), Value::LInt(-1)),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        for_address(&image, &at("%MB4"), Value::LInt(200)),
        Ok(Value::Byte(200))
    );
    assert_eq!(
        for_address(&image, &at("%QX0.0"), Value::LInt(1)),
        Ok(Value::LInt(1))
    );
}
