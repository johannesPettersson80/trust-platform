//! The tests of `ctl install` (specification 11, 6.8.1; specification 34, PORT-05). The runtime is
//! scripted behind an in-memory stream, and its upload is the services' own transfer session, so
//! the install is checked by the rules a chip runs by. Every expected value is written as a literal
//! worked out from the specification; the digests were computed with `hashlib` and `zlib`.

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex, MutexGuard};

use trust_runtime_client::ByteStream;
use trust_runtime_services::transfer::{wire, Complete, Received, TransferSession};

use super::*;

/// The program identity that the scripted runtime answers `program.install` with. The command
/// prints what the reply gives, so none of these is a value it could work out itself.
const INSTALLED_SHA256: &str = "1111111111111111111111111111111111111111111111111111111111111111";

/// The SHA-256 of [`container`], computed with `hashlib`.
const CONTAINER_SHA256: &str = "0cd0bf930677960951dda8588edcb6b293c0c3b26ef3ba72cddff4ddfc6822c7";

/// A container of 10,000 bytes: two parts of 4,096 bytes and a third of 1,808.
fn container() -> Vec<u8> {
    (0..10_000_u32)
        .map(|index| u8::try_from(index % 251).expect("a remainder below 251 is a byte"))
        .collect()
}

/// A runtime that is stopped, behind an in-memory stream. Its upload is the services' own
/// transfer session, so the install is checked by the rules a chip runs by.
#[derive(Debug)]
struct Device {
    session: TransferSession,
    /// The container that `program.install` took.
    installed: Option<Vec<u8>>,
    /// The requests that came.
    requests: Vec<Value>,
    /// How many `transfer.part` requests came.
    parts: usize,
    /// The reply to the `transfer.part` request with this number is lost.
    lose_reply_of_part: Option<usize>,
    /// The replies to the `transfer.part` requests from this number on are lost.
    lose_replies_from_part: Option<usize>,
    /// The `part_bytes` that the reply to `transfer.begin` gives instead of 4,096.
    part_bytes: Option<u64>,
    /// The refusal of `program.install`: its text and its code.
    install_refusal: Option<(&'static str, &'static str)>,
    /// The states that `status` answers, in order; the last one repeats.
    states: VecDeque<&'static str>,
    /// The waits that the client set.
    timeouts: Vec<Duration>,
}

impl Device {
    fn new() -> Self {
        Self {
            session: TransferSession::new(1, 64 * 1024 * 1024, 1024 * 1024),
            installed: None,
            requests: Vec::new(),
            parts: 0,
            lose_reply_of_part: None,
            lose_replies_from_part: None,
            part_bytes: None,
            install_refusal: None,
            states: VecDeque::from(["running"]),
            timeouts: Vec::new(),
        }
    }

    /// The reply to `request`, or `None` for a reply that is lost.
    fn answer(&mut self, request: &Value) -> Option<Value> {
        self.requests.push(request.clone());
        let id = request["id"].clone();
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let result: Result<Value, (String, Option<&'static str>)> =
            match request["type"].as_str().expect("a request type") {
                "transfer.begin" => {
                    let begin = wire::begin_request(&params).expect("a readable begin");
                    self.session
                        .begin(begin.bytes, begin.digest)
                        .map(|transfer| {
                            let mut reply = wire::begin_reply(transfer);
                            if let Some(part_bytes) = self.part_bytes {
                                reply["part_bytes"] = json!(part_bytes);
                            }
                            reply
                        })
                        .map_err(|error| (error.to_string(), None))
                }
                "transfer.part" => {
                    self.parts += 1;
                    let part = wire::part_request(&params).expect("a readable part");
                    let result = self
                        .session
                        .part(part.transfer, part.offset, part.encoded, part.crc32)
                        .map(|received| wire::part_reply(next_offset(received)))
                        .map_err(|error| (error.to_string(), None));
                    let lost = self.lose_reply_of_part == Some(self.parts)
                        || self
                            .lose_replies_from_part
                            .is_some_and(|from| self.parts >= from);
                    if lost {
                        return None;
                    }
                    result
                }
                "program.install" => self.install(&params),
                "transfer.abort" => {
                    let transfer = wire::transfer_request(&params).expect("a transfer id");
                    self.session
                        .abort(transfer)
                        .map(|()| wire::abort_reply())
                        .map_err(|error| (error.to_string(), None))
                }
                "restart" => Ok(json!({})),
                "status" => {
                    let state = if self.states.len() > 1 {
                        self.states.pop_front().expect("a state is queued")
                    } else {
                        *self.states.front().expect("a state is queued")
                    };
                    Ok(json!({"state": state, "fault": "the watchdog expired"}))
                }
                other => panic!("the runtime was asked for {other}"),
            };
        Some(match result {
            Ok(result) => json!({"id": id, "ok": true, "result": result}),
            Err((error, None)) => json!({"id": id, "ok": false, "error": error}),
            Err((error, Some(code))) => {
                json!({"id": id, "ok": false, "error": error, "error_code": code})
            }
        })
    }

    /// `program.install`: a refusal before the upload is taken, as a runtime that is not
    /// stopped refuses it, or the install of the finished upload.
    fn install(&mut self, params: &Value) -> Result<Value, (String, Option<&'static str>)> {
        if let Some((text, code)) = self.install_refusal {
            return Err((text.to_string(), Some(code)));
        }
        let transfer = wire::transfer_request(params).expect("a transfer id");
        let container = self
            .session
            .take_complete(transfer)
            .map(memory_bytes)
            .map_err(|error| (error.to_string(), None))?;
        self.installed = Some(container);
        Ok(json!({
            "status": "installed",
            "sequence": 3,
            "program_sha256": INSTALLED_SHA256,
            "bytes": 777,
        }))
    }
}

fn lock(device: &Mutex<Device>) -> MutexGuard<'_, Device> {
    device.lock().expect("the device's lock is not poisoned")
}

/// The client's end of the stream to a `Device`: a request is answered when its line is
/// whole, and a read with nothing to read times out at once.
#[derive(Debug)]
struct Link {
    device: Arc<Mutex<Device>>,
    pending: Vec<u8>,
    outbox: VecDeque<u8>,
}

impl Read for Link {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.outbox.is_empty() {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "nothing to read"));
        }
        let count = buf.len().min(self.outbox.len());
        for slot in &mut buf[..count] {
            *slot = self.outbox.pop_front().expect("count bytes are queued");
        }
        Ok(count)
    }
}

impl Write for Link {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for &byte in buf {
            if byte != b'\n' {
                self.pending.push(byte);
                continue;
            }
            let line = std::mem::take(&mut self.pending);
            let request: Value =
                serde_json::from_slice(&line).expect("the client writes JSON requests");
            let reply = lock(&self.device).answer(&request);
            if let Some(reply) = reply {
                self.outbox.extend(format!("{reply}\n").into_bytes());
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl ByteStream for Link {
    fn set_timeout(&self, timeout: Duration) -> io::Result<()> {
        lock(&self.device).timeouts.push(timeout);
        Ok(())
    }
}

/// The waits of the tests: a socket's reply wait, the 120 seconds of the install, and no
/// pause between two questions about the state.
fn waits() -> Waits {
    Waits {
        reply: Duration::from_millis(750),
        install: Duration::from_mins(2),
        running: Duration::from_secs(10),
        poll: Duration::ZERO,
    }
}

/// Installs `container` into `device`, and returns the result and the device.
fn install_into(
    device: Device,
    container: &[u8],
    restart: Option<&str>,
    waits: &Waits,
) -> (anyhow::Result<InstallReport>, Arc<Mutex<Device>>) {
    let device = Arc::new(Mutex::new(device));
    let link = Link {
        device: Arc::clone(&device),
        pending: Vec::new(),
        outbox: VecDeque::new(),
    };
    let mut client = ControlClient::from_stream(link, None);
    let result = install(&mut client, container, restart, waits);
    (result, device)
}

/// A `transfer.part` request without its data, and the data decoded.
fn part_without_data(request: &Value) -> (Value, Vec<u8>) {
    let mut request = request.clone();
    let data = request["params"]["data"]
        .take()
        .as_str()
        .map(|text| STANDARD.decode(text).expect("canonical base64"))
        .expect("a part carries data");
    request["params"]
        .as_object_mut()
        .expect("params")
        .remove("data");
    (request, data)
}

/// The expected `transfer.part` request, without its data.
fn part(id: u64, offset: u64, crc32: &str) -> Value {
    json!({
        "id": id,
        "type": "transfer.part",
        "params": {"transfer": "0000000100000001", "offset": offset, "crc32": crc32},
    })
}

#[test]
fn ctl_install_uploads_in_parts_and_installs_the_container() {
    // Specification 11 6.8.1 and specification 34 PORT-05: `transfer.begin` carries the length
    // and the SHA-256 of the file; the parts follow in order, 4,096 bytes each and the rest in
    // the last (10,000 = 2 x 4,096 + 1,808), each with its base64 data and its CRC-32 (computed
    // with `zlib`); `program.install` names the transfer; and the command prints the identity
    // that the reply gives. The session that receives it is the services' own, which checks
    // each CRC-32 and the SHA-256 of the whole. Wrong behaviour: a part of another size or
    // offset, the CRC-32 of the base64 text, a SHA-256 of other bytes, a restart nobody asked
    // for, or an identity worked out by the command instead of the reply's.
    let container = container();
    let (result, device) = install_into(Device::new(), &container, None, &waits());

    let report = result.expect("a container that the runtime takes is installed");

    assert_eq!(
        report.render(),
        format!("installed sequence=3 program_sha256={INSTALLED_SHA256} bytes=777")
    );
    let device = lock(&device);
    assert_eq!(device.installed, Some(container.clone()));
    let [begin, first, second, last, committed] = device.requests.as_slice() else {
        panic!("expected five requests, got {:?}", device.requests);
    };
    assert_eq!(
        *begin,
        json!({
            "id": 1,
            "type": "transfer.begin",
            "params": {"bytes": 10_000, "sha256": CONTAINER_SHA256},
        })
    );
    for (request, expected, range) in [
        (first, part(2, 0, "d465f907"), 0..4_096),
        (second, part(3, 4_096, "891e3520"), 4_096..8_192),
        (last, part(4, 8_192, "867bb762"), 8_192..10_000),
    ] {
        let (without_data, data) = part_without_data(request);
        assert_eq!(without_data, expected);
        assert_eq!(data, container[range]);
    }
    assert_eq!(
        *committed,
        json!({
            "id": 5,
            "type": "program.install",
            "params": {"transfer": "0000000100000001"},
        })
    );
    assert_eq!(
        device.timeouts,
        [Duration::from_mins(2), Duration::from_millis(750)],
        "the reply to program.install is awaited 120 seconds, then the ordinary wait returns"
    );
}

#[test]
fn ctl_install_resends_a_part_whose_reply_was_lost() {
    // Specification 11 6.8.1: a part whose reply does not arrive in time is sent again, and
    // specification 34 PORT-05: the repeat of a received part is acknowledged. The chip took
    // the second part and its reply was lost; the client sends the same part again, gets the
    // acknowledgement and goes on. Wrong behaviour: the install fails at the lost reply; the
    // client goes on with the next part without the acknowledgement; or it sends the repeat
    // with other bytes or another id of the part.
    let container = container();
    let mut device = Device::new();
    device.lose_reply_of_part = Some(2);
    let (result, device) = install_into(device, &container, None, &waits());

    let report = result.expect("a lost reply is no failure when the repeat is acknowledged");

    assert_eq!(report.installed.sequence, 3);
    let device = lock(&device);
    assert_eq!(device.installed, Some(container));
    let sequence: Vec<(String, Option<u64>)> = device
        .requests
        .iter()
        .map(|request| {
            (
                request["type"].as_str().expect("a type").to_string(),
                request["params"]["offset"].as_u64(),
            )
        })
        .collect();
    assert_eq!(
        sequence,
        [
            ("transfer.begin".to_string(), None),
            ("transfer.part".to_string(), Some(0)),
            ("transfer.part".to_string(), Some(4_096)),
            ("transfer.part".to_string(), Some(4_096)),
            ("transfer.part".to_string(), Some(8_192)),
            ("program.install".to_string(), None),
        ]
    );
    let (lost, repeat) = (&device.requests[2], &device.requests[3]);
    assert_eq!(lost["params"], repeat["params"], "the same part again");
    assert_eq!(
        (lost["id"].as_u64(), repeat["id"].as_u64()),
        (Some(3), Some(4))
    );
}

#[test]
fn ctl_install_sends_a_part_three_times_at_most() {
    // Specification 11 6.8.1: a part is sent again at most twice, so three times in all. A
    // runtime that never answers the second part ends the install with the failure of that
    // part, and the upload is freed. Wrong behaviour: the command waits for ever, or ends
    // after one attempt, or leaves the upload open.
    let mut device = Device::new();
    device.lose_replies_from_part = Some(2);
    let (result, device) = install_into(device, &container(), None, &waits());

    let error = result.expect_err("a part that is never answered ends the install");

    assert_eq!(
        error.to_string(),
        "transfer.part at offset 4096 failed: nothing to read"
    );
    let device = lock(&device);
    let types: Vec<&str> = device
        .requests
        .iter()
        .map(|request| request["type"].as_str().expect("a type"))
        .collect();
    assert_eq!(
        types,
        [
            "transfer.begin",
            "transfer.part",
            "transfer.part",
            "transfer.part",
            "transfer.part",
            "transfer.abort"
        ],
        "the first part, three attempts at the second, then the abort"
    );
}

#[test]
fn ctl_install_frees_the_upload_when_the_runtime_refuses_the_install() {
    // Specification 11 6.8.1: when a step after `transfer.begin` fails, the command sends
    // `transfer.abort`, and reports the runtime's text and code. The runtime refuses the
    // install as one that is not stopped does (specification 34 section 11.2); its upload
    // stays open until it is aborted, so the next `transfer.begin` would be refused with
    // `transfer busy`. Wrong behaviour: no abort, or the runtime's text lost.
    let mut device = Device::new();
    device.install_refusal = Some(("resource is running, stop it first", "resource_not_running"));
    let (result, device) = install_into(device, &container(), None, &waits());

    let error = result.expect_err("a refused install fails");

    assert_eq!(
        error.to_string(),
        "program.install failed: resource is running, stop it first (resource_not_running)"
    );
    let mut device = lock(&device);
    assert_eq!(
        device
            .requests
            .last()
            .map(|request| request["type"].clone()),
        Some(json!("transfer.abort"))
    );
    assert_eq!(device.installed, None);
    assert!(
        device.session.begin(10, [0; 32]).is_ok(),
        "no upload is left open"
    );
}

#[test]
fn ctl_install_refuses_a_part_size_it_cannot_use() {
    // Specification 34 PORT-05: every part is 4,096 bytes but the last. A `part_bytes` of 0
    // would send no part, and one above 4,096 would send parts the frame of a chip cannot
    // carry. Wrong behaviour: a part size of 0 loops or panics, or one above 4,096 is used.
    for part_bytes in [0, 4_097] {
        let mut device = Device::new();
        device.part_bytes = Some(part_bytes);
        let (result, device) = install_into(device, &container(), None, &waits());

        let error = result.expect_err("an unusable part size ends the install");

        assert_eq!(
            error.to_string(),
            "transfer.begin: part_bytes is not a whole number from 1 to 4096"
        );
        let device = lock(&device);
        assert_eq!(
            device.requests.len(),
            2,
            "begin, then the abort that frees the upload"
        );
    }
}

#[test]
fn ctl_install_restart_starts_the_program_and_waits_for_running() {
    // Specification 11 6.8.1: with `--restart` the command sends `restart {mode}` and then
    // `status` until the state is `running`, and prints the mode and the state. The runtime
    // says `stopped` once, so the command asks twice. Wrong behaviour: no restart request,
    // another mode, no wait (the state `stopped` is printed as started), or a wait for ever.
    let mut device = Device::new();
    device.states = VecDeque::from(["stopped", "running"]);
    let (result, device) = install_into(device, &container(), Some("warm"), &waits());

    let report = result.expect("a program that runs after the restart is a success");

    assert_eq!(
        report.render(),
        format!(
            "installed sequence=3 program_sha256={INSTALLED_SHA256} bytes=777\n\
             restarted mode=warm state=running"
        )
    );
    let device = lock(&device);
    let after_install: Vec<&Value> = device.requests.iter().skip(5).collect();
    assert_eq!(
        after_install,
        [
            &json!({"id": 6, "type": "restart", "params": {"mode": "warm"}}),
            &json!({"id": 7, "type": "status"}),
            &json!({"id": 8, "type": "status"}),
        ]
    );
}

#[test]
fn ctl_install_stops_waiting_when_the_program_faults() {
    // Specification 11 6.8.1: a `faulted` state ends the wait at once and fails with the
    // fault. Wrong behaviour: the command waits the 10 seconds for a program that cannot run.
    let mut device = Device::new();
    device.states = VecDeque::from(["faulted"]);
    let (result, device) = install_into(device, &container(), Some("cold"), &waits());

    let error = result.expect_err("a program that faults on start is a failure");

    assert_eq!(
        error.to_string(),
        "the installed program faulted when it started: the watchdog expired"
    );
    assert_eq!(
        lock(&device).requests.len(),
        7,
        "one status after the restart"
    );
}

#[test]
fn ctl_install_gives_up_when_the_program_does_not_run() {
    // Specification 11 6.8.1: the wait for `running` ends after its limit, here none, with
    // the state the runtime reports. Wrong behaviour: the command waits for ever.
    let mut device = Device::new();
    device.states = VecDeque::from(["stopped"]);
    let mut impatient = waits();
    impatient.running = Duration::ZERO;
    let (result, _) = install_into(device, &container(), Some("cold"), &impatient);

    let error = result.expect_err("a program that does not run is a failure");

    assert_eq!(
        error.to_string(),
        "the program is not running 0 seconds after the restart: its state is stopped"
    );
}

/// The next offset of a part that an in-memory session took.
fn next_offset(received: Received) -> usize {
    let Received::Next(next) = received else {
        panic!("an upload held in memory takes each part at once");
    };
    next
}

/// The bytes of an upload that an in-memory session held.
fn memory_bytes(complete: Complete) -> Vec<u8> {
    let Complete::Memory(bytes, _) = complete else {
        panic!("an upload held in memory completes in memory");
    };
    bytes
}
