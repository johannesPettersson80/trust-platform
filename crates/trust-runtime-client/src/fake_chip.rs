//! A scripted chip behind an in-memory byte stream, for the tests of a link with a frame
//! (specification 34, PORT-05): no thread, no clock and no port. The chip answers each request line
//! the moment the client has written it, with the bytes its script gives, and the client reads them
//! back in packets of 64 bytes, as a chip's USB controller moves them. A read with nothing to read
//! times out at once.

use std::collections::VecDeque;
use std::fmt;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde_json::Value;

use crate::ByteStream;

/// The bytes of one packet of a chip's USB link.
const PACKET: usize = 64;

/// What the chip sends back for a request: the chunks of bytes, in order. None is a reply that was
/// lost.
pub(crate) type Answer = Vec<Vec<u8>>;

/// One reply line: `value` as JSON and a line feed.
pub(crate) fn reply_line(value: &Value) -> Answer {
    vec![format!("{value}\n").into_bytes()]
}

struct Shared {
    /// Everything the client wrote.
    written: Vec<u8>,
    /// The request line that is being written.
    pending: Vec<u8>,
    /// The request lines that were complete, parsed.
    requests: Vec<Value>,
    /// The bytes that the client has not read yet.
    outbox: VecDeque<u8>,
    /// The script of the chip.
    chip: Box<dyn FnMut(&Value) -> Answer + Send>,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().expect("the fake chip's lock is not poisoned")
}

/// The client's end of the stream.
pub(crate) struct FakeChip {
    shared: Arc<Mutex<Shared>>,
    frame: Option<usize>,
}

/// The test's view of the stream.
pub(crate) struct Watch {
    shared: Arc<Mutex<Shared>>,
}

impl Watch {
    /// The requests that the client wrote as whole lines.
    pub(crate) fn requests(&self) -> Vec<Value> {
        lock(&self.shared).requests.clone()
    }

    /// The number of bytes that the client wrote.
    pub(crate) fn written_bytes(&self) -> usize {
        lock(&self.shared).written.len()
    }
}

/// A stream to a chip that answers each request with what `script` returns for it. `frame` is the
/// frame of the link, or `None` for a stream without one, as a socket is.
pub(crate) fn chip(
    frame: Option<usize>,
    script: impl FnMut(&Value) -> Answer + Send + 'static,
) -> (FakeChip, Watch) {
    let shared = Arc::new(Mutex::new(Shared {
        written: Vec::new(),
        pending: Vec::new(),
        requests: Vec::new(),
        outbox: VecDeque::new(),
        chip: Box::new(script),
    }));
    let watch = Watch {
        shared: Arc::clone(&shared),
    };
    (FakeChip { shared, frame }, watch)
}

impl fmt::Debug for FakeChip {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("FakeChip").finish_non_exhaustive()
    }
}

impl Read for FakeChip {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut shared = lock(&self.shared);
        if shared.outbox.is_empty() {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "nothing to read"));
        }
        let count = buf.len().min(PACKET).min(shared.outbox.len());
        for slot in &mut buf[..count] {
            *slot = shared.outbox.pop_front().expect("count bytes are queued");
        }
        Ok(count)
    }
}

impl Write for FakeChip {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut shared = lock(&self.shared);
        shared.written.extend_from_slice(buf);
        for &byte in buf {
            if byte != b'\n' {
                shared.pending.push(byte);
                continue;
            }
            let line = std::mem::take(&mut shared.pending);
            let request: Value =
                serde_json::from_slice(&line).expect("the client writes JSON request lines");
            shared.requests.push(request.clone());
            let answer = (shared.chip)(&request);
            for chunk in answer {
                shared.outbox.extend(chunk);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl ByteStream for FakeChip {
    fn set_timeout(&self, _timeout: Duration) -> io::Result<()> {
        Ok(())
    }

    fn frame_bytes(&self) -> Option<usize> {
        self.frame
    }
}
