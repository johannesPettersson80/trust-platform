//! The plan of an identification experiment (`experiment.ts`, `experimentRecording.ts`, `projectExperimentHold.ts`):
//! its kinds and defaults, and the review that turns the loop's current evidence into a plan the runtime can run
//! (specification 35, A6.5 and B3; annex E5.2 and E5.3).
//!
//! A plan is the complete trajectory of one experiment, with its mode, its restoration and its stop conditions.
//! The review checks each rule before a plan is presented, and the first failed check is the refusal. The runtime
//! checks them again when the experiment starts (annex E5.3).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::identification::campaign::plant_input_range;
use crate::loop_definition::{
    AuxiliaryRole, AuxiliarySignal, Direction, Execution, ExperimentKind, LoopDefinition,
    ManualOutputType, Topology,
};
use crate::recording::EvidenceSource;
use crate::signals::Quality;

use super::automatic::{automatic_default_amplitude, automatic_selection, Target};
use super::duration::duration_within_limits;
use super::number_text::js_number;
use super::trajectory::{automatic_trajectory, experiment_trajectory};

#[path = "plan_project.rs"]
mod project;

use project::{project_experiment, project_sequence_plan};

const DURATION_REFUSAL: &str = "Duration must be between 1 s and 24 h.";
const AMPLITUDE_REFUSAL: &str =
    "Amplitude must be non-zero and duration must be between 1 s and 24 h.";
const STALE_REFUSAL: &str = "Fresh synchronized SP, PV, and MV are required.";
const GOOD_PRIMARY_REFUSAL: &str = "Fresh good-quality SP, PV, and MV are required.";
const PHYSICAL_TOPOLOGY_REFUSAL: &str =
    "A reviewed physical topology is required before approval can be requested.";
const MANUAL_MODE_REFUSAL: &str = "Output steps require confirmed Manual controller mode.";
const MANUAL_COMMAND_REFUSAL: &str =
    "Output Test requires the declared Manual command of REAL or LREAL type.";
const AUTO_MODE_REFUSAL: &str = "Setpoint steps require confirmed Auto controller mode.";
const SETPOINT_REFUSAL: &str = "A setpoint step requires a declared closed-loop setpoint signal.";
const DISTURBANCE_REFUSAL: &str = "A current declared writeable disturbance signal is required.";
const FORCED_SIGNAL_REFUSAL: &str = "The affected signal is forced.";
const TOO_SHORT_REFUSAL: &str =
    "Automatic Test duration is too short to capture baseline, held step and restoration.";
const OUT_OF_RANGE_REFUSAL: &str = "The generated excitation exceeds the declared signal range.";

/// The kind of an experiment the runtime runs (annex E5.2). `output_step` is a legacy alias of `mv_step` that old
/// plans still carry; `project_sequence` is the Boolean authority of a project-owned experiment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ControlExperimentKind {
    /// Observation only: nothing is written.
    Observation,
    /// A step of the setpoint of a closed loop, in Auto.
    SetpointStep,
    /// A step of a writeable disturbance signal.
    LoadStep,
    /// A step of the manual output, in Manual.
    MvStep,
    /// A rise and return of the manual output.
    MvPulse,
    /// A relay on the measured process value.
    Relay,
    /// A pseudo-random binary sequence of the manual output.
    Prbs,
    /// A multisine of the manual output.
    Multisine,
    /// A legacy alias of `mv_step`.
    OutputStep,
    /// The Boolean authority of a project-owned experiment.
    ProjectSequence,
}

impl ControlExperimentKind {
    /// The name the runtime and the study use for the kind.
    #[must_use]
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::Observation => "observation",
            Self::SetpointStep => "setpoint_step",
            Self::LoadStep => "load_step",
            Self::MvStep => "mv_step",
            Self::MvPulse => "mv_pulse",
            Self::Relay => "relay",
            Self::Prbs => "prbs",
            Self::Multisine => "multisine",
            Self::OutputStep => "output_step",
            Self::ProjectSequence => "project_sequence",
        }
    }

    /// Whether the kind writes the manual output, which the review checks against Manual mode.
    #[must_use]
    fn excites_output(self) -> bool {
        matches!(
            self,
            Self::OutputStep
                | Self::MvStep
                | Self::MvPulse
                | Self::Relay
                | Self::Prbs
                | Self::Multisine
        )
    }
}

/// What a request asks for: an experiment kind, or the automatic choice, which the review resolves to a reviewed
/// kind (`ControlExperimentRequestKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ControlExperimentRequestKind {
    /// The least intrusive bounded excitation the current evidence allows.
    Automatic,
    /// Observation only.
    Observation,
    /// A step of the setpoint.
    SetpointStep,
    /// A step of a writeable disturbance.
    LoadStep,
    /// A step of the manual output.
    MvStep,
    /// A rise and return of the manual output.
    MvPulse,
    /// A relay on the measured process value.
    Relay,
    /// A pseudo-random binary sequence.
    Prbs,
    /// A multisine.
    Multisine,
    /// A legacy alias of `mv_step`.
    OutputStep,
    /// The Boolean authority of a project-owned experiment.
    ProjectSequence,
}

impl ControlExperimentRequestKind {
    /// The kind a request names, or `None` for the automatic choice.
    #[must_use]
    pub fn kind(self) -> Option<ControlExperimentKind> {
        Some(match self {
            Self::Automatic => return None,
            Self::Observation => ControlExperimentKind::Observation,
            Self::SetpointStep => ControlExperimentKind::SetpointStep,
            Self::LoadStep => ControlExperimentKind::LoadStep,
            Self::MvStep => ControlExperimentKind::MvStep,
            Self::MvPulse => ControlExperimentKind::MvPulse,
            Self::Relay => ControlExperimentKind::Relay,
            Self::Prbs => ControlExperimentKind::Prbs,
            Self::Multisine => ControlExperimentKind::Multisine,
            Self::OutputStep => ControlExperimentKind::OutputStep,
            Self::ProjectSequence => ControlExperimentKind::ProjectSequence,
        })
    }
}

impl From<ControlExperimentKind> for ControlExperimentRequestKind {
    fn from(kind: ControlExperimentKind) -> Self {
        match kind {
            ControlExperimentKind::Observation => Self::Observation,
            ControlExperimentKind::SetpointStep => Self::SetpointStep,
            ControlExperimentKind::LoadStep => Self::LoadStep,
            ControlExperimentKind::MvStep => Self::MvStep,
            ControlExperimentKind::MvPulse => Self::MvPulse,
            ControlExperimentKind::Relay => Self::Relay,
            ControlExperimentKind::Prbs => Self::Prbs,
            ControlExperimentKind::Multisine => Self::Multisine,
            ControlExperimentKind::OutputStep => Self::OutputStep,
            ControlExperimentKind::ProjectSequence => Self::ProjectSequence,
        }
    }
}

/// One choice the engineer is offered for an experiment (`ControlExperimentChoice`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ControlExperimentChoice {
    /// The request the choice makes.
    pub value: ControlExperimentRequestKind,
    /// The words the choice shows.
    pub label: &'static str,
}

/// The experiment choices offered to the engineer, in order. The legacy alias and the project authority are not
/// offered (`controlExperimentChoices`).
#[must_use]
pub fn control_experiment_choices() -> Vec<ControlExperimentChoice> {
    let choice = |value, label| ControlExperimentChoice { value, label };
    vec![
        choice(
            ControlExperimentRequestKind::Automatic,
            "Automatic bounded excitation",
        ),
        choice(
            ControlExperimentRequestKind::Observation,
            "Observe without excitation",
        ),
        choice(ControlExperimentRequestKind::SetpointStep, "Setpoint step"),
        choice(
            ControlExperimentRequestKind::LoadStep,
            "Load disturbance step",
        ),
        choice(ControlExperimentRequestKind::MvStep, "Manual output step"),
        choice(ControlExperimentRequestKind::MvPulse, "Manual output pulse"),
        choice(ControlExperimentRequestKind::Relay, "Relay feedback"),
        choice(ControlExperimentRequestKind::Prbs, "PRBS"),
        choice(ControlExperimentRequestKind::Multisine, "Multisine"),
    ]
}

/// The starting values of a Test form (`ControlExperimentDefaults`): what the loop's configured experiment says,
/// or the automatic default when none is configured.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ControlExperimentDefaults {
    /// Who runs the experiment.
    pub execution: Execution,
    /// The request the form starts with.
    pub kind: ControlExperimentRequestKind,
    /// The amplitude the form starts with.
    pub amplitude: f64,
    /// The duration the form starts with, in s.
    pub duration_seconds: f64,
    /// The seed of a pseudo-random excitation.
    pub seed: u64,
}

/// The starting values of the Test form for `loop_definition` (`controlExperimentDefaults`). A configured
/// observation with no amplitude is an automatic request, with a window of at least 10 s. A configured experiment that
/// declares no duration starts at 30 s, as the form does with no experiment at all.
#[must_use]
pub fn control_experiment_defaults(loop_definition: &LoopDefinition) -> ControlExperimentDefaults {
    match loop_definition.experiment.as_ref() {
        Some(configured)
            if configured.execution != Execution::ProjectScripted
                && configured.kind == ExperimentKind::Observation
                && is_zero(configured.amplitude) =>
        {
            ControlExperimentDefaults {
                execution: configured.execution,
                kind: ControlExperimentRequestKind::Automatic,
                amplitude: automatic_default_amplitude(loop_definition),
                duration_seconds: configured.duration_seconds.unwrap_or(30.0).max(10.0),
                seed: configured.seed,
            }
        }
        Some(configured) => ControlExperimentDefaults {
            execution: configured.execution,
            kind: request_of_experiment(configured.kind),
            amplitude: configured.amplitude,
            duration_seconds: configured.duration_seconds.unwrap_or(30.0),
            seed: configured.seed,
        },
        None => ControlExperimentDefaults {
            execution: Execution::ControlStudio,
            kind: ControlExperimentRequestKind::Automatic,
            amplitude: automatic_default_amplitude(loop_definition),
            duration_seconds: 30.0,
            seed: 1,
        },
    }
}

fn request_of_experiment(kind: ExperimentKind) -> ControlExperimentRequestKind {
    match kind {
        ExperimentKind::Observation => ControlExperimentRequestKind::Observation,
        ExperimentKind::SetpointStep => ControlExperimentRequestKind::SetpointStep,
        ExperimentKind::LoadStep => ControlExperimentRequestKind::LoadStep,
        ExperimentKind::MvStep => ControlExperimentRequestKind::MvStep,
        ExperimentKind::MvPulse => ControlExperimentRequestKind::MvPulse,
        ExperimentKind::Relay => ControlExperimentRequestKind::Relay,
        ExperimentKind::Prbs => ControlExperimentRequestKind::Prbs,
        ExperimentKind::Multisine => ControlExperimentRequestKind::Multisine,
    }
}

/// One point of a trajectory: the value written at `at_ms` after the first execution (`ControlExperimentPoint`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ControlExperimentPoint {
    /// The process time from the first execution, in ms.
    pub at_ms: f64,
    /// The value written at that time.
    pub value: f64,
}

/// A number, or a Boolean for the project's authority (`number | boolean` in the plan).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ExperimentValue {
    /// A number.
    Number(f64),
    /// A Boolean.
    Boolean(bool),
}

/// The relay of a relay experiment: the measurement it switches on and its thresholds (`ControlRelayFeedbackPlan`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ControlRelayFeedbackPlan {
    /// The measurement's symbol.
    pub process_variable: String,
    /// The measurement's address, which the relay reads.
    pub address: String,
    /// The measurement's IEC type.
    pub value_type: String,
    /// The measurement's unit.
    pub unit: String,
    /// The centre of the switching band.
    pub center: f64,
    /// The half-width of the switching band.
    pub hysteresis: f64,
    /// The level below which the relay switches high.
    pub lower_threshold: f64,
    /// The level above which the relay switches low.
    pub upper_threshold: f64,
    /// The output while the relay is low.
    pub low_output: f64,
    /// The output while the relay is high.
    pub high_output: f64,
}

/// The mode of a plan: the mode signal and the value that means Manual (`modeBinding`, A6.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentModeBinding {
    /// The mode signal's symbol.
    pub symbol: String,
    /// The value that means Manual.
    pub manual_value: bool,
}

/// The abort of a project-owned experiment: the latched variable that asserts an abort (`abortBinding`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanAbortBinding {
    /// The abort variable's symbol.
    pub symbol: String,
    /// The abort variable's address.
    pub address: String,
    /// The abort variable's IEC type, `BOOL`.
    pub value_type: String,
    /// The value that asserts an abort.
    pub asserted_value: bool,
}

/// Whether a plan runs on a simulated or a physical target (`source` of the plan).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PlanSource {
    /// A simulated target.
    Simulation,
    /// A physical target.
    Physical,
}

/// The mode of the loop as the sample reads it (`controller.mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ControllerMode {
    /// Auto.
    Auto,
    /// Manual.
    Manual,
    /// The mode cannot be read.
    Unavailable,
}

/// The complete plan of one experiment (`ControlExperimentPlan`): what it writes, when, and how it ends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ControlExperimentPlan {
    /// The experiment's identifier, `experiment:<loop>:<scan>:<kind>`.
    pub id: String,
    /// Whether the plan runs on a simulated or a physical target.
    pub source: PlanSource,
    /// The experiment's kind.
    pub kind: ControlExperimentKind,
    /// The signal the experiment writes, by symbol.
    pub affected_signal: String,
    /// The signal's address, when the experiment writes it through its address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// The IEC type of the written value.
    pub value_type: String,
    /// The value the experiment starts from.
    pub from: ExperimentValue,
    /// The value the experiment moves to.
    pub to: ExperimentValue,
    /// The written value's unit.
    pub unit: String,
    /// The smallest value the experiment may write.
    pub minimum: f64,
    /// The largest value the experiment may write.
    pub maximum: f64,
    /// The duration of the experiment, in ms of process time.
    pub duration_ms: f64,
    /// The sequence value of the first execution the experiment may use.
    pub expected_scan: u64,
    /// The task that runs the experiment, or `$background`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The mode signal and its Manual value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_binding: Option<ExperimentModeBinding>,
    /// The conditions that stop the experiment early, as the engineer reads them.
    pub stop_conditions: Vec<String>,
    /// The value restored at the end.
    pub restoration_value: ExperimentValue,
    /// Whether the plan needs the physical approval of E10.2.
    pub requires_physical_approval: bool,
    /// The points the experiment writes, in order.
    pub trajectory: Vec<ControlExperimentPoint>,
    /// The abort of a project-owned experiment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abort_binding: Option<PlanAbortBinding>,
    /// The relay of a relay experiment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_feedback: Option<ControlRelayFeedbackPlan>,
    /// The interval between the writes of a pseudo-random sequence, in ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_ms: Option<f64>,
    /// The seed of a pseudo-random sequence, 1 to 2³² − 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,
    /// The frequencies of a multisine, in Hz.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequencies_hz: Option<Vec<f64>>,
}

/// A signal of the live sample: its value and quality at one scan, and its address and unit (the
/// `ControlMonitorSignal` the review reads).
#[derive(Debug, Clone, PartialEq)]
pub struct PlanSignal {
    /// The signal's symbol.
    pub symbol: String,
    /// The signal's address, when it has one.
    pub address: Option<String>,
    /// The value at the scan.
    pub value: f64,
    /// The signal's unit.
    pub unit: String,
    /// The signal's quality at the scan.
    pub quality: Quality,
}

/// A further signal of the live sample, with its identifier in the loop (`ControlMonitorAuxiliarySignal`).
#[derive(Debug, Clone, PartialEq)]
pub struct PlanAuxiliarySignal {
    /// The signal's identifier in the loop.
    pub id: String,
    /// The signal's value, address, unit and quality.
    pub signal: PlanSignal,
}

/// The live sample a Test is reviewed against: one coherent scan of the loop's signals (`AvailableLoopSample`).
/// A sample that is not available is passed to the review as `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct ExperimentSample {
    /// The scan the values come from.
    pub scan: u64,
    /// Whether the runtime is simulated or physical.
    pub source: EvidenceSource,
    /// The loop's mode at the scan.
    pub controller_mode: ControllerMode,
    /// The setpoint, when the loop has one.
    pub setpoint: Option<PlanSignal>,
    /// The measurement.
    pub process_variable: PlanSignal,
    /// The manipulated variable.
    pub manipulated_variable: PlanSignal,
    /// The further signals of the loop at the scan.
    pub additional: Vec<PlanAuxiliarySignal>,
}

/// What the review is given (`PlanInput`).
#[derive(Debug, Clone, Copy)]
pub struct PlanInput<'a> {
    /// The loop the experiment belongs to.
    pub loop_definition: &'a LoopDefinition,
    /// The live sample, or `None` when no fresh synchronized sample is available.
    pub sample: Option<&'a ExperimentSample>,
    /// The requested kind, or the automatic choice.
    pub kind: ControlExperimentRequestKind,
    /// The requested amplitude; the automatic choice may reduce it.
    pub amplitude: f64,
    /// The requested duration, in s.
    pub duration_seconds: f64,
    /// Whether the engineer has reviewed the physical topology.
    pub physical_topology_reviewed: bool,
    /// The seed of a pseudo-random excitation; 1 when absent.
    pub seed: Option<u32>,
    /// The mean of the measurement over the steady baseline; it centres a relay's switching band.
    pub process_center: Option<f64>,
    /// The half-range of the measurement over the steady baseline; it sizes a relay's switching band.
    pub process_noise: Option<f64>,
    /// The applied value of the loop's declared hold parameter, in s, for a project step test.
    pub hold_seconds: Option<f64>,
}

/// The review's answer (`ControlExperimentPlanResult`): an accepted plan, or the reason it is refused.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ControlExperimentPlanResult {
    /// The plan may be presented for approval.
    Accepted {
        /// The reviewed plan.
        plan: Box<ControlExperimentPlan>,
    },
    /// The request is refused; the reason names the first failed check.
    Rejected {
        /// The sentence the engineer reads.
        reason: String,
    },
}

/// Reviews an experiment request against the loop and its current evidence (`buildControlExperimentPlan`). The
/// first failed check is the refusal, and its text is the reason.
#[must_use]
pub fn build_control_experiment_plan(input: &PlanInput<'_>) -> ControlExperimentPlanResult {
    match plan_or_refusal(input) {
        Ok(plan) => ControlExperimentPlanResult::Accepted {
            plan: Box::new(plan),
        },
        Err(reason) => ControlExperimentPlanResult::Rejected { reason },
    }
}

fn plan_or_refusal(input: &PlanInput<'_>) -> Result<ControlExperimentPlan, String> {
    if !duration_within_limits(input.duration_seconds) {
        return Err(DURATION_REFUSAL.to_string());
    }
    let sample = input.sample.ok_or_else(|| STALE_REFUSAL.to_string())?;
    match project_experiment(input.loop_definition) {
        Some((experiment, activation)) => {
            project_sequence_plan(input, sample, experiment, activation)
        }
        None => direct_plan(input, sample),
    }
}

/// The plan of a direct experiment: an excitation of the setpoint, the output or a disturbance, or an observation
/// (`buildControlExperimentPlan`, after the project branch).
fn direct_plan(
    input: &PlanInput<'_>,
    sample: &ExperimentSample,
) -> Result<ControlExperimentPlan, String> {
    let loop_definition = input.loop_definition;
    let (kind, amplitude) = match input.kind.kind() {
        Some(kind) => (kind, input.amplitude),
        None => automatic_selection(input, sample)?,
    };
    if !amplitude.is_finite()
        || (kind != ControlExperimentKind::Observation && is_zero(amplitude))
        || !duration_within_limits(input.duration_seconds)
    {
        return Err(AMPLITUDE_REFUSAL.to_string());
    }
    let manual_command = check_mode_and_authority(input, sample, kind)?;
    let disturbance = writeable_disturbance(loop_definition, sample);
    if kind == ControlExperimentKind::LoadStep && disturbance.is_none() {
        return Err(DISTURBANCE_REFUSAL.to_string());
    }
    let (signal, target) = affected_signal(kind, sample, loop_definition, disturbance)
        .ok_or_else(|| missing_signal_refusal(kind))?;
    if signal.quality == Quality::Forced {
        return Err(FORCED_SIGNAL_REFUSAL.to_string());
    }
    let primary = [
        sample.setpoint.as_ref(),
        Some(&sample.process_variable),
        Some(&sample.manipulated_variable),
    ];
    if primary
        .into_iter()
        .flatten()
        .any(|candidate| candidate.quality != Quality::Good && candidate.quality != Quality::Forced)
    {
        return Err(GOOD_PRIMARY_REFUSAL.to_string());
    }
    let excursion = checked_excursion(kind, signal, &target, loop_definition, amplitude)?;
    let duration_ms = (input.duration_seconds * 1000.0).round();
    let excitation = plan_excitation(input, sample, kind, &excursion, duration_ms)?;
    let address = match &manual_command {
        Some(command) => command.address.clone(),
        None => signal.address.clone(),
    };
    Ok(ControlExperimentPlan {
        id: format!(
            "experiment:{}:{}:{}",
            loop_definition.id,
            sample.scan,
            kind.wire_name()
        ),
        source: plan_source(sample.source),
        kind,
        affected_signal: manual_command
            .as_ref()
            .map_or_else(|| signal.symbol.clone(), |command| command.symbol.clone()),
        address,
        value_type: manual_command.as_ref().map_or_else(
            || target.iec_type.to_string(),
            |command| command.iec_type.to_string(),
        ),
        from: ExperimentValue::Number(signal.value),
        to: ExperimentValue::Number(excursion.requested),
        unit: signal.unit.clone(),
        minimum: excursion.minimum,
        maximum: excursion.maximum,
        duration_ms,
        expected_scan: sample.scan,
        task: Some(experiment_task(loop_definition)),
        mode_binding: experiment_mode_binding(loop_definition),
        stop_conditions: vec![
            "Abort".to_string(),
            format!("{} s timeout", js_number(input.duration_seconds)),
            "Loop mode changed".to_string(),
            "Bad runtime data".to_string(),
            "Signal force".to_string(),
            "Signal outside its limits".to_string(),
            "Runtime fault".to_string(),
        ],
        restoration_value: ExperimentValue::Number(signal.value),
        requires_physical_approval: sample.source == EvidenceSource::PhysicalPlant,
        trajectory: excitation.trajectory,
        abort_binding: None,
        relay_feedback: excitation.relay_feedback,
        interval_ms: excitation.interval_ms,
        seed: excitation.seed,
        frequencies_hz: excitation.frequencies_hz,
    })
}

/// The rules on the controller's mode and the plant's authority, in the `TypeScript`'s order: exciting the output of a
/// closed loop needs Manual and, when the loop declares one, its Manual command; a setpoint step, and a load step in a
/// closed loop, needs Auto; a setpoint step needs a declared setpoint; a physical plant needs a reviewed topology.
/// Returns the Manual command an output excitation writes.
fn check_mode_and_authority(
    input: &PlanInput<'_>,
    sample: &ExperimentSample,
    kind: ControlExperimentKind,
) -> Result<Option<ManualCommand>, String> {
    let loop_definition = input.loop_definition;
    // A closed-loop controller must be in Manual before its output is excited. An open-loop experiment owns the
    // output, so the review does not invent a controller mode for it.
    let closed_loop = loop_definition.topology != Some(Topology::OpenLoop);
    let excites = kind.excites_output();
    if excites && closed_loop && sample.controller_mode != ControllerMode::Manual {
        return Err(MANUAL_MODE_REFUSAL.to_string());
    }
    let manual_command = if excites && closed_loop {
        manual_command_binding(loop_definition)
    } else {
        None
    };
    if excites
        && closed_loop
        && non_empty(loop_definition.controller.manual_output_signal.as_deref()).is_some()
        && manual_command.is_none()
    {
        return Err(MANUAL_COMMAND_REFUSAL.to_string());
    }
    let needs_automatic_controller = kind == ControlExperimentKind::SetpointStep
        || (kind == ControlExperimentKind::LoadStep && closed_loop);
    if needs_automatic_controller && sample.controller_mode != ControllerMode::Auto {
        return Err(AUTO_MODE_REFUSAL.to_string());
    }
    if kind == ControlExperimentKind::SetpointStep
        && (sample.setpoint.is_none() || loop_definition.signals.setpoint.is_none())
    {
        return Err(SETPOINT_REFUSAL.to_string());
    }
    if sample.source == EvidenceSource::PhysicalPlant && !input.physical_topology_reviewed {
        return Err(PHYSICAL_TOPOLOGY_REFUSAL.to_string());
    }
    Ok(manual_command)
}

/// The values an experiment moves between and the limits that apply to them: an output excitation stays inside the
/// controller's output limits, any other excitation inside its signal's range.
struct Excursion {
    from: f64,
    amplitude: f64,
    requested: f64,
    relay_low_output: f64,
    minimum: f64,
    maximum: f64,
}

/// The excursion of `signal`, refused when the requested value (or a relay's low output) leaves its limits. An output
/// excitation is bounded by the plant input range, which must be set (B3.4 step 1); a missing limit refuses it with the
/// campaign's own sentence.
fn checked_excursion(
    kind: ControlExperimentKind,
    signal: &PlanSignal,
    target: &Target<'_>,
    loop_definition: &LoopDefinition,
    amplitude: f64,
) -> Result<Excursion, String> {
    let (minimum, maximum) = if kind.excites_output() {
        let range = plant_input_range(loop_definition)
            .map_err(|failure| failure.detail.unwrap_or(failure.message))?;
        (range.minimum, range.maximum)
    } else {
        (target.range.minimum, target.range.maximum)
    };
    let requested = signal.value
        + if kind == ControlExperimentKind::Observation {
            0.0
        } else {
            amplitude
        };
    if requested < minimum || requested > maximum {
        return Err(format!(
            "The requested value {} {} is outside {}…{} {}.",
            js_number(requested),
            signal.unit,
            js_number(minimum),
            js_number(maximum),
            signal.unit
        ));
    }
    let relay_low_output = signal.value - amplitude;
    if kind == ControlExperimentKind::Relay
        && (relay_low_output < minimum || relay_low_output > maximum)
    {
        return Err(format!(
            "The requested relay value {} {} is outside {}…{} {}.",
            js_number(relay_low_output),
            signal.unit,
            js_number(minimum),
            js_number(maximum),
            signal.unit
        ));
    }
    Ok(Excursion {
        from: signal.value,
        amplitude,
        requested,
        relay_low_output,
        minimum,
        maximum,
    })
}

/// What an experiment writes over time: its trajectory, and the parameters of a multisine, a relay or a PRBS.
struct Excitation {
    trajectory: Vec<ControlExperimentPoint>,
    frequencies_hz: Option<Vec<f64>>,
    relay_feedback: Option<ControlRelayFeedbackPlan>,
    interval_ms: Option<f64>,
    seed: Option<u32>,
}

/// The excitation of `kind` over `duration_ms`, refused when the automatic trajectory does not fit the duration or a
/// point leaves the excursion's limits.
fn plan_excitation(
    input: &PlanInput<'_>,
    sample: &ExperimentSample,
    kind: ControlExperimentKind,
    excursion: &Excursion,
    duration_ms: f64,
) -> Result<Excitation, String> {
    let period = input
        .loop_definition
        .observed_sample_timing
        .nominal_period_ms;
    let seed = input.seed.unwrap_or(1).max(1);
    let trajectory = if input.kind.kind().is_none() {
        automatic_trajectory(excursion.from, excursion.amplitude, duration_ms, period)
            .ok_or_else(|| TOO_SHORT_REFUSAL.to_string())?
    } else {
        experiment_trajectory(
            kind,
            excursion.from,
            excursion.amplitude,
            duration_ms,
            period,
            seed,
        )
    };
    if trajectory
        .iter()
        .any(|point| point.value < excursion.minimum || point.value > excursion.maximum)
    {
        return Err(OUT_OF_RANGE_REFUSAL.to_string());
    }
    let frequencies_hz = (kind == ControlExperimentKind::Multisine).then(|| {
        vec![
            1.0 / input.duration_seconds,
            2.0 / input.duration_seconds,
            3.0 / input.duration_seconds,
        ]
    });
    let relay_feedback = if kind == ControlExperimentKind::Relay {
        Some(relay_feedback(
            input,
            sample,
            excursion.relay_low_output,
            excursion.requested,
        )?)
    } else {
        None
    };
    let (interval_ms, seed) = if kind == ControlExperimentKind::Prbs {
        (Some((period * 10.0).max(500.0)), Some(seed))
    } else {
        (None, None)
    };
    Ok(Excitation {
        trajectory,
        frequencies_hz,
        relay_feedback,
        interval_ms,
        seed,
    })
}

/// The signal an experiment kind affects and its target: the setpoint, the measurement (an observation), the
/// writeable disturbance, or the manipulated variable.
fn affected_signal<'a>(
    kind: ControlExperimentKind,
    sample: &'a ExperimentSample,
    loop_definition: &'a LoopDefinition,
    disturbance: Option<(&'a AuxiliarySignal, &'a PlanSignal)>,
) -> Option<(&'a PlanSignal, Target<'a>)> {
    match kind {
        ControlExperimentKind::Observation => Some((
            &sample.process_variable,
            Target::of_signal(&loop_definition.signals.process_variable),
        )),
        ControlExperimentKind::SetpointStep => Some((
            sample.setpoint.as_ref()?,
            Target::of_signal(loop_definition.signals.setpoint.as_ref()?),
        )),
        ControlExperimentKind::LoadStep => {
            disturbance.map(|(binding, signal)| (signal, Target::of_auxiliary(binding)))
        }
        _ => Some((
            &sample.manipulated_variable,
            Target::of_signal(&loop_definition.signals.manipulated_variable),
        )),
    }
}

fn missing_signal_refusal(kind: ControlExperimentKind) -> String {
    match kind {
        ControlExperimentKind::SetpointStep => SETPOINT_REFUSAL.to_string(),
        ControlExperimentKind::LoadStep => DISTURBANCE_REFUSAL.to_string(),
        _ => STALE_REFUSAL.to_string(),
    }
}

/// The first writeable disturbance of the loop, with its value in the sample.
fn writeable_disturbance<'a>(
    loop_definition: &'a LoopDefinition,
    sample: &'a ExperimentSample,
) -> Option<(&'a AuxiliarySignal, &'a PlanSignal)> {
    let binding = loop_definition
        .signals
        .additional
        .iter()
        .flatten()
        .find(|signal| is_writeable_disturbance(signal))?;
    let measured = sample
        .additional
        .iter()
        .find(|candidate| candidate.id == binding.id)?;
    Some((binding, &measured.signal))
}

pub(super) fn is_writeable_disturbance(signal: &AuxiliarySignal) -> bool {
    signal.role == AuxiliaryRole::Disturbance && signal.direction == Direction::Write
}

/// The relay of a relay experiment: it switches on the measurement at its I/O address, around the centre of the
/// measured baseline, with a band just above the measured noise (`experimentTrajectory`, `relayFeedback`).
fn relay_feedback(
    input: &PlanInput<'_>,
    sample: &ExperimentSample,
    low_output: f64,
    high_output: f64,
) -> Result<ControlRelayFeedbackPlan, String> {
    let measurement = &sample.process_variable;
    let address = non_empty(measurement.address.as_deref()).ok_or_else(|| {
        format!(
            "The feedback relay switches on the measurement's I/O address, and {} has none. Choose another identification method.",
            measurement.symbol
        )
    })?;
    let range = input.loop_definition.signals.process_variable.range;
    let span = range.maximum - range.minimum;
    // Switch just above the measured noise, so the band stays small next to the cycle.
    let hysteresis = match input.process_noise {
        None => span * 0.005,
        Some(noise) => (2.0 * noise).max(span * 0.0001),
    };
    let center = input.process_center.unwrap_or(measurement.value);
    Ok(ControlRelayFeedbackPlan {
        process_variable: measurement.symbol.clone(),
        address: address.to_string(),
        value_type: input
            .loop_definition
            .signals
            .process_variable
            .iec_type
            .clone(),
        unit: measurement.unit.clone(),
        center,
        hysteresis,
        lower_threshold: center - hysteresis,
        upper_threshold: center + hysteresis,
        low_output,
        high_output,
    })
}

/// A signal the loop's manual output is written through: its symbol, its address when it has one, and its REAL or
/// LREAL type (`manualCommandBinding`).
struct ManualCommand {
    symbol: String,
    address: Option<String>,
    iec_type: &'static str,
}

fn manual_command_binding(loop_definition: &LoopDefinition) -> Option<ManualCommand> {
    let controller = &loop_definition.controller;
    let symbol = non_empty(controller.manual_output_signal.as_deref())?;
    let iec_type = match controller.manual_output_type? {
        ManualOutputType::Real => "REAL",
        ManualOutputType::Lreal => "LREAL",
        ManualOutputType::Bool => return None,
    };
    Some(ManualCommand {
        symbol: symbol.to_string(),
        address: non_empty(controller.manual_output_address.as_deref()).map(str::to_string),
        iec_type,
    })
}

/// The task the runtime's executor follows: the loop's scheduled task, else the background programs (A3.5). A background
/// program is named `$background`; a scheduled task never starts with `$`.
#[must_use]
pub fn experiment_task(loop_definition: &LoopDefinition) -> String {
    loop_definition
        .parameter_binding
        .as_ref()
        .map(|binding| binding.task_name.as_str())
        .filter(|task| !task.is_empty() && !task.starts_with('$'))
        .map_or_else(|| "$background".to_string(), str::to_string)
}

/// The task a recording names in its start request (E4.2, A4.1): the loop's scheduled task, or none for a background program,
/// which every resource cycle samples.
#[must_use]
pub fn recording_task(loop_definition: &LoopDefinition) -> Option<String> {
    let task = experiment_task(loop_definition);
    (!task.starts_with('$')).then_some(task)
}

/// The mode signal and its Manual value, as the experiment checks them (A6.5).
fn experiment_mode_binding(loop_definition: &LoopDefinition) -> Option<ExperimentModeBinding> {
    loop_definition
        .parameter_binding
        .as_ref()
        .map(|binding| ExperimentModeBinding {
            symbol: binding.mode.symbol.clone(),
            manual_value: binding.mode.manual_value,
        })
}

fn plan_source(source: EvidenceSource) -> PlanSource {
    match source {
        EvidenceSource::SimulatedPlant => PlanSource::Simulation,
        EvidenceSource::PhysicalPlant => PlanSource::Physical,
    }
}

pub(super) fn non_empty(text: Option<&str>) -> Option<&str> {
    text.filter(|text| !text.is_empty())
}

/// Whether a value is exactly zero: a zero amplitude is no excitation, and a zero step has no headroom.
#[allow(clippy::float_cmp)] // Zero is exact here: a zero amplitude is refused.
pub(super) fn is_zero(value: f64) -> bool {
    value == 0.0
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
