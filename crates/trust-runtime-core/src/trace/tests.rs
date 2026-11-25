//! The `.trusttrace` layout of specification 48 Appendix C, its expected bytes worked out by hand
//! from the appendix's offsets.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use super::*;

fn meta() -> Meta {
    Meta {
        image_sha256: [0x11; 32],
        sources_hash: [0x22; 32],
        target: 3,
        end: TraceEnd::Complete,
        cycle_interval_ns: 10_000_000,
        first_cycle: 7,
        cycle_count: 2,
        started_utc_ms: 1_795_000_000_000,
        runtime_version: "0.44.1".into(),
        runtime_id: "pc-1".into(),
        platform: "linux-x64".into(),
        resource: "Cell".into(),
        task: Some("Main".into()),
        origin: TraceOrigin::OnDemand,
    }
}

fn empty_trace() -> Trace {
    Trace {
        meta: meta(),
        located: Vec::new(),
        start: StartState {
            areas: Vec::new(),
            forces: Vec::new(),
        },
        image: ImageTables::default(),
        cycles: Vec::new(),
    }
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}

fn text(s: &str) -> String {
    String::from(s)
}

/// A trace with every kind of record: two located variables, a start state with an area and two
/// forces, the image's tables, and two cycles, the second faulted.
fn full_trace() -> Trace {
    let mut trace = empty_trace();
    trace.meta.end = TraceEnd::Faulted;
    trace.meta.origin = TraceOrigin::FlightRecorder;
    trace.located = vec![
        Located {
            path: text("Main.start"),
            type_name: text("BOOL"),
            area: IoArea::Input,
            bit: Some(3),
            offset: 0,
            size: 1,
        },
        Located {
            path: text("Main.speed"),
            type_name: text("INT"),
            area: IoArea::Output,
            bit: None,
            offset: 2,
            size: 2,
        },
    ];
    trace.start = StartState {
        areas: vec![(1, vec![0x08]), (6, vec![1, 2, 3, 4, 5, 6, 7, 8, 9])],
        forces: vec![
            Change {
                source: ChangeSource::ControlLink,
                kind: ChangeKind::VariableForce {
                    path: text("Main.limit"),
                    type_name: text("REAL"),
                    value: 21.5f32.to_le_bytes().to_vec(),
                },
            },
            Change {
                source: ChangeSource::Screen,
                kind: ChangeKind::AddressForce {
                    area: IoArea::Output,
                    bit: Some(0),
                    offset: 4,
                    bytes: vec![1],
                },
            },
        ],
    };
    trace.image = ImageTables {
        areas: vec![1, 2, 3],
        tasks: vec![4; 9],
        iobind: vec![5; 16],
        symbols: vec![6; 24],
        types: vec![7],
        strings: vec![0, b'M', 0],
    };
    let changes = vec![
        Change {
            source: ChangeSource::Screen,
            kind: ChangeKind::VariableWrite {
                path: text("Main.x"),
                type_name: text("INT"),
                value: vec![0x2A, 0],
            },
        },
        Change {
            source: ChangeSource::Service,
            kind: ChangeKind::VariableRelease {
                path: text("Main.limit"),
            },
        },
        Change {
            source: ChangeSource::Mesh,
            kind: ChangeKind::AddressWrite {
                area: IoArea::Memory,
                bit: None,
                offset: 12,
                bytes: vec![9, 8, 7],
            },
        },
        Change {
            source: ChangeSource::ProtocolServer,
            kind: ChangeKind::AddressRelease {
                area: IoArea::Output,
                bit: Some(0),
                offset: 4,
                size: 1,
            },
        },
        Change {
            source: ChangeSource::ControlStudio,
            kind: ChangeKind::MethodCall {
                instance: text("Main.Heating"),
                interface: text("Plant.ISequence"),
                method: text("Start"),
                inputs: vec![
                    (text("Setpoint"), text("21.5")),
                    (text("Mode"), text("'auto'")),
                ],
            },
        },
        Change {
            source: ChangeSource::Plant,
            kind: ChangeKind::VariableWrite {
                path: text("Tank.level"),
                type_name: text("LREAL"),
                value: 0.75f64.to_le_bytes().to_vec(),
            },
        },
    ];
    trace.cycles = vec![
        Cycle {
            time_ns: 0,
            activations: vec![Activation { task: 0, now_ns: 0 }],
            wall_clock_ms: vec![1_795_000_000_000, 1_795_000_000_001],
            changes,
            inputs: vec![0x08, 0, 0],
            outputs: vec![0, 0, 0x2A, 0],
            fault: None,
        },
        Cycle {
            time_ns: 10_000_000,
            activations: vec![
                Activation {
                    task: 1,
                    now_ns: 10_000_000,
                },
                Activation { task: 0, now_ns: 0 },
            ],
            wall_clock_ms: Vec::new(),
            changes: Vec::new(),
            inputs: vec![0, 0, 0],
            outputs: vec![0, 0, 0, 0],
            fault: Some(CycleFault {
                code: 3,
                pou: 12,
                statement: 345,
            }),
        },
    ];
    trace
}

#[test]
fn the_header_and_section_table_lie_where_appendix_c_puts_them() {
    // Catches a header or table field at the wrong offset. Worked by hand: the header is 64 bytes,
    // the ten required sections' table 10 x 32 = 320, so META, 128 bytes, lies at 384 and STRS at
    // 512; the header's CRC-32 covers bytes 12 to 384.
    let bytes = encode(&empty_trace());
    assert_eq!(&bytes[..8], b"TRSTT001");
    assert_eq!(u16::from_le_bytes([bytes[12], bytes[13]]), 1);
    assert_eq!(u64_at(&bytes, 16), bytes.len() as u64);
    assert_eq!(u32_at(&bytes, 24), 10);
    assert_eq!(u64_at(&bytes, 32), 64);
    assert_eq!(u32_at(&bytes, 8), crate::crc32::checksum(&bytes[12..384]));
    // The first entry: META at 384, 128 bytes, its CRC-32.
    assert_eq!(u32_at(&bytes, 64), section::META);
    assert_eq!(u64_at(&bytes, 72), 384);
    assert_eq!(u64_at(&bytes, 80), 128);
    assert_eq!(u32_at(&bytes, 88), crate::crc32::checksum(&bytes[384..512]));
    // The second: STRS at 512.
    assert_eq!(u32_at(&bytes, 96), section::STRS);
    assert_eq!(u64_at(&bytes, 104), 512);
    // META: the end at 68 and the origin at 124 of the section.
    assert_eq!(u32_at(&bytes, 384 + 68), TraceEnd::Complete as u32);
    assert_eq!(u32_at(&bytes, 384 + 124), TraceOrigin::OnDemand as u32);
    // Every section starts at a multiple of 8.
    for entry in 0..10 {
        assert_eq!(u64_at(&bytes, 64 + 32 * entry + 8) % 8, 0);
    }
}

#[test]
fn a_cycle_record_lies_as_c6_gives_it() {
    // Catches a cycle field at the wrong offset or a block without its padding. Worked by hand:
    // 24 bytes of time and counts, one activation (16), one reading (8), then the input block at
    // 48 (8 + 1 byte + 7 padding) and the output block at 64 (8), 72 bytes in all.
    let cycle = Cycle {
        time_ns: 10_000_000,
        activations: vec![Activation {
            task: 2,
            now_ns: 9_000_000,
        }],
        wall_clock_ms: vec![1_795_000_000_123],
        changes: Vec::new(),
        inputs: vec![0xAA],
        outputs: Vec::new(),
        fault: None,
    };
    let mut bytes = Vec::new();
    encode_cycle(&cycle, &mut bytes);
    assert_eq!(bytes.len(), 72);
    assert_eq!(u64_at(&bytes, 0), 10_000_000);
    assert_eq!(
        [
            u32_at(&bytes, 8),
            u32_at(&bytes, 12),
            u32_at(&bytes, 16),
            u32_at(&bytes, 20)
        ],
        [1, 0, 1, 0]
    );
    assert_eq!(u32_at(&bytes, 24), 2);
    assert_eq!(u64_at(&bytes, 32), 9_000_000);
    assert_eq!(u64_at(&bytes, 40), 1_795_000_000_123);
    assert_eq!(u32_at(&bytes, 48), 1);
    assert_eq!(bytes[56], 0xAA);
    assert_eq!(&bytes[57..64], &[0; 7]);
    assert_eq!(u32_at(&bytes, 64), 0);
}

#[test]
fn a_variables_write_lies_as_its_kind_gives_it() {
    // Catches a change's header or data in the wrong place. Worked by hand: kind 1, source 2 (a
    // screen), then the data: "Main.x" (2 + 6), "INT" (2 + 3), the value's length 2 (4) and its 2
    // bytes, 19 bytes, padded with 5 to 32 bytes from the change's start at 24.
    let cycle = Cycle {
        changes: vec![Change {
            source: ChangeSource::Screen,
            kind: ChangeKind::VariableWrite {
                path: text("Main.x"),
                type_name: text("INT"),
                value: vec![0x2A, 0],
            },
        }],
        ..Cycle::default()
    };
    let mut bytes = Vec::new();
    encode_cycle(&cycle, &mut bytes);
    assert_eq!(&bytes[24..28], &[1, 2, 0, 0]);
    assert_eq!(u32_at(&bytes, 28), 19);
    assert_eq!(&bytes[32..34], &6u16.to_le_bytes());
    assert_eq!(&bytes[34..40], b"Main.x");
    assert_eq!(&bytes[40..42], &3u16.to_le_bytes());
    assert_eq!(&bytes[42..45], b"INT");
    assert_eq!(u32_at(&bytes, 45), 2);
    assert_eq!(&bytes[49..51], &[0x2A, 0]);
    assert_eq!(&bytes[51..56], &[0; 5]);
    // The input block follows at 56.
    assert_eq!(u32_at(&bytes, 56), 0);
}

#[test]
fn a_trace_reads_back_as_it_was_written() {
    // Catches a field the reader takes from another place than the writer puts it, for every
    // record: located variables, the start state's areas and forces, the image's tables, every kind
    // and source of change, activations, readings, images and a fault.
    let trace = full_trace();
    let decoded = decode(&encode(&trace)).expect("a trace");
    assert_eq!(decoded.damaged_cycle, None);
    assert_eq!(decoded.trace, trace);
}

#[test]
fn the_texts_of_meta_and_syms_share_one_pool() {
    // Catches a pool without the empty string first, or a task's absence written as a text.
    // Worked by hand: the full trace has 12 sections (10 required, 2 cycles), so its META lies at
    // 64 + 12 x 32 = 448; "0.44.1", META's first text, lies at 1, right after the empty string.
    let trace = full_trace();
    let bytes = encode(&trace);
    assert_eq!(u64_at(&bytes, 72), 448);
    let strs = u64_at(&bytes, 104) as usize;
    assert_eq!(bytes[strs], 0);
    assert_eq!(&bytes[strs + 1..strs + 8], b"0.44.1\0");
    assert_eq!(u32_at(&bytes, 448 + 104), 1);
    let mut no_task = trace;
    no_task.meta.task = None;
    let bytes = encode(&no_task);
    assert_eq!(u32_at(&bytes, 448 + 120), 0);
    assert_eq!(decode(&bytes).unwrap().trace.meta.task, None);
}

#[test]
fn a_damaged_cycle_ends_the_trace_before_it() {
    // Catches a reader that refuses the whole trace, or reads on past, when one cycle is damaged
    // (Appendix C, C.2): the first cycle is kept, and the trace ends at the second.
    let trace = full_trace();
    let mut bytes = encode(&trace);
    let second_cycle = u64_at(&bytes, 64 + 32 * 11 + 8) as usize;
    bytes[second_cycle] ^= 0xFF;
    let decoded = decode(&bytes).expect("a trace");
    assert_eq!(decoded.damaged_cycle, Some(1));
    assert_eq!(decoded.trace.cycles, trace.cycles[..1]);
}

#[test]
fn a_damaged_header_or_required_section_refuses_the_trace() {
    // Catches a reader that takes a damaged file as a trace: each refusal names its cause.
    let bytes = encode(&full_trace());
    let mut damaged = bytes.clone();
    damaged[448 + 70] ^= 1; // META's end code (META at 448, as above)
    assert_eq!(decode(&damaged), Err(TraceError::SectionDamaged("META")));
    let mut damaged = bytes.clone();
    damaged[64 + 8] ^= 1; // META's offset in the section table
    assert_eq!(decode(&damaged), Err(TraceError::HeaderDamaged));
    let mut damaged = bytes.clone();
    damaged[0] = b'X';
    assert_eq!(decode(&damaged), Err(TraceError::NotATrace));
    let mut damaged = bytes.clone();
    damaged[12] = 2;
    assert_eq!(decode(&damaged), Err(TraceError::UnsupportedVersion(2)));
    assert_eq!(
        decode(&bytes[..bytes.len() - 8]),
        Err(TraceError::Truncated("the file"))
    );
    assert_eq!(
        decode(&bytes[..40]),
        Err(TraceError::Truncated("the header"))
    );
}

#[test]
fn a_record_from_parts_is_the_cycles_record_and_its_length_is_known() {
    // Catches a recorder's record that differs from the cycle's, or a length the recorder reserves
    // that is not what is written: the full trace's faulted cycle, with every change kind of the first.
    let trace = full_trace();
    for cycle in &trace.cycles {
        let mut whole = Vec::new();
        encode_cycle(cycle, &mut whole);
        let mut changes = Vec::new();
        for change in &cycle.changes {
            let mut pairs = Vec::new();
            let borrowed = change.kind.borrowed(&mut pairs);
            let before = changes.len();
            encode_change(change.source, &borrowed, &mut changes);
            assert_eq!(changes.len() - before, change_len(&borrowed));
        }
        let parts = CycleParts {
            time_ns: cycle.time_ns,
            activations: &cycle.activations,
            wall_clock_ms: &cycle.wall_clock_ms,
            change_count: cycle.changes.len(),
            changes: &changes,
            inputs: &cycle.inputs,
            outputs: &cycle.outputs,
            fault: cycle.fault,
        };
        let mut from_parts = Vec::new();
        encode_cycle_parts(&parts, &mut from_parts);
        assert_eq!(from_parts, whole);
        assert_eq!(cycle_record_len(&parts), whole.len());
    }
}

#[test]
fn a_record_written_into_reserved_room_allocates_nothing() {
    // Catches a writer that grows its buffer during a scan (specification 34 MEM-09): with room for
    // the record, the buffer keeps its place and its capacity.
    let cycle = &full_trace().cycles[1];
    let parts = CycleParts {
        time_ns: cycle.time_ns,
        activations: &cycle.activations,
        wall_clock_ms: &cycle.wall_clock_ms,
        change_count: 0,
        changes: &[],
        inputs: &cycle.inputs,
        outputs: &cycle.outputs,
        fault: cycle.fault,
    };
    let mut out = Vec::with_capacity(cycle_record_len(&parts));
    let (place, capacity) = (out.as_ptr(), out.capacity());
    encode_cycle_parts(&parts, &mut out);
    assert_eq!((out.as_ptr(), out.capacity()), (place, capacity));
    assert_eq!(out.len(), capacity);
}

#[test]
fn a_trace_of_records_reads_back_as_the_trace() {
    // Catches a file assembled from a recorder's records that reads back otherwise than the trace.
    let trace = full_trace();
    let records: Vec<Vec<u8>> = trace
        .cycles
        .iter()
        .map(|cycle| {
            let mut bytes = Vec::new();
            encode_cycle(cycle, &mut bytes);
            bytes
        })
        .collect();
    let records: Vec<&[u8]> = records.iter().map(Vec::as_slice).collect();
    let mut forces = Vec::new();
    for force in &trace.start.forces {
        let mut pairs = Vec::new();
        encode_change(force.source, &force.kind.borrowed(&mut pairs), &mut forces);
    }
    let areas: Vec<(u32, &[u8])> = trace
        .start
        .areas
        .iter()
        .map(|(id, bytes)| (*id, bytes.as_slice()))
        .collect();
    let start = StartParts {
        areas: &areas,
        force_count: trace.start.forces.len(),
        forces: &forces,
    };
    let bytes = encode_records(&trace.meta, &trace.located, &start, &trace.image, &records);
    assert_eq!(bytes, encode(&trace));
    assert_eq!(decode(&bytes).unwrap().trace, trace);
}

#[test]
fn a_traces_tasks_are_named_from_its_own_tables() {
    // Specification 48 RU-60, "Each cycle": a cycle's activations index the recorded image's task
    // table, and the build's tasks of the same names run them, so the names come from the trace's
    // own task table and string pool. Worked by hand: two records naming offset 1 and offset 6 of
    // the pool "\0Fast\0Slow\0" are `Fast` and `Slow`; a count of 3 with two records' bytes is
    // malformed. Catches a name read at the wrong offset, a record read past the table, and a
    // malformed table taken.
    use super::replay::task_names;
    use crate::native::image::{Record, TaskRecord, NONE, TABLE_PREFIX_BYTES};

    let record = |name| {
        let mut bytes = vec![0; TaskRecord::BYTES];
        TaskRecord {
            name,
            interval_ns: 10_000_000,
            priority: 0,
            single: NONE,
            program_count: 0,
            program_first: 0,
            flags: 0,
        }
        .write(&mut bytes);
        bytes
    };
    let table = |count: u32| {
        let mut tasks = vec![0; TABLE_PREFIX_BYTES];
        tasks[..4].copy_from_slice(&count.to_le_bytes());
        tasks.extend(record(1));
        tasks.extend(record(6));
        ImageTables {
            tasks,
            strings: b"\0Fast\0Slow\0".to_vec(),
            ..ImageTables::default()
        }
    };
    assert_eq!(task_names(&table(2)).unwrap(), [text("Fast"), text("Slow")]);
    assert!(matches!(
        task_names(&table(3)),
        Err(TraceError::Malformed {
            section: "ITASKS",
            ..
        })
    ));
}

#[test]
fn a_cycles_outputs_differ_by_the_located_output_variables() {
    // Specification 48 RU-60, "Each cycle": a replay compares the output image after a cycle with
    // the trace's by the trace's located output variables at their addresses: a variable of one bit
    // by its bit, any other by its bytes; an input is not compared, and a variable one image does
    // not hold differs. Worked by hand: the trace's outputs 02 00 05 00 .. 01 (ten bytes) and the
    // build's 06 00 06 00: `Lamp` %QX0.1 is set in both (bit 2, which differs, is no variable's);
    // `Speed` %QW2 is 5 against 6; `Start` %IX0.0 is an input; `Far` %QB9 lies past the build's
    // image. Catches a bit compared by its whole byte, an input compared, and a variable past an
    // image taken as equal.
    use super::replay::differing_outputs;

    let located = |path: &str, area, bit, offset, size| Located {
        path: text(path),
        type_name: text("ANY"),
        area,
        bit,
        offset,
        size,
    };
    let variables = [
        located("Lamp", IoArea::Output, Some(1), 0, 1),
        located("Speed", IoArea::Output, None, 2, 2),
        located("Start", IoArea::Input, Some(0), 0, 1),
        located("Far", IoArea::Output, None, 9, 1),
    ];
    let trace = [0b0000_0010, 0, 5, 0, 0, 0, 0, 0, 0, 1];
    let build = [0b0000_0110, 0, 6, 0];
    let differing: Vec<&str> = differing_outputs(&variables, &trace, &build)
        .iter()
        .map(|variable| variable.path.as_str())
        .collect();
    assert_eq!(differing, ["Speed", "Far"]);
    assert_eq!(
        differing_outputs(&variables, &trace, &trace),
        Vec::<&Located>::new()
    );
}

#[test]
fn a_recorded_image_holds_the_traces_tables_and_its_memory_reads_the_start() {
    // Specification 48 RU-60, "The start": a build laid out otherwise carries the recorded memory as
    // an online change carries a running program's, from the recorded program's image and memory.
    // Worked by hand from `full_trace`: the image put together from its tables holds each of them
    // in its own section, so it lays its memory out as they say; its memory reads area 6's bytes
    // 3, 4, 5 at offset 2, but nothing past the area's 9 bytes, nothing of area 2, which the start
    // does not hold, and takes no write. A trace that names no target (0) has no image. Catches a
    // table put in another section, an area read at the wrong offset or past its end, a recorded
    // memory that can be written, and an image made for a target no build has.
    use super::replay::{recorded_image, same_layout, RecordedMemory};
    use crate::native::image::{Image, SectionId};
    use crate::native::memory::ProgramMemory;

    let trace = full_trace();
    let bytes = recorded_image(&trace).unwrap();
    let image = Image::parse(&bytes).unwrap();
    assert!(same_layout(&trace.image, &image));
    for (id, table) in [
        (SectionId::Tasks, &trace.image.tasks),
        (SectionId::IoBind, &trace.image.iobind),
        (SectionId::Strings, &trace.image.strings),
    ] {
        assert_eq!(image.section_bytes(id), &table[..], "{id:?}");
    }
    let mut memory = RecordedMemory::new(image, &trace.start.areas);
    let mut out = [0; 3];
    assert!(memory.read_area(6, 2, &mut out));
    assert_eq!(out, [3, 4, 5]);
    assert!(!memory.read_area(6, 7, &mut out));
    assert!(!memory.read_area(2, 0, &mut out));
    assert!(!memory.write_area(6, 0, &[0]));
    let output = crate::io_address::IoAddress::parse("%QB0").unwrap();
    assert!(memory
        .write_image(&output, crate::value::Value::Byte(1))
        .is_err());

    let mut nowhere = full_trace();
    nowhere.meta.target = 0;
    assert!(matches!(
        recorded_image(&nowhere),
        Err(TraceError::Malformed {
            section: "META",
            ..
        })
    ));
}

#[test]
fn a_recorded_number_is_replayed_as_the_literal_a_request_gave() {
    // Specification 48 RU-60, "Each cycle": a change is applied as the runtime applied it, and the
    // runtime typed a request's integer literal for its variable, so an integer or a bit string is
    // given back as that literal. Worked by hand: INT -5 is the literal -5, WORD 65 535 the literal
    // 65 535, LWORD 5 the literal 5; ULINT 2^64 - 1 is past the literal's range and stays, and so do
    // a REAL and a BOOL. Catches a sign lost, a bit string left typed, which a build that changed
    // the variable's type refuses, and a number cut to the literal's range.
    use super::replay::as_requested;
    use crate::value::Value;

    assert_eq!(as_requested(Value::Int(-5)), Value::LInt(-5));
    assert_eq!(as_requested(Value::Word(65_535)), Value::LInt(65_535));
    assert_eq!(as_requested(Value::LWord(5)), Value::LInt(5));
    assert_eq!(as_requested(Value::ULInt(u64::MAX)), Value::ULInt(u64::MAX));
    assert_eq!(as_requested(Value::Real(1.5)), Value::Real(1.5));
    assert_eq!(as_requested(Value::Bool(true)), Value::Bool(true));
}

#[test]
fn a_files_variables_and_instances_are_named_as_the_debugger_names_them() {
    // Appendix C, C.6: a trace names each variable and each method call's instance as the debugger
    // names it, whatever form the request named it in; the recording keeps the names as given and
    // its file names them. Here the debugger names `Main.x` `Main.X`, `Main.limit` `Main.Limit` and
    // `Main.Heating` `Plant1.Heating`, and gives `Tank.level` no name. Worked by hand from
    // `full_trace`: the write, the release, the method call and the start's force take the new
    // names; `Tank.level` and every address change stay as they were, and so do the values, the
    // inputs and the cycle without changes, byte for byte. Catches a path left as recorded, an
    // instance not named, a name given to a path the debugger does not name, and any other change.
    let mut debugger = |path: &str| -> Option<String> {
        match path {
            "Main.x" => Some(text("Main.X")),
            "Main.limit" => Some(text("Main.Limit")),
            "Main.Heating" => Some(text("Plant1.Heating")),
            _ => None,
        }
    };
    let trace = full_trace();
    let mut expected = full_trace();
    let changes = &mut expected.cycles[0].changes;
    if let ChangeKind::VariableWrite { path, .. } = &mut changes[0].kind {
        *path = text("Main.X");
    }
    if let ChangeKind::VariableRelease { path } = &mut changes[1].kind {
        *path = text("Main.Limit");
    }
    if let ChangeKind::MethodCall { instance, .. } = &mut changes[4].kind {
        *instance = text("Plant1.Heating");
    }
    if let ChangeKind::VariableForce { path, .. } = &mut expected.start.forces[0].kind {
        *path = text("Main.Limit");
    }
    for (cycle, expected) in trace.cycles.iter().zip(&expected.cycles) {
        let mut record = Vec::new();
        encode_cycle(cycle, &mut record);
        let named = named_cycle(&record, &mut debugger).unwrap();
        assert_eq!(&super::read::read_cycle(&named).unwrap(), expected);
        assert_eq!(named_cycle(&record, &mut |_| None).unwrap(), record);
    }
    let mut forces = Vec::new();
    for force in &trace.start.forces {
        encode_change(
            force.source,
            &force.kind.borrowed(&mut Vec::new()),
            &mut forces,
        );
    }
    let count = trace.start.forces.len();
    let named = named_changes(&forces, count, &mut debugger).unwrap();
    assert_eq!(
        super::read::read_change_run(&named, count).unwrap(),
        expected.start.forces
    );
}
