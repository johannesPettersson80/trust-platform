//! The held reply of the transfer session (specification 34, PORT-05): a reply longer than the
//! frame is held in the parts it was written in, its head carries the SHA-256 of the whole reply,
//! `get` reads it back in ranges, and its parts are freed when its last byte is read, when it is
//! aborted and when a newer reply replaces it. The payloads and digests are those of `tests.rs`:
//! Python's hashlib and zlib gave the SHA-256 and CRC-32 values, and the offsets and lengths are
//! worked out by hand.

use alloc::string::ToString;
use alloc::vec::Vec;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::json;

use super::tests::{
    digest, pattern, ten_parts, CRC_9000_LAST, CRC_9000_MIDDLE, CRC_PART_0, DIGEST_9000,
    DIGEST_TEN_PARTS,
};
use super::{wire, ReplyParts, TransferError, TransferSession};

/// A reply of these `bytes` held in the parts of 4,096 bytes that a heap with room gives.
fn parts_of(bytes: &[u8]) -> ReplyParts {
    let mut parts = ReplyParts::new();
    parts.try_write(bytes).expect("a heap with room");
    parts
}

#[test]
fn a_reply_above_the_frame_is_fetched_in_parts_with_its_sha256() {
    // Spec 34 PORT-05 and design section 3: a reply of 9,000 bytes is longer than the 8,192-byte
    // frame. It is held in parts, its head names its length and the SHA-256 of the whole reply, which
    // is worked out over the parts, and get reads it in ranges of 4,096 bytes, the last one 808 bytes
    // long. Catches a reply cut to the frame, or a head whose digest belongs to other bytes, such as
    // the digest of the last part only.
    let reply = pattern(9_000);
    let mut session = TransferSession::new(1, 10_000, 16_384);
    let head = session
        .hold_reply(parts_of(&reply))
        .expect("a 9,000-byte reply fits a 16,384-byte buffer");
    assert_eq!(
        wire::reply_head(&head),
        json!({
            "reply_transfer": "0000000100000001",
            "bytes": 9000,
            "sha256": DIGEST_9000,
            "part_bytes": 4096
        })
    );
    let first = session
        .get(head.transfer, 0, 4_096)
        .expect("the first part reads");
    assert_eq!(first.offset, 0);
    assert_eq!(first.data, &reply[..4_096]);
    assert_eq!(first.crc32, CRC_PART_0);
    let middle = session
        .get(head.transfer, 4_096, 4_096)
        .expect("the middle part reads");
    assert_eq!(middle.data, &reply[4_096..8_192]);
    assert_eq!(middle.crc32, CRC_9000_MIDDLE);
    let last = session
        .get(head.transfer, 8_192, 4_096)
        .expect("the last part reads");
    assert_eq!(last.data.len(), 808);
    assert_eq!(last.data, &reply[8_192..]);
    assert_eq!(last.crc32, CRC_9000_LAST);
}

#[test]
fn a_reply_of_ten_parts_has_the_digest_of_its_bytes_and_reads_back_whole() {
    // Spec 34 PORT-05: the head's SHA-256 is that of the whole reply, however many parts it is held
    // in. `ten_parts()` is nine full parts and a last one of 808 bytes, 37,672 bytes, whose digest
    // Python's hashlib gives. Reading it from offset 0 in ranges of 4,096 bytes gives the bytes back.
    // Catches a digest worked out over some of the parts, or over the parts and their padding, and a
    // part read from the wrong place.
    let payload = ten_parts();
    let mut session = TransferSession::new(1, 10_000, 65_536);
    let head = session
        .hold_reply(parts_of(&payload))
        .expect("a 37,672-byte reply fits a 65,536-byte buffer");
    assert_eq!(head.bytes, 37_672);
    assert_eq!(head.sha256, digest(DIGEST_TEN_PARTS));
    let mut read = Vec::new();
    while read.len() < payload.len() {
        let offset = u64::try_from(read.len()).expect("an offset fits a u64");
        let part = session
            .get(head.transfer, offset, 4_096)
            .expect("each range of the reply reads");
        assert_eq!(part.offset, read.len());
        read.extend_from_slice(&part.data);
    }
    assert_eq!(read, payload);
}

#[test]
fn a_get_across_two_parts_answers_the_same_bytes() {
    // Spec 34 PORT-05: `offset` is anywhere up to the reply's length, so a range may cross the end of
    // a part. Bytes 4,000 to 4,199 of `pattern(9000)` are the last 96 bytes of the first part and the
    // first 104 bytes of the second; their CRC-32 (Python's zlib) is 0x670be4a2 and their base64 is
    // 268 characters. The range at 8,000 to 8,399 crosses the second boundary. Catches a get that
    // stops at the end of the part it starts in, answering 96 bytes, and one that restarts the second
    // part at the wrong byte.
    let reply = pattern(9_000);
    let mut session = TransferSession::new(1, 10_000, 16_384);
    let head = session
        .hold_reply(parts_of(&reply))
        .expect("a 9,000-byte reply is held");
    let across = session
        .get(head.transfer, 4_000, 200)
        .expect("a range across two parts reads");
    assert_eq!(across.offset, 4_000);
    assert_eq!(across.data.len(), 200);
    assert_eq!(across.data, &reply[4_000..4_200]);
    assert_eq!(across.crc32, 0x670b_e4a2);
    let answer = wire::get_reply(&across);
    assert_eq!(answer["offset"], json!(4_000));
    assert_eq!(answer["crc32"], json!("670be4a2"));
    assert_eq!(
        STANDARD
            .decode(answer["data"].as_str().expect("the data is text"))
            .expect("the data is base64"),
        &reply[4_000..4_200]
    );
    let second = session
        .get(head.transfer, 8_000, 400)
        .expect("a range across the second boundary reads");
    assert_eq!(second.data, &reply[8_000..8_400]);
    assert_eq!(second.crc32, 0xdc18_21d2);
}

#[test]
fn a_get_frees_the_reply_only_when_it_reads_its_last_byte() {
    // Spec 34 PORT-05, held reply freed: the parts are freed when a get has read the reply's last
    // byte, and a get that reads before it, once or again, leaves the reply held. A get at the end of
    // the reply reads no byte, and a get of no bytes reads none, so neither frees it. The last range
    // is answered whole and then the reply is gone, whether the range starts a part, starts in the
    // middle of one, or crosses two parts. Catches a reply freed by the first get, one freed by a get
    // that reads nothing, and a last range that is cut because its reply was freed first.
    let reply = pattern(9_000);
    let mut session = TransferSession::new(1, 10_000, 16_384);
    let head = session
        .hold_reply(parts_of(&reply))
        .expect("a 9,000-byte reply is held");
    for _ in 0..2 {
        let middle = session
            .get(head.transfer, 4_096, 4_096)
            .expect("a middle part reads again");
        assert_eq!(middle.data, &reply[4_096..8_192]);
    }
    let nothing: &[u8] = &[];
    assert_eq!(
        session
            .get(head.transfer, 9_000, 4_096)
            .expect("the end of the reply is a valid offset")
            .data,
        nothing
    );
    assert_eq!(
        session
            .get(head.transfer, 0, 0)
            .expect("a range of no bytes reads")
            .data,
        nothing
    );
    let up_to_the_last_but_one = session
        .get(head.transfer, 8_192, 807)
        .expect("a range that stops one byte short reads");
    assert_eq!(up_to_the_last_but_one.data, &reply[8_192..8_999]);

    let last = session
        .get(head.transfer, 8_100, 4_096)
        .expect("a range across the boundary that reaches the last byte reads");
    assert_eq!(last.data, &reply[8_100..]);
    assert_eq!(last.crc32, 0xbccd_504e);
    assert_eq!(
        session.get(head.transfer, 0, 1),
        Err(TransferError::Unknown),
        "the reply is freed once its last byte is read"
    );
    assert_eq!(session.abort(head.transfer), Err(TransferError::Unknown));
}

#[test]
fn a_newer_held_reply_replaces_the_older_and_abort_frees_a_held_reply() {
    // Spec 34 PORT-05, held reply freed: a newer held reply replaces the older one, whose id is then
    // unknown, and abort frees the reply it names. Catches two replies held at once, which a chip's
    // heap cannot afford, and an abort that leaves the reply readable.
    let mut session = TransferSession::new(1, 10_000, 16_384);
    let older = session
        .hold_reply(parts_of(&pattern(9_000)))
        .expect("the first reply is held");
    let newer = session
        .hold_reply(parts_of(&pattern(5_000)))
        .expect("the second reply is held");
    assert_ne!(older.transfer, newer.transfer);
    assert_eq!(
        session.get(older.transfer, 0, 1),
        Err(TransferError::Unknown)
    );
    assert_eq!(
        session
            .get(newer.transfer, 0, 1)
            .expect("the newer reply reads")
            .data
            .len(),
        1
    );
    assert_eq!(session.abort(newer.transfer), Ok(()));
    assert_eq!(
        session.get(newer.transfer, 0, 1),
        Err(TransferError::Unknown)
    );
    assert_eq!(session.abort(newer.transfer), Err(TransferError::Unknown));
}

#[test]
fn a_reply_above_the_buffer_is_refused_not_cut() {
    // Spec 34 PORT-05: a reply above the buffer's capacity is refused with its limit, never cut,
    // and the reply held before stays as it was. Catches a reply truncated to the capacity and
    // held, or a refusal that drops the held reply.
    let mut session = TransferSession::new(1, 10_000, 100);
    let held = session
        .hold_reply(parts_of(b"0123456789"))
        .expect("a 10-byte reply is held");
    assert_eq!(
        session.hold_reply(parts_of(&[b'x'; 101])),
        Err(TransferError::ReplyTooLong { capacity: 100 })
    );
    assert_eq!(
        TransferError::ReplyTooLong { capacity: 100 }.to_string(),
        "reply longer than 100 bytes"
    );
    let read = session
        .get(held.transfer, 0, 4_096)
        .expect("the held reply still reads");
    assert_eq!(read.data, b"0123456789".as_slice());
}

#[test]
fn a_reply_of_exactly_the_buffer_capacity_is_held_whole() {
    // Spec 34 PORT-05: the capacity itself is accepted. Catches a limit off by one.
    let mut session = TransferSession::new(1, 10_000, 100);
    let head = session
        .hold_reply(parts_of(&[b'x'; 100]))
        .expect("a reply of exactly the capacity is held");
    assert_eq!(head.bytes, 100);
    let read = session
        .get(head.transfer, 0, 4_096)
        .expect("the held reply reads");
    assert_eq!(read.data.len(), 100);
}

#[test]
fn get_refuses_a_length_above_a_part_and_an_offset_past_the_reply() {
    // Spec 34 PORT-05: len is at most 4,096, and the reply ends at its length. Catches a longer
    // part served, or a read past the end treated as data.
    let mut session = TransferSession::new(1, 10_000, 16_384);
    let head = session
        .hold_reply(parts_of(&pattern(9_000)))
        .expect("a 9,000-byte reply is held");
    assert_eq!(
        session.get(head.transfer, 0, 4_097),
        Err(TransferError::GetLength { len: 4_097 })
    );
    assert_eq!(
        session.get(head.transfer, 9_001, 1),
        Err(TransferError::GetOffset {
            offset: 9_001,
            bytes: 9_000
        })
    );
    // At the very end there is nothing left to read: an empty part, not an error.
    let end = session
        .get(head.transfer, 9_000, 4_096)
        .expect("the end of the reply is a valid offset");
    assert_eq!(end.offset, 9_000);
    let nothing: &[u8] = &[];
    assert_eq!(end.data, nothing);
}
