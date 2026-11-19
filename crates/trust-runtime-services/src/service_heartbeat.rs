//! `service.heartbeat` (specification 11 §6.8.1, "Service heartbeats"; specification 48 RU-63): a
//! service's heartbeat, which only its service token sends (specification 11 §6.9.6). The runtime
//! adds one to the value the last completed scan left in the service's global `<name>_heartbeat`,
//! and the next cycle's start writes the sum through the Live Values write queue, as the program's
//! input and never as a force, on a computer and on a chip alike: both apply the queue at every
//! cycle start, with the debug surface on or off.

use alloc::format;
use alloc::string::ToString;

use serde_json::json;
use trust_runtime_core::ports::WallClock;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::Value;

use crate::live_values::queues::PendingVarTarget;
use crate::live_values::var_requests::VariableQueuePort;
use crate::ports::{AccessPort, ResourcePort};
use crate::protocol::ControlResponse;
use crate::variables::{refusal_text, ReadRefusal};

/// The refusal of a request that no service token sends.
const NOT_A_SERVICE: &str = "service.heartbeat needs a service token";

/// `service.heartbeat`: `{service, value}`, the service the token `auth` speaks for and the value
/// the next cycle's start writes into its heartbeat. The token is checked first, then the
/// resource's state, then the program's heartbeat for the service.
pub fn heartbeat<H>(id: u64, auth: Option<&str>, host: &H) -> ControlResponse
where
    H: AccessPort + WallClock + ResourcePort + VariableQueuePort,
{
    // A service token does not expire, so the clock only prunes claimed tokens that did.
    let now = host.utc_ms().map_or(0, |utc_ms| utc_ms / 1_000);
    let Some(service) = auth.and_then(|token| host.pairing()?.service_of(token, now)) else {
        return ControlResponse::error(id, NOT_A_SERVICE.to_string());
    };
    let state = host.resource_state();
    if state != ResourceState::Running {
        let state = format!("{state:?}").to_ascii_lowercase();
        return ControlResponse::error(id, format!("the program does not run: {state}"));
    }
    let path = format!("{service}_heartbeat");
    let undeclared = || {
        ControlResponse::error(
            id,
            format!("the program declares no heartbeat for service {service}"),
        )
    };
    let set = match host.read_variables(core::slice::from_ref(&path)) {
        Ok(set) => set,
        Err(ReadRefusal::Unknown(_) | ReadRefusal::NotElementary(_)) => return undeclared(),
        Err(refusal) => return ControlResponse::error(id, refusal_text(&refusal)),
    };
    let Some((_, Value::UDInt(beat))) = set.values.first() else {
        return undeclared();
    };
    let value = beat.wrapping_add(1);
    match host.enqueue_var_write(PendingVarTarget::Global(path.into()), Value::UDInt(value)) {
        Ok(()) => ControlResponse::ok(id, json!({ "service": service, "value": value })),
        Err(full) => ControlResponse::error(id, full.to_string()),
    }
}

#[cfg(test)]
#[path = "service_heartbeat_tests.rs"]
mod tests;
