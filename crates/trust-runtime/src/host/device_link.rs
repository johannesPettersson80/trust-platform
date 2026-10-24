//! A device's control link from the engineering computer (specification 34 PORT-05): one request
//! at a time over the device's serial port, or over TCP to a bridge to that port on another
//! computer. On it the conformance suite and the test runner run stepped scans (`cycle.step`,
//! specification 11 §6.8.1; specification 44 §14.2): they stop the device, install a program and
//! start the idle program whose record turns the debug surface on.

use std::fmt;
use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde_json::{json, Value};

use crate::native::Target;

/// How long a reply to a request other than `program.install` may take: a chip's watchdog interval
/// of four seconds bounds the pass that reads the request, and the record's watchdog of one second
/// bounds a stepped scan (specification 11 §6.8.1, "Scriptable control client").
pub const REPLY_WAIT: Duration = Duration::from_secs(5);

/// How long a device may take to report the state a request moved it to.
const STATE_WAIT: Duration = Duration::from_secs(10);

/// The bytes of one `transfer.part` (specification 34, PORT-05).
const PART_BYTES: usize = 4096;

/// The longest line a device may send; its frames are far shorter.
const LINE_LIMIT: usize = 1 << 20;

/// How long one read of the link waits for bytes.
const READ_WAIT: Duration = Duration::from_millis(20);

/// The configuration a program for stepped scans is installed with: the debug surface on, which
/// `cycle.step` needs, a watchdog of one second, which bounds every stepped scan, and otherwise the
/// sections a project's `runtime.toml` must have, at their defaults.
const STEPPING_RUNTIME: &str = "\
[bundle]
version = 1

[resource]
name = \"Stepping\"
cycle_interval_ms = 10

[runtime.control]
endpoint = \"unix:///tmp/trust-stepping.sock\"
mode = \"production\"
debug_enabled = true

[runtime.log]
level = \"info\"

[runtime.retain]
mode = \"none\"
save_interval_ms = 1000

[runtime.watchdog]
enabled = true
timeout_ms = 1000
action = \"halt\"

[runtime.fault]
policy = \"halt\"
";

/// The program a run of stepped scans starts first, to load a record whose debug surface is on: it
/// runs no statement, so it cannot fault.
const IDLE_PROGRAM: &str = "PROGRAM SteppingIdle\nEND_PROGRAM\n";

/// Why a request over the link, or the preparation of a program for it, failed.
#[derive(Debug)]
pub enum DeviceLinkError {
    /// The port or the bridge could not be opened.
    Open(String),
    /// The link failed to send or to receive.
    Io(String),
    /// The device refused the request with this error.
    Refused {
        /// The request's type.
        kind: String,
        /// The device's error text.
        error: String,
    },
    /// No reply came within the wait.
    Timeout {
        /// The request's type.
        kind: String,
        /// How long the request waited.
        wait: Duration,
    },
    /// The link closed while a request waited for its reply.
    Closed {
        /// The request's type.
        kind: String,
    },
    /// The device sent a line longer than any reply.
    LineTooLong,
    /// The device answered, but not as the request needs.
    Device(String),
    /// A program for the device could not be built.
    Build(String),
}

impl fmt::Display for DeviceLinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open(reason) | Self::Io(reason) | Self::Device(reason) | Self::Build(reason) => {
                formatter.write_str(reason)
            }
            Self::Refused { kind, error } => write!(formatter, "{kind} refused: {error}"),
            Self::Timeout { kind, wait } => {
                write!(
                    formatter,
                    "no reply to {kind} within {} s",
                    wait.as_secs_f64()
                )
            }
            Self::Closed { kind } => {
                write!(
                    formatter,
                    "the device link closed while {kind} waited for its reply"
                )
            }
            Self::LineTooLong => {
                write!(
                    formatter,
                    "the device sent a line longer than {LINE_LIMIT} bytes"
                )
            }
        }
    }
}

impl std::error::Error for DeviceLinkError {}

/// The byte stream to a device.
trait Stream: Read + Write {}

impl<T: Read + Write> Stream for T {}

/// A device's control link: newline-delimited JSON, one request at a time, with the device's own
/// log lines between the replies passed over (specification 34, PORT-05).
pub struct DeviceLink {
    port: Box<dyn Stream>,
    buffer: Vec<u8>,
    next_id: u64,
}

impl DeviceLink {
    /// Opens `port`: a serial port, opened without resetting the device (the data terminal ready
    /// line on and the request to send line off, as the USB serial link of an ESP32 reads a reset
    /// from the other combinations), or `tcp://<host>:<port>`, a bridge to the device's serial port
    /// on another computer.
    ///
    /// # Errors
    ///
    /// [`DeviceLinkError::Open`] when the port or the bridge cannot be opened.
    pub fn open(port: &str) -> Result<Self, DeviceLinkError> {
        let open = |reason: String| DeviceLinkError::Open(format!("open '{port}': {reason}"));
        let stream: Box<dyn Stream> = if let Some(address) = port.strip_prefix("tcp://") {
            let stream =
                std::net::TcpStream::connect(address).map_err(|error| open(error.to_string()))?;
            stream
                .set_read_timeout(Some(READ_WAIT))
                .and_then(|()| stream.set_nodelay(true))
                .map_err(|error| open(error.to_string()))?;
            Box::new(stream)
        } else {
            let mut serial = serialport::new(port, 115_200)
                .timeout(READ_WAIT)
                .dtr_on_open(true)
                .open()
                .map_err(|error| open(error.to_string()))?;
            serial
                .write_request_to_send(false)
                .map_err(|error| open(error.to_string()))?;
            Box::new(serial)
        };
        Ok(Self {
            port: stream,
            buffer: Vec::new(),
            next_id: 1,
        })
    }

    /// Sends request `kind` with `params` and returns the result of its reply, which must come
    /// within `wait`.
    ///
    /// # Errors
    ///
    /// The [`DeviceLinkError`] of a link that failed, of a reply that did not come, or the device's
    /// refusal.
    pub fn call(
        &mut self,
        kind: &str,
        params: Option<Value>,
        wait: Duration,
    ) -> Result<Value, DeviceLinkError> {
        let id = self.next_id;
        self.next_id += 1;
        let mut request = json!({"id": id, "type": kind});
        if let Some(params) = params {
            request["params"] = params;
        }
        let mut line = request.to_string().into_bytes();
        line.push(b'\n');
        self.port
            .write_all(&line)
            .and_then(|()| self.port.flush())
            .map_err(|error| DeviceLinkError::Io(format!("send {kind}: {error}")))?;
        let deadline = Instant::now() + wait;
        let mut chunk = [0u8; 4096];
        loop {
            while let Some(end) = self.buffer.iter().position(|byte| *byte == b'\n') {
                let line: Vec<u8> = self.buffer.drain(..=end).collect();
                let text = String::from_utf8_lossy(&line);
                let Some(start) = text.find("{\"id\"") else {
                    continue;
                };
                let Ok(reply) = serde_json::from_str::<Value>(text[start..].trim_end()) else {
                    continue;
                };
                if reply.get("id").and_then(Value::as_u64) != Some(id) {
                    continue;
                }
                if reply.get("ok").and_then(Value::as_bool) == Some(true) {
                    return Ok(reply.get("result").cloned().unwrap_or_default());
                }
                return Err(DeviceLinkError::Refused {
                    kind: kind.to_string(),
                    error: reply
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("no error text")
                        .to_string(),
                });
            }
            if Instant::now() >= deadline {
                return Err(DeviceLinkError::Timeout {
                    kind: kind.to_string(),
                    wait,
                });
            }
            match self.port.read(&mut chunk) {
                Ok(0) => {
                    return Err(DeviceLinkError::Closed {
                        kind: kind.to_string(),
                    });
                }
                Ok(read) => self.buffer.extend_from_slice(&chunk[..read]),
                // A read that waited for nothing: a serial port's timeout, a socket's would-block.
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) => {}
                Err(error) => {
                    return Err(DeviceLinkError::Io(format!(
                        "read the reply to {kind}: {error}"
                    )));
                }
            }
            if self.buffer.len() > LINE_LIMIT {
                return Err(DeviceLinkError::LineTooLong);
            }
        }
    }

    /// The resource state the device reports.
    ///
    /// # Errors
    ///
    /// The [`DeviceLinkError`] of the `status` request.
    pub fn state(&mut self) -> Result<String, DeviceLinkError> {
        let status = self.call("status", None, REPLY_WAIT)?;
        status
            .get("state")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| DeviceLinkError::Device("status named no state".to_string()))
    }

    /// Waits until the device reports `wanted`.
    ///
    /// # Errors
    ///
    /// [`DeviceLinkError::Device`] when another state lasts longer than ten seconds.
    pub fn wait_for(&mut self, wanted: &str) -> Result<(), DeviceLinkError> {
        let deadline = Instant::now() + STATE_WAIT;
        loop {
            let state = self.state()?;
            if state == wanted {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(DeviceLinkError::Device(format!(
                    "the device is {state}, not {wanted}"
                )));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Leaves the device Stopped: `shutdown` stops a running, paused or faulted program
    /// (specification 34 STATE-08), and a faulted one is never started again on the way.
    ///
    /// # Errors
    ///
    /// The [`DeviceLinkError`] of a request, or of a device that does not stop: one whose stop
    /// failed stays faulted with the stop's error.
    pub fn stop(&mut self) -> Result<(), DeviceLinkError> {
        if self.state()? == "stopped" {
            return Ok(());
        }
        self.call("shutdown", None, REPLY_WAIT)?;
        self.wait_for("stopped")
    }

    /// Installs `container` on the device, which it leaves Stopped (specification 34 §11.2): the
    /// transfer in parts, then `program.install`, whose reply comes once the device has committed the
    /// container. The next cold restart of a step loads the program.
    ///
    /// # Errors
    ///
    /// The [`DeviceLinkError`] of a request.
    pub fn install(&mut self, container: &[u8]) -> Result<(), DeviceLinkError> {
        self.stop()?;
        let digest = trust_runtime_core::deployment::sha256(container);
        let begun = self.call(
            "transfer.begin",
            Some(json!({"bytes": container.len(), "sha256": hex(&digest)})),
            REPLY_WAIT,
        )?;
        let transfer = begun
            .get("transfer")
            .cloned()
            .ok_or_else(|| DeviceLinkError::Device("transfer.begin named no transfer".into()))?;
        for (index, part) in container.chunks(PART_BYTES).enumerate() {
            self.call(
                "transfer.part",
                Some(json!({
                    "transfer": transfer,
                    "offset": index * PART_BYTES,
                    "data": base64::engine::general_purpose::STANDARD.encode(part),
                    "crc32": format!("{:08x}", trust_runtime_core::crc32::checksum(part)),
                })),
                REPLY_WAIT,
            )?;
        }
        self.call(
            "program.install",
            Some(json!({"transfer": transfer})),
            install_wait(container.len()),
        )?;
        Ok(())
    }

    /// Prepares the device for stepped scans: a device takes its debug surface from the record of the
    /// program it has loaded (specification 34 §5.1), and `cycle.step` needs it on, so the idle
    /// program, compiled for `target` with `toolchain` in folder `work`, is installed, started and
    /// stopped.
    ///
    /// # Errors
    ///
    /// The [`DeviceLinkError`] of the idle program's build or of a request.
    pub fn prepare_steps(
        &mut self,
        target: Target,
        toolchain: &crate::native::Toolchain,
        work: &Path,
    ) -> Result<(), DeviceLinkError> {
        let mut built = crate::native::build(
            &[crate::harness::SourceFile::new(IDLE_PROGRAM)],
            &crate::native::Options {
                target,
                hooks: false,
            },
            toolchain,
            &work.join("native"),
        )
        .map_err(|error| DeviceLinkError::Build(format!("the idle program: {error}")))?;
        // A device admits what a key it trusts signed: the user's own (specification 44 §5.11).
        crate::native::user_key()
            .and_then(|key| crate::native::sign(&mut built.image, &key))
            .map_err(|error| DeviceLinkError::Build(format!("the idle program: {error}")))?;
        let container = stepping_container(&built.image, work)?;
        self.install(&container)?;
        self.call("restart", Some(json!({"mode": "cold"})), REPLY_WAIT)?;
        self.wait_for("running")?;
        self.stop()
    }
}

/// The container of program image `image` for stepped scans, with the deployment record of a project
/// whose debug surface is on; `work` is a folder the build may write to.
///
/// # Errors
///
/// [`DeviceLinkError::Build`] when the record or the container cannot be written.
pub fn stepping_container(image: &[u8], work: &Path) -> Result<Vec<u8>, DeviceLinkError> {
    let build = |error: String| DeviceLinkError::Build(error);
    std::fs::create_dir_all(work)
        .and_then(|()| std::fs::write(work.join("runtime.toml"), STEPPING_RUNTIME))
        .map_err(|error| build(error.to_string()))?;
    let record =
        crate::deployment::image_record(work, image).map_err(|error| build(error.to_string()))?;
    trust_runtime_services::installation::encode_image(
        trust_runtime_services::installation::FIRST_SEQUENCE,
        image,
        &record,
    )
    .map_err(|error| build(error.to_string()))
}

/// How long the reply to `program.install` of a container of `bytes` may take (specification 11
/// §6.8.1): T = 4 + 0.4 × ⌈L / 4096⌉ + 0.01 × ⌈L / 256⌉ seconds.
#[must_use]
pub fn install_wait(bytes: usize) -> Duration {
    let blocks = u32::try_from(bytes.div_ceil(4096)).unwrap_or(u32::MAX);
    let lines = u32::try_from(bytes.div_ceil(256)).unwrap_or(u32::MAX);
    Duration::from_secs(4) + Duration::from_millis(400) * blocks + Duration::from_millis(10) * lines
}

/// The lowercase hexadecimal text of `bytes`.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Specification 11 §6.8.1: the wait for the reply to an install of L bytes is
    /// 4 + 0.4 × ⌈L/4096⌉ + 0.01 × ⌈L/256⌉ seconds. For the C6's 83,499-byte pasteurizer that is
    /// 4 + 0.4 × 21 + 0.01 × 327 = 15.67 s. Catches a wait that rounds the blocks down, which a
    /// container just over a block's size would time out by.
    #[test]
    fn the_install_wait_follows_the_specifications_formula() {
        assert_eq!(install_wait(83_499), Duration::from_millis(15_670));
        assert_eq!(
            install_wait(4_097),
            Duration::from_millis(4_000 + 800 + 170)
        );
    }
}
