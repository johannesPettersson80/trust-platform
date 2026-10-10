//! The automatic choice of an experiment (`automaticExperimentSelection` and `automaticDefaultAmplitude` in
//! `experiment.ts`; specification 35, B3.4). The automatic choice resolves to the least intrusive excitation the
//! live evidence allows: the setpoint in Auto, the manual output in Manual or in an open loop, else a writeable
//! disturbance.

use crate::loop_definition::{AuxiliarySignal, LoopDefinition, LoopSignal, Range, Topology};

use super::plan::{
    is_writeable_disturbance, is_zero, ControlExperimentKind, ControllerMode, ExperimentSample,
    PlanInput,
};

const AUTOMATIC_NEEDS_PATH: &str = "Automatic Test selection needs an Auto setpoint path, a Manual output path, or a declared writeable disturbance.";
const AUTOMATIC_NEEDS_SIGNAL: &str =
    "Automatic Test selection needs a current declared signal to excite.";
const AUTOMATIC_NEEDS_RANGE: &str =
    "Automatic Test selection needs a positive finite amplitude and declared signal range.";
const AUTOMATIC_NO_HEADROOM: &str =
    "Automatic Test selection has no safe excitation headroom at the current operating point.";

/// The amplitude an automatic request starts with: a tenth of the declared span of the setpoint, or of the output limits when
/// the loop has no setpoint. A missing output limit has no span, so the amplitude is zero, which the automatic selection
/// refuses for its range (B3.9). Live headroom is checked by the review.
pub(super) fn automatic_default_amplitude(loop_definition: &LoopDefinition) -> f64 {
    let span = match loop_definition.signals.setpoint.as_ref() {
        Some(setpoint) => (setpoint.range.maximum - setpoint.range.minimum).abs(),
        None => match loop_definition
            .limits
            .output_minimum
            .zip(loop_definition.limits.output_maximum)
        {
            Some((minimum, maximum)) => (maximum - minimum).abs(),
            None => 0.0,
        },
    };
    span * 0.1
}

/// The request an automatic Test resolves to, and the amplitude it uses (`automaticExperimentSelection`). The
/// amplitude is halved toward the side with more headroom, so the excitation stays inside the signal's range.
pub(super) fn automatic_selection(
    input: &PlanInput<'_>,
    sample: &ExperimentSample,
) -> Result<(ControlExperimentKind, f64), String> {
    let loop_definition = input.loop_definition;
    let kind = if sample.controller_mode == ControllerMode::Auto
        && sample.setpoint.is_some()
        && loop_definition.signals.setpoint.is_some()
    {
        ControlExperimentKind::SetpointStep
    } else if sample.controller_mode == ControllerMode::Manual
        || loop_definition.topology == Some(Topology::OpenLoop)
    {
        ControlExperimentKind::MvStep
    } else if loop_definition
        .signals
        .additional
        .iter()
        .flatten()
        .any(is_writeable_disturbance)
    {
        ControlExperimentKind::LoadStep
    } else {
        return Err(AUTOMATIC_NEEDS_PATH.to_string());
    };
    let (signal, target) = match kind {
        ControlExperimentKind::SetpointStep => (
            sample.setpoint.as_ref(),
            loop_definition
                .signals
                .setpoint
                .as_ref()
                .map(Target::of_signal),
        ),
        ControlExperimentKind::LoadStep => {
            let binding = loop_definition
                .signals
                .additional
                .iter()
                .flatten()
                .find(|signal| is_writeable_disturbance(signal));
            let measured = sample.additional.iter().find(|candidate| {
                loop_definition
                    .signals
                    .additional
                    .iter()
                    .flatten()
                    .any(|binding| binding.id == candidate.id && is_writeable_disturbance(binding))
            });
            (
                measured.map(|measured| &measured.signal),
                binding.map(Target::of_auxiliary),
            )
        }
        _ => (
            Some(&sample.manipulated_variable),
            Some(Target::of_signal(
                &loop_definition.signals.manipulated_variable,
            )),
        ),
    };
    let (Some(signal), Some(target)) = (signal, target) else {
        return Err(AUTOMATIC_NEEDS_SIGNAL.to_string());
    };
    if !signal.value.is_finite() {
        return Err(AUTOMATIC_NEEDS_SIGNAL.to_string());
    }
    let range = if kind == ControlExperimentKind::MvStep {
        loop_definition
            .limits
            .output_minimum
            .zip(loop_definition.limits.output_maximum)
    } else {
        Some((target.range.minimum, target.range.maximum))
    };
    let requested = input.amplitude.abs();
    let Some((minimum, maximum)) = range else {
        return Err(AUTOMATIC_NEEDS_RANGE.to_string());
    };
    if !requested.is_finite() || requested <= 0.0 || maximum <= minimum {
        return Err(AUTOMATIC_NEEDS_RANGE.to_string());
    }
    let increase = maximum - signal.value;
    let decrease = signal.value - minimum;
    let signed = if increase >= decrease {
        requested.min(increase * 0.5)
    } else {
        -requested.min(decrease * 0.5)
    };
    if !signed.is_finite() || is_zero(signed) {
        return Err(AUTOMATIC_NO_HEADROOM.to_string());
    }
    Ok((kind, signed))
}

/// The range and the IEC type of a signal a plan reads or writes.
#[derive(Debug, Clone, Copy)]
pub(super) struct Target<'a> {
    pub(super) range: Range,
    pub(super) iec_type: &'a str,
}

impl<'a> Target<'a> {
    /// The target of a loop signal.
    pub(super) fn of_signal(signal: &'a LoopSignal) -> Self {
        Self {
            range: signal.range,
            iec_type: signal.iec_type.as_str(),
        }
    }

    /// The target of a further signal of the loop.
    pub(super) fn of_auxiliary(signal: &'a AuxiliarySignal) -> Self {
        Self {
            range: signal.range,
            iec_type: signal.iec_type.as_str(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::automatic_default_amplitude;
    use crate::recording::fixtures::loop_definition;

    #[test]
    fn the_default_amplitude_without_a_setpoint_is_a_tenth_of_the_output_limits_span() {
        let mut open = loop_definition();
        open.signals.setpoint = None;
        // A tenth of the output span 100 - 0 of the fixture is 10.0.
        assert!((automatic_default_amplitude(&open) - 10.0).abs() <= 1e-12);
        open.limits.output_maximum = None;
        assert!(
            automatic_default_amplitude(&open).abs() <= 1e-12,
            "a missing output limit leaves no span, so no amplitude (B3.9)"
        );
    }
}
