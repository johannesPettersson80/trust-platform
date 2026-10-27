//! The runtime requests behind a debugger's write of a variable (specification 13, "setVariable
//! and setExpression"; specification 11, "Live Values"): `debug.set`, which writes into a held scan
//! at once and otherwise checks the write and names the variable's path, and `set`, `var.force` and
//! `var.unforce`, which queue a write and hold and release a force by that path. A request of a
//! write that the runtime refuses as busy is sent again after a short wait (`write_request`), the
//! I/O requests of the Live Values too.

use std::thread;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};

use super::{answer, ControlResponse, RemoteResult, RemoteSession};

/// The stable code of a request that the runtime refused because its command queue was full;
/// nothing was queued (specification 11 §6.9).
const RESOURCE_BUSY: &str = "resource_busy";

/// The waits before each new send of a request that the runtime refused as busy: at most five
/// sends more, 1,475 ms in all (specification 13, "setVariable and setExpression").
const BUSY_RETRY_DELAYS: [Duration; 5] = [
    Duration::from_millis(75),
    Duration::from_millis(150),
    Duration::from_millis(250),
    Duration::from_millis(400),
    Duration::from_millis(600),
];

/// The variable a debugger's write names: as a listing showed it, or by a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::adapter) enum WriteTarget {
    /// The child `name` of the listing whose handle is `variables_reference`.
    Listed {
        /// The handle of the listing.
        variables_reference: u32,
        /// The name the listing showed.
        name: String,
    },
    /// A variable path, its first name looked up among the variables of the frame `frame_id`
    /// first while the debugger holds the scan.
    Path {
        /// The path.
        path: String,
        /// The frame of the stack the client evaluates in.
        frame_id: Option<u32>,
    },
}

/// The runtime's answer to `debug.set`: the variable's path when it has one, its type and value,
/// whether the value was written into the held scan, and the address of an I/O row.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub(in crate::adapter) struct DebugSetReply {
    /// The path the variable is queued and forced by; none for a variable of a frame.
    #[serde(default)]
    pub path: Option<String>,
    /// The variable's type.
    #[serde(default, rename = "type")]
    pub type_name: Option<String>,
    /// The value, written or checked.
    #[serde(default)]
    pub value: Option<String>,
    /// Whether the value went into the held scan.
    #[serde(default)]
    pub written: bool,
    /// The address of an I/O row, which the I/O requests write.
    #[serde(default)]
    pub address: Option<String>,
}

impl RemoteSession {
    /// The request `kind` of a write, a force or a release with `params`, sent again after each of
    /// [`BUSY_RETRY_DELAYS`] while the runtime refuses it as busy, and answered with the runtime's
    /// last answer (specification 13, "setVariable and setExpression").
    pub(super) fn write_request(&mut self, kind: &str, params: &Value) -> RemoteResult<Value> {
        let mut response = self.exchange(kind, Some(params.clone()))?;
        for delay in BUSY_RETRY_DELAYS {
            if !is_busy(&response) {
                break;
            }
            thread::sleep(delay);
            response = self.exchange(kind, Some(params.clone()))?;
        }
        answer(response)
    }

    /// `debug.set` of `target`: with `value`, written into the held scan at once or else checked;
    /// without it, read; `force` refuses a variable that no force can hold, a frame's.
    pub(in crate::adapter) fn debug_set(
        &mut self,
        target: &WriteTarget,
        value: Option<&str>,
        force: bool,
    ) -> RemoteResult<DebugSetReply> {
        let mut params = match target {
            WriteTarget::Listed {
                variables_reference,
                name,
            } => json!({ "variables_reference": variables_reference, "name": name }),
            WriteTarget::Path { path, frame_id } => {
                let mut params = json!({ "path": path });
                if let Some(frame_id) = frame_id {
                    params["frame_id"] = json!(frame_id);
                }
                params
            }
        };
        if let Some(value) = value {
            params["value"] = json!(value);
        }
        if force {
            params["force"] = json!(true);
        }
        let payload = self.write_request("debug.set", &params)?;
        serde_json::from_value(payload).map_err(|err| err.to_string())
    }

    /// `set` of the variable `path` names: one write of `value` at the next cycle start.
    pub(in crate::adapter) fn var_set(&mut self, path: &str, value: &str) -> RemoteResult<()> {
        let params = json!({ "target": format!("global:{path}"), "value": value });
        let _ = self.write_request("set", &params)?;
        Ok(())
    }

    /// `var.force` of the variable `path` names at `value`.
    pub(in crate::adapter) fn var_force(&mut self, path: &str, value: &str) -> RemoteResult<()> {
        let params = json!({ "target": format!("global:{path}"), "value": value });
        let _ = self.write_request("var.force", &params)?;
        Ok(())
    }

    /// `var.unforce` of the variable `path` names.
    pub(in crate::adapter) fn var_unforce(&mut self, path: &str) -> RemoteResult<()> {
        let params = json!({ "target": format!("global:{path}") });
        let _ = self.write_request("var.unforce", &params)?;
        Ok(())
    }
}

/// Whether the runtime refused a request because its command queue was full.
fn is_busy(response: &ControlResponse) -> bool {
    !response.ok && response.error_code.as_deref() == Some(RESOURCE_BUSY)
}
