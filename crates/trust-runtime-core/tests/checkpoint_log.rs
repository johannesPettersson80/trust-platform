//! The checkpoint log of the chips' retain store (specification 34 §10.1; RETAIN-01, RETAIN-02):
//! the record layout, the order of a write, recovery after a power cut at every byte of a write
//! and an erase, and the refusal of a damaged newest checkpoint.
//!
//! The flash is simulated (`checkpoint_log/flash.rs`) with the ESP32-C6's program and erase units,
//! which `checkpoint_log/chips.rs` reads from its chip file.

#[path = "checkpoint_log/chips.rs"]
mod chips;
#[path = "checkpoint_log/flash.rs"]
mod flash;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use flash::{Cut, EraseCut, Op, SimFlash};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::checkpoint::{
    CheckpointError, CheckpointIdentity, CheckpointLog, Corruption, FlashError, FlashGeometry,
    Progress, RecordInfo, Slot, COMMIT_MARKER, FORMAT_VERSION, RECORD_MAGIC, SLOT_MAGIC,
};

/// A checkpoint as recovery returns it: the record and its payload.
type Checkpoint = (RecordInfo, Vec<u8>);

/// What the recovery rule returns.
type Recovered = Result<Option<Checkpoint>, CheckpointError>;

/// A write in a sweep: the tag of its identity and payload, and the payload length.
#[derive(Debug, Clone, Copy)]
struct Write {
    tag: u8,
    len: usize,
}

fn identity(tag: u8) -> CheckpointIdentity {
    CheckpointIdentity {
        application: [tag; 16],
        schema: [!tag; 16],
    }
}

/// A payload of `len` bytes that differs for every tag.
fn payload(tag: u8, len: usize) -> Vec<u8> {
    (0..len)
        .map(|index| (index as u8).wrapping_mul(31) ^ tag.wrapping_mul(97))
        .collect()
}

/// Start the log on `flash`, as after a reset, and apply the recovery rule.
fn recover(flash: &mut SimFlash) -> Recovered {
    let mut log = CheckpointLog::open(&mut *flash)?;
    let mut buffer = vec![0; 16 * 1024];
    let record = log.recover(&mut buffer)?;
    Ok(record.map(|record| (record, buffer[..record.payload_len as usize].to_vec())))
}

/// Start the log on `flash` and write a checkpoint.
fn write(flash: &mut SimFlash, tag: u8, len: usize) -> Checkpoint {
    let data = payload(tag, len);
    let record = CheckpointLog::open(&mut *flash)
        .and_then(|mut log| log.append(identity(tag), &data))
        .unwrap_or_else(|error| panic!("checkpoint {tag}: {error}"));
    assert_eq!(
        (record.identity, record.payload_len as usize),
        (identity(tag), len)
    );
    (record, data)
}

/// A flash that holds the checkpoints `(tag, payload length)`, written in order.
fn store(geometry: FlashGeometry, checkpoints: &[(u8, usize)]) -> SimFlash {
    let mut flash = SimFlash::new(geometry);
    for &(tag, len) in checkpoints {
        write(&mut flash, tag, len);
    }
    flash
}

/// Two checkpoints in `checkpoint_a`, with room for more.
fn two_records(geometry: FlashGeometry) -> SimFlash {
    store(geometry, &[(1, 100), (2, 120)])
}

/// Checkpoints in both 16 KiB slots: `checkpoint_a` holds the two oldest; `checkpoint_b`,
/// formatted later, holds four more, the newest of 100 bytes, and has 96 bytes left, too few for
/// another of 100 bytes.
fn both_slots(geometry: FlashGeometry) -> SimFlash {
    store(
        geometry,
        &[
            (1, 7000),
            (2, 7000),
            (3, 7000),
            (4, 7000),
            (5, 1900),
            (6, 100),
        ],
    )
}

/// The records of a slot, read by the layout of §10.1 with 4-byte program units:
/// `(offset, payload length)`.
fn records(flash: &SimFlash, slot: Slot) -> Vec<(u32, u32)> {
    let bytes = flash.slot(slot);
    let mut offset = 16;
    let mut found = Vec::new();
    while offset + 68 <= bytes.len() && bytes[offset..offset + 4] == RECORD_MAGIC {
        let len = u32::from_le_bytes(bytes[offset + 48..offset + 52].try_into().unwrap());
        found.push((offset as u32, len));
        offset += 68 + (len as usize).next_multiple_of(4);
    }
    found
}

/// Run `first` from `start` with power cut at `cut`, and power the flash again.
fn interrupted(start: &SimFlash, first: Write, cut: Cut) -> SimFlash {
    let mut flash = start.clone();
    flash.cut_at(cut);
    let data = payload(first.tag, first.len);
    let result =
        CheckpointLog::open(&mut flash).and_then(|mut log| log.append(identity(first.tag), &data));
    assert!(
        result.is_err(),
        "{cut:?}: the write committed although power was cut"
    );
    assert!(
        !flash.powered(),
        "{cut:?}: the write failed without a power cut"
    );
    flash.power_cycle();
    flash
}

/// The operations of `first` from `start`, and the checkpoint it writes.
fn operations(start: &SimFlash, first: Write) -> (Vec<Op>, Checkpoint) {
    let mut flash = start.clone();
    flash.record_operations();
    let checkpoint = write(&mut flash, first.tag, first.len);
    assert_eq!(recover(&mut flash).unwrap(), Some(checkpoint.clone()));
    (flash.operations().to_vec(), checkpoint)
}

/// The bytes programmed or erased before the program operation number `program` of `ops`.
fn before_program(ops: &[Op], program: usize) -> u64 {
    let index = ops
        .iter()
        .enumerate()
        .filter(|(_, op)| !op.erase)
        .nth(program)
        .map(|(index, _)| index)
        .expect("the program operation");
    ops[..index].iter().map(|op| u64::from(op.len)).sum()
}

/// Cut power at every byte `first` programs or erases from `start`, with a clean and a half-done
/// byte at the cut of a program and both erase outcomes at the cut of an erase. After every cut,
/// recovery returns exactly the checkpoint before the write, and `next` then writes and recovers.
/// Returns the number of cuts.
fn sweep(start: &SimFlash, first: Write, next: Write) -> usize {
    let before = recover(&mut start.clone()).expect("the start state recovers");
    let (ops, (after, _)) = operations(start, first);
    let mut cuts = 0;
    let mut done = 0;
    for op in &ops {
        let variants = if op.erase {
            [(true, EraseCut::LeaveOld), (true, EraseCut::Zero)]
        } else {
            [(false, EraseCut::LeaveOld), (true, EraseCut::LeaveOld)]
        };
        for byte in 0..u64::from(op.len) {
            for (partial, erase) in variants {
                let cut = Cut {
                    after: done + byte,
                    partial,
                    erase,
                };
                let mut flash = interrupted(start, first, cut);
                let recovered =
                    recover(&mut flash).unwrap_or_else(|error| panic!("{cut:?}: {error}"));
                assert_eq!(
                    recovered, before,
                    "{cut:?}: recovery must return the previous checkpoint"
                );
                let (record, data) = write(&mut flash, next.tag, next.len);
                assert!(
                    record.sequence == after.sequence || record.sequence == after.sequence + 1,
                    "{cut:?}: the next sequence is {}, after {}",
                    record.sequence,
                    after.sequence
                );
                let recovered =
                    recover(&mut flash).unwrap_or_else(|error| panic!("{cut:?}: {error}"));
                assert_eq!(
                    recovered,
                    Some((record, data)),
                    "{cut:?}: the next write must recover"
                );
                cuts += 1;
            }
        }
        done += u64::from(op.len);
    }
    cuts
}

fn geometries() -> [FlashGeometry; 1] {
    chips::chips().map(|chip| chip.geometry())
}

#[test]
fn chip_files_give_each_checkpoint_slot_whole_erase_units() {
    let [c6] = chips::chips();
    // ESP32-C6: four 4 KiB sectors each, from sector 0x7F8 = 2040 at offset 0x7F_8000 and sector
    // 0x7FC = 2044 at offset 0x7F_C000.
    assert_eq!(c6.slots.map(|slot| slot.first_unit), [2040, 2044]);
    assert_eq!(
        c6.slots.map(|slot| (slot.len, slot.erase_unit)),
        [(16_384, 4096); 2]
    );
    assert!(c6
        .slots
        .iter()
        .all(|slot| slot.start.is_multiple_of(slot.erase_unit)));
    let geometry = c6.geometry();
    assert_eq!(geometry.program_unit(), 4);
    assert_eq!(geometry.max_payload_len(), 16_300);
    // Records per slot before it is erased again: the wear figures put to the owner.
    for (len, per_slot) in [(256, 50), (1024, 14), (4096, 3)] {
        let record = geometry.record_len(len).unwrap();
        assert_eq!((geometry.slot_len() - 16) / record, per_slot, "{len}");
    }
    assert_eq!(geometry.erase_units(), 4);
}

#[test]
fn geometry_refuses_flash_the_layout_cannot_use() {
    for (program, erase, slot) in [
        (0, 4096, 16_384),
        (3, 4096, 16_384),
        (128, 4096, 16_384),
        (4, 0, 16_384),
        (4, 4094, 16_384),
        (4, 4096, 16_000),
        (4, 4, 80),
    ] {
        assert_eq!(
            FlashGeometry::new(program, erase, slot),
            Err(CheckpointError::Geometry),
            "{program}, {erase}, {slot}"
        );
    }
    let smallest = FlashGeometry::new(4, 4, 84).unwrap();
    assert_eq!(smallest.max_payload_len(), 0);
    assert_eq!(smallest.record_len(0), Some(68));
    assert_eq!(smallest.record_len(1), None);
    let wide = FlashGeometry::new(32, 4096, 16_384).unwrap();
    assert_eq!((wide.first_record_offset(), wide.commit_len()), (32, 32));
    assert_eq!(wide.record_len(1), Some(64 + 32 + 32));
    let bytewise = FlashGeometry::new(1, 4096, 16_384).unwrap();
    assert_eq!(
        (bytewise.first_record_offset(), bytewise.commit_len()),
        (16, 4)
    );
    assert_eq!(bytewise.record_len(3), Some(64 + 4 + 3));
    let chip = geometries()[0];
    assert_eq!((chip.first_record_offset(), chip.commit_len()), (16, 4));
    assert_eq!(chip.record_len(1), Some(72));
    assert_eq!(chip.record_len(16_300), Some(16_368));
    assert_eq!(chip.record_len(16_301), None);
}

#[test]
fn slot_header_and_record_bytes_follow_the_specified_layout() {
    for geometry in geometries() {
        let mut flash = SimFlash::new(geometry);
        let identity = CheckpointIdentity {
            application: core::array::from_fn(|index| index as u8),
            schema: core::array::from_fn(|index| 0x80 | index as u8),
        };
        let data = payload(7, 10);
        let mut log = CheckpointLog::open(&mut flash).unwrap();
        let first = log.append(identity, &data).unwrap();
        let second = log.append(identity, &data).unwrap();
        assert_eq!((first.sequence, second.sequence), (1, 2));

        let slot = flash.slot(Slot::A);
        assert_eq!(slot[0..4], SLOT_MAGIC);
        assert_eq!(slot[4..6], FORMAT_VERSION.to_le_bytes());
        assert_eq!(slot[6..8], [0, 0]);
        assert_eq!(slot[8..12], 1u32.to_le_bytes(), "the first epoch");
        assert_eq!(slot[12..16], crc32fast::hash(&slot[0..12]).to_le_bytes());

        let record = &slot[16..96];
        assert_eq!(record[0..4], RECORD_MAGIC);
        assert_eq!(record[4..6], FORMAT_VERSION.to_le_bytes());
        assert_eq!(record[6..8], [0, 0], "flags");
        assert_eq!(record[8..16], 1u64.to_le_bytes(), "sequence");
        assert_eq!(record[16..32], identity.application);
        assert_eq!(record[32..48], identity.schema);
        assert_eq!(record[48..52], 10u32.to_le_bytes(), "payload length");
        assert_eq!(record[52..56], crc32fast::hash(&data).to_le_bytes());
        assert_eq!(record[56..60], [0; 4], "reserved");
        assert_eq!(
            record[60..64],
            crc32fast::hash(&record[0..60]).to_le_bytes()
        );
        assert_eq!(record[64..68], COMMIT_MARKER);
        assert_eq!(record[68..78], data[..]);
        assert_eq!(record[78..80], [0xFF; 2], "padding to the program unit");

        // The second record starts at the next program unit: 16 + 68 + 12.
        assert_eq!(slot[96..100], RECORD_MAGIC);
        assert_eq!(slot[104..112], 2u64.to_le_bytes());
        assert_eq!(records(&flash, Slot::A), [(16, 10), (96, 10)]);
        assert!(flash.slot(Slot::A)[176..].iter().all(|&byte| byte == 0xFF));
        assert!(flash.slot(Slot::B).iter().all(|&byte| byte == 0xFF));
    }
}

#[test]
fn a_blank_flash_recovers_as_an_empty_store() {
    for geometry in geometries() {
        let mut flash = SimFlash::new(geometry);
        assert_eq!(recover(&mut flash), Ok(None));
        let mut log = CheckpointLog::open(&mut flash).unwrap();
        assert_eq!(log.newest(), Ok(None));
        let mut empty: [u8; 0] = [];
        assert_eq!(log.recover(&mut empty), Ok(None));
    }
}

#[test]
fn checkpoints_alternate_between_the_slots_and_erase_once_per_slot_of_records() {
    for chip in chips::chips() {
        let geometry = chip.geometry();
        let mut flash = SimFlash::new(geometry);
        let per_slot = (geometry.slot_len() - 16) / geometry.record_len(1024).unwrap();
        let writes: u32 = 100;
        for number in 0..writes {
            let checkpoint = write(&mut flash, number as u8, 1024);
            assert_eq!(checkpoint.0.sequence, u64::from(number) + 1);
            assert_eq!(
                recover(&mut flash).unwrap(),
                Some(checkpoint),
                "{}",
                chip.name
            );
        }
        // The first write formats checkpoint_a; every further slot of records formats the other.
        let formats = 1 + (writes - 1) / per_slot;
        let units = geometry.erase_units();
        assert_eq!(per_slot, 14);
        assert_eq!(
            flash.erases(Slot::A),
            formats.div_ceil(2) * units,
            "{}",
            chip.name
        );
        assert_eq!(flash.erases(Slot::B), formats / 2 * units, "{}", chip.name);
    }
}

#[test]
fn payloads_of_every_size_recover_exactly() {
    for geometry in geometries() {
        let max = geometry.max_payload_len() as usize;
        let mut flash = SimFlash::new(geometry);
        let lengths = [
            0,
            1,
            2,
            3,
            4,
            5,
            63,
            64,
            65,
            255,
            256,
            257,
            4095,
            4096,
            max - 1,
            max,
            0,
            7,
        ];
        for (tag, len) in lengths.into_iter().enumerate() {
            let checkpoint = write(&mut flash, tag as u8, len);
            assert_eq!(recover(&mut flash).unwrap(), Some(checkpoint));
        }
        let previous = recover(&mut flash).unwrap();
        let mut log = CheckpointLog::open(&mut flash).unwrap();
        assert_eq!(log.geometry(), geometry);
        let too_large = payload(9, max + 1);
        assert_eq!(
            log.append(identity(9), &too_large),
            Err(CheckpointError::TooLarge {
                len: max + 1,
                max: max as u32
            })
        );
        let mut short = vec![0; 6];
        assert_eq!(
            log.recover(&mut short),
            Err(CheckpointError::BufferTooSmall { needed: 7 })
        );
        assert_eq!(recover(&mut flash).unwrap(), previous);
    }
}

#[test]
fn power_cut_at_every_byte_of_the_first_write() {
    for geometry in geometries() {
        let blank = SimFlash::new(geometry);
        let (ops, _) = operations(&blank, Write { tag: 1, len: 100 });
        assert!(ops[0].erase && ops[0].slot == Slot::A && ops[0].offset == 0);
        let cuts = sweep(
            &blank,
            Write { tag: 1, len: 100 },
            Write { tag: 2, len: 50 },
        );
        assert_eq!(cuts, 2 * (16_384 + 16 + 64 + 100 + 4));
    }
}

#[test]
fn power_cut_at_every_byte_of_an_append() {
    for geometry in geometries() {
        let start = two_records(geometry);
        let (ops, _) = operations(&start, Write { tag: 3, len: 150 });
        assert!(ops.iter().all(|op| !op.erase && op.slot == Slot::A));
        let cuts = sweep(
            &start,
            Write { tag: 3, len: 150 },
            Write { tag: 4, len: 30 },
        );
        assert_eq!(cuts, 2 * (64 + 148 + 4 + 4));
    }
}

#[test]
fn power_cut_at_every_byte_of_a_rotation() {
    for geometry in geometries() {
        let start = both_slots(geometry);
        assert_eq!(records(&start, Slot::B).len(), 4);
        let (ops, _) = operations(&start, Write { tag: 7, len: 100 });
        // The slot that holds the newest checkpoint is kept; the other one is erased.
        assert!(ops.iter().all(|op| op.slot == Slot::A));
        assert_eq!(
            ops.iter().filter(|op| op.erase).count() as u32,
            geometry.erase_units()
        );
        sweep(
            &start,
            Write { tag: 7, len: 100 },
            Write { tag: 8, len: 40 },
        );
    }
}

#[test]
fn power_cut_at_every_byte_of_the_write_after_a_torn_first_record() {
    for geometry in geometries() {
        // A rotation formatted checkpoint_a and lost power inside its first record's header:
        // the newer slot holds no checkpoint, and the newest stays in checkpoint_b.
        let start = both_slots(geometry);
        let first = Write { tag: 7, len: 100 };
        let (ops, _) = operations(&start, first);
        let cut = Cut {
            after: before_program(&ops, 1) + 10,
            partial: false,
            erase: EraseCut::LeaveOld,
        };
        let torn = interrupted(&start, first, cut);
        let (ops, _) = operations(&torn, Write { tag: 8, len: 100 });
        // The next write erases the newer slot again, never the one with the newest checkpoint.
        assert!(ops.iter().all(|op| op.slot == Slot::A) && ops[0].erase);
        sweep(&torn, Write { tag: 8, len: 100 }, Write { tag: 9, len: 40 });
    }
}

#[test]
fn power_cut_at_every_byte_of_the_write_after_a_torn_header() {
    for geometry in geometries() {
        // An append lost power inside its header: the log of checkpoint_a ends there.
        let start = two_records(geometry);
        let first = Write { tag: 3, len: 100 };
        let (ops, _) = operations(&start, first);
        let cut = Cut {
            after: before_program(&ops, 0) + 21,
            partial: true,
            erase: EraseCut::LeaveOld,
        };
        let torn = interrupted(&start, first, cut);
        let (ops, _) = operations(&torn, Write { tag: 4, len: 100 });
        assert!(ops.iter().all(|op| op.slot == Slot::B) && ops[0].erase);
        sweep(&torn, Write { tag: 4, len: 100 }, Write { tag: 5, len: 40 });
    }
}

#[test]
fn power_cut_at_every_byte_of_the_write_after_a_torn_payload() {
    for geometry in geometries() {
        // An append lost power inside its payload: the record is absent, and the log goes on
        // after it.
        let start = two_records(geometry);
        let first = Write { tag: 3, len: 100 };
        let (ops, _) = operations(&start, first);
        let cut = Cut {
            after: before_program(&ops, 1) + 33,
            partial: true,
            erase: EraseCut::LeaveOld,
        };
        let torn = interrupted(&start, first, cut);
        let (ops, (record, _)) = operations(&torn, Write { tag: 4, len: 100 });
        assert!(ops.iter().all(|op| !op.erase && op.slot == Slot::A));
        assert_eq!(ops[0].offset, 16 + 168 + 188 + 168, "after the torn record");
        assert_eq!(record.sequence, 4, "the torn record took sequence 3");
        sweep(&torn, Write { tag: 4, len: 100 }, Write { tag: 5, len: 40 });
    }
}

#[test]
fn other_program_units_lay_out_and_recover_after_cuts() {
    for program_unit in [1, 32] {
        let geometry = FlashGeometry::new(program_unit, 4096, 16_384).unwrap();
        let mut flash = SimFlash::new(geometry);
        for tag in 0..40 {
            let checkpoint = write(&mut flash, tag, 700 + usize::from(tag) * 3);
            assert_eq!(recover(&mut flash).unwrap(), Some(checkpoint));
        }
        assert!(flash.erases(Slot::A) > 0 && flash.erases(Slot::B) > 0);
        let start = store(geometry, &[(1, 101), (2, 33)]);
        sweep(&start, Write { tag: 3, len: 57 }, Write { tag: 4, len: 9 });
    }
}

/// `both_slots` with the bits of `mask` flipped at `offset` in `slot`, recovered.
fn damaged(geometry: FlashGeometry, slot: Slot, offset: u32, mask: u8) -> (SimFlash, Recovered) {
    let mut flash = both_slots(geometry);
    flash.flip(slot, offset, mask);
    let recovered = recover(&mut flash);
    (flash, recovered)
}

/// After a refused warm start, a cold start writes new checkpoints, and they recover.
fn cold_start_recovers(mut flash: SimFlash) {
    let checkpoint = write(&mut flash, 0x70, 100);
    assert_eq!(recover(&mut flash).unwrap(), Some(checkpoint));
}

#[test]
fn a_damaged_newest_payload_fails_the_warm_start() {
    for geometry in geometries() {
        let newest = records(&both_slots(geometry), Slot::B)[3].0;
        let (mut flash, recovered) = damaged(geometry, Slot::B, newest + 68 + 10, 0x04);
        assert_eq!(
            recovered,
            Err(CheckpointError::Corrupt(Corruption::Payload))
        );
        // The header is intact, so the record is still the newest; it is not replaced by an
        // older one.
        let mut log = CheckpointLog::open(&mut flash).unwrap();
        assert_eq!(log.newest().unwrap().map(|record| record.sequence), Some(6));
        cold_start_recovers(flash);
    }
}

#[test]
fn a_damaged_newest_header_fails_the_warm_start() {
    for geometry in geometries() {
        let newest = records(&both_slots(geometry), Slot::B)[3].0;
        let (mut flash, recovered) = damaged(geometry, Slot::B, newest + 8, 0x01);
        assert_eq!(
            recovered,
            Err(CheckpointError::Corrupt(Corruption::RecordHeader))
        );
        let mut log = CheckpointLog::open(&mut flash).unwrap();
        assert_eq!(
            log.newest(),
            Err(CheckpointError::Corrupt(Corruption::RecordHeader))
        );
        cold_start_recovers(flash);
    }
}

#[test]
fn damage_before_the_newest_record_in_its_slot_fails_the_warm_start() {
    for geometry in geometries() {
        // The damaged header of the third record of checkpoint_b ends that slot's log, so the
        // newest record behind it cannot be reached.
        let third = records(&both_slots(geometry), Slot::B)[2].0;
        let (flash, recovered) = damaged(geometry, Slot::B, third + 48, 0x10);
        assert_eq!(
            recovered,
            Err(CheckpointError::Corrupt(Corruption::RecordHeader))
        );
        cold_start_recovers(flash);
    }
}

#[test]
fn damage_to_superseded_records_and_the_older_slot_header_is_ignored() {
    for geometry in geometries() {
        let start = both_slots(geometry);
        let newest = recover(&mut start.clone()).unwrap();
        assert_eq!(newest.as_ref().map(|(record, _)| record.sequence), Some(6));
        let second = records(&start, Slot::B)[1].0;
        let oldest = records(&start, Slot::A)[0].0;
        for (slot, offset) in [
            (Slot::B, second + 68 + 100), // an older payload in the newest slot
            (Slot::A, oldest + 8),        // a header in the older slot
            (Slot::A, oldest + 68 + 5),   // a payload in the older slot
            (Slot::A, 8),                 // the older slot's header
        ] {
            let (_, recovered) = damaged(geometry, slot, offset, 0x20);
            assert_eq!(recovered.as_ref(), Ok(&newest), "{slot:?} at {offset}");
        }
    }
}

#[test]
fn a_damaged_header_of_the_newest_slot_fails_the_warm_start() {
    for geometry in geometries() {
        // checkpoint_b holds checkpoints newer than any in checkpoint_a, so a damaged header in
        // front of them must not hide them.
        let (flash, recovered) = damaged(geometry, Slot::B, 9, 0x01);
        assert_eq!(
            recovered,
            Err(CheckpointError::Corrupt(Corruption::SlotHeader))
        );
        cold_start_recovers(flash);
    }
}

#[test]
fn a_damaged_commit_marker_reads_as_an_interrupted_write() {
    for geometry in geometries() {
        // The residual the specification states: the record before it is loaded.
        let newest = records(&both_slots(geometry), Slot::B)[3].0;
        let (_, recovered) = damaged(geometry, Slot::B, newest + 64, 0x02);
        let (record, data) = recovered.unwrap().unwrap();
        assert_eq!((record.sequence, data), (5, payload(5, 1900)));
    }
}

#[test]
fn a_program_that_does_not_take_is_never_committed() {
    for geometry in geometries() {
        // The next record of `two_records` starts at 372: 16 + 168 + 188.
        let next = 16 + 168 + 188;
        let data = payload(3, 100);
        let previous = recover(&mut two_records(geometry)).unwrap();
        for (offset, wanted) in [(next + 68 + 5, data[5]), (next + 8, 3)] {
            let mut flash = two_records(geometry);
            let mask = 1 << (!wanted).trailing_zeros();
            flash.stick(Slot::A, offset, mask);
            let mut log = CheckpointLog::open(&mut flash).unwrap();
            assert_eq!(log.append(identity(3), &data), Err(CheckpointError::Verify));
            flash.unstick();
            assert_eq!(recover(&mut flash).unwrap(), previous, "at {offset}");
            let checkpoint = write(&mut flash, 4, 100);
            assert_eq!(recover(&mut flash).unwrap(), Some(checkpoint));
        }
    }
}

#[test]
fn the_log_never_programs_flash_that_is_not_erased() {
    for geometry in geometries() {
        let next = 16 + 168 + 188;
        // Stray bits where the next record would go: the write goes to the other slot.
        let mut flash = two_records(geometry);
        flash.flip(Slot::A, next + 70, 0xFF);
        let checkpoint = write(&mut flash, 3, 100);
        assert_eq!(flash.slot(Slot::A)[next as usize + 70], 0x00);
        assert_eq!(records(&flash, Slot::B).len(), 1);
        assert_eq!(recover(&mut flash).unwrap(), Some(checkpoint));

        // Stray bits that appear in the payload's place after the header is programmed stop the
        // write before it programs over them.
        let mut flash = two_records(geometry);
        let previous = recover(&mut flash).unwrap();
        let stray = next + 68 + 40;
        flash.disturb_after(1, Slot::A, stray);
        let mut log = CheckpointLog::open(&mut flash).unwrap();
        let data = payload(3, 100);
        assert_eq!(
            log.append(identity(3), &data),
            Err(CheckpointError::NotErased)
        );
        assert_eq!(flash.slot(Slot::A)[stray as usize], 0x00);
        assert_eq!(recover(&mut flash).unwrap(), previous);
        let checkpoint = write(&mut flash, 4, 100);
        assert_eq!(recover(&mut flash).unwrap(), Some(checkpoint));
    }
}

#[test]
fn a_write_runs_as_single_flash_operations() {
    for geometry in geometries() {
        let units = geometry.erase_units() as usize;
        let data = payload(1, 10);
        for (budget, programs) in [(1, 4 + 16 + 2 + 1 + 1), (usize::MAX, 5)] {
            let mut flash = SimFlash::new(geometry);
            flash.record_operations();
            let mut log = CheckpointLog::open(&mut flash).unwrap();
            log.begin(identity(1), &data).unwrap();
            let mut steps = 0;
            let record = loop {
                steps += 1;
                if let Progress::Committed(record) = log.step(&data, budget).unwrap() {
                    break record;
                }
            };
            let ops = flash.operations();
            assert_eq!(steps, units + programs, "one flash operation per step");
            assert_eq!(ops.len(), steps);
            assert!(ops[..units].iter().all(|op| op.erase));
            if budget == 1 {
                assert!(ops[units..].iter().all(|op| op.len == 4));
            }
            assert_eq!(recover(&mut flash).unwrap(), Some((record, data.clone())));
        }
    }
}

#[test]
fn a_write_refuses_a_changed_payload_and_overlapping_writes() {
    for geometry in geometries() {
        let mut flash = two_records(geometry);
        let previous = recover(&mut flash).unwrap();
        let data = payload(3, 100);
        let mut log = CheckpointLog::open(&mut flash).unwrap();
        assert_eq!(log.step(&data, 64), Err(CheckpointError::NoWrite));
        log.begin(identity(3), &data).unwrap();
        assert_eq!(
            log.begin(identity(3), &data),
            Err(CheckpointError::WriteInProgress)
        );
        // Another length.
        assert_eq!(
            log.step(&data[..99], 64),
            Err(CheckpointError::PayloadChanged)
        );
        assert_eq!(log.step(&data, 64), Err(CheckpointError::NoWrite));
        // The same length, other bytes: refused before the commit.
        log.begin(identity(3), &data).unwrap();
        let changed = payload(4, 100);
        let result = loop {
            match log.step(&changed, 64) {
                Ok(Progress::Pending) => {}
                other => break other,
            }
        };
        assert_eq!(result, Err(CheckpointError::PayloadChanged));
        assert_eq!(recover(&mut flash).unwrap(), previous);
        let checkpoint = write(&mut flash, 5, 100);
        assert_eq!(recover(&mut flash).unwrap(), Some(checkpoint));
    }
}

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

/// The system allocator, counting the allocations of a thread while it counts.
struct CountingAllocator;

// SAFETY: every call goes unchanged to the system allocator; the thread-local cells neither
// allocate nor panic.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = COUNTING.try_with(|counting| {
            if counting.get() {
                let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
            }
        });
        // SAFETY: the caller's layout is passed on unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the pointer and layout come from `alloc` above.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// The allocations `run` makes on this thread.
fn allocations(run: impl FnOnce()) -> u64 {
    ALLOCATIONS.with(|count| count.set(0));
    COUNTING.with(|counting| counting.set(true));
    run();
    COUNTING.with(|counting| counting.set(false));
    ALLOCATIONS.with(Cell::get)
}

#[test]
fn the_log_does_not_allocate() {
    for geometry in geometries() {
        let mut flash = both_slots(geometry);
        let payloads: Vec<Vec<u8>> = (0..40).map(|tag| payload(tag, 900)).collect();
        let mut buffer = vec![0; 16 * 1024];
        let count = allocations(|| {
            let mut log = CheckpointLog::open(&mut flash).unwrap();
            assert!(log.recover(&mut buffer).unwrap().is_some());
            for (tag, data) in payloads.iter().enumerate() {
                log.append(identity(tag as u8), data).unwrap();
            }
            log.begin(identity(40), &payloads[0]).unwrap();
            while log.step(&payloads[0], 64).unwrap() == Progress::Pending {}
            assert!(log.newest().unwrap().is_some());
            assert!(log.recover(&mut buffer).unwrap().is_some());
        });
        assert_eq!(count, 0, "the log allocated");
        assert!(flash.erases(Slot::A) > 0 && flash.erases(Slot::B) > 0);
    }
}

#[test]
fn checkpoint_failures_map_to_retain_errors() {
    for corruption in [
        Corruption::RecordHeader,
        Corruption::Payload,
        Corruption::SlotHeader,
    ] {
        assert!(matches!(
            RuntimeError::from(CheckpointError::Corrupt(corruption)),
            RuntimeError::RetainCorruption(_)
        ));
    }
    for error in [
        CheckpointError::Flash(FlashError::Read),
        CheckpointError::Flash(FlashError::Program),
        CheckpointError::Flash(FlashError::Erase),
        CheckpointError::NotErased,
        CheckpointError::Verify,
        CheckpointError::TooLarge { len: 1, max: 0 },
        CheckpointError::BufferTooSmall { needed: 1 },
        CheckpointError::WriteInProgress,
        CheckpointError::NoWrite,
        CheckpointError::PayloadChanged,
        CheckpointError::Exhausted,
        CheckpointError::Geometry,
    ] {
        assert!(
            matches!(RuntimeError::from(error), RuntimeError::RetainStore(_)),
            "{error}"
        );
    }
}
