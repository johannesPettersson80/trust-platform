//! The contract tests of the transfer session and of its wire shapes (specification 34, PORT-05).
//! Payloads are built from a formula in the test. Their SHA-256 digests and CRC-32 values come
//! from an independent implementation (Python's hashlib and zlib), not from the code under test.
//! Offsets, lengths, messages and JSON shapes are worked out from the specification and design
//! section 3. The base64 is checked against the crate the session decodes with, and against the
//! vectors of RFC 4648.

use alloc::borrow::Cow;
use alloc::collections::TryReserveError;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::json;

use super::{
    wire, Complete, Received, ReplyPart, State, Status, TransferError, TransferId, TransferSession,
};

/// SHA-256 of `abc` (FIPS 180-2, appendix B.1).
pub(super) const DIGEST_ABC: &str =
    "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
/// SHA-256 of no bytes (FIPS 180-2).
const DIGEST_EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
/// SHA-256 of `pattern(5000)`.
pub(super) const DIGEST_5000: &str =
    "69dbee893909fa17d1be397e0c07691336fe42049c29d403467d3d4a1fc3b5a1";
/// SHA-256 of `pattern(9000)`.
pub(super) const DIGEST_9000: &str =
    "4b81efbd205e7fb4e42bc0d72d9d7413642298735289d35a74c1755883bcc45c";
/// SHA-256 of `ten_parts()`.
pub(super) const DIGEST_TEN_PARTS: &str =
    "33682e67392cf2b0488346bdd1734e9c6193de69d1b5a0667c3f22205f56ddec";

/// CRC-32 of `pattern(4096)`, which is also the first part of `pattern(5000)`.
pub(super) const CRC_PART_0: u32 = 0xd465_f907;
/// CRC-32 of `pattern(5000)[4096..]`, the 904-byte last part of a 5,000-byte upload.
pub(super) const CRC_5000_LAST: u32 = 0x6508_4d9b;
/// CRC-32 of `pattern(5000)[..4000]`.
const CRC_4000: u32 = 0xe0e4_d2de;
/// CRC-32 of `pattern(9000)[4096..8192]`.
pub(super) const CRC_9000_MIDDLE: u32 = 0x891e_3520;
/// CRC-32 of `pattern(9000)[8192..]`, the 808-byte last part of a 9,000-byte reply.
pub(super) const CRC_9000_LAST: u32 = 0xd588_1d0a;
/// CRC-32 of `pattern(808)`, the last part of `ten_parts()`.
const CRC_TAIL_808: u32 = 0x6693_c423;
/// CRC-32 of 4,096 bytes of `0xee`.
pub(super) const CRC_OTHER_BYTES: u32 = 0xd036_f404;
/// CRC-32 of `abc`.
const CRC_ABC: u32 = 0x3524_41c2;
/// CRC-32 of `abcd`.
const CRC_ABCD: u32 = 0xed82_cd11;
/// CRC-32 of `abd`.
const CRC_ABD: u32 = 0xab40_d461;

/// The payload `0, 1, ..., 250, 0, 1, ...` of `len` bytes.
pub(super) fn pattern(len: usize) -> Vec<u8> {
    (0..len)
        .map(|index| u8::try_from(index % 251).expect("below 251"))
        .collect()
}

/// Nine full parts of `pattern(4096)`, then `pattern(808)`: ten parts in all, the last one short.
pub(super) fn ten_parts() -> Vec<u8> {
    let mut payload = Vec::new();
    for _ in 0..9 {
        payload.extend(pattern(4_096));
    }
    payload.extend(pattern(808));
    payload
}

/// The 32 bytes that the 64 hexadecimal digits of `text` spell.
pub(super) fn digest(text: &str) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks(2)) {
        let pair = core::str::from_utf8(pair).expect("hexadecimal digits are ASCII");
        *byte = u8::from_str_radix(pair, 16).expect("hexadecimal digits");
    }
    bytes
}

/// The standard base64 of `bytes`: the `data` of a part.
pub(super) fn encoded(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// A reservation the heap refuses, as `try_reserve_exact` refuses a size it cannot hold.
pub(super) fn heap_refuses(_len: usize) -> Result<Vec<u8>, TryReserveError> {
    Err(Vec::<u8>::new()
        .try_reserve_exact(usize::MAX)
        .expect_err("usize::MAX bytes never fit"))
}

/// A heap that refuses the part buffer of 4,096 bytes and holds every other buffer.
fn refuses_the_part_buffer(len: usize) -> Result<Vec<u8>, TryReserveError> {
    if len == 4_096 {
        heap_refuses(len)
    } else {
        Ok(vec![0_u8; len])
    }
}

#[test]
fn part_at_another_offset_is_refused_and_nothing_is_stored() {
    // Spec 34 PORT-05: a part is accepted only at the next offset. Catches a part at offset 4096
    // stored before offset 0, which writes the image out of order.
    let data = pattern(5000);
    let first = encoded(&data[..4096]);
    let last = encoded(&data[4096..]);
    let mut session = TransferSession::new(1, 10_000, 1_000);
    let id = session
        .begin(5000, digest(DIGEST_5000))
        .expect("a 5,000-byte upload opens");
    assert_eq!(
        session.part(id, 4096, &last, CRC_5000_LAST),
        Err(TransferError::PartAtOffset {
            offset: 4096,
            next_offset: 0
        })
    );
    assert_eq!(
        session.status(id),
        Ok(Status {
            state: State::Receiving,
            next_offset: 0,
            bytes: 5000
        })
    );
    assert_eq!(
        session.part(id, 0, &first, CRC_PART_0),
        Ok(Received::Next(4096))
    );
    assert_eq!(
        session.part(id, 4096, &last, CRC_5000_LAST),
        Ok(Received::Next(5000))
    );
    assert_eq!(session.take_complete(id), Ok(Complete::Memory(data)));
}

#[test]
fn repeated_part_with_the_same_bytes_is_acknowledged_again() {
    // Spec 34 PORT-05: a repeat of a received part with the same bytes is acknowledged again, so
    // a retry after a dropped reply is safe. Catches a duplicate refused, which fails the retry.
    let data = pattern(5000);
    let first = encoded(&data[..4096]);
    let last = encoded(&data[4096..]);
    let mut session = TransferSession::new(1, 10_000, 1_000);
    let id = session
        .begin(5000, digest(DIGEST_5000))
        .expect("a 5,000-byte upload opens");
    assert_eq!(
        session.part(id, 0, &first, CRC_PART_0),
        Ok(Received::Next(4096))
    );
    assert_eq!(
        session.part(id, 0, &first, CRC_PART_0),
        Ok(Received::Next(4096))
    );
    assert_eq!(
        session.part(id, 4096, &last, CRC_5000_LAST),
        Ok(Received::Next(5000))
    );
    assert_eq!(session.take_complete(id), Ok(Complete::Memory(data)));
}

#[test]
fn repeated_offset_with_other_bytes_is_refused() {
    // Spec 34 PORT-05: a repeat with other bytes is refused and nothing changes. Catches a received
    // part overwritten silently, which would show as other bytes in the completed upload.
    let data = pattern(5000);
    let first = encoded(&data[..4096]);
    let last = encoded(&data[4096..]);
    let other = encoded(&vec![0xee_u8; 4096]);
    let mut session = TransferSession::new(1, 10_000, 1_000);
    let id = session
        .begin(5000, digest(DIGEST_5000))
        .expect("a 5,000-byte upload opens");
    assert_eq!(
        session.part(id, 0, &first, CRC_PART_0),
        Ok(Received::Next(4096))
    );
    assert_eq!(
        session.part(id, 0, &other, CRC_OTHER_BYTES),
        Err(TransferError::PartOtherBytes { offset: 0 })
    );
    assert_eq!(
        session.status(id),
        Ok(Status {
            state: State::Receiving,
            next_offset: 4096,
            bytes: 5000
        })
    );
    assert_eq!(
        session.part(id, 4096, &last, CRC_5000_LAST),
        Ok(Received::Next(5000))
    );
    assert_eq!(session.take_complete(id), Ok(Complete::Memory(data)));
}

#[test]
fn part_with_a_wrong_crc_is_refused_and_next_offset_is_unchanged() {
    // Spec 34 PORT-05: a part whose CRC-32 does not match its bytes is refused. Catches corrupt
    // bytes kept, or the next offset moved by a refused part.
    let data = pattern(5000);
    let first = encoded(&data[..4096]);
    let mut session = TransferSession::new(1, 10_000, 1_000);
    let id = session
        .begin(5000, digest(DIGEST_5000))
        .expect("a 5,000-byte upload opens");
    // The CRC-32 of the first part with its last bit flipped.
    assert_eq!(
        session.part(id, 0, &first, 0xd465_f906),
        Err(TransferError::PartCrc { offset: 0 })
    );
    assert_eq!(
        session.status(id),
        Ok(Status {
            state: State::Receiving,
            next_offset: 0,
            bytes: 5000
        })
    );
    assert_eq!(
        session.part(id, 0, &first, CRC_PART_0),
        Ok(Received::Next(4096))
    );
}

#[test]
fn begin_above_the_maximum_allocates_nothing() {
    // Spec 34 PORT-05: begin refuses a size above the maximum before it allocates anything. A
    // 64 MiB + 1 request on a device with 4,096 bytes of room must reserve nothing. Catches an
    // allocation kept after the size check. A size no allocator can serve (u64::MAX) also catches
    // an allocation attempted before the check, which would abort the test.
    let mut session = TransferSession::new(1, 4_096, 1_000);
    assert_eq!(
        session.begin(67_108_865, digest(DIGEST_ABC)),
        Err(TransferError::TooLarge {
            bytes: 67_108_865,
            max: 4_096
        })
    );
    assert_eq!(session.buffer_capacity(), 0);
    assert_eq!(session.part_buffer_capacity(), 0);
    assert_eq!(
        session.begin(u64::MAX, digest(DIGEST_ABC)),
        Err(TransferError::TooLarge {
            bytes: u64::MAX,
            max: 4_096
        })
    );
    assert_eq!(session.buffer_capacity(), 0);
    assert_eq!(
        TransferError::TooLarge {
            bytes: 67_108_865,
            max: 4_096
        }
        .to_string(),
        "transfer too large: 67108865, the device takes at most 4096"
    );
}

#[test]
fn begin_at_the_maximum_is_accepted_and_one_byte_more_is_refused() {
    // Spec 34 PORT-05: the maximum itself is accepted. Catches a limit off by one in either
    // direction.
    let mut session = TransferSession::new(1, 4_096, 1_000);
    assert_eq!(
        session.begin(4_097, digest(DIGEST_ABC)),
        Err(TransferError::TooLarge {
            bytes: 4_097,
            max: 4_096
        })
    );
    let id = session
        .begin(4_096, digest(DIGEST_ABC))
        .expect("the maximum is accepted");
    assert_eq!(
        session.status(id),
        Ok(Status {
            state: State::Receiving,
            next_offset: 0,
            bytes: 4_096
        })
    );
}

#[test]
fn a_full_part_fits_the_frame() {
    // Spec 34 PORT-05 and design section 3: a full part of 4,096 bytes encodes to 4 x 1,366 = 5,464
    // base64 characters that end in "==" (4,096 = 3 x 1,365 + 1). The whole request must fit the
    // 8,192-byte frame. Catches a part size that the frame refuses.
    let part = pattern(4_096);
    let text = encoded(&part);
    assert_eq!(text.len(), 5_464);
    assert!(text.ends_with("=="));
    let request = json!({
        "transfer": "0000000100000001",
        "offset": 0,
        "data": text,
        "crc32": "d465f907",
    });
    let line = serde_json::to_string(&request).expect("a JSON object encodes");
    assert!(
        line.len() <= 8_192,
        "the request is {} bytes, the frame is 8192",
        line.len()
    );
    let read = wire::part_request(&request).expect("the full part reads back");
    assert_eq!(
        STANDARD
            .decode(read.encoded)
            .expect("the part's text decodes"),
        part
    );
    assert_eq!(read.crc32, CRC_PART_0);
}

#[test]
fn progress_survives_a_new_host_session() {
    // Spec 34 PORT-05: a new host session drops only the reply and any partly received request,
    // so the open upload keeps its parts. Catches an upload dropped with the host session.
    let data = pattern(5000);
    let first = encoded(&data[..4096]);
    let last = encoded(&data[4096..]);
    let mut session = TransferSession::new(1, 10_000, 1_000);
    let id = session
        .begin(5000, digest(DIGEST_5000))
        .expect("a 5,000-byte upload opens");
    assert_eq!(
        session.part(id, 0, &first, CRC_PART_0),
        Ok(Received::Next(4096))
    );
    // The next host session asks where the upload is and carries on from there.
    assert_eq!(
        session.status(id),
        Ok(Status {
            state: State::Receiving,
            next_offset: 4096,
            bytes: 5000
        })
    );
    assert_eq!(
        session.part(id, 4096, &last, CRC_5000_LAST),
        Ok(Received::Next(5000))
    );
    assert_eq!(session.take_complete(id), Ok(Complete::Memory(data)));
}

#[test]
fn ids_differ_after_a_boot() {
    // Spec 34 PORT-05: ids never repeat within a boot or across boots, and a reset loses the
    // upload, so an old id is unknown. Catches a new boot that counts from the old id, or that
    // still answers the old id.
    let mut before = TransferSession::new(1, 10_000, 1_000);
    let old = before
        .begin(3, digest(DIGEST_ABC))
        .expect("the first upload opens");
    assert_eq!(old.to_string(), "0000000100000001");
    let mut after = TransferSession::new(2, 10_000, 1_000);
    let new = after
        .begin(3, digest(DIGEST_ABC))
        .expect("the first upload of boot 2 opens");
    assert_eq!(new.to_string(), "0000000200000001");
    assert_ne!(old, new);
    assert_eq!(after.status(old), Err(TransferError::Unknown));
}

#[test]
fn abort_frees_the_buffers_and_its_id() {
    // Spec 34 PORT-05: abort frees the buffer and the id. Catches a buffer kept after the abort,
    // a later use still accepted, or an id handed out again.
    let mut session = TransferSession::new(1, 10_000, 1_000);
    let first = session
        .begin(5000, digest(DIGEST_5000))
        .expect("a 5,000-byte upload opens");
    assert_eq!(session.buffer_capacity(), 5000);
    assert_eq!(session.part_buffer_capacity(), 4_096);
    session.abort(first).expect("the open upload aborts");
    assert_eq!(session.buffer_capacity(), 0);
    assert_eq!(session.part_buffer_capacity(), 0);
    assert_eq!(session.status(first), Err(TransferError::Unknown));
    assert_eq!(session.abort(first), Err(TransferError::Unknown));
    let second = session
        .begin(5000, digest(DIGEST_5000))
        .expect("the next upload opens");
    assert_eq!(second.to_string(), "0000000100000002");
}

#[test]
fn begin_while_an_upload_is_open_is_refused_as_busy() {
    // Spec 34 PORT-05: one open transfer at a time. Catches a second upload that replaces the first.
    let mut session = TransferSession::new(1, 10_000, 1_000);
    let open = session
        .begin(5000, digest(DIGEST_5000))
        .expect("a 5,000-byte upload opens");
    assert_eq!(
        session.begin(3, digest(DIGEST_ABC)),
        Err(TransferError::Busy(open))
    );
    assert_eq!(
        TransferError::Busy(open).to_string(),
        "transfer busy: 0000000100000001"
    );
}

#[test]
fn a_container_with_the_wrong_sha256_is_dropped_when_its_last_byte_arrives() {
    // Spec 34 PORT-05 and design section 3: the SHA-256 of the container is checked against begin's.
    // Catches a container of sound parts but wrong bytes kept open or accepted as complete, and
    // buffers kept after the drop.
    let mut session = TransferSession::new(1, 10_000, 1_000);
    let id = session
        .begin(3, digest(DIGEST_ABC))
        .expect("a 3-byte upload opens");
    assert_eq!(
        session.part(id, 0, "YWJk", CRC_ABD),
        Err(TransferError::Sha256Mismatch)
    );
    assert_eq!(session.status(id), Err(TransferError::Unknown));
    assert_eq!(session.buffer_capacity(), 0);
    assert_eq!(session.part_buffer_capacity(), 0);
    let again = session
        .begin(3, digest(DIGEST_ABC))
        .expect("the slot is free after the drop");
    assert_eq!(again.to_string(), "0000000100000002");
}

#[test]
fn a_correct_container_completes_and_is_taken_once() {
    // Spec 34 PORT-05: complete means every byte received and the SHA-256 equal to begin's; the
    // bytes are taken once, and the buffers go with them. Catches a complete upload that stays
    // open, taken twice, or holding its part buffer after it is taken.
    let mut session = TransferSession::new(1, 10_000, 1_000);
    let id = session
        .begin(3, digest(DIGEST_ABC))
        .expect("a 3-byte upload opens");
    assert_eq!(session.part(id, 0, "YWJj", CRC_ABC), Ok(Received::Next(3)));
    assert_eq!(
        session.status(id),
        Ok(Status {
            state: State::Complete,
            next_offset: 3,
            bytes: 3
        })
    );
    assert_eq!(
        session.take_complete(id),
        Ok(Complete::Memory(b"abc".to_vec()))
    );
    assert_eq!(session.part_buffer_capacity(), 0);
    assert_eq!(session.status(id), Err(TransferError::Unknown));
}

#[test]
fn an_upload_is_taken_only_when_complete() {
    // Spec 34 PORT-05: the bytes are taken only when complete. Catches bytes handed out while
    // parts are still missing, and an upload that the refusal then loses.
    let data = pattern(5000);
    let first = encoded(&data[..4096]);
    let mut session = TransferSession::new(1, 10_000, 1_000);
    let id = session
        .begin(5000, digest(DIGEST_5000))
        .expect("a 5,000-byte upload opens");
    assert_eq!(
        session.part(id, 0, &first, CRC_PART_0),
        Ok(Received::Next(4096))
    );
    assert_eq!(
        session.take_complete(id),
        Err(TransferError::NotComplete {
            next_offset: 4096,
            bytes: 5000
        })
    );
    assert_eq!(
        TransferError::NotComplete {
            next_offset: 4096,
            bytes: 5000
        }
        .to_string(),
        "transfer is receiving: next offset 4096 of 5000 bytes"
    );
    assert_eq!(
        session.status(id),
        Ok(Status {
            state: State::Receiving,
            next_offset: 4096,
            bytes: 5000
        })
    );
}

#[test]
fn a_short_part_in_the_middle_is_refused_and_nothing_is_stored() {
    // Spec 34 PORT-05: parts are 4,096 bytes except the last. Catches a short part that moves
    // every later part off the grid of offsets.
    let data = pattern(5000);
    let first = encoded(&data[..4096]);
    let last = encoded(&data[4096..]);
    let mut session = TransferSession::new(1, 10_000, 1_000);
    let id = session
        .begin(5000, digest(DIGEST_5000))
        .expect("a 5,000-byte upload opens");
    assert_eq!(
        session.part(id, 0, &encoded(&data[..4000]), CRC_4000),
        Err(TransferError::PartLength {
            offset: 0,
            len: 4000,
            expected: 4096
        })
    );
    assert_eq!(
        session.part(id, 0, &first, CRC_PART_0),
        Ok(Received::Next(4096))
    );
    assert_eq!(
        session.part(id, 4096, &last, CRC_5000_LAST),
        Ok(Received::Next(5000))
    );
    assert_eq!(session.take_complete(id), Ok(Complete::Memory(data)));
}

#[test]
fn a_part_longer_than_the_rest_of_the_upload_is_refused() {
    // Spec 34 PORT-05: a part is as long as the bytes that remain, at most a part. Catches a part
    // that runs past the declared size.
    let mut session = TransferSession::new(1, 10_000, 1_000);
    let id = session
        .begin(3, digest(DIGEST_ABC))
        .expect("a 3-byte upload opens");
    assert_eq!(
        session.part(id, 0, "YWJjZA==", CRC_ABCD),
        Err(TransferError::PartLength {
            offset: 0,
            len: 4,
            expected: 3
        })
    );
    assert_eq!(session.part(id, 0, "YWJj", CRC_ABC), Ok(Received::Next(3)));
}

#[test]
fn a_part_longer_than_a_part_is_refused() {
    // Spec 34 PORT-05: a part is at most 4,096 bytes. Catches a 4,097-byte part decoded past the
    // part buffer, or accepted with the bytes of its end.
    let mut session = TransferSession::new(1, 10_000, 1_000);
    let id = session
        .begin(5_000, digest(DIGEST_5000))
        .expect("a 5,000-byte upload opens");
    // The text does not fit the part buffer, so the CRC-32 is never reached; zero stands in for it.
    assert_eq!(
        session.part(id, 0, &encoded(&pattern(4_097)), 0),
        Err(TransferError::PartTooLong { offset: 0 })
    );
    assert_eq!(
        TransferError::PartTooLong { offset: 0 }.to_string(),
        "part at offset 0 is longer than 4096 bytes"
    );
    assert_eq!(
        session.status(id),
        Ok(Status {
            state: State::Receiving,
            next_offset: 0,
            bytes: 5_000
        })
    );
}

#[test]
fn a_part_whose_data_is_not_canonical_base64_is_refused() {
    // Spec 34 PORT-05: a part's data is standard base64 with padding and canonical trailing bits.
    // Catches a part whose text is read leniently, or whose bytes are stored before the text is
    // checked: the upload stays as it was.
    for text in [
        "Zg", "Zg=", "Z===", "Zh==", "Zg==Zg==", "Zm9v!", "Zm=v", "====",
    ] {
        let mut session = TransferSession::new(1, 10_000, 1_000);
        let id = session
            .begin(3, digest(DIGEST_ABC))
            .expect("a 3-byte upload opens");
        assert_eq!(
            session.part(id, 0, text, CRC_ABC),
            Err(TransferError::PartData { offset: 0 }),
            "{text}"
        );
        assert_eq!(
            session.status(id),
            Ok(Status {
                state: State::Receiving,
                next_offset: 0,
                bytes: 3
            }),
            "{text}"
        );
    }
}

#[test]
fn an_empty_upload_is_complete_at_once_only_with_the_digest_of_no_bytes() {
    // Spec 34 PORT-05: complete means the SHA-256 equals begin's, and an empty upload has no part
    // to check it at. Catches an empty upload with a wrong digest accepted as complete.
    let mut session = TransferSession::new(1, 10_000, 1_000);
    assert_eq!(
        session.begin(0, digest(DIGEST_ABC)),
        Err(TransferError::Sha256Mismatch)
    );
    assert_eq!(session.buffer_capacity(), 0);
    assert_eq!(session.part_buffer_capacity(), 0);
    let id = session
        .begin(0, digest(DIGEST_EMPTY))
        .expect("the digest of no bytes is complete");
    assert_eq!(
        session.status(id),
        Ok(Status {
            state: State::Complete,
            next_offset: 0,
            bytes: 0
        })
    );
    assert_eq!(session.take_complete(id), Ok(Complete::Memory(Vec::new())));
    assert_eq!(session.part_buffer_capacity(), 0);
}

#[test]
fn an_id_that_names_no_open_transfer_is_unknown_to_every_operation() {
    // Spec 34 PORT-05: an id that names no open upload or held reply is unknown. Catches an
    // operation that accepts an id it did not give.
    let mut session = TransferSession::new(1, 10_000, 1_000);
    let stranger = TransferId::new(1, 9);
    assert_eq!(session.status(stranger), Err(TransferError::Unknown));
    assert_eq!(
        session.part(stranger, 0, "YWJj", CRC_ABC),
        Err(TransferError::Unknown)
    );
    assert_eq!(session.take_complete(stranger), Err(TransferError::Unknown));
    assert_eq!(session.abort(stranger), Err(TransferError::Unknown));
    assert_eq!(session.get(stranger, 0, 1), Err(TransferError::Unknown));
    assert_eq!(TransferError::Unknown.to_string(), "unknown transfer");
}

#[test]
fn parts_are_decoded_without_allocating() {
    // Spec 34 PORT-05, the transfer begin bullet: begin reserves a part buffer of 4,096 bytes, and
    // each part is decoded into it. Across ten parts, nine full and the last of 808 bytes, that
    // buffer keeps its capacity. Catches a part buffer replaced by one sized to the part in hand:
    // the last part would show 808 bytes.
    let payload = ten_parts();
    let mut session = TransferSession::new(1, 100_000, 1_000);
    let id = session
        .begin(37_672, digest(DIGEST_TEN_PARTS))
        .expect("a 37,672-byte upload opens");
    assert_eq!(session.part_buffer_capacity(), 4_096);
    for index in 0..10 {
        let start = index * 4_096;
        let end = (start + 4_096).min(payload.len());
        let crc = if index == 9 { CRC_TAIL_808 } else { CRC_PART_0 };
        let offset = u64::try_from(start).expect("an offset fits a u64");
        assert_eq!(
            session.part(id, offset, &encoded(&payload[start..end]), crc),
            Ok(Received::Next(end))
        );
        assert_eq!(session.part_buffer_capacity(), 4_096);
    }
    assert_eq!(session.take_complete(id), Ok(Complete::Memory(payload)));
}

#[test]
fn a_begin_the_heap_cannot_hold_is_refused_not_aborted() {
    // Spec 34 PORT-05, the transfer begin bullet: a reservation the device cannot make is refused
    // with `no room for a transfer of <bytes> bytes`, never aborted. Catches a begin that reserves
    // with an infallible `vec!`, which aborts the device when the heap cannot hold the upload, and
    // a begin that keeps the upload buffer when the part buffer is refused.
    let mut refused = TransferSession::with_reserve(1, 10_000, 1_000, heap_refuses);
    assert_eq!(
        refused.begin(5_000, digest(DIGEST_5000)),
        Err(TransferError::NoRoom { bytes: 5_000 })
    );
    assert_eq!(refused.buffer_capacity(), 0);
    let mut part_refused = TransferSession::with_reserve(1, 10_000, 1_000, refuses_the_part_buffer);
    assert_eq!(
        part_refused.begin(5_000, digest(DIGEST_5000)),
        Err(TransferError::NoRoom { bytes: 5_000 })
    );
    assert_eq!(part_refused.buffer_capacity(), 0);
    assert_eq!(part_refused.part_buffer_capacity(), 0);
    // On the real heap, 2^62 bytes cannot be reserved: the same refusal, not an abort.
    let mut real = TransferSession::new(1, usize::MAX, 1_000);
    assert_eq!(
        real.begin(1 << 62, digest(DIGEST_5000)),
        Err(TransferError::NoRoom { bytes: 1 << 62 })
    );
    assert_eq!(real.buffer_capacity(), 0);
    assert_eq!(
        TransferError::NoRoom { bytes: 5_000 }.to_string(),
        "no room for a transfer of 5000 bytes"
    );
}

#[test]
fn base64_of_the_rfc_4648_vectors_round_trips() {
    // RFC 4648, section 10: the vectors of the standard alphabet with padding, encoded and decoded
    // by the crate the session uses. Catches an encoder that drops the padding, or a decoder that
    // rejects it.
    for (plain, text) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(STANDARD.encode(plain.as_bytes()), text);
        assert_eq!(
            STANDARD.decode(text).expect("RFC 4648 text decodes"),
            plain.as_bytes()
        );
    }
}

#[test]
fn base64_text_that_is_not_canonical_is_refused() {
    // RFC 4648, section 3.5: an encoding with bits after its last byte is not canonical, and
    // padding belongs only at the end. The crate the session uses refuses each text here. Catches
    // a decoder that accepts two spellings of one part, or reads a partial group as data.
    for text in [
        "Zg", "Zg=", "Z===", "Zh==", "Zg==Zg==", "Zm9v!", "Zm=v", "====",
    ] {
        assert!(
            STANDARD.decode(text).is_err(),
            "{text} is not canonical base64"
        );
    }
}

#[test]
fn the_reply_shapes_are_those_of_design_section_3() {
    // Design section 3: begin replies the id, the part size and next offset 0; part replies the
    // next offset; status names the state; abort replies nothing. Catches a renamed member.
    let id = TransferId::new(1, 1);
    assert_eq!(
        wire::begin_reply(id),
        json!({"transfer": "0000000100000001", "part_bytes": 4096, "next_offset": 0})
    );
    assert_eq!(wire::part_reply(4096), json!({"next_offset": 4096}));
    assert_eq!(
        wire::status_reply(Status {
            state: State::Receiving,
            next_offset: 4096,
            bytes: 5000
        }),
        json!({"state": "receiving", "next_offset": 4096, "bytes": 5000})
    );
    assert_eq!(
        wire::status_reply(Status {
            state: State::Complete,
            next_offset: 5000,
            bytes: 5000
        }),
        json!({"state": "complete", "next_offset": 5000, "bytes": 5000})
    );
    assert_eq!(wire::abort_reply(), json!({}));
}

#[test]
fn a_get_reply_carries_its_offset_base64_and_crc32() {
    // Design section 3: get replies the offset, the base64 of the part and its CRC-32 as eight
    // hexadecimal digits. The base64 of "abc" is "YWJj". Catches a CRC written as a number.
    let part = ReplyPart {
        offset: 0,
        data: Cow::Borrowed(&b"abc"[..]),
        crc32: CRC_ABC,
    };
    assert_eq!(
        wire::get_reply(&part),
        json!({"offset": 0, "data": "YWJj", "crc32": "352441c2"})
    );
}

#[test]
fn a_begin_request_reads_its_size_and_digest() {
    // Design section 3: begin takes the size and the 64-digit SHA-256 of the container.
    let request = json!({"bytes": 5000, "sha256": DIGEST_5000});
    assert_eq!(
        wire::begin_request(&request),
        Ok(wire::BeginRequest {
            bytes: 5000,
            digest: digest(DIGEST_5000)
        })
    );
}

#[test]
fn a_part_request_with_a_seven_digit_crc32_is_refused() {
    // Design section 3: the CRC-32 is 8 hexadecimal digits. Catches a short CRC read as a value.
    let request = json!({
        "transfer": "0000000100000001",
        "offset": 0,
        "data": "YWJj",
        "crc32": "352441c"
    });
    let error = wire::part_request(&request).expect_err("seven digits are not a CRC-32");
    assert_eq!(
        error.to_string(),
        "invalid request: crc32 must be 8 hexadecimal digits"
    );
}

#[test]
fn a_transfer_id_of_fifteen_digits_is_refused() {
    // Spec 34 PORT-05: a transfer id is 16 hexadecimal digits. Catches a short id matched to an
    // open transfer by its prefix.
    let request = json!({"transfer": "000000010000001"});
    assert_eq!(
        wire::transfer_request(&request),
        Err(wire::WireError::Member {
            name: "transfer",
            expected: "16 hexadecimal digits"
        })
    );
}
