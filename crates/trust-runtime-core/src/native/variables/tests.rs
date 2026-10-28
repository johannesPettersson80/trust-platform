//! A compiled program's variables reached by path for a write, and its process images by address
//! (specification 44 §13.3; specification 11, "Runtime control I/O and variable mutation contract"),
//! over an image written by hand:
//!
//! ```text
//! TYPE Point : STRUCT X : INT; Y : INT; END_STRUCT; END_TYPE              (* 4 bytes *)
//! FUNCTION_BLOCK Pump VAR Speed : INT; Pos : Point; END_VAR               (* 6 bytes: +0, +2 *)
//! VAR_GLOBAL
//!     Table : ARRAY[1..2, 1..3] OF INT;      (* G 0, 12 bytes *)
//!     Origin : Point;                         (* G 12 *)
//! END_VAR
//! VAR_GLOBAL CONSTANT
//!     Limit : INT := 7;                       (* G 16 *)
//!     Cfg : Point := (X := 1, Y := 2);        (* G 18 *)
//! END_VAR
//! PROGRAM MainProg VAR Pumps : ARRAY[1..3] OF Pump; Count : DINT; END_VAR    (* P 0, P 20 *)
//! (* the instance Main of MainProg at P 0, 24 bytes; inputs %IB0..%IB1, outputs %QB0 *)
//! ```
//!
//! The starting values are left at zero: the tests read what their writes leave.

use alloc::vec;
use alloc::vec::Vec;

use trust_plc_abi::kernel::{Entry, WITHOUT_DEBUGGER};
use trust_plc_abi::module::run;
use trust_plc_abi::Cx;

use super::CompiledVariables;
use crate::error::RuntimeError;
use crate::io_address::IoAddress;
use crate::native::image::{
    type_kind, AreaRecord, CodeImage, EntryKind, EntryRecord, FileRecord, Image, ImageWriter,
    IoBindRecord, MemberDetail, MemoryFigure, Record, RetainRecord, SectionId, StmtRecord,
    StringPool, SymbolRecord, Target, TaskRecord, TypeRecord, NONE, SYMBOL_CONSTANT,
    SYMBOL_PROGRAM, TABLE_PREFIX_BYTES,
};
use crate::native::memory::Place;
use crate::native::program::{admit, Device, ModuleCode, NativeProgram};
use crate::native::reflect::{
    array_shape, leaf_type, path_of, resolve_for_write, resolve_path, Located, PathError,
};
use crate::value::{Duration, Value};

#[cfg(target_arch = "x86_64")]
extern "sysv64" fn nothing(cx: *mut Cx) -> u32 {
    run(cx, |_, _| Ok(()))
}

#[cfg(not(target_arch = "x86_64"))]
extern "C" fn nothing(cx: *mut Cx) -> u32 {
    run(cx, |_, _| Ok(()))
}

/// A module whose every entry point does nothing.
struct Idle;

impl ModuleCode for Idle {
    fn entry(&self, _offset: u32) -> Option<Entry> {
        Some(Entry::from_fn(nothing))
    }
}

/// The type `INT`.
const INT: u32 = 0;
/// The type `DINT`.
const DINT: u32 = 1;
/// The structure `Point`.
const POINT: u32 = 2;
/// The function block `Pump`.
const PUMP: u32 = 3;
/// `ARRAY[1..3] OF Pump`.
const PUMPS: u32 = 4;
/// `ARRAY[1..2, 1..3] OF INT`.
const TABLE: u32 = 5;
/// The program type `MainProg`.
const MAIN: u32 = 6;

/// The type table of the declarations above and its detail list.
fn types(strings: &mut StringPool) -> (Vec<TypeRecord>, Vec<u8>) {
    let count = 7usize;
    let base = TABLE_PREFIX_BYTES + count * TypeRecord::BYTES;
    let mut details = Vec::new();
    let at = |details: &[u8]| u32::try_from(base + details.len()).unwrap();
    let mut members = |details: &mut Vec<u8>, rows: &[(&str, u32, u32)]| {
        for &(name, type_index, offset) in rows {
            let member = MemberDetail {
                name: strings.add(name),
                type_index,
                offset,
                flags: 0,
            };
            details.extend_from_slice(&member.bytes());
        }
    };
    let point = at(&details);
    members(&mut details, &[("X", INT, 0), ("Y", INT, 2)]);
    let pump = at(&details);
    members(&mut details, &[("Speed", INT, 0), ("Pos", POINT, 2)]);
    // A function block's detail names its base, none, and its interfaces, none (§5.8).
    details.extend_from_slice(&NONE.to_le_bytes());
    details.extend_from_slice(&0u32.to_le_bytes());
    let pumps = at(&details);
    details.extend_from_slice(&PUMP.to_le_bytes());
    details.extend_from_slice(&0u32.to_le_bytes());
    details.extend_from_slice(&1i64.to_le_bytes());
    details.extend_from_slice(&3i64.to_le_bytes());
    let table = at(&details);
    details.extend_from_slice(&INT.to_le_bytes());
    details.extend_from_slice(&0u32.to_le_bytes());
    for (lower, upper) in [(1i64, 2i64), (1, 3)] {
        details.extend_from_slice(&lower.to_le_bytes());
        details.extend_from_slice(&upper.to_le_bytes());
    }
    let main = at(&details);
    members(&mut details, &[("Pumps", PUMPS, 0), ("Count", DINT, 20)]);
    let record = |kind, name, size, alignment, detail, count| TypeRecord {
        kind,
        name,
        size,
        alignment,
        detail,
        count,
    };
    let types = vec![
        record(type_kind::INT, strings.add("INT"), 2, 2, 0, 0),
        record(type_kind::DINT, strings.add("DINT"), 4, 4, 0, 0),
        record(type_kind::STRUCT, strings.add("Point"), 4, 2, point, 2),
        record(
            type_kind::FUNCTION_BLOCK,
            strings.add("Pump"),
            6,
            2,
            pump,
            2,
        ),
        record(type_kind::ARRAY, 0, 18, 2, pumps, 1),
        record(type_kind::ARRAY, 0, 12, 2, table, 2),
        record(type_kind::PROGRAM, strings.add("MainProg"), 24, 4, main, 2),
    ];
    assert_eq!(types.len(), count);
    (types, details)
}

/// One symbol: canonical name, display name, type, area, flags, offset, size and parent.
type SymbolRow = (&'static str, &'static str, u32, u8, u8, u32, u32, u32);

/// The symbols of the declarations above, sorted by canonical name.
const SYMBOLS: [SymbolRow; 7] = [
    ("CFG", "Cfg", POINT, 4, SYMBOL_CONSTANT, 18, 4, NONE),
    ("LIMIT", "Limit", INT, 4, SYMBOL_CONSTANT, 16, 2, NONE),
    ("MAIN", "Main", MAIN, 6, SYMBOL_PROGRAM, 0, 24, NONE),
    ("MAIN.COUNT", "Main.Count", DINT, 6, 0, 20, 4, 2),
    ("MAIN.PUMPS", "Main.Pumps", PUMPS, 6, 0, 0, 18, 2),
    ("ORIGIN", "Origin", POINT, 4, 0, 12, 4, NONE),
    ("TABLE", "Table", TABLE, 4, 0, 0, 12, NONE),
];

/// The image of the declarations above.
fn image() -> Vec<u8> {
    let mut strings = StringPool::new();
    let (types, details) = types(&mut strings);
    let symbols: Vec<SymbolRecord> = SYMBOLS
        .iter()
        .map(
            |&(canonical, display, type_index, area, flags, offset, size, parent)| SymbolRecord {
                canonical: strings.add(canonical),
                display: strings.add(display),
                type_index,
                area,
                flags,
                offset,
                size,
                parent,
                pou: if canonical == "MAIN" { 0 } else { NONE },
            },
        )
        .collect();
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
    writer.table(
        &[EntryRecord {
            kind: EntryKind::ColdInit as u32,
            index: 0,
            code_offset: 0,
        }],
        &[],
    );
    let area = |id: u8, length: u64| AreaRecord {
        id: u32::from(id),
        length,
        alignment: 8,
        flags: 0,
        init_offset: 0,
    };
    writer.table(
        &[
            area(1, 2),
            area(2, 1),
            area(3, 0),
            area(4, 22),
            area(5, 0),
            area(6, 24),
        ],
        &[],
    );
    writer.section(SectionId::Init, Vec::new());
    writer.table::<TaskRecord>(&[], &[]);
    writer.table::<IoBindRecord>(&[], &[]);
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
    let figure = MemoryFigure {
        total: 1000,
        stack: 64,
        ..MemoryFigure::default()
    };
    writer.section(SectionId::MemFig, figure.bytes().to_vec());
    writer.section(SectionId::Strings, strings.into_bytes());
    writer.finish().unwrap()
}

fn device() -> Device {
    Device {
        target: Target::this_cpu().unwrap_or(Target::X86_64),
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 1000,
        trusted_keys: &[],
    }
}

/// The program of `bytes`, started cold.
fn program(bytes: &[u8]) -> NativeProgram<'_, Idle> {
    let device = device();
    let mut program = NativeProgram::new(
        admit(bytes, &device).unwrap(),
        Idle,
        &device,
        &WITHOUT_DEBUGGER,
    )
    .unwrap();
    program.cold_start(Duration::ZERO).unwrap();
    program
}

fn area(area: u8, offset: u32) -> Place {
    Place::Area { area, offset }
}

/// The variable at `place` of type `type_index`.
fn at(place: Place, type_index: u32) -> Located {
    Located { place, type_index }
}

#[test]
fn a_path_resolves_through_elements_members_and_two_dimensions() {
    // Specification 44 §13.3, worked by hand: `Pumps` lies at P 0 and an element of
    // `ARRAY[1..3] OF Pump` is 6 bytes, so element 2 lies at P 6 and its `Speed` at +0, P 6; element 3
    // lies at P 12, its `Pos` at +2 and `Pos.Y` at +2 more, P 16; `Table[2, 3]` is the row-major
    // position (2 - 1) * 3 + (3 - 1) = 5 of `ARRAY[1..2, 1..3] OF INT`, 5 * 2 = G 10. The names are
    // compared without case. Catches an element counted from 0, a member at its instance's own
    // offset, a column-major position, and names compared with case.
    let bytes = image();
    let image = Image::parse(&bytes).unwrap();
    let none = |_: &str| None;
    assert_eq!(
        resolve_for_write(&image, "Main.Pumps[2].Speed", &none),
        Ok(at(area(6, 6), INT))
    );
    assert_eq!(
        resolve_for_write(&image, "main.PUMPS[3].pos.y", &none),
        Ok(at(area(6, 16), INT))
    );
    assert_eq!(
        resolve_for_write(&image, "Table[2, 3]", &none),
        Ok(at(area(4, 10), INT))
    );
    assert_eq!(
        resolve_for_write(&image, "Origin.Y", &none),
        Ok(at(area(4, 14), INT))
    );
}

#[test]
fn a_write_refuses_a_constant_an_instance_and_a_path_the_program_does_not_hold() {
    // Specification 11 ("Runtime control I/O and variable mutation contract") and IEC 61131-3
    // §6.5.6: a constant, a member of one and an instance, which holds no value, are refused with
    // their own reason; an element past an array's bounds, a member a block does not declare and a
    // name nothing declares name no variable. Catches a constant written, an instance taken for its
    // first member, and an index outside the bounds taken modulo.
    let bytes = image();
    let image = Image::parse(&bytes).unwrap();
    let none = |_: &str| None;
    let refusal = |path: &str| resolve_for_write(&image, path, &none);
    assert_eq!(refusal("Limit"), Err(PathError::Constant("Limit".into())));
    assert_eq!(refusal("cfg.X"), Err(PathError::Constant("cfg.X".into())));
    assert_eq!(
        refusal("Main.Pumps[2]"),
        Err(PathError::NoValue("Main.Pumps[2]".into()))
    );
    assert_eq!(refusal("Main"), Err(PathError::NoValue("Main".into())));
    assert_eq!(
        refusal("Main.Pumps[4].Speed"),
        Err(PathError::Index("Main.Pumps[4]".into()))
    );
    assert_eq!(
        refusal("Main.Pumps[2].Nothing"),
        Err(PathError::NoMember("Main.Pumps[2].Nothing".into()))
    );
    assert_eq!(
        refusal("Nothing"),
        Err(PathError::Unknown("Nothing".into()))
    );
    // A path that does not resolve for a write still resolves for a read: a constant reads.
    assert_eq!(resolve_path(&image, "Limit"), Ok(at(area(4, 16), INT)));
}

#[test]
fn a_named_write_takes_a_path_and_leaves_the_rest_of_the_memory() {
    // Specification 44 §13.3: a Live Values write by name reaches a member of an element of an
    // instance array, a field of a structure in an instance and an element of a two-dimensional
    // array; an integer literal takes the declared INT. Each write changes only its own two bytes:
    // the neighbouring elements and members still read 0. Catches a write by name that finds only
    // symbols (`Main.Pumps[2].Speed` has none), and a write of the wrong element.
    let bytes = image();
    let mut program = program(&bytes);
    CompiledVariables::write_named(&mut program, "main.pumps[2].speed", &Value::LInt(42)).unwrap();
    CompiledVariables::write_named(&mut program, "Main.Pumps[3].Pos.Y", &Value::LInt(-7)).unwrap();
    CompiledVariables::write_named(&mut program, "Table[2, 3]", &Value::LInt(99)).unwrap();
    let read = |path: &str| CompiledVariables::read_named(&program, path);
    assert_eq!(read("Main.Pumps[2].Speed"), Ok(Value::Int(42)));
    assert_eq!(read("Main.Pumps[3].Pos.Y"), Ok(Value::Int(-7)));
    assert_eq!(read("Table[2,3]"), Ok(Value::Int(99)));
    for untouched in [
        "Main.Pumps[1].Speed",
        "Main.Pumps[3].Speed",
        "Main.Pumps[3].Pos.X",
        "Table[1, 3]",
        "Table[2, 2]",
    ] {
        assert_eq!(read(untouched), Ok(Value::Int(0)), "{untouched}");
    }
    let mut instances = [0u8; 24];
    assert!(program.read(6, 0, &mut instances));
    assert_eq!(&instances[6..8], &42i16.to_le_bytes());
    assert_eq!(&instances[16..18], &(-7i16).to_le_bytes());
}

#[test]
fn a_refused_named_write_says_why_and_changes_nothing() {
    // Specification 11 ("Runtime control I/O and variable mutation contract"): a constant, a member
    // of a constant and an instance are refused with a plain reason, a path the program does not
    // hold as an undefined variable, and a value the type refuses with its error; every refusal
    // leaves the memory as it was (all zero). Catches a constant written through its symbol, an
    // instance written as its first member, and a refused write that changed a byte first.
    let bytes = image();
    let mut program = program(&bytes);
    let mut write =
        |name: &str, value: Value| CompiledVariables::write_named(&mut program, name, &value);
    let reason = |text: &str| Err(RuntimeError::ControlError(text.into()));
    assert_eq!(
        write("Limit", Value::LInt(8)),
        reason("Limit is a constant")
    );
    assert_eq!(
        write("Cfg.X", Value::LInt(8)),
        reason("Cfg.X is a constant")
    );
    assert_eq!(
        write("Main.Pumps[2]", Value::LInt(8)),
        reason("Main.Pumps[2] is an instance, which holds no value")
    );
    assert_eq!(
        write("Main", Value::LInt(8)),
        reason("Main is an instance, which holds no value")
    );
    assert_eq!(
        write("Main.Pumps[4].Speed", Value::LInt(8)),
        Err(RuntimeError::UndefinedVariable(
            "Main.Pumps[4].Speed".into()
        ))
    );
    assert_eq!(
        write("Main.Pumps[2].Speed", Value::Bool(true)),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        write("Main.Pumps[2].Speed", Value::LInt(70_000)),
        Err(RuntimeError::Overflow)
    );
    let mut globals = [0xFFu8; 22];
    let mut instances = [0xFFu8; 24];
    assert!(program.read(4, 0, &mut globals));
    assert!(program.read(6, 0, &mut instances));
    assert_eq!(globals, [0; 22]);
    assert_eq!(instances, [0; 24]);
}

#[test]
fn a_place_has_the_path_that_finds_it_again() {
    // Specification 44 §5.7 and §5.8: the path of a place is the display name of the innermost
    // symbol that holds it, then the members and elements down to it, which resolve back to the
    // place. Worked by hand from the layout above. A frame variable has no path. Catches an outer
    // symbol taken where an inner one names the place, an element index counted from 0, and a
    // member found by offset alone whatever its type (P 6 is both `Pumps[2]` and its `Speed`).
    let bytes = image();
    let image = Image::parse(&bytes).unwrap();
    for (located, path) in [
        (at(area(6, 16), INT), "Main.Pumps[3].Pos.Y"),
        (at(area(6, 6), INT), "Main.Pumps[2].Speed"),
        (at(area(6, 6), PUMP), "Main.Pumps[2]"),
        (at(area(4, 10), INT), "Table[2,3]"),
        (at(area(6, 20), DINT), "Main.Count"),
        (at(area(4, 14), INT), "Origin.Y"),
    ] {
        assert_eq!(path_of(&image, &located), Ok(Some(path.into())), "{path}");
        assert_eq!(resolve_path(&image, path), Ok(located), "{path}");
    }
    let frame = at(
        Place::Frame {
            frame: 0,
            offset: 0,
        },
        INT,
    );
    assert_eq!(path_of(&image, &frame), Ok(None));
}

#[test]
fn a_leaf_and_an_array_are_found_by_their_layout() {
    // Specification 44 §5.6 and §5.8: the value at +4 of a `Pump` is `Pos.Y`, an INT; nothing
    // starts at +1 of a `Point`; `ARRAY[1..2, 1..3] OF INT` has 6 elements 2 bytes apart, and its
    // position 5 is `[2, 3]`, the last. Catches a leaf found by its offset inside a scalar, and
    // indices that move the first dimension fastest.
    let bytes = image();
    let image = Image::parse(&bytes).unwrap();
    assert_eq!(leaf_type(&image, PUMP, 4, 2), Ok(Some(INT)));
    assert_eq!(leaf_type(&image, POINT, 1, 1), Ok(None));
    let shape = array_shape(&image, TABLE).unwrap().unwrap();
    assert_eq!((shape.element, shape.stride, shape.count()), (INT, 2, 6));
    assert_eq!(shape.indices(0), vec![1, 1]);
    assert_eq!(shape.indices(3), vec![2, 1]);
    assert_eq!(shape.indices(5), vec![2, 3]);
    assert_eq!(array_shape(&image, POINT), Ok(None));
}

#[test]
fn a_write_past_the_input_or_output_area_is_refused_and_the_image_keeps_its_length() {
    // Specification 44 §5.6 (0.35): the input area is 2 bytes and the output area 1, so `%IB1` and
    // `%QX0.7` are the last places a write reaches; `%IB2`, `%IW1` (bytes 1 and 2) and `%QB1` lie
    // past them and are refused with their area's reason, and neither image grows, so the sample
    // and the publication still copy them. Catches an image grown by a write, which stopped every
    // input's sampling.
    let bytes = image();
    let mut program = program(&bytes);
    let address = |text: &str| IoAddress::parse(text).unwrap();
    program
        .write_address(&address("%IB1"), Value::Byte(7))
        .unwrap();
    program
        .write_address(&address("%QX0.7"), Value::Bool(true))
        .unwrap();
    assert_eq!(program.read_address(&address("%IB1")), Ok(Value::Byte(7)));
    assert_eq!(
        program.read_address(&address("%QB0")),
        Ok(Value::Byte(0x80))
    );
    let input = Err(RuntimeError::IoAddress(
        "the address lies outside the program's input area".into(),
    ));
    let output = Err(RuntimeError::IoAddress(
        "the address lies outside the program's output area".into(),
    ));
    assert_eq!(
        program.write_address(&address("%IB2"), Value::Byte(1)),
        input
    );
    assert_eq!(
        program.write_address(&address("%IW1"), Value::Word(1)),
        input
    );
    assert_eq!(
        program.write_address(&address("%QB1"), Value::Byte(1)),
        output
    );
    assert_eq!((program.inputs.len(), program.outputs.len()), (2, 1));
    assert_eq!(program.inputs, [0, 7]);
}
