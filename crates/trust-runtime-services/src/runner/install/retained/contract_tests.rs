//! The length of the newest checkpoint's payload, which an install and an online change keep room
//! for (specification 34 §10.1 and §11.3), read from the header of the newest committed record of
//! the fake checkpoint flash. Every expected value is written out here from the specification.

use alloc::vec::Vec;

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::value::Value;

use super::newest_checkpoint_payload_len;
use crate::tests::program_store::CheckpointProbe;

#[test]
fn the_newest_checkpoints_payload_length_is_read_from_its_header() {
    // Specification 34 §10.1 and §11.3: the length that a change keeps room for is the payload of the
    // newest committed record, which its header gives. A log with no committed record has nothing to
    // read, so 0. A checkpoint of one DINT is a payload of the length the codec writes for it, and
    // reading the length programs and erases nothing. A damaged record header cannot say how long
    // the payload is, and the answer is the error that a warm start meets. Wrong behaviour caught: a
    // length of 0 for a log that holds a checkpoint; the largest payload for the newest one; a
    // damaged header that reads as an empty log.
    assert_eq!(
        newest_checkpoint_payload_len(&mut CheckpointProbe::erased()),
        Ok(0)
    );

    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("@program/Plant/activations", Value::DInt(7));
    let mut payload = Vec::new();
    snapshot
        .encode_snapshot_payload_into(&mut payload)
        .expect("the snapshot encodes");
    let mut checkpoints = CheckpointProbe::with_checkpoint(&snapshot);
    assert_eq!(
        newest_checkpoint_payload_len(&mut checkpoints),
        Ok(payload.len())
    );
    assert!(payload.len() < 64, "a payload of one value is short");
    assert_eq!(checkpoints.writes(), 0, "the slots are only read");

    // The first record of slot A starts after the 16-byte slot header; byte 48 of its header is the
    // low byte of the payload length, so a flipped bit there fails the header's check while the
    // commit marker after the header still reads as written.
    checkpoints.damage(16 + 48);
    assert!(matches!(
        newest_checkpoint_payload_len(&mut checkpoints),
        Err(RuntimeError::RetainCorruption(_))
    ));
}
