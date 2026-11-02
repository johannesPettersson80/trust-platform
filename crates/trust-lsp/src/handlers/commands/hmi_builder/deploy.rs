//! The web server an HMI deployment goes to (specification 39, §16.2 item 8): the selected
//! runtime reports its own web server in `fleet.topology`; its address is the listen port on the
//! host of the runtime's control endpoint, or the listen address itself when that names a host of
//! its own (not `0.0.0.0` or `::`). Live mode asks the runtime, passes its answer here and talks to
//! the address this answers (§11); Deploy, which the language server carries out
//! (`trust-lsp.hmiDeploy`), reads the same rule.
//!
//! `{ controlEndpoint, topology }` → `{ url }`, the web server's base URL, or `url: null` when the
//! runtime reports none.

use serde::Deserialize;
use serde_json::{json, Value};

use super::decode;

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Args {
    /// The runtime's control endpoint: `tcp://host:port`, `unix://…`, `npipe:…`.
    control_endpoint: String,
    /// What the runtime answered to `fleet.topology`.
    topology: Value,
}

/// The answer of `trust-lsp.hmiDeployTarget`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    let args: Args = decode(argument)?;
    let url = web_server_url(&args.control_endpoint, &args.topology);
    Ok(json!({ "ok": true, "url": url }))
}

/// The base URL of the web server that the runtime at `control_endpoint` reports for itself in
/// `topology`, its answer to `fleet.topology`; `None` when it reports none.
pub(in crate::handlers::commands) fn web_server_url(
    control_endpoint: &str,
    topology: &Value,
) -> Option<String> {
    own_web_server(topology).and_then(|(listen, tls)| web_address(control_endpoint, &listen, tls))
}

/// The web server the runtime reports for itself: the first `web` endpoint of source `self`
/// among its hosts' and their containers' runtimes, its listen address and whether it uses TLS.
fn own_web_server(topology: &Value) -> Option<(String, bool)> {
    let list = |value: &Value, key: &str| -> Vec<Value> {
        value
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    list(topology, "hosts")
        .iter()
        .flat_map(|host| {
            let mut runtimes = list(host, "runtimes");
            for container in list(host, "containers") {
                runtimes.extend(list(&container, "runtimes"));
            }
            runtimes
        })
        .flat_map(|runtime| list(&runtime, "endpoints"))
        .find_map(|endpoint| {
            let own = endpoint.get("protocol").and_then(Value::as_str) == Some("web")
                && endpoint.get("source").and_then(Value::as_str) == Some("self");
            let params = endpoint.get("params")?;
            let listen = params.get("listen")?.as_str()?.to_owned();
            own.then(|| {
                (
                    listen,
                    params.get("tls").and_then(Value::as_bool) == Some(true),
                )
            })
        })
}

/// The web server's base URL: `http(s)://<host>:<port>`, the host the listen address's own, or
/// the control endpoint's when the listen address is `0.0.0.0` or `::`; an IPv6 host in brackets.
fn web_address(control_endpoint: &str, listen: &str, tls: bool) -> Option<String> {
    let (listen_host, port) = host_and_port(listen.trim())?;
    let any_host = matches!(listen_host, "0.0.0.0" | "::" | "");
    let host = if any_host {
        control_host(control_endpoint)
    } else {
        listen_host
    };
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    let scheme = if tls { "https" } else { "http" };
    Some(format!("{scheme}://{host}:{port}"))
}

/// `host:port` or `[host]:port`, the port all digits.
fn host_and_port(listen: &str) -> Option<(&str, &str)> {
    let (host, port) = match listen.strip_prefix('[') {
        Some(bracketed) => {
            let (host, rest) = bracketed.split_once(']')?;
            (host, rest.strip_prefix(':')?)
        }
        None => {
            let (host, port) = listen.split_once(':')?;
            (!host.is_empty()).then_some((host, port))?
        }
    };
    let digits = !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit());
    (digits && !host.is_empty() && !port.contains(':')).then_some((host, port))
}

/// The host of a control endpoint; a local socket or pipe is this machine.
fn control_host(endpoint: &str) -> &str {
    let Some(rest) = endpoint.trim().strip_prefix("tcp://") else {
        return "127.0.0.1";
    };
    let host = match rest.strip_prefix('[') {
        Some(bracketed) => bracketed.split_once(']').map(|(host, _)| host),
        None => rest.split([':', '/']).next(),
    };
    host.filter(|host| !host.is_empty()).unwrap_or("127.0.0.1")
}

#[cfg(test)]
#[path = "deploy_tests.rs"]
mod tests;
