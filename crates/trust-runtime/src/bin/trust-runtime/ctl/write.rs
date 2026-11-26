//! `trust-runtime ctl write <path>=<value>...`: writes variables as a service writes them, with
//! `variables.write` (specification 11 §6.8.1, "Writing variables"), sent with the service's token
//! from `--token`. Each value is JSON, as `variables.read` gives it; the command reads every
//! assignment before it sends the set, and prints the reply.

use anyhow::Context;
use serde_json::{json, Value as Json};

use super::{ensure_control_response_ok, send_request, ResolvedControlTarget};

/// Sends the writes of `assignments` to `target` and prints the reply.
pub(super) fn run(target: &ResolvedControlTarget, assignments: &[String]) -> anyhow::Result<()> {
    let writes = assignments
        .iter()
        .map(|text| assignment(text))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let request = json!({
        "id": 1,
        "type": "variables.write",
        "auth": target.auth_token.as_deref(),
        "params": { "writes": writes },
    });
    let response = send_request(&target.endpoint, &request)?;
    ensure_control_response_ok(&response)?;
    outln!("{}", serde_json::to_string(&response)?);
    Ok(())
}

/// The write `<path>=<value>` spells: the path before the first `=`, the JSON value after it.
fn assignment(text: &str) -> anyhow::Result<Json> {
    let (path, value) = text
        .split_once('=')
        .with_context(|| format!("'{text}' is no <path>=<value>"))?;
    let value: Json = serde_json::from_str(value)
        .with_context(|| format!("the value of {path} is no JSON value: {value}"))?;
    Ok(json!({ "path": path, "value": value }))
}
