//! The client over a link with a frame (specification 34, PORT-05, the client's rules and "Reply
//! head and get"), against a scripted chip: the frame, text on the port, and a reply that the chip
//! holds whole. Every expected value is worked out from the specification and written as a
//! literal; the digests of the held reply were computed with `hashlib` and `zlib`, not with this
//! crate.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Value};

use crate::fake_chip::{self, reply_line, Answer};
use crate::{ControlClient, ControlError};

/// The frame of a chip's USB link (PORT-05).
const FRAME: usize = 8_192;

/// The id of the reply that the scripted chip holds.
const TRANSFER: &str = "0000000100000001";

/// The SHA-256 of [`held_text`].
const HELD_SHA256: &str = "95dbf65b53b3ec0be241f943b38d98c2355e6c40eedc6235936e84ca4e1a0d79";

/// The SHA-256 of "abc": a valid digest that is not the held reply's.
const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// The digits that fill the held reply.
fn samples() -> String {
    format!("{}0123456", "0123456789".repeat(993))
}

/// The reply that the chip holds: a response to request 1 of exactly 10,000 bytes, which is two
/// parts of 4,096 bytes and a third of 1,808.
fn held_text() -> String {
    let text = format!(
        r#"{{"id":1,"ok":true,"result":{{"samples":"{}"}},"audit_id":"audit-1"}}"#,
        samples()
    );
    assert_eq!(text.len(), 10_000);
    text
}

/// A chip that holds a reply longer than the frame: it answers `status` with the head of `text`
/// (with the SHA-256 `sha256`) and serves `transfer.get` and `transfer.abort`. The CRC-32 of the
/// part at `wrong_crc_at` is wrong, and the part at `short_at` lacks its last byte.
struct Holding {
    text: String,
    sha256: &'static str,
    wrong_crc_at: Option<usize>,
    short_at: Option<usize>,
}

impl Holding {
    fn of(text: String, sha256: &'static str) -> Self {
        Self {
            text,
            sha256,
            wrong_crc_at: None,
            short_at: None,
        }
    }

    fn script(self) -> impl FnMut(&Value) -> Answer + Send + 'static {
        move |request| {
            let id = request["id"].clone();
            match request["type"].as_str().expect("a request type") {
                "status" => reply_line(&json!({
                    "id": id,
                    "ok": true,
                    "result": {
                        "reply_transfer": TRANSFER,
                        "bytes": self.text.len(),
                        "sha256": self.sha256,
                        "part_bytes": 4096,
                    },
                    "audit_id": "audit-1",
                })),
                "transfer.get" => {
                    let params = &request["params"];
                    assert_eq!(params["transfer"], json!(TRANSFER));
                    let offset = usize::try_from(params["offset"].as_u64().expect("an offset"))
                        .expect("an offset that fits");
                    let len = usize::try_from(params["len"].as_u64().expect("a length"))
                        .expect("a length that fits");
                    let end = (offset + len).min(self.text.len());
                    let mut part = self.text.as_bytes()[offset..end].to_vec();
                    if self.short_at == Some(offset) {
                        part.pop();
                    }
                    let mut crc = crc32fast::hash(&part);
                    if self.wrong_crc_at == Some(offset) {
                        crc ^= 1;
                    }
                    reply_line(&json!({
                        "id": id,
                        "ok": true,
                        "result": {
                            "offset": offset,
                            "data": STANDARD.encode(part.as_slice()),
                            "crc32": format!("{crc:08x}"),
                        },
                    }))
                }
                "transfer.abort" => reply_line(&json!({"id": id, "ok": true, "result": {}})),
                other => panic!("the chip was asked for {other}"),
            }
        }
    }
}

/// A `transfer.get` request as the client must send it.
fn get(id: u64, offset: u64, len: u64) -> Value {
    json!({
        "id": id,
        "type": "transfer.get",
        "params": {"transfer": TRANSFER, "offset": offset, "len": len},
    })
}

/// A `transfer.abort` request as the client must send it.
fn abort(id: u64) -> Value {
    json!({"id": id, "type": "transfer.abort", "params": {"transfer": TRANSFER}})
}

/// A chip that sends `chunks` for every request.
fn saying(chunks: Vec<Vec<u8>>) -> impl FnMut(&Value) -> Answer + Send + 'static {
    move |_| chunks.clone()
}

/// A chip that answers every request with a successful, empty result of its id.
fn echoing(request: &Value) -> Answer {
    reply_line(&json!({"id": request["id"], "ok": true, "result": {}}))
}

/// What the error says, for an error of the kind `Malformed`.
fn malformed(error: ControlError) -> String {
    match error {
        ControlError::Malformed(text) => text,
        other => panic!("expected a malformed answer, got {other:?}"),
    }
}

#[test]
fn a_request_longer_than_the_frame_is_refused_before_it_is_sent() {
    // Specification 34 PORT-05, "The client: the frame": a request of 8,192 bytes without its line
    // end is written, one of 8,193 is refused before any byte is written, with the chip's own text.
    // A link without a frame writes both. Wrong behaviour: an off-by-one at the bound (8,191 or
    // 8,193 allowed), the request written and cut by the chip (which would answer once with
    // `invalid request` and id 0 and leave the client waiting for its own id), or the bound
    // applied to a socket as well.
    let overhead = serde_json::to_string(&json!({"id": 1, "type": "status", "params": ""}))
        .expect("a request serializes")
        .len();
    let request_of = |length: usize| Some(Value::String("x".repeat(length - overhead)));

    let (stream, watch) = fake_chip::chip(Some(FRAME), echoing);
    let mut client = ControlClient::from_stream(stream, None);
    client
        .request("status", request_of(8_192))
        .expect("a request of exactly the frame is sent");
    assert_eq!(
        watch.written_bytes(),
        8_192 + 1,
        "the request and its line feed"
    );

    let (stream, watch) = fake_chip::chip(Some(FRAME), echoing);
    let mut client = ControlClient::from_stream(stream, None);
    assert_eq!(
        client.request("status", request_of(8_193)),
        Err(ControlError::Refused {
            error: "invalid request: request longer than 8192 bytes".into(),
            code: None,
        })
    );
    assert_eq!(watch.written_bytes(), 0, "nothing is written");
    client
        .request("status", None)
        .expect("the client is usable after a refusal");
    assert_eq!(
        watch.requests(),
        vec![json!({"id": 1, "type": "status"})],
        "the refused request used no id"
    );

    let (stream, watch) = fake_chip::chip(None, echoing);
    let mut client = ControlClient::from_stream(stream, None);
    client
        .request("status", request_of(8_193))
        .expect("a link without a frame has no such bound");
    assert_eq!(watch.written_bytes(), 8_193 + 1);
}

#[test]
fn a_reply_longer_than_the_frame_is_read_whole_with_transfer_get() {
    // Specification 34 PORT-05, "Reply head and get" and "The client: a reply longer than the
    // frame": a reply head is read with `transfer.get` at offsets 0, 4,096 and 8,192, of 4,096,
    // 4,096 and the remaining 1,808 bytes (10,000 = 2 x 4,096 + 1,808), each with the head's
    // transfer id and the next request id, and the whole reply is the answer. Wrong behaviour: the
    // head is handed on as the result, the parts are of another size or start at another offset,
    // the transfer id is not the head's, the abort is sent although the reply was read, or the
    // reply is cut.
    let (stream, watch) =
        fake_chip::chip(Some(FRAME), Holding::of(held_text(), HELD_SHA256).script());
    let mut client = ControlClient::from_stream(stream, None);

    let result = client
        .request("status", None)
        .expect("the held reply is read whole");

    assert_eq!(result, json!({"samples": samples()}));
    assert_eq!(
        watch.requests(),
        vec![
            json!({"id": 1, "type": "status"}),
            get(2, 0, 4_096),
            get(3, 4_096, 4_096),
            get(4, 8_192, 1_808),
        ]
    );
}

#[test]
fn the_whole_held_reply_is_handed_on_as_if_it_had_come_in_one_line() {
    // Specification 34 PORT-05: the whole reply is the answer, "handed on as if it had come in one
    // line": its audit record id is there too. Wrong behaviour: only the result is kept, or the
    // head's members are mixed into the reply.
    let (stream, _) = fake_chip::chip(Some(FRAME), Holding::of(held_text(), HELD_SHA256).script());
    let mut client = ControlClient::from_stream(stream, None);

    let response = client
        .exchange("status", None)
        .expect("the held reply is read whole");

    assert_eq!(
        response,
        json!({
            "id": 1,
            "ok": true,
            "result": {"samples": samples()},
            "audit_id": "audit-1",
        })
    );
}

#[test]
fn a_reply_whose_sha256_does_not_match_is_an_error() {
    // Specification 34 PORT-05, "The client: a reply longer than the frame": a reply whose SHA-256
    // is not the head's is never handed on, and the client frees the chip's reply buffer with
    // `transfer.abort`. The parts here are good (their CRC-32 match); only the digest of the whole
    // differs. Wrong behaviour: the reply is returned although its digest differs (an engineer
    // would read wrong values), the digest is checked against the base64 text or the parts, or no
    // abort is sent.
    let (stream, watch) =
        fake_chip::chip(Some(FRAME), Holding::of(held_text(), ABC_SHA256).script());
    let mut client = ControlClient::from_stream(stream, None);

    let error = client
        .request("status", None)
        .expect_err("a reply with another SHA-256 is an error");

    assert_eq!(
        malformed(error),
        "the answer to status does not match the SHA-256 of its head"
    );
    assert_eq!(
        watch.requests(),
        vec![
            json!({"id": 1, "type": "status"}),
            get(2, 0, 4_096),
            get(3, 4_096, 4_096),
            get(4, 8_192, 1_808),
            abort(5),
        ]
    );
}

#[test]
fn a_part_with_a_wrong_crc32_is_an_error() {
    // Specification 34 PORT-05: each part's `crc32` is the CRC-32 of its bytes. The first part
    // comes with a CRC-32 that differs in one bit: the client reports it, reads no further part,
    // and frees the reply buffer. Wrong behaviour: the CRC-32 is not checked, the whole reply is
    // read first, or no abort is sent.
    let mut chip = Holding::of(held_text(), HELD_SHA256);
    chip.wrong_crc_at = Some(0);
    let (stream, watch) = fake_chip::chip(Some(FRAME), chip.script());
    let mut client = ControlClient::from_stream(stream, None);

    let error = client
        .request("status", None)
        .expect_err("a part with a wrong CRC-32 is an error");

    assert_eq!(
        malformed(error),
        "the part of the reply at offset 0 has the wrong crc32"
    );
    assert_eq!(
        watch.requests(),
        vec![
            json!({"id": 1, "type": "status"}),
            get(2, 0, 4_096),
            abort(3)
        ]
    );
}

#[test]
fn a_part_that_is_short_is_an_error() {
    // Specification 34 PORT-05: a part has exactly the bytes asked for (the reply ends at its
    // head's `bytes`). The second part lacks its last byte, with a CRC-32 that fits what it
    // carries. Wrong behaviour: the client takes the short part and reads on, so the reply it
    // hands on is shifted or cut.
    let mut chip = Holding::of(held_text(), HELD_SHA256);
    chip.short_at = Some(4_096);
    let (stream, watch) = fake_chip::chip(Some(FRAME), chip.script());
    let mut client = ControlClient::from_stream(stream, None);

    let error = client
        .request("status", None)
        .expect_err("a short part is an error");

    assert_eq!(
        malformed(error),
        "the part of the reply at offset 4096 has 4095 bytes, not the 4096 that were asked for"
    );
    assert_eq!(
        watch.requests().last(),
        Some(&abort(4)),
        "the reply buffer is freed"
    );
}

#[test]
fn a_held_reply_above_the_response_limit_is_refused_and_freed() {
    // Specification 34 PORT-05: a head whose `bytes` are above the caller's limit for a reply line
    // is a failure; the line the reply would be is its bytes and a line end. A limit of 10,001 takes
    // the reply of 10,000 bytes, a limit of 10,000 does not, and then no part is read: the reply
    // buffer is freed at once. Wrong behaviour: the limit is ignored for a held reply (a runtime
    // could make the client read without a bound), or it is off by the line end.
    let (stream, _) = fake_chip::chip(Some(FRAME), Holding::of(held_text(), HELD_SHA256).script());
    ControlClient::from_stream(stream, None)
        .with_response_limit(10_001)
        .request("status", None)
        .expect("a held reply of 10,000 bytes is a line of 10,001");

    let (stream, watch) =
        fake_chip::chip(Some(FRAME), Holding::of(held_text(), HELD_SHA256).script());
    let error = ControlClient::from_stream(stream, None)
        .with_response_limit(10_000)
        .request("status", None)
        .expect_err("a held reply above the limit is refused");

    assert_eq!(
        malformed(error),
        "the answer to status is longer than 10000 bytes"
    );
    assert_eq!(
        watch.requests(),
        vec![json!({"id": 1, "type": "status"}), abort(2)]
    );
}

#[test]
fn a_reply_head_that_is_not_well_formed_is_malformed() {
    // Specification 34 PORT-05: a reply head names its transfer with 16 hexadecimal digits. A head
    // without that is malformed, and the client sends nothing more: it cannot name the reply to
    // read or to free. Wrong behaviour: the client asks for a transfer with an invalid id, or
    // hands the head on as a result.
    let head = json!({
        "id": 1,
        "ok": true,
        "result": {"reply_transfer": "xyz", "bytes": 10_000, "sha256": HELD_SHA256},
    });
    let (stream, watch) = fake_chip::chip(Some(FRAME), saying(reply_line(&head)));
    let mut client = ControlClient::from_stream(stream, None);

    let error = client
        .request("status", None)
        .expect_err("a head without a transfer id is malformed");

    assert_eq!(
        malformed(error),
        "the reply head is malformed: reply_transfer is not 16 hexadecimal digits"
    );
    assert_eq!(watch.requests(), vec![json!({"id": 1, "type": "status"})]);
}

#[test]
fn text_on_the_port_before_the_reply_is_skipped() {
    // Specification 34 PORT-05, "The client: one request in flight": the first line that is a JSON
    // object is the reply; the client skips an empty line, the boot loader's line, the line-feed
    // repair, the stale tail of a dropped reply, the stale head of a dropped reply, and the text
    // before a reply on the same line. Wrong behaviour: the first line is taken as the reply and
    // fails to parse, so a request right after a reset, or after a dropped reply, fails.
    let text = [
        "\r\n",
        "ESP-ROM:esp32c6-20220919\r\n",
        "\n",
        "\"bytes\":10000,\"sha256\":\"ab\"}}\n",
        "{\"id\":9,\"ok\":true,\"result\":{\"sta\n",
        "I (31) boot: ready{\"id\":1,\"ok\":true,\"result\":{\"state\":\"running\"}}\n",
    ];
    let chunks = text.iter().map(|chunk| chunk.as_bytes().to_vec()).collect();
    let (stream, _) = fake_chip::chip(Some(FRAME), saying(chunks));
    let mut client = ControlClient::from_stream(stream, None);

    let result = client
        .request("status", None)
        .expect("the reply is found among the text");

    assert_eq!(result, json!({"state": "running"}));
}

#[test]
fn a_link_without_a_frame_does_not_skip_text() {
    // A socket carries no text but replies (specification 11 §6.8.1): a line that is not a JSON
    // object is malformed there, as it always was. Wrong behaviour: the rules of a chip's port
    // reach the sockets, and a runtime that answers garbage makes the client wait for its timeout.
    let (stream, _) = fake_chip::chip(None, saying(vec![b"ESP-ROM:esp32c6\n".to_vec()]));
    let mut client = ControlClient::from_stream(stream, None);

    let error = client
        .request("status", None)
        .expect_err("text is not a reply on a socket");

    assert!(matches!(error, ControlError::Malformed(_)), "{error:?}");
}

#[test]
fn a_port_that_only_prints_text_is_not_a_control_link() {
    // Specification 34 PORT-05: a client that has skipped 65,536 bytes of text without finding a
    // reply reports that the port does not speak the control protocol. 64 lines of 1,024 bytes are
    // 65,536; 63 lines and a reply are not. Wrong behaviour: the client reads text forever from a
    // port that is not a chip (each line resets the wait), or the bound is off by a line.
    let text_line = || {
        let mut line = vec![b'x'; 1_023];
        line.push(b'\n');
        line
    };
    let reply = reply_line(&json!({"id": 1, "ok": true, "result": {"ok": true}}));

    let mut chunks: Vec<Vec<u8>> = (0..64).map(|_| text_line()).collect();
    chunks.extend(reply.clone());
    let (stream, _) = fake_chip::chip(Some(FRAME), saying(chunks));
    let error = ControlClient::from_stream(stream, None)
        .request("status", None)
        .expect_err("65,536 bytes of text end the request");
    assert_eq!(
        malformed(error),
        "the port does not speak the control protocol: 65536 bytes of text came before any \
         answer to status"
    );

    let mut chunks: Vec<Vec<u8>> = (0..63).map(|_| text_line()).collect();
    chunks.extend(reply);
    let (stream, _) = fake_chip::chip(Some(FRAME), saying(chunks));
    let result = ControlClient::from_stream(stream, None)
        .request("status", None)
        .expect("63,000 bytes of text before the reply are skipped");
    assert_eq!(result, json!({"ok": true}));
}

#[test]
fn a_reply_line_longer_than_the_frame_is_malformed() {
    // Specification 34 PORT-05, "The client: the frame": a reply line longer than the frame is
    // malformed. A reply of exactly 8,192 bytes is read (the object is 30 bytes around its
    // string), one of 8,193 is not. Wrong behaviour: an off-by-one at the bound, or the bound
    // missing, so a port that never ends a line makes the client buffer without limit.
    let reply_of = |length: usize| {
        let text = format!(
            r#"{{"id":1,"ok":true,"result":"{}"}}"#,
            "y".repeat(length - 30)
        );
        assert_eq!(text.len(), length);
        format!("{text}\n").into_bytes()
    };

    let (stream, _) = fake_chip::chip(Some(FRAME), saying(vec![reply_of(8_192)]));
    let result = ControlClient::from_stream(stream, None)
        .request("status", None)
        .expect("a reply of exactly the frame is read");
    assert_eq!(result.as_str().map(str::len), Some(8_192 - 30));

    let (stream, _) = fake_chip::chip(Some(FRAME), saying(vec![reply_of(8_193)]));
    let error = ControlClient::from_stream(stream, None)
        .request("status", None)
        .expect_err("a reply line above the frame is malformed");
    assert_eq!(
        malformed(error),
        "the answer to status is longer than 8193 bytes"
    );
}

#[test]
fn exchange_returns_the_whole_response_and_request_turns_a_refusal_into_an_error() {
    // Specification 11 §6.8.1: `ctl` prints the raw response, so `exchange` returns the response
    // object as the runtime wrote it, a refusal included; `request` returns the result, or the
    // refusal as an error. Wrong behaviour: `exchange` keeps only the result (the audit record id
    // is lost) or turns a refusal into an error (the command cannot print the runtime's code).
    let refusal = json!({
        "id": 1,
        "ok": false,
        "error": "insufficient role for shutdown",
        "error_code": "insufficient_role",
        "audit_id": "audit-7",
    });
    let (stream, _) = fake_chip::chip(Some(FRAME), saying(reply_line(&refusal)));
    let mut client = ControlClient::from_stream(stream, None);
    assert_eq!(client.exchange("shutdown", None), Ok(refusal.clone()));

    let (stream, _) = fake_chip::chip(Some(FRAME), saying(reply_line(&refusal)));
    let mut client = ControlClient::from_stream(stream, None);
    assert_eq!(
        client.request("shutdown", None),
        Err(ControlError::Refused {
            error: "insufficient role for shutdown".into(),
            code: Some("insufficient_role".into()),
        })
    );
}
