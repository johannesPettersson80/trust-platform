//! `trust-runtime ctl service-token <name> [--role <role>] --out <file>`: creates the service token
//! of a service, or replaces the one it has, with `pair.service` (specification 11 §6.9.6;
//! specification 48 RU-81). The token goes into the file, through a new file renamed over it with
//! mode 0600 on Unix, in a folder made when it is missing, and never to the terminal: the command
//! prints the token's id, its role and the file.

use std::path::Path;

use anyhow::Context;
use serde_json::json;
use trust_runtime::control::endpoint::write_private_file;

use super::{ensure_control_response_ok, send_request, ResolvedControlTarget};

/// Sends `pair.service` for `name` with `role` to `target` and writes the token into `out`.
pub(super) fn run(
    target: &ResolvedControlTarget,
    name: &str,
    role: Option<&str>,
    out: &Path,
) -> anyhow::Result<()> {
    let mut params = json!({ "name": name });
    if let Some(role) = role {
        params["role"] = json!(role);
    }
    let request = json!({
        "id": 1,
        "type": "pair.service",
        "auth": target.auth_token.as_deref(),
        "params": params,
    });
    let response = send_request(&target.endpoint, &request)?;
    ensure_control_response_ok(&response)?;
    let result = &response["result"];
    let text = |key: &str| {
        result[key]
            .as_str()
            .with_context(|| format!("pair.service reply missing string '{key}'"))
    };
    let (id, role, token) = (text("id")?, text("role")?, text("token")?);
    write_token(out, token)
        .with_context(|| format!("the service token {id} was created but not written"))?;
    outln!("id={id} role={role} token_file={}", out.display());
    Ok(())
}

/// Writes `token` into `out`, making the file's folder when it is missing.
fn write_token(out: &Path, token: &str) -> anyhow::Result<()> {
    if let Some(folder) = out.parent().filter(|folder| !folder.as_os_str().is_empty()) {
        std::fs::create_dir_all(folder).with_context(|| format!("create {}", folder.display()))?;
    }
    Ok(write_private_file(out, token.as_bytes())?)
}
