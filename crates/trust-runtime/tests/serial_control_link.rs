//! The serial control link end to end, without hardware (specification 34, PORT-05 and ARCH-12;
//! specification 11 §6.8.1). The shared control client talks as it talks to a chip's USB port, over
//! an in-memory duplex that stands for the port, to the code a chip runs: the services' resource
//! loop and line link, and the services' transfer session. No test opens a real port.

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_runtime_client::{ControlClient, SerialLine, SerialStream};
use trust_runtime_core::deployment::{ControlMode, DeviceDescriptor};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{
    ControlLink, HardwareWatchdog, LinkFrame, MonotonicClock, MonotonicInstant, PeerClass,
    PortError, RunnerWait, TimeSpan, WakeReason, WakeToken,
};
use trust_runtime_services::capabilities::{LastReset, ResetCauseName};
use trust_runtime_services::chip::platform::{status_settings, ChipPlatform, ChipProfile};
use trust_runtime_services::link::DeviceLineLink;
use trust_runtime_services::runner::engine::{EngineCycle, ProcessIo};
use trust_runtime_services::runner::{EngineLiveValues, Runner};
use trust_runtime_services::status::IoDriverStatus;
use trust_runtime_services::transfer::{wire, TransferSession};

/// How long the client end waits for the chip before the test fails.
const WAIT: Duration = Duration::from_secs(5);

/// The client's end of the in-memory duplex: the serial port.
struct Port {
    to_chip: Sender<Vec<u8>>,
    from_chip: Receiver<Vec<u8>>,
    pending: VecDeque<u8>,
    timeout: Duration,
}

impl Read for Port {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pending.is_empty() {
            match self.from_chip.recv_timeout(self.timeout) {
                Ok(bytes) => self.pending.extend(bytes),
                Err(RecvTimeoutError::Timeout) => {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "no data"));
                }
                Err(RecvTimeoutError::Disconnected) => return Ok(0),
            }
        }
        let count = buf.len().min(self.pending.len());
        for slot in &mut buf[..count] {
            *slot = self.pending.pop_front().expect("count bytes are queued");
        }
        Ok(count)
    }
}

impl Write for Port {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.to_chip.send(buf.to_vec()).map_err(|_| {
            io::Error::new(io::ErrorKind::BrokenPipe, "the chip's thread has ended")
        })?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl SerialLine for Port {
    fn set_dtr(&mut self, _on: bool) -> io::Result<()> {
        Ok(())
    }

    fn set_rts(&mut self, _on: bool) -> io::Result<()> {
        Ok(())
    }

    fn discard_input(&mut self) -> io::Result<()> {
        self.pending.clear();
        Ok(())
    }

    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        self.timeout = timeout;
        Ok(())
    }
}

/// A control client on a serial stream over `port`, as the command line opens one.
fn client_on(port: Port) -> ControlClient {
    let stream = SerialStream::new(port, "/dev/ttyACM0", WAIT).expect("the port takes its setup");
    ControlClient::from_stream(stream, None)
}

/// The chip's thread and the client's port.
struct Chip {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Chip {
    /// Starts a chip whose thread is `body`, and returns it with the client's port. The thread
    /// stops when the chip is dropped.
    fn start(
        body: impl FnOnce(Receiver<Vec<u8>>, Sender<Vec<u8>>, &AtomicBool) + Send + 'static,
    ) -> (Self, Port) {
        let (to_chip, from_client) = mpsc::channel();
        let (to_client, from_chip) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = thread::spawn(move || body(from_client, to_client, &thread_stop));
        let port = Port {
            to_chip,
            from_chip,
            pending: VecDeque::new(),
            timeout: WAIT,
        };
        (
            Self {
                stop,
                thread: Some(thread),
            },
            port,
        )
    }
}

impl Drop for Chip {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            // A chip thread that panicked has already failed the test that read its silence.
            let _ = thread.join();
        }
    }
}

/// A chip of the services' shared platform. Its profile names it `testchip`.
struct TestChip;

impl ChipProfile for TestChip {
    type Reason = u8;

    fn watchdog_reset() -> u8 {
        7
    }

    fn platform_name() -> &'static str {
        "testchip"
    }

    fn device_descriptor() -> DeviceDescriptor {
        DeviceDescriptor::hosted()
    }

    fn checkpoint_step_bytes() -> Option<usize> {
        None
    }
}

/// A hardware watchdog that is fed and does nothing.
struct Dog;

impl HardwareWatchdog for Dog {
    fn feed_interval(&self) -> TimeSpan {
        TimeSpan::new(Duration::from_secs(1))
    }

    fn feed(&mut self) {}
}

/// The chip's monotonic clock: real time from its start.
#[derive(Clone)]
struct Clock {
    origin: Instant,
}

impl MonotonicClock for Clock {
    fn now(&self) -> MonotonicInstant {
        MonotonicInstant::from_origin(self.origin.elapsed())
    }
}

/// The loop's wait: a short pause, then the next pass.
struct PacedWait;

impl RunnerWait for PacedWait {
    fn wake_token(&self) -> WakeToken {
        WakeToken::new(0)
    }

    fn wait_until(
        &mut self,
        _deadline: MonotonicInstant,
        _observed: WakeToken,
    ) -> Result<WakeReason, PortError> {
        thread::sleep(Duration::from_micros(200));
        Ok(WakeReason::Deadline)
    }
}

/// The process I/O of a program the chip does not have. Only its type is named: a loop without a
/// program holds no cycle.
struct NoIo;

impl ProcessIo for NoIo {
    fn read_inputs(&mut self, _image: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, _image: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_safe_outputs(&mut self, _image: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn driver_health(&self) -> Vec<IoDriverStatus> {
        Vec::new()
    }
}

/// The chip's end of the duplex as the resource loop's control link: the bytes that the client
/// wrote reach the services' line link as a USB controller's interrupt would hand them over, and
/// the reply bytes that the line link holds go back to the client.
struct DuplexLink {
    line: DeviceLineLink,
    from_client: Receiver<Vec<u8>>,
    to_client: Sender<Vec<u8>>,
    clock: Clock,
}

impl DuplexLink {
    fn receive(&mut self) {
        while let Ok(bytes) = self.from_client.try_recv() {
            let taken = self.line.receive(&bytes);
            assert_eq!(taken, bytes.len(), "the line link takes the small requests");
        }
    }

    fn transmit(&mut self) {
        let unsent = self.line.unsent().to_vec();
        if !unsent.is_empty() {
            self.line.mark_sent(unsent.len());
            // A client that has gone has no use for the reply.
            let _ = self.to_client.send(unsent);
        }
    }
}

impl ControlLink for DuplexLink {
    fn max_frame_len(&self) -> usize {
        DeviceLineLink::MAX_FRAME_LEN
    }

    fn poll_frame(&mut self, buffer: &mut [u8]) -> Option<LinkFrame> {
        self.receive();
        self.transmit();
        self.line.poll_frame(buffer, self.clock.now())
    }

    fn send_frame(&mut self, frame: &[u8]) -> Result<(), PortError> {
        self.line
            .send_frame(frame, self.clock.now())
            .map_err(|_| PortError { code: 1, device: 0 })?;
        self.transmit();
        Ok(())
    }

    fn sending(&self) -> bool {
        self.line.reply_in_flight()
    }
}

/// The chip of the services' resource loop, Stopped without a program, on its shared platform.
fn runner_chip(from_client: Receiver<Vec<u8>>, to_client: Sender<Vec<u8>>, stop: &AtomicBool) {
    let clock = Clock {
        origin: Instant::now(),
    };
    let link = DuplexLink {
        line: DeviceLineLink::new(PeerClass::Local),
        from_client,
        to_client,
        clock: clock.clone(),
    };
    let platform = ChipPlatform::<TestChip, Dog>::new(
        Dog,
        SmolStr::new("Controller"),
        status_settings(),
        false,
        ControlMode::Production,
        EngineLiveValues::empty(),
        LastReset {
            cause: ResetCauseName::PowerOn,
            text: SmolStr::default(),
            boot_count: 1,
        },
        SmolStr::new("testchip-0001"),
    );
    let mut runner = Runner::<_, _, EngineCycle<'static, Clock, NoIo>, _, _>::stopped(
        link,
        PacedWait,
        platform,
        clock,
        SmolStr::new("no program is installed"),
        100,
    );
    while !stop.load(Ordering::Relaxed) {
        runner.run_pass().expect("the loop's wait does not fail");
    }
}

#[test]
fn a_serial_endpoint_round_trips_status_through_the_shared_runner() {
    // Specification 34 PORT-05 and ARCH-12: the control protocol is the same on a chip's USB link
    // as on a socket. Here the shared client sends `status` and `status` again over the serial
    // stream to the services' resource loop of a chip without a program (STATE-08): the loop
    // serves the link Stopped, reports its platform (specification 11, `status`), its resource,
    // its runtime ID and `keeps_control` (STATE-08), and answers each request with its id.
    // Wrong behaviour: the framing of the client and of the line link disagree (a request that
    // is not read as a line, a reply that is not read to its line feed), the ids are not echoed,
    // or the chip's reply is not the protocol's.
    let (_chip, port) = Chip::start(runner_chip);
    let mut client = client_on(port);

    let first = client
        .exchange("status", None)
        .expect("the chip answers status");
    let second = client
        .exchange("status", None)
        .expect("the chip answers status again");

    for (response, id) in [(&first, 1), (&second, 2)] {
        assert_eq!(response["id"], json!(id));
        assert_eq!(response["ok"], json!(true), "{response}");
        let status = &response["result"];
        assert_eq!(status["state"], json!("stopped"), "{status}");
        assert_eq!(status["fault"], Value::Null);
        assert_eq!(status["resource"], json!("Controller"));
        assert_eq!(status["platform"], json!("testchip"));
        assert_eq!(status["runtime_id"], json!("testchip-0001"));
        assert_eq!(status["program_control"], json!("keeps_control"));
    }
}

/// The reply that the session holds in the second test: a response to request 1 of exactly 10,000
/// bytes, which is two parts of 4,096 bytes and a third of 1,808.
fn held_reply() -> Vec<u8> {
    let samples = format!("{}0123456", "0123456789".repeat(993));
    let text =
        format!(r#"{{"id":1,"ok":true,"result":{{"samples":"{samples}"}},"audit_id":"audit-1"}}"#);
    assert_eq!(text.len(), 10_000);
    text.into_bytes()
}

/// A chip that answers `status` with the head of `reply`, held in the services' own transfer
/// session, and serves `transfer.get` and `transfer.abort` from it, as the dispatcher does.
fn session_chip(
    reply: Vec<u8>,
) -> impl FnOnce(Receiver<Vec<u8>>, Sender<Vec<u8>>, &AtomicBool) + Send + 'static {
    move |from_client, to_client, _stop| {
        let mut session = TransferSession::new(1, 1 << 20, 1 << 20);
        let mut line = Vec::new();
        for bytes in from_client {
            for byte in bytes {
                if byte != b'\n' {
                    line.push(byte);
                    continue;
                }
                let request: Value = serde_json::from_slice(&std::mem::take(&mut line))
                    .expect("the client writes JSON request lines");
                let id = request["id"].clone();
                let params = &request["params"];
                let response = match request["type"].as_str().expect("a request type") {
                    "status" => {
                        let head = session
                            .hold_reply(reply.clone())
                            .expect("room for the reply");
                        json!({"id": id, "ok": true, "result": wire::reply_head(&head)})
                    }
                    "transfer.get" => {
                        let get = wire::get_request(params).expect("a readable get");
                        match session.get(get.transfer, get.offset, get.len) {
                            Ok(part) => {
                                json!({"id": id, "ok": true, "result": wire::get_reply(&part)})
                            }
                            Err(error) => {
                                json!({"id": id, "ok": false, "error": error.to_string()})
                            }
                        }
                    }
                    "transfer.abort" => {
                        let transfer = wire::transfer_request(params).expect("a transfer id");
                        match session.abort(transfer) {
                            Ok(()) => json!({"id": id, "ok": true, "result": wire::abort_reply()}),
                            Err(error) => {
                                json!({"id": id, "ok": false, "error": error.to_string()})
                            }
                        }
                    }
                    other => panic!("the chip was asked for {other}"),
                };
                if to_client
                    .send(format!("{response}\n").into_bytes())
                    .is_err()
                {
                    return;
                }
            }
        }
    }
}

#[test]
fn a_long_reply_is_read_whole_from_the_shared_transfer_session() {
    // Specification 34 PORT-05, "Reply head and get" and "The client: a reply longer than the
    // frame": the reply of 10,000 bytes is above the frame of 8,192, so the chip holds it whole in
    // its transfer session and answers with the head, `reply_transfer` `0000000100000001` (boot 1,
    // sequence 1, the first id of a boot). The client reads the reply with `transfer.get`, which
    // the session answers with the CRC-32 of each part, and checks the SHA-256 that the head gives.
    // Here the session and its JSON shapes are the services' own, so a shape that the client
    // reads differently from the chip's is found. Wrong behaviour: the client reads `data` or
    // `crc32` under other names, takes base64 with other padding, or hands on the head.
    let (_chip, port) = Chip::start(session_chip(held_reply()));
    let mut client = client_on(port);

    let response = client
        .exchange("status", None)
        .expect("the held reply is read whole");

    assert_eq!(
        response.to_string().len(),
        10_000,
        "the reply is as long as the chip held it"
    );
    assert_eq!(response["id"], json!(1));
    assert_eq!(response["audit_id"], json!("audit-1"));
    assert_eq!(
        response["result"]["samples"],
        json!(format!("{}0123456", "0123456789".repeat(993)))
    );
}
