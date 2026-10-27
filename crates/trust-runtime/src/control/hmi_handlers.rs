//! The HMI Builder's Live mode reads the bound paths of the page being edited (specification 39,
//! §11 item 6): `hmi.read` answers the values, the cycle count and the missing paths as the
//! runtime display's stream does (§11.1).

use std::collections::BTreeMap;
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

use super::{ControlResponse, ControlState};
use crate::hmi::display_values::{stale_limit, MIN_PERIOD};
use crate::hmi::member_paths::MemberPath;

/// The most paths one request reads.
const MAX_PATHS: usize = 1_000;
/// How long the runtime thread may take to answer.
const READ_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HmiReadParams {
    paths: Vec<String>,
}

pub(super) fn handle_hmi_read(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: HmiReadParams = match params.map(serde_json::from_value) {
        Some(Ok(params)) => params,
        Some(Err(error)) => return ControlResponse::error(id, format!("invalid params: {error}")),
        None => return ControlResponse::error(id, "missing params".into()),
    };
    if params.paths.len() > MAX_PATHS {
        return ControlResponse::error(id, "hmi.read takes at most 1000 paths".into());
    }
    let mut parsed = Vec::with_capacity(params.paths.len());
    for path in &params.paths {
        match MemberPath::parse(path) {
            Ok(member) => parsed.push(member),
            Err(error) => return ControlResponse::error(id, format!("invalid params: {error}")),
        }
    }
    // Live mode reads at the program's next Hmi.Display call (specification 39, §18 Phase 5).
    let Some(read) = state
        .resource
        .hmi_exchange()
        .read_next(&parsed, READ_TIMEOUT)
    else {
        return ControlResponse::error(
            id,
            "no values: the program does not call Hmi.Display in a running task".into(),
        );
    };
    let mut values = BTreeMap::new();
    let mut missing = Vec::new();
    for ((path, value), present) in params.paths.iter().zip(read.values).zip(read.present) {
        if !present {
            missing.push(path.clone());
        }
        values.insert(path.clone(), value);
    }
    // Live mode reads at the display's shortest period; its values are stale after the runtime's
    // limit without news, as the runtime display's are (§5.3 item 2, §11.1).
    ControlResponse::ok(
        id,
        json!({
            "cycle": read.cycle,
            "values": values,
            "missing": missing,
            "period_ms": MIN_PERIOD.as_millis(),
            "stale_ms": stale_limit(MIN_PERIOD).as_millis(),
        }),
    )
}
