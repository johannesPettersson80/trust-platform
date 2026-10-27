//! `trust-runtime ctl install`: uploads a program container to a stopped runtime and installs it,
//! over any control endpoint (specification 11, 6.8.1; specification 34, section 11.2 and PORT-05).
//!
//! The upload is `transfer.begin`, one `transfer.part` for each part of the container, and
//! `program.install`. Every request goes through the shared control client, so the same code
//! installs through a chip's serial port, a Unix socket and loopback TCP.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, ensure};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use trust_runtime::control::ControlEndpoint;
use trust_runtime_client::{ControlClient, ControlEndpoint as ClientEndpoint, ControlError};

use super::{
    CtlEndpoint, ResolvedControlTarget, CONTROL_CONNECT_TIMEOUT, CONTROL_IO_TIMEOUT,
    MAX_CONTROL_RESPONSE_BYTES, NETWORK_IO_TIMEOUT, SERIAL_IO_TIMEOUT,
};

/// The longest part of an upload (specification 34, PORT-05): every part but the last has the
/// size that `transfer.begin` names, and a full part's base64 fits a chip's frame.
const PART_BYTES_MAX: usize = 4_096;

/// How often a part is sent: once, and again after each wait that runs out, twice at most. The
/// runtime acknowledges the repeat of a part it has received (specification 34, PORT-05).
const PART_ATTEMPTS: u32 = 3;

/// The wait for the reply to `program.install` on every endpoint: the runtime writes the program
/// before it replies, a chip one erase unit per loop pass (specification 11, 6.8.1).
const INSTALL_REPLY_TIMEOUT: Duration = Duration::from_mins(2);

/// How long `--restart` waits for the program to run (specification 11, 6.8.1).
const RUNNING_WAIT: Duration = Duration::from_secs(10);

/// How often `--restart` asks for the state while it waits.
const STATUS_POLL: Duration = Duration::from_millis(100);

/// The waits of one install.
struct Waits {
    /// The wait for the reply to every request but `program.install`.
    reply: Duration,
    /// The wait for the reply to `program.install`.
    install: Duration,
    /// How long `--restart` waits for the state `running`.
    running: Duration,
    /// How often `--restart` asks for the state.
    poll: Duration,
}

/// The identity that the reply to `program.install` gives the installed program.
#[derive(Debug, PartialEq, Eq)]
struct Installed {
    sequence: u64,
    program_sha256: String,
    bytes: u64,
}

/// The program that `--restart` started.
#[derive(Debug, PartialEq, Eq)]
struct Restarted {
    mode: String,
    state: String,
}

/// What a finished install tells the user.
#[derive(Debug, PartialEq, Eq)]
struct InstallReport {
    installed: Installed,
    restarted: Option<Restarted>,
}

impl InstallReport {
    /// The lines the command prints: the identity of the installed program, and with `--restart`
    /// the mode and the state of the program that was started.
    fn render(&self) -> String {
        let installed = &self.installed;
        let mut lines = vec![format!(
            "installed sequence={} program_sha256={} bytes={}",
            installed.sequence, installed.program_sha256, installed.bytes
        )];
        if let Some(restarted) = &self.restarted {
            lines.push(format!(
                "restarted mode={} state={}",
                restarted.mode, restarted.state
            ));
        }
        lines.join("\n")
    }
}

/// An upload that the runtime has opened.
pub(super) struct Upload {
    /// The id of the upload: 16 hexadecimal digits.
    pub(super) transfer: String,
    /// The size of every part but the last.
    part_bytes: usize,
}

/// Installs the program container in the file `container` into the runtime at `target`, and
/// prints the identity that the runtime gives the program. With `restart`, `cold` or `warm`, it
/// then starts the program and waits until it runs.
pub(super) fn run(
    target: &ResolvedControlTarget,
    container: &Path,
    restart: Option<&str>,
) -> anyhow::Result<()> {
    let bytes = std::fs::read(container)
        .map_err(|error| anyhow!("cannot read the container {}: {error}", container.display()))?;
    let (mut client, reply_wait) = connect(&target.endpoint, target.auth_token.as_deref())?;
    let waits = Waits {
        reply: reply_wait,
        install: INSTALL_REPLY_TIMEOUT,
        running: RUNNING_WAIT,
        poll: STATUS_POLL,
    };
    let report = install(&mut client, &bytes, restart, &waits)?;
    println!("{}", report.render());
    Ok(())
}

/// Connects the shared client to `endpoint`, with the waits that the endpoint has: on a socket of
/// this computer a connection of 500 milliseconds and a reply of 750, on a chip's serial port a
/// reply of 5 seconds, and on a chip's network a connection and a reply of 5 seconds each
/// (specification 11, 6.8.1).
pub(super) fn connect(
    endpoint: &CtlEndpoint,
    token: Option<&str>,
) -> anyhow::Result<(ControlClient, Duration)> {
    let (endpoint, connect_wait, reply_wait) = match endpoint {
        CtlEndpoint::Socket(ControlEndpoint::Tcp(address)) => (
            ClientEndpoint::Tcp(address.to_string()),
            CONTROL_CONNECT_TIMEOUT,
            CONTROL_IO_TIMEOUT,
        ),
        #[cfg(unix)]
        CtlEndpoint::Socket(ControlEndpoint::Unix(path)) => (
            ClientEndpoint::Unix(path.clone()),
            CONTROL_CONNECT_TIMEOUT,
            CONTROL_IO_TIMEOUT,
        ),
        CtlEndpoint::Serial(serial) => (serial.clone(), CONTROL_CONNECT_TIMEOUT, SERIAL_IO_TIMEOUT),
        CtlEndpoint::Network(host) => (host.clone(), NETWORK_IO_TIMEOUT, NETWORK_IO_TIMEOUT),
    };
    let client = ControlClient::connect(&endpoint, token, connect_wait)?
        .with_response_limit(MAX_CONTROL_RESPONSE_BYTES);
    client.set_timeout(reply_wait)?;
    Ok((client, reply_wait))
}

/// The steps of the install. When a step after `transfer.begin` fails, the upload is freed on the
/// runtime and the first failure is the result.
fn install(
    client: &mut ControlClient,
    container: &[u8],
    restart: Option<&str>,
    waits: &Waits,
) -> anyhow::Result<InstallReport> {
    let upload = begin(client, container, false)?;
    let installed = match send_parts(client, &upload, container)
        .and_then(|()| commit(client, &upload.transfer, waits))
    {
        Ok(installed) => installed,
        Err(error) => {
            // A client that cannot take its wait back cannot reach the runtime either.
            let _ = client.set_timeout(waits.reply);
            free_upload(client, &upload.transfer);
            return Err(error);
        }
    };
    let restarted = restart
        .map(|mode| restart_and_wait(client, mode, waits))
        .transpose()?;
    Ok(InstallReport {
        installed,
        restarted,
    })
}

/// Opens the upload with the length and the SHA-256 of the container (`transfer.begin`), or of a
/// firmware image with `"for": "firmware"` (specification 34 TARGET-04).
pub(super) fn begin(
    client: &mut ControlClient,
    container: &[u8],
    firmware: bool,
) -> anyhow::Result<Upload> {
    let mut params = json!({ "bytes": container.len(), "sha256": sha256_hex(container) });
    if firmware {
        params["for"] = json!("firmware");
    }
    let reply = client
        .request("transfer.begin", Some(params))
        .map_err(|error| step_failed("transfer.begin", &error))?;
    let transfer = reply
        .get("transfer")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| anyhow!("transfer.begin: the reply names no transfer"))?;
    let Some(part_bytes) = reply
        .get("part_bytes")
        .and_then(Value::as_u64)
        .and_then(|bytes| usize::try_from(bytes).ok())
        .filter(|bytes| (1..=PART_BYTES_MAX).contains(bytes))
    else {
        // The upload is open, and no part of this size can be sent to it.
        free_upload(client, transfer);
        bail!("transfer.begin: part_bytes is not a whole number from 1 to {PART_BYTES_MAX}");
    };
    Ok(Upload {
        transfer: transfer.to_string(),
        part_bytes,
    })
}

/// Sends the container in order, one `transfer.part` for each part of the upload's size.
pub(super) fn send_parts(
    client: &mut ControlClient,
    upload: &Upload,
    container: &[u8],
) -> anyhow::Result<()> {
    for (index, part) in container.chunks(upload.part_bytes).enumerate() {
        send_part(client, &upload.transfer, index * upload.part_bytes, part)?;
    }
    Ok(())
}

/// Sends one part with its CRC-32 and checks the next offset of the reply. A part whose reply
/// does not come in time is sent again, up to `PART_ATTEMPTS` times in all: the runtime
/// acknowledges the repeat of a part it has received (specification 34, PORT-05).
fn send_part(
    client: &mut ControlClient,
    transfer: &str,
    offset: usize,
    part: &[u8],
) -> anyhow::Result<()> {
    let params = json!({
        "transfer": transfer,
        "offset": offset,
        "data": STANDARD.encode(part),
        "crc32": format!("{:08x}", crc32fast::hash(part)),
    });
    let next_offset = u64::try_from(offset + part.len()).ok();
    let mut attempts = 0;
    loop {
        attempts += 1;
        match client.request("transfer.part", Some(params.clone())) {
            Ok(reply) => {
                ensure!(
                    reply.get("next_offset").and_then(Value::as_u64) == next_offset,
                    "transfer.part at offset {offset}: the reply does not say that the next part \
                     is at {}",
                    offset + part.len()
                );
                return Ok(());
            }
            Err(ControlError::Transport(_)) if attempts < PART_ATTEMPTS => {}
            Err(error) => {
                return Err(step_failed(
                    &format!("transfer.part at offset {offset}"),
                    &error,
                ));
            }
        }
    }
}

/// Installs the finished upload (`program.install`), waiting for the reply as long as the install
/// takes, and returns the identity that the reply gives the program.
fn commit(client: &mut ControlClient, transfer: &str, waits: &Waits) -> anyhow::Result<Installed> {
    client.set_timeout(waits.install)?;
    let reply = client
        .request("program.install", Some(json!({ "transfer": transfer })))
        .map_err(|error| step_failed("program.install", &error))?;
    client.set_timeout(waits.reply)?;
    ensure!(
        reply.get("status").and_then(Value::as_str) == Some("installed"),
        "program.install: the reply does not say that the program is installed: {reply}"
    );
    let number = |name: &str| {
        reply
            .get(name)
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow!("program.install: the reply has no {name}"))
    };
    let program_sha256 = reply
        .get("program_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("program.install: the reply has no program_sha256"))?;
    Ok(Installed {
        sequence: number("sequence")?,
        program_sha256: program_sha256.to_string(),
        bytes: number("bytes")?,
    })
}

/// Frees the upload on the runtime after a failure, so that none stays open and blocks the next
/// install with `transfer busy`. The failure that is reported is the first one: an abort that
/// fails too (the upload is gone, or so is the link) changes nothing.
pub(super) fn free_upload(client: &mut ControlClient, transfer: &str) {
    let _ = client.request("transfer.abort", Some(json!({ "transfer": transfer })));
}

/// Starts the installed program (`restart`) and asks `status` until the program runs.
fn restart_and_wait(
    client: &mut ControlClient,
    mode: &str,
    waits: &Waits,
) -> anyhow::Result<Restarted> {
    client
        .request("restart", Some(json!({ "mode": mode })))
        .map_err(|error| step_failed("restart", &error))?;
    let started = Instant::now();
    loop {
        let status = client
            .request("status", None)
            .map_err(|error| step_failed("status", &error))?;
        let state = status
            .get("state")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("status: the reply has no state"))?;
        match state {
            "running" => {
                return Ok(Restarted {
                    mode: mode.to_string(),
                    state: state.to_string(),
                });
            }
            "faulted" => {
                let fault = status
                    .get("fault")
                    .and_then(Value::as_str)
                    .unwrap_or("no reason is given");
                bail!("the installed program faulted when it started: {fault}");
            }
            _ => {}
        }
        if started.elapsed() >= waits.running {
            bail!(
                "the program is not running {} seconds after the restart: its state is {state}",
                waits.running.as_secs()
            );
        }
        std::thread::sleep(waits.poll);
    }
}

/// The failure of `step` with the runtime's own text, and its code when it gives one.
pub(super) fn step_failed(step: &str, error: &ControlError) -> anyhow::Error {
    match error {
        ControlError::Refused {
            error,
            code: Some(code),
        } => anyhow!("{step} failed: {error} ({code})"),
        other => anyhow!("{step} failed: {other}"),
    }
}

/// The SHA-256 of `bytes` as 64 lowercase hexadecimal digits.
fn sha256_hex(bytes: &[u8]) -> String {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    hex::encode(digest)
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
