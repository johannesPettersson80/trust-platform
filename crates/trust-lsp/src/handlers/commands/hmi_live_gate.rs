//! Live mode's operating gate (specification 39, §11 item 6): whether the HMI Builder opens the
//! operating view on the deployed HMI of the selected run target, or why Live mode stays
//! read-only. The extension gathers the facts and shows the verdict: the project's folder and the
//! paths of the editor's unsaved documents (one under its `hmi/` folder is an unsaved HMI change),
//! the project's bundle as `trust-lsp.hmiBundle` checks it, whether the run target has an HMI web
//! server (else the run target, to name it), and the web server's answer when it reads the page
//! there.
//!
//! `trust-lsp.hmiLiveGate` `{ root, unsaved, bundle?, webServer, runTarget?, page, served? }`, with
//! `bundle` `{ ok?, version? }`, `runTarget` `{ label, simulator }` and `served` `{ status, body }`,
//! answers `{ ok: true, verdict }`, the verdict's `kind` one of:
//! - `read_page`: before the page is read (no `served`), the HMI is saved, its bundle checked and
//!   the run target has an HMI web server, so the extension reads the page there;
//! - `operate`, with `bundle`: the web server serves the page from the project's bundle and the
//!   HMI is still saved; the operating view runs that deployed bundle;
//! - `read_only`, with `reason`: why Live mode stays read-only.
//!
//! Arguments it cannot read answer `{ ok: false, error }`.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{json, Value};

use super::hmi_files::{holds_unsaved_hmi, single_argument};
use super::HMI_LIVE_GATE_COMMAND;

#[cfg(test)]
mod tests;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Args {
    root: PathBuf,
    unsaved: Vec<PathBuf>,
    #[serde(default)]
    bundle: Option<Bundle>,
    web_server: bool,
    #[serde(default)]
    run_target: Option<RunTarget>,
    page: String,
    #[serde(default)]
    served: Option<Served>,
}

/// The project's bundle as `trust-lsp.hmiBundle` checks it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bundle {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    version: Option<String>,
}

/// The selected run target (`trust-lsp.runTargets` `hmiTarget`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunTarget {
    label: String,
    simulator: bool,
}

/// The web server's answer to `GET /api/hmi/page?name=<page>`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Served {
    status: u16,
    #[serde(default)]
    body: Value,
}

const UNSAVED: &str = "Save the HMI before operating its deployed version.";
const UNCHECKED: &str = "Check and deploy the HMI before operating it.";
const UNDEPLOYED: &str = "Deploy this saved HMI to the selected runtime before operating it.";
const CHANGED: &str = "The HMI changed. Save and deploy it before operating again.";

/// The answer of `trust-lsp.hmiLiveGate`.
pub(super) fn live_gate_value(args: Vec<Value>) -> Value {
    match single_argument::<Args>(args, HMI_LIVE_GATE_COMMAND) {
        Ok(args) => json!({ "ok": true, "verdict": verdict(&args) }),
        Err(error) => json!({ "ok": false, "error": error }),
    }
}

fn verdict(args: &Args) -> Value {
    let version = args
        .bundle
        .as_ref()
        .filter(|bundle| bundle.ok)
        .and_then(|bundle| bundle.version.as_deref())
        .filter(|version| !version.is_empty());
    let dirty = holds_unsaved_hmi(&args.root, &args.unsaved);
    let Some(served) = &args.served else {
        // Before the page is read: the saved HMI, its checked bundle, the web server, in turn.
        if dirty {
            return read_only(UNSAVED);
        }
        if version.is_none() {
            return read_only(UNCHECKED);
        }
        if !args.web_server {
            return read_only(&without_web_server(args.run_target.as_ref()));
        }
        return json!({ "kind": "read_page" });
    };
    let Some(version) = version else {
        return read_only(UNCHECKED);
    };
    // The web server names its active bundle by the first 16 characters of the bundle's version.
    let deployed = version.chars().take(16).collect::<String>();
    let served_page = served.body.pointer("/page/name").and_then(Value::as_str);
    let served_bundle = served.body.get("bundle").and_then(Value::as_str);
    if served.status != 200
        || served_page != Some(args.page.as_str())
        || served_bundle != Some(deployed.as_str())
    {
        return read_only(UNDEPLOYED);
    }
    if dirty {
        // Edited while the page was read.
        return read_only(CHANGED);
    }
    json!({ "kind": "operate", "bundle": deployed })
}

fn read_only(reason: &str) -> Value {
    json!({ "kind": "read_only", "reason": reason })
}

/// Why Live mode stays read-only on a run target without an HMI web server, naming it.
fn without_web_server(target: Option<&RunTarget>) -> String {
    match target {
        Some(target) if target.simulator => {
            "The simulator has no HMI web server. Operate the page, \
            with its alarms, on a runtime: select or connect one in Devices & Connections."
                .to_owned()
        }
        Some(target) => format!(
            "{} has no HMI web server. Enable [runtime.web] in its runtime.toml.",
            target.label
        ),
        None => "The selected target has no HMI web server. Start a runtime with [runtime.web] \
            enabled."
            .to_owned(),
    }
}
