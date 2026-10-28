//! The loop contract (specification 35, §17.1 and A5): what a project's `control-loops/*.json`
//! declares about one control loop, decoded and checked before any runtime access.
//!
//! The parts of the contract that the Rust engine does not run (the two-loop-file cascade, which C6
//! does not serve; channels outside the shapes the structures define; declared analyses outside the
//! C12 observation loops; and the program's own laws outside the seeds and the search C2 and C5
//! serve) are kept as declared and refused by [`LoopDefinition::rust_engine_support`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::failure::{Cause, Failure, NextAction};

mod analysis_rules;
mod bindings;
mod channel_selection;
mod channels;
mod checks;
mod contract_rules;
mod controller;
mod identification_policy;
mod on_off;
mod pid_group;
mod project;
mod schema_v1;
mod structure_rules;
mod texts;

pub use channel_selection::{
    channel_choices, check_selection, effective_loop, ChannelChoices, ChannelPick, ChannelRow,
    ChannelSelection, SignalChoice,
};
pub use channels::{
    channel_signal, measured_disturbance, ChannelInput, ChannelOutput, ChannelRequirements,
    ChannelSignal, LoopChannels,
};

pub use texts::NO_CONTROLLER_LAW;

pub use bindings::{
    Activation, AuxiliaryRole, AuxiliarySignal, Direction, Execution, Experiment, ExperimentKind,
    Limits, LoopSignal, ModeBinding, Parameter, ParameterBinding, ParameterPurpose, Range,
    RealType, RelayDirection, SampleTiming, Signals, Topology,
};
pub use controller::{
    Action, AntiWindup, AntiWindupMethod, Controller, ControllerLaw, Design, InitialSource,
    InitialValue, InitializedState, ManualOutputType, ModeWhenTrue, PidInitialization, PidLaw,
    PostScan, StateReference, StateType,
};
pub use pid_group::{PidControllerGroup, PidGroupMember};
pub use project::{
    AdditionalMode, ControllerProject, GainSchedule, ObjectiveDirection, ProjectAxis,
    ProjectObjective, ProjectSeedBlock, SchedulePoint, SchedulingVariable,
};

pub use on_off::{
    BandConvention, OnOffController, ThresholdEquality, DWELL_TIMER_MAXIMUM_SECONDS,
    MINIMUM_TIME_MAXIMUM_SECONDS,
};

use checks::{expected_primary_direction, is_numeric};

/// The loop file format this crate writes. It also reads version 1 files, by the rules of the `schema_v1` module, and keeps them as
/// they are.
pub const LOOP_SCHEMA_VERSION: u32 = 2;

/// One control loop of a project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LoopDefinition {
    /// Always [`LOOP_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The loop's identifier, stable across edits.
    pub id: String,
    /// The loop's name as the engineer reads it.
    pub name: String,
    /// The content revision, `sha256:…`.
    pub revision: String,
    /// The project the loop belongs to.
    pub project_id: String,
    /// The runtime the loop is bound to.
    pub runtime_id: String,
    /// The controller that runs the loop.
    pub controller: Controller,
    /// The loop's signals.
    pub signals: Signals,
    /// The output's limits.
    pub limits: Limits,
    /// How often the loop's values are observed.
    pub observed_sample_timing: SampleTiming,
    /// The controller's parameter record, as `control.parameters.*` writes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_binding: Option<ParameterBinding>,
    /// What the bound parameters are for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_purpose: Option<ParameterPurpose>,
    /// Closed loop (the default) or open loop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topology: Option<Topology>,
    /// The loop's identification experiment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experiment: Option<Experiment>,
    /// The simulated plant the loop is tuned against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plant_id: Option<String>,
    /// The operating point's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operating_point: Option<String>,
    /// How Identify treats the plant; ported with Identify (stage V1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identification_policy: Option<Value>,
    /// The declared validation scenarios: a list, which Validate decodes and checks (stage V3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(schema_with = "crate::json::object_list_schema")]
    pub verification_scenarios: Option<Value>,
    /// The loop's channels: the model's inputs and outputs (specification 35, C7 and C8). The gate parks a loop with channels until
    /// their shapes are served (checklist V7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<LoopChannels>,
    /// A declared control analysis (N7.6). A C12 observation or open-loop loop is served with it; any other loop parks on it (stage V7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<Value>,
    /// The analysis's parameters mapped to program symbols. Control Studio writes only the mapped symbols, within the analysis
    /// bounds (B6.1, C12); a loop that is not a C12 observation or open-loop loop parks on it (stage V7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis_parameter_symbols: Option<Value>,
    /// Parked (stage V7): the loop's place in a cascade.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cascade: Option<Value>,
}

/// The project and runtime a loop is opened for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopScope {
    /// The project's identifier.
    pub project_id: String,
    /// The runtime's identifier.
    pub runtime_id: String,
}

/// Why a loop file is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LoopRejection {
    /// The file is not a loop definition of this format.
    #[error("the loop definition is malformed: {0}")]
    Malformed(String),
    /// The file has another schema version.
    #[error("the loop definition has schema version {0}; Control Studio reads versions 1 and 2")]
    UnsupportedVersion(u32),
    /// The loop belongs to another project or runtime.
    #[error("the loop belongs to another project or runtime")]
    CrossRuntime,
    /// A PID-designed loop's controller is not `Control.PID`.
    #[error("only a Control.PID is designed by Control Studio's PID designer")]
    IncompatibleControllerDesign,
    /// The PID initialization does not fit the loop.
    #[error("the PID initialization does not fit the loop: {0}")]
    IncompatiblePidInitialization(String),
    /// An open loop is designed by Control Studio's PID designer.
    #[error("an open loop needs a controller the project designs")]
    IncompatibleTopology,
    /// A signal has the wrong direction.
    #[error("{0} has the wrong direction")]
    InvalidSignalDirection(String),
    /// A signal has a type the loop cannot use.
    #[error("{0} has a type the loop cannot use")]
    IncompatibleSignalType(String),
    /// The setpoint and the measurement have different units.
    #[error("the setpoint and the measurement have different units")]
    IncompatibleUnits,
    /// A further signal reuses an identifier.
    #[error("the signal identifier {0} is used twice")]
    DuplicateSignalId(String),
    /// Two further signals have the same role in one channel.
    #[error("{0} repeats a role in its channel")]
    DuplicateSignalRole(String),
    /// Two signals share address space.
    #[error("{0} and {1} share address space")]
    AmbiguousSignalAddress(String, String),
    /// A channel's setpoint and measurement have different units.
    #[error("channel {0}'s setpoint and measurement have different units")]
    IncompatibleAuxiliaryUnits(String),
}

impl LoopDefinition {
    /// Reads and checks a loop file: its fields, and the rules that need no runtime. A version 1 file is read as version 2, by the
    /// rules of `schema_v1`; the file itself is not rewritten.
    ///
    /// # Errors
    /// The text is not a loop definition of this format, or breaks one of its rules.
    pub fn decode(text: &str) -> Result<Self, LoopRejection> {
        let mut value: Value = crate::json::parse_text(text)
            .map_err(|error| LoopRejection::Malformed(error.to_string()))?;
        let version = value.get("schemaVersion").and_then(Value::as_u64);
        if version == Some(u64::from(schema_v1::SCHEMA_VERSION)) {
            schema_v1::read_as_current(&mut value)?;
        } else if version != Some(u64::from(LOOP_SCHEMA_VERSION)) {
            return Err(LoopRejection::UnsupportedVersion(
                version
                    .and_then(|version| u32::try_from(version).ok())
                    .unwrap_or(0),
            ));
        }
        contract_rules::check(&value).map_err(LoopRejection::Malformed)?;
        let mut loop_definition: Self = crate::json::decode_value(value)
            .map_err(|error| LoopRejection::Malformed(error.to_string()))?;
        loop_definition
            .check_fields()
            .map_err(LoopRejection::Malformed)?;
        if let Some(policy) = &loop_definition.identification_policy {
            identification_policy::check(policy).map_err(LoopRejection::Malformed)?;
        }
        analysis_rules::check(&loop_definition).map_err(LoopRejection::Malformed)?;
        structure_rules::check(&loop_definition).map_err(LoopRejection::Malformed)?;
        loop_definition.state_dimensionless_units();
        if let Some(channels) = &loop_definition.channels {
            channels
                .check_bindings(&loop_definition)
                .map_err(LoopRejection::Malformed)?;
        }
        Ok(loop_definition)
    }

    /// Checks the relationships between the loop's parts and the scope it is opened for.
    ///
    /// # Errors
    /// The loop belongs elsewhere, or its signals, units, addresses or initialization conflict.
    pub fn validate(&self, scope: &LoopScope) -> Result<(), LoopRejection> {
        if self.project_id != scope.project_id || self.runtime_id != scope.runtime_id {
            return Err(LoopRejection::CrossRuntime);
        }
        let pid_designed = self.controller.design.unwrap_or(Design::Pid) == Design::Pid;
        if pid_designed && self.controller.type_name != "Control.PID" {
            return Err(LoopRejection::IncompatibleControllerDesign);
        }
        if self.controller.manual_output_type == Some(ManualOutputType::Bool)
            && (self.controller.on_off.is_none()
                || !self
                    .signals
                    .manipulated_variable
                    .iec_type
                    .eq_ignore_ascii_case("BOOL"))
        {
            return Err(LoopRejection::IncompatibleSignalType(
                "manipulatedVariable".into(),
            ));
        }
        if self.topology == Some(Topology::OpenLoop) && pid_designed {
            return Err(LoopRejection::IncompatibleTopology);
        }
        let execution = self
            .experiment
            .as_ref()
            .map(|experiment| experiment.execution);
        for (name, binding) in self.primary_signals() {
            if binding.direction != expected_primary_direction(execution, name) {
                return Err(LoopRejection::InvalidSignalDirection(name.into()));
            }
        }
        for (name, binding) in self.primary_signals() {
            if !is_numeric(&binding.iec_type) {
                return Err(LoopRejection::IncompatibleSignalType(name.into()));
            }
        }
        if self
            .signals
            .setpoint
            .as_ref()
            .is_some_and(|setpoint| setpoint.unit != self.signals.process_variable.unit)
        {
            return Err(LoopRejection::IncompatibleUnits);
        }
        self.validate_additional(execution)?;
        self.validate_addresses()?;
        if let Some(initialization) = &self.controller.pid_initialization {
            self.validate_pid_initialization(initialization)
                .map_err(LoopRejection::IncompatiblePidInitialization)?;
        }
        Ok(())
    }

    /// Refuses a loop that declares more than one controller structure: a controller law with the program's own law, or any two of a
    /// law, the program's own law, state feedback, an ON/OFF controller and a PID group (specification 35, C2). Control Studio
    /// cannot tell which one tunes the loop, so the Rust engine's gate and the loop discovery both use this check.
    ///
    /// # Errors
    /// The loop declares more than one controller structure.
    pub fn require_one_controller_structure(&self) -> Result<(), Failure> {
        if self.controller.law.is_some() && self.controller.project.is_some() {
            return Err(Failure::new(
                "This loop declares both a controller law and the program's own law, so Control Studio cannot tell which one tunes it.",
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            ));
        }
        if self.controller_structures() > 1 {
            return Err(Failure::new(
                "This loop declares more than one controller structure (a controller law, the program's own law, state feedback, an ON/OFF controller or a PID group), so Control Studio cannot tell which one tunes it.",
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            ));
        }
        Ok(())
    }

    /// Whether the Rust engine runs this loop. A loop with a parked feature (checklist V7) stays on
    /// the `TypeScript` engine until that feature is ported. A loop without a controller law is not parked: it is an open-loop
    /// experiment or an observation loop (specification 35, C12), which the engine opens and identifies; the stages that need a
    /// law refuse it with [`Self::require_controller_law`].
    ///
    /// # Errors
    /// The loop uses a feature the Rust engine does not run yet, or it has no controller law and is not one of the C12 kinds.
    pub fn rust_engine_support(&self) -> Result<(), Failure> {
        let parked = [
            (
                "channels",
                self.channels.is_some() && !self.serves_channels(),
            ),
            ("a declared analysis", self.declares_parked_analysis()),
            ("a cascade", self.cascade.is_some()),
        ];
        if let Some((feature, _)) = parked.iter().find(|(_, used)| *used) {
            return Err(parked_failure(feature));
        }
        if let Some(feature) = self
            .controller
            .project
            .as_ref()
            .and_then(ControllerProject::parked_feature)
        {
            return Err(parked_failure(&feature));
        }
        self.require_one_controller_structure()?;
        if self.controller.law.is_none()
            && self.controller.project.is_none()
            && self.controller.state_feedback.is_none()
            && self.controller.on_off.is_none()
            && self.controller.group.is_none()
            && !self.is_observation_or_open_loop()
        {
            // A loop that should have its law and has none is a loop setup to repair, not a C12 loop.
            return Err(Failure::new(
                "This loop declares no controller law, so Control Studio cannot tune it.",
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            ));
        }
        Ok(())
    }

    /// Whether the loop is one of the two C12 kinds that have no controller law: an open-loop experiment loop (`topology:
    /// "open-loop"`), or an observation loop whose experiment the project runs (`execution: "project-scripted"`). The project
    /// designs the controller of both (`design: "project"`) and binds no law (specification 35, C12). A loop with a program's own law
    /// (`controller.project`, C2), a state-feedback law (C4), an ON/OFF controller (C3) or a PID group (C7) is never one of them: that
    /// structure is its controller.
    #[must_use]
    pub fn is_observation_or_open_loop(&self) -> bool {
        self.controller.law.is_none()
            && self.controller.project.is_none()
            && self.controller.state_feedback.is_none()
            && self.controller.on_off.is_none()
            && self.controller.group.is_none()
            && self.controller.design == Some(Design::Project)
            && (self.topology == Some(Topology::OpenLoop)
                || self
                    .experiment
                    .as_ref()
                    .is_some_and(|experiment| experiment.execution == Execution::ProjectScripted))
    }

    /// How many controller structures the loop declares: its controller law, the program's own law or a PID group, state feedback,
    /// and an ON/OFF controller. A PID group's seed is its program law (C7), so the two together are one structure. A served loop
    /// declares at most one.
    fn controller_structures(&self) -> usize {
        let controller = &self.controller;
        [
            controller.law.is_some(),
            controller.project.is_some() || controller.group.is_some(),
            controller.state_feedback.is_some(),
            controller.on_off.is_some(),
        ]
        .into_iter()
        .filter(|declared| *declared)
        .count()
    }

    /// Whether the engine serves this loop's typed channels: a state-feedback law reads its controlled outputs from them (C4), a
    /// C12 loop records each of them for its measured paths (C7), C8's two-input shape names the measured disturbance, a
    /// cascade's one input and two outputs name its inner and outer measurements (C6), an ON/OFF controller's input carries its
    /// actuator's dwell limits (C3), and a PID group's members are paired with its channels (C7).
    pub(crate) fn serves_channels(&self) -> bool {
        self.controller.state_feedback.is_some()
            || self.is_observation_or_open_loop()
            || self.is_feedforward_channels()
            || self.is_cascade_channels()
            || self.is_on_off_channels()
            || self.controller.group.is_some()
    }

    /// Whether the loop's channels have the shape of a measured-disturbance feedforward (C8): the controller output and then the measured
    /// disturbance are its two inputs, and the process variable is its one output. Only a loop whose program law is a feedforward seed
    /// has that shape, and the gate serves it; any other channels stay parked.
    #[must_use]
    pub(crate) fn is_feedforward_channels(&self) -> bool {
        let Some(loop_channels) = &self.channels else {
            return false;
        };
        if self
            .controller
            .project
            .as_ref()
            .and_then(ControllerProject::feedforward)
            .is_none()
        {
            return false;
        }
        let [controller, disturbance] = loop_channels.inputs.as_slice() else {
            return false;
        };
        controller.signal_id == "manipulatedVariable"
            && channels::measured_disturbance(self, disturbance)
            && loop_channels.outputs.len() == 1
            && loop_channels.outputs[0].signal_id == "processVariable"
    }

    /// Whether the loop's declared analysis or its parameter mapping parks the loop (stage V7). The declared analysis of a C12
    /// observation or open-loop loop is served: the engine reads its tools, and writes its mapped parameters (specification 35,
    /// §0.1a and C12; B6.1). Any other loop with the analysis keys stays on the `TypeScript` engine.
    #[must_use]
    pub fn declares_parked_analysis(&self) -> bool {
        (self.analysis.is_some() || self.analysis_parameter_symbols.is_some())
            && !self.is_observation_or_open_loop()
    }

    /// Refuses a loop of the C12 kinds for the stages that design, check or write a controller law: Tune, Validate, Apply, Restore
    /// and Keep (specification 35, C12). The project owns the actuator of such a loop, so there is nothing for these stages to do.
    /// Any other loop passes here; a loop without a law that is not one of those kinds is refused by `rust_engine_support`.
    ///
    /// # Errors
    /// The loop is an open-loop experiment or observation loop. The failure needs no action.
    pub fn require_controller_law(&self) -> Result<(), Failure> {
        if !self.is_observation_or_open_loop() {
            return Ok(());
        }
        Err(Failure::new(
            NO_CONTROLLER_LAW,
            Cause::Implementation,
            NextAction::None,
        ))
    }
}

/// The parked feature (checklist V7) that a loop file uses, read from its JSON by the keys that
/// [`LoopDefinition::rust_engine_support`] checks, for a file that does not decode as a loop the Rust engine runs: such a loop
/// stays on the `TypeScript` engine until its feature is ported, and is refused as such rather than as a file without a usable
/// loop (V6 decision D12). A program's own law is read by the same gate as the typed loop (`project::parked_feature_of_parts`).
/// `None` when the file uses none of them.
#[must_use]
pub fn parked_feature_of(raw: &Value) -> Option<String> {
    // A file of another schema version is refused by its version (a version-1 file cannot declare these keys), not as parked.
    if raw.get("schemaVersion").and_then(Value::as_u64) != Some(u64::from(LOOP_SCHEMA_VERSION)) {
        return None;
    }
    let present = |value: &Value, key: &str| value.get(key).is_some_and(|member| !member.is_null());
    let controller = &raw["controller"];
    let project_feature = controller
        .get("project")
        .filter(|member| !member.is_null())
        .and_then(project::raw_parked_feature);
    // The channels of a state-feedback law (C4), of a C12 loop (C7) and of a feedforward (C8) are served, so only the other loops
    // with channels are parked by them.
    let feedforward_seed = raw
        .pointer("/controller/project/seed/kind")
        .and_then(Value::as_str)
        == Some(project::FEEDFORWARD_PID);
    let cascade_seed = raw
        .pointer("/controller/project/seed/kind")
        .and_then(Value::as_str)
        == Some(project::CASCADE_PID);
    let served_channels = feedforward_seed
        || cascade_seed
        || present(controller, "stateFeedback")
        || is_observation_or_open_loop_raw(raw)
        || on_off::on_off_channels_raw(raw)
        || present(controller, "group");
    [
        project_feature,
        present(raw, "channels")
            .then(|| "channels".to_owned())
            .filter(|_| !served_channels),
        declares_parked_analysis_raw(raw).then(|| "a declared analysis".to_owned()),
        present(raw, "cascade").then(|| "a cascade".to_owned()),
    ]
    .into_iter()
    .flatten()
    .next()
}

/// Whether a loop file that does not decode parks on its declared analysis, read from its JSON as
/// [`LoopDefinition::declares_parked_analysis`] reads a decoded loop.
fn declares_parked_analysis_raw(raw: &Value) -> bool {
    let present = |key: &str| raw.get(key).is_some_and(|member| !member.is_null());
    (present("analysis") || present("analysisParameterSymbols"))
        && !is_observation_or_open_loop_raw(raw)
}

/// Whether a loop file is one of the two C12 kinds without a controller law, read from its JSON as
/// [`LoopDefinition::is_observation_or_open_loop`] reads a decoded loop: no law, no program's own law, the project's design, and an
/// open-loop topology or a project-scripted experiment.
fn is_observation_or_open_loop_raw(raw: &Value) -> bool {
    let controller = &raw["controller"];
    let absent = |value: &Value, key: &str| value.get(key).is_none_or(Value::is_null);
    absent(controller, "law")
        && absent(controller, "project")
        && absent(controller, "stateFeedback")
        && absent(controller, "onOff")
        && absent(controller, "group")
        && controller.get("design").and_then(Value::as_str) == Some("project")
        && (raw.get("topology").and_then(Value::as_str) == Some("open-loop")
            || raw.pointer("/experiment/execution").and_then(Value::as_str)
                == Some("project-scripted"))
}

/// The refusal of a loop that uses `feature`, which the Rust engine does not run yet (checklist V7).
#[must_use]
pub fn parked_failure(feature: &str) -> Failure {
    Failure::new(
        &format!("This loop uses {feature}, which Control Studio's Rust engine does not run yet."),
        Cause::ControllerCapability,
        NextAction::None,
    )
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "loop_definition/project_tests.rs"]
mod project_tests;

#[cfg(test)]
#[path = "loop_definition/project_feedforward_tests.rs"]
mod project_feedforward_tests;

#[cfg(test)]
#[path = "loop_definition/analysis_gate_tests.rs"]
mod analysis_gate_tests;

#[cfg(test)]
#[path = "loop_definition/engine_support_tests.rs"]
mod engine_support_tests;

#[cfg(test)]
#[path = "loop_definition/experiment_rules_tests.rs"]
mod experiment_rules_tests;
