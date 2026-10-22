//! The `serve` front end (specification 35, §0.1a): the engine's methods as JSON-RPC 2.0 messages on
//! a reader and a writer. Requests are answered on the reader's thread, because `execute` answers at
//! once. An operation's updates are notifications written through the same writer, one frame at a
//! time, so frames never interleave. The loop ends when its input ends and waits for nothing else.

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{json, Value};
use trust_control::messages::OperationUpdate;

use crate::commands::{ControlStudio, UPDATE_NOTIFICATION};
use crate::operation::UpdateSink;
use crate::rpc::{self, ErrorObject, FrameError, Incoming, METHOD_NOT_FOUND};

/// Why `serve` ended with an error.
#[derive(Debug)]
pub enum StreamError {
    /// The input is not framed as JSON-RPC. The client was told why, when the input allowed it.
    Frame(FrameError),
    /// An answer cannot be written.
    Write(io::Error),
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frame(error) => write!(formatter, "{error}"),
            Self::Write(error) => write!(formatter, "The answer cannot be written: {error}."),
        }
    }
}

impl std::error::Error for StreamError {}

/// Answers the messages read from `input` on `output`, until `input` ends. The updates of the
/// engine's operations are written on `output` as they happen.
///
/// # Errors
/// The input is not framed as JSON-RPC (`StreamError::Frame`), or an answer cannot be written
/// (`StreamError::Write`).
pub fn serve<R: BufRead, W: Write + Send + 'static>(
    mut input: R,
    output: W,
) -> Result<(), StreamError> {
    let writer = Arc::new(Mutex::new(output));
    let studio = ControlStudio::new(Arc::new(Notifications {
        writer: Arc::clone(&writer),
    }));
    loop {
        match rpc::read_message(&mut input) {
            Ok(Some(body)) => {
                if let Some(answer) = respond(&studio, &rpc::classify(&body)) {
                    write_value(&writer, &answer).map_err(StreamError::Write)?;
                }
            }
            Ok(None) => return Ok(()),
            Err(error) => {
                if let Some(answer) = error.answer() {
                    write_value(&writer, &answer).map_err(StreamError::Write)?;
                }
                return Err(StreamError::Frame(error));
            }
        }
    }
}

/// The answer to one classified message, when it has one.
fn respond(studio: &ControlStudio, incoming: &Incoming) -> Option<Value> {
    match incoming {
        Incoming::Call {
            id,
            method,
            arguments,
        } => Some(match studio.execute(method, arguments) {
            Some(answer) => rpc::result_answer(id.clone(), answer),
            None => rpc::error_answer(
                id.clone(),
                &ErrorObject::new(METHOD_NOT_FOUND, "Method not found"),
            ),
        }),
        Incoming::Notification { method, arguments } => {
            // A notification runs like a call, and its answer is not sent.
            let _ = studio.execute(method, arguments);
            None
        }
        Incoming::Rejected { id, error } => Some(rpc::error_answer(id.clone(), error)),
        Incoming::Ignored => None,
    }
}

/// Writes one message as a frame, holding the writer for that frame alone.
fn write_value<W: Write>(writer: &Mutex<W>, value: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(value).map_err(io::Error::other)?;
    rpc::write_message(&mut *writer.lock(), &body)
}

/// The sink of the engine's operations: each update is written as a `controlStudio/update`
/// notification.
struct Notifications<W> {
    writer: Arc<Mutex<W>>,
}

impl<W: Write + Send> UpdateSink for Notifications<W> {
    fn send(&self, update: OperationUpdate) {
        let Ok(params) = serde_json::to_value(&update) else {
            return;
        };
        let notification = json!({
            "jsonrpc": rpc::VERSION,
            "method": UPDATE_NOTIFICATION,
            "params": params,
        });
        // A client that has closed its end receives nothing more. The runtime keeps the operation
        // and the journal keeps its identity, so reopening the loop reattaches to it (A8.5, A8.6).
        let _ = write_value(&self.writer, &notification);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;
    use std::path::PathBuf;

    use trust_control::messages::OperationState;

    use super::*;

    const EXAMPLE: &str =
        include_str!("../../../examples/control-pid-commissioning/control-loop.json");

    /// A writer whose bytes the test reads back.
    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<u8>>>);

    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// A folder of the test, removed when the test ends, whether or not it passed.
    struct Folder(PathBuf);

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn framed(body: &Value) -> Vec<u8> {
        let mut bytes = Vec::new();
        rpc::write_message(&mut bytes, body.to_string().as_bytes()).expect("framed");
        bytes
    }

    /// The frames the test's output holds, each read back as JSON.
    fn frames(output: &Shared) -> Vec<Value> {
        let bytes = output.0.lock().clone();
        let mut reader = Cursor::new(bytes);
        let mut values = Vec::new();
        while let Some(body) = rpc::read_message(&mut reader).expect("a frame") {
            values.push(serde_json::from_slice(&body).expect("JSON"));
        }
        values
    }

    #[test]
    fn an_unknown_method_is_answered_with_method_not_found_and_the_end_of_input_ends_serve() {
        let input =
            framed(&json!({ "jsonrpc": "2.0", "id": 1, "method": "controlStudio/nothing" }));
        let output = Shared::default();
        serve(Cursor::new(input), output.clone()).expect("serve ends at the end of its input");
        let answered = frames(&output);
        assert_eq!(answered.len(), 1, "{answered:?}");
        assert_eq!(answered[0]["id"], json!(1));
        assert_eq!(answered[0]["error"]["code"], json!(-32601));
    }

    #[test]
    fn open_answers_with_the_loops_as_its_result() {
        let folder =
            Folder(std::env::temp_dir().join(format!("trust-serve-open-{}", std::process::id())));
        let _ = fs::remove_dir_all(&folder.0);
        fs::create_dir_all(&folder.0).expect("the project folder");
        fs::write(folder.0.join("control-loop.json"), EXAMPLE).expect("the loop");
        let request = json!({
            "jsonrpc": "2.0",
            "id": "open-1",
            "method": "controlStudio/open",
            "params": {
                "projectRoot": folder.0.display().to_string(),
                "runtimeId": "simulator",
                "storageFolder": folder.0.join("storage").display().to_string(),
            },
        });
        let output = Shared::default();
        serve(Cursor::new(framed(&request)), output.clone()).expect("serve ends");
        let answered = frames(&output);
        assert_eq!(answered[0]["id"], json!("open-1"));
        assert_eq!(answered[0]["result"]["result"]["kind"], json!("loops"));
        assert_eq!(
            answered[0]["result"]["result"]["loops"][0]["loopId"],
            json!("control-pid-commissioning")
        );
    }

    #[test]
    fn a_framing_error_is_answered_with_an_invalid_request_and_ends_serve() {
        let output = Shared::default();
        let error = serve(
            Cursor::new(b"Content-Type: text/plain\r\n\r\n{}".to_vec()),
            output.clone(),
        )
        .expect_err("the frame has no length");
        assert!(matches!(error, StreamError::Frame(FrameError::Header(_))));
        assert_eq!(frames(&output)[0]["error"]["code"], json!(-32600));
    }

    #[test]
    fn an_operation_update_is_written_as_a_notification_without_an_identifier() {
        let output = Shared::default();
        let sink = Notifications {
            writer: Arc::new(Mutex::new(output.clone())),
        };
        sink.send(OperationUpdate {
            operation_id: "operation-1".to_string(),
            command: "identify".to_string(),
            state: OperationState::Running,
            stage: None,
            progress: Some("Recording the plant".to_string()),
            result: None,
            failure: None,
            message: None,
            stages: None,
        });
        let sent = frames(&output);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["method"], json!(UPDATE_NOTIFICATION));
        assert_eq!(sent[0]["params"]["operationId"], json!("operation-1"));
        assert!(
            sent[0].get("id").is_none(),
            "a notification has no identifier"
        );
    }
}
