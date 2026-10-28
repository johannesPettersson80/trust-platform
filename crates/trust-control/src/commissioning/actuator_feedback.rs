//! The feedback of an actuator, when the applied action differs from the command (specification 35, A5.3 and A6.3;
//! annex E6.2), from `transferActuatorFeedback.ts`. A loop whose actuator is measured tracks the applied action in the
//! actuator's own coordinates, and the output's bounds follow the range of its signal.
//!
//! The `TypeScript` also reads channel-based feedback. Channels are a parked feature (checklist V7), and the Rust loop
//! contract refuses them before any transfer, so only the loop's own actuator is tracked here.

use std::collections::BTreeMap;

use super::texts;
use super::transfer::{TransferBinding, TransferMember, TransferValueType};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;

/// Tracks the measured applied action when the command and the actuator differ (`withActuatorFeedback`). The measured
/// actuator becomes a guard of the binding, the output names it as its feedback (`tracking_outputs`), and the output's
/// bounds become the range of the manipulated variable. A binding without a measured actuator is returned unchanged.
///
/// # Errors
/// The actuator is not a typed `%I` input in the command's unit, or it disagrees in type or unit with a guard that the
/// binding already checks.
pub fn with_actuator_feedback(
    loop_definition: &LoopDefinition,
    binding: TransferBinding,
) -> Result<TransferBinding, Failure> {
    let mut guards = binding.guards.clone();
    let mut tracking_outputs = BTreeMap::new();
    let output = track(
        loop_definition,
        &binding.output,
        &mut guards,
        &mut tracking_outputs,
    )?;
    let mut additional_outputs = Vec::with_capacity(binding.additional_outputs.len());
    for member in &binding.additional_outputs {
        additional_outputs.push(track(
            loop_definition,
            member,
            &mut guards,
            &mut tracking_outputs,
        )?);
    }
    if tracking_outputs.is_empty() {
        return Ok(binding);
    }
    Ok(TransferBinding {
        tracking_outputs,
        guards,
        output,
        additional_outputs,
        ..binding
    })
}

/// The output as the transfer holds it: tracked when the loop measures its actuator, otherwise unchanged. The guard is
/// added once, and a second output with the same actuator reuses it.
fn track(
    loop_definition: &LoopDefinition,
    output: &TransferMember,
    guards: &mut Vec<TransferMember>,
    tracking_outputs: &mut BTreeMap<String, String>,
) -> Result<TransferMember, Failure> {
    let signal = &loop_definition.signals.manipulated_variable;
    let applied = match loop_definition.signals.actuator.as_ref() {
        Some(actuator) if output.symbol == signal.symbol => actuator,
        _ => return Ok(output.clone()),
    };
    let typed_input = applied
        .address
        .as_deref()
        .is_some_and(|address| address.starts_with("%I"));
    let iec_type = TransferValueType::from_iec(&applied.iec_type)
        .filter(|_| typed_input && applied.unit == output.unit)
        .ok_or_else(|| {
            Failure::new(
                &texts::feedback_needs_command_input(),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })?;
    let member = TransferMember {
        symbol: applied.symbol.clone(),
        iec_type,
        unit: applied.unit.clone(),
        minimum: output.minimum,
        maximum: output.maximum,
    };
    let existing = guards
        .iter()
        .find(|guard| guard.symbol == member.symbol)
        .cloned();
    match existing {
        Some(existing) if existing.unit != member.unit || existing.iec_type != member.iec_type => {
            return Err(Failure::new(
                &texts::feedback_disagrees(),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            ));
        }
        Some(_) => {}
        None => guards.push(member.clone()),
    }
    tracking_outputs.insert(output.symbol.clone(), member.symbol);
    Ok(TransferMember {
        minimum: signal.range.minimum,
        maximum: signal.range.maximum,
        ..output.clone()
    })
}

/// The member whose bounds an output's value is checked against: its feedback guard when the output is tracked, the
/// output itself otherwise (`transferOutputBasis`). `None` when the binding names a feedback guard it does not carry; a
/// binding built by [`with_actuator_feedback`] never does.
#[must_use]
pub fn transfer_output_basis<'a>(
    binding: &'a TransferBinding,
    output: &'a TransferMember,
) -> Option<&'a TransferMember> {
    match binding.tracking_outputs.get(&output.symbol) {
        None => Some(output),
        Some(feedback) => binding
            .guards
            .iter()
            .find(|guard| guard.symbol == *feedback),
    }
}

/// The allowance of the target across the outputs, in the primary output's span: each output's span, relative to its
/// basis, scaled to the primary basis's span, and the largest of them (`transferTargetAllowance`, E6.9). An output whose
/// basis has no width contributes zero.
///
/// `None` when a basis cannot be found (see [`transfer_output_basis`]).
#[must_use]
pub fn transfer_target_allowance(binding: &TransferBinding) -> Option<f64> {
    let primary = transfer_output_basis(binding, &binding.output)?;
    let span = primary.maximum - primary.minimum;
    let allowances = std::iter::once(&binding.output)
        .chain(binding.additional_outputs.iter())
        .map(|output| {
            let basis = transfer_output_basis(binding, output)?;
            let width = basis.maximum - basis.minimum;
            Some(if width > 0.0 {
                (output.maximum - output.minimum) / width * span
            } else {
                0.0
            })
        })
        .collect::<Option<Vec<f64>>>()?;
    allowances.into_iter().reduce(f64::max)
}

#[cfg(test)]
#[path = "actuator_feedback_tests.rs"]
mod tests;
