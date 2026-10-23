//! The contract tests of an upload written into a program store (specification 34 §5.1, PORT-05):
//! the session keeps only the container's header and its part buffer, a part waits for the platform
//! to write it before it counts, the SHA-256 is worked out part by part, and a repeat is compared
//! with what the store and the session hold. Payloads, digests and CRC-32 values are those of the
//! session's other tests, which come from an independent implementation (Python's hashlib and zlib).

use alloc::vec;

use super::tests::{
    digest, encoded, heap_refuses, pattern, CRC_5000_LAST, CRC_OTHER_BYTES, CRC_PART_0,
    DIGEST_5000, DIGEST_ABC,
};
use super::{
    Complete, Received, State, Status, StoredUpload, TransferError, TransferId, TransferSession,
    HEADER_BYTES,
};

/// A session whose upload of `pattern(5000)` is written into a store, with its id.
fn stored_5000(digest_text: &str) -> (TransferSession, TransferId) {
    let mut session = TransferSession::new(1, 3_129_344, 114_688);
    let id = session
        .begin_in_store(5_000, digest(digest_text))
        .expect("a 5,000-byte upload into the store opens");
    (session, id)
}

/// The two parts of `pattern(5000)`, written as the platform writes them.
fn write_both(session: &mut TransferSession, id: TransferId) -> Result<usize, TransferError> {
    let data = pattern(5_000);
    assert_eq!(
        session.part(id, 0, &encoded(&data[..4_096]), CRC_PART_0),
        Ok(Received::Write { offset: 0 })
    );
    assert_eq!(session.written(id), Ok(4_096));
    assert_eq!(
        session.part(id, 4_096, &encoded(&data[4_096..]), CRC_5000_LAST),
        Ok(Received::Write { offset: 4_096 })
    );
    session.written(id)
}

#[test]
fn an_upload_written_into_the_store_reserves_its_header_and_its_part_buffer_only() {
    // Spec 34 PORT-05, the transfer begin bullet, and §5.1: begin reserves only the part buffer and
    // the 60 bytes of a container's header. Catches a store upload that reserves the whole upload in
    // the heap, which would show 5,000 bytes.
    let (session, _) = stored_5000(DIGEST_5000);
    assert_eq!(session.buffer_capacity(), HEADER_BYTES);
    assert_eq!(HEADER_BYTES, 60);
    assert_eq!(session.part_buffer_capacity(), 4_096);
}

#[test]
fn a_part_counts_only_once_the_platform_has_written_it() {
    // Spec 34 §5.1 and PORT-05, the transfer part bullet: a part of an upload in the store is checked
    // in the pass that reads it and then written in steps; the next offset moves when the write is
    // done. The upload's first 60 bytes are the header the session keeps. Catches a next offset moved
    // before the write, which a failed write would leave pointing past bytes the slot does not hold.
    let (mut session, id) = stored_5000(DIGEST_5000);
    let data = pattern(5_000);
    assert_eq!(
        session.part(id, 0, &encoded(&data[..4_096]), CRC_PART_0),
        Ok(Received::Write { offset: 0 })
    );
    assert_eq!(session.part_bytes(id), Some(&data[..4_096]));
    assert_eq!(
        session.status(id),
        Ok(Status {
            state: State::Receiving,
            next_offset: 0,
            bytes: 5_000,
        })
    );
    assert_eq!(session.written(id), Ok(4_096));
    assert_eq!(session.part_bytes(id), None);
    assert_eq!(
        session.part(id, 4_096, &encoded(&data[4_096..]), CRC_5000_LAST),
        Ok(Received::Write { offset: 4_096 })
    );
    assert_eq!(session.written(id), Ok(5_000));
    assert_eq!(
        session.take_complete(id),
        Ok(Complete::Store(StoredUpload {
            transfer: id,
            bytes: 5_000,
            header: data[..60].to_vec(),
        }))
    );
}

#[test]
fn an_upload_in_the_store_whose_sha256_differs_is_dropped_at_its_last_part() {
    // Spec 34 PORT-05, the transfer completion bullet: the SHA-256 of an upload in the store is
    // worked out part by part and compared when the last part is written; a mismatch drops the
    // upload. The digest begin names is that of `abc`, not of `pattern(5000)`. Catches a digest
    // compared over the header alone, or never compared.
    let (mut session, id) = stored_5000(DIGEST_ABC);
    assert_eq!(
        write_both(&mut session, id),
        Err(TransferError::Sha256Mismatch)
    );
    assert_eq!(session.status(id), Err(TransferError::Unknown));
}

#[test]
fn a_repeat_is_compared_with_the_store_and_with_the_header_the_session_kept() {
    // Spec 34 PORT-05, the transfer part bullet: a repeat of a received part is compared with the
    // bytes the store and the session hold for it. The slot holds no header (its first 60 bytes stay
    // erased), so they are compared with the kept header. Catches a repeat compared with the slot's
    // erased header, which refuses a true repeat, and one never compared, which accepts other bytes.
    let (mut session, id) = stored_5000(DIGEST_5000);
    let data = pattern(5_000);
    assert_eq!(write_both(&mut session, id), Ok(5_000));
    let mut slot = vec![0xff_u8; HEADER_BYTES];
    slot.extend_from_slice(&data[HEADER_BYTES..4_096]);
    assert_eq!(
        session.part(id, 0, &encoded(&data[..4_096]), CRC_PART_0),
        Ok(Received::Repeat { offset: 0 })
    );
    assert_eq!(session.repeated(id, &slot), Ok(5_000));

    // The same part where the slot holds another byte after the header.
    let mut changed = slot.clone();
    changed[100] ^= 0x01;
    assert_eq!(
        session.part(id, 0, &encoded(&data[..4_096]), CRC_PART_0),
        Ok(Received::Repeat { offset: 0 })
    );
    assert_eq!(
        session.repeated(id, &changed),
        Err(TransferError::PartOtherBytes { offset: 0 })
    );

    // Other bytes in the header: 4,096 bytes of 0xee against the kept header.
    let other = vec![0xee_u8; 4_096];
    assert_eq!(
        session.part(id, 0, &encoded(&other), CRC_OTHER_BYTES),
        Ok(Received::Repeat { offset: 0 })
    );
    assert_eq!(
        session.repeated(id, &slot),
        Err(TransferError::PartOtherBytes { offset: 0 })
    );
    assert_eq!(
        session.status(id),
        Ok(Status {
            state: State::Complete,
            next_offset: 5_000,
            bytes: 5_000,
        })
    );
}

#[test]
fn a_refused_part_leaves_no_part_waiting_for_the_platform() {
    // Spec 34 PORT-05, the transfer part bullet: a refused part stores nothing. Catches a part whose
    // CRC-32 is wrong left in the part buffer as the part to write, which the platform would then
    // write and count.
    let (mut session, id) = stored_5000(DIGEST_5000);
    let data = pattern(5_000);
    assert_eq!(
        session.part(id, 0, &encoded(&data[..4_096]), CRC_PART_0 ^ 1),
        Err(TransferError::PartCrc { offset: 0 })
    );
    assert_eq!(session.part_bytes(id), None);
    assert_eq!(session.written(id), Err(TransferError::Unknown));
}

#[test]
fn a_start_ends_an_upload_in_the_store_and_not_one_in_memory() {
    // Spec 34 PORT-05, the transfer status and abort bullet: a `restart` that starts the resource ends
    // an upload written into the store, and its id is then unknown; an upload held in memory stays
    // open. Catches an end that drops either upload alike.
    let mut memory = TransferSession::new(1, 100_000, 1_000);
    let open = memory
        .begin(5_000, digest(DIGEST_5000))
        .expect("an upload in memory opens");
    assert_eq!(memory.end_store_upload(), None);
    assert!(memory.status(open).is_ok());

    let (mut session, id) = stored_5000(DIGEST_5000);
    assert_eq!(session.end_store_upload(), Some(id));
    assert_eq!(session.status(id), Err(TransferError::Unknown));
}

#[test]
fn a_begin_into_the_store_the_heap_cannot_hold_is_refused() {
    // Spec 34 PORT-05, the transfer begin bullet: a reservation the device cannot make is refused
    // with `no room for a transfer of <bytes> bytes`, never aborted, also for an upload into the
    // store. Catches a header reserved with an infallible allocation.
    let mut session = TransferSession::with_reserve(1, 3_129_344, 114_688, heap_refuses);
    assert_eq!(
        session.begin_in_store(5_000, digest(DIGEST_5000)),
        Err(TransferError::NoRoom { bytes: 5_000 })
    );
    assert_eq!(session.buffer_capacity(), 0);
}
