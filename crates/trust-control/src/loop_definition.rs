//! The loop contract (specification 35, §17.1 and A5): what a project's `control-loops/*.json`
//! declares about one control loop, decoded and checked before any runtime access.
//!
//! The parts of the contract that the Rust engine does not run yet (state feedback, ON/OFF, the
//! program's own laws, PID groups, channels, cascades and declared analyses; checklist stage V7)
//! are kept as declared and refused by [`LoopDefinition::rust_engine_support`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::failure::{Cause, Failure, NextAction};

mod bindings;
mod checks;
mod controller;

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

use checks::{expected_primary_direction, is_numeric};

/// The loop file format this crate reads.
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
    /// The declared validation scenarios; ported with Validate (stage V3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_scenarios: Option<Value>,
    /// Parked (stage V7): the loop's channels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<Value>,
    /// Parked (stage V7): a declared control analysis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<Value>,
    /// Parked (stage V7): the analysis's parameter names.
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
    #[error("the loop definition has schema version {0}; Control Studio reads version 2")]
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
    /// Reads and checks a loop file: its fields, and the rules that need no runtime.
    ///
    /// # Errors
    /// The text is not a loop definition of this format, or breaks one of its rules.
    pub fn decode(text: &str) -> Result<Self, LoopRejection> {
        let value: Value = crate::json::parse_text(text)
            .map_err(|error| LoopRejection::Malformed(error.to_string()))?;
        let version = value.get("schemaVersion").and_then(Value::as_u64);
        if version != Some(u64::from(LOOP_SCHEMA_VERSION)) {
            return Err(LoopRejection::UnsupportedVersion(
                version
                    .and_then(|version| u32::try_from(version).ok())
                    .unwrap_or(0),
            ));
        }
        let mut loop_definition: Self = crate::json::decode_value(value)
            .map_err(|error| LoopRejection::Malformed(error.to_string()))?;
        loop_definition
            .check_fields()
            .map_err(LoopRejection::Malformed)?;
        loop_definition.state_dimensionless_units();
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

    /// Whether the Rust engine runs this loop. A loop with a parked feature (checklist V7) stays on
    /// the `TypeScript` engine until that feature is ported.
    ///
    /// # Errors
    /// The loop uses a feature the Rust engine does not run yet, or has no controller law.
    pub fn rust_engine_support(&self) -> Result<(), Failure> {
        let parked = [
            ("state feedback", self.controller.state_feedback.is_some()),
            ("an ON/OFF controller", self.controller.on_off.is_some()),
            ("the program's own law", self.controller.project.is_some()),
            ("a PID group", self.controller.group.is_some()),
            ("channels", self.channels.is_some()),
            (
                "a declared analysis",
                self.analysis.is_some() || self.analysis_parameter_symbols.is_some(),
            ),
            ("a cascade", self.cascade.is_some()),
        ];
        if let Some((feature, _)) = parked.iter().find(|(_, used)| *used) {
            return Err(Failure::new(
                &format!("This loop uses {feature}, which Control Studio's Rust engine does not run yet."),
                Cause::ControllerCapability,
                NextAction::None,
            ));
        }
        if self.controller.law.is_none() {
            return Err(Failure::new(
                "This loop declares no controller law, so Control Studio cannot tune it.",
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
