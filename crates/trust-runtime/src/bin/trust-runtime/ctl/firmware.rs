//! `trust-runtime ctl firmware <image>`: updates a chip's truST firmware over its control connection
//! (specification 34 TARGET-04; specification 11, the firmware update contract).
//!
//! The command reads `capabilities` and `status` first: a runtime that keeps no firmware slots, a
//! computer among them, is refused, and so is one whose program is not stopped. It then uploads the
//! image with `"for": "firmware"` and sends `firmware.install`, whose reply names the new firmware's
//! version and slot. The chip resets into the new firmware once the reply is sent, so the command
//! connects again until `capabilities` answers, and succeeds when the chip runs the new version,
//! confirmed. A chip that rolled the update back, because the new firmware did not finish its boot,
//! answers with the version it rolled back, and the command fails with both versions.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, ensure};
use serde_json::{json, Value};
use trust_runtime_client::ControlClient;

use super::install::{begin, connect, free_upload, send_parts, step_failed};
use super::ResolvedControlTarget;

/// The wait for the reply to `firmware.install`: the chip checks the image in the pass that reads
/// the request, then commits its first bytes and its selection in a few passes (specification 34
/// TARGET-04).
const INSTALL_REPLY_TIMEOUT: Duration = Duration::from_secs(60);

/// How often the command tries to reach the chip again while it restarts.
const RECONNECT_POLL: Duration = Duration::from_secs(1);

/// What the chip reports of its firmware in `capabilities`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Firmware {
    slot: String,
    version: String,
    confirmed: bool,
    rolled_back: Option<String>,
}

impl Firmware {
    /// The `firmware` member of a `capabilities` reply, `None` when it is null.
    fn of(capabilities: &Value) -> anyhow::Result<Option<Self>> {
        let firmware = &capabilities["firmware"];
        if firmware.is_null() {
            return Ok(None);
        }
        let text = |name: &str| {
            firmware[name]
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| anyhow!("capabilities: firmware has no {name}"))
        };
        Ok(Some(Self {
            slot: text("slot")?,
            version: text("version")?,
            confirmed: firmware["confirmed"]
                .as_bool()
                .ok_or_else(|| anyhow!("capabilities: firmware has no confirmed"))?,
            rolled_back: firmware["rolled_back"].as_str().map(str::to_string),
        }))
    }
}

/// Updates the firmware of the chip at `target` to the image in the file `image`, and waits up to
/// `wait` for the chip to run it.
pub(super) fn run(
    target: &ResolvedControlTarget,
    image: &Path,
    wait: Duration,
) -> anyhow::Result<()> {
    let bytes = std::fs::read(image).map_err(|error| {
        anyhow!(
            "cannot read the firmware image {}: {error}",
            image.display()
        )
    })?;
    let token = target.auth_token.as_deref();
    let report = update(
        || connect(&target.endpoint, token),
        &bytes,
        wait,
        RECONNECT_POLL,
        |line| println!("{line}"),
    )?;
    println!("{}", report.render());
    Ok(())
}

/// What a finished update tells the user.
#[derive(Debug, PartialEq, Eq)]
struct UpdateReport {
    version: String,
    slot: String,
}

impl UpdateReport {
    fn render(&self) -> String {
        format!(
            "running version={} slot={} confirmed",
            self.version, self.slot
        )
    }
}

/// The steps of the update over the clients that `connect` opens: the checks, the upload, the
/// install, whose line `progress` prints, and the wait of at most `wait` for the restarted chip,
/// asked again every `poll`.
fn update(
    mut connect: impl FnMut() -> anyhow::Result<(ControlClient, Duration)>,
    bytes: &[u8],
    wait: Duration,
    poll: Duration,
    mut progress: impl FnMut(&str),
) -> anyhow::Result<UpdateReport> {
    let (mut client, reply_wait) = connect()?;
    let before = request(&mut client, "capabilities", None)?;
    let Some(running) = Firmware::of(&before)? else {
        bail!(
            "this runtime keeps no firmware slots: truST on a computer is updated by installing \
             truST on that computer"
        );
    };
    let status = request(&mut client, "status", None)?;
    let state = status["state"].as_str().unwrap_or("unknown");
    ensure!(
        state == "stopped",
        "the runtime's program is {state}: stop it first (trust-runtime ctl shutdown), then update \
         its firmware"
    );
    let upload = begin(&mut client, bytes, true)?;
    let installed = match send_parts(&mut client, &upload, bytes)
        .and_then(|()| commit(&mut client, &upload.transfer, reply_wait))
    {
        Ok(installed) => installed,
        Err(error) => {
            let _ = client.set_timeout(reply_wait);
            free_upload(&mut client, &upload.transfer);
            return Err(error);
        }
    };
    progress(&format!(
        "installed version={} slot={}; the runtime restarts into it (it ran {} from slot {})",
        installed.version, installed.slot, running.version, running.slot
    ));
    drop(client);
    let after = wait_for_restart(&mut connect, wait, poll)?;
    if after.version == installed.version && after.slot == installed.slot {
        ensure!(
            after.confirmed,
            "the runtime runs truST {} from slot {} but has not confirmed it",
            after.version,
            after.slot
        );
        return Ok(UpdateReport {
            version: after.version,
            slot: after.slot,
        });
    }
    if after.rolled_back.as_deref() == Some(installed.version.as_str()) {
        bail!(
            "the runtime rolled the update back: truST {} did not finish its boot, and the runtime \
             runs truST {} from slot {} again",
            installed.version,
            after.version,
            after.slot
        );
    }
    bail!(
        "the runtime runs truST {} from slot {}, not the installed {} from slot {}",
        after.version,
        after.slot,
        installed.version,
        installed.slot
    )
}

/// What the reply to `firmware.install` names.
struct Installed {
    version: String,
    slot: String,
}

/// Installs the finished upload (`firmware.install`), waiting for the reply as long as the check and
/// the commit take.
fn commit(
    client: &mut ControlClient,
    transfer: &str,
    reply_wait: Duration,
) -> anyhow::Result<Installed> {
    client.set_timeout(INSTALL_REPLY_TIMEOUT)?;
    let reply = client
        .request("firmware.install", Some(json!({ "transfer": transfer })))
        .map_err(|error| step_failed("firmware.install", &error))?;
    client.set_timeout(reply_wait)?;
    ensure!(
        reply["status"].as_str() == Some("installed"),
        "firmware.install: the reply does not say that the firmware is installed: {reply}"
    );
    let text = |name: &str| {
        reply[name]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| anyhow!("firmware.install: the reply has no {name}"))
    };
    Ok(Installed {
        version: text("version")?,
        slot: text("slot")?,
    })
}

/// Connects again until the restarted chip answers `capabilities`, at most `wait`.
fn wait_for_restart(
    connect: &mut impl FnMut() -> anyhow::Result<(ControlClient, Duration)>,
    wait: Duration,
    poll: Duration,
) -> anyhow::Result<Firmware> {
    let started = Instant::now();
    loop {
        std::thread::sleep(poll);
        let answer =
            connect().and_then(|(mut client, _)| request(&mut client, "capabilities", None));
        if let Ok(capabilities) = answer
            && let Some(firmware) = Firmware::of(&capabilities)?
        {
            return Ok(firmware);
        }
        if started.elapsed() >= wait {
            bail!(
                "the runtime did not answer within {} seconds of its restart into the new firmware",
                wait.as_secs()
            );
        }
    }
}

/// Sends `kind` with `params` and returns the result.
fn request(client: &mut ControlClient, kind: &str, params: Option<Value>) -> anyhow::Result<Value> {
    client
        .request(kind, params)
        .map_err(|error| step_failed(kind, &error))
}

#[cfg(test)]
#[path = "firmware_tests.rs"]
mod tests;
