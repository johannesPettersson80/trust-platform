//! The controller of a loop: its type, direction, design, law and the states an Auto entry
//! initializes (specification 35, §17.1, A6.3).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::bindings::RealType;
use super::on_off::OnOffController;
use super::pid_group::PidControllerGroup;
use super::project::ControllerProject;

/// The controller that runs the loop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Controller {
    /// The controller's instance path, for example `Main.TIC101`.
    pub instance: String,
    /// The controller's type, for example `Control.PID`.
    #[serde(rename = "type")]
    pub type_name: String,
    /// The controller's content revision, `sha256:…`.
    pub revision: String,
    /// Direct: the output rises while the measurement is below the setpoint.
    pub action: Action,
    /// The period of the task that executes the controller, in ms.
    pub task_period_ms: f64,
    /// Who designs the controller: Control Studio's PID designer (the default) or the project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design: Option<Design>,
    /// The controller's equation and the parameters that hold its gains.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub law: Option<ControllerLaw>,
    /// The mode signal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_signal: Option<String>,
    /// The mode the mode signal's TRUE value means.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_when_true: Option<ModeWhenTrue>,
    /// The tracking signal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracking_signal: Option<String>,
    /// The manual output's variable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_output_signal: Option<String>,
    /// The manual output's address; absent for a variable without one, which is written by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_output_address: Option<String>,
    /// The manual output's type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_output_type: Option<ManualOutputType>,
    /// The program copies the output into the manual output outside Manual (§17.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_output_follows_output: Option<bool>,
    /// The controller states an Auto entry initializes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid_initialization: Option<PidInitialization>,
    /// Parked (stage V7): a state-feedback law.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_feedback: Option<Value>,
    /// The switching controller (C3): its three settings, its band, its output levels and its retained states. [`OnOffController`]
    /// decodes and checks it; the gate serves it with its ON/OFF channel shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Value>")]
    pub on_off: Option<OnOffController>,
    /// The program's own law (C2): its family, its states, its seed and its search settings. [`ControllerProject`] decodes and
    /// checks it; the engine serves the `parallel-pid` shape and refuses the rest by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Value>")]
    pub project: Option<ControllerProject>,
    /// A PID group (C7): its `Control.PID` members and their channels. [`PidControllerGroup`] checks the declaration against the loop's
    /// channels; the group's design, Validate and Apply are the engine's (stage V7, wave 4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<PidControllerGroup>,
}

/// The controller's direction of action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// The output rises while the measurement is below the setpoint (positive gain).
    Direct,
    /// The output falls while the measurement is below the setpoint (negative gain).
    Reverse,
}

/// Who designs the controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Design {
    /// Control Studio's PID designer.
    Pid,
    /// The runnable project.
    Project,
}

/// The mode a mode signal's TRUE value means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ModeWhenTrue {
    /// TRUE means Auto.
    Auto,
    /// TRUE means Manual.
    Manual,
}

/// The manual output's type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum ManualOutputType {
    /// `REAL`.
    Real,
    /// `LREAL`.
    Lreal,
    /// `BOOL`, for an ON/OFF output.
    Bool,
}

/// The controller's equation and the parameters that hold its gains.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ControllerLaw {
    /// u = Kp·e.
    #[serde(rename_all = "camelCase")]
    Proportional {
        /// The parameter holding Kp.
        gain_symbol: String,
    },
    /// The parallel PID of a parameter record.
    Pid(Box<PidLaw>),
}

/// The parallel PID law of a parameter record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PidLaw {
    /// The record the controller is configured from.
    pub parameter_record_symbol: String,
    /// An optional counter the project bumps when it adopts a new record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_revision_symbol: Option<String>,
    /// The record field holding Kp.
    pub proportional_gain_symbol: String,
    /// The record field holding Ki.
    pub integral_gain_symbol: String,
    /// The record field holding Kd.
    pub derivative_gain_symbol: String,
    /// The record field β in P = Kp·(β·SP − PV), in [0, 1].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proportional_setpoint_weight_symbol: Option<String>,
    /// The record field γ in D = Kd·(γ·dSP/dt − dPV/dt), in [0, 1].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derivative_setpoint_weight_symbol: Option<String>,
    /// The setting Tf of the measurement filter, in s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurement_filter_time_symbol: Option<String>,
    /// Back-calculation anti-windup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anti_windup: Option<AntiWindup>,
}

/// Back-calculation anti-windup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AntiWindup {
    /// Always back-calculation.
    pub method: AntiWindupMethod,
    /// The record field holding the tracking gain, in 1/s.
    pub tracking_gain_symbol: String,
}

/// The anti-windup method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AntiWindupMethod {
    /// Back-calculation.
    BackCalculation,
}

/// The controller states an Auto entry initializes (specification 35, A6.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind")]
pub enum PidInitialization {
    /// A parallel PID's integral and derivative history.
    #[serde(rename = "parallel-pid-v1", rename_all = "camelCase")]
    ParallelPid {
        /// The integral state.
        integral_state: StateReference,
        /// The derivative history.
        derivative_state: StateReference,
        /// β of the law.
        proportional_setpoint_weight: f64,
        /// γ of the law.
        derivative_setpoint_weight: f64,
        /// The oldest snapshot an Auto entry may use, in ms.
        maximum_snapshot_age_ms: u64,
    },
    /// The seven retained states of the shipped `Control.PID`.
    #[serde(rename = "control-pid-v1", rename_all = "camelCase")]
    ControlPid {
        /// The oldest snapshot an Auto entry may use, in ms.
        maximum_snapshot_age_ms: u64,
        /// The seven states.
        states: Vec<InitializedState>,
        /// Further states of the owning program the entry sets.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        application_states: Option<Vec<InitializedState>>,
    },
}

/// A real-valued state of a parallel PID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StateReference {
    /// The state's path.
    pub symbol: String,
    /// `REAL` or `LREAL`.
    pub iec_type: RealType,
    /// The state's unit.
    pub unit: String,
    /// The state's smallest value.
    pub minimum: f64,
    /// The state's largest value.
    pub maximum: f64,
}

/// A declared controller state with its initial value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InitializedState {
    /// The state's path.
    pub symbol: String,
    /// `REAL`, `LREAL` or `BOOL`.
    pub iec_type: StateType,
    /// The state's unit; `1` for a BOOL.
    pub unit: String,
    /// The state's smallest value.
    pub minimum: f64,
    /// The state's largest value.
    pub maximum: f64,
    /// Where the initial value comes from.
    pub initial: InitialValue,
    /// What the controller's first scan may do to the state.
    #[serde(rename = "postScan")]
    pub post_scan: PostScan,
}

/// The type of a declared state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum StateType {
    /// `REAL`.
    Real,
    /// `LREAL`.
    Lreal,
    /// `BOOL`.
    Bool,
}

/// What the controller's first scan may do to a state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PostScan {
    /// It keeps the value written.
    Unchanged,
    /// It may move within its bounds.
    Bounded,
}

/// A declared state's initial value: a number, or a coordinate of the model or the controller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum InitialValue {
    /// A fixed number.
    Constant(f64),
    /// A coordinate.
    From(InitialSource),
}

/// Where a declared state's initial value comes from (specification 35, §17).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "kebab-case")]
pub enum InitialSource {
    /// A model input at the operating point.
    ModelInput {
        /// The input's index.
        index: usize,
    },
    /// A model output at the operating point.
    ModelOutput {
        /// The output's index.
        index: usize,
    },
    /// A model state at the operating point.
    ModelState {
        /// The state's index.
        index: usize,
    },
    /// A model input that is the given value.
    ModelInputIs {
        /// The input's index.
        index: usize,
        /// The value.
        value: f64,
    },
    /// A PID's integral that holds the operating point.
    #[serde(rename_all = "camelCase")]
    PidIntegral {
        /// The output's index.
        index: usize,
        /// The input's index.
        input_index: usize,
        /// The parameter holding the gain.
        gain_symbol: String,
        /// The parameter holding the weight.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        weight_symbol: Option<String>,
        /// A fixed weight.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        weight: Option<f64>,
    },
    /// A weighted error at the operating point.
    #[serde(rename_all = "camelCase")]
    WeightedError {
        /// The output's index.
        index: usize,
        /// The parameter holding the weight.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        weight_symbol: Option<String>,
        /// A fixed weight.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        weight: Option<f64>,
    },
    /// A model input times a controller parameter.
    #[serde(rename_all = "camelCase")]
    ScaledModelInput {
        /// The input's index.
        index: usize,
        /// The parameter holding the gain.
        gain_symbol: String,
    },
    /// Row `index` of D⁻¹·u for a decoupled group member (serde kind `decoupled-output`).
    #[serde(rename_all = "camelCase")]
    DecoupledOutput {
        /// The row.
        index: usize,
        /// The decoupler's matrix, as parameter paths.
        matrix_symbols: Vec<Vec<String>>,
    },
    /// A notch filter's history at the operating point.
    #[serde(rename_all = "camelCase")]
    NotchHistory {
        /// The output's index.
        index: usize,
        /// The parameters holding the denominator.
        denominator_symbols: [String; 2],
    },
}
