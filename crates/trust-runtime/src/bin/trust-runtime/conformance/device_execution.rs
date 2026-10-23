/// How long a reply to a request other than `program.install` may take: a chip's watchdog interval
/// of four seconds bounds the pass that reads the request, and a step's scan is bounded by the
/// case's watchdog (specification 11 §6.8.1, "Scriptable control client").
const DEVICE_REPLY_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// How long a device may take to report Stopped after `shutdown`.
const DEVICE_STOP_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// The bytes of one `transfer.part` (specification 34, PORT-05).
const DEVICE_PART_BYTES: usize = 4096;

/// The longest line a device may send; its frames are far shorter.
const DEVICE_LINE_LIMIT: usize = 1 << 20;

/// The configuration a case's program is installed with: the debug surface on, which `cycle.step`
/// needs (specification 11 §6.8.1), a watchdog of one second, which bounds every stepped scan, and
/// otherwise the sections a project's `runtime.toml` must have, at their defaults.
const DEVICE_CASE_RUNTIME: &str = "\
[bundle]
version = 1

[resource]
name = \"Conformance\"
cycle_interval_ms = 10

[runtime.control]
endpoint = \"unix:///tmp/trust-conformance.sock\"
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

/// The device the runtime cases run on (specification 44 §14.2): its control link, the target its
/// cases are compiled for and the toolchain that compiles them.
struct DeviceRun {
    link: DeviceLink,
    target: Target,
    toolchain: Toolchain,
}

impl DeviceRun {
    /// The device on serial port `port`, whose cases are compiled for `target` as
    /// `trust-runtime build --target` names it.
    ///
    /// A device takes its debug surface from the record of the program it has loaded
    /// (specification 34 §5.1), and `cycle.step` needs it on. The run therefore starts the idle
    /// program of [`DEVICE_IDLE_PROGRAM`], whose record turns it on, and stops it; each case is then
    /// loaded by its first step.
    fn open(port: &str, target: &str) -> anyhow::Result<Self> {
        let target = crate::build::native_target(target)?;
        let toolchain = Toolchain::from_env()
            .map_err(|error| anyhow!("the native backend needs its toolchain: {error}"))?;
        let mut device = Self {
            link: DeviceLink::open(port)?,
            target,
            toolchain,
        };
        let idle = device_container("idle", &[DEVICE_IDLE_PROGRAM.to_string()], &device)
            .map_err(|error| anyhow!("build the idle program: {error:#}"))?;
        device
            .link
            .install(&idle)
            .map_err(|error| anyhow!("install the idle program: {error:#}"))?;
        device
            .link
            .call("restart", Some(json!({"mode": "cold"})), DEVICE_REPLY_WAIT)?;
        device.link.wait_for("running")?;
        device.link.stop()?;
        Ok(device)
    }
}

/// The program a device run starts first, to load a record whose debug surface is on: it runs no
/// statement, so it cannot fault.
const DEVICE_IDLE_PROGRAM: &str = "PROGRAM ConformanceIdle\nEND_PROGRAM\n";

/// A device's control link over a serial port, one request at a time: newline-delimited JSON, with
/// the device's own log lines between the replies passed over (specification 34, PORT-05).
struct DeviceLink {
    port: Box<dyn DeviceStream>,
    buffer: Vec<u8>,
    next_id: u64,
}

/// The byte stream to a device: its serial port, or a TCP connection to a bridge to the port.
trait DeviceStream: std::io::Read + std::io::Write {}

impl<T: std::io::Read + std::io::Write> DeviceStream for T {}

/// How long one read of the link waits for bytes.
const DEVICE_READ_WAIT: std::time::Duration = std::time::Duration::from_millis(20);

impl DeviceLink {
    /// Opens `port`: a serial port, opened without resetting the device (the data terminal ready
    /// line on and the request to send line off, as the USB serial link of an ESP32 reads a reset
    /// from the other combinations), or `tcp://<host>:<port>`, a bridge to the device's serial port
    /// on another computer.
    fn open(port: &str) -> anyhow::Result<Self> {
        let stream: Box<dyn DeviceStream> = if let Some(address) = port.strip_prefix("tcp://") {
            let stream = std::net::TcpStream::connect(address)
                .with_context(|| format!("connect to the device bridge '{address}'"))?;
            stream.set_read_timeout(Some(DEVICE_READ_WAIT))?;
            stream.set_nodelay(true)?;
            Box::new(stream)
        } else {
            let mut serial = serialport::new(port, 115_200)
                .timeout(DEVICE_READ_WAIT)
                .dtr_on_open(true)
                .open()
                .with_context(|| format!("open the device link '{port}'"))?;
            serial
                .write_request_to_send(false)
                .context("release the request to send line")?;
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
    fn call(
        &mut self,
        kind: &str,
        params: Option<serde_json::Value>,
        wait: std::time::Duration,
    ) -> anyhow::Result<serde_json::Value> {
        use std::io::{Read as _, Write as _};
        let id = self.next_id;
        self.next_id += 1;
        let mut request = json!({"id": id, "type": kind});
        if let Some(params) = params {
            request["params"] = params;
        }
        let mut line = serde_json::to_vec(&request)?;
        line.push(b'\n');
        self.port
            .write_all(&line)
            .and_then(|()| self.port.flush())
            .with_context(|| format!("send {kind}"))?;
        let deadline = Instant::now() + wait;
        let mut chunk = [0u8; 4096];
        loop {
            while let Some(end) = self.buffer.iter().position(|byte| *byte == b'\n') {
                let line: Vec<u8> = self.buffer.drain(..=end).collect();
                let text = String::from_utf8_lossy(&line);
                let Some(start) = text.find("{\"id\"") else {
                    continue;
                };
                let Ok(reply) = serde_json::from_str::<serde_json::Value>(text[start..].trim_end())
                else {
                    continue;
                };
                if reply.get("id").and_then(serde_json::Value::as_u64) != Some(id) {
                    continue;
                }
                if reply.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
                    return Ok(reply.get("result").cloned().unwrap_or_default());
                }
                let error = reply
                    .get("error")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("no error text");
                bail!("{kind} refused: {error}");
            }
            if Instant::now() >= deadline {
                bail!("no reply to {kind} within {} s", wait.as_secs_f64());
            }
            match self.port.read(&mut chunk) {
                Ok(0) => bail!("the device link closed while {kind} waited for its reply"),
                Ok(read) => self.buffer.extend_from_slice(&chunk[..read]),
                // A read that waited for nothing: a serial port's timeout, a socket's would-block.
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) => {}
                Err(error) => {
                    return Err(error).with_context(|| format!("read the reply to {kind}"));
                }
            }
            if self.buffer.len() > DEVICE_LINE_LIMIT {
                bail!("the device sent a line longer than {DEVICE_LINE_LIMIT} bytes");
            }
        }
    }

    /// The resource state the device reports.
    fn state(&mut self) -> anyhow::Result<String> {
        let status = self.call("status", None, DEVICE_REPLY_WAIT)?;
        status
            .get("state")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| anyhow!("status named no state"))
    }

    /// Waits until the device reports `wanted`.
    fn wait_for(&mut self, wanted: &str) -> anyhow::Result<()> {
        let deadline = Instant::now() + DEVICE_STOP_WAIT;
        loop {
            let state = self.state()?;
            if state == wanted {
                return Ok(());
            }
            if Instant::now() >= deadline {
                bail!("the device is {state}, not {wanted}");
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    /// Leaves the device Stopped: a running program is shut down, and a faulted one, which takes
    /// nothing but a restart (specification 34 STATE-04), is restarted and then shut down.
    fn stop(&mut self) -> anyhow::Result<()> {
        match self.state()?.as_str() {
            "stopped" => return Ok(()),
            "faulted" => {
                self.call("restart", Some(json!({"mode": "cold"})), DEVICE_REPLY_WAIT)?;
            }
            _ => {}
        }
        self.call("shutdown", None, DEVICE_REPLY_WAIT)?;
        self.wait_for("stopped")
    }

    /// Installs `container` on the device, which it leaves Stopped (specification 34 §11.2): the
    /// transfer in parts, then `program.install`, whose reply comes once the device has committed
    /// the container. The next cold restart of a step loads the program.
    fn install(&mut self, container: &[u8]) -> anyhow::Result<()> {
        self.stop()?;
        let digest = trust_runtime_core::deployment::sha256(container);
        let begun = self.call(
            "transfer.begin",
            Some(json!({"bytes": container.len(), "sha256": hex(&digest)})),
            DEVICE_REPLY_WAIT,
        )?;
        let transfer = begun
            .get("transfer")
            .cloned()
            .ok_or_else(|| anyhow!("transfer.begin named no transfer"))?;
        for (index, part) in container.chunks(DEVICE_PART_BYTES).enumerate() {
            self.call(
                "transfer.part",
                Some(json!({
                    "transfer": transfer,
                    "offset": index * DEVICE_PART_BYTES,
                    "data": base64::engine::general_purpose::STANDARD.encode(part),
                    "crc32": format!("{:08x}", trust_runtime_core::crc32::checksum(part)),
                })),
                DEVICE_REPLY_WAIT,
            )?;
        }
        self.call(
            "program.install",
            Some(json!({"transfer": transfer})),
            install_wait(container.len()),
        )?;
        Ok(())
    }
}

/// How long the reply to `program.install` of a container of `bytes` may take (specification 11
/// §6.8.1): T = 4 + 0.4 × ⌈L / 4096⌉ + 0.01 × ⌈L / 256⌉ seconds.
fn install_wait(bytes: usize) -> std::time::Duration {
    let blocks = u32::try_from(bytes.div_ceil(4096)).unwrap_or(u32::MAX);
    let lines = u32::try_from(bytes.div_ceil(256)).unwrap_or(u32::MAX);
    std::time::Duration::from_secs(4)
        + std::time::Duration::from_millis(400) * blocks
        + std::time::Duration::from_millis(10) * lines
}

/// The lowercase hexadecimal text of `bytes`.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// The runtime case `case` run on the device (specification 44 §14.2): compiled into a program image
/// for the device and installed, then stepped cycle by cycle with `cycle.step` (specification 11
/// §6.8.1), each step with the restart, time, inputs and watches that the native runner takes for
/// that cycle, so that its trace compares with the same expected artifact.
fn execute_runtime_case_device(
    case: &CaseDefinition,
    sources: &[String],
    device: &mut DeviceRun,
) -> anyhow::Result<CaseArtifact> {
    let cycles = case.manifest.cycles;
    if cycles == 0 {
        bail!("runtime case '{}' must declare cycles > 0", case.id);
    }
    validate_series_lengths(case, cycles)?;
    let container = device_container(&case.id, sources, device)?;
    device.link.install(&container)?;

    let mut now = Duration::ZERO;
    let mut trace = Vec::with_capacity(cycles as usize);
    for cycle_idx in 0..(cycles as usize) {
        let cycle_number = u32::try_from(cycle_idx + 1).unwrap_or(u32::MAX);
        let mut modes = Vec::new();
        for restart in case
            .manifest
            .restarts
            .iter()
            .filter(|entry| entry.before_cycle == cycle_number)
        {
            modes.push(parse_restart_mode(&restart.mode)?);
        }
        // The first step restarts the installed program cold, which loads it, as the native runner
        // starts its program cold; a warm restart after it changes nothing. Before a later cycle the
        // restarts the manifest names act as one: cold when one of them is.
        let restart = if cycle_number == 1 || modes.contains(&RestartMode::Cold) {
            Some("cold")
        } else if modes.is_empty() {
            None
        } else {
            Some("warm")
        };

        if !case.manifest.advance_ms.is_empty() {
            let advance = case.manifest.advance_ms[cycle_idx];
            let advance = Duration::checked_from_millis(advance).with_context(|| {
                format!(
                    "case '{}' advance_ms[{cycle_idx}] {advance} is outside the millisecond time range",
                    case.id
                )
            })?;
            now = Duration::from_nanos(
                now.as_nanos()
                    .checked_add(advance.as_nanos())
                    .ok_or_else(|| anyhow!("the case's time leaves the nanosecond range"))?,
            );
        }

        let mut inputs = serde_json::Map::new();
        for (name, series) in &case.manifest.input_series {
            let raw = &series[cycle_idx];
            if should_skip_step_value(raw) {
                continue;
            }
            parse_typed_value(raw)
                .with_context(|| format!("parse input series value for '{name}'"))?;
            inputs.insert(name.clone(), json!(raw));
        }
        let mut direct_inputs = serde_json::Map::new();
        for (address, series) in &case.manifest.direct_input_series {
            let raw = &series[cycle_idx];
            if should_skip_step_value(raw) {
                continue;
            }
            parse_typed_value(raw)
                .with_context(|| format!("parse direct input value for '{address}'"))?;
            direct_inputs.insert(address.clone(), json!(raw));
        }
        let mut params = json!({
            "time_ns": now.as_nanos(),
            "inputs": inputs,
            "direct_inputs": direct_inputs,
            "watch": case.manifest.watch_globals,
            "watch_direct": case.manifest.watch_direct,
        });
        if let Some(mode) = restart {
            params["restart"] = json!(mode);
        }
        let stepped = device
            .link
            .call("cycle.step", Some(params), DEVICE_REPLY_WAIT)
            .map_err(|error| anyhow!("step cycle {cycle_number}: {error:#}"))?;

        let read = |member: &str,
                    names: &[String]|
         -> anyhow::Result<BTreeMap<String, serde_json::Value>> {
            names
                .iter()
                .map(|name| {
                    let value = stepped
                        .get(member)
                        .and_then(|values| values.get(name))
                        .ok_or_else(|| anyhow!("the step read no value of '{name}'"))?;
                    Ok((name.clone(), normalized(value)))
                })
                .collect()
        };
        let globals = read("values", &case.manifest.watch_globals)?;
        let direct = read("direct", &case.manifest.watch_direct)?;
        let errors = stepped
            .get("errors")
            .cloned()
            .ok_or_else(|| anyhow!("the step named no errors"))?;

        trace.push(json!({
            "cycle": cycle_number,
            "runtime_time_nanos": now.as_nanos(),
            "globals": globals,
            "direct": direct,
            "errors": errors
        }));
    }

    Ok(CaseArtifact {
        payload: json!({
            "version": 1,
            "case_id": case.id,
            "category": case.category,
            "kind": "runtime",
            "description": case.manifest.description,
            "cycles": cycles,
            "trace": trace
        }),
        cycles: Some(u64::from(cycles)),
    })
}

/// The container of `sources` for the device, named `name` while it is built: the sources compiled
/// into a program image for the device's target, with the deployment record of a project whose
/// debug surface is on.
fn device_container(name: &str, sources: &[String], device: &DeviceRun) -> anyhow::Result<Vec<u8>> {
    let work = std::env::temp_dir().join(format!(
        "trust-conformance-device-{}-{name}",
        std::process::id()
    ));
    let container = (|| {
        fs::create_dir_all(&work)?;
        fs::write(work.join("runtime.toml"), DEVICE_CASE_RUNTIME)?;
        let files: Vec<SourceFile> = sources
            .iter()
            .map(|text| SourceFile::new(text.as_str()))
            .collect();
        let built = native::build(
            &files,
            &native::Options {
                target: device.target,
                hooks: true,
            },
            &device.toolchain,
            &work.join("native"),
        )
        .map_err(|error| anyhow!("the case does not compile for the device: {error}"))?;
        let record = trust_runtime::deployment::image_record(&work, &built.image)?;
        Ok::<_, anyhow::Error>(trust_runtime_services::installation::encode_image(
            trust_runtime_services::installation::FIRST_SEQUENCE,
            &built.image,
            &record,
        )?)
    })();
    let _ = fs::remove_dir_all(&work);
    container
}

/// `value`, a value as a device encodes it, as this computer encodes it: the number of a `REAL` or
/// an `LREAL` is written again from the value it holds. A device's JSON writes a `REAL` through its
/// double, where this computer's writes the shortest text of the `REAL` itself, and the two texts
/// would not compare equal.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the number of a REAL is a REAL widened to a double, which narrows back exactly"
)]
fn normalized(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            let kind = object.get("type").and_then(serde_json::Value::as_str);
            let members = object
                .iter()
                .map(|(key, member)| {
                    let member = match (kind, key.as_str(), member.as_f64()) {
                        (Some("REAL"), "value", Some(number)) => json!(number as f32),
                        (Some("LREAL"), "value", Some(number)) => json!(number),
                        _ => normalized(member),
                    };
                    (key.clone(), member)
                })
                .collect();
            serde_json::Value::Object(members)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(normalized).collect())
        }
        other => other.clone(),
    }
}
