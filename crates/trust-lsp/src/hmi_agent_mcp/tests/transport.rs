//! The transport of the stdio server (specification 43, §3.1 and Repairs — 7 October 2026, item 3):
//! message lines and their 8 MiB limit, the loopback's answer read whole with its raw-byte limit,
//! and the start, which needs the loopback's tool list.

use std::io::Cursor;
use std::net::{Ipv4Addr, SocketAddr, TcpListener};

use serde_json::{json, Value};

use super::super::bridge::CALL;
use super::super::connection::Connection;
use super::super::lines::{read_line, Line};
use super::super::server::StartError;
use super::super::Limits;
use super::{
    call, ping, refusal, tools, unavailable, Calls, FakeBridge, Session, CAPABILITY, WORKSPACE,
};

/// A text whose characters take two to four bytes each.
const TEXT: &str = "Pumpe Ø 50 – Füllstand · 🌊";

/// The byte limit of a line: 8 MiB.
const LINE_LIMIT: usize = 8 * 1024 * 1024;

/// A ping with id `id` whose line is exactly `length` bytes long.
fn ping_line(id: u32, length: usize) -> Vec<u8> {
    let empty =
        json!({"jsonrpc": "2.0", "id": id, "method": "ping", "params": {"pad": ""}}).to_string();
    let padded = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "ping",
        "params": {"pad": "x".repeat(length - empty.len())}
    })
    .to_string();
    assert_eq!(padded.len(), length);
    padded.into_bytes()
}

/// Catches: a line of exactly 8 MiB being refused, a line one byte longer being run, the `\r` of a
/// `\r\n` line end being counted, or the rest of a long line being read as further messages.
#[test]
fn a_line_over_8_mib_is_refused_and_the_next_line_is_served() {
    let bridge = FakeBridge::start(Calls::Never);
    let session = Session::start(&bridge, Limits::PRODUCT);
    let mut exact = ping_line(1, LINE_LIMIT);
    exact.push(b'\n');
    session.send_raw(&exact);
    assert_eq!(
        session.next(),
        json!({"jsonrpc": "2.0", "id": 1, "result": {}})
    );
    let mut crlf = ping_line(2, LINE_LIMIT);
    crlf.extend_from_slice(b"\r\n");
    session.send_raw(&crlf);
    assert_eq!(session.next()["id"], json!(2));
    let mut over = ping_line(3, LINE_LIMIT + 1);
    over.push(b'\n');
    session.send_raw(&over);
    assert_eq!(session.next(), unavailable(Value::Null), "refused unread");
    assert_eq!(
        session.ask(&ping(json!(4))),
        json!({"jsonrpc": "2.0", "id": 4, "result": {}}),
        "the next line is the next message"
    );
    session.close();
}

/// Catches: the line reader splitting or joining lines wrongly, keeping a long line's bytes or
/// losing a last line without a line end.
#[test]
fn lines_end_at_a_newline_and_are_limited_before_their_line_end() {
    let mut input = Cursor::new(b"abc\r\nabcdefgh\r\nabcdefghi\n\nabcdefgh\rx\nlast".to_vec());
    let mut lines = Vec::new();
    while let Some(line) = read_line(&mut input, 8).expect("read") {
        lines.push(line);
    }
    assert_eq!(
        lines,
        [
            Line::Text(b"abc".to_vec()),
            Line::Text(b"abcdefgh".to_vec()),
            Line::TooLong,
            Line::Text(Vec::new()),
            Line::TooLong,
            Line::Text(b"last".to_vec()),
        ]
    );
}

/// A tool result whose text is `text`, as the loopback's answer body.
fn result_body(text: &str) -> Vec<u8> {
    json!({"content": [{"type": "text", "text": text}]})
        .to_string()
        .into_bytes()
}

/// Catches: the answer being decoded read by read, which turns a character split across two
/// network reads into replacement characters (Repairs — 7 October 2026, item 3).
#[test]
fn a_character_split_across_reads_reaches_the_agent_intact() {
    let body = result_body(TEXT);
    for character in ["Ø", "–", "🌊"] {
        let start = body
            .windows(character.len())
            .position(|window| window == character.as_bytes())
            .expect("the character is in the body");
        for inside in 1..character.len() {
            let bridge = FakeBridge::start(Calls::Split(body.clone(), start + inside));
            let session = Session::start(&bridge, Limits::PRODUCT);
            let answer = session.ask(&call(json!(1), "hmi_get_context", json!({})));
            assert_eq!(
                answer,
                json!({"jsonrpc": "2.0", "id": 1, "result": {"content": [{"type": "text", "text": TEXT}]}}),
                "{character} split after its byte {inside}"
            );
            session.close();
        }
    }
}

/// Catches: the answer limit counting characters instead of raw bytes, or refusing an answer of
/// exactly the limit.
#[test]
fn the_answer_limit_counts_raw_bytes() {
    let body = result_body(&"Ø".repeat(2000));
    let characters = String::from_utf8(body.clone())
        .expect("UTF-8")
        .chars()
        .count();
    assert!(characters < body.len(), "each Ø takes two bytes");
    for (limit, accepted) in [
        (body.len(), true),
        (body.len() - 1, false),
        (characters, false),
    ] {
        let bridge = FakeBridge::start(Calls::Split(body.clone(), body.len()));
        let session = Session::start(
            &bridge,
            Limits {
                answer_bytes: limit,
                ..Limits::PRODUCT
            },
        );
        let answer = session.ask(&call(json!(5), "hmi_get_context", json!({})));
        if accepted {
            assert_eq!(
                answer["result"],
                serde_json::from_slice::<Value>(&body).expect("JSON")
            );
        } else {
            assert_eq!(answer, unavailable(json!(5)), "limit {limit}");
        }
        session.close();
    }
}

/// Catches: a server starting without the loopback's tool list; it would refuse every call later
/// instead of failing at once with the message the agent's client shows.
#[test]
fn the_server_starts_only_with_the_loopbacks_tool_list() {
    let closed = {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind");
        listener.local_addr().expect("address").port()
    };
    let nobody = Connection {
        address: SocketAddr::from((Ipv4Addr::LOCALHOST, closed)),
        capability: CAPABILITY.to_owned(),
        workspace_id: WORKSPACE.to_owned(),
    };
    assert_eq!(
        Session::try_start(nobody, Limits::PRODUCT).err(),
        Some(StartError::Bridge)
    );
    let bridge = FakeBridge::start(Calls::Never);
    let other_capability = Connection {
        capability: "f".repeat(64),
        ..bridge.connection()
    };
    assert_eq!(
        Session::try_start(other_capability, Limits::PRODUCT).err(),
        Some(StartError::Bridge),
        "the loopback refuses another capability"
    );
    for answer in [
        json!({"tools": {"name": "hmi_get_context"}}),
        json!({"tools": [{"description": "no name"}]}),
        json!({"tools": [{"name": 1}]}),
        json!(tools()),
    ] {
        let bridge = FakeBridge::with_tools(Calls::Never, answer.clone());
        assert_eq!(
            Session::try_start(bridge.connection(), Limits::PRODUCT).err(),
            Some(StartError::Tools),
            "{answer}"
        );
    }
}

/// Catches: arguments that reach the loopback as other text than the agent's JSON, after the
/// server read them (escapes, quotes and line breaks in strings).
#[test]
fn the_call_body_is_the_agents_arguments_as_json() {
    let bridge = FakeBridge::start(Calls::Answer(json!({"content": []})));
    let session = Session::start(&bridge, Limits::PRODUCT);
    session.send_raw(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"hmi_get_context\",\"arguments\":{\"input\":{\"title\":\"\\u00d8 \\\"q\\\" \\n\",\"n\":12}}}}\n"
            .as_bytes(),
    );
    assert_eq!(session.next()["id"], json!(1));
    let calls = bridge.received_on(CALL);
    assert_eq!(
        calls[0].body["arguments"],
        json!({"input": {"title": "Ø \"q\" \n", "n": 12}, "workspaceId": WORKSPACE})
    );
    assert_eq!(
        session.ask(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "hmi_get_context"}})),
        refusal(json!(2), -32602, "Tool arguments must be an object.")
    );
    session.close();
}
