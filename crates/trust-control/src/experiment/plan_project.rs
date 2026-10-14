//! The plan of a project-owned experiment (`Execution::ProjectScripted`; specification 35, A6.5 and B3; annex E5.2).
//! The project's own sequence sets the excitation, so the plan writes only the project's Boolean authority, with the
//! baseline it records before that authority is enabled. `plan.rs` calls these functions for such a loop.

use crate::experiment::duration::project_duration_ms;
use crate::experiment::number_text::{js_bool, js_number};
use crate::experiment::trajectory::point;
use crate::loop_definition::{Activation, Execution, Experiment, LoopDefinition};
use crate::recording::EvidenceSource;
use crate::signals::Quality;

use super::{
    experiment_mode_binding, experiment_task, plan_source, ControlExperimentKind,
    ControlExperimentPlan, ExperimentSample, ExperimentValue, PlanAbortBinding, PlanInput,
    PlanSignal, GOOD_PRIMARY_REFUSAL, PHYSICAL_TOPOLOGY_REFUSAL,
};

const FORCED_PRIMARY_REFUSAL: &str =
    "Remove forcing from SP, PV, and MV before starting a new Test.";
const BASELINE_REFUSAL: &str =
    "Maximum duration must include the declared baseline and at least one experiment sample.";

/// The project-owned experiment of a loop and its activation, when the loop has both.
pub(super) fn project_experiment(
    loop_definition: &LoopDefinition,
) -> Option<(&Experiment, &Activation)> {
    let experiment = loop_definition.experiment.as_ref()?;
    if experiment.execution != Execution::ProjectScripted {
        return None;
    }
    let activation = experiment.activation.as_ref()?;
    Some((experiment, activation))
}

/// The plan of a project-owned experiment: the project sets its own excitation, and the plan writes only its
/// Boolean authority, with the baseline it records before the authority is enabled.
pub(super) fn project_sequence_plan(
    input: &PlanInput<'_>,
    sample: &ExperimentSample,
    experiment: &Experiment,
    activation: &Activation,
) -> Result<ControlExperimentPlan, String> {
    let loop_definition = input.loop_definition;
    let primary: Vec<&PlanSignal> = sample
        .setpoint
        .iter()
        .chain([&sample.process_variable, &sample.manipulated_variable])
        .collect();
    if primary
        .iter()
        .any(|signal| signal.quality == Quality::Forced)
    {
        return Err(FORCED_PRIMARY_REFUSAL.to_string());
    }
    if primary.iter().any(|signal| signal.quality != Quality::Good) {
        return Err(GOOD_PRIMARY_REFUSAL.to_string());
    }
    if sample.source == EvidenceSource::PhysicalPlant && !input.physical_topology_reviewed {
        return Err(PHYSICAL_TOPOLOGY_REFUSAL.to_string());
    }
    let duration_ms = project_duration_ms(input, experiment)?;
    let nominal = loop_definition.observed_sample_timing.nominal_period_ms;
    let baseline_seconds = experiment.baseline_seconds.unwrap_or(0.0);
    let baseline_ms = (baseline_seconds * 1000.0 / nominal).ceil() * nominal;
    if !baseline_seconds.is_finite()
        || baseline_seconds < 0.0
        || baseline_ms + nominal > duration_ms
    {
        return Err(BASELINE_REFUSAL.to_string());
    }
    // The abort is asserted only by a stop other than completion (E5.5); a loop that decodes always has its value.
    let abort = experiment
        .abort
        .as_ref()
        .and_then(|abort| abort.asserted_value.map(|asserted| (abort, asserted)));
    let stop_conditions = project_stop_conditions(
        duration_ms / 1000.0,
        abort.map(|(abort, asserted)| (abort.symbol.as_str(), asserted)),
    );
    let trajectory = if baseline_ms > 0.0 {
        vec![point(0.0, 0.0), point(baseline_ms, 1.0)]
    } else {
        Vec::new()
    };
    Ok(ControlExperimentPlan {
        id: format!(
            "experiment:{}:{}:project_sequence",
            loop_definition.id, sample.scan
        ),
        source: plan_source(sample.source),
        kind: ControlExperimentKind::ProjectSequence,
        affected_signal: activation.symbol.clone(),
        address: Some(activation.address.clone()),
        value_type: activation.iec_type.clone(),
        from: ExperimentValue::Boolean(false),
        to: ExperimentValue::Boolean(true),
        unit: "authority".to_string(),
        minimum: 0.0,
        maximum: 1.0,
        duration_ms,
        expected_scan: sample.scan,
        task: Some(experiment_task(loop_definition)),
        mode_binding: experiment_mode_binding(loop_definition),
        stop_conditions,
        restoration_value: ExperimentValue::Boolean(false),
        requires_physical_approval: sample.source == EvidenceSource::PhysicalPlant,
        trajectory,
        abort_binding: abort.map(|(abort, asserted)| PlanAbortBinding {
            symbol: abort.symbol.clone(),
            address: abort.address.clone(),
            value_type: abort.iec_type.clone(),
            asserted_value: asserted,
        }),
        relay_feedback: None,
        interval_ms: None,
        seed: None,
        frequencies_hz: None,
    })
}

/// The stop conditions of a project-owned experiment (E5.5, C12). The plan and the Identify review name the same list.
/// `abort` is the abort symbol and the value its assertion writes, when the loop declares an abort.
pub(crate) fn project_stop_conditions(
    duration_seconds: f64,
    abort: Option<(&str, bool)>,
) -> Vec<String> {
    let mut stop_conditions = vec![
        "Abort".to_string(),
        format!("{} s timeout", js_number(duration_seconds)),
        "Loop mode changed".to_string(),
        "Bad runtime data".to_string(),
        "Signal force".to_string(),
        "Signal outside its limits".to_string(),
        "Runtime fault".to_string(),
    ];
    if let Some((symbol, asserted)) = abort {
        stop_conditions.push(format!(
            "Abort or failure asserts {symbol} = {}; normal completion does not.",
            js_bool(asserted)
        ));
    }
    stop_conditions
}
