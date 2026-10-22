//! The checkpoint of a compiled program's retained variables (specification 44 §5.10, §11;
//! specification 11 §6.7), over an image written by hand:
//!
//! ```text
//! VAR_GLOBAL RETAIN
//!     Count : UDINT := 5;            (* R 0 *)
//!     Ratio : REAL := 1.5;           (* R 4 *)
//!     Name : STRING[8] := 'ab';      (* R 8 *)
//!     Mode : Mode := On;             (* R 18, TYPE Mode : (Off, On, Fault := -1) INT *)
//!     Wide : WSTRING[4] := "x";      (* R 20 *)
//!     Level : INT(0..100) := 50;     (* R 30 *)
//! END_VAR
//! VAR_GLOBAL Plain : INT; END_VAR    (* G 0 *)
//! PROGRAM MainProg
//!     VAR Speed : INT; END_VAR       (* P 0 *)
//!     VAR RETAIN
//!         Total : DINT := -7;        (* R 32 *)
//!         Point : Point := (X := 3, Y := 4);   (* R 36, STRUCT X, Y : INT *)
//!         History : ARRAY[1..3] OF USINT := [7, 8, 9];   (* R 40 *)
//!         Timer : Ton;               (* R 48, a function block *)
//!     END_VAR
//! (* the instance Main of MainProg *)
//! ```

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_plc_abi::kernel::{Entry, WITHOUT_DEBUGGER};
use trust_plc_abi::module::run;
use trust_plc_abi::Cx;

use super::{check, payload_bound, stage};
use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::native::image::{
    type_kind, AreaRecord, CodeImage, EntryKind, EntryRecord, FileRecord, ImageWriter,
    IoBindRecord, MemberDetail, MemoryFigure, Record, RetainRecord, SectionId, StmtRecord,
    StringPool, SymbolRecord, Target, TaskRecord, TypeRecord, AREA_HAS_INIT, AREA_RETAINED, NONE,
    SYMBOL_INSTANCE, SYMBOL_PROGRAM, SYMBOL_RETAINED, TABLE_PREFIX_BYTES,
};
use crate::native::program::{admit, Device, ModuleCode, NativeProgram};
use crate::retain::capture::RetainCapture;
use crate::retain::codec::decode_snapshot_payload_v3;
use crate::retain::RetainSnapshot;
use crate::value::{ArrayValue, Duration, EnumValue, StructValue, Value};
use crate::vm::RetainApplyEvent;

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

/// The retain area's length.
const R: usize = 56;

/// The retain area's starting memory, as the declarations above give it.
fn starting() -> Vec<u8> {
    let mut bytes = vec![0u8; R];
    bytes[0..4].copy_from_slice(&5u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&1.5f32.to_le_bytes());
    bytes[8..12].copy_from_slice(&[2, 0, b'a', b'b']);
    bytes[18..20].copy_from_slice(&1i16.to_le_bytes());
    bytes[20..24].copy_from_slice(&[1, 0, b'x', 0]);
    bytes[30..32].copy_from_slice(&50i16.to_le_bytes());
    bytes[32..36].copy_from_slice(&(-7i32).to_le_bytes());
    bytes[36..40].copy_from_slice(&[3, 0, 4, 0]);
    bytes[40..43].copy_from_slice(&[7, 8, 9]);
    bytes
}

/// The type table of the declarations above and its detail list.
fn types(strings: &mut StringPool) -> (Vec<TypeRecord>, Vec<u8>) {
    let mut s = |text: &str| strings.add(text);
    let type_count = 14usize;
    let base = TABLE_PREFIX_BYTES + type_count * TypeRecord::BYTES;
    let mut details = Vec::new();
    let at = |details: &[u8]| u32::try_from(base + details.len()).unwrap();
    let elementary = |kind: u32, size: u32, name: u32| TypeRecord {
        kind,
        name,
        size,
        alignment: size,
        detail: 0,
        count: 0,
    };
    let compound = |kind: u32, name: u32, size: u32, detail: u32, count: u32| TypeRecord {
        kind,
        name,
        size,
        alignment: size.min(2),
        detail,
        count,
    };
    let mode_detail = at(&details);
    for (name, value) in [(s("Off"), 0i64), (s("On"), 1), (s("Fault"), -1)] {
        details.extend_from_slice(&name.to_le_bytes());
        details.extend_from_slice(&0u32.to_le_bytes());
        details.extend_from_slice(&value.to_le_bytes());
    }
    let point_detail = at(&details);
    for (name, offset) in [(s("X"), 0), (s("Y"), 2)] {
        let member = MemberDetail {
            name,
            type_index: 6,
            offset,
            flags: 0,
        };
        details.extend_from_slice(&member.bytes());
    }
    let array_detail = at(&details);
    details.extend_from_slice(&8u32.to_le_bytes());
    details.extend_from_slice(&0u32.to_le_bytes());
    details.extend_from_slice(&1i64.to_le_bytes());
    details.extend_from_slice(&3i64.to_le_bytes());
    let subrange_detail = at(&details);
    details.extend_from_slice(&type_kind::INT.to_le_bytes());
    details.extend_from_slice(&0u32.to_le_bytes());
    details.extend_from_slice(&0i64.to_le_bytes());
    details.extend_from_slice(&100i64.to_le_bytes());
    let ton_detail = at(&details);
    let q = MemberDetail {
        name: s("Q"),
        type_index: 12,
        offset: 0,
        flags: 0,
    };
    details.extend_from_slice(&q.bytes());
    let types = vec![
        elementary(type_kind::UDINT, 4, s("UDINT")),
        elementary(type_kind::REAL, 4, s("REAL")),
        compound(type_kind::STRING, s("STRING[8]"), 10, 0, 8),
        compound(type_kind::ENUM, s("Mode"), 2, mode_detail, 3),
        compound(type_kind::WSTRING, s("WSTRING[4]"), 10, 0, 4),
        elementary(type_kind::DINT, 4, s("DINT")),
        elementary(type_kind::INT, 2, s("INT")),
        compound(type_kind::STRUCT, s("Point"), 4, point_detail, 2),
        elementary(type_kind::USINT, 1, s("USINT")),
        compound(type_kind::ARRAY, 0, 3, array_detail, 1),
        compound(type_kind::SUBRANGE, s("INT(0..100)"), 2, subrange_detail, 0),
        compound(type_kind::FUNCTION_BLOCK, s("Ton"), 8, ton_detail, 1),
        elementary(type_kind::BOOL, 1, s("BOOL")),
        compound(type_kind::PROGRAM, s("MainProg"), 2, 0, 0),
    ];
    assert_eq!(types.len(), type_count);
    (types, details)
}

/// One symbol: canonical name, display name, type, area, flags, offset, size and parent.
type SymbolRow = (&'static str, &'static str, u32, u8, u8, u32, u32, u32);

/// The symbols of the declarations above, sorted by canonical name.
fn symbols(strings: &mut StringPool) -> Vec<SymbolRecord> {
    let retained = SYMBOL_RETAINED;
    let instance = SYMBOL_RETAINED | SYMBOL_INSTANCE;
    let rows: [SymbolRow; 16] = [
        ("COUNT", "Count", 0, 5, retained, 0, 4, NONE),
        ("LEVEL", "Level", 10, 5, retained, 30, 2, NONE),
        ("MAIN", "Main", 13, 6, SYMBOL_PROGRAM, 0, 2, NONE),
        ("MAIN.HISTORY", "Main.History", 9, 5, retained, 40, 3, 2),
        ("MAIN.POINT", "Main.Point", 7, 5, retained, 36, 4, 2),
        ("MAIN.POINT.X", "Main.Point.X", 6, 5, retained, 36, 2, 4),
        ("MAIN.POINT.Y", "Main.Point.Y", 6, 5, retained, 38, 2, 4),
        ("MAIN.SPEED", "Main.Speed", 6, 6, 0, 0, 2, 2),
        ("MAIN.TIMER", "Main.Timer", 11, 5, instance, 48, 8, 2),
        ("MAIN.TIMER.Q", "Main.Timer.Q", 12, 5, retained, 48, 1, 8),
        ("MAIN.TOTAL", "Main.Total", 5, 5, retained, 32, 4, 2),
        ("MODE", "Mode", 3, 5, retained, 18, 2, NONE),
        ("NAME", "Name", 2, 5, retained, 8, 10, NONE),
        ("PLAIN", "Plain", 6, 4, 0, 0, 2, NONE),
        ("RATIO", "Ratio", 1, 5, retained, 4, 4, NONE),
        ("WIDE", "Wide", 4, 5, retained, 20, 10, NONE),
    ];
    rows.iter()
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
                pou: NONE,
            },
        )
        .collect()
}

/// The image of the declarations above.
fn image() -> Vec<u8> {
    let mut strings = StringPool::new();
    let (types, details) = types(&mut strings);
    let symbols = symbols(&mut strings);
    // The retain map in declaration order: the globals, then Main's retained variables.
    let map: Vec<RetainRecord> = [0u32, 14, 12, 11, 15, 1, 10, 4, 3, 8]
        .iter()
        .map(|&index| RetainRecord {
            symbol: index,
            offset: symbols[index as usize].offset,
            size: symbols[index as usize].size,
        })
        .collect();
    let label = strings.add("main.st");
    let mut writer = ImageWriter::new(Target::this_cpu().unwrap_or(Target::X86_64), 1);
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
    let area = |id: u8, length: u64, flags: u32| AreaRecord {
        id: u32::from(id),
        length,
        alignment: 8,
        flags,
        init_offset: 0,
    };
    writer.table(
        &[
            area(1, 0, 0),
            area(2, 0, 0),
            area(3, 0, 0),
            area(4, 2, 0),
            area(5, R as u64, AREA_RETAINED | AREA_HAS_INIT),
            area(6, 2, 0),
        ],
        &[],
    );
    writer.section(SectionId::Init, starting());
    writer.table::<TaskRecord>(&[], &[]);
    writer.table::<IoBindRecord>(&[], &[]);
    writer.table(&symbols, &[]);
    writer.table(&types, &details);
    writer.table::<StmtRecord>(&[], &[]);
    writer.table(&map, &[]);
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
    }
}

fn int(value: i16) -> Value {
    Value::Int(value)
}

fn mode(variant: &str, value: i64) -> Value {
    Value::Enum(Box::new(EnumValue::from_canonical_parts(
        "Mode".into(),
        SmolStr::new(variant),
        value,
    )))
}

fn point(x: Value, y: Value) -> Value {
    let mut fields = OrderedMap::default();
    fields.insert(SmolStr::new("X"), x);
    fields.insert(SmolStr::new("Y"), y);
    Value::Struct(Arc::new(StructValue::from_canonical_parts(
        "Point".into(),
        fields,
    )))
}

/// The checkpoint of the starting values: each retained variable under its key, in declaration
/// order, the function block instance `Timer` left out (specification 11 §6.7).
fn expected() -> RetainSnapshot {
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("Count", Value::UDInt(5));
    snapshot.insert("Ratio", Value::Real(1.5));
    snapshot.insert("Name", Value::String("ab".into()));
    snapshot.insert("Mode", mode("On", 1));
    snapshot.insert("Wide", Value::WString(String::from("x")));
    snapshot.insert("Level", int(50));
    snapshot.insert("@program/Main/Total", Value::DInt(-7));
    snapshot.insert("@program/Main/Point", point(int(3), int(4)));
    snapshot.insert(
        "@program/Main/History",
        Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::USInt(7), Value::USInt(8), Value::USInt(9)],
            vec![(1, 3)],
        ))),
    );
    snapshot
}

/// A capture writes, for every retained variable, the bytes the shared codec writes for the same
/// value held as a `Value`, under the key of specification 11 §6.7, in the retain map's order, and
/// leaves out the function block instance. Catches a key that is the canonical name instead of the
/// declared spelling, a program variable without its `@program/` key, an instance written, or an
/// encoding that differs from the interpreter's.
#[test]
fn a_capture_writes_the_bytes_the_interpreter_writes() {
    let bytes = image();
    let device = device();
    let mut program = NativeProgram::new(
        admit(&bytes, &device).unwrap(),
        Idle,
        &device,
        &WITHOUT_DEBUGGER,
    )
    .unwrap();
    program.cold_start(Duration::ZERO).unwrap();
    let mut capture = RetainCapture::new();
    program.capture_retain(&mut capture).unwrap();
    let mut written = Vec::new();
    expected()
        .encode_snapshot_payload_into(&mut written)
        .unwrap();
    assert_eq!(capture.staged_payload(), written.as_slice());
    // The first entry, by hand: one count of 9 entries, the key `Count` and a `UDINT` 5 (tag 8).
    assert_eq!(&written[..4], &9u32.to_le_bytes());
    assert_eq!(
        &written[4..18],
        &[5, 0, 0, 0, b'C', b'o', b'u', b'n', b't', 8, 5, 0, 0, 0]
    );
}

/// A non-finite `REAL` is refused as the codec refuses it, and the capture keeps no payload, so no
/// checkpoint of it can be written (specification 11 §6.7). Catches a capture that writes a NaN.
#[test]
fn a_capture_refuses_a_non_finite_real() {
    let bytes = image();
    let device = device();
    let mut program = NativeProgram::new(
        admit(&bytes, &device).unwrap(),
        Idle,
        &device,
        &WITHOUT_DEBUGGER,
    )
    .unwrap();
    program.cold_start(Duration::ZERO).unwrap();
    assert!(program.write(5, 4, &f32::NAN.to_le_bytes()));
    let mut capture = RetainCapture::new();
    assert_eq!(
        program.capture_retain(&mut capture),
        Err(RuntimeError::RetainStore(
            "cannot persist non-finite REAL retain value".into()
        ))
    );
    assert!(capture.staged_payload().is_empty());
}

/// The largest values the types hold fit the bound a capture reserves, so a capture into the
/// reserved payload does not grow it (specification 34 MEM-09): a full `STRING[8]`, a `WSTRING[4]`
/// of four three-byte characters, the longest enumeration value name. Catches a bound that counts
/// a `WSTRING` in code units, or forgets the keys or the headers.
#[test]
fn the_largest_values_fit_the_reserved_payload() {
    let bytes = image();
    let device = device();
    let mut program = NativeProgram::new(
        admit(&bytes, &device).unwrap(),
        Idle,
        &device,
        &WITHOUT_DEBUGGER,
    )
    .unwrap();
    program.cold_start(Duration::ZERO).unwrap();
    assert!(program.write(
        5,
        8,
        &[8, 0, b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h']
    ));
    let wide: Vec<u8> = [4u16, 0x20AC, 0x20AC, 0x20AC, 0x20AC]
        .iter()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    assert!(program.write(5, 20, &wide));
    assert!(program.write(5, 18, &(-1i16).to_le_bytes()));
    let bound = program.retain_payload_bound().unwrap();
    let mut capture = RetainCapture::new();
    capture.reserve(bound).unwrap();
    program.capture_retain(&mut capture).unwrap();
    assert!(capture.staged_payload().len() <= bound);
    let decoded = decode_snapshot_payload_v3(capture.staged_payload()).unwrap();
    assert_eq!(
        decoded.values().get("Wide"),
        Some(&Value::WString(String::from("€€€€")))
    );
    assert_eq!(decoded.values().get("Mode"), Some(&mode("Fault", -1)));
}

/// A checkpoint restores what it captured, and nothing is reported: each value goes back to the
/// bytes it came from. Catches a restore that writes at another offset or another width.
#[test]
fn a_checkpoint_restores_what_it_captured() {
    let bytes = image();
    let parsed = admit(&bytes, &device()).unwrap();
    let mut capture = RetainCapture::new();
    let mut program = NativeProgram::new(parsed, Idle, &device(), &WITHOUT_DEBUGGER).unwrap();
    program.cold_start(Duration::ZERO).unwrap();
    program.capture_retain(&mut capture).unwrap();
    let snapshot = decode_snapshot_payload_v3(capture.staged_payload()).unwrap();
    let (staged, events) = stage(&parsed, &snapshot, &[0u8; R]).unwrap();
    assert_eq!(events, []);
    // Everything but the instance `Timer`, which no checkpoint holds, is the starting memory.
    assert_eq!(&staged[..48], &starting()[..48]);
}

/// A checkpoint applies by name without regard to case, with the interpreter's migrations
/// (specification 11 §6.7): an `UINT` widened to the `UDINT` `Count`, an `INT` widened to the `DINT`
/// `Total`, a text cut to its capacity, an enumeration rebuilt with its declared spelling, a
/// structure whose missing member `X` takes its starting value 3 and whose member `Z` is dropped;
/// each is reported with its value before and after. A key nothing declares is an orphan; a key
/// that names a variable without `RETAIN` leaves it alone and is not reported. A retained
/// variable the checkpoint lacks keeps what the area holds.
#[test]
fn a_checkpoint_migrates_by_the_interpreters_rules() {
    let bytes = image();
    let parsed = admit(&bytes, &device()).unwrap();
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("count", Value::UInt(9));
    snapshot.insert("@PROGRAM/main/total", Value::Int(-2));
    snapshot.insert("Name", Value::String("abcdefghij".into()));
    snapshot.insert(
        "Mode",
        Value::Enum(Box::new(EnumValue::from_canonical_parts(
            "MODE".into(),
            "FAULT".into(),
            -1,
        ))),
    );
    snapshot.insert("Gone", Value::DInt(1));
    snapshot.insert("Plain", int(3));
    snapshot.insert("@program/Main/Speed", int(1));
    let mut fields = OrderedMap::default();
    fields.insert(SmolStr::new("y"), int(9));
    fields.insert(SmolStr::new("Z"), int(1));
    let saved_point = Value::Struct(Arc::new(StructValue::from_canonical_parts(
        "Point".into(),
        fields,
    )));
    snapshot.insert("@program/Main/Point", saved_point.clone());
    let mut current = starting();
    current[30..32].copy_from_slice(&77i16.to_le_bytes());
    let (staged, events) = stage(&parsed, &snapshot, &current).unwrap();
    assert_eq!(&staged[0..4], &9u32.to_le_bytes());
    assert_eq!(&staged[32..36], &(-2i32).to_le_bytes());
    assert_eq!(
        &staged[8..18],
        &[8, 0, b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h']
    );
    assert_eq!(&staged[18..20], &(-1i16).to_le_bytes());
    assert_eq!(&staged[36..40], &[3, 0, 9, 0]);
    // `Level` is not in the checkpoint: it keeps 77.
    assert_eq!(&staged[30..32], &77i16.to_le_bytes());
    assert_eq!(
        events,
        [
            RetainApplyEvent::Migrated {
                name: "count".into(),
                before: Value::UInt(9),
                after: Value::UDInt(9),
            },
            RetainApplyEvent::Migrated {
                name: "@PROGRAM/main/total".into(),
                before: Value::Int(-2),
                after: Value::DInt(-2),
            },
            RetainApplyEvent::Migrated {
                name: "Name".into(),
                before: Value::String("abcdefghij".into()),
                after: Value::String("abcdefgh".into()),
            },
            RetainApplyEvent::Migrated {
                name: "Mode".into(),
                before: Value::Enum(Box::new(EnumValue::from_canonical_parts(
                    "MODE".into(),
                    "FAULT".into(),
                    -1,
                ))),
                after: mode("Fault", -1),
            },
            RetainApplyEvent::Orphan {
                name: "Gone".into()
            },
            RetainApplyEvent::Migrated {
                name: "@program/Main/Point".into(),
                before: saved_point,
                after: point(int(3), int(9)),
            },
        ]
    );
}

/// A value outside its subrange refuses the whole checkpoint with its numbers, a value of another
/// family (a `DINT` for the `UDINT` `Count`) with a migration error that names the key, and a
/// warm start that is refused changes no retained variable (specification 11 §6.7). Catches a
/// restore that clamps, converts across families, or applies the entries before the refusal.
#[test]
fn a_refused_value_changes_nothing() {
    let bytes = image();
    let parsed = admit(&bytes, &device()).unwrap();
    let mut outside = RetainSnapshot::default();
    outside.insert("Count", Value::UDInt(1));
    outside.insert("Level", int(101));
    assert_eq!(
        stage(&parsed, &outside, &starting()).err(),
        Some(RuntimeError::RetainSubrangeViolation {
            name: "Level".into(),
            value: 101,
            lower: 0,
            upper: 100,
        })
    );
    let mut signed = RetainSnapshot::default();
    signed.insert("Count", Value::DInt(1));
    assert!(matches!(
        check(&parsed, &signed),
        Err(RuntimeError::RetainMigration(text)) if text.starts_with("invalid retained value for 'Count'")
    ));
    let mut program = NativeProgram::new(parsed, Idle, &device(), &WITHOUT_DEBUGGER).unwrap();
    program.cold_start(Duration::ZERO).unwrap();
    assert!(program.warm_start_from(Duration::ZERO, &outside).is_err());
    let mut area = vec![0u8; R];
    assert!(program.read(5, 0, &mut area));
    assert_eq!(area, starting());
}

/// A warm restart keeps the retained values but the function block instance `Timer`, which takes
/// its starting value, since an instance is never retained (specification 10). Catches a warm
/// restart that keeps a retained instance's state, which the interpreter re-initializes.
#[test]
fn a_warm_restart_starts_a_retained_instance_again() {
    let bytes = image();
    let device = device();
    let mut program = NativeProgram::new(
        admit(&bytes, &device).unwrap(),
        Idle,
        &device,
        &WITHOUT_DEBUGGER,
    )
    .unwrap();
    program.cold_start(Duration::ZERO).unwrap();
    assert!(program.write(5, 0, &9u32.to_le_bytes()));
    assert!(program.write(5, 48, &[1, 1, 1, 1, 1, 1, 1, 1]));
    program.warm_start(Duration::ZERO).unwrap();
    let mut area = vec![0u8; R];
    assert!(program.read(5, 0, &mut area));
    assert_eq!(&area[0..4], &9u32.to_le_bytes());
    assert_eq!(&area[48..56], &[0u8; 8]);
}

/// The bound counts every retained variable but the instance, under its key, at its largest
/// encoding. Worked out by hand from the codec's layout.
#[test]
fn the_bound_is_the_largest_encoding_of_each_variable() {
    let bytes = image();
    let parsed = admit(&bytes, &device()).unwrap();
    // Each entry: 4 bytes of key length, the key, then the value.
    let entries = [
        4 + 5 + (1 + 4),                                // Count: UDINT
        4 + 5 + (1 + 4),                                // Ratio: REAL
        4 + 4 + (1 + 4 + 8),                            // Name: STRING[8]
        4 + 4 + (1 + 4 + 4 + 4 + 5 + 8), // Mode: type name, longest value name `Fault`
        4 + 4 + (1 + 4 + 3 * 4),         // Wide: WSTRING[4], three bytes per unit
        4 + 5 + (1 + 2),                 // Level: INT(0..100)
        4 + 19 + (1 + 4),                // @program/Main/Total: DINT
        4 + 19 + (1 + 4 + 5 + 4 + (4 + 1 + 3) * 2 + 1), // @program/Main/Point
        4 + 21 + (1 + 4 + 4 + 16 + 3 * 2), // @program/Main/History
    ];
    assert_eq!(
        payload_bound(&parsed),
        Ok(4 + entries.iter().sum::<usize>())
    );
}
