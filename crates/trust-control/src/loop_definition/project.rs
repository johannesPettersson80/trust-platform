//! The program's own law as a loop declares it (specification 35, C2 and C11): the `controller.project` block, decoded and
//! checked the way `decodeProjectController` checks it in `TypeScript`, and the one gate that says which shapes the Rust engine
//! serves. The engine serves a `parallel-pid` seed, a `trajectory-feedforward-pid` seed (C11), and a `feedforward-pid` seed of the
//! feedforward family (C8), of family P, PI, PD or PID with no tuning axis, component or model parameter. Every other shape stays
//! parked by its name (checklist V7). The search-law form (C5), a block with tuning axes and no seed, is served too: its finite search is
//! `tuning::search`.

#![allow(clippy::module_name_repetitions)] // `ProjectAxis` and its siblings name the module's subject.

use std::collections::{BTreeMap, BTreeSet};

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{json, Map, Value};

use super::{InitializedState, LoopDefinition, Parameter, ParameterBinding, RealType};
use crate::commissioning::initial_state::ControllerInitialization;
use crate::limits::{ISOLATED_SIMULATION_SCAN_LIMIT, SEARCH_WALL_TIME_LIMIT_MS};
use crate::tuning::project_components::{
    validate_components, ProjectTuningComponent, MAXIMUM_COMPONENTS,
};
use crate::tuning::project_model::{ModelParameter, ModelSource};
use crate::tuning::project_seed::{
    check_family_terms, CascadeGains, CascadePidSeed, FeedforwardPidSeed, ParallelPidSeed,
    SmithPidSeed, TrajectoryFeedforwardSeed,
};
use crate::tuning::proportional::GainBounds;
use crate::tuning::settings_pins::single_precision;
use crate::tuning::structure::Structure;

mod cascade_channels;
mod cascade_seed;
mod feedforward_seed;
mod gain_schedule;
mod group_seed;
mod smith_seed;

pub use gain_schedule::{GainSchedule, SchedulePoint, SchedulingVariable};
pub use group_seed::DecoupledPidSeed;

/// The convention a program's own law declares (`decodeProjectController`).
const PROJECT_CONVENTION: &str = "project-owned-native-controller";
/// The families a program's own law may name (`PROJECT_CONTROLLER_FAMILIES`).
const PROJECT_FAMILIES: [&str; 19] = [
    "P",
    "PI",
    "PD",
    "PID",
    "cascade",
    "feedforward",
    "ratio",
    "split-range",
    "override",
    "Smith",
    "paired-PI",
    "decoupled-PI",
    "gain-scheduled",
    "MPC",
    "adaptive",
    "robust",
    "feedback-linearization",
    "observer-feedback",
    "lead-lag",
];
/// The families the engine serves for a `parallel-pid` seed (C2).
const SERVED_FAMILIES: [&str; 4] = ["P", "PI", "PD", "PID"];
/// The seed kind of a parallel law whose gains are named by variables (C2).
const PARALLEL_PID: &str = "parallel-pid";
/// The seed kind of a trajectory-following law: its PID variables and the two feedforward gains' variables (C11).
const TRAJECTORY_PID: &str = "trajectory-feedforward-pid";
/// The seed kind of a measured-disturbance feedforward: its PI variables, the feedforward gain and the optional lead and lag (C8).
pub(super) const FEEDFORWARD_PID: &str = "feedforward-pid";
/// The family a feedforward's project block names (C8).
pub(super) const FEEDFORWARD_FAMILY: &str = "feedforward";
/// The seed kind of a Smith predictor: its PI variables, the model's gain, time constant and delay, and the library block's variables
/// when the program uses it (C9).
const SMITH_PID: &str = "smith-pid";
/// The family a Smith predictor's project block names (C9).
const SMITH_FAMILY: &str = "Smith";
/// The seed of a PID group whose members' PIs are designed on the group model (C7).
pub(super) const DECENTRALIZED_PID: &str = "decentralized-pid";
/// The seed of a PID group whose program applies the static decoupler `D` (C7).
pub(super) const DECOUPLED_PID: &str = "decoupled-pid";
/// The seed of a cascade: the inner and the outer PI, each with its three variables (C6). Its family is `cascade`.
pub(super) const CASCADE_PID: &str = "cascade-pid";
/// The family a cascade's project block names (C6).
pub(super) const CASCADE_FAMILY: &str = "cascade";
/// The seed kinds that stay parked (checklist V7), refused by name. None is left after the cascade (C6) was served.
const PARKED_SEED_KINDS: [&str; 0] = [];
/// The most tuning axes a block declares.
const MAXIMUM_AXES: usize = 8;
/// The most values one axis may try.
const MAXIMUM_AXIS_VALUES: usize = 64;
/// The most model parameters a block declares.
const MAXIMUM_MODEL_PARAMETERS: usize = 64;
/// The most scenario identifiers a block requires.
const MAXIMUM_REQUIRED_SCENARIOS: usize = 64;
/// The most additional modes a block declares.
const MAXIMUM_ADDITIONAL_MODES: usize = 8;

/// A program's own law as its loop declares it (C2; the `ProjectControllerBinding` of `TypeScript`).
#[derive(Debug, Clone, PartialEq)]
pub struct ControllerProject {
    /// The family of the law: one of the nineteen project families.
    pub family: String,
    /// The states an Auto entry initializes, with their initial values: the law's own states, then the application states.
    pub initialization: ControllerInitialization,
    /// The symbols of the application states among `initialization`'s states (C1, `applicationStates`): the states that accompany
    /// the law's states, such as a startup-done flag that an Auto entry sets. They belong to the controller's owning program.
    pub application_states: Vec<String>,
    /// The seed that names the law's variables; `None` when the block names none.
    pub seed: Option<ProjectSeedBlock>,
    /// The tuning axes of a finite search (C5), at most eight.
    pub axes: Vec<ProjectAxis>,
    /// What the search ranks its candidates by.
    pub objective: ProjectObjective,
    /// The most candidates a search tries, from 1 to 64.
    pub maximum_candidates: u32,
    /// The most scans the trials of a search may run together (A7, scans per job), from 1 to the isolated-run scan limit.
    pub maximum_cycles: u64,
    /// The wall-clock budget of a search, in milliseconds.
    pub wall_budget_ms: u64,
    /// The tuning components of the law (C5), at most eight.
    pub components: Vec<ProjectTuningComponent>,
    /// The bound parameters that the model's timing or gains set (C5), each with its source.
    pub model_parameters: BTreeMap<String, ModelSource>,
    /// The verification scenarios that the candidates must pass, at most 64.
    pub required_scenario_ids: Vec<String>,
    /// The further BOOL modes of the program, at most eight, each distinct from the loop's mode.
    pub additional_modes: Vec<AdditionalMode>,
    /// The schedule of a `gain-scheduled` law (C10): the table of operating points that this loop's point belongs to. `None` for
    /// every other law.
    pub schedule: Option<GainSchedule>,
}

/// The seed of a program's own law: a parallel law or a trajectory-following law whose variables the engine serves, or a seed of a parked
/// kind.
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectSeedBlock {
    /// A `parallel-pid` seed: the family, and the variables of the proportional, integral and derivative gains.
    ParallelPid(ParallelPidSeed),
    /// A `trajectory-feedforward-pid` seed (C11): the PID variables, and the variables of the rate and acceleration feedforward gains.
    TrajectoryFeedforward(TrajectoryFeedforwardSeed),
    /// A `feedforward-pid` seed (C8): the PI variables, the variable of the feedforward gain, and the lead and lag variables when named.
    Feedforward(FeedforwardPidSeed),
    /// A `smith-pid` seed (C9): the PI variables, the model's gain, time constant and delay, and the library block's variables when named.
    Smith(SmithPidSeed),
    /// A `decentralized-pid` seed (C7): each member's PI is designed on the group model, and the group names the members' variables.
    Decentralized,
    /// A `decoupled-pid` seed (C7): the static decoupler's variables `D[i][j]`.
    Decoupled(DecoupledPidSeed),
    /// A `cascade-pid` seed (C6): the inner and the outer PI, each with its three variables.
    Cascade(CascadePidSeed),
    /// A seed of a parked kind, refused by its name before its other fields are read.
    Parked {
        /// The seed's kind, for example `feedforward-pid`.
        kind: String,
    },
}

/// One tuning axis: a bound parameter and the values that a search may try for it (C5).
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectAxis {
    /// The bound parameter's path.
    pub symbol: String,
    /// The values to try, each finite and within the parameter's bounds.
    pub values: Vec<f64>,
}

/// Whether the objective is made smaller or larger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectiveDirection {
    /// The smaller the value, the better.
    Minimize,
    /// The larger the value, the better.
    Maximize,
}

/// What a search ranks its candidates by: a declared requirement and the direction of the ranking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectObjective {
    /// The requirement's identifier.
    pub requirement_id: String,
    /// Whether the requirement's value is minimized or maximized.
    pub direction: ObjectiveDirection,
}

/// A further BOOL mode of the program, distinct from the loop's mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdditionalMode {
    /// The mode's variable.
    pub symbol: String,
    /// The value that means Manual.
    pub manual_value: bool,
}

/// The one gate of a program's own law (C2, checklist V7): the name of the parked feature that a block uses, or `None` when the
/// engine serves it. `seed_kind` is `None` for a block without a seed. `searches` is true when the block declares tuning axes,
/// components or model parameters. The typed loop, the raw file and the adapter all read a block through this function.
pub(super) fn parked_feature_of_parts(
    seed_kind: Option<&str>,
    family: &str,
    searches: bool,
) -> Option<String> {
    match seed_kind {
        None => Some("the program's own law without a seed".to_owned()),
        // A gain schedule's point is a parallel PI seed of the gain-scheduled family (C10).
        Some(PARALLEL_PID) if family == gain_schedule::FAMILY && !searches => None,
        Some(PARALLEL_PID | TRAJECTORY_PID) if !SERVED_FAMILIES.contains(&family) => {
            Some(format!("the program's own law of family {family}"))
        }
        Some(FEEDFORWARD_PID) if family != FEEDFORWARD_FAMILY => {
            Some(format!("the program's own law of family {family}"))
        }
        Some(SMITH_PID) if family != SMITH_FAMILY => {
            Some(format!("the program's own law of family {family}"))
        }
        Some(CASCADE_PID) if family != CASCADE_FAMILY => {
            Some(format!("the program's own law of family {family}"))
        }
        Some(
            PARALLEL_PID | TRAJECTORY_PID | FEEDFORWARD_PID | SMITH_PID | DECENTRALIZED_PID
            | DECOUPLED_PID | CASCADE_PID,
        ) if searches => Some(
            "the program's own law with tuning axes, components or model parameters".to_owned(),
        ),
        Some(
            PARALLEL_PID | TRAJECTORY_PID | FEEDFORWARD_PID | SMITH_PID | DECENTRALIZED_PID
            | DECOUPLED_PID | CASCADE_PID,
        ) => None,
        Some(kind) => Some(format!("the program's own law with a {kind} seed")),
    }
}

/// The search-law form (C5) read from its parts: no seed, at least one tuning axis, and no component or model parameter. Its variables are
/// the axes' grid, which the finite search tunes (`tuning::search`).
fn is_search_law_parts(has_seed: bool, has_axes: bool, has_other_searches: bool) -> bool {
    !has_seed && has_axes && !has_other_searches
}

/// The parked feature of a `controller.project` value read as raw JSON, for a file that does not decode (V6 decision D12). A seed of
/// a kind the loop contract does not know makes the block malformed, not parked, so `None` is returned for it. The search-law form (C5)
/// is served, so it is not parked.
pub(super) fn raw_parked_feature(project: &Value) -> Option<String> {
    let seed_kind = match project.get("seed").filter(|seed| !seed.is_null()) {
        None => None,
        Some(seed) => Some(seed.get("kind").and_then(Value::as_str).filter(|kind| {
            [
                PARALLEL_PID,
                TRAJECTORY_PID,
                FEEDFORWARD_PID,
                SMITH_PID,
                DECENTRALIZED_PID,
                DECOUPLED_PID,
                CASCADE_PID,
            ]
            .contains(kind)
                || PARKED_SEED_KINDS.contains(kind)
        })?),
    };
    let family = project
        .get("family")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let has_other_searches = ["components", "modelParameters"]
        .iter()
        .any(|key| project.get(key).is_some_and(has_members));
    let search_law = is_search_law_parts(
        seed_kind.is_some(),
        project.get("axes").is_some_and(has_members),
        has_other_searches,
    );
    if search_law {
        return None;
    }
    let searches = has_other_searches || project.get("axes").is_some_and(has_members);
    parked_feature_of_parts(seed_kind, family, searches)
}

/// Whether a list or an object of a raw block holds at least one member.
fn has_members(member: &Value) -> bool {
    member.as_array().is_some_and(|items| !items.is_empty())
        || member.as_object().is_some_and(|fields| !fields.is_empty())
}

impl ControllerProject {
    /// Reads a `controller.project` block: every rule of the loop contract that needs no parameter binding. These are the convention,
    /// the family, the initialization, the seed's shape, the axes, the objective, the budgets, the components' shape and the lists.
    /// The variables are checked against the loop's parameter binding when the loop is decoded.
    ///
    /// # Errors
    /// The block breaks one of these rules. The text names the field.
    pub fn from_value(value: &Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or("controller.project must be an object")?;
        if text_field(object, "convention")? != PROJECT_CONVENTION {
            return Err(format!(
                "controller.project.convention must be {PROJECT_CONVENTION}"
            ));
        }
        let family = text_field(object, "family")?;
        if !PROJECT_FAMILIES.contains(&family) {
            return Err(format!(
                "controller.project.family {family} is not a project family"
            ));
        }
        let candidates = bounded_integer(object, "maximumCandidates", 1, 64)?;
        let (initialization, application_states) =
            initialization(field(object, "initialization")?)?;
        let seed = match object.get("seed") {
            None | Some(Value::Null) => None,
            Some(seed) => Some(seed_block(seed)?),
        };
        let schedule = gain_schedule::schedule_of(object)?;
        gain_schedule::check_block(family, schedule.as_ref(), seed.as_ref())?;
        Ok(Self {
            family: family.to_owned(),
            initialization,
            application_states,
            seed,
            schedule,
            axes: axes(object)?,
            objective: objective(field(object, "objective")?)?,
            maximum_candidates: u32::try_from(candidates).map_err(|_| {
                "controller.project.maximumCandidates must be a whole number from 1 to 64"
                    .to_owned()
            })?,
            maximum_cycles: bounded_integer(
                object,
                "maximumCycles",
                1,
                ISOLATED_SIMULATION_SCAN_LIMIT,
            )?,
            wall_budget_ms: bounded_integer(object, "wallBudgetMs", 1, SEARCH_WALL_TIME_LIMIT_MS)?,
            components: components(object)?,
            model_parameters: model_parameters(object)?,
            required_scenario_ids: required_scenario_ids(object)?,
            additional_modes: additional_modes(object)?,
        })
    }

    /// Whether the block is the search-law form (C5): no seed, at least one tuning axis, and no component or model parameter.
    #[must_use]
    pub fn is_search_law(&self) -> bool {
        is_search_law_parts(
            self.seed.is_some(),
            !self.axes.is_empty(),
            !self.components.is_empty() || !self.model_parameters.is_empty(),
        )
    }

    /// The parked feature that the block uses, or `None` when the engine serves it (C2). This is the gate of
    /// `parked_feature_of_parts`, read from the typed block. The search-law form (C5) is served.
    #[must_use]
    pub fn parked_feature(&self) -> Option<String> {
        if self.is_search_law() {
            return None;
        }
        let seed_kind = match &self.seed {
            None => None,
            Some(ProjectSeedBlock::ParallelPid(_)) => Some(PARALLEL_PID),
            Some(ProjectSeedBlock::TrajectoryFeedforward(_)) => Some(TRAJECTORY_PID),
            Some(ProjectSeedBlock::Feedforward(_)) => Some(FEEDFORWARD_PID),
            Some(ProjectSeedBlock::Smith(_)) => Some(SMITH_PID),
            Some(ProjectSeedBlock::Decentralized) => Some(DECENTRALIZED_PID),
            Some(ProjectSeedBlock::Decoupled(_)) => Some(DECOUPLED_PID),
            Some(ProjectSeedBlock::Cascade(_)) => Some(CASCADE_PID),
            Some(ProjectSeedBlock::Parked { kind }) => Some(kind.as_str()),
        };
        let searches = !self.axes.is_empty()
            || !self.components.is_empty()
            || !self.model_parameters.is_empty();
        parked_feature_of_parts(seed_kind, &self.family, searches)
    }

    /// Whether a state of the initialization is an application state (C1), by its symbol.
    #[must_use]
    pub fn is_application_state(&self, symbol: &str) -> bool {
        self.application_states
            .iter()
            .any(|name| name.as_str() == symbol)
    }

    /// The trajectory feedforward seed of the block (C11), when the block declares one.
    #[must_use]
    pub fn trajectory(&self) -> Option<&TrajectoryFeedforwardSeed> {
        match self.seed.as_ref()? {
            ProjectSeedBlock::TrajectoryFeedforward(seed) => Some(seed),
            ProjectSeedBlock::ParallelPid(_)
            | ProjectSeedBlock::Feedforward(_)
            | ProjectSeedBlock::Smith(_)
            | ProjectSeedBlock::Decentralized
            | ProjectSeedBlock::Decoupled(_)
            | ProjectSeedBlock::Cascade(_)
            | ProjectSeedBlock::Parked { .. } => None,
        }
    }

    /// The feedforward seed of the block (C8), when the block declares one.
    #[must_use]
    pub fn feedforward(&self) -> Option<&FeedforwardPidSeed> {
        match self.seed.as_ref()? {
            ProjectSeedBlock::Feedforward(seed) => Some(seed),
            ProjectSeedBlock::ParallelPid(_)
            | ProjectSeedBlock::TrajectoryFeedforward(_)
            | ProjectSeedBlock::Smith(_)
            | ProjectSeedBlock::Decentralized
            | ProjectSeedBlock::Decoupled(_)
            | ProjectSeedBlock::Cascade(_)
            | ProjectSeedBlock::Parked { .. } => None,
        }
    }

    /// The cascade seed of the block (C6), when the block declares one.
    #[must_use]
    pub fn cascade(&self) -> Option<&CascadePidSeed> {
        match self.seed.as_ref()? {
            ProjectSeedBlock::Cascade(seed) => Some(seed),
            ProjectSeedBlock::ParallelPid(_)
            | ProjectSeedBlock::TrajectoryFeedforward(_)
            | ProjectSeedBlock::Feedforward(_)
            | ProjectSeedBlock::Smith(_)
            | ProjectSeedBlock::Decentralized
            | ProjectSeedBlock::Decoupled(_)
            | ProjectSeedBlock::Parked { .. } => None,
        }
    }

    /// The Smith predictor's seed of the block (C9), when the block declares one.
    #[must_use]
    pub fn smith(&self) -> Option<&SmithPidSeed> {
        match self.seed.as_ref()? {
            ProjectSeedBlock::Smith(seed) => Some(seed),
            ProjectSeedBlock::ParallelPid(_)
            | ProjectSeedBlock::TrajectoryFeedforward(_)
            | ProjectSeedBlock::Feedforward(_)
            | ProjectSeedBlock::Decentralized
            | ProjectSeedBlock::Decoupled(_)
            | ProjectSeedBlock::Cascade(_)
            | ProjectSeedBlock::Parked { .. } => None,
        }
    }

    /// The families the engine offers for the law (C2 Setup): P always, and PI, PD and PID where the seed names their terms
    /// (`projectControllerFamilies`). Empty for a block without a served seed. A feedforward's feedback law is a PI and no other
    /// family (C0, C8).
    #[must_use]
    pub fn offered_families(&self) -> Vec<Structure> {
        if self.feedforward().is_some() || self.smith().is_some() || self.cascade().is_some() {
            return vec![Structure::PI];
        }
        let Some(seed) = self.seed.as_ref().and_then(ProjectSeedBlock::pid_seed) else {
            return Vec::new();
        };
        [Structure::P, Structure::PI, Structure::PD, Structure::PID]
            .into_iter()
            .filter(|family| check_family_terms(seed, *family).is_ok())
            .collect()
    }

    /// The block as the loop file writes it. The optional lists are written only when they hold values.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut block = Map::new();
        block.insert("convention".to_owned(), Value::from(PROJECT_CONVENTION));
        block.insert("family".to_owned(), Value::from(self.family.as_str()));
        let (states, application): (Vec<&InitializedState>, Vec<&InitializedState>) = self
            .initialization
            .states()
            .iter()
            .partition(|state| !self.is_application_state(&state.symbol));
        let mut initialization = json!({
            "kind": "declared-state-v1",
            "states": serde_json::to_value(states).unwrap_or_default(),
        });
        if !application.is_empty() {
            initialization["applicationStates"] =
                serde_json::to_value(application).unwrap_or_default();
        }
        block.insert("initialization".to_owned(), initialization);
        if let Some(seed) = &self.seed {
            block.insert("seed".to_owned(), seed.to_value());
        }
        let axes = self.axes.iter().map(ProjectAxis::to_value).collect();
        block.insert("axes".to_owned(), Value::Array(axes));
        block.insert(
            "objective".to_owned(),
            json!({
                "requirementId": self.objective.requirement_id,
                "direction": self.objective.direction.word(),
            }),
        );
        block.insert(
            "maximumCandidates".to_owned(),
            Value::from(self.maximum_candidates),
        );
        block.insert("maximumCycles".to_owned(), Value::from(self.maximum_cycles));
        block.insert("wallBudgetMs".to_owned(), Value::from(self.wall_budget_ms));
        if !self.components.is_empty() {
            let components = serde_json::to_value(&self.components).unwrap_or_default();
            block.insert("components".to_owned(), components);
        }
        if !self.model_parameters.is_empty() {
            let sources: Map<String, Value> = self
                .model_parameters
                .iter()
                .map(|(symbol, source)| {
                    (
                        symbol.clone(),
                        json!({ "kind": model_source_word(*source) }),
                    )
                })
                .collect();
            block.insert("modelParameters".to_owned(), Value::Object(sources));
        }
        if !self.required_scenario_ids.is_empty() {
            block.insert(
                "requiredScenarioIds".to_owned(),
                json!(self.required_scenario_ids),
            );
        }
        if !self.additional_modes.is_empty() {
            let modes = self
                .additional_modes
                .iter()
                .map(|mode| json!({ "symbol": mode.symbol, "manualValue": mode.manual_value }))
                .collect();
            block.insert("additionalModes".to_owned(), Value::Array(modes));
        }
        if let Some(schedule) = &self.schedule {
            block.insert("schedule".to_owned(), schedule.to_value());
        }
        Value::Object(block)
    }

    /// Checks the block against the loop's parameter binding (`decodeProjectController` and `validParallelSeed`): every variable is a
    /// bound parameter, each axis value is within its bounds and, for a `REAL`, exact in single precision, the components are valid
    /// and do not share a variable with an axis, and each additional mode differs from the loop's mode and from the others.
    ///
    /// # Errors
    /// A variable is not a bound parameter, or a value breaks its rule. The text names the field.
    pub(super) fn check_binding(&self, binding: &ParameterBinding) -> Result<(), String> {
        if let Some(seed) = &self.seed {
            for symbol in seed.variables() {
                require_bound(binding, symbol, "controller.project.seed")?;
            }
        }
        for axis in &self.axes {
            let parameter = require_bound(binding, &axis.symbol, "controller.project.axes")?;
            let single = parameter.iec_type == RealType::Real;
            let fits = axis.values.iter().all(|value| {
                *value >= parameter.minimum
                    && *value <= parameter.maximum
                    && (!single || single_precision(*value).to_bits() == value.to_bits())
            });
            if !fits {
                return Err(format!(
                    "controller.project.axes values of {} are outside its bounds or not exact REAL values",
                    axis.symbol
                ));
            }
        }
        let parameters: Vec<ModelParameter> = binding
            .parameters
            .iter()
            .map(|parameter| ModelParameter {
                symbol: parameter.symbol.clone(),
                unit: parameter.unit.clone(),
                real: parameter.iec_type == RealType::Real,
                bounds: GainBounds {
                    minimum: parameter.minimum,
                    maximum: parameter.maximum,
                },
            })
            .collect();
        validate_components(&self.components, &parameters).map_err(|refusal| refusal.0)?;
        let shared = self
            .components
            .iter()
            .flat_map(ProjectTuningComponent::symbols);
        if shared
            .into_iter()
            .any(|symbol| self.axes.iter().any(|axis| axis.symbol == symbol))
        {
            return Err(
                "controller.project.components and controller.project.axes name the same variable"
                    .to_owned(),
            );
        }
        for symbol in self.model_parameters.keys() {
            require_bound(binding, symbol, "controller.project.modelParameters")?;
        }
        let mut modes = vec![binding.mode.symbol.to_uppercase()];
        for mode in &self.additional_modes {
            let upper = mode.symbol.to_uppercase();
            if modes.contains(&upper) {
                return Err(format!(
                    "controller.project.additionalModes {} repeats the loop's mode or another mode",
                    mode.symbol
                ));
            }
            modes.push(upper);
        }
        Ok(())
    }
}

impl ObjectiveDirection {
    /// The word that the loop file writes for the direction.
    fn word(self) -> &'static str {
        match self {
            Self::Minimize => "minimize",
            Self::Maximize => "maximize",
        }
    }
}

impl ProjectSeedBlock {
    /// The PID part of a served seed: a `parallel-pid` seed, or the PID variables of a trajectory seed (C11). `None` for a parked seed.
    #[must_use]
    pub fn pid_seed(&self) -> Option<&ParallelPidSeed> {
        match self {
            Self::ParallelPid(seed) => Some(seed),
            Self::TrajectoryFeedforward(seed) => Some(&seed.pid),
            Self::Feedforward(seed) => Some(&seed.pid),
            Self::Smith(seed) => Some(&seed.pid),
            Self::Decentralized | Self::Decoupled(_) | Self::Cascade(_) | Self::Parked { .. } => {
                None
            }
        }
    }

    /// The variables the seed names, the proportional one first; a parked seed names none (C2, C8, C11).
    #[must_use]
    pub fn variables(&self) -> Vec<&str> {
        match self {
            Self::ParallelPid(seed) => seed_symbols(seed),
            Self::TrajectoryFeedforward(seed) => seed.variables(),
            Self::Feedforward(seed) => seed.variables(),
            Self::Smith(seed) => seed.variables(),
            Self::Decoupled(seed) => seed.variables(),
            Self::Cascade(seed) => seed.variables(),
            Self::Decentralized | Self::Parked { .. } => Vec::new(),
        }
    }

    /// The seed as the loop file writes it.
    fn to_value(&self) -> Value {
        match self {
            Self::ParallelPid(seed) => {
                let mut value = serde_json::to_value(seed).unwrap_or_default();
                if let Value::Object(fields) = &mut value {
                    fields.insert("kind".to_owned(), Value::from(PARALLEL_PID));
                }
                value
            }
            Self::TrajectoryFeedforward(seed) => {
                let mut value = serde_json::to_value(&seed.pid).unwrap_or_default();
                if let Value::Object(fields) = &mut value {
                    fields.insert("kind".to_owned(), Value::from(TRAJECTORY_PID));
                    fields.insert(
                        "rateSymbol".to_owned(),
                        Value::from(seed.rate_symbol.as_str()),
                    );
                    fields.insert(
                        "accelerationSymbol".to_owned(),
                        Value::from(seed.acceleration_symbol.as_str()),
                    );
                }
                value
            }
            Self::Feedforward(seed) => {
                let mut value = serde_json::to_value(&seed.pid).unwrap_or_default();
                if let Value::Object(fields) = &mut value {
                    fields.insert("kind".to_owned(), Value::from(FEEDFORWARD_PID));
                    fields.insert(
                        "gainSymbol".to_owned(),
                        Value::from(seed.gain_symbol.as_str()),
                    );
                    if let Some(lead_lag) = &seed.lead_lag {
                        fields.insert(
                            "leadSymbol".to_owned(),
                            Value::from(lead_lag.lead_symbol.as_str()),
                        );
                        fields.insert(
                            "lagSymbol".to_owned(),
                            Value::from(lead_lag.lag_symbol.as_str()),
                        );
                    }
                }
                value
            }
            Self::Smith(seed) => {
                let mut value = serde_json::to_value(&seed.pid).unwrap_or_default();
                if let Value::Object(fields) = &mut value {
                    fields.insert("kind".to_owned(), Value::from(SMITH_PID));
                    fields.insert(
                        "gainSymbol".to_owned(),
                        Value::from(seed.gain_symbol.as_str()),
                    );
                    fields.insert(
                        "timeConstantSymbol".to_owned(),
                        Value::from(seed.time_constant_symbol.as_str()),
                    );
                    fields.insert(
                        "delaySymbol".to_owned(),
                        Value::from(seed.delay_symbol.as_str()),
                    );
                    if let Some(block) = &seed.library_block {
                        fields.insert(
                            "predictorOutputSymbol".to_owned(),
                            Value::from(block.output_symbol.as_str()),
                        );
                        fields.insert(
                            "maximumCorrectionSymbol".to_owned(),
                            Value::from(block.maximum_correction_symbol.as_str()),
                        );
                        fields.insert(
                            "operatingInputSymbol".to_owned(),
                            Value::from(block.operating_input_symbol.as_str()),
                        );
                        fields.insert(
                            "operatingOutputSymbol".to_owned(),
                            Value::from(block.operating_output_symbol.as_str()),
                        );
                    }
                }
                value
            }
            Self::Decentralized => json!({ "kind": DECENTRALIZED_PID, "family": "PI" }),
            Self::Decoupled(seed) => json!({
                "kind": DECOUPLED_PID,
                "family": "PI",
                "matrixSymbols": seed.matrix_symbols,
            }),
            Self::Cascade(seed) => json!({
                "kind": CASCADE_PID,
                "family": "PI",
                "inner": cascade_gains_value(&seed.inner),
                "outer": cascade_gains_value(&seed.outer),
            }),
            Self::Parked { kind } => json!({ "kind": kind }),
        }
    }
}

impl ProjectAxis {
    /// The axis as the loop file writes it.
    fn to_value(&self) -> Value {
        json!({ "symbol": self.symbol, "values": self.values })
    }
}

impl Serialize for ControllerProject {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_value().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ControllerProject {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Self::from_value(&value).map_err(D::Error::custom)
    }
}

impl LoopDefinition {
    /// Checks the program's own law against the loop: the loop needs its parameter binding to check the variables against.
    ///
    /// # Errors
    /// The loop has no parameter binding, or the block does not fit the binding.
    pub(super) fn check_project(&self) -> Result<(), String> {
        let Some(project) = &self.controller.project else {
            return Ok(());
        };
        let Some(binding) = &self.parameter_binding else {
            return Err("controller.project needs the loop's parameter binding".to_owned());
        };
        let root = format!(
            "{}.",
            self.controller
                .instance
                .split('.')
                .next()
                .unwrap_or_default()
                .to_uppercase()
        );
        if project
            .application_states
            .iter()
            .any(|symbol| !symbol.to_uppercase().starts_with(&root))
        {
            return Err(
                "controller.project.initialization.applicationStates belong to the controller's owning program"
                    .to_owned(),
            );
        }
        project.check_binding(binding)
    }
}

/// The variables a parallel seed names, the proportional one first.
fn seed_symbols(seed: &ParallelPidSeed) -> Vec<&str> {
    let mut symbols = vec![seed.proportional_symbol.as_str()];
    symbols.extend(seed.integral_symbol.as_deref());
    symbols.extend(seed.derivative_symbol.as_deref());
    symbols
}

/// One controller of a cascade seed as the loop file writes it (C6): its instance and its three variables.
fn cascade_gains_value(gains: &CascadeGains) -> Value {
    json!({
        "instance": gains.instance,
        "proportionalSymbol": gains.proportional_symbol,
        "integralSymbol": gains.integral_symbol,
        "derivativeSymbol": gains.derivative_symbol,
    })
}

/// Whether two of the symbols are the same variable.
fn has_repeat(symbols: &[&str]) -> bool {
    let mut sorted = symbols.to_vec();
    sorted.sort_unstable();
    sorted.windows(2).any(|pair| pair[0] == pair[1])
}

/// The bound parameter that `symbol` names, or the refusal that names the field it is declared in.
fn require_bound<'a>(
    binding: &'a ParameterBinding,
    symbol: &str,
    context: &str,
) -> Result<&'a Parameter, String> {
    binding
        .parameters
        .iter()
        .find(|parameter| parameter.symbol == symbol)
        .ok_or_else(|| {
            format!("{context} names {symbol}, which is not a bound parameter of the loop")
        })
}

fn field<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a Value, String> {
    object
        .get(name)
        .ok_or_else(|| format!("controller.project.{name} is missing"))
}

fn text_field<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a str, String> {
    field(object, name)?
        .as_str()
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("controller.project.{name} must be a non-empty text"))
}

fn list_field<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a Vec<Value>, String> {
    field(object, name)?
        .as_array()
        .ok_or_else(|| format!("controller.project.{name} must be a list"))
}

fn bounded_integer(
    object: &Map<String, Value>,
    name: &str,
    minimum: u64,
    maximum: u64,
) -> Result<u64, String> {
    field(object, name)?
        .as_u64()
        .filter(|number| (minimum..=maximum).contains(number))
        .ok_or_else(|| {
            format!("controller.project.{name} must be a whole number from {minimum} to {maximum}")
        })
}

/// The declared states of a program's law and its application states (C1), merged in that order, with the symbols of the application
/// states. The application states are optional; they follow the law's states in the merged record, as Control.PID's do.
fn initialization(value: &Value) -> Result<(ControllerInitialization, Vec<String>), String> {
    let object = value
        .as_object()
        .ok_or("controller.project.initialization must be an object")?;
    if text_field(object, "kind")? != "declared-state-v1" {
        return Err("controller.project.initialization.kind must be declared-state-v1".to_owned());
    }
    let states: Vec<InitializedState> = crate::json::decode_value(field(object, "states")?.clone())
        .map_err(|error| format!("controller.project.initialization.states: {error}"))?;
    let application: Vec<InitializedState> = match object.get("applicationStates") {
        None | Some(Value::Null) => Vec::new(),
        Some(list) => crate::json::decode_value(list.clone()).map_err(|error| {
            format!("controller.project.initialization.applicationStates: {error}")
        })?,
    };
    let symbols = application
        .iter()
        .map(|state| state.symbol.clone())
        .collect();
    let merged = ControllerInitialization::new(states.into_iter().chain(application).collect())
        .map_err(|failure| failure.message)?;
    Ok((merged, symbols))
}

fn objective(value: &Value) -> Result<ProjectObjective, String> {
    let object = value
        .as_object()
        .ok_or("controller.project.objective must be an object")?;
    let requirement_id = text_field(object, "requirementId")?;
    let direction = match text_field(object, "direction")? {
        "minimize" => ObjectiveDirection::Minimize,
        "maximize" => ObjectiveDirection::Maximize,
        _ => {
            return Err(
                "controller.project.objective.direction must be minimize or maximize".to_owned(),
            )
        }
    };
    Ok(ProjectObjective {
        requirement_id: requirement_id.to_owned(),
        direction,
    })
}

/// The seed of a block: a parked kind is read no further (its refusal is by name); a `parallel-pid` or a trajectory seed is read whole.
fn seed_block(value: &Value) -> Result<ProjectSeedBlock, String> {
    let object = value
        .as_object()
        .ok_or("controller.project.seed must be an object")?;
    let kind = text_field(object, "kind")?;
    if PARKED_SEED_KINDS.contains(&kind) {
        return Ok(ProjectSeedBlock::Parked {
            kind: kind.to_owned(),
        });
    }
    if kind == TRAJECTORY_PID {
        return trajectory_seed(object).map(ProjectSeedBlock::TrajectoryFeedforward);
    }
    if kind == FEEDFORWARD_PID {
        return feedforward_seed::feedforward_seed(object).map(ProjectSeedBlock::Feedforward);
    }
    if kind == CASCADE_PID {
        return cascade_seed::cascade_seed(object).map(ProjectSeedBlock::Cascade);
    }
    if kind == SMITH_PID {
        return smith_seed::smith_seed(object).map(ProjectSeedBlock::Smith);
    }
    if kind == DECENTRALIZED_PID {
        return group_seed::decentralized(object).map(|()| ProjectSeedBlock::Decentralized);
    }
    if kind == DECOUPLED_PID {
        return group_seed::decoupled(object).map(ProjectSeedBlock::Decoupled);
    }
    if kind != PARALLEL_PID {
        return Err(format!(
            "controller.project.seed.kind {kind} is not a seed kind"
        ));
    }
    let mut fields = object.clone();
    fields.remove("kind");
    let seed: ParallelPidSeed = crate::json::decode_value(Value::Object(fields))
        .map_err(|error| format!("controller.project.seed: {error}"))?;
    check_family_terms(&seed, seed.family).map_err(|refusal| refusal.0)?;
    if has_repeat(&seed_symbols(&seed)) {
        return Err("controller.project.seed names one variable for two gains".to_owned());
    }
    Ok(ProjectSeedBlock::ParallelPid(seed))
}

/// The fields of a `trajectory-feedforward-pid` seed other than its kind (C11). Exactly these six keys are read: a seventh is refused.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TrajectoryFields {
    family: Structure,
    proportional_symbol: String,
    integral_symbol: String,
    derivative_symbol: String,
    rate_symbol: String,
    acceleration_symbol: String,
}

/// A trajectory seed (C11): its PID variables, all named, and its rate and acceleration feedforward variables. The five variables are
/// distinct (`validTrajectorySeed`).
fn trajectory_seed(object: &Map<String, Value>) -> Result<TrajectoryFeedforwardSeed, String> {
    let mut fields = object.clone();
    fields.remove("kind");
    let fields: TrajectoryFields = crate::json::decode_value(Value::Object(fields))
        .map_err(|error| format!("controller.project.seed: {error}"))?;
    let seed = TrajectoryFeedforwardSeed {
        pid: ParallelPidSeed {
            family: fields.family,
            proportional_symbol: fields.proportional_symbol,
            integral_symbol: Some(fields.integral_symbol),
            derivative_symbol: Some(fields.derivative_symbol),
        },
        rate_symbol: fields.rate_symbol,
        acceleration_symbol: fields.acceleration_symbol,
    };
    if has_repeat(&seed.variables()) {
        return Err("controller.project.seed names one variable for two gains".to_owned());
    }
    Ok(seed)
}

fn axes(object: &Map<String, Value>) -> Result<Vec<ProjectAxis>, String> {
    let items = list_field(object, "axes")?;
    if items.len() > MAXIMUM_AXES {
        return Err(format!(
            "controller.project.axes holds at most {MAXIMUM_AXES} axes"
        ));
    }
    let mut symbols = BTreeSet::new();
    let mut axes = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let entry = item
            .as_object()
            .ok_or_else(|| format!("controller.project.axes[{index}] must be an object"))?;
        let symbol = text_field(entry, "symbol")?;
        if !symbols.insert(symbol) {
            return Err(format!(
                "controller.project.axes[{index}].symbol {symbol} is the symbol of another axis"
            ));
        }
        let values = list_field(entry, "values")?;
        if values.is_empty() || values.len() > MAXIMUM_AXIS_VALUES {
            return Err(format!(
                "controller.project.axes[{index}].values holds 1 to {MAXIMUM_AXIS_VALUES} values"
            ));
        }
        let values = values
            .iter()
            .map(|value| {
                value
                    .as_f64()
                    .filter(|number| number.is_finite())
                    .ok_or_else(|| {
                        format!("controller.project.axes[{index}].values must be finite numbers")
                    })
            })
            .collect::<Result<Vec<f64>, String>>()?;
        axes.push(ProjectAxis {
            symbol: symbol.to_owned(),
            values,
        });
    }
    Ok(axes)
}

fn components(object: &Map<String, Value>) -> Result<Vec<ProjectTuningComponent>, String> {
    let Some(value) = object.get("components") else {
        return Ok(Vec::new());
    };
    if value
        .as_array()
        .is_none_or(|items| items.len() > MAXIMUM_COMPONENTS)
    {
        return Err(format!(
            "controller.project.components must be a list of at most {MAXIMUM_COMPONENTS} components"
        ));
    }
    crate::json::decode_value(value.clone())
        .map_err(|error| format!("controller.project.components: {error}"))
}

fn model_parameters(object: &Map<String, Value>) -> Result<BTreeMap<String, ModelSource>, String> {
    let Some(value) = object.get("modelParameters") else {
        return Ok(BTreeMap::new());
    };
    let entries = value
        .as_object()
        .filter(|entries| entries.len() <= MAXIMUM_MODEL_PARAMETERS)
        .ok_or_else(|| {
            format!(
                "controller.project.modelParameters must be an object of at most {MAXIMUM_MODEL_PARAMETERS} symbols"
            )
        })?;
    entries
        .iter()
        .map(|(symbol, source)| -> Result<(String, ModelSource), String> {
            let word = source
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let source = model_source(word).ok_or_else(|| {
                format!("controller.project.modelParameters.{symbol} has no model source that the engine reads")
            })?;
            Ok((symbol.clone(), source))
        })
        .collect()
}

fn required_scenario_ids(object: &Map<String, Value>) -> Result<Vec<String>, String> {
    let Some(value) = object.get("requiredScenarioIds") else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .filter(|items| items.len() <= MAXIMUM_REQUIRED_SCENARIOS)
        .ok_or_else(|| {
            format!(
                "controller.project.requiredScenarioIds must be a list of at most {MAXIMUM_REQUIRED_SCENARIOS} identifiers"
            )
        })?;
    items
        .iter()
        .map(|item| {
            item.as_str()
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    "controller.project.requiredScenarioIds must hold non-empty texts".to_owned()
                })
        })
        .collect()
}

fn additional_modes(object: &Map<String, Value>) -> Result<Vec<AdditionalMode>, String> {
    let Some(value) = object.get("additionalModes") else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .filter(|items| items.len() <= MAXIMUM_ADDITIONAL_MODES)
        .ok_or_else(|| {
            format!(
                "controller.project.additionalModes must be a list of at most {MAXIMUM_ADDITIONAL_MODES} modes"
            )
        })?;
    items.iter().map(additional_mode).collect()
}

fn additional_mode(item: &Value) -> Result<AdditionalMode, String> {
    let fields = item
        .as_object()
        .filter(|fields| fields.len() == 2)
        .ok_or("controller.project.additionalModes entries hold a symbol and a manualValue")?;
    let symbol = fields
        .get("symbol")
        .and_then(Value::as_str)
        .filter(|symbol| !symbol.trim().is_empty())
        .ok_or("controller.project.additionalModes symbol must be a non-empty text")?;
    let manual_value = fields
        .get("manualValue")
        .and_then(Value::as_bool)
        .ok_or("controller.project.additionalModes manualValue must be true or false")?;
    Ok(AdditionalMode {
        symbol: symbol.to_owned(),
        manual_value,
    })
}

/// The source that a model parameter's `kind` names (C0; `ProjectModelParameter`), or `None` for a kind the engine does not read.
fn model_source(word: &str) -> Option<ModelSource> {
    Some(match word {
        "sample-time" => ModelSource::SampleTime,
        "plant-delay" => ModelSource::PlantDelay,
        "plant-gain" => ModelSource::PlantGain,
        "inverse-plant-gain" => ModelSource::InversePlantGain,
        "plant-time-constant" => ModelSource::PlantTimeConstant,
        _ => return None,
    })
}

fn model_source_word(source: ModelSource) -> &'static str {
    match source {
        ModelSource::SampleTime => "sample-time",
        ModelSource::PlantDelay => "plant-delay",
        ModelSource::PlantGain => "plant-gain",
        ModelSource::InversePlantGain => "inverse-plant-gain",
        ModelSource::PlantTimeConstant => "plant-time-constant",
    }
}

#[cfg(test)]
#[path = "project_trajectory_tests.rs"]
mod trajectory_tests;

#[cfg(test)]
#[path = "project_cascade_tests.rs"]
mod cascade_tests;

#[cfg(test)]
#[path = "project_smith_tests.rs"]
mod smith_tests;
