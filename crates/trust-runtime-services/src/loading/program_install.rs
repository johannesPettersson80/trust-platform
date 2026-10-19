//! `program.install {transfer}`: the request rule of an install on every runtime (specification 34
//! §11.2; decisions Q11 and G13).
//!
//! [`request`] refuses what the resource state or the platform does not allow, and takes the
//! complete transfer only after those checks pass. It then runs the shared checks of
//! [`super::install::check`], whose retained-value check the platform supplies, and commits the
//! container through the platform's program store. The reply is the identity the store gives the
//! program. Nothing here holds a flash, a file or a clock.

use alloc::string::ToString;

use serde_json::{json, Value};
use trust_runtime_core::scheduler::ResourceState;

use super::install;
use crate::dispatch::transfers::{take_complete, transfer_member};
use crate::ports::{ControlHost, ProgramIdentity};
use crate::protocol::ControlResponse;
use crate::resource::replies::{hung_refusal, resource_not_running};
use crate::transfer::wire;

/// Answers `program.install {transfer}`. The request pipeline refuses every role below Admin before
/// this runs.
///
/// The first failing check is the reply, in this order: the params (`missing params`, then the
/// `transfer` member's refusal); a hung resource; a resource that is not Stopped, refused as
/// `resource_not_running` with its state; a platform without a program store, which answers
/// `unsupported request`; the complete transfer; the checks of [`install::check`]; and the commit.
/// The transfer is taken only once the state and store checks have passed, so those refusals leave
/// it open. A refusal after it uses the transfer up, as `bytecode.reload {transfer}` does.
#[must_use]
pub fn request<H: ControlHost>(id: u64, params: Option<&Value>, host: &H) -> ControlResponse {
    let Some(params) = params else {
        return ControlResponse::error(id, "missing params".to_string());
    };
    let transfer = match transfer_member(id, params) {
        Ok(transfer) => transfer,
        Err(refusal) => return refusal,
    };
    if host.is_hung() {
        return hung_refusal(id);
    }
    let resource = host.resource_state();
    if !matches!(resource, ResourceState::Stopped) {
        return resource_not_running(id, resource);
    }
    if !host.has_program_store() {
        return ControlResponse::error(id, "unsupported request".to_string());
    }
    let container = match take_complete(id, transfer, host) {
        Ok(container) => container,
        Err(refusal) => return refusal,
    };
    let device = host.device_descriptor();
    if let Err(error) = install::check(&container, &device, |admitted| {
        host.check_retained_values(admitted)
    }) {
        return ControlResponse::error(id, error.to_string());
    }
    match host.commit_program(&container) {
        Ok(identity) => ControlResponse::ok(id, installed_result(identity)),
        Err(error) => ControlResponse::error(id, error),
    }
}

/// The result of an install that committed: `status` and the identity of the program, as
/// `program.active` reports it (specification 34 §11.2).
fn installed_result(identity: ProgramIdentity) -> Value {
    json!({
        "status": "installed",
        "sequence": identity.sequence,
        "program_sha256": wire::hex_text(&identity.program_sha256),
        "bytes": identity.bytes,
    })
}

#[cfg(test)]
mod contract_tests;
