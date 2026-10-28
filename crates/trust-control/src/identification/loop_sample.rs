//! The live sample of a loop, read from one runtime inspection (specification 35, A3.1, A5, A6.4 and B3.4 step 1; runtime
//! protocol annex E3; `symbolicLoopObservation.ts` and `observeReadings` of `loopMonitor.ts`): the conversion the production
//! sampler of an identification makes of an available inspection into the sample the run is reviewed against.
//!
//! The inspection reads the bindings of [`crate::signals::loop_inspection_bindings`] in one coherent scan, so every value of
//! the sample comes from that scan (E3, rule 2). A signal is read when its binding is in the inspection with a finite number
//! whose quality is good or forced; a forced value keeps its force, so the review refuses it by name (B3.9). The sample is
//! not available when the setpoint the loop declares, the measurement, the manipulated variable or a further signal of the
//! loop cannot be read. The loop's mode comes from its mode signal's usable value. Whether successive scans still advance
//! is judged by whoever reads them over time (A3.4), not here.

use crate::experiment::{ControllerMode, ExperimentSample, PlanAuxiliarySignal, PlanSignal};
use crate::loop_definition::{LoopDefinition, ModeWhenTrue};
use crate::recording::channels::{
    MANIPULATED_VARIABLE_BINDING, PROCESS_VARIABLE_BINDING, SETPOINT_BINDING,
};
use crate::recording::EvidenceSource;
use crate::signals::{loop_inspection_mode_id, InspectionSample, Quality};

/// The sample of `loop_definition` that the available inspection `sample` gives, on a target of `source`: the scan, the
/// loop's mode, the setpoint when the loop declares one, the measurement, the manipulated variable and the loop's further
/// signals, each with its symbol, address, unit, value and quality. `None` when one of these signals cannot be read.
#[must_use]
pub fn experiment_sample(
    loop_definition: &LoopDefinition,
    source: EvidenceSource,
    sample: &InspectionSample,
) -> Option<ExperimentSample> {
    let signals = &loop_definition.signals;
    let setpoint = match &signals.setpoint {
        Some(setpoint) => Some(reading(
            sample,
            SETPOINT_BINDING,
            &setpoint.symbol,
            setpoint.address.as_deref(),
            &setpoint.unit,
        )?),
        None => None,
    };
    let measurement = &signals.process_variable;
    let process_variable = reading(
        sample,
        PROCESS_VARIABLE_BINDING,
        &measurement.symbol,
        measurement.address.as_deref(),
        &measurement.unit,
    )?;
    let output = &signals.manipulated_variable;
    let manipulated_variable = reading(
        sample,
        MANIPULATED_VARIABLE_BINDING,
        &output.symbol,
        output.address.as_deref(),
        &output.unit,
    )?;
    let additional = signals
        .additional
        .iter()
        .flatten()
        .map(|further| {
            let signal = reading(
                sample,
                &further.id,
                &further.symbol,
                further.address.as_deref(),
                &further.unit,
            )?;
            Some(PlanAuxiliarySignal {
                id: further.id.clone(),
                signal,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(ExperimentSample {
        scan: sample.scan,
        source,
        controller_mode: controller_mode(loop_definition, sample),
        setpoint,
        process_variable,
        manipulated_variable,
        additional,
    })
}

/// The reading of the binding `id` as the review reads it (`read` of `symbolicLoopObservation.ts`): the value when the binding
/// is in the inspection with a finite number whose quality is good or forced, with the symbol, address and unit the loop
/// declares. The value is forced when the runtime reports it forced or its quality is forced.
fn reading(
    sample: &InspectionSample,
    id: &str,
    symbol: &str,
    address: Option<&str>,
    unit: &str,
) -> Option<PlanSignal> {
    let index = sample
        .bindings
        .iter()
        .position(|binding| binding.id == id)?;
    let value = sample
        .values
        .get(index)
        .copied()
        .flatten()
        .filter(|value| value.is_finite())?;
    let quality = match sample.quality.get(index)? {
        Quality::Good if !sample.forced.get(index).copied().unwrap_or(false) => Quality::Good,
        Quality::Good | Quality::Forced => Quality::Forced,
        Quality::Bad | Quality::Missing => return None,
    };
    Some(PlanSignal {
        symbol: symbol.to_owned(),
        address: address.map(str::to_owned),
        value,
        unit: unit.to_owned(),
        quality,
    })
}

/// The loop's mode at the scan (`controller.mode`): read from the binding of its mode signal when that signal's value is
/// usable, and unavailable otherwise or when the loop declares no mode signal.
fn controller_mode(loop_definition: &LoopDefinition, sample: &InspectionSample) -> ControllerMode {
    let mode = loop_definition
        .controller
        .mode_signal
        .as_deref()
        .filter(|symbol| !symbol.is_empty())
        .and_then(|symbol| loop_inspection_mode_id(loop_definition, &sample.bindings, symbol))
        .and_then(|id| sample.controller_mode(loop_definition, id));
    match mode {
        Some(ModeWhenTrue::Auto) => ControllerMode::Auto,
        Some(ModeWhenTrue::Manual) => ControllerMode::Manual,
        None => ControllerMode::Unavailable,
    }
}

#[cfg(test)]
#[path = "loop_sample_tests.rs"]
mod tests;
