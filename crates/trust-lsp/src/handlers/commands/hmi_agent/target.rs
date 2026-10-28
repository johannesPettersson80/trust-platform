//! The simulator target check of the HMI agent's simulator journeys (specification 43, AT-08).
//!
//! An agent's simulator journey sends commands, so it may only reach a local simulator: a runtime in
//! simulation whose HMI web address and control endpoint are both on this computer. A physical or
//! remote runtime is refused. `trust-lsp.hmiAgent.checkTarget` answers this for the extension when
//! the customer connects a named simulator journey and before each of its requests.

use serde_json::{json, Value};
use tower_lsp::lsp_types::Url;

/// The command that checks a simulator journey's target.
pub(super) const CHECK_TARGET: &str = "trust-lsp.hmiAgent.checkTarget";

/// The host names of this computer in an HMI web address.
const LOCAL_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];

/// `trust-lsp.hmiAgent.checkTarget`: `{ "local": true }` when the one argument
/// `{ "simulator", "url", "endpoint" }` is a local simulator, otherwise `{ "local": false }`.
pub(super) fn check_target(arguments: &[Value]) -> Value {
    json!({ "local": arguments.first().is_some_and(is_local_simulator) })
}

/// Whether `target` is a runtime in simulation with a local web address and a local control
/// endpoint.
fn is_local_simulator(target: &Value) -> bool {
    target.get("simulator") == Some(&Value::Bool(true))
        && target
            .get("url")
            .and_then(Value::as_str)
            .is_some_and(is_local_web_address)
        && target
            .get("endpoint")
            .and_then(Value::as_str)
            .is_some_and(is_local_control_endpoint)
}

/// Whether the host of a web address is this computer, as the browser reads the address.
fn is_local_web_address(url: &str) -> bool {
    Url::parse(url).is_ok_and(|url| {
        url.host_str()
            .is_some_and(|host| LOCAL_HOSTS.contains(&host))
    })
}

/// Whether a control endpoint is on this computer: TCP to a local host and port, a Unix socket or a
/// Windows named pipe.
fn is_local_control_endpoint(endpoint: &str) -> bool {
    endpoint.starts_with("unix://")
        || endpoint.starts_with("npipe:")
        || endpoint.strip_prefix("tcp://").is_some_and(|address| {
            LOCAL_HOSTS.iter().any(|host| {
                address
                    .strip_prefix(host)
                    .is_some_and(|rest| rest.starts_with(':'))
            })
        })
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
