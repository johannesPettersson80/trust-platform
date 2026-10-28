//! Operator actions on an alarm (specification 40, §4, §5.1): the instance's request field, the
//! value written, the role needed and the state that confirms it.

use serde_json::Value as Json;

use super::board::EventKind;
use crate::hmi::commands::write::WriteAction;
use crate::security::AccessRole;

/// One operator action, ready to write.
#[derive(Debug, Clone, PartialEq)]
pub struct AlarmAction {
    /// The record the action makes.
    pub kind: EventKind,
    /// The action's name, as the event log says it.
    pub name: &'static str,
    /// The field of the instance's `Request` member written.
    pub field: &'static str,
    /// The value written.
    pub write: WriteAction,
    /// The lowest role that may send it.
    pub role: AccessRole,
    /// The member of the instance, and its value, that show the action applied.
    pub confirm: (&'static str, Json),
    /// The seconds of a shelving or the text of a comment, for the record.
    pub detail: Option<String>,
    /// A shelving's seconds, for the OpenOT record (specification 40, §5.2).
    pub seconds: Option<u32>,
}

/// The longest shelving: 24 hours.
const MAX_SHELVE_SECONDS: f64 = 86_400.0;

/// A shelving's duration as people say it: whole hours in hours, whole minutes in minutes,
/// otherwise seconds.
fn duration_text(seconds: f64) -> String {
    // The duration is 1 to 86 400 s, so the cast is exact for whole seconds.
    let whole = seconds.round() as u64;
    if (seconds - whole as f64).abs() > f64::EPSILON {
        return format!("{seconds} s");
    }
    if whole.is_multiple_of(3_600) {
        format!("{} h", whole / 3_600)
    } else if whole.is_multiple_of(60) {
        format!("{} min", whole / 60)
    } else {
        format!("{whole} s")
    }
}

/// The action named by a request, or why it is refused.
pub fn alarm_action(action: &str, value: Option<&Json>) -> Result<AlarmAction, String> {
    let flag = |kind, name, field, role, confirm: (&'static str, bool)| {
        if value.is_some() {
            return Err(format!("the action \"{action}\" takes no value"));
        }
        Ok(AlarmAction {
            kind,
            name,
            field,
            write: WriteAction::Set(Json::Bool(true)),
            role,
            confirm: (confirm.0, Json::Bool(confirm.1)),
            detail: None,
            seconds: None,
        })
    };
    match action {
        "acknowledge" => flag(
            EventKind::Acknowledged,
            "Acknowledge",
            "Acknowledge",
            AccessRole::Operator,
            ("Unacknowledged", false),
        ),
        "unshelve" => flag(
            EventKind::Unshelved,
            "Unshelve",
            "Unshelve",
            AccessRole::Operator,
            ("Shelved", false),
        ),
        "reset" => flag(
            EventKind::Reset,
            "Reset",
            "Reset",
            AccessRole::Operator,
            ("Active", false),
        ),
        "remove-from-service" => flag(
            EventKind::OutOfService,
            "Take out of service",
            "RemoveFromService",
            AccessRole::Engineer,
            ("OutOfService", true),
        ),
        "return-to-service" => flag(
            EventKind::InService,
            "Return to service",
            "ReturnToService",
            AccessRole::Engineer,
            ("OutOfService", false),
        ),
        "shelve" => {
            let seconds = value
                .and_then(Json::as_f64)
                .filter(|seconds| (1.0..=MAX_SHELVE_SECONDS).contains(seconds))
                .ok_or_else(|| "a shelving lasts 1 to 86400 seconds".to_string())?;
            Ok(AlarmAction {
                kind: EventKind::Shelved,
                name: "Shelve",
                field: "Shelve",
                write: WriteAction::Set(Json::from(seconds)),
                role: AccessRole::Operator,
                confirm: ("Shelved", Json::Bool(true)),
                detail: Some(duration_text(seconds)),
                // The duration is 1 to 86 400 s.
                seconds: Some(seconds.round() as u32),
            })
        }
        "comment" => {
            // The characters are counted as the field holds them: UTF-16 code units of a
            // `WSTRING[120]` (specification 40, §5 item 3).
            let text = value
                .and_then(Json::as_str)
                .filter(|text| {
                    !text.trim().is_empty()
                        && text.encode_utf16().count() <= super::offers::COMMENT_MAX_CHARS
                })
                .ok_or_else(|| "a comment has 1 to 120 characters".to_string())?;
            Ok(AlarmAction {
                kind: EventKind::Comment,
                name: "Comment",
                field: "Comment",
                write: WriteAction::Set(Json::from(text)),
                role: AccessRole::Operator,
                confirm: ("Request.Comment", Json::from("")),
                detail: Some(text.to_string()),
                seconds: None,
            })
        }
        _ => Err(format!(
            "\"{action}\" is not an alarm action; use acknowledge, shelve, unshelve, reset, comment, remove-from-service or return-to-service"
        )),
    }
}
