//! `trust-runtime identify`: asks a runtime to show where it is by blinking its identify output
//! (specification 11, "Identify"). The request, its checks and its answers belong to the runtime;
//! this command builds the request, sends it over the control connection and prints the answer.

use anyhow::Context;
use serde_json::json;

use crate::cli::IdentifyArgs;
use crate::ctl;

pub fn run_identify(args: IdentifyArgs) -> anyhow::Result<()> {
    println!("{}", identify(args)?);
    Ok(())
}

/// Sends `identify` to the runtime that `args` name and returns the runtime's result as one JSON
/// line. A refusal is an error that carries the runtime's text and its error code.
fn identify(args: IdentifyArgs) -> anyhow::Result<String> {
    let target = ctl::resolve_control_target(args.project, args.endpoint, args.token)?;
    let request = json!({
        "id": 1,
        "type": "identify",
        "auth": target.auth_token,
        "params": { "duration_s": args.duration_s },
    });
    let response = ctl::send_request(&target.endpoint, &request)?;
    ctl::ensure_control_response_ok(&response)?;
    let result = response
        .get("result")
        .context("identify response missing 'result'")?;
    Ok(serde_json::to_string(result)?)
}
