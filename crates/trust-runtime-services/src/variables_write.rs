//! `variables.write` (specification 11 §6.8.1, "Writing variables"; specification 48 RU-62, RU-63):
//! a service's write of a set of the program's variables by their paths. Only a service token sends
//! it (specification 11 §6.9.6), and each path must name a variable the token's service may write,
//! one its `writes` names, which the signed image carries (specification 44 §5.14). The whole set is
//! checked before any of it is queued, and the next cycle's start writes it together through the
//! Live Values write queue, as the program's input and never as a force, on a computer and on a chip
//! alike: both apply that queue at every cycle start, with the debug surface on or off.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::{json, Value as Json};
use smol_str::SmolStr;
use trust_runtime_core::native::image::Image;
use trust_runtime_core::native::reflect::{resolve_writable, Located};
use trust_runtime_core::ports::WallClock;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::Value;

use crate::live_values::queues::PendingVarTarget;
use crate::live_values::value_grammar::json_value;
use crate::live_values::var_requests::VariableQueuePort;
use crate::ports::{AccessPort, ResourcePort};
use crate::protocol::ControlResponse;

/// The most values one request writes.
pub const MAX_WRITES: usize = 256;

/// `variables.write {writes}`: `{service, count}`, the service the token `auth` speaks for and the
/// number of values the next cycle's start writes. The parameters are read first, then the token,
/// the resource's state, and every path and value of the set.
pub fn write<H>(id: u64, auth: Option<&str>, params: Option<&Json>, host: &H) -> ControlResponse
where
    H: AccessPort + WallClock + ResourcePort + VariableQueuePort,
{
    let writes = match writes(params) {
        Ok(writes) => writes,
        Err(reason) => return ControlResponse::error(id, format!("invalid params: {reason}")),
    };
    // A service token does not expire, so the clock only prunes claimed tokens that did.
    let now = host.utc_ms().map_or(0, |utc_ms| utc_ms / 1_000);
    let Some(service) = auth.and_then(|token| host.pairing()?.service_of(token, now)) else {
        return ControlResponse::error(id, "variables.write needs a service token".to_string());
    };
    let state = host.resource_state();
    if state != ResourceState::Running {
        let state = format!("{state:?}").to_ascii_lowercase();
        return ControlResponse::error(id, format!("the program does not run: {state}"));
    }
    let mut admitted = Err(String::from("variables unavailable: no program"));
    host.with_program_image(&mut |image| admitted = admit(image, &service, &writes));
    let admitted = match admitted {
        Ok(admitted) => admitted,
        Err(reason) => return ControlResponse::error(id, reason),
    };
    let count = admitted.len();
    match host.enqueue_var_writes(admitted) {
        Ok(()) => ControlResponse::ok(id, json!({ "service": service, "count": count })),
        Err(full) => ControlResponse::error(id, full.to_string()),
    }
}

/// The writes `params` name: exactly `writes`, 1 to [`MAX_WRITES`] objects of exactly `path`, a
/// variable's path, and `value`.
fn writes(params: Option<&Json>) -> Result<Vec<(String, Json)>, &'static str> {
    let Some(Json::Object(params)) = params else {
        return Err("writes is required");
    };
    if params.keys().any(|key| key != "writes") {
        return Err("only writes is taken");
    }
    let Some(Json::Array(writes)) = params.get("writes") else {
        return Err("writes must be an array of {path, value} objects");
    };
    if writes.is_empty() || writes.len() > MAX_WRITES {
        return Err("writes must hold 1 to 256 values");
    }
    writes
        .iter()
        .map(|write| {
            let Json::Object(write) = write else {
                return Err("writes must be an array of {path, value} objects");
            };
            let path = write.get("path").and_then(Json::as_str);
            match (path, write.get("value"), write.len()) {
                (Some(path), Some(value), 2) => Ok((path.to_string(), value.clone())),
                _ => Err("each write is exactly a path and a value"),
            }
        })
        .collect()
}

/// The set of `service` admitted against `image`: each path one the service's `writes` names,
/// compared as the variables the paths name, and each value one its variable's type takes; the
/// first refusal refuses the set.
fn admit(
    image: &Image<'_>,
    service: &str,
    writes: &[(String, Json)],
) -> Result<Vec<(PendingVarTarget, Value)>, String> {
    let services = image.services().map_err(|_| {
        String::from("variables unavailable: the program image's service table does not read")
    })?;
    let Some(own) = services
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(service))
    else {
        return Err(format!("service {service} may write no variable"));
    };
    let allowed: Vec<Located> = own
        .paths
        .iter()
        .filter_map(|path| resolve_writable(image, path).ok())
        .collect();
    writes
        .iter()
        .map(|(path, value)| {
            let at = resolve_writable(image, path)
                .ok()
                .filter(|at| allowed.contains(at))
                .ok_or_else(|| format!("service {service} may not write {path}"))?;
            let value = json_value(image, at.type_index, value)
                .map_err(|reason| format!("invalid value for {path}: {reason}"))?;
            Ok((PendingVarTarget::Global(SmolStr::new(path)), value))
        })
        .collect()
}

#[cfg(test)]
#[path = "variables_write_tests.rs"]
mod tests;
