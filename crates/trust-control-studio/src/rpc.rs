//! The JSON-RPC 2.0 transport of `serve` (specification 35, §0.1a). A message is a body framed by a
//! `Content-Length` header, as the language server protocol frames its messages: header lines are
//! read up to an empty line, their names are case-insensitive and other headers are ignored. This
//! module reads and writes frames, and tells a call from a notification and an answer from an error
//! object. It decides nothing about Control Studio.

use std::io::{self, BufRead, ErrorKind, Write};

use serde_json::{json, Value};

/// The JSON-RPC version every message carries.
pub const VERSION: &str = "2.0";

/// The largest body accepted: 64 MiB. A request is small, and a body announced beyond this is
/// refused before it is read.
pub const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;

/// JSON-RPC error code: the body is not JSON.
pub const PARSE_ERROR: i64 = -32700;
/// JSON-RPC error code: the body is not a valid request.
pub const INVALID_REQUEST: i64 = -32600;
/// JSON-RPC error code: the method is not one of the engine's.
pub const METHOD_NOT_FOUND: i64 = -32601;
/// JSON-RPC error code: the parameters are neither an object nor an array.
pub const INVALID_PARAMS: i64 = -32602;

/// Why a frame cannot be read.
#[derive(Debug)]
pub enum FrameError {
    /// The header is not valid: a line is not `Name: value`, `Content-Length` is missing or given
    /// twice, or it is not a byte count.
    Header(String),
    /// The announced body is larger than [`MAX_BODY_BYTES`]; it is refused without being read.
    TooLarge {
        /// The length the header announced.
        announced: usize,
    },
    /// The input ends inside the body.
    Truncated {
        /// The length the header announced.
        announced: usize,
    },
    /// The input cannot be read.
    Io(io::Error),
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Header(reason) => write!(formatter, "The message header is invalid: {reason}."),
            Self::TooLarge { announced } => write!(
                formatter,
                "The message announces {announced} bytes, more than the {MAX_BODY_BYTES} accepted."
            ),
            Self::Truncated { announced } => {
                write!(
                    formatter,
                    "The input ends inside a message of {announced} bytes."
                )
            }
            Self::Io(error) => write!(formatter, "The input cannot be read: {error}."),
        }
    }
}

impl std::error::Error for FrameError {}

impl FrameError {
    /// The answer that tells the client why its input was refused, when the input can still be
    /// answered: a bad header or a message beyond the limit is an invalid request with a null
    /// identifier. A truncated or unreadable input gets none.
    #[must_use]
    pub fn answer(&self) -> Option<Value> {
        match self {
            Self::Header(_) | Self::TooLarge { .. } => {
                let error = ErrorObject {
                    data: Some(self.to_string()),
                    ..invalid_request()
                };
                Some(error_answer(Value::Null, &error))
            }
            Self::Truncated { .. } | Self::Io(_) => None,
        }
    }
}

/// Reads the next message body. `Ok(None)` means the input ended between two messages.
///
/// # Errors
/// The header is not valid, the body is larger than [`MAX_BODY_BYTES`], the input ends inside the
/// body, or the input cannot be read.
pub fn read_message<R: BufRead>(reader: &mut R) -> Result<Option<Vec<u8>>, FrameError> {
    let mut announced = None;
    let mut started = false;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).map_err(header_io_error)? == 0 {
            return if started {
                Err(FrameError::Header(
                    "the input ends before the empty line that closes the header".to_string(),
                ))
            } else {
                Ok(None)
            };
        }
        started = true;
        let text = line.trim_end_matches(['\r', '\n']);
        if text.is_empty() {
            break;
        }
        let Some((name, value)) = text.split_once(':') else {
            return Err(FrameError::Header(format!(
                "`{text}` is not a `Name: value` line"
            )));
        };
        if name.trim().eq_ignore_ascii_case("Content-Length") {
            if announced.is_some() {
                return Err(FrameError::Header(
                    "Content-Length is given twice".to_string(),
                ));
            }
            let length = value.trim().parse::<usize>().map_err(|_| {
                FrameError::Header(format!(
                    "Content-Length `{}` is not a byte count",
                    value.trim()
                ))
            })?;
            announced = Some(length);
        }
    }
    let Some(length) = announced else {
        return Err(FrameError::Header("Content-Length is missing".to_string()));
    };
    if length > MAX_BODY_BYTES {
        return Err(FrameError::TooLarge { announced: length });
    }
    let mut body = vec![0_u8; length];
    reader.read_exact(&mut body).map_err(|error| {
        if error.kind() == ErrorKind::UnexpectedEof {
            FrameError::Truncated { announced: length }
        } else {
            FrameError::Io(error)
        }
    })?;
    Ok(Some(body))
}

/// A header line that is not text is a bad header, not an unreadable input.
fn header_io_error(error: io::Error) -> FrameError {
    if error.kind() == ErrorKind::InvalidData {
        FrameError::Header("a header line is not UTF-8 text".to_string())
    } else {
        FrameError::Io(error)
    }
}

/// Writes one message body as a frame, and flushes it, so the client reads it at once.
///
/// # Errors
/// The writer fails.
pub fn write_message<W: Write>(writer: &mut W, body: &[u8]) -> io::Result<()> {
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(body)?;
    writer.flush()
}

/// What one body asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    /// A call: the client waits for the answer, which carries the call's identifier.
    Call {
        /// The identifier, as the client sent it.
        id: Value,
        /// The method's name.
        method: String,
        /// The arguments that the parameters stand for.
        arguments: Vec<Value>,
    },
    /// A notification: the client asks for no answer.
    Notification {
        /// The method's name.
        method: String,
        /// The arguments that the parameters stand for.
        arguments: Vec<Value>,
    },
    /// A body that is refused. `id` is the call's identifier when it can be read, else null.
    Rejected {
        /// The identifier to answer with.
        id: Value,
        /// The refusal.
        error: ErrorObject,
    },
    /// A notification whose parameters cannot be used. As every notification, it gets no answer.
    Ignored,
}

/// A JSON-RPC error object: its code, its standard message and, when it adds one, its data.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorObject {
    /// The code, one of the constants of this module.
    pub code: i64,
    /// The standard message of the code.
    pub message: &'static str,
    /// What the standard message does not say.
    pub data: Option<String>,
}

impl ErrorObject {
    /// An error object with its standard message and no data.
    #[must_use]
    pub const fn new(code: i64, message: &'static str) -> Self {
        Self {
            code,
            message,
            data: None,
        }
    }
}

/// Classifies one body (JSON-RPC 2.0, sections 4 and 5). A body that is not JSON is a parse error.
/// A body that is not a request object with the version `2.0`, a string method and an identifier
/// of string, number or null is an invalid request. A request without an identifier is a
/// notification.
#[must_use]
pub fn classify(body: &[u8]) -> Incoming {
    let Ok(message) = serde_json::from_slice::<Value>(body) else {
        return rejected(Value::Null, ErrorObject::new(PARSE_ERROR, "Parse error"));
    };
    let Value::Object(fields) = &message else {
        return rejected(Value::Null, invalid_request());
    };
    let identifier = fields.get("id").filter(|id| is_identifier(id));
    let (Some(VERSION), Some(method)) = (
        fields.get("jsonrpc").and_then(Value::as_str),
        fields.get("method").and_then(Value::as_str),
    ) else {
        return rejected(
            identifier.cloned().unwrap_or(Value::Null),
            invalid_request(),
        );
    };
    let params = fields.get("params");
    match fields.get("id") {
        None => match arguments_of(params) {
            Some(arguments) => Incoming::Notification {
                method: method.to_string(),
                arguments,
            },
            None => Incoming::Ignored,
        },
        Some(id) if is_identifier(id) => match arguments_of(params) {
            Some(arguments) => Incoming::Call {
                id: id.clone(),
                method: method.to_string(),
                arguments,
            },
            None => rejected(
                id.clone(),
                ErrorObject::new(INVALID_PARAMS, "Invalid params"),
            ),
        },
        Some(_) => rejected(Value::Null, invalid_request()),
    }
}

/// The arguments that parameters stand for: none when they are absent, the items of an array, or
/// the object itself. Any other value is refused (JSON-RPC 2.0, section 4.2).
fn arguments_of(params: Option<&Value>) -> Option<Vec<Value>> {
    match params {
        None => Some(Vec::new()),
        Some(Value::Array(items)) => Some(items.clone()),
        Some(object @ Value::Object(_)) => Some(vec![object.clone()]),
        Some(_) => None,
    }
}

/// An identifier is a string, a number or null (JSON-RPC 2.0, section 4).
fn is_identifier(id: &Value) -> bool {
    matches!(id, Value::Null | Value::String(_) | Value::Number(_))
}

fn rejected(id: Value, error: ErrorObject) -> Incoming {
    Incoming::Rejected { id, error }
}

fn invalid_request() -> ErrorObject {
    ErrorObject::new(INVALID_REQUEST, "Invalid Request")
}

/// The answer of a call that succeeded.
#[must_use]
pub fn result_answer(id: Value, result: Value) -> Value {
    call_answer(id, "result", result)
}

/// The answer of a call that failed in the protocol.
#[must_use]
pub fn error_answer(id: Value, error: &ErrorObject) -> Value {
    let body = match &error.data {
        Some(data) => json!({ "code": error.code, "message": error.message, "data": data }),
        None => json!({ "code": error.code, "message": error.message }),
    };
    call_answer(id, "error", body)
}

/// An answer to the call `id` whose `member` is `content` (`result` or `error`).
fn call_answer(id: Value, member: &str, content: Value) -> Value {
    let mut fields = serde_json::Map::new();
    fields.insert("jsonrpc".to_owned(), Value::from(VERSION));
    fields.insert("id".to_owned(), id);
    fields.insert(member.to_owned(), content);
    Value::Object(fields)
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use super::*;

    fn frame(body: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        write_message(&mut bytes, body.as_bytes()).expect("the frame is written");
        bytes
    }

    fn rejection(body: &str) -> (Value, i64) {
        match classify(body.as_bytes()) {
            Incoming::Rejected { id, error } => (id, error.code),
            other => panic!("expected a rejection, got {other:?}"),
        }
    }

    #[test]
    fn a_frame_is_written_with_crlf_and_read_back_whole() {
        assert_eq!(frame("{}"), b"Content-Length: 2\r\n\r\n{}".to_vec());
        let mut reader = Cursor::new(frame("{\"id\":1}"));
        assert_eq!(
            read_message(&mut reader).expect("read").expect("a message"),
            b"{\"id\":1}".to_vec()
        );
        assert!(
            read_message(&mut reader).expect("read").is_none(),
            "the input ends between messages"
        );
    }

    #[test]
    fn split_and_joined_messages_are_read_one_body_at_a_time() {
        let mut joined = frame("{\"a\":1}");
        joined.extend(frame("{\"b\":2}"));
        // One byte per read, so every header line and every body arrives split.
        let mut reader = BufReader::with_capacity(1, Cursor::new(joined));
        assert_eq!(
            read_message(&mut reader).expect("read").expect("first"),
            b"{\"a\":1}".to_vec()
        );
        assert_eq!(
            read_message(&mut reader).expect("read").expect("second"),
            b"{\"b\":2}".to_vec()
        );
        assert!(read_message(&mut reader).expect("read").is_none());
    }

    #[test]
    fn header_names_are_case_insensitive_and_other_headers_are_ignored() {
        let text =
            "content-type: application/vscode-jsonrpc; charset=utf-8\r\nCONTENT-LENGTH:2\r\n\r\n{}";
        let mut reader = Cursor::new(text.as_bytes().to_vec());
        assert_eq!(
            read_message(&mut reader).expect("read").expect("a message"),
            b"{}".to_vec()
        );
    }

    #[test]
    fn a_missing_repeated_or_invalid_content_length_is_refused() {
        for text in [
            "X-Other: 1\r\n\r\n{}",
            "Content-Length: two\r\n\r\n{}",
            "Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}",
            "not a header\r\n\r\n",
            "Content-Length: 2\r\n",
        ] {
            let mut reader = Cursor::new(text.as_bytes().to_vec());
            assert!(
                matches!(read_message(&mut reader), Err(FrameError::Header(_))),
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_body_shorter_than_announced_is_truncated() {
        let mut reader = Cursor::new(b"Content-Length: 10\r\n\r\nabc".to_vec());
        assert!(matches!(
            read_message(&mut reader),
            Err(FrameError::Truncated { announced: 10 })
        ));
    }

    #[test]
    fn a_message_beyond_the_maximum_is_refused_before_its_body_is_read() {
        let announced = MAX_BODY_BYTES + 1;
        let mut reader = Cursor::new(format!("Content-Length: {announced}\r\n\r\n").into_bytes());
        assert!(matches!(
            read_message(&mut reader),
            Err(FrameError::TooLarge { announced: found }) if found == announced
        ));
    }

    #[test]
    fn a_body_that_is_not_json_is_a_parse_error_with_a_null_id() {
        assert_eq!(rejection("{not json"), (Value::Null, PARSE_ERROR));
    }

    #[test]
    fn a_body_that_is_not_a_request_is_an_invalid_request() {
        assert_eq!(rejection("[1,2]"), (Value::Null, INVALID_REQUEST));
        assert_eq!(rejection("3"), (Value::Null, INVALID_REQUEST));
        assert_eq!(
            rejection(r#"{"id":7,"method":"controlStudio/status"}"#),
            (json!(7), INVALID_REQUEST),
            "no jsonrpc"
        );
        assert_eq!(
            rejection(r#"{"jsonrpc":"1.0","id":8,"method":"x"}"#),
            (json!(8), INVALID_REQUEST),
            "another version"
        );
        assert_eq!(
            rejection(r#"{"jsonrpc":"2.0","id":3}"#),
            (json!(3), INVALID_REQUEST),
            "no method"
        );
        assert_eq!(
            rejection(r#"{"jsonrpc":"2.0","id":4,"method":5}"#),
            (json!(4), INVALID_REQUEST),
            "a method that is not a string"
        );
        assert_eq!(
            rejection(r#"{"method":"x"}"#),
            (Value::Null, INVALID_REQUEST),
            "a notification must be a valid request"
        );
        assert_eq!(
            rejection(r#"{"jsonrpc":"2.0","id":{},"method":"x"}"#),
            (Value::Null, INVALID_REQUEST),
            "an object is not an identifier"
        );
    }

    #[test]
    fn params_must_be_an_object_or_an_array() {
        assert_eq!(
            rejection(r#"{"jsonrpc":"2.0","id":4,"method":"x","params":5}"#),
            (json!(4), INVALID_PARAMS)
        );
        assert_eq!(
            rejection(r#"{"jsonrpc":"2.0","id":4,"method":"x","params":null}"#),
            (json!(4), INVALID_PARAMS)
        );
        assert_eq!(
            classify(br#"{"jsonrpc":"2.0","id":1,"method":"x","params":{"a":1}}"#),
            Incoming::Call {
                id: json!(1),
                method: "x".to_string(),
                arguments: vec![json!({ "a": 1 })],
            }
        );
        assert_eq!(
            classify(br#"{"jsonrpc":"2.0","id":1,"method":"x","params":[1,"b"]}"#),
            Incoming::Call {
                id: json!(1),
                method: "x".to_string(),
                arguments: vec![json!(1), json!("b")],
            }
        );
        assert_eq!(
            classify(br#"{"jsonrpc":"2.0","id":1,"method":"x"}"#),
            Incoming::Call {
                id: json!(1),
                method: "x".to_string(),
                arguments: Vec::new(),
            }
        );
    }

    #[test]
    fn a_notification_gets_no_answer_even_when_it_cannot_run() {
        assert_eq!(
            classify(br#"{"jsonrpc":"2.0","method":"controlStudio/status","params":{}}"#),
            Incoming::Notification {
                method: "controlStudio/status".to_string(),
                arguments: vec![json!({})],
            }
        );
        assert_eq!(
            classify(br#"{"jsonrpc":"2.0","method":"x","params":5}"#),
            Incoming::Ignored
        );
    }

    #[test]
    fn a_frame_error_is_answered_only_when_the_input_can_still_be_answered() {
        let answer = FrameError::Header("Content-Length is missing".to_string())
            .answer()
            .expect("answered");
        assert_eq!(answer["id"], Value::Null);
        assert_eq!(answer["error"]["code"], INVALID_REQUEST);
        assert!(FrameError::Truncated { announced: 3 }.answer().is_none());
    }
}
