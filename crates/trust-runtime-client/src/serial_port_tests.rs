//! The serial port as a control stream (specification 34, PORT-05, "The client: opening the port"
//! and "The client: a link that goes away", and specification 11 §6.8.1): the endpoint, how the port
//! is opened, the frame, the wait, and a port that went away. A scripted line stands for the
//! operating system's port; no test opens a real one.

use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::{
    ByteStream, ControlClient, ControlEndpoint, ControlError, SerialLine, SerialStream,
    SERIAL_FRAME_BYTES,
};

/// A call that the client made on the line to set it up.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Dtr(bool),
    Rts(bool),
    DiscardInput,
    Timeout(Duration),
}

/// What the line does when it is read.
#[derive(Clone, Copy)]
enum Reading {
    /// The wait runs out.
    TimesOut,
    /// The operating system fails the read, as it does for a port whose USB device is gone.
    BreaksPipe,
    /// The read returns no bytes.
    EndsStream,
}

/// A serial port that records the calls made on it and fails as its script says.
struct ScriptedLine {
    calls: Arc<Mutex<Vec<Call>>>,
    reading: Reading,
    writing_fails: bool,
    refuses_rts: bool,
}

impl ScriptedLine {
    fn new(calls: &Arc<Mutex<Vec<Call>>>) -> Self {
        Self {
            calls: Arc::clone(calls),
            reading: Reading::TimesOut,
            writing_fails: false,
            refuses_rts: false,
        }
    }

    fn record(&self, call: Call) {
        self.calls
            .lock()
            .expect("the lock is not poisoned")
            .push(call);
    }
}

impl Read for ScriptedLine {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        match self.reading {
            Reading::TimesOut => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Operation timed out",
            )),
            Reading::BreaksPipe => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Input/output error",
            )),
            Reading::EndsStream => Ok(0),
        }
    }
}

impl Write for ScriptedLine {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.writing_fails {
            Err(io::Error::other("Input/output error"))
        } else {
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl SerialLine for ScriptedLine {
    fn set_dtr(&mut self, on: bool) -> io::Result<()> {
        self.record(Call::Dtr(on));
        Ok(())
    }

    fn set_rts(&mut self, on: bool) -> io::Result<()> {
        self.record(Call::Rts(on));
        if self.refuses_rts {
            Err(io::Error::other("RTS is not available"))
        } else {
            Ok(())
        }
    }

    fn discard_input(&mut self) -> io::Result<()> {
        self.record(Call::DiscardInput);
        Ok(())
    }

    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        self.record(Call::Timeout(timeout));
        Ok(())
    }
}

/// The calls recorded so far.
fn calls_of(calls: &Arc<Mutex<Vec<Call>>>) -> Vec<Call> {
    calls.lock().expect("the lock is not poisoned").clone()
}

/// A `status` request over the line, which waits 5 seconds.
fn status_over(line: ScriptedLine) -> Result<Value, ControlError> {
    let stream = SerialStream::new(line, "/dev/ttyACM0", Duration::from_secs(5))
        .expect("a line that takes every setting opens");
    ControlClient::from_stream(stream, None).request("status", None)
}

#[cfg(unix)]
#[test]
fn a_serial_endpoint_is_the_absolute_port_path_after_serial_slash_slash() {
    // Specification 11 §6.8.1: the text after `serial://`, trimmed, is the port name; on Linux and
    // macOS it is an absolute path, and it is not empty. Wrong behaviour: an empty name or a
    // relative one is accepted, so the port would be opened relative to the working directory, or
    // the scheme is read with another case or with leading space.
    assert_eq!(
        ControlEndpoint::parse("serial:///dev/ttyACM0"),
        Some(ControlEndpoint::Serial("/dev/ttyACM0".into()))
    );
    assert_eq!(
        ControlEndpoint::parse("serial://  /dev/cu.usbmodem1101 "),
        Some(ControlEndpoint::Serial("/dev/cu.usbmodem1101".into()))
    );
    assert_eq!(ControlEndpoint::parse("serial://"), None);
    assert_eq!(ControlEndpoint::parse("serial://ttyACM0"), None);
    assert_eq!(ControlEndpoint::parse("SERIAL:///dev/ttyACM0"), None);
    assert_eq!(ControlEndpoint::parse(" serial:///dev/ttyACM0"), None);
}

#[cfg(not(unix))]
#[test]
fn a_serial_endpoint_is_the_port_name_after_serial_slash_slash() {
    // Specification 11 §6.8.1: on Windows the port name is any text that is not empty, such as
    // `COM5`. Wrong behaviour: the name is empty, or the scheme is read with another case.
    assert_eq!(
        ControlEndpoint::parse("serial://COM5"),
        Some(ControlEndpoint::Serial("COM5".into()))
    );
    assert_eq!(ControlEndpoint::parse("serial://"), None);
    assert_eq!(ControlEndpoint::parse("SERIAL://COM5"), None);
}

#[test]
fn opening_the_port_asserts_dtr_and_releases_rts() {
    // Specification 34 PORT-05, "The client: opening the port": right after the open the client
    // sets DTR on and then RTS off, as the bench does, and nothing else; then it discards the
    // bytes from before the open, and sets the wait. Wrong behaviour: RTS stays on, which is the
    // level pair that the boot loader's flash tool starts its reset with; DTR is off; the two are
    // set in the other order; a line is pulsed; the old bytes stay, so a reply of an earlier
    // session with the same id is taken for this session's. The stream also moves the frame of a
    // chip's USB link, 8,192 bytes.
    let calls = Arc::new(Mutex::new(Vec::new()));
    let stream = SerialStream::new(
        ScriptedLine::new(&calls),
        "/dev/ttyACM0",
        Duration::from_secs(5),
    )
    .expect("a line that takes every setting opens");
    assert_eq!(
        calls_of(&calls),
        vec![
            Call::Dtr(true),
            Call::Rts(false),
            Call::DiscardInput,
            Call::Timeout(Duration::from_secs(5)),
        ]
    );
    assert_eq!(stream.frame_bytes(), Some(8_192));
    assert_eq!(SERIAL_FRAME_BYTES, 8_192);
}

#[test]
fn a_port_that_refuses_a_setting_is_not_opened() {
    // Specification 34 PORT-05: "A setting that fails is reported, never ignored." Wrong
    // behaviour: the failure is dropped and the client goes on with a port whose RTS is on, which
    // can reset the chip.
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut line = ScriptedLine::new(&calls);
    line.refuses_rts = true;
    let error = SerialStream::new(line, "/dev/ttyACM0", Duration::from_secs(5))
        .expect_err("a refused RTS ends the open");
    assert_eq!(error.to_string(), "RTS is not available");
    assert_eq!(
        calls_of(&calls),
        vec![Call::Dtr(true), Call::Rts(false)],
        "nothing is set after the refused setting"
    );
}

#[test]
fn a_port_that_went_away_is_reported_plainly() {
    // Specification 34 PORT-05, "The client: a link that goes away": a read or write that fails
    // for any reason but the wait running out, and a read that returns no bytes, end the request
    // with the port's name, the operating system's reason and what to do. Wrong behaviour: the
    // bare operating system text, which does not say that a reset or a power cycle enumerates the
    // port again; or a wait that runs out is reported as a port that went away.
    let plain = |reason: &str| {
        format!(
            "serial port /dev/ttyACM0 went away ({reason}): the chip reset, lost power or was \
             unplugged; its port returns when the chip has started again, so run the command again"
        )
    };
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut broken_read = ScriptedLine::new(&calls);
    broken_read.reading = Reading::BreaksPipe;
    let mut empty_read = ScriptedLine::new(&calls);
    empty_read.reading = Reading::EndsStream;
    let mut broken_write = ScriptedLine::new(&calls);
    broken_write.writing_fails = true;
    for (line, reason) in [
        (broken_read, "Input/output error"),
        (empty_read, "the port gave no bytes"),
        (broken_write, "Input/output error"),
    ] {
        assert_eq!(
            status_over(line),
            Err(ControlError::Transport(plain(reason)))
        );
    }
}

#[test]
fn a_wait_that_runs_out_is_not_a_port_that_went_away() {
    // Specification 34 PORT-05: a wait that runs out is a failure of the link that names the wait.
    // Wrong behaviour: it is reported as a port that went away, so a slow chip looks unplugged.
    let calls = Arc::new(Mutex::new(Vec::new()));
    assert_eq!(
        status_over(ScriptedLine::new(&calls)),
        Err(ControlError::Transport(
            "no response from serial port /dev/ttyACM0 within 5000 ms".into()
        ))
    );
}

#[test]
fn the_wait_the_caller_sets_reaches_the_port() {
    // Specification 11 §6.8.1: the reply to `program.install` is awaited 120 seconds, so the wait
    // that `set_timeout` gives the client must reach the port. Wrong behaviour: it stays at the
    // wait of the open, and the install of a large container ends after 5 seconds.
    let calls = Arc::new(Mutex::new(Vec::new()));
    let stream = SerialStream::new(
        ScriptedLine::new(&calls),
        "/dev/ttyACM0",
        Duration::from_secs(5),
    )
    .expect("a line that takes every setting opens");
    let mut client = ControlClient::from_stream(stream, None);
    client
        .set_timeout(Duration::from_mins(2))
        .expect("the stream takes the wait");
    assert!(matches!(
        client.request("program.install", None),
        Err(ControlError::Transport(_))
    ));
    assert_eq!(
        calls_of(&calls).last(),
        Some(&Call::Timeout(Duration::from_mins(2)))
    );
}
