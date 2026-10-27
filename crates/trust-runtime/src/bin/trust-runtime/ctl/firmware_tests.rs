//! The tests of `ctl firmware` (specification 34 TARGET-04; specification 11, the firmware update
//! contract). The chip is scripted behind an in-memory stream with the services' own transfer
//! session, and it reports the firmware facts of `capabilities` that a chip reports before and after
//! its restart. Every expected value is a literal worked out from the specification.

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex, MutexGuard};

use trust_runtime_client::ByteStream;
use trust_runtime_services::transfer::{wire, Received, TransferSession};

use super::*;

/// The facts of a chip that runs `version` from `slot`.
fn facts(slot: &str, version: &str, rolled_back: Option<&str>) -> Value {
    json!({"slot": slot, "version": version, "confirmed": true, "rolled_back": rolled_back})
}

/// A chip behind an in-memory stream: `capabilities` answers `firmware`, which becomes `restarted`
/// once `firmware.install` has answered, as the chip resets into the new firmware.
#[derive(Debug)]
struct Device {
    session: TransferSession,
    state: &'static str,
    firmware: Value,
    restarted: Value,
    requests: Vec<Value>,
}

impl Device {
    fn new(firmware: Value, restarted: Value) -> Self {
        Self {
            session: TransferSession::new(1, 4 * 1024 * 1024, 1024 * 1024),
            state: "stopped",
            firmware,
            restarted,
            requests: Vec::new(),
        }
    }

    fn answer(&mut self, request: &Value) -> Value {
        self.requests.push(request.clone());
        let id = request["id"].clone();
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let result: Result<Value, String> = match request["type"].as_str().expect("a type") {
            "capabilities" => Ok(json!({"firmware": self.firmware})),
            "status" => Ok(json!({"state": self.state})),
            "transfer.begin" => {
                let begin = wire::begin_request(&params).expect("a readable begin");
                assert!(begin.firmware, "the upload is a firmware upload: {params}");
                self.session
                    .begin(begin.bytes, begin.digest)
                    .map(wire::begin_reply)
                    .map_err(|error| error.to_string())
            }
            "transfer.part" => {
                let part = wire::part_request(&params).expect("a readable part");
                self.session
                    .part(part.transfer, part.offset, part.encoded, part.crc32)
                    .map(|received| wire::part_reply(next_offset(received)))
                    .map_err(|error| error.to_string())
            }
            "firmware.install" => {
                let transfer = wire::transfer_request(&params).expect("a transfer id");
                match self.session.take_complete(transfer) {
                    Ok(_) => {
                        self.firmware = self.restarted.clone();
                        Ok(
                            json!({"status": "installed", "version": "0.24.76", "slot": "b",
                            "restarting": true}),
                        )
                    }
                    Err(error) => Err(error.to_string()),
                }
            }
            "transfer.abort" => Ok(json!({})),
            other => panic!("the chip was asked for {other}"),
        };
        match result {
            Ok(result) => json!({"id": id, "ok": true, "result": result}),
            Err(error) => json!({"id": id, "ok": false, "error": error}),
        }
    }
}

fn lock(device: &Mutex<Device>) -> MutexGuard<'_, Device> {
    device.lock().expect("the device's lock is not poisoned")
}

/// The client's end of the stream to a `Device`.
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
            let request: Value = serde_json::from_slice(&line).expect("a JSON request");
            let reply = lock(&self.device).answer(&request);
            self.outbox.extend(format!("{reply}\n").into_bytes());
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl ByteStream for Link {
    fn set_timeout(&self, _timeout: Duration) -> io::Result<()> {
        Ok(())
    }
}

/// Updates `device` with a 9,000-byte image and returns the result, the progress lines and the
/// device.
fn update_on(
    device: Device,
) -> (
    anyhow::Result<UpdateReport>,
    Vec<String>,
    Arc<Mutex<Device>>,
) {
    let device = Arc::new(Mutex::new(device));
    let image: Vec<u8> = (0..9_000_u32)
        .map(|index| u8::try_from(index % 7).expect("a remainder below 7 is a byte"))
        .collect();
    let shared = Arc::clone(&device);
    let connect = move || {
        let link = Link {
            device: Arc::clone(&shared),
            pending: Vec::new(),
            outbox: VecDeque::new(),
        };
        Ok((
            ControlClient::from_stream(link, None),
            Duration::from_millis(750),
        ))
    };
    let mut lines = Vec::new();
    let result = update(
        connect,
        &image,
        Duration::from_secs(5),
        Duration::ZERO,
        |line| lines.push(line.to_string()),
    );
    (result, lines, device)
}

fn types(device: &Arc<Mutex<Device>>) -> Vec<String> {
    lock(device)
        .requests
        .iter()
        .map(|request| request["type"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// The update reads the chip's firmware and state, uploads the image in three parts with `"for":
/// "firmware"`, installs it, and after the restart reads that the chip runs 0.24.76 from slot b,
/// confirmed. Catches an upload sent as a program, and an update that reports success without asking
/// the restarted chip what it runs.
#[test]
fn an_update_installs_the_image_and_waits_for_the_chip_to_run_it() {
    let (result, lines, device) = update_on(Device::new(
        facts("a", "0.24.75", None),
        facts("b", "0.24.76", None),
    ));
    let report = result.expect("the update succeeds");
    assert_eq!(report.render(), "running version=0.24.76 slot=b confirmed");
    assert_eq!(
        lines,
        vec![
            "installed version=0.24.76 slot=b; the runtime restarts into it (it ran 0.24.75 from \
             slot a)"
        ]
    );
    assert_eq!(
        types(&device),
        vec![
            "capabilities",
            "status",
            "transfer.begin",
            "transfer.part",
            "transfer.part",
            "transfer.part",
            "firmware.install",
            "capabilities",
        ]
    );
}

/// A chip whose new firmware did not finish its boot runs the old one again and reports the version
/// it rolled back: the update fails and names both. Catches a rollback reported as success.
#[test]
fn a_rolled_back_update_fails_with_both_versions() {
    let (result, _, _) = update_on(Device::new(
        facts("a", "0.24.75", None),
        facts("a", "0.24.75", Some("0.24.76")),
    ));
    assert_eq!(
        result.expect_err("the update fails").to_string(),
        "the runtime rolled the update back: truST 0.24.76 did not finish its boot, and the runtime \
         runs truST 0.24.75 from slot a again"
    );
}

/// A runtime without firmware slots, a computer, and a chip whose program runs are refused before
/// anything is uploaded. Catches an upload that a computer refuses only after the whole image, and a
/// firmware written while the control program runs.
#[test]
fn an_update_is_refused_before_any_upload_where_it_cannot_run() {
    let (result, _, device) = update_on(Device::new(Value::Null, Value::Null));
    assert_eq!(
        result.expect_err("refused").to_string(),
        "this runtime keeps no firmware slots: truST on a computer is updated by installing truST \
         on that computer"
    );
    assert_eq!(types(&device), vec!["capabilities"]);

    let mut running = Device::new(facts("a", "0.24.75", None), Value::Null);
    running.state = "running";
    let (result, _, device) = update_on(running);
    assert_eq!(
        result.expect_err("refused").to_string(),
        "the runtime's program is running: stop it first (trust-runtime ctl shutdown), then update \
         its firmware"
    );
    assert_eq!(types(&device), vec!["capabilities", "status"]);
}

/// The next offset of a part that an in-memory session took.
fn next_offset(received: Received) -> usize {
    let Received::Next(next) = received else {
        panic!("an upload held in memory takes each part at once");
    };
    next
}
