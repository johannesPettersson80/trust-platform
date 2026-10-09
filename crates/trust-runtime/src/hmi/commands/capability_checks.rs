//! Method commands checked against the compiled program and the control that sends them
//! (specification 39, §21.2 item 2): the block implements the interface, the interface declares
//! the method, the method returns BOOL and takes inputs only, and every input fits its type, the
//! control's options and its limits.
use super::capability::{Argument, MethodCommand};
use crate::{
    hmi::page_files::{HmiObject, HmiScalar},
    program_model::{ClassDef, FunctionBlockBase, FunctionBlockDef, InterfaceDef, MethodDef},
    runtime::RuntimeMetadata,
    value::Value,
};
use serde_json::{json, Value as Json};
use smol_str::SmolStr;
use std::collections::BTreeSet;
use trust_hir::symbols::ParamDirection;

/// Whether a class or FB implements an interface, including inherited capabilities.
pub fn implements(metadata: &RuntimeMetadata, block: &str, interface: &str) -> bool {
    implements_in(
        metadata.function_blocks(),
        metadata.classes(),
        metadata.interfaces(),
        block,
        interface,
    )
}

pub(crate) fn implements_in(
    blocks_by_name: &indexmap::IndexMap<smol_str::SmolStr, FunctionBlockDef>,
    classes: &indexmap::IndexMap<smol_str::SmolStr, ClassDef>,
    interface_defs: &indexmap::IndexMap<smol_str::SmolStr, InterfaceDef>,
    block: &str,
    interface: &str,
) -> bool {
    let mut blocks = vec![block.to_owned()];
    let mut visited = BTreeSet::new();
    while let Some(name) = blocks.pop() {
        if !visited.insert(name.to_ascii_uppercase()) {
            continue;
        }
        let (interfaces, base) = if let Some(definition) = blocks_by_name
            .values()
            .find(|item| item.name.eq_ignore_ascii_case(&name))
        {
            (
                &definition.interfaces,
                definition.base.as_ref().map(|base| match base {
                    FunctionBlockBase::FunctionBlock(name) | FunctionBlockBase::Class(name) => {
                        name.to_string()
                    }
                }),
            )
        } else if let Some(definition) = classes
            .values()
            .find(|item| item.name.eq_ignore_ascii_case(&name))
        {
            (
                &definition.interfaces,
                definition.base.as_ref().map(ToString::to_string),
            )
        } else {
            continue;
        };
        if let Some(base) = base {
            blocks.push(base);
        }
        let mut pending: Vec<_> = interfaces.iter().map(ToString::to_string).collect();
        let mut checked = BTreeSet::new();
        while let Some(candidate) = pending.pop() {
            if candidate.eq_ignore_ascii_case(interface) {
                return true;
            }
            if !checked.insert(candidate.to_ascii_uppercase()) {
                continue;
            }
            if let Some(definition) = interface_defs
                .values()
                .find(|item| item.name.eq_ignore_ascii_case(&candidate))
            {
                if let Some(base) = &definition.base {
                    pending.push(base.to_string());
                }
            }
        }
    }
    false
}

pub(crate) fn declared_method<'a>(
    metadata: &'a RuntimeMetadata,
    interface: &str,
    method: &str,
) -> Option<&'a MethodDef> {
    declared_method_in(metadata.interfaces(), interface, method)
}

pub(crate) fn declared_method_in<'a>(
    interface_defs: &'a indexmap::IndexMap<smol_str::SmolStr, InterfaceDef>,
    interface: &str,
    method: &str,
) -> Option<&'a MethodDef> {
    let mut name = interface.to_owned();
    let mut visited = BTreeSet::new();
    while visited.insert(name.to_ascii_uppercase()) {
        let definition = interface_defs
            .values()
            .find(|definition| definition.name.eq_ignore_ascii_case(&name))?;
        if let Some(found) = definition
            .methods
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(method))
        {
            return Some(found);
        }
        name = definition.base.as_ref()?.to_string();
    }
    None
}

/// Validate a declared command without executing the PLC or changing any instance: `block`
/// implements the interface, the interface declares the method, the method returns BOOL (TRUE
/// when the instance accepts the command) and takes inputs only, and the arguments name its
/// inputs, each fixed value fitting its type.
pub fn check(
    metadata: &RuntimeMetadata,
    block: &str,
    command: &MethodCommand,
) -> Result<(), String> {
    command.validate()?;
    if !implements(metadata, block, &command.interface) {
        return Err(format!(
            "{} does not implement {}",
            block, command.interface
        ));
    }
    let method = declared_method(metadata, &command.interface, &command.method)
        .ok_or_else(|| format!("{} has no method {}", command.interface, command.method))?;
    if method.return_type != Some(trust_hir::TypeId::BOOL) {
        return Err(format!(
            "{}.{} must return BOOL, TRUE when the command is accepted",
            command.interface, command.method
        ));
    }
    if method
        .params
        .iter()
        .any(|p| p.direction != ParamDirection::In)
    {
        return Err("HMI methods accept only input parameters".into());
    }
    let mut entered = 0;
    for (name, value) in &command.arguments {
        let parameter = method
            .params
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("{} has no argument {name}", command.method))?;
        match value {
            Argument::Input { .. } => entered += 1,
            Argument::Constant(value) => {
                convert(metadata, parameter.type_id, &scalar(value))
                    .map_err(|e| format!("{name}: {e}"))?;
            }
        }
    }
    if entered > 1 {
        return Err("One visual control supplies one entered value".into());
    }
    for parameter in &method.params {
        if parameter.default.is_none()
            && !command
                .arguments
                .keys()
                .any(|name| parameter.name.eq_ignore_ascii_case(name))
        {
            return Err(format!("Required argument {} is missing", parameter.name));
        }
    }
    Ok(())
}

/// The value a control sends with its method command, checked against the control: a selector's
/// or a mode selector's must be one of its options and becomes the option as listed, which an
/// enumeration input takes by name (`OutOfService` for `E_Mode#OutOfService`); a number must be
/// finite and within `low` and `high`, the control's limits.
pub(crate) fn entered_value(
    object: &HmiObject,
    value: &Json,
    low: Option<f64>,
    high: Option<f64>,
) -> Result<Json, String> {
    if matches!(object.kind.as_str(), "selector" | "mode-selector") {
        let (option, _) = super::plan::choice(object, value)?;
        return Ok(Json::String(option));
    }
    if let Some(number) = value.as_f64() {
        if !number.is_finite() {
            return Err(format!("{value} is not a finite number"));
        }
        if let Some(low) = low.filter(|low| number < *low) {
            return Err(format!("{number} is below the lowest allowed value {low}"));
        }
        if let Some(high) = high.filter(|high| number > *high) {
            return Err(format!(
                "{number} is above the highest allowed value {high}"
            ));
        }
    }
    Ok(value.clone())
}

/// The method's inputs converted to their exact PLC types: the fixed values the command declares
/// and `entered`, the value the control sent, already checked by [`entered_value`]. Call only
/// after [`check`].
pub(crate) fn inputs(
    metadata: &RuntimeMetadata,
    command: &MethodCommand,
    entered: Option<&Json>,
) -> Result<Vec<(SmolStr, Value)>, String> {
    let method = declared_method(metadata, &command.interface, &command.method)
        .ok_or("the command's method is not declared")?;
    let takes_value = command
        .arguments
        .values()
        .any(|value| matches!(value, Argument::Input { input: true }));
    if entered.is_some() && !takes_value {
        return Err("this control sends no value".into());
    }
    let mut result = Vec::new();
    for parameter in &method.params {
        let Some((_, argument)) = command
            .arguments
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(&parameter.name))
        else {
            continue;
        };
        let requested = match argument {
            Argument::Input { .. } => entered.cloned().ok_or("this control needs a value")?,
            Argument::Constant(value) => scalar(value),
        };
        let value = convert(metadata, parameter.type_id, &requested)
            .map_err(|e| format!("{}: {e}", parameter.name))?;
        result.push((parameter.name.clone(), value));
    }
    Ok(result)
}

/// A requested value as a value of a parameter's type.
fn convert(
    metadata: &RuntimeMetadata,
    type_id: trust_hir::TypeId,
    requested: &Json,
) -> Result<Value, String> {
    let default =
        crate::value::default_value_for_type_id(type_id, metadata.registry(), &metadata.profile())
            .map_err(|e| format!("{e:?}"))?;
    crate::hmi::write_value::value_for_target_in(metadata.registry(), &default, requested)
}

fn scalar(value: &HmiScalar) -> Json {
    match value {
        HmiScalar::Bool(flag) => json!(flag),
        HmiScalar::Number(number) => json!(number),
        HmiScalar::Text(text) => json!(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    const SOURCE: &str = r#"
TYPE E_Mode : (Auto, Manual, OutOfService); END_TYPE
INTERFACE IDevice
    METHOD SetMode : BOOL
    VAR_INPUT
        Value : E_Mode;
    END_VAR
    END_METHOD
    METHOD GetMode : E_Mode
    END_METHOD
END_INTERFACE
INTERFACE ISpeed EXTENDS IDevice
    METHOD SetSpeed : BOOL
    VAR_INPUT
        Value : LREAL;
    END_VAR
    END_METHOD
END_INTERFACE
FUNCTION_BLOCK Drive IMPLEMENTS ISpeed
VAR_OUTPUT
    Mode : E_Mode;
    Speed : LREAL;
END_VAR
METHOD PUBLIC SetMode : BOOL
VAR_INPUT
    Value : E_Mode;
END_VAR
    Mode := Value;
    SetMode := TRUE;
END_METHOD
METHOD PUBLIC GetMode : E_Mode
    GetMode := Mode;
END_METHOD
METHOD PUBLIC SetSpeed : BOOL
VAR_INPUT
    Value : LREAL;
END_VAR
    Speed := Value;
    SetSpeed := TRUE;
END_METHOD
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    D1 : Drive;
END_VAR
END_PROGRAM
"#;

    fn metadata() -> RuntimeMetadata {
        crate::harness::TestHarness::from_source(SOURCE)
            .expect("the program compiles")
            .runtime()
            .metadata_snapshot()
    }

    fn command(interface: &str, method: &str, value: Argument) -> MethodCommand {
        MethodCommand {
            interface: interface.into(),
            method: method.into(),
            instance: "Main.D1".into(),
            arguments: BTreeMap::from([("Value".into(), value)]),
        }
    }

    fn mode_selector(options: &[&str]) -> HmiObject {
        crate::hmi::page_files::parse_page(
            "hmi/pages/p.toml",
            &format!(
                "format = 1\ntitle = \"P\"\n[[object]]\nid = \"mode\"\ntype = \"mode-selector\"\noptions = [{}]\nat = [0, 0]\nsize = [200, 48]\n",
                options
                    .iter()
                    .map(|option| format!("\"{option}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
        .expect("the page loads")
        .objects
        .remove(0)
    }

    #[test]
    fn a_plain_bool_method_of_an_implemented_interface_passes() {
        let metadata = metadata();
        let set_mode = command(
            "IDevice",
            "SetMode",
            Argument::Constant(HmiScalar::Text("OutOfService".into())),
        );
        assert_eq!(check(&metadata, "Drive", &set_mode), Ok(()));
        let speed = command("ISpeed", "SetSpeed", Argument::Input { input: true });
        assert_eq!(check(&metadata, "Drive", &speed), Ok(()));
        assert_eq!(
            check(
                &metadata,
                "Drive",
                &command("ISpeed", "SetMode", Argument::Input { input: true })
            ),
            Ok(()),
            "a method of the interface's base"
        );
    }

    #[test]
    fn a_method_that_returns_no_bool_or_is_not_declared_is_refused() {
        let metadata = metadata();
        let getter = MethodCommand {
            arguments: BTreeMap::new(),
            ..command("IDevice", "GetMode", Argument::Input { input: true })
        };
        assert!(check(&metadata, "Drive", &getter)
            .is_err_and(|reason| reason.contains("must return BOOL")));
        assert!(check(
            &metadata,
            "Drive",
            &command("IDevice", "Start", Argument::Input { input: true })
        )
        .is_err_and(|reason| reason.contains("has no method")));
        assert!(check(
            &metadata,
            "Main",
            &command("IDevice", "SetMode", Argument::Input { input: true })
        )
        .is_err_and(|reason| reason.contains("does not implement")));
        let wrong = command(
            "IDevice",
            "SetMode",
            Argument::Constant(HmiScalar::Text("Sideways".into())),
        );
        assert!(
            check(&metadata, "Drive", &wrong).is_err(),
            "not a value of E_Mode"
        );
    }

    #[test]
    fn an_enumeration_input_takes_the_value_of_that_name() {
        let metadata = metadata();
        let set_mode = command("IDevice", "SetMode", Argument::Input { input: true });
        let inputs = inputs(&metadata, &set_mode, Some(&json!("OutOfService"))).expect("inputs");
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].0.as_str(), "Value");
        let Value::Enum(mode) = &inputs[0].1 else {
            panic!("an enumeration value: {:?}", inputs[0].1);
        };
        assert_eq!(mode.variant_name().as_str(), "OutOfService");
        assert!(inputs_of_text(&metadata, &set_mode, "Sideways").is_err());
    }

    fn inputs_of_text(
        metadata: &RuntimeMetadata,
        command: &MethodCommand,
        text: &str,
    ) -> Result<Vec<(SmolStr, Value)>, String> {
        inputs(metadata, command, Some(&json!(text)))
    }

    #[test]
    fn a_mode_selector_sends_only_its_options_and_a_number_stays_within_the_limits() {
        let selector = mode_selector(&["Auto", "Manual"]);
        assert_eq!(
            entered_value(&selector, &json!("manual"), None, None),
            Ok(json!("Manual")),
            "the option as the control lists it"
        );
        assert!(entered_value(&selector, &json!("OutOfService"), None, None).is_err());
        let mut setpoint = selector.clone();
        setpoint.kind = "setpoint".into();
        assert_eq!(
            entered_value(&setpoint, &json!(40.0), Some(0.0), Some(80.0)),
            Ok(json!(40.0))
        );
        assert!(
            entered_value(&setpoint, &json!(90.0), Some(0.0), Some(80.0))
                .is_err_and(|reason| reason.contains("above"))
        );
        assert!(entered_value(&setpoint, &json!(-1.0), Some(0.0), None)
            .is_err_and(|reason| reason.contains("below")));
    }
}
