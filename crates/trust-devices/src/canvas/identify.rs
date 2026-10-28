//! `identify`: a runtime node's `Identify` (specification 25 §5.12; specification 11, "Identify").
//! The canvas sends the runtime's name and the control endpoint and token it reaches the runtime
//! with; this sends `identify {"duration_s": 10}` there and answers the runtime's own answer as the
//! notice: `Identifying <name>` with the seconds the runtime reports, or its refusal as it is.

use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};

use super::act::{CanvasActRequest, CanvasNotice, CanvasReply};
use crate::actions::write;
use crate::texts::graph as texts;
use crate::view::lifecycle::IDENTIFY_SECONDS;

/// The wait for the runtime's answer.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(4);

/// The arguments of `identify`: the runtime's name.
#[derive(Debug, Deserialize)]
pub(super) struct IdentifyArgs {
    #[serde(default)]
    name: String,
}

/// `identify`: asks the runtime that `request.runtime` reaches. Without a runtime that answers,
/// nothing is sent and the notice says so.
pub(super) fn identify(request: &CanvasActRequest, args: &IdentifyArgs) -> CanvasReply {
    let name = match args.name.trim() {
        "" => texts::UNNAMED_PEER,
        name => name,
    };
    let Some(runtime) = request.live() else {
        return CanvasReply::warning(texts::identify_unanswered(name));
    };
    let params = json!({ "duration_s": IDENTIFY_SECONDS });
    match write::send_with_token(
        &runtime.endpoint,
        runtime.token.as_deref(),
        "identify",
        &params,
        REQUEST_TIMEOUT,
    ) {
        Ok(answer) => {
            let seconds = answer
                .get("duration_s")
                .and_then(Value::as_u64)
                .unwrap_or(IDENTIFY_SECONDS);
            CanvasReply {
                notice: Some(CanvasNotice::info(texts::identifying(name, seconds))),
                ..CanvasReply::nothing()
            }
        }
        Err(message) => CanvasReply::warning(message),
    }
}
