//! The loop definition of a trajectory-following law (specification 35, C11): a program's own PID law whose `trajectory-feedforward-pid`
//! seed names the PID's three gains and the rate and acceleration feedforward gains, and whose feedforward term the program adds to its
//! output from the planned trajectory. No TypeScript generator exists for this structure. The own-law generator writes the PID loop, and
//! this module adds the trajectory's variables to it: the seed, the feedforward term declared at zero, the planned trajectory as a
//! further signal, and the two feedforward gains as parameters of the record.

#![allow(clippy::module_name_repetitions)] // `TrajectoryLoopDraft` names the module's subject, as `OwnLawLoopDraft` does.

use serde::Deserialize;

use super::catalog::CompiledCatalog;
use super::loop_bindings::{
    binding, compiled, measured_signal, real_type, state_type, stated_unit, with_revision,
    FormSignal,
};
use super::own_law_loop::{own_law_loop_definition, OwnLawFamily, OwnLawGain, OwnLawLoopDraft};
use super::texts::save as texts;
use crate::commissioning::initial_state::ControllerInitialization;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    AuxiliaryRole, AuxiliarySignal, Direction, InitialValue, InitializedState, LoopDefinition,
    LoopScope, Parameter, PostScan, ProjectSeedBlock,
};
use crate::tuning::project_seed::TrajectoryFeedforwardSeed;

/// The identifier and the channel of the planned trajectory among the loop's further signals (C11, `signals.additional`).
const PLANNED_TRAJECTORY: &str = "planned-trajectory";

/// A trajectory-following program law as the engineer confirmed it (C11): the PID law the feedforward is added to, the two feedforward
/// gains with their ranges, the planned trajectory that the feedforward follows, and the variable of the feedforward term.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrajectoryLoopDraft {
    /// The program's PID law, as the own-law setup asks for it. Its family is PID: the seed names all three gains.
    pub own_law: OwnLawLoopDraft,
    /// The variable that holds the rate feedforward gain `K_v`, with the range Tune may choose from.
    pub rate_gain: OwnLawGain,
    /// The variable that holds the acceleration feedforward gain `K_a`, with the range Tune may choose from.
    pub acceleration_gain: OwnLawGain,
    /// The planned trajectory the feedforward follows: a program variable that the program computes and the loop reads. The setpoint's
    /// change starts it, and Validate observes it (C11).
    pub planned_trajectory: FormSignal,
    /// The variable that holds the feedforward term that the program adds to its output: zero at rest, so the switch to Auto does not bump.
    pub feedforward_term: String,
}

/// The loop definition of a trajectory-following law (`trajectoryLoopDefinition`, C11): the own-law loop of its PID, whose seed names the
/// trajectory's variables, with the feedforward term declared at zero, the planned trajectory as a further signal, and the two feedforward
/// gains as parameters of the record. The startup-done flag is an application state, as the own-law loop writes it (C1).
///
/// # Errors
/// The own law is not a PID; the own-law generator refuses the PID; a variable of the trajectory is not compiled, or is not `REAL` or
/// `LREAL`; or the loop has no parameter binding to take the gains.
pub fn trajectory_loop_definition(
    catalog: &CompiledCatalog,
    scope: &LoopScope,
    draft: &TrajectoryLoopDraft,
) -> Result<LoopDefinition, Failure> {
    if draft.own_law.law != OwnLawFamily::Pid {
        return Err(refusal(texts::trajectory_law_not_pid()));
    }
    let mut loop_definition = own_law_loop_definition(catalog, scope, &draft.own_law)?;
    attach_seed(&mut loop_definition, draft)?;
    add_feedforward_state(catalog, &mut loop_definition, draft)?;
    add_planned_trajectory(catalog, &mut loop_definition, draft)?;
    add_feedforward_parameters(catalog, &mut loop_definition, draft)?;
    Ok(with_revision(loop_definition))
}

/// The seed of the own law becomes the trajectory seed: the PID variables the own-law loop names, and the two feedforward variables.
fn attach_seed(
    loop_definition: &mut LoopDefinition,
    draft: &TrajectoryLoopDraft,
) -> Result<(), Failure> {
    let project = loop_definition
        .controller
        .project
        .as_mut()
        .ok_or_else(|| refusal(texts::trajectory_law_not_pid()))?;
    let Some(ProjectSeedBlock::ParallelPid(pid)) = project.seed.take() else {
        return Err(refusal(texts::trajectory_law_not_pid()));
    };
    project.seed = Some(ProjectSeedBlock::TrajectoryFeedforward(
        TrajectoryFeedforwardSeed {
            pid,
            rate_symbol: draft.rate_gain.symbol.clone(),
            acceleration_symbol: draft.acceleration_gain.symbol.clone(),
        },
    ));
    Ok(())
}

/// The feedforward term is a law state that starts at zero (C11 Apply): at rest the planned rate and acceleration are zero, so the
/// switch to Auto does not bump. Its range is the output's.
fn add_feedforward_state(
    catalog: &CompiledCatalog,
    loop_definition: &mut LoopDefinition,
    draft: &TrajectoryLoopDraft,
) -> Result<(), Failure> {
    let output = measured_signal(
        &draft.own_law.manipulated_variable,
        "output",
        Some("own.manipulatedVariable"),
    )?;
    let state = InitializedState {
        symbol: draft.feedforward_term.clone(),
        iec_type: state_type(real_type(compiled(catalog, &draft.feedforward_term)?)?),
        unit: stated_unit(&output.unit),
        minimum: output.minimum,
        maximum: output.maximum,
        initial: InitialValue::Constant(0.0),
        post_scan: PostScan::Bounded,
    };
    let project = loop_definition
        .controller
        .project
        .as_mut()
        .ok_or_else(|| refusal(texts::trajectory_law_not_pid()))?;
    let mut states = project.initialization.states().to_vec();
    states.push(state);
    project.initialization = ControllerInitialization::new(states)?;
    Ok(())
}

/// The planned trajectory is a further signal of the loop, read from the program (C11, `signals.additional`), in the role of a state.
fn add_planned_trajectory(
    catalog: &CompiledCatalog,
    loop_definition: &mut LoopDefinition,
    draft: &TrajectoryLoopDraft,
) -> Result<(), Failure> {
    let planned = measured_signal(
        &draft.planned_trajectory,
        "planned trajectory",
        Some("traj.plannedTrajectory"),
    )?;
    let signal = binding(catalog, &planned, Direction::Read)?;
    loop_definition.signals.additional = Some(vec![AuxiliarySignal {
        id: PLANNED_TRAJECTORY.to_owned(),
        channel: PLANNED_TRAJECTORY.to_owned(),
        role: AuxiliaryRole::State,
        symbol: signal.symbol,
        address: signal.address,
        iec_type: signal.iec_type,
        direction: signal.direction,
        unit: signal.unit,
        range: signal.range,
    }]);
    Ok(())
}

/// The two feedforward gains as parameters of the record: each variable's own `REAL` or `LREAL` type, the output per the rate and the
/// acceleration of the process (C0), and its range.
fn add_feedforward_parameters(
    catalog: &CompiledCatalog,
    loop_definition: &mut LoopDefinition,
    draft: &TrajectoryLoopDraft,
) -> Result<(), Failure> {
    let output = draft.own_law.manipulated_variable.unit.as_str();
    let process = draft.own_law.process_variable.unit.as_str();
    let rate = feedforward_parameter(
        catalog,
        &draft.rate_gain,
        format!("{output}*s/{process}"),
        "Rate feedforward gain",
    )?;
    let acceleration = feedforward_parameter(
        catalog,
        &draft.acceleration_gain,
        format!("{output}*s^2/{process}"),
        "Acceleration feedforward gain",
    )?;
    let binding = loop_definition
        .parameter_binding
        .as_mut()
        .ok_or_else(|| refusal(texts::trajectory_law_not_pid()))?;
    binding.parameters.extend([rate, acceleration]);
    Ok(())
}

/// One feedforward gain as a parameter: its variable's real type, its unit, its convention and its range.
fn feedforward_parameter(
    catalog: &CompiledCatalog,
    gain: &OwnLawGain,
    unit: String,
    convention: &str,
) -> Result<Parameter, Failure> {
    Ok(Parameter {
        symbol: gain.symbol.clone(),
        iec_type: real_type(compiled(catalog, &gain.symbol)?)?,
        unit,
        convention: convention.to_owned(),
        minimum: gain.minimum,
        maximum: gain.maximum,
    })
}

/// A refusal of the draft: the engineer must change a setting before the loop can be written.
fn refusal(text: &str) -> Failure {
    Failure::new(text, Cause::Settings, NextAction::OpenLoopSetup)
}

#[cfg(test)]
#[path = "trajectory_loop_tests.rs"]
mod tests;
