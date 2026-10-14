//! The binding of a loop's parameter record as the runtime protocol names it (annex E7.1), built from the loop definition: the
//! binding that every `control.parameters.*` request of Apply and Restore carries (specification 35, B6.1 and B6.4). From
//! `connectRuntimeParameters` (runtimeParameterConnection.ts), which hands the loop's declared `parameterBinding` to the
//! parameter client, and the client's check of it (`decodeParameterBinding`, parameterBinding.ts, which
//! `parameter_application::check_binding` holds).

use super::application_texts;
use super::parameter_application::{check_binding, RuntimeBinding, RuntimeMode, RuntimeParameter};
use crate::failure::Failure;
use crate::loop_definition::LoopDefinition;

/// The binding of the loop's parameter record (E7.1): the loop's declared parameter binding, which the loop file writes in the
/// protocol's form, member for member: the loop, the task that owns the record, each parameter with its symbol, IEC type, unit,
/// convention and bounds, and the mode symbol with its Manual value. It is checked as the parameter client checks it before any
/// runtime call (E7.1, E11.8), against the loop's own identifier.
///
/// # Errors
/// The loop declares no parameter binding (its setup is the next action), or the binding breaks a rule of E7.1
/// (`check_binding`'s refusal).
pub fn runtime_binding_of(loop_definition: &LoopDefinition) -> Result<RuntimeBinding, Failure> {
    let declared = loop_definition
        .parameter_binding
        .as_ref()
        .ok_or_else(application_texts::parameter_binding_missing)?;
    let binding = RuntimeBinding {
        loop_id: declared.loop_id.clone(),
        task_name: declared.task_name.clone(),
        parameters: declared
            .parameters
            .iter()
            .map(|parameter| RuntimeParameter {
                symbol: parameter.symbol.clone(),
                iec_type: parameter.iec_type,
                unit: parameter.unit.clone(),
                convention: parameter.convention.clone(),
                minimum: parameter.minimum,
                maximum: parameter.maximum,
            })
            .collect(),
        mode: RuntimeMode {
            symbol: declared.mode.symbol.clone(),
            manual_value: declared.mode.manual_value,
        },
    };
    check_binding(&binding, &loop_definition.id)?;
    Ok(binding)
}

#[cfg(test)]
#[path = "parameter_binding_tests.rs"]
mod tests;
