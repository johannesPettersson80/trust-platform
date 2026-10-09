//! Commands that call a method of a function block instance (specification 39, §21.2): the
//! control names an interface, one of its methods and the instance, with fixed arguments or the
//! value the control sends. These describe what a control may call; they are never client input.
use crate::hmi::page_files::HmiScalar;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// How a panel names the instance it is opened for (§21.2 item 1).
pub const PANEL_INSTANCE: &str = "$instance";

/// One fixed scalar or the control's entered value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Argument {
    /// Bind the entered control value; false is invalid.
    Input {
        /// Must be true.
        input: bool,
    },
    /// Fixed value declared in the active bundle.
    Constant(HmiScalar),
}

/// A method command as a page or panel declares it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MethodCommand {
    /// The interface the instance implements, with its namespace.
    pub interface: String,
    /// A method the interface declares.
    pub method: String,
    /// The instance path, or in a panel `$instance` or `$instance.<member>`.
    pub instance: String,
    /// The method's inputs: fixed values, or the value the control sends.
    #[serde(default)]
    pub arguments: BTreeMap<String, Argument>,
}

impl MethodCommand {
    /// Whether the command names the instance its panel is opened for, `$instance` or a block
    /// inside it, `$instance.<member>` (§21.2 item 1).
    pub fn names_panel_instance(&self) -> bool {
        self.instance == PANEL_INSTANCE || self.panel_member().is_some()
    }

    /// The member of `$instance.<member>`.
    fn panel_member(&self) -> Option<&str> {
        self.instance
            .strip_prefix(PANEL_INSTANCE)
            .and_then(|rest| rest.strip_prefix('.'))
    }

    /// Check syntax and bounds without touching a running PLC.
    pub fn validate(&self) -> Result<(), String> {
        let qualified = qualified_name;
        if !qualified(&self.interface) || !qualified(&self.method) || self.method.contains('.') {
            return Err("a command names an interface and one of its methods".into());
        }
        let instance = match self.panel_member() {
            Some(member) => {
                crate::hmi::member_paths::MemberPath::parse(&format!("Instance.{member}")).is_ok()
            }
            None => {
                self.instance == PANEL_INSTANCE
                    || crate::hmi::member_paths::MemberPath::parse(&self.instance).is_ok()
            }
        };
        if !instance {
            return Err(
                "a command names an instance path, `$instance` or `$instance.<member>`".into(),
            );
        }
        if self.arguments.len() > 31 {
            return Err("a command has at most 31 arguments".into());
        }
        let mut seen = BTreeSet::new();
        for (name, value) in &self.arguments {
            if !qualified(name) || name.contains('.') || !seen.insert(name.to_ascii_uppercase()) {
                return Err(format!("the argument `{name}` must be an input named once"));
            }
            match value {
                Argument::Input { input: false } => {
                    return Err("an entered argument is written `{ input = true }`".into())
                }
                Argument::Constant(HmiScalar::Number(n)) if !n.is_finite() => {
                    return Err(format!("the argument `{name}` must be a finite number"))
                }
                Argument::Constant(HmiScalar::Text(s)) if s.len() > 4096 => {
                    return Err(format!("the argument `{name}` is too long"))
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn qualified_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && name.split('.').all(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command() -> MethodCommand {
        MethodCommand {
            interface: "Plant.IDevice".into(),
            method: "SetMode".into(),
            instance: "Main.Pumps[2]".into(),
            arguments: BTreeMap::from([(
                "Value".into(),
                Argument::Constant(HmiScalar::Text("Manual".into())),
            )]),
        }
    }

    #[test]
    fn instance_paths_and_the_panel_instance_are_supported() {
        let mut c = command();
        assert!(c.validate().is_ok());
        assert!(!c.names_panel_instance());
        c.instance = "$instance".into();
        assert!(c.validate().is_ok());
        assert!(c.names_panel_instance());
        c.instance = "$instance.Outer".into();
        assert!(c.validate().is_ok(), "a block inside the panel's instance");
        assert!(c.names_panel_instance());
        for wrong in ["$instance.", "$instance.1x", "$instances", "Main..Pump", ""] {
            c.instance = wrong.into();
            assert!(c.validate().is_err(), "{wrong}");
        }
    }

    #[test]
    fn case_duplicate_arguments_are_rejected() {
        let mut c = command();
        c.arguments
            .insert("vALUE".into(), Argument::Input { input: true });
        assert!(c.validate().is_err());
    }

    #[test]
    fn invalid_declarations_do_not_become_commands() {
        let mut c = command();
        c.method = "Other.SetMode".into();
        assert!(c.validate().is_err());
        c = command();
        c.arguments
            .insert("Value".into(), Argument::Input { input: false });
        assert!(c.validate().is_err());
        c.arguments.insert(
            "Value".into(),
            Argument::Constant(HmiScalar::Number(f64::NAN)),
        );
        assert!(c.validate().is_err());
        c.arguments.insert(
            "Value".into(),
            Argument::Constant(HmiScalar::Text("x".repeat(4097))),
        );
        assert!(c.validate().is_err());
    }

    #[test]
    fn a_command_carries_no_role_source_or_context_of_its_own() {
        // The control's `role` is the role a command needs (§21.2 item 1); nothing else.
        let value = serde_json::json!({
            "interface": "Plant.IRunDevice", "method": "Start",
            "instance": "Main.P101", "source": "maintenance"
        });
        assert!(serde_json::from_value::<MethodCommand>(value).is_err());
        let plain = serde_json::json!({
            "interface": "Plant.IRunDevice", "method": "Start", "instance": "$instance"
        });
        let parsed: MethodCommand = serde_json::from_value(plain).expect("a plain command");
        assert!(parsed.arguments.is_empty());
    }
}
