//! What an HMI command does (specification 39, §12.8): its target, the write, the number its
//! limits apply to, its interlock and what confirms it, all from the control on the page.

use std::time::Duration;

use serde_json::{json, Value as Json};

use super::write::WriteAction;
use crate::hmi::catalogue::{ComponentCategory, ComponentDef};
use crate::hmi::member_paths::MemberPath;
use crate::hmi::page_files::{
    HmiBindingDirection, HmiButtonAction, HmiObject, HmiScalar, MODE_NAMES,
};

/// How long a command may take to be confirmed when the control does not say.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a pulse lasts when the pulse button does not say.
pub const DEFAULT_PULSE: Duration = Duration::from_secs(1);
/// Controls whose command is confirmed by the value read back.
/// The controls whose entry is a number typed as text.
const NUMBER_ENTRIES: &[&str] = &["setpoint", "numeric-input"];

const VALUE_CONTROLS: &[&str] = &[
    "setpoint",
    "numeric-input",
    "slider",
    "switch",
    "selector",
    "mode-selector",
];
/// The controls that act on the value they show (specification 39 §5.8).
const SHOWING_CONTROLS: &[&str] = &["switch", "selector", "mode-selector", "slider"];
/// Why a control that shows no value acts on nothing (§5.8): a word of the display's text table,
/// so that the page says it in its language.
pub const NOT_KNOWN: &str = "The value of this control is not known, so it sends nothing.";

/// What confirms a command.
#[derive(Debug, Clone, PartialEq)]
pub enum Expected {
    /// The feedback reports this value.
    Value(Json),
    /// The feedback reports the value read back after the write.
    Readback,
}

/// A command, ready to be checked and written.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// The point written: the control's writing binding.
    pub path: MemberPath,
    /// What is written.
    pub action: WriteAction,
    /// What was asked, for the audit log.
    pub requested: Json,
    /// The number the limits apply to.
    pub number: Option<f64>,
    /// For a pulse: how long it lasts.
    pub pulse: Option<Duration>,
    /// The BOOL permissive and its reason text.
    pub interlock: Option<(MemberPath, Option<MemberPath>)>,
    /// The point and value that confirm the command.
    pub confirm: Option<(MemberPath, Expected)>,
    /// How long confirmation may take.
    pub timeout: Duration,
}

/// The plan for a request to a control, or why the request does not fit the control. `shown` is
/// the value the control showed when it sent the request (`Json::Null` when it showed none); a
/// sender that does not say leaves it out.
pub fn plan(
    object: &HmiObject,
    component: &ComponentDef,
    action: &str,
    value: Option<&Json>,
    shown: Option<&Json>,
) -> Result<Plan, String> {
    if object.command.is_some() {
        return Err("a method command is sent as `invoke`".into());
    }
    let writing = object
        .bind
        .iter()
        .find(|(_, binding)| binding.direction != HmiBindingDirection::Read);
    let (Some((_, binding)), ComponentCategory::Control) = (writing, component.category) else {
        return Err(format!("\"{}\" sends no command", object.id));
    };
    let path = parse(&binding.path)?;
    // A control that acts on the value it shows does nothing while it shows none (§5.8).
    if SHOWING_CONTROLS.contains(&component.kind.as_str()) && shown == Some(&Json::Null) {
        return Err(NOT_KNOWN.into());
    }
    let (write, requested, pulse, expect) = match component.kind.as_str() {
        "button" => {
            let own = object.action.unwrap_or(HmiButtonAction::Set);
            let name = match own {
                HmiButtonAction::Set => "set",
                HmiButtonAction::Reset => "reset",
                HmiButtonAction::Toggle => "toggle",
            };
            if action != name {
                return Err(format!("this button sends {name}, not {action}"));
            }
            match own {
                HmiButtonAction::Set => (
                    WriteAction::Set(json!(true)),
                    json!(true),
                    None,
                    Some(json!(true)),
                ),
                HmiButtonAction::Reset => (
                    WriteAction::Set(json!(false)),
                    json!(false),
                    None,
                    Some(json!(false)),
                ),
                HmiButtonAction::Toggle => (WriteAction::Toggle, json!("toggle"), None, None),
            }
        }
        "pulse-button" => {
            if action != "pulse" {
                return Err(format!("this pulse button sends pulse, not {action}"));
            }
            let pulse = object.pulse.map_or(DEFAULT_PULSE, Duration::from_secs_f64);
            (
                WriteAction::Set(json!(true)),
                json!(true),
                Some(pulse),
                Some(json!(true)),
            )
        }
        "selector" | "mode-selector" => {
            if action != "write" {
                return Err(format!("this control sends write, not {action}"));
            }
            let value = value.ok_or("a write needs a value")?;
            let (name, index) = choice(object, value)?;
            let write = if component.kind == "selector" {
                WriteAction::Choice {
                    name: name.clone(),
                    index,
                }
            } else {
                WriteAction::Set(json!(name))
            };
            (write, json!(name), None, None)
        }
        // A click on a switch writes the opposite of the value it shows (§5.8), which the page
        // sends; without it, the switch shows no value.
        "switch" if action == "toggle" => {
            let shown = match shown {
                Some(Json::Bool(shown)) => *shown,
                None | Some(Json::Null) => return Err(NOT_KNOWN.into()),
                Some(other) => return Err(format!("a switch shows TRUE or FALSE, not {other}")),
            };
            (WriteAction::Set(json!(!shown)), json!(!shown), None, None)
        }
        kind if VALUE_CONTROLS.contains(&kind) => {
            if action != "write" {
                return Err(format!("this control sends write, not {action}"));
            }
            let value = value.ok_or("a write needs a value")?;
            // A number entry as the operator typed it, a comma or a point before the decimals
            // (specification 39 §12.8); its limits are checked as any number's.
            let value = if NUMBER_ENTRIES.contains(&kind) && value.is_string() {
                crate::hmi::setting_values::entered_number(value)
                    .map(|(number, _)| number)
                    .ok_or_else(|| format!("enter a number, not {value}"))?
            } else {
                value.clone()
            };
            (WriteAction::Set(value.clone()), value, None, None)
        }
        _ => return Err(format!("\"{}\" sends no command", object.id)),
    };
    let number = matches!(write, WriteAction::Set(_))
        .then(|| requested.as_f64())
        .flatten();
    let interlock = interlock(object)?;
    let confirm = match object.bind.get("feedback") {
        Some(feedback) => {
            let expected = object
                .expect
                .as_ref()
                .map(scalar)
                .or(expect)
                .map_or(Expected::Readback, Expected::Value);
            Some((parse(&feedback.path)?, expected))
        }
        None if VALUE_CONTROLS.contains(&component.kind.as_str()) => {
            Some((path.clone(), Expected::Readback))
        }
        None => None,
    };
    let timeout = object
        .timeout
        .map_or(DEFAULT_TIMEOUT, Duration::from_secs_f64);
    Ok(Plan {
        path,
        action: write,
        requested,
        number,
        pulse,
        interlock,
        confirm,
        timeout,
    })
}

/// A control's `interlock` permit and its `interlock-reason` text (§12.6, §12.8 item 4): a command
/// is refused while the permit does not read TRUE.
pub(crate) fn interlock(
    object: &HmiObject,
) -> Result<Option<(MemberPath, Option<MemberPath>)>, String> {
    let Some(permit) = object.bind.get("interlock") else {
        return Ok(None);
    };
    let permit = parse(&permit.path)?;
    let reason = object
        .bind
        .get("interlock-reason")
        .map(|reason| parse(&reason.path))
        .transpose()?;
    Ok(Some((permit, reason)))
}

/// The option a selector or mode selector was asked to write, as the control lists it, and its
/// position (§5.8).
pub(crate) fn choice(object: &HmiObject, value: &Json) -> Result<(String, usize), String> {
    let offered: Vec<&str> = match &object.options {
        Some(options) => options.iter().map(String::as_str).collect(),
        None if object.kind == "mode-selector" => MODE_NAMES.to_vec(),
        None => Vec::new(),
    };
    let what = if object.kind == "mode-selector" {
        "modes"
    } else {
        "options"
    };
    value
        .as_str()
        .and_then(|asked| {
            offered
                .iter()
                .position(|option| option.eq_ignore_ascii_case(asked))
        })
        .map(|index| (offered[index].to_string(), index))
        .ok_or_else(|| format!("{value} is not one of its {what}"))
}

/// Whether a value read from the program matches the expected one; numbers within the
/// precision of a REAL.
pub fn matches(read: &Json, expected: &Json) -> bool {
    match (read.as_f64(), expected.as_f64()) {
        (Some(read), Some(expected)) => (read - expected).abs() <= 1e-5 * expected.abs().max(1.0),
        _ => read == expected,
    }
}

fn parse(path: &str) -> Result<MemberPath, String> {
    MemberPath::parse(path).map_err(|error| format!("\"{path}\" is not a valid path: {error}"))
}

fn scalar(value: &HmiScalar) -> Json {
    match value {
        HmiScalar::Bool(flag) => json!(flag),
        HmiScalar::Number(number) => json!(number),
        HmiScalar::Text(text) => json!(text),
    }
}
