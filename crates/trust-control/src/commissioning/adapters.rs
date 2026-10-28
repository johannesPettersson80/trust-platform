//! The controller adapters of Control Studio (specification 35, C0, C1, C2 and C13; `controllerAdapters.ts`): which adapter a
//! loop's controller has, the structures it can realize, the transfer and the initialization it needs, the commissioning profile
//! of its declared states, and the parameter values that a set of gains writes. Tune and the commissioning flows call these
//! decisions and run the runtime themselves.
//!
//! Six adapters are ported: the shipped `Control.PID` (C1, with the library's `control-pid-v1` states), a program's parallel PID
//! record (C2, `parallel-pid-v1`), the connected P (C13), a program's own PI, PD or PID law (C2) whose seed names its gain
//! variables and whose declared states start its integral and derivative terms, a state-feedback law (C4, its coefficients `K`, `N`
//! and `L`), and a finite search over a program's law (C5, its axes as coefficients). A program's own P law is the connected P on its
//! seed's variable. A loop that uses another family is refused, never guessed at: ON/OFF (C3), cascade (C6), feedforward
//! (C8), Smith (C9), gain scheduling (C10), trajectory (C11) and experiment loops (C12) have no adapter here, and channels
//! (multivariable loops) are parked with stage V7. `projectController.ts` is a parked feature for every seed but `parallel-pid`, and
//! is not ported.

use std::collections::BTreeMap;

use super::cascade_adapter::cascade_values;
use super::coefficient_record::coefficient_values;
use super::declared_initialization::{
    declared_commissioning_profile, declared_initialization_plan, declared_transfer_binding,
};
use super::group_adapter::{
    check_group, group_gains_refused, group_needs_project, group_transfer_binding,
};
use super::initial_state::ControllerInitialization;
use super::initialization_plan::{
    pid_initialization_plan, proportional_initialization_plan, ParallelPidStartPlan, StartPlan,
};
use super::library_pid::library_pid_initialization;
use super::on_off_adapter;
use super::pid_initialization::pid_initialization_adapter;
use super::profile::CommissioningProfile;
use super::project_law::{has_pid_terms, served_law};
use super::search_adapter::{self, search_law};
use super::smith_adapter;
use super::state_adapter::{self, family_label, state_law, StateLaw};
use super::texts;
use super::transfer::{
    pid_transfer_binding, proportional_parameter, proportional_transfer_binding, TransferBinding,
};
use super::transfer_tracking::{parallel_pid_tracking, proportional_tracking, TransferTracking};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    ControllerLaw, ControllerProject, InitialValue, LoopDefinition, Parameter, ParameterBinding,
    PidControllerGroup, PidInitialization, PidLaw, RealType,
};
use crate::model::GeneralModel;
use crate::tuning::cascade_candidate::served_cascade;
use crate::tuning::project_seed::{
    seed_values, CascadePidSeed, FeedforwardPidSeed, ParallelPidSeed, SmithPidSeed, SmithTerms,
    TrajectoryFeedforwardSeed,
};
use crate::tuning::proportional::GainBounds;
use crate::tuning::settings_pins::single_precision;
use crate::tuning::structure::{is_exactly_zero, ParallelGains, Structure};
use crate::validation::engineer_verification_plan::EngineerRequirements;

/// The structures a PID adapter can realize (C0, "Families and zero terms").
const PID_FAMILIES: [Structure; 4] = [Structure::P, Structure::PI, Structure::PD, Structure::PID];

/// The structures the connected P can realize (C13): the proportional one only.
const PROPORTIONAL_FAMILIES: [Structure; 1] = [Structure::P];

/// The label of the shipped `Control.PID`'s commissioning profile.
const CONTROL_PID_LABEL: &str = "Control.PID";

/// The label of a switching controller's commissioning profile (C3).
const ON_OFF_LABEL: &str = "ON/OFF";

/// The refusal of a state or search law's values asked for as gains: such a law writes its coefficients (C4, C5).
const COEFFICIENTS_NOT_GAINS: &str =
    "A state or search law writes its coefficients, not gains (C4, C5).";
/// The refusal of a coefficient write asked of a `Control.PID` or a program's own law, which writes its gains (C0).
const COEFFICIENTS_NOT_LAW: &str =
    "Only a state or search law writes coefficients; this law writes its gains (C0).";

/// The gains that an adapter maps (`ControllerGains`, C0). A term that the family does not use is written as an explicit zero,
/// so an absent integral or derivative gain is zero. The optional terms are written only where the law declares them.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ControllerGains {
    /// The proportional gain `Kp`. Every PID record and the connected P require it.
    pub proportional_gain: Option<f64>,
    /// The integral gain `Ki`.
    pub integral_gain: Option<f64>,
    /// The derivative gain `Kd`.
    pub derivative_gain: Option<f64>,
    /// The back-calculation tracking gain `1/Tt` (C0), in 1/s.
    pub back_calculation_gain: Option<f64>,
    /// The proportional setpoint weight `β` (C0).
    pub proportional_setpoint_weight: Option<f64>,
    /// The derivative setpoint weight `γ` (C0).
    pub derivative_setpoint_weight: Option<f64>,
    /// The measurement filter time `Tf` in seconds (C0).
    pub measurement_filter_seconds: Option<f64>,
    /// The rate feedforward gain `K_v` of a trajectory-following law (C11), written only where the law's seed names its variable.
    pub rate_gain: Option<f64>,
    /// The acceleration feedforward gain `K_a` of a trajectory-following law (C11), written only where the law's seed names its variable.
    pub acceleration_gain: Option<f64>,
    /// The measured-disturbance feedforward gain `K_ff` (C8), written only where the law's seed names its variable.
    pub feedforward_gain: Option<f64>,
    /// The lead time `T_lead` in seconds of a measured-disturbance feedforward (C8), written with the lag time where the seed names both.
    pub lead_seconds: Option<f64>,
    /// The lag time `T_lag` in seconds of a measured-disturbance feedforward (C8), written with the lead time where the seed names both.
    pub lag_seconds: Option<f64>,
    /// The hysteresis of a switching controller (C3), written under its symbol.
    pub on_off_hysteresis: Option<f64>,
    /// The minimum ON time of a switching controller, in seconds (C3), written under its symbol.
    pub on_off_minimum_on_seconds: Option<f64>,
    /// The minimum OFF time of a switching controller, in seconds (C3), written under its symbol.
    pub on_off_minimum_off_seconds: Option<f64>,
    /// The Smith predictor's model values (C9): the gain, the time constant and the delay, and the operating point when the seed names the
    /// library block. Written only where the law's seed names them.
    pub smith: Option<SmithTerms>,
    /// The outer controller's PI of a cascade (C6): written to its own record beside the inner controller's gains, which
    /// `proportional_gain`, `integral_gain` and `derivative_gain` hold. Written only for a cascade.
    pub outer_pid: Option<ParallelGains>,
}

/// Which adapter a loop's controller has (`controllerAdapter`): the shipped `Control.PID` with its states, a parallel PID with its
/// `parallel-pid-v1` contract, the connected P, or a program's own PI, PD or PID law with its declared states.
#[derive(Debug, Clone, PartialEq)]
pub enum AdapterKind {
    /// The shipped `Control.PID` (C1): its retained states, initialized by the library's `control-pid-v1` declaration.
    ControlPid(ControllerInitialization),
    /// A program's parallel PID (C2): its integral and derivative states, declared by `parallel-pid-v1`.
    ParallelPid,
    /// The connected P (C13): the proportional gain only, with its integral and derivative gains written as zero. A program's own P
    /// law (C2) is one too, on the variable its seed names.
    ConnectedP,
    /// A program's own PI, PD or PID law (C2): its seed names the gain variables, and its declared states start the integral and
    /// derivative terms as Validate does. The block is the law's controller project (the family its profile shows, its declared
    /// states, and the families its seed offers); `seed` is the block's `parallel-pid` seed.
    ProjectPid {
        /// The controller's `project` block, as the loop declares it (boxed: it is far larger than the other kinds).
        project: Box<ControllerProject>,
        /// The seed that names the law's gain variables.
        seed: ParallelPidSeed,
    },
    /// A cascade (C6): the inner and the outer PI, each written to its own record, with the outer controller's mode switched with the
    /// inner's. Its block is the cascade's controller project, and `seed` names both controllers' variables.
    Cascade {
        /// The controller's `project` block, as the loop declares it.
        project: Box<ControllerProject>,
        /// The seed that names both controllers' gain variables.
        seed: CascadePidSeed,
    },
    /// A state-feedback law (C4): its coefficients are the gains `K`, the reference gains `N` and, for LQG, the observer gains `L`, and its
    /// declared states start at an Auto entry. Boxed: the decoded law is large.
    State(Box<StateLaw>),
    /// A switching (ON/OFF) controller (C3): its three settings are written through the bound parameters, and its declared states
    /// (the latch and the dwell timer) are the controller's initialization.
    OnOff(ControllerInitialization),
    /// A finite search over a program's law (C5): its tuning axes are the coefficient variables, and its declared states start at an
    /// Auto entry as a program's own PID's do. It has no seed, and its Auto entry needs a reviewed tolerance.
    Search {
        /// The controller's `project` block, in the search-law form.
        project: Box<ControllerProject>,
    },
    /// A PID group (C7): its members are `Control.PID` loops, each on one input and one output of the loop, and one transaction writes
    /// them together. `project` owns every member's states, and `group` names the members and their channels.
    Group {
        /// The controller's `project` block, which declares every member's states.
        project: Box<ControllerProject>,
        /// The group's members, in the order that the transfer holds them.
        group: Box<PidControllerGroup>,
    },
}

/// The start plan of an adapter: one plan of declared states, or a parallel PID's plan with its integral and derivative states.
#[derive(Debug, Clone, PartialEq)]
pub enum AdapterStartPlan {
    /// The plan of a `Control.PID`'s declared states or of the connected P.
    Start(StartPlan),
    /// The plan of a parallel PID, with the states it starts.
    ParallelPid(ParallelPidStartPlan),
}

/// The adapter of one loop, bound to that loop (`controllerAdapter`).
#[derive(Debug, Clone, PartialEq)]
pub struct ControllerAdapter<'a> {
    loop_definition: &'a LoopDefinition,
    kind: AdapterKind,
}

/// The adapter of a loop's controller (`controllerAdapter`): the shipped `Control.PID` when its PID law declares `control-pid-v1`, a
/// parallel PID for any other PID law, a program's own PI, PD or PID law for a seed with an integral or a derivative variable, and
/// the connected P for a proportional law or a program's own P law.
///
/// # Errors
/// The loop uses a feature that no adapter here realizes; the library's initialization of a `Control.PID` does not fit the loop; the
/// loop has no proportional law that the connected P can use; or a program's own P law declares a law state, which a P law cannot
/// start, or an application state that is not a constant.
pub fn controller_adapter(
    loop_definition: &LoopDefinition,
) -> Result<ControllerAdapter<'_>, Failure> {
    let project_feature = loop_definition
        .controller
        .project
        .as_ref()
        .and_then(crate::loop_definition::ControllerProject::parked_feature);
    if let Some(feature) = project_feature
        .as_deref()
        .or_else(|| parked_feature(loop_definition))
    {
        return Err(Failure::new(
            &texts::adapter_parked_feature(feature),
            Cause::ControllerCapability,
            NextAction::None,
        ));
    }
    let kind = if let Some(group) = loop_definition.controller.group.as_ref() {
        // A group is checked before any other kind, as `controllerAdapter` checks it first: a group never falls through to a scalar PID.
        let project = loop_definition
            .controller
            .project
            .as_ref()
            .ok_or_else(group_needs_project)?;
        check_group(loop_definition, project, group)?;
        AdapterKind::Group {
            project: Box::new(project.clone()),
            group: Box::new(group.clone()),
        }
    } else if let (Some(ControllerLaw::Pid(_)), Some(_)) = (
        &loop_definition.controller.law,
        &loop_definition.parameter_binding,
    ) {
        match &loop_definition.controller.pid_initialization {
            Some(PidInitialization::ControlPid { .. }) => {
                AdapterKind::ControlPid(library_pid_initialization(loop_definition)?)
            }
            _ => AdapterKind::ParallelPid,
        }
    } else if let (Some(project), Some(seed)) = (
        loop_definition.controller.project.as_ref(),
        served_cascade(loop_definition),
    ) {
        AdapterKind::Cascade {
            project: Box::new(project.clone()),
            seed: seed.clone(),
        }
    } else if let Some(law) = served_law(loop_definition).filter(|law| has_pid_terms(law.seed)) {
        AdapterKind::ProjectPid {
            project: Box::new(law.project.clone()),
            seed: law.seed.clone(),
        }
    } else if let Some(law) = state_law(loop_definition)? {
        AdapterKind::State(Box::new(law))
    } else if let Some(project) = search_law(loop_definition) {
        AdapterKind::Search {
            project: Box::new(project.clone()),
        }
    } else if let Some(block) = loop_definition.controller.on_off.as_ref() {
        AdapterKind::OnOff(block.initialization.clone())
    } else {
        proportional_parameter(loop_definition)
            .map_err(|failure| unrecognised_type(loop_definition, &failure))?;
        connected_p_states(loop_definition)?;
        AdapterKind::ConnectedP
    };
    Ok(ControllerAdapter {
        loop_definition,
        kind,
    })
}

impl ControllerAdapter<'_> {
    /// The adapter's kind, which the commissioning flow uses to choose its review (C1, C2, C13).
    #[must_use]
    pub fn kind(&self) -> &AdapterKind {
        &self.kind
    }

    /// The structures the adapter can realize (`families`): the PID kinds offer P, PI, PD and PID, and the connected P offers P. A
    /// program's own law offers the structures its seed's variables allow (C0 "Families", C2).
    #[must_use]
    pub fn families(&self) -> Vec<Structure> {
        match &self.kind {
            AdapterKind::ConnectedP => PROPORTIONAL_FAMILIES.to_vec(),
            AdapterKind::ControlPid(_) | AdapterKind::ParallelPid => PID_FAMILIES.to_vec(),
            AdapterKind::ProjectPid { project, .. }
            | AdapterKind::Search { project }
            | AdapterKind::Cascade { project, .. } => project.offered_families(),
            AdapterKind::Group { .. } => vec![Structure::PI],
            AdapterKind::State(_) | AdapterKind::OnOff(_) => Vec::new(),
        }
    }

    /// The transfer binding of the adapter (`transferBinding`, A6.3): the references, the mode and the outputs that an Auto entry
    /// writes.
    ///
    /// # Errors
    /// The adapter's declared states or bindings do not fit the loop.
    pub fn transfer_binding(&self) -> Result<TransferBinding, Failure> {
        match &self.kind {
            AdapterKind::ControlPid(initialization) | AdapterKind::OnOff(initialization) => {
                declared_binding(self.loop_definition, initialization)
            }
            AdapterKind::ProjectPid { project, .. }
            | AdapterKind::Search { project }
            | AdapterKind::Cascade { project, .. } => {
                declared_binding(self.loop_definition, &project.initialization)
            }
            AdapterKind::State(law) => declared_binding(self.loop_definition, &law.initialization),
            AdapterKind::Group { project, group } => {
                group_transfer_binding(self.loop_definition, project, group)
            }
            AdapterKind::ParallelPid => pid_transfer_binding(self.loop_definition),
            AdapterKind::ConnectedP => proportional_transfer_binding(self.loop_definition),
        }
    }

    /// The tracking that a recovery to Manual holds (`recoveryTracking`, A6.2): the declared states of the library PID or of a
    /// program's own law, the parallel PID's states, or the proportional reference. The tracking is not initialized here.
    ///
    /// # Errors
    /// The adapter's bindings or its declared states do not fit the loop.
    pub fn recovery_tracking(&self, model: &GeneralModel) -> Result<TransferTracking, Failure> {
        match &self.kind {
            AdapterKind::ControlPid(initialization) => {
                let profile = declared_profile_of(
                    self.loop_definition,
                    model,
                    CONTROL_PID_LABEL,
                    initialization,
                    &BTreeMap::new(),
                )?;
                let binding = declared_binding(self.loop_definition, initialization)?;
                profile.tracking(&binding, false)
            }
            AdapterKind::OnOff(initialization) => {
                let profile = declared_profile_of(
                    self.loop_definition,
                    model,
                    ON_OFF_LABEL,
                    initialization,
                    &BTreeMap::new(),
                )?;
                let binding = declared_binding(self.loop_definition, initialization)?;
                profile.tracking(&binding, false)
            }
            AdapterKind::ProjectPid { project, .. } | AdapterKind::Cascade { project, .. } => {
                let profile = declared_profile_of(
                    self.loop_definition,
                    model,
                    &project.family,
                    &project.initialization,
                    &BTreeMap::new(),
                )?;
                let binding = declared_binding(self.loop_definition, &project.initialization)?;
                profile.tracking(&binding, false)
            }
            AdapterKind::Group { project, group } => {
                // The declared profile's own binding is the group's, so its tracking names every member's channels (C7).
                let profile = declared_profile_of(
                    self.loop_definition,
                    model,
                    &project.family,
                    &project.initialization,
                    &BTreeMap::new(),
                )?;
                let binding = group_transfer_binding(self.loop_definition, project, group)?;
                profile.tracking(&binding, false)
            }
            AdapterKind::State(law) => declared_tracking(
                self.loop_definition,
                model,
                family_label(law.binding.family),
                &law.initialization,
            ),
            AdapterKind::Search { project } => declared_tracking(
                self.loop_definition,
                model,
                &project.family,
                &project.initialization,
            ),
            AdapterKind::ParallelPid => {
                let binding = pid_transfer_binding(self.loop_definition)?;
                parallel_pid_tracking(self.loop_definition, &binding, false)
                    .map(TransferTracking::ParallelPid)
            }
            AdapterKind::ConnectedP => {
                let binding = proportional_transfer_binding(self.loop_definition)?;
                proportional_tracking(self.loop_definition, &binding, false)
                    .map(TransferTracking::Proportional)
            }
        }
    }

    /// The commissioning profile of the adapter's declared states (`declaredProfile`, C1, C2), at the given values. The shipped
    /// `Control.PID` and a program's own PI, PD or PID law declare one; the parallel PID and the connected P have none.
    ///
    /// # Errors
    /// The declared states do not fit the loop, or the profile does not hold at the operating point.
    pub fn declared_profile(
        &self,
        model: &GeneralModel,
        values: &BTreeMap<String, f64>,
    ) -> Result<Option<CommissioningProfile>, Failure> {
        match &self.kind {
            AdapterKind::ControlPid(initialization) => declared_profile_of(
                self.loop_definition,
                model,
                CONTROL_PID_LABEL,
                initialization,
                values,
            )
            .map(Some),
            AdapterKind::ProjectPid { project, .. }
            | AdapterKind::Search { project }
            | AdapterKind::Cascade { project, .. }
            | AdapterKind::Group { project, .. } => declared_profile_of(
                self.loop_definition,
                model,
                &project.family,
                &project.initialization,
                values,
            )
            .map(Some),
            AdapterKind::State(law) => declared_profile_of(
                self.loop_definition,
                model,
                family_label(law.binding.family),
                &law.initialization,
                values,
            )
            .map(Some),
            AdapterKind::OnOff(initialization) => declared_profile_of(
                self.loop_definition,
                model,
                ON_OFF_LABEL,
                initialization,
                values,
            )
            .map(Some),
            AdapterKind::ParallelPid | AdapterKind::ConnectedP => Ok(None),
        }
    }

    /// Checks that the adapter can start an Auto entry (`checkInitialization`, C0): the declared states and their binding for a
    /// `Control.PID` or a program's own law, the explicit contract for a parallel PID, and the gain parameter for the connected P.
    ///
    /// # Errors
    /// The adapter's initialization is missing or does not fit the loop.
    pub fn check_initialization(&self) -> Result<(), Failure> {
        match &self.kind {
            AdapterKind::ControlPid(initialization) => {
                declared_binding(self.loop_definition, initialization).map(|_| ())
            }
            AdapterKind::ParallelPid => {
                pid_initialization_adapter(self.loop_definition).map(|_| ())
            }
            AdapterKind::ConnectedP => proportional_parameter(self.loop_definition).map(|_| ()),
            AdapterKind::Group { project, group } => {
                check_group(self.loop_definition, project, group)?;
                declared_binding(self.loop_definition, &project.initialization).map(|_| ())
            }
            AdapterKind::ProjectPid { project, .. }
            | AdapterKind::Search { project }
            | AdapterKind::Cascade { project, .. } => {
                declared_binding(self.loop_definition, &project.initialization).map(|_| ())
            }
            AdapterKind::OnOff(initialization) => {
                declared_binding(self.loop_definition, initialization).map(|_| ())
            }
            AdapterKind::State(law) => {
                declared_binding(self.loop_definition, &law.initialization).map(|_| ())
            }
        }
    }

    /// The parameter values that a set of gains writes (`materialize`, C0): the gains as the adapter's record holds them, each
    /// checked against its declared binding, with the observed values kept.
    ///
    /// # Errors
    /// The proportional gain is missing; a gain or a term is outside its binding; the declared law lacks a term that a non-neutral
    /// setting needs; or a `REAL` value changes under single precision.
    pub fn materialize(
        &self,
        gains: &ControllerGains,
        observed: &BTreeMap<String, f64>,
    ) -> Result<BTreeMap<String, f64>, Failure> {
        match &self.kind {
            AdapterKind::ControlPid(_) => pid_record_values(
                self.loop_definition,
                gains,
                observed,
                &texts::adapter_control_pid_gain_required(),
            ),
            AdapterKind::ParallelPid => pid_record_values(
                self.loop_definition,
                gains,
                observed,
                &texts::adapter_pid_record_gain_required(),
            ),
            AdapterKind::ConnectedP => connected_p_values(self.loop_definition, gains, observed),
            AdapterKind::Group { .. } => Err(group_gains_refused()),
            AdapterKind::State(_) | AdapterKind::Search { .. } => Err(Failure::new(
                COEFFICIENTS_NOT_GAINS,
                Cause::Implementation,
                NextAction::None,
            )),
            AdapterKind::Cascade { seed, .. } => cascade_values(
                self.loop_definition.parameter_binding.as_ref(),
                seed,
                gains,
                observed,
            ),
            AdapterKind::OnOff(_) => {
                on_off_adapter::materialize(self.loop_definition, gains, observed)
            }
            AdapterKind::ProjectPid { project, seed } => project_pid_values(
                self.loop_definition,
                seed,
                project.trajectory(),
                project.feedforward(),
                project.smith(),
                gains,
                observed,
            ),
        }
    }

    /// The parameter values that a state or search law's coefficients write (C4, C5): each coefficient checked against its binding, with
    /// the observed values kept. A `Control.PID` or a program's own law writes its gains with [`ControllerAdapter::materialize`].
    ///
    /// # Errors
    /// The law is not a state or search law; or a coefficient is not a parameter of the record, or is outside its bounds once rounded.
    pub fn materialize_coefficients(
        &self,
        coefficients: &BTreeMap<String, f64>,
        observed: &BTreeMap<String, f64>,
    ) -> Result<BTreeMap<String, f64>, Failure> {
        match &self.kind {
            AdapterKind::State(_) | AdapterKind::Search { .. } | AdapterKind::Group { .. } => {
                coefficient_values(self.loop_definition, coefficients, observed)
            }
            AdapterKind::ControlPid(_)
            | AdapterKind::ParallelPid
            | AdapterKind::ConnectedP
            | AdapterKind::ProjectPid { .. }
            | AdapterKind::Cascade { .. }
            | AdapterKind::OnOff(_) => Err(Failure::new(
                COEFFICIENTS_NOT_LAW,
                Cause::Implementation,
                NextAction::None,
            )),
        }
    }

    /// The start plan of the adapter (`initialization`, A6.3 and A6.4): the values an Auto entry starts the controller from, and the
    /// run that proves them. The measured output noise sets the Auto-entry allowance of the plan (A6.4).
    ///
    /// # Errors
    /// The adapter's bindings or declared states do not fit the loop, or the operating point does not allow a start.
    pub fn initialization_plan(
        &self,
        model: &GeneralModel,
        task: &EngineerRequirements,
        values: &BTreeMap<String, f64>,
        process_noise: Option<f64>,
    ) -> Result<AdapterStartPlan, Failure> {
        match &self.kind {
            AdapterKind::ControlPid(initialization) | AdapterKind::OnOff(initialization) => {
                declared_binding(self.loop_definition, initialization)?;
                declared_initialization_plan(
                    self.loop_definition,
                    model,
                    task,
                    values,
                    initialization,
                    process_noise,
                )
                .map(AdapterStartPlan::Start)
            }
            AdapterKind::ProjectPid { project, .. }
            | AdapterKind::Cascade { project, .. }
            | AdapterKind::Group { project, .. } => {
                declared_binding(self.loop_definition, &project.initialization)?;
                declared_initialization_plan(
                    self.loop_definition,
                    model,
                    task,
                    values,
                    &project.initialization,
                    process_noise,
                )
                .map(AdapterStartPlan::Start)
            }
            AdapterKind::State(law) => state_adapter::initialization_plan(
                self.loop_definition,
                model,
                task,
                values,
                process_noise,
                law,
            )
            .map(AdapterStartPlan::Start),
            AdapterKind::Search { project } => search_adapter::initialization_plan(
                self.loop_definition,
                model,
                task,
                values,
                process_noise,
                project,
            )
            .map(AdapterStartPlan::Start),
            AdapterKind::ParallelPid => {
                pid_initialization_plan(self.loop_definition, model, task, values, process_noise)
                    .map(AdapterStartPlan::ParallelPid)
            }
            AdapterKind::ConnectedP => proportional_initialization_plan(
                self.loop_definition,
                model,
                task,
                values,
                process_noise,
            )
            .map(AdapterStartPlan::Start),
        }
    }
}

/// The first feature of a loop that no adapter here realizes (C0 "Families", C3 to C12): such a loop is refused, not guessed at. A
/// program's own law is read first by `ControllerProject::parked_feature`, the one gate of C2, in `controller_adapter`.
fn parked_feature(loop_definition: &LoopDefinition) -> Option<&'static str> {
    let controller = &loop_definition.controller;
    // A PID group's channels are its members (C7), so they are not parked channels.
    if loop_definition.channels.is_some()
        && controller.group.is_none()
        && !loop_definition.is_feedforward_channels()
        && !loop_definition.is_on_off_channels()
        && !loop_definition.is_cascade_channels()
        && controller.state_feedback.is_none()
    {
        Some("channels")
    } else if loop_definition.declares_parked_analysis() {
        Some("a declared analysis")
    } else if loop_definition.cascade.is_some() {
        Some("a cascade")
    } else {
        None
    }
}

/// The reason of a failure, as the whole text the engineer reads (A11.1).
fn full_text(failure: &Failure) -> &str {
    failure.detail.as_deref().unwrap_or(&failure.message)
}

/// The refusal of a loop that no adapter recognises: it names the controller's type and the reason (C0, "Missing adapters").
fn unrecognised_type(loop_definition: &LoopDefinition, failure: &Failure) -> Failure {
    Failure::new(
        &texts::adapter_unrecognised_type(
            &loop_definition.controller.type_name,
            full_text(failure),
        ),
        Cause::ControllerCapability,
        NextAction::OpenLoopSetup,
    )
}

/// The transfer binding of a declared initialization (`requiredInitialization`, C1, C2): the declared states must bind the loop's
/// references, mode and manual command. A refusal keeps its reason and offers the choice of a method (A11.2).
fn declared_binding(
    loop_definition: &LoopDefinition,
    initialization: &ControllerInitialization,
) -> Result<TransferBinding, Failure> {
    declared_transfer_binding(loop_definition, initialization).map_err(|failure| {
        Failure::new(
            full_text(&failure),
            Cause::ControllerCapability,
            NextAction::ChooseMethod,
        )
    })
}

/// The tracking that a recovery to Manual holds for a declared law (`recoveryTracking`, C4, C5): the profile of its declared states and
/// their transfer binding, without the states being initialized.
fn declared_tracking(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    label: &str,
    initialization: &ControllerInitialization,
) -> Result<TransferTracking, Failure> {
    let profile = declared_profile_of(
        loop_definition,
        model,
        label,
        initialization,
        &BTreeMap::new(),
    )?;
    let binding = declared_binding(loop_definition, initialization)?;
    profile.tracking(&binding, false)
}

/// The commissioning profile of a declared initialization (`profile`, C1, C2), labelled with the controller's name, at the given
/// values.
fn declared_profile_of(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    label: &str,
    initialization: &ControllerInitialization,
    values: &BTreeMap<String, f64>,
) -> Result<CommissioningProfile, Failure> {
    declared_binding(loop_definition, initialization)?;
    declared_commissioning_profile(loop_definition, model, initialization, label, values)
}

/// The declared states of a program's own P law (C13, C1): its application states are written at the Auto entry, at the same boundary
/// as the reference, so each must take a constant initial value. A law state (an integral or a derivative term, which a P law cannot
/// start) is refused by its name. A loop that is not a program's own law passes.
fn connected_p_states(loop_definition: &LoopDefinition) -> Result<(), Failure> {
    let Some(law) = served_law(loop_definition) else {
        return Ok(());
    };
    for state in law.project.initialization.states() {
        if !law.project.is_application_state(&state.symbol) {
            return Err(Failure::new(
                &texts::project_p_law_state(&state.symbol),
                Cause::ControllerCapability,
                NextAction::OpenLoopSetup,
            ));
        }
        if !matches!(state.initial, InitialValue::Constant(_)) {
            return Err(Failure::new(
                &texts::project_p_application_formula(&state.symbol),
                Cause::ControllerCapability,
                NextAction::OpenLoopSetup,
            ));
        }
    }
    Ok(())
}

/// One term of a PID record: the parameter it writes, whether the parameter is a gain (bounded by its magnitude, N6.4), and the value.
pub(super) struct Term<'a> {
    pub(super) symbol: &'a str,
    pub(super) gain: bool,
    pub(super) value: f64,
}

/// The terms of a PID law's record (`materializePidGainRecord`, C0, C1, C2): the three gains, then each term that the law declares.
/// A term the law does not declare must be neutral: a setpoint weight of one, and a derivative weight and a filter time of zero.
///
/// # Errors
/// The back-calculation gain has no tracking term in the law, or a non-neutral weight or filter time has no declared term.
fn pid_terms<'a>(
    law: &'a PidLaw,
    gains: &ControllerGains,
    proportional: f64,
) -> Result<Vec<Term<'a>>, Failure> {
    let mut terms = vec![
        Term {
            symbol: law.proportional_gain_symbol.as_str(),
            gain: true,
            value: proportional,
        },
        Term {
            symbol: law.integral_gain_symbol.as_str(),
            gain: true,
            value: gains.integral_gain.unwrap_or(0.0),
        },
        Term {
            symbol: law.derivative_gain_symbol.as_str(),
            gain: true,
            value: gains.derivative_gain.unwrap_or(0.0),
        },
    ];
    if let Some(back_calculation) = gains.back_calculation_gain {
        let Some(anti_windup) = &law.anti_windup else {
            return Err(Failure::new(
                &texts::parameter_anti_windup_required(),
                Cause::ControllerCapability,
                NextAction::OpenLoopSetup,
            ));
        };
        terms.push(Term {
            symbol: anti_windup.tracking_gain_symbol.as_str(),
            gain: false,
            value: back_calculation,
        });
    }
    if let Some(symbol) = &law.proportional_setpoint_weight_symbol {
        terms.push(Term {
            symbol: symbol.as_str(),
            gain: false,
            value: gains.proportional_setpoint_weight.unwrap_or(1.0),
        });
    } else if gains
        .proportional_setpoint_weight
        .is_some_and(|weight| !is_one(weight))
    {
        return Err(Failure::new(
            &texts::parameter_setpoint_weight_required(),
            Cause::ControllerCapability,
            NextAction::OpenLoopSetup,
        ));
    }
    if let Some(symbol) = &law.derivative_setpoint_weight_symbol {
        terms.push(Term {
            symbol: symbol.as_str(),
            gain: false,
            value: gains.derivative_setpoint_weight.unwrap_or(0.0),
        });
    } else if gains
        .derivative_setpoint_weight
        .is_some_and(|weight| !is_exactly_zero(weight))
    {
        return Err(Failure::new(
            &texts::parameter_derivative_weight_required(),
            Cause::ControllerCapability,
            NextAction::OpenLoopSetup,
        ));
    }
    if let Some(symbol) = &law.measurement_filter_time_symbol {
        terms.push(Term {
            symbol: symbol.as_str(),
            gain: false,
            value: gains.measurement_filter_seconds.unwrap_or(0.0),
        });
    } else if gains
        .measurement_filter_seconds
        .is_some_and(|time| !is_exactly_zero(time))
    {
        return Err(Failure::new(
            &texts::parameter_filter_time_required(),
            Cause::ControllerCapability,
            NextAction::OpenLoopSetup,
        ));
    }
    Ok(terms)
}

/// The parameter values of a PID law (`materializePidGainRecord`, C0, C1, C2): the gains and terms of its record, each written to its
/// bound parameter, with the observed values kept. The proportional gain is required; `missing_gain` is the sentence of its absence,
/// which names the adapter.
///
/// # Errors
/// The gain is missing; the law or its parameter binding is absent; or a term is not a declared parameter or is outside its binding.
fn pid_record_values(
    loop_definition: &LoopDefinition,
    gains: &ControllerGains,
    observed: &BTreeMap<String, f64>,
    missing_gain: &str,
) -> Result<BTreeMap<String, f64>, Failure> {
    let Some(proportional) = gains.proportional_gain else {
        return Err(Failure::new(
            missing_gain,
            Cause::Settings,
            NextAction::Tune,
        ));
    };
    let (Some(ControllerLaw::Pid(law)), Some(binding)) = (
        &loop_definition.controller.law,
        &loop_definition.parameter_binding,
    ) else {
        return Err(Failure::new(
            &texts::parameter_pid_record_required(),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ));
    };
    let terms = pid_terms(law, gains, proportional)?;
    bound_values(binding, &terms, observed)
}

/// The values of a PID record's terms within their bindings (`materializePidGainRecord`, C0; N6.4): a gain is bounded by its
/// magnitude, and every other term by its signed bounds. A `REAL` value is rounded to single precision, and the rounded value is
/// checked again.
///
/// # Errors
/// A term is not a declared parameter, or its value or its rounded value is outside the binding.
pub(super) fn bound_values(
    binding: &ParameterBinding,
    terms: &[Term<'_>],
    observed: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    let mut values = observed.clone();
    for term in terms {
        let Some(parameter) = binding
            .parameters
            .iter()
            .find(|parameter| parameter.symbol == term.symbol)
        else {
            return Err(Failure::new(
                &texts::parameter_not_declared(term.symbol),
                Cause::Settings,
                NextAction::Tune,
            ));
        };
        let bounds = GainBounds {
            minimum: parameter.minimum,
            maximum: parameter.maximum,
        };
        if !term.value.is_finite() || !within(term.value, bounds, term.gain) {
            return Err(outside_binding(term, term.value, parameter, false));
        }
        let realized = if matches!(parameter.iec_type, RealType::Real) {
            single_precision(term.value)
        } else {
            term.value
        };
        if !realized.is_finite() || !within(realized, bounds, term.gain) {
            return Err(outside_binding(term, realized, parameter, true));
        }
        values.insert(term.symbol.to_owned(), realized);
    }
    Ok(values)
}

/// The refusal of a term whose value, or whose single-precision value, is outside its binding (A11.1): the value and the bounds it
/// is checked against.
fn outside_binding(term: &Term<'_>, value: f64, parameter: &Parameter, realized: bool) -> Failure {
    let bounds = GainBounds {
        minimum: parameter.minimum,
        maximum: parameter.maximum,
    };
    let (lowest, highest) = displayed_bounds(bounds, term.gain);
    let sentence = if realized {
        texts::parameter_realized_outside_binding(
            term.symbol,
            value,
            lowest,
            highest,
            &parameter.unit,
            term.gain,
        )
    } else {
        texts::parameter_outside_binding(
            term.symbol,
            value,
            lowest,
            highest,
            &parameter.unit,
            term.gain,
        )
    };
    Failure::new(&sentence, Cause::Settings, NextAction::Tune)
}

/// The value of the connected P (`materialize`, C13): the proportional gain only, rounded as its `REAL` or `LREAL` type holds it,
/// and bounded by its magnitude (N6.4). A nonzero integral or derivative gain is refused (C13).
///
/// # Errors
/// The proportional gain is missing; an integral or derivative gain is nonzero; or the realized gain is outside its bounds.
fn connected_p_values(
    loop_definition: &LoopDefinition,
    gains: &ControllerGains,
    observed: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    let Some(proportional) = gains.proportional_gain else {
        return Err(Failure::new(
            &texts::adapter_proportional_gain_required(),
            Cause::Settings,
            NextAction::Tune,
        ));
    };
    if truthy(gains.integral_gain) || truthy(gains.derivative_gain) {
        return Err(Failure::new(
            &texts::adapter_proportional_only(),
            Cause::ControllerCapability,
            NextAction::ChooseMethod,
        ));
    }
    let parameter = proportional_parameter(loop_definition)?;
    let value = if matches!(parameter.iec_type, RealType::Real) {
        single_precision(proportional)
    } else {
        proportional
    };
    let bounds = GainBounds {
        minimum: parameter.minimum,
        maximum: parameter.maximum,
    };
    if !value.is_finite() || !within(value, bounds, true) {
        let (lowest, highest) = bounds.magnitude_range();
        return Err(Failure::new(
            &texts::adapter_proportional_outside_bounds(value, lowest, highest, &parameter.unit),
            Cause::Settings,
            NextAction::Tune,
        ));
    }
    let mut values = observed.clone();
    values.insert(parameter.symbol.clone(), value);
    Ok(values)
}

/// The values that a program's own PI, PD or PID law writes (`materialize` of the project law, C2): the gains its seed names, as
/// `seed_values` gives them, each checked against its binding by magnitude (N6.4) and rounded as its `REAL` type holds it. A term
/// the seed names no variable for must be zero, and a term the law cannot hold must be neutral.
///
/// # Errors
/// The proportional gain is missing; a term the law cannot hold is not neutral; a nonzero term has no variable in the seed; the loop
/// has no parameter binding; or a value is outside its binding.
fn project_pid_values(
    loop_definition: &LoopDefinition,
    seed: &ParallelPidSeed,
    trajectory: Option<&TrajectoryFeedforwardSeed>,
    feedforward: Option<&FeedforwardPidSeed>,
    smith: Option<&SmithPidSeed>,
    gains: &ControllerGains,
    observed: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    let Some(proportional_gain) = gains.proportional_gain else {
        return Err(Failure::new(
            &texts::project_proportional_gain_required(),
            Cause::Settings,
            NextAction::Tune,
        ));
    };
    unrepresented_terms(gains)?;
    let parallel = ParallelGains {
        proportional_gain,
        integral_gain: named_term(
            gains.integral_gain,
            seed.integral_symbol.is_some(),
            "integral",
        )?,
        derivative_gain: named_term(
            gains.derivative_gain,
            seed.derivative_symbol.is_some(),
            "derivative",
        )?,
    };
    let Some(binding) = loop_definition.parameter_binding.as_ref() else {
        return Err(Failure::new(
            &texts::parameter_pid_record_required(),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ));
    };
    let written = seed_values(seed, parallel);
    let mut terms: Vec<Term<'_>> = written
        .iter()
        .map(|(symbol, value)| Term {
            symbol: symbol.as_str(),
            gain: true,
            value: *value,
        })
        .collect();
    terms.extend(feedforward_terms(trajectory, gains)?);
    terms.extend(gain_feedforward_terms(feedforward, gains)?);
    if let Some(smith) = smith {
        let model = smith_adapter::smith_terms(smith, gains.smith.as_ref())?;
        terms.extend(model.into_iter().map(|(symbol, value)| Term {
            symbol,
            gain: false,
            value,
        }));
    }
    bound_values(binding, &terms, observed)
}

/// The feedforward terms of a measured-disturbance feedforward (C8): its gain `K_ff`, and its lead and lag times when the seed names
/// them, each written to the variable its seed names. The seed's lead and lag are both written or both absent. A law without such a
/// feedforward has nowhere to write one, so a nonzero term is refused there.
///
/// # Errors
/// The candidate lacks the feedforward gain, or its lead and lag are not both present as the seed names them; or a law without a
/// feedforward holds a nonzero term.
fn gain_feedforward_terms<'a>(
    feedforward: Option<&'a FeedforwardPidSeed>,
    gains: &ControllerGains,
) -> Result<Vec<Term<'a>>, Failure> {
    let Some(seed) = feedforward else {
        named_term(gains.feedforward_gain, false, "feedforward")?;
        named_term(gains.lead_seconds, false, "lead")?;
        named_term(gains.lag_seconds, false, "lag")?;
        return Ok(Vec::new());
    };
    let refusal = || {
        Failure::new(
            &texts::project_feedforward_gain_required(),
            Cause::Settings,
            NextAction::Tune,
        )
    };
    let gain = gains.feedforward_gain.ok_or_else(refusal)?;
    let mut terms = vec![Term {
        symbol: seed.gain_symbol.as_str(),
        gain: true,
        value: gain,
    }];
    match (&seed.lead_lag, gains.lead_seconds, gains.lag_seconds) {
        (Some(lead_lag), Some(lead), Some(lag)) => {
            terms.push(Term {
                symbol: lead_lag.lead_symbol.as_str(),
                gain: true,
                value: lead,
            });
            terms.push(Term {
                symbol: lead_lag.lag_symbol.as_str(),
                gain: true,
                value: lag,
            });
        }
        (None, None, None) => {}
        _ => return Err(refusal()),
    }
    Ok(terms)
}

/// The feedforward terms of a trajectory-following law (C11): its rate gain `K_v` and acceleration gain `K_a`, each written to the
/// variable its seed names. Both gains are required for such a law. A law without a trajectory has nowhere to write a feedforward gain,
/// so a nonzero one is refused, and a zero one is neutral.
///
/// # Errors
/// The trajectory law's candidate lacks one of its feedforward gains, or a law without a trajectory holds a nonzero one.
fn feedforward_terms<'a>(
    trajectory: Option<&'a TrajectoryFeedforwardSeed>,
    gains: &ControllerGains,
) -> Result<Vec<Term<'a>>, Failure> {
    let Some(seed) = trajectory else {
        named_term(gains.rate_gain, false, "rate")?;
        named_term(gains.acceleration_gain, false, "acceleration")?;
        return Ok(Vec::new());
    };
    let (Some(rate), Some(acceleration)) = (gains.rate_gain, gains.acceleration_gain) else {
        return Err(Failure::new(
            &texts::project_feedforward_required(),
            Cause::Settings,
            NextAction::Tune,
        ));
    };
    Ok(vec![
        Term {
            symbol: seed.rate_symbol.as_str(),
            gain: true,
            value: rate,
        },
        Term {
            symbol: seed.acceleration_symbol.as_str(),
            gain: true,
            value: acceleration,
        },
    ])
}

/// The value of an integral or derivative gain of a program's own law (C2): the gain, or zero when it is absent. A nonzero gain for a
/// term the seed names no variable for has nowhere to be written, so it is refused.
///
/// # Errors
/// The seed names no variable for the term, and the gain is not zero.
fn named_term(gain: Option<f64>, named: bool, term: &str) -> Result<f64, Failure> {
    match gain {
        Some(value) if !named && !is_exactly_zero(value) => Err(Failure::new(
            &texts::project_term_missing(term),
            Cause::ControllerCapability,
            NextAction::ChooseMethod,
        )),
        _ => Ok(gain.unwrap_or(0.0)),
    }
}

/// Refuses a term of the gain record that a program's own law has no variable for (C0, C2): a back-calculation gain, a setpoint weight
/// other than one, a derivative setpoint weight other than zero, or a measurement filter time other than zero.
///
/// # Errors
/// The gain record holds one of those terms without a variable to write it to.
pub(super) fn unrepresented_terms(gains: &ControllerGains) -> Result<(), Failure> {
    let sentence = if gains.back_calculation_gain.is_some() {
        texts::parameter_anti_windup_required()
    } else if gains
        .proportional_setpoint_weight
        .is_some_and(|weight| !is_one(weight))
    {
        texts::parameter_setpoint_weight_required()
    } else if gains
        .derivative_setpoint_weight
        .is_some_and(|weight| !is_exactly_zero(weight))
    {
        texts::parameter_derivative_weight_required()
    } else if gains
        .measurement_filter_seconds
        .is_some_and(|time| !is_exactly_zero(time))
    {
        texts::parameter_filter_time_required()
    } else {
        return Ok(());
    };
    Err(Failure::new(
        &sentence,
        Cause::ControllerCapability,
        NextAction::OpenLoopSetup,
    ))
}

/// Whether a value is within its declared binding (N6.4, C0): a gain by its magnitude, any other term by its signed bounds.
fn within(value: f64, bounds: GainBounds, gain: bool) -> bool {
    if gain {
        let (lowest, highest) = bounds.magnitude_range();
        (lowest..=highest).contains(&value.abs())
    } else {
        (bounds.minimum..=bounds.maximum).contains(&value)
    }
}

/// The bounds a sentence shows for a binding: the magnitudes a gain admits, or the signed bounds of any other term.
fn displayed_bounds(bounds: GainBounds, gain: bool) -> (f64, f64) {
    if gain {
        bounds.magnitude_range()
    } else {
        (bounds.minimum, bounds.maximum)
    }
}

/// Whether an optional gain is truthy as the TypeScript reads it: present, not zero and not NaN.
fn truthy(value: Option<f64>) -> bool {
    value.is_some_and(|value| !value.is_nan() && !is_exactly_zero(value))
}

/// Whether a value is exactly one: the TypeScript's `!== 1`, compared on the bits so that no float comparison is needed.
fn is_one(value: f64) -> bool {
    value.to_bits() == 1.0_f64.to_bits()
}

#[cfg(test)]
#[path = "adapters_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "trajectory_adapter_tests.rs"]
mod trajectory_tests;

#[cfg(test)]
#[path = "feedforward_adapter_tests.rs"]
mod feedforward_tests;
