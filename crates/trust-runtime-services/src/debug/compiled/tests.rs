//! The debug reads of a compiled program (specification 44 §13.2, "The debugger of compiled
//! programs"), over an image written by hand and a memory that stands in for a held scan. The
//! program, in two files:
//!
//! ```text
//! (* lib/blocks.st *)
//! FUNCTION_BLOCK Counter                         (* POU 2 *)
//!     VAR_INPUT Step : INT; END_VAR              (* +0 *)
//!     VAR Count : DINT; END_VAR                  (* +4 *)
//!     Count := Count + Step;                     (* statement 3, 5:5 *)
//! END_FUNCTION_BLOCK
//! FUNCTION SumTo : DINT                          (* POU 1 *)
//!     VAR_INPUT N : INT; END_VAR                 (* frame +0 *)
//!     VAR Tally : Counter; END_VAR               (* frame +4 *)
//!     SumTo := N * 2;                            (* statement 2, 12:5 *)
//! END_FUNCTION
//!
//! (* main.st *)
//! TYPE Point : STRUCT X : INT; Y : INT; END_STRUCT; END_TYPE
//! VAR_GLOBAL
//!     Limit : INT := 7;                          (* G 0 *)
//!     Origin : Point := (X := 3, Y := -4);       (* G 2 *)
//!     Levels : ARRAY[0..2] OF INT := [10, 20, 30];   (* G 6 *)
//! END_VAR
//! VAR_GLOBAL RETAIN Total : DINT := 100; END_VAR     (* R 0 *)
//! PROGRAM Main                                   (* POU 0, its instance Plant at P 0 *)
//!     VAR
//!         Ctr : Counter;                         (* P 0 *)
//!         Speed : INT := 5;                      (* P 8 *)
//!         Ctrs : ARRAY[1..2] OF Counter;         (* P 12 and P 20 *)
//!     END_VAR
//!     VAR RETAIN Hits : DINT := 9; END_VAR       (* R 4 *)
//!     Ctr(Step := 2);                            (* statement 0, 14:5 *)
//!     Total := SumTo(N := 12);                   (* statement 1, 15:5 *)
//! END_PROGRAM
//! ```
//!
//! The scan is held with `Plant.Ctr` at `Step` 2 and `Count` 40, the elements of `Plant.Ctrs` at
//! 1 and 10 and at 3 and 30, and, in a call of `SumTo`, its `N` at 12 and its `Tally` at 6 and 60.
//! The frames of a held stack are numbered from the outermost, 1, so in a call of `SumTo` from
//! `Main` the frame of `SumTo` is frame 2.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use serde_json::{json, Value as Json};
use trust_runtime_core::debug::StatementSpan;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::{FrameId, InstanceId};
use trust_runtime_core::native::image::{
    type_kind, AreaRecord, CodeImage, EntryKind, EntryRecord, FileRecord, Image, ImageWriter,
    IoBindRecord, MemberDetail, MemoryFigure, Record, RetainRecord, SectionId, StmtRecord,
    StringPool, SymbolRecord, Target, TaskRecord, TypeRecord, NONE, SYMBOL_INPUT, SYMBOL_INSTANCE,
    SYMBOL_PROGRAM, SYMBOL_RETAINED, TABLE_PREFIX_BYTES,
};
use trust_runtime_core::native::memory::{Place, ProgramMemory, StopFrame};
use trust_runtime_core::value::Value;

use crate::debug::projection::{DebugVariableHandles, VariableHandle, CHIP_HANDLE_BOUND};
use crate::debug::reads::ReadState;
use crate::debug::{BreakpointView, Debugger, LineSources, Span, StepKind, StopReason, StopRecord};
use crate::live_values::io_requests::IoSnapshotRead;
use crate::live_values::io_snapshot::{IoSnapshot, IoSnapshotEntry, IoSnapshotValue};
use crate::protocol::ControlResponse;

/// The type `INT`.
const INT: u32 = 0;
/// The type `DINT`.
const DINT: u32 = 1;
/// The structure `Point`.
const POINT: u32 = 2;
/// The type of `Levels`, `ARRAY[0..2] OF INT`.
const LEVELS: u32 = 3;
/// The function block `Counter`.
const COUNTER: u32 = 4;
/// The type of `Ctrs`, `ARRAY[1..2] OF Counter`.
const COUNTERS: u32 = 5;
/// The program type `Main`.
const MAIN: u32 = 6;

/// Where `Plant` and its first variable `Ctr` lie: offset 0 of the instance area.
const PLANT: Place = Place::Area { area: 6, offset: 0 };

/// One symbol: canonical name, display name, type, area, flags, offset, size, parent and POU.
type SymbolRow = (&'static str, &'static str, u32, u8, u8, u32, u32, u32, u32);

/// One record of the retain map: key, area, offset and type.
type RetainRow = (&'static str, u32, u32, u32);

/// The symbols of the program, sorted by canonical name, as a debug image has them. The POU symbols
/// have area 0 and the POU's id, the frame variables area 7 and their offsets in the frame, and
/// the program root the instance's number.
#[rustfmt::skip]
const SYMBOLS: [SymbolRow; 20] = [
    // 0 to 6: the POU symbols, then the frame variables of `SumTo`.
    ("@COUNTER",           "Counter",           COUNTER,  0, 0,               0,  0,  NONE, 2),
    ("@MAIN",              "Main",              MAIN,     0, 0,               0,  0,  NONE, 0),
    ("@SUMTO",             "SumTo",             NONE,     0, 0,               0,  12, NONE, 1),
    ("@SUMTO.N",           "SumTo.N",           INT,      7, 0,               0,  2,  2,    1),
    ("@SUMTO.TALLY",       "SumTo.Tally",       COUNTER,  7, SYMBOL_INSTANCE, 4,  8,  2,    1),
    ("@SUMTO.TALLY.COUNT", "SumTo.Tally.Count", DINT,     7, 0,               8,  4,  4,    1),
    ("@SUMTO.TALLY.STEP",  "SumTo.Tally.Step",  INT,      7, SYMBOL_INPUT,    4,  2,  4,    1),
    // 7 to 11: the globals `Levels`, `Limit` and `Origin`, then the members of `Origin`.
    ("LEVELS",             "Levels",            LEVELS,   4, 0,               6,  6,  NONE, NONE),
    ("LIMIT",              "Limit",             INT,      4, 0,               0,  2,  NONE, NONE),
    ("ORIGIN",             "Origin",            POINT,    4, 0,               2,  4,  NONE, NONE),
    ("ORIGIN.X",           "Origin.X",          INT,      4, 0,               2,  2,  9,    NONE),
    ("ORIGIN.Y",           "Origin.Y",          INT,      4, 0,               4,  2,  9,    NONE),
    // 12 to 18: the program root `Plant` and its variables, `Ctr` (13) with its members.
    ("PLANT",              "Plant",             MAIN,     6, SYMBOL_PROGRAM,  0,  28, NONE, 0),
    ("PLANT.CTR",          "Plant.Ctr",         COUNTER,  6, SYMBOL_INSTANCE, 0,  8,  12,   NONE),
    ("PLANT.CTR.COUNT",    "Plant.Ctr.Count",   DINT,     6, 0,               4,  4,  13,   NONE),
    ("PLANT.CTR.STEP",     "Plant.Ctr.Step",    INT,      6, SYMBOL_INPUT,    0,  2,  13,   NONE),
    ("PLANT.CTRS",         "Plant.Ctrs",        COUNTERS, 6, 0,               12, 16, 12,   NONE),
    ("PLANT.HITS",         "Plant.Hits",        DINT,     5, SYMBOL_RETAINED, 4,  4,  12,   NONE),
    ("PLANT.SPEED",        "Plant.Speed",       INT,      6, 0,               8,  2,  12,   NONE),
    // 19: the retained global `Total`.
    ("TOTAL",              "Total",             DINT,     5, SYMBOL_RETAINED, 0,  4,  NONE, NONE),
];

/// The symbols of a program of the function `SumTo` alone, whose only instance, `Tally`, lies in
/// its frame.
#[rustfmt::skip]
const BARE: [SymbolRow; 3] = [
    ("@SUMTO",       "SumTo",       NONE,    0, 0,               0, 12, NONE, 1),
    ("@SUMTO.N",     "SumTo.N",     INT,     7, 0,               0, 2,  0,    1),
    ("@SUMTO.TALLY", "SumTo.Tally", COUNTER, 7, SYMBOL_INSTANCE, 4, 8,  0,    1),
];

/// The retain map in the order of specification 11 §6.7: the globals, then the program's
/// variables, each under its checkpoint key.
const RETAINED: [RetainRow; 2] = [("Total", 5, 0, DINT), ("@program/Plant/Hits", 5, 4, DINT)];

/// The detail of the members `(name, type, offset)` of a structure, a function block or a program
/// (specification 44 §5.8).
fn members(strings: &mut StringPool, rows: &[(&str, u32, u32)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for &(name, type_index, offset) in rows {
        let member = MemberDetail {
            name: strings.add(name),
            type_index,
            offset,
            flags: 0,
        };
        bytes.extend_from_slice(&member.bytes());
    }
    bytes
}

/// The detail of a one-dimensional array of `element` from `lower` to `upper`.
fn dimension(element: u32, lower: i64, upper: i64) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&element.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&lower.to_le_bytes());
    bytes.extend_from_slice(&upper.to_le_bytes());
    bytes
}

/// The type table, at the indexes of the constants above, and its detail list.
fn types(strings: &mut StringPool) -> (Vec<TypeRecord>, Vec<u8>) {
    let details = [
        members(strings, &[("X", INT, 0), ("Y", INT, 2)]),
        dimension(INT, 0, 2),
        members(strings, &[("Step", INT, 0), ("Count", DINT, 4)]),
        dimension(COUNTER, 1, 2),
        members(
            strings,
            &[
                ("Ctr", COUNTER, 0),
                ("Speed", INT, 8),
                ("Ctrs", COUNTERS, 12),
            ],
        ),
    ];
    // Each detail lies after the seven type records, at its offset from the start of TYPES.
    let mut offsets = Vec::new();
    let mut at = TABLE_PREFIX_BYTES + 7 * TypeRecord::BYTES;
    for detail in &details {
        offsets.push(u32::try_from(at).expect("a small table"));
        at += detail.len();
    }
    let ty = |kind, name, size, alignment, detail, count| TypeRecord {
        kind,
        name,
        size,
        alignment,
        detail,
        count,
    };
    let types = vec![
        ty(type_kind::INT, strings.add("INT"), 2, 2, 0, 0),
        ty(type_kind::DINT, strings.add("DINT"), 4, 4, 0, 0),
        ty(type_kind::STRUCT, strings.add("Point"), 4, 2, offsets[0], 2),
        ty(type_kind::ARRAY, 0, 6, 2, offsets[1], 1),
        ty(
            type_kind::FUNCTION_BLOCK,
            strings.add("Counter"),
            8,
            4,
            offsets[2],
            2,
        ),
        ty(type_kind::ARRAY, 0, 16, 4, offsets[3], 1),
        ty(
            type_kind::PROGRAM,
            strings.add("Main"),
            28,
            4,
            offsets[4],
            3,
        ),
    ];
    (types, details.concat())
}

/// The statement table: the two statements of `Main` in `main.st`, then the one of `SumTo` and
/// the one of `Counter` in `lib/blocks.st`, with their 1-based starts and ends.
fn statements() -> [StmtRecord; 4] {
    let statement = |file, pou, start_line, start_column, end_line, end_column| StmtRecord {
        file,
        pou,
        start_line,
        start_column,
        end_line,
        end_column,
    };
    [
        statement(0, 0, 14, 5, 14, 20),
        statement(0, 0, 15, 5, 15, 29),
        statement(1, 1, 12, 5, 12, 20),
        statement(1, 2, 5, 5, 5, 27),
    ]
}

/// The image of the program with the symbols `symbols` and the retain map `retained`.
fn image(symbols: &[SymbolRow], retained: &[RetainRow]) -> Vec<u8> {
    let mut strings = StringPool::new();
    let (types, details) = types(&mut strings);
    let symbols: Vec<SymbolRecord> = symbols
        .iter()
        .map(
            |&(canonical, display, type_index, area, flags, offset, size, parent, pou)| {
                SymbolRecord {
                    canonical: strings.add(canonical),
                    display: strings.add(display),
                    type_index,
                    area,
                    flags,
                    offset,
                    size,
                    parent,
                    pou,
                }
            },
        )
        .collect();
    let retain: Vec<RetainRecord> = retained
        .iter()
        .map(|&(key, area, offset, type_index)| RetainRecord {
            key: strings.add(key),
            area,
            offset,
            type_index,
        })
        .collect();
    let files = ["main.st", "lib/blocks.st"].map(|label| FileRecord {
        label: strings.add(label),
        source_offset: NONE,
        source_length: 0,
    });
    let mut writer = ImageWriter::new(
        Target::this_cpu().unwrap_or(Target::X86_64),
        u16::try_from(trust_plc_abi::INTERFACE_VERSION).expect("a 16-bit interface version"),
    );
    writer.module(CodeImage {
        code: vec![0; 64],
        rodata: Vec::new(),
        rodata_distance: 64,
    });
    writer.table(
        &[EntryRecord {
            kind: EntryKind::ProgramBody as u32,
            index: 0,
            code_offset: 0,
        }],
        &[],
    );
    let area = |id, length| AreaRecord {
        id,
        length,
        alignment: 8,
        flags: 0,
        init_offset: 0,
    };
    writer.table(
        &[
            area(1, 1),
            area(2, 1),
            area(3, 0),
            area(4, 12),
            area(5, 8),
            area(6, 28),
            area(7, 64),
        ],
        &[],
    );
    writer.section(SectionId::Init, Vec::new());
    writer.table::<TaskRecord>(&[], &[]);
    writer.table::<IoBindRecord>(&[], &[]);
    writer.table(&symbols, &[]);
    writer.table(&types, &details);
    writer.table(&statements(), &[]);
    writer.table(&retain, &[]);
    writer.table(&files, &[]);
    writer.section(SectionId::MemFig, MemoryFigure::default().bytes().to_vec());
    writer.section(SectionId::Strings, strings.into_bytes());
    writer.finish().expect("the image builds")
}

/// Writes `value` at `offset` of `bytes`.
fn put(bytes: &mut [u8], offset: usize, value: &[u8]) {
    bytes[offset..offset + value.len()].copy_from_slice(value);
}

/// The areas as the held scan has them, by their ids (specification 44 §7.1): the globals `G`, the
/// retained variables `R` and the instances `P` of the program; the others are empty.
fn areas() -> Vec<Vec<u8>> {
    let mut globals = vec![0u8; 12];
    put(&mut globals, 0, &7i16.to_le_bytes());
    put(&mut globals, 2, &3i16.to_le_bytes());
    put(&mut globals, 4, &(-4i16).to_le_bytes());
    put(&mut globals, 6, &10i16.to_le_bytes());
    put(&mut globals, 8, &20i16.to_le_bytes());
    put(&mut globals, 10, &30i16.to_le_bytes());
    let mut retain = vec![0u8; 8];
    put(&mut retain, 0, &100i32.to_le_bytes());
    put(&mut retain, 4, &9i32.to_le_bytes());
    let mut instances = vec![0u8; 28];
    put(&mut instances, 0, &2i16.to_le_bytes());
    put(&mut instances, 4, &40i32.to_le_bytes());
    put(&mut instances, 8, &5i16.to_le_bytes());
    put(&mut instances, 12, &1i16.to_le_bytes());
    put(&mut instances, 16, &10i32.to_le_bytes());
    put(&mut instances, 20, &3i16.to_le_bytes());
    put(&mut instances, 24, &30i32.to_le_bytes());
    vec![
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        globals,
        retain,
        instances,
        Vec::new(),
    ]
}

/// The frame of the function `SumTo` at statement `stmt`, with its variables: `N` 12 at 0 and
/// `Tally` at 4, whose `Step` is 6 and `Count` 60.
fn sum_to(stmt: u32) -> (StopFrame, Vec<u8>) {
    let mut variables = vec![0u8; 12];
    put(&mut variables, 0, &12i16.to_le_bytes());
    put(&mut variables, 4, &6i16.to_le_bytes());
    put(&mut variables, 8, &60i32.to_le_bytes());
    let frame = StopFrame {
        pou: 1,
        stmt,
        instance: None,
        locals: true,
    };
    (frame, variables)
}

/// The frame of the body of `Main` on the instance `Plant`, at statement `stmt`.
fn main_body(stmt: u32) -> (StopFrame, Vec<u8>) {
    let frame = StopFrame {
        pou: 0,
        stmt,
        instance: Some(PLANT),
        locals: false,
    };
    (frame, Vec::new())
}

/// The frame of the body of `Counter` on the instance `Plant.Ctr`, which lies where `Plant` does,
/// at statement `stmt`.
fn counter_body(stmt: u32) -> (StopFrame, Vec<u8>) {
    let frame = StopFrame {
        pou: 2,
        stmt,
        instance: Some(PLANT),
        locals: false,
    };
    (frame, Vec::new())
}

/// A compiled program's memory as a held scan lends it: the image, each area's bytes by its id,
/// the frames of the call stack, innermost first, and each frame's variables.
struct Memory {
    image: Vec<u8>,
    areas: Vec<Vec<u8>>,
    frames: Vec<StopFrame>,
    variables: Vec<Vec<u8>>,
}

/// Copies the bytes at `offset` of `bytes` into `out`; `false` past their end.
fn copy(bytes: Option<&[u8]>, offset: usize, out: &mut [u8]) -> bool {
    let end = offset.checked_add(out.len());
    let Some(source) = bytes
        .zip(end)
        .and_then(|(bytes, end)| bytes.get(offset..end))
    else {
        return false;
    };
    out.copy_from_slice(source);
    true
}

impl ProgramMemory for Memory {
    fn image(&self) -> Image<'_> {
        Image::parse(&self.image).expect("the fixture's image parses")
    }

    fn read_area(&self, id: u8, offset: usize, out: &mut [u8]) -> bool {
        copy(
            self.areas.get(usize::from(id)).map(Vec::as_slice),
            offset,
            out,
        )
    }

    fn write_area(&mut self, _id: u8, _offset: usize, _data: &[u8]) -> bool {
        false
    }

    fn frames(&self) -> Vec<StopFrame> {
        self.frames.clone()
    }

    fn read_frame(&self, frame: usize, offset: usize, out: &mut [u8]) -> bool {
        copy(self.variables.get(frame).map(Vec::as_slice), offset, out)
    }

    fn write_image(&mut self, _address: &IoAddress, _value: Value) -> Result<(), RuntimeError> {
        Err(RuntimeError::TypeMismatch)
    }
}

/// A debugger that holds the fixture's program, with the handle table of a chip: the dispatcher's
/// side of a device loop that holds a compiled program's scan.
struct Held {
    memory: Memory,
    sources: LineSources,
    handles: RefCell<DebugVariableHandles>,
    io: IoSnapshotRead,
    last_stop: Option<StopRecord>,
    /// The breakpoints each `breakpoints.set` replaced a file's with, by the file.
    replaced: RefCell<Vec<Replaced>>,
}

/// The breakpoints one `breakpoints.set` replaced a file's with: the file, and each breakpoint's
/// span with its options.
type Replaced = (
    u32,
    Vec<(Span<(u32, u32)>, crate::debug::BreakpointOptions)>,
);

impl Held {
    /// The program with `symbols` and `retained`, held with `frames`, innermost first, each with
    /// its variables; no I/O snapshot is published and no stop is recorded.
    fn of(
        symbols: &[SymbolRow],
        retained: &[RetainRow],
        frames: Vec<(StopFrame, Vec<u8>)>,
    ) -> Self {
        let bytes = image(symbols, retained);
        let sources = LineSources::from_image(&Image::parse(&bytes).expect("the image parses"));
        let (frames, variables): (Vec<StopFrame>, Vec<Vec<u8>>) = frames.into_iter().unzip();
        Self {
            memory: Memory {
                image: bytes,
                areas: areas(),
                frames,
                variables,
            },
            sources,
            handles: RefCell::new(DebugVariableHandles::with_bound(CHIP_HANDLE_BOUND)),
            io: IoSnapshotRead::Missing,
            last_stop: None,
            replaced: RefCell::new(Vec::new()),
        }
    }

    /// The program held with `frames`.
    fn new(frames: Vec<(StopFrame, Vec<u8>)>) -> Self {
        Self::of(&SYMBOLS, &RETAINED, frames)
    }

    /// `debug.stack`.
    fn stack(&self) -> ControlResponse {
        super::stack(1, &self.memory, self, &self.sources)
    }

    /// `debug.scopes` of frame `frame_id`.
    fn scopes(&self, frame_id: u32) -> ControlResponse {
        let params = json!({ "frame_id": frame_id });
        super::scopes(1, Some(&params), &self.memory, self, &self.sources)
    }

    /// `debug.variables` with `params`.
    fn variables(&self, params: &Json) -> ControlResponse {
        super::variables(1, Some(params), &self.memory, self)
    }

    /// The variables that `debug.variables` lists for handle `reference`.
    fn listed(&self, reference: u32) -> Json {
        let params = json!({ "variables_reference": reference });
        result(self.variables(&params))["variables"].clone()
    }

    /// Lists the scopes of frame `frame_id`, which hands out the scope handles from 1.
    fn open_scopes(&self, frame_id: u32) {
        let _ = result(self.scopes(frame_id));
    }

    /// How many variable handles are live.
    fn live_handles(&self) -> usize {
        self.handles.borrow().len()
    }
}

impl Debugger for Held {
    fn sources(&self) -> Option<&LineSources> {
        Some(&self.sources)
    }

    fn read_state(&self) -> Result<ReadState<'_>, String> {
        Err(String::from(
            "a compiled program is read through its memory",
        ))
    }

    fn io_snapshot(&self) -> IoSnapshotRead {
        self.io.clone()
    }

    fn is_held(&self) -> bool {
        !self.memory.frames.is_empty()
    }

    fn accepts_step(&self) -> bool {
        false
    }

    fn step(&self, _kind: StepKind) {}

    fn last_stop(&self) -> Option<StopRecord> {
        self.last_stop
    }

    fn drain_stops(&self) -> Vec<StopRecord> {
        Vec::new()
    }

    fn frame_span(&self, _frame: FrameId) -> Option<StatementSpan> {
        None
    }

    fn breakpoints(&self) -> Vec<BreakpointView> {
        Vec::new()
    }

    fn replace_breakpoints(
        &self,
        file_id: u32,
        breakpoints: Vec<(Span<(u32, u32)>, crate::debug::BreakpointOptions)>,
    ) -> u64 {
        let mut replaced = self.replaced.borrow_mut();
        replaced.push((file_id, breakpoints));
        u64::try_from(replaced.len()).unwrap_or(u64::MAX)
    }

    fn clear_breakpoints(&self) {}

    fn with_handles(&self, operation: &mut dyn FnMut(&mut DebugVariableHandles)) {
        operation(&mut self.handles.borrow_mut());
    }

    fn program_memory(&self) -> Option<&dyn ProgramMemory> {
        Some(&self.memory)
    }
}

/// The result of a reply that succeeded.
fn result(response: ControlResponse) -> Json {
    assert!(response.ok, "{:?}", response.error);
    response.result.expect("a result")
}

/// The error text of a reply that failed.
fn error(response: &ControlResponse) -> &str {
    assert!(!response.ok, "{:?}", response.result);
    response.error.as_deref().expect("an error text")
}

/// A breakpoint stop at the statement of file `file_index` from `start` to `end`, one-based.
fn stop_at(file_index: u32, start: (u32, u32), end: (u32, u32)) -> StopRecord {
    StopRecord {
        reason: StopReason::Breakpoint,
        statement: StatementSpan {
            file_index,
            start,
            end,
        },
        thread: Some(1),
        breakpoint_generation: Some(1),
    }
}

/// A published I/O snapshot with one input, `start` at `%IX0.0`, TRUE.
fn one_input() -> IoSnapshotRead {
    IoSnapshotRead::Published(IoSnapshot {
        scan: Some(1),
        inputs: vec![IoSnapshotEntry {
            name: Some("start".into()),
            address: IoAddress::parse("%IX0.0").expect("an input address"),
            value_type_name: Some("BOOL".into()),
            value: IoSnapshotValue::Value(Value::Bool(true)),
            source: None,
        }],
        ..IoSnapshot::default()
    })
}

/// The source `main.st` as a frame or a scope names it.
fn main_st() -> Json {
    json!({"name": "main.st", "path": "main.st"})
}

/// The source `lib/blocks.st` as a frame or a scope names it.
fn blocks_st() -> Json {
    json!({"name": "blocks.st", "path": "lib/blocks.st"})
}

/// A scope without a position.
fn scope(name: &str, reference: u32) -> Json {
    json!({"name": name, "variablesReference": reference, "expensive": false})
}

/// The entry of a value without members, evaluated by its own name.
fn scalar(name: &str, value: &str, type_name: &str) -> Json {
    json!({
        "name": name,
        "value": value,
        "type": type_name,
        "variablesReference": 0,
        "evaluateName": name
    })
}

/// The entry of an instance of `type_name` with `members` members, holding handle `reference`.
fn instance_entry(name: &str, type_name: &str, reference: u32, members: u32) -> Json {
    json!({
        "name": name,
        "value": type_name,
        "type": type_name,
        "variablesReference": reference,
        "evaluateName": name,
        "namedVariables": members
    })
}

/// The entry of an array of `elements` elements, holding handle `reference`, as the interpreter
/// shows an array; an array of instances is shown so too.
fn array_entry(name: &str, reference: u32, elements: u32) -> Json {
    json!({
        "name": name,
        "value": format!("[{elements}]"),
        "type": "ARRAY",
        "variablesReference": reference,
        "evaluateName": name,
        "indexedVariables": elements
    })
}

#[test]
fn the_stack_lists_the_held_frames_innermost_first_at_their_statements() {
    // Specification 44 §13.2: while the scan is held, `debug.stack` lists the frames innermost
    // first, each named by its POU symbol and placed at the statement it stands at, the held one
    // for the innermost and the call for each caller; of n frames the one at index i is frame
    // n - i. Worked: held in `SumTo` at statement 2 (lib/blocks.st 12:5), called from `Main` at
    // statement 1 (main.st 15:5): `SumTo` is frame 2 at (11, 4) of `lib/blocks.st` and `Main`
    // frame 1 at (14, 4) of `main.st`, zero-based.
    // Wrong behaviour caught: an outermost-first order, frames numbered from 0 or from the
    // innermost, a frame named by its symbol's canonical name (`@SUMTO`) or by its instance
    // (`Plant`), a position in another frame's file, one-based positions, and every frame placed at
    // the held statement.
    let held = Held::new(vec![sum_to(2), main_body(1)]);

    assert_eq!(
        result(held.stack()),
        json!({
            "stack_frames": [
                {"id": 2, "name": "SumTo", "source": blocks_st(), "line": 11, "column": 4},
                {"id": 1, "name": "Main", "source": main_st(), "line": 14, "column": 4},
            ],
            "total_frames": 2
        })
    );
}

#[test]
fn a_frame_without_a_position_stands_at_the_last_stop_or_is_left_out() {
    // Specification 44 §13.2 with the interpreter's stack (specification 13, "Stack Trace and
    // Navigation"): a frame whose statement has no position stands at the latest stop, and without
    // a stop it is left out. Worked: `Main` at statement 99, which the statement table does not
    // hold, with the latest stop at lib/blocks.st 5:5: `Main` is frame 1 at (4, 4) of
    // `lib/blocks.st`, and `SumTo` keeps its own statement, frame 2 at (11, 4). Without the stop
    // only `SumTo` is listed, still as frame 2.
    // Wrong behaviour caught: a frame without a position placed at another frame's statement, a
    // frame listed without a source, a stack that fails, a frame with a position of its own moved
    // to the stop, and frames renumbered after one is left out.
    let mut held = Held::new(vec![sum_to(2), main_body(99)]);
    held.last_stop = Some(stop_at(1, (5, 5), (5, 27)));
    let sum_to_frame =
        json!({"id": 2, "name": "SumTo", "source": blocks_st(), "line": 11, "column": 4});

    assert_eq!(
        result(held.stack()),
        json!({
            "stack_frames": [
                sum_to_frame.clone(),
                {"id": 1, "name": "Main", "source": blocks_st(), "line": 4, "column": 4},
            ],
            "total_frames": 2
        })
    );
    held.last_stop = None;
    assert_eq!(
        result(held.stack()),
        json!({"stack_frames": [sum_to_frame], "total_frames": 1})
    );
}

#[test]
fn between_scans_the_stack_is_the_one_main_frame_at_the_last_stop() {
    // Specification 44 §13.2: between scans the reads answer without a call stack. The stack is
    // then the interpreter's (specification 11 §6.13.1): the one synthetic frame `Main`, numbered
    // 0, at the latest stop, and none without a stop. Worked: the latest stop at lib/blocks.st
    // 12:5 gives frame 0 at (11, 4) of `lib/blocks.st`.
    // Wrong behaviour caught: the frame numbered 1, the frame named by the stop's POU (`SumTo`), a
    // stack that fails between scans, and a frame invented without a stop.
    let mut held = Held::new(Vec::new());
    held.last_stop = Some(stop_at(1, (12, 5), (12, 20)));

    assert_eq!(
        result(held.stack()),
        json!({
            "stack_frames": [
                {"id": 0, "name": "Main", "source": blocks_st(), "line": 11, "column": 4},
            ],
            "total_frames": 1
        })
    );
    held.last_stop = None;
    assert_eq!(
        result(held.stack()),
        json!({"stack_frames": [], "total_frames": 0})
    );
}

#[test]
fn the_scopes_of_a_frame_are_its_locals_at_its_statement_then_what_the_program_has() {
    // Specification 44 §13.2 and specification 13, "Variable Visibility": a frame's scopes are its
    // `Locals`, at the frame's statement, then `Globals`, `Retain`, `I/O` and `Instances`, each
    // when the program has something to show, with handles that restart at 1 at every listing.
    // Worked: the program has globals, two retained variables, one published input and two
    // instances in its areas. Frame 2 (`SumTo`) is at (11, 4) of `lib/blocks.st`, and its locals
    // begin with `N`; frame 1 (`Main`) is at (14, 4) of `main.st`, and its locals, handle 1 again,
    // begin with `Ctr`.
    // Wrong behaviour caught: a scope missing or out of order, handles that go on from the earlier
    // listing, a frame's scope at another frame's statement, and the locals of another frame (frame
    // 1 counted from the innermost).
    let mut held = Held::new(vec![sum_to(2), main_body(1)]);
    held.io = one_input();
    let others = [
        scope("Globals", 2),
        scope("Retain", 3),
        scope("I/O", 4),
        scope("Instances", 5),
    ];
    let locals = |source: Json, line: u32| {
        json!({
            "name": "Locals",
            "variablesReference": 1,
            "expensive": false,
            "source": source,
            "line": line,
            "column": 4
        })
    };

    let mut innermost = vec![locals(blocks_st(), 11)];
    innermost.extend(others.iter().cloned());
    assert_eq!(result(held.scopes(2)), json!({ "scopes": innermost }));
    assert_eq!(held.listed(1)[0]["name"], json!("N"));

    let mut outermost = vec![locals(main_st(), 14)];
    outermost.extend(others.iter().cloned());
    assert_eq!(result(held.scopes(1)), json!({ "scopes": outermost }));
    assert_eq!(held.listed(1)[0]["name"], json!("Ctr"));
}

#[test]
fn a_program_with_nothing_else_to_show_has_only_the_locals() {
    // Specification 44 §13.2, with the interpreter's scopes: `Globals` is listed only when the
    // program has a global, `Retain` only when the retain map has a record, `I/O` only when a
    // snapshot with entries is published, and `Instances` only when an instance lies in an area.
    // Worked: a program of the function `SumTo` alone, whose only instance, `Tally`, lies in its
    // frame, with an empty retain map and no I/O snapshot.
    // Wrong behaviour caught: a scope listed whatever it holds, a POU symbol or a frame variable
    // counted as a global, and an instance in a frame counted as one of the program's instances.
    let held = Held::of(&BARE, &[], vec![sum_to(2)]);

    assert_eq!(
        result(held.scopes(1)),
        json!({"scopes": [{
            "name": "Locals",
            "variablesReference": 1,
            "expensive": false,
            "source": blocks_st(),
            "line": 11,
            "column": 4
        }]})
    );
}

#[test]
fn scopes_refuse_a_frame_the_stack_does_not_have_and_an_unreadable_snapshot() {
    // Specification 11 §6.9.5 and §6.13.3, as the interpreter answers: a frame identifier that
    // names no frame of the held stack fails with `unknown frame id <n>`, between scans too, when
    // there is none; an I/O snapshot that cannot be read fails with `I/O snapshot unavailable`.
    // Worked: the two held frames are 1 and 2, so 0 and 3 name none; between scans 1 names none.
    // Wrong behaviour caught: scopes of a frame that is not there (an empty list, or the innermost
    // frame's), frame identifiers counted from 0, and scopes listed without the I/O they cannot
    // read.
    let mut held = Held::new(vec![sum_to(2), main_body(1)]);
    assert_eq!(error(&held.scopes(0)), "unknown frame id 0");
    assert_eq!(error(&held.scopes(3)), "unknown frame id 3");
    held.io = IoSnapshotRead::Unavailable;
    assert_eq!(error(&held.scopes(2)), "I/O snapshot unavailable");

    let between = Held::new(Vec::new());
    assert_eq!(error(&between.scopes(1)), "unknown frame id 1");
}

#[test]
fn scopes_that_the_handle_bound_refuses_leave_no_handle() {
    // Specification 11 §6.13.3, as the interpreter's scopes: scopes that would take the handle
    // table past its bound are refused with `variable_handles_full`, and the table is cleared.
    // Worked: the frame of `Main` has four scopes without I/O, and a table of three holds three.
    // Wrong behaviour caught: scopes cut to what fits, and a refusal that leaves handles live.
    let held = Held::new(vec![main_body(1)]);
    *held.handles.borrow_mut() = DebugVariableHandles::with_bound(3);

    let refused = held.scopes(1);

    assert_eq!(refused.error_code.as_deref(), Some("variable_handles_full"));
    assert_eq!(
        error(&refused),
        "the variable handle table is full: request the scopes again"
    );
    assert_eq!(held.live_handles(), 0);
}

#[test]
fn a_listing_keeps_its_handle_and_a_refused_one_is_rolled_back() {
    // Specification 13, "Runtime debug state, mutation, and projection invariants", and
    // specification 11 §6.13.3, as the interpreter lists: a handle stays valid after its listing,
    // each listing hands out new handles for its members, an unknown handle fails with `unknown
    // variables reference <n>`, and a listing that would take the table past its bound is refused
    // with `variable_handles_full` and rolled back. Worked: the scopes of `Main` take 1 to 4; its
    // locals give `Ctr` 5 and `Ctrs` 6, listed again 7 and 8, and 5 still lists `Step` and
    // `Count`. With a bound of 5, the locals need 5 and 6 beside the four scopes, the listed one
    // counted while it is out, so they are refused; a page of `Ctr` alone then takes 5 again.
    // Wrong behaviour caught: a handle its own listing consumes, member handles that alias live
    // ones, a refused listing that keeps the handle it took (`Ctr` would take 6), and a listing cut
    // to what fits instead of refused.
    let held = Held::new(vec![main_body(1)]);
    held.open_scopes(1);
    assert_eq!(
        error(&held.variables(&json!({"variables_reference": 42}))),
        "unknown variables reference 42"
    );
    assert_eq!(held.listed(1)[0]["variablesReference"], json!(5));
    assert_eq!(held.listed(1)[1]["variablesReference"], json!(8));
    assert_eq!(
        held.listed(5),
        json!([scalar("Step", "2", "INT"), scalar("Count", "40", "DINT")])
    );

    let bounded = Held::new(vec![main_body(1)]);
    *bounded.handles.borrow_mut() = DebugVariableHandles::with_bound(5);
    bounded.open_scopes(1);
    let refused = bounded.variables(&json!({"variables_reference": 1}));
    assert_eq!(refused.error_code.as_deref(), Some("variable_handles_full"));
    assert_eq!(bounded.live_handles(), 4);
    let first = result(bounded.variables(&json!({"variables_reference": 1, "count": 1})));
    assert_eq!(
        first["variables"],
        json!([instance_entry("Ctr", "Counter", 5, 2)])
    );
}

#[test]
fn a_listing_pages_named_children_and_the_elements_of_an_array_of_instances() {
    // Specification 11 §6.13.3, "Variable pages": the variables a listing names are paged as named
    // children and the elements of an array of instances as indexed ones, by `filter`, `start` and
    // `count`. Worked: the locals of `Main` are `Ctr`, `Ctrs`, `Hits` and `Speed`; `start` 1 with
    // `count` 2 lists `Ctrs` (handle 5, as the skipped `Ctr` takes none) and `Hits`, and the filter
    // `indexed` none of them. The elements of `Ctrs` are `[0]` and `[1]`: `start` 1 lists `[1]`
    // (handle 6), and the filter `named` none.
    // Wrong behaviour caught: named children paged as indexed ones or the reverse, a start off by
    // one, a count that is ignored, a skipped child that still takes a handle, and an element named
    // by its place in the page.
    let held = Held::new(vec![main_body(1)]);
    held.open_scopes(1);
    let page = |params: Json| result(held.variables(&params))["variables"].clone();

    assert_eq!(
        page(json!({"variables_reference": 1, "start": 1, "count": 2})),
        json!([array_entry("Ctrs", 5, 2), scalar("Hits", "9", "DINT")])
    );
    assert_eq!(
        page(json!({"variables_reference": 1, "filter": "indexed"})),
        json!([])
    );
    assert_eq!(
        page(json!({"variables_reference": 5, "filter": "indexed", "start": 1})),
        json!([instance_entry("[1]", "Counter", 6, 2)])
    );
    assert_eq!(
        page(json!({"variables_reference": 5, "filter": "named"})),
        json!([])
    );
}

mod evaluate;
mod variables;
