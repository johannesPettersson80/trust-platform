//! A loop's mode signal, as transfers and parameter transactions bind it (protocol annex E6.2, E7.1;
//! specification 35 A6.1): a BOOL with the value that means Manual, whose other value means Auto, or
//! an enumeration with the member that means Manual and the member that means Auto, whose other
//! members mean neither.

use crate::value::Value;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use trust_runtime_core::native::image::Image;
use trust_runtime_core::native::memory::enum_value;

/// The mode signal of a binding.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Mode {
    pub(super) symbol: String,
    /// The value that means Manual: a boolean for a BOOL, a member's name for an enumeration.
    pub(super) manual_value: ModeValue,
    /// The member that means Auto, for an enumeration only. A BOOL binding omits it, so its
    /// serialization, and with it the binding revision, is the one it had before enumerations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) auto_value: Option<String>,
}

/// A value of a mode signal as a binding names it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub(super) enum ModeValue {
    /// A BOOL's value.
    Bool(bool),
    /// The name of an enumeration's member.
    Member(String),
}

impl Mode {
    /// Checks the binding against the signal's value `current` in the program of `image`: a BOOL
    /// takes a boolean Manual value and no Auto value; an enumeration takes the names of two
    /// different members of its type.
    pub(super) fn check(&self, current: &Value, image: &Image<'_>) -> Result<(), String> {
        match (current, &self.manual_value, &self.auto_value) {
            (Value::Bool(_), ModeValue::Bool(_), None) => Ok(()),
            (Value::Bool(_), _, _) => Err(format!(
                "mode {}: a BOOL mode takes a boolean manual_value and no auto_value",
                self.symbol
            )),
            (Value::Enum(enumeration), ModeValue::Member(manual), Some(auto)) => {
                let type_name = enumeration.type_name();
                for member in [manual, auto] {
                    if enum_value(image, type_name, member).is_none() {
                        return Err(format!(
                            "mode {}: {member} is not a value of {type_name}",
                            self.symbol
                        ));
                    }
                }
                if manual.eq_ignore_ascii_case(auto) {
                    return Err(format!(
                        "mode {}: manual_value and auto_value name the same value",
                        self.symbol
                    ));
                }
                Ok(())
            }
            (Value::Enum(enumeration), _, _) => Err(format!(
                "mode {}: an enumeration mode takes the names of two values of {} as manual_value and auto_value",
                self.symbol,
                enumeration.type_name()
            )),
            _ => Err(format!(
                "mode {} must be BOOL or an enumeration",
                self.symbol
            )),
        }
    }

    /// The value the signal holds in Manual (`manual`) or in Auto, for a binding that
    /// [`Mode::check`] accepted against `current`.
    pub(super) fn target(
        &self,
        manual: bool,
        current: &Value,
        image: &Image<'_>,
    ) -> Result<Value, String> {
        match (&self.manual_value, current) {
            (ModeValue::Bool(value), Value::Bool(_)) => Ok(Value::Bool(*value == manual)),
            (ModeValue::Member(member), Value::Enum(enumeration)) => {
                let member = if manual {
                    member
                } else {
                    self.auto_value.as_ref().unwrap_or(member)
                };
                enum_value(image, enumeration.type_name(), member)
                    .ok_or_else(|| format!("mode {}: {member} is not a value", self.symbol))
            }
            _ => Err(format!(
                "mode {} must be BOOL or an enumeration",
                self.symbol
            )),
        }
    }

    /// Whether `current` is the value that means Manual (`manual`) or Auto. An enumeration's
    /// other members are neither.
    pub(super) fn holds(&self, manual: bool, current: &Value) -> bool {
        match (&self.manual_value, current) {
            (ModeValue::Bool(value), Value::Bool(current)) => (*current == *value) == manual,
            (ModeValue::Member(member), Value::Enum(enumeration)) => {
                let wanted = if manual {
                    Some(member)
                } else {
                    self.auto_value.as_ref()
                };
                wanted.is_some_and(|wanted| enumeration.variant_name().eq_ignore_ascii_case(wanted))
            }
            _ => false,
        }
    }
}

/// The signal's value `current` as status reports it (E6.3): a BOOL as a boolean, an enumeration
/// as the name of its member as its type declares it.
pub(super) fn shown(symbol: &str, current: &Value) -> Result<JsonValue, String> {
    match current {
        Value::Bool(value) => Ok(JsonValue::Bool(*value)),
        Value::Enum(enumeration) => Ok(JsonValue::String(enumeration.variant_name().to_string())),
        _ => Err(format!("mode {symbol} must be BOOL or an enumeration")),
    }
}
