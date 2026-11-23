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
