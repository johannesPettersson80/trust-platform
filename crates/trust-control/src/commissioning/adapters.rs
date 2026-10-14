//! The controller adapters of Control Studio (specification 35, C0, C1, C2 and C13; `controllerAdapters.ts`): which adapter a
//! loop's controller has, the structures it can realize, the transfer and the initialization it needs, the commissioning profile
//! of its declared states, and the parameter values that a set of gains writes. Tune and the commissioning flows call these
//! decisions and run the runtime themselves.
//!
//! Three adapters are ported: the shipped `Control.PID` (C1, with the library's `control-pid-v1` states), a program's parallel PID
//! (C2, `parallel-pid-v1`) and the connected P (C13). A loop that uses another family is refused, never guessed at: the program's
//! own law, ON/OFF (C3), state feedback (C4), notch (C5), cascade (C6), PID groups (C7), feedforward (C8), Smith (C9), gain
//! scheduling (C10), trajectory (C11) and experiment loops (C12) have no law in `loop_definition` yet, and channels (multivariable
//! loops) are parked with stage V7. `projectController.ts` is a parked feature and is not ported.

use std::collections::BTreeMap;

use super::declared_initialization::{
    declared_commissioning_profile, declared_initialization_plan, declared_transfer_binding,
};
use super::initial_state::ControllerInitialization;
use super::initialization_plan::{
    pid_initialization_plan, proportional_initialization_plan, ParallelPidStartPlan, StartPlan,
};
use super::library_pid::library_pid_initialization;
use super::pid_initialization::pid_initialization_adapter;
use super::profile::CommissioningProfile;
use super::texts;
use super::transfer::{
    pid_transfer_binding, proportional_parameter, proportional_transfer_binding, TransferBinding,
};
use super::transfer_tracking::{parallel_pid_tracking, proportional_tracking, TransferTracking};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    ControllerLaw, LoopDefinition, Parameter, ParameterBinding, PidInitialization, PidLaw, RealType,
};
use crate::model::GeneralModel;
use crate::tuning::proportional::GainBounds;
use crate::tuning::settings_pins::single_precision;
use crate::tuning::structure::{is_exactly_zero, Structure};
use crate::validation::engineer_verification_plan::EngineerRequirements;

/// The structures a PID adapter can realize (C0, "Families and zero terms").
const PID_FAMILIES: [Structure; 4] = [Structure::P, Structure::PI, Structure::PD, Structure::PID];

/// The structures the connected P can realize (C13): the proportional one only.
const PROPORTIONAL_FAMILIES: [Structure; 1] = [Structure::P];

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
}

/// Which adapter a loop's controller has (`controllerAdapter`): the shipped `Control.PID` with its states, a parallel PID with its
/// `parallel-pid-v1` contract, or the connected P.
#[derive(Debug, Clone, PartialEq)]
pub enum AdapterKind {
    /// The shipped `Control.PID` (C1): its retained states, initialized by the library's `control-pid-v1` declaration.
    ControlPid(ControllerInitialization),
    /// A program's parallel PID (C2): its integral and derivative states, declared by `parallel-pid-v1`.
    ParallelPid,
    /// The connected P (C13): the proportional gain only, with its integral and derivative gains written as zero.
    ConnectedP,
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
/// parallel PID for any other PID law, and the connected P for a proportional law.
///
/// # Errors
/// The loop uses a feature that no adapter here realizes; the library's initialization of a `Control.PID` does not fit the loop; or
/// the loop has no proportional law that the connected P can use.
pub fn controller_adapter(
    loop_definition: &LoopDefinition,
) -> Result<ControllerAdapter<'_>, Failure> {
    if let Some(feature) = parked_feature(loop_definition) {
        return Err(Failure::new(
            &texts::adapter_parked_feature(feature),
            Cause::ControllerCapability,
            NextAction::None,
        ));
    }
    let kind = if let (Some(ControllerLaw::Pid(_)), Some(_)) = (
        &loop_definition.controller.law,
        &loop_definition.parameter_binding,
    ) {
        match &loop_definition.controller.pid_initialization {
            Some(PidInitialization::ControlPid { .. }) => {
                AdapterKind::ControlPid(library_pid_initialization(loop_definition)?)
            }
            _ => AdapterKind::ParallelPid,
        }
    } else {
        proportional_parameter(loop_definition)
            .map_err(|failure| unrecognised_type(loop_definition, &failure))?;
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

    /// The structures the adapter can realize (`families`): the PID kinds offer P, PI, PD and PID, and the connected P offers P.
    #[must_use]
    pub fn families(&self) -> &'static [Structure] {
        match self.kind {
            AdapterKind::ConnectedP => &PROPORTIONAL_FAMILIES,
            AdapterKind::ControlPid(_) | AdapterKind::ParallelPid => &PID_FAMILIES,
        }
    }

    /// The transfer binding of the adapter (`transferBinding`, A6.3): the references, the mode and the outputs that an Auto entry
    /// writes.
    ///
    /// # Errors
    /// The adapter's declared states or bindings do not fit the loop.
    pub fn transfer_binding(&self) -> Result<TransferBinding, Failure> {
        match &self.kind {
            AdapterKind::ControlPid(initialization) => {
                control_pid_binding(self.loop_definition, initialization)
            }
            AdapterKind::ParallelPid => pid_transfer_binding(self.loop_definition),
            AdapterKind::ConnectedP => proportional_transfer_binding(self.loop_definition),
        }
    }

    /// The tracking that a recovery to Manual holds (`recoveryTracking`, A6.2): the declared states of the library PID, the parallel
    /// PID's states, or the proportional reference. The tracking is not initialized here.
    ///
    /// # Errors
    /// The adapter's bindings or its declared states do not fit the loop.
    pub fn recovery_tracking(&self, model: &GeneralModel) -> Result<TransferTracking, Failure> {
        match &self.kind {
            AdapterKind::ControlPid(initialization) => {
                let profile = control_pid_profile(
                    self.loop_definition,
                    model,
                    initialization,
                    &BTreeMap::new(),
                )?;
                let binding = control_pid_binding(self.loop_definition, initialization)?;
                profile.tracking(&binding, false)
            }
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

    /// The commissioning profile of the adapter's declared states (`declaredProfile`, C1), at the given values. Only the shipped
    /// `Control.PID` declares one; the parallel PID and the connected P have none.
    ///
    /// # Errors
    /// The declared states do not fit the loop, or the profile does not hold at the operating point.
    pub fn declared_profile(
        &self,
        model: &GeneralModel,
        values: &BTreeMap<String, f64>,
    ) -> Result<Option<CommissioningProfile>, Failure> {
        match &self.kind {
            AdapterKind::ControlPid(initialization) => {
                control_pid_profile(self.loop_definition, model, initialization, values).map(Some)
            }
            AdapterKind::ParallelPid | AdapterKind::ConnectedP => Ok(None),
        }
    }

    /// Checks that the adapter can start an Auto entry (`checkInitialization`, C0): the declared states and their binding for a
    /// `Control.PID`, the explicit contract for a parallel PID, and the gain parameter for the connected P.
    ///
    /// # Errors
    /// The adapter's initialization is missing or does not fit the loop.
    pub fn check_initialization(&self) -> Result<(), Failure> {
        match &self.kind {
            AdapterKind::ControlPid(initialization) => {
                control_pid_binding(self.loop_definition, initialization).map(|_| ())
            }
            AdapterKind::ParallelPid => {
                pid_initialization_adapter(self.loop_definition).map(|_| ())
            }
            AdapterKind::ConnectedP => proportional_parameter(self.loop_definition).map(|_| ()),
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
            AdapterKind::ControlPid(initialization) => {
                control_pid_binding(self.loop_definition, initialization)?;
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

/// The first feature of a loop that no adapter here realizes (C0 "Families", C3 to C12): such a loop is refused, not guessed at.
fn parked_feature(loop_definition: &LoopDefinition) -> Option<&'static str> {
    let controller = &loop_definition.controller;
    if controller.project.is_some() {
        Some("the program's own law")
    } else if controller.on_off.is_some() {
        Some("an ON/OFF controller")
    } else if controller.state_feedback.is_some() {
        Some("state feedback")
    } else if controller.group.is_some() {
        Some("a PID group")
    } else if loop_definition.channels.is_some() {
        Some("channels")
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

/// The transfer binding of the library PID (`requiredInitialization`, C1): the declared states must bind the loop's references, mode
/// and manual command. A refusal keeps its reason and offers the choice of a method (A11.2).
fn control_pid_binding(
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

/// The commissioning profile of the library PID (`profile`, C1), at the given values.
fn control_pid_profile(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    initialization: &ControllerInitialization,
    values: &BTreeMap<String, f64>,
) -> Result<CommissioningProfile, Failure> {
    control_pid_binding(loop_definition, initialization)?;
    declared_commissioning_profile(
        loop_definition,
        model,
        initialization,
        "Control.PID",
        values,
    )
}

/// One term of a PID record: the parameter it writes, whether the parameter is a gain (bounded by its magnitude, N6.4), and the value.
struct Term<'a> {
    symbol: &'a str,
    gain: bool,
    value: f64,
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
fn bound_values(
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
