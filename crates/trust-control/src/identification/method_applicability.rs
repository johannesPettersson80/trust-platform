//! Which identification methods a loop can run (`identificationMethodApplicability.ts`). A method is unavailable when the
//! loop definition lacks what the method needs, and the refusal names the missing part. The multichannel refusals are
//! parked with stage V7: a loop with channel bindings is refused by the Rust engine before Identify runs, so a loop
//! here has one plant input and one measured output.

use crate::identification::acquisition_settings::AcquisitionMethod;
use crate::loop_definition::{ExperimentKind, LoopDefinition};

/// Whether an identification method can run for a loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applicability {
    /// The method can run.
    Available,
    /// The method cannot run; the refusal names the missing prerequisite.
    Unavailable(MethodRefusal),
}

/// The prerequisite of an identification method that the loop lacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodRefusal {
    /// A relay needs the process direction the loop declares.
    RelayDirection,
    /// A relay needs the Manual command and the mode binding.
    RelayManualOwnership,
    /// Frequency excitation needs intermediate output levels, which an ON/OFF output does not have.
    FrequencyOnOff,
    /// A setpoint step needs the loop's configured reference signal.
    SetpointReference,
    /// An open-loop method needs the Manual command and the mode binding.
    OpenLoopBindings,
    /// A closed-loop setpoint step needs the controller's mode binding.
    ClosedLoopModeBinding,
}

/// Whether a method can run for a loop (`identificationMethodApplicability`). `Configured` asks what the loop declares:
/// the declared method when it is one of the methods above, and Automatic otherwise.
#[must_use]
pub fn method_applicability(
    loop_definition: &LoopDefinition,
    method: AcquisitionMethod,
) -> Applicability {
    match method {
        AcquisitionMethod::Configured => {
            let declared = loop_definition
                .experiment
                .as_ref()
                .and_then(|experiment| method_of_kind(experiment.kind))
                .unwrap_or(AcquisitionMethod::Automatic);
            method_applicability(loop_definition, declared)
        }
        AcquisitionMethod::Automatic => Applicability::Available,
        _ => single_channel_applicability(loop_definition, method),
    }
}

/// The method that an experiment kind names, when the method selector offers it.
fn method_of_kind(kind: ExperimentKind) -> Option<AcquisitionMethod> {
    match kind {
        ExperimentKind::SetpointStep => Some(AcquisitionMethod::SetpointStep),
        ExperimentKind::MvStep => Some(AcquisitionMethod::MvStep),
        ExperimentKind::MvPulse => Some(AcquisitionMethod::MvPulse),
        ExperimentKind::Relay => Some(AcquisitionMethod::Relay),
        ExperimentKind::Prbs => Some(AcquisitionMethod::Prbs),
        ExperimentKind::Multisine => Some(AcquisitionMethod::Multisine),
        ExperimentKind::Observation | ExperimentKind::LoadStep => None,
    }
}

/// The rules of the methods for a loop with one plant input and one measured output, in the order the `TypeScript` checks
/// them: the relay, the frequency excitation, the setpoint step, the open-loop methods, then the closed-loop mode.
fn single_channel_applicability(
    loop_definition: &LoopDefinition,
    method: AcquisitionMethod,
) -> Applicability {
    let manual_command_missing = loop_definition
        .controller
        .manual_output_signal
        .as_deref()
        .is_none_or(str::is_empty);
    let bindings_missing = manual_command_missing || loop_definition.parameter_binding.is_none();
    if method == AcquisitionMethod::Relay {
        let direction_missing = loop_definition
            .experiment
            .as_ref()
            .and_then(|experiment| experiment.relay_direction)
            .is_none();
        if direction_missing {
            return Applicability::Unavailable(MethodRefusal::RelayDirection);
        }
        if bindings_missing {
            return Applicability::Unavailable(MethodRefusal::RelayManualOwnership);
        }
    }
    let frequency = matches!(
        method,
        AcquisitionMethod::Prbs | AcquisitionMethod::Multisine
    );
    if frequency && loop_definition.controller.on_off.is_some() {
        return Applicability::Unavailable(MethodRefusal::FrequencyOnOff);
    }
    if method == AcquisitionMethod::SetpointStep && loop_definition.signals.setpoint.is_none() {
        return Applicability::Unavailable(MethodRefusal::SetpointReference);
    }
    let open_loop = matches!(
        method,
        AcquisitionMethod::MvStep
            | AcquisitionMethod::MvPulse
            | AcquisitionMethod::Prbs
            | AcquisitionMethod::Multisine
    );
    if open_loop && bindings_missing {
        return Applicability::Unavailable(MethodRefusal::OpenLoopBindings);
    }
    if method == AcquisitionMethod::SetpointStep && loop_definition.parameter_binding.is_none() {
        return Applicability::Unavailable(MethodRefusal::ClosedLoopModeBinding);
    }
    Applicability::Available
}

#[cfg(test)]
#[path = "method_applicability_tests.rs"]
mod tests;
