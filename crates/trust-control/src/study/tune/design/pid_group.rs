//! The Tune of a PID group (specification 35, C7; `decentralizedPidDesign.ts` and `decoupledGroupDesign.ts`): each member's SIMC PI on
//! its own path of the study's model, for the gain that the member sees with the other loops closed (C7 steps 1 to 3). A program that
//! applies the static decoupler also gets `D = K⁻¹·diag(K)`, and each member's PI is then designed through it. A group is designed
//! Automatic only, with no pinned settings: it has no named method yet (C7, B4.2). The designs are
//! [`crate::tuning::structures::pid_group`]'s; this module reads the request and the program's seed, and writes the section and the answer.

use std::collections::BTreeMap;

use trust_control_math::identify::gain::ControllerAction;

use super::{declared_response, refused};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{LoopDefinition, PidControllerGroup, ProjectSeedBlock};
use crate::messages::{CandidateOutcome, TuneGainUnits, TuneResult};
use crate::model::GeneralModel;
use crate::stages::{stage_availability, StageEvidence};
use crate::study::tune::{
    controller_action, current_for, stage_evidence, TuneInputs, TuneOutcome, TuneSection,
};
use crate::tuning::candidate::CandidateSource;
use crate::tuning::group_candidate::{
    decoupler_values, member_design, member_values, GroupCandidate, GroupDecoupler,
    GroupMemberDesign,
};
use crate::tuning::objective::TuningObjective;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::structures::pid_group::{
    design_decentralized_pi, design_decoupled_group, DecentralizedPair, DecouplerMember,
};
use crate::tuning::structures::texts::DECOUPLER_MEMBERS;

/// A PID group has no named method: Automatic designs it (C7).
const GROUP_AUTOMATIC: &str = "A PID group is designed by its own rule. Choose Automatic.";
/// The pinned settings of a group are not served yet (C7).
const GROUP_PINS: &str =
    "Pinned settings are not yet served for a PID group. Clear the pins and Tune again.";
/// A group's program declares its seed: the decentralized one, or the decoupled one with its variables (C7).
const GROUP_SEED: &str =
    "A PID group needs its program to declare a decentralized or a decoupled seed.";
/// The loop declares no group, which the caller checks before it designs one.
const NO_GROUP: &str = "Tune reached the PID group design without a controller group.";
/// The requirements the study holds: the run takes them from the request's record or the earlier Tune (B4.3).
const NEEDS_REQUIREMENTS: &str = "Tune needs the process requirements: declare them before Tune.";
/// The sentence of a paired group: each member's PI, designed on its own path (C7).
const READY_PAIRED: &str = "Each member's PI is designed by SIMC on its own path, for the gain it sees with the other loops closed. Validate it before applying.";
/// The sentence of a decoupled group: each member's PI is designed through the decoupler (C7).
const READY_DECOUPLED: &str = "Each member's PI is designed by SIMC on its own path through the decoupler D = K⁻¹·diag(K). Validate it before applying.";
/// The method in words, for the answer (C7).
const METHOD_DESCRIPTION: &str = "Each member's PI by SIMC on its own path, for a PID group; a program that applies the static decoupler designs them through D = K⁻¹·diag(K) (C7).";
/// A group's answer is not predicted on the model: Validate runs every member (C7).
const UNPREDICTED: &str =
    "A PID group is not predicted here: Validate runs every member on the model.";
/// A group has no margins of one loop, so none are given (C7).
const MARGINS_UNAVAILABLE: &str =
    "The margins of a PID group are not given: Validate tests the group on the model.";
/// The method's title for Automatic (B4.2).
const AUTOMATIC_TITLE: &str = "Automatic";

/// The checks of a PID group before its design (C7): Automatic only, no pins, the engine runs the loop, and the study's model is current.
///
/// # Errors
/// A named method, pinned settings, a loop the engine does not run, or no current model.
pub(crate) fn check(inputs: &TuneInputs<'_>) -> Result<(), Failure> {
    let loop_definition = inputs.loop_definition;
    if inputs.request.method.is_some() {
        return Err(Failure::new(
            GROUP_AUTOMATIC,
            Cause::Settings,
            NextAction::ChooseMethod,
        ));
    }
    if inputs.request.pins.is_some() {
        return Err(Failure::new(GROUP_PINS, Cause::Settings, NextAction::None));
    }
    loop_definition.rust_engine_support()?;
    current_for(inputs.model, loop_definition).map(|_| ())
}

/// The group's design of a request: each member's PI on the study's current model, through the program's decoupler when it has one
/// (C7). The result gives the section to store, unapplied, and the answer (B4.1 to B4.6).
///
/// # Errors
/// The checks of [`check`]; requirements that the study does not hold; a seed the program does not declare; or a member, path or
/// decoupler that the design refuses.
pub(crate) fn run(
    inputs: &TuneInputs<'_>,
    held: Option<&TuningRequirements>,
) -> Result<TuneOutcome, Failure> {
    check(inputs)?;
    let loop_definition = inputs.loop_definition;
    let current = current_for(inputs.model, loop_definition)?;
    let group = declared_group(loop_definition)?;
    let requirements = held
        .or_else(|| inputs.previous.map(|previous| &previous.requirements))
        .ok_or_else(|| {
            Failure::new(
                NEEDS_REQUIREMENTS,
                Cause::Settings,
                NextAction::OpenRequirements,
            )
        })?;
    let objective = declared_response(requirements);
    let action = controller_action(loop_definition);
    let model = &current.model;
    let designed = match loop_definition
        .controller
        .project
        .as_ref()
        .and_then(|project| project.seed.as_ref())
    {
        Some(ProjectSeedBlock::Decentralized) => paired(group, model, objective, action)?,
        Some(ProjectSeedBlock::Decoupled(seed)) => {
            decoupled(group, &seed.matrix_symbols, model, objective, action)?
        }
        _ => {
            return Err(Failure::new(
                GROUP_SEED,
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            ))
        }
    };
    let candidate = GroupCandidate {
        applied: false,
        source: CandidateSource::Simc,
        members: designed.members,
        decoupler: designed.decoupler,
        values: designed.values,
    };
    let section = TuneSection {
        method: None,
        model_revision: model.identity().revision.clone(),
        requirements: requirements.clone(),
        candidate: None,
        project_candidate: None,
        state_candidate: None,
        search_candidate: None,
        on_off_candidate: None,
        group_candidate: Some(candidate.clone()),
        reason: designed.reason,
        closed_loop_time_seconds: None,
        attempts: Vec::new(),
        pins: None,
    };
    let evidence = stage_evidence(
        loop_definition,
        Some(current),
        Some(&section),
        current.identify.is_some(),
    );
    let result = answer(&candidate, &section, objective, evidence);
    Ok(TuneOutcome { section, result })
}

/// A group's design: its values (every variable Apply writes), its members, its decoupler when it has one, and the reason.
struct Designed {
    values: BTreeMap<String, f64>,
    members: Vec<GroupMemberDesign>,
    decoupler: Option<GroupDecoupler>,
    reason: String,
}

/// The group the loop declares.
fn declared_group(loop_definition: &LoopDefinition) -> Result<&PidControllerGroup, Failure> {
    loop_definition
        .controller
        .group
        .as_ref()
        .ok_or_else(|| Failure::new(NO_GROUP, Cause::Implementation, NextAction::OpenLoopSetup))
}

/// The members' PIs of a decentralized group: each member's own path, designed for the gain it sees with the others closed (C7).
fn paired(
    group: &PidControllerGroup,
    model: &GeneralModel,
    objective: TuningObjective,
    action: ControllerAction,
) -> Result<Designed, Failure> {
    let pairs: Vec<DecentralizedPair> = group
        .members
        .iter()
        .map(|member| DecentralizedPair {
            input_id: member.input_channel_id.clone(),
            output_id: member.output_channel_id.clone(),
        })
        .collect();
    let design = design_decentralized_pi(model, &pairs, objective, action)
        .map_err(refused(Cause::ModelOrData, NextAction::OpenLoopSetup))?;
    Ok(Designed {
        values: member_values(group, &design.members)?,
        members: group
            .members
            .iter()
            .zip(&design.members)
            .map(|(declared, designed)| member_design(declared, designed))
            .collect::<Result<Vec<GroupMemberDesign>, Failure>>()?,
        decoupler: None,
        reason: READY_PAIRED.to_owned(),
    })
}

/// The members' PIs of a decoupled group: `D = K⁻¹·diag(K)` from the group's steady-state gains, and each member's PI designed on the
/// model that the decoupler gives (C7). The program's variables for `D` are the seed's matrix, one row per plant input.
fn decoupled(
    group: &PidControllerGroup,
    symbols: &[Vec<String>],
    model: &GeneralModel,
    objective: TuningObjective,
    action: ControllerAction,
) -> Result<Designed, Failure> {
    if symbols.len() != group.members.len()
        || symbols.iter().any(|row| row.len() != group.members.len())
    {
        return Err(Failure::new(
            DECOUPLER_MEMBERS,
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    let members: Vec<DecouplerMember> = group
        .members
        .iter()
        .map(|member| DecouplerMember {
            instance: member.instance.clone(),
            input_channel_id: member.input_channel_id.clone(),
            output_channel_id: member.output_channel_id.clone(),
        })
        .collect();
    let design = design_decoupled_group(model, &members, Some(objective), action)
        .map_err(refused(Cause::ModelOrData, NextAction::OpenLoopSetup))?;
    let mut values = member_values(group, &design.members)?;
    values.extend(decoupler_values(symbols, &design.matrix));
    Ok(Designed {
        values,
        members: group
            .members
            .iter()
            .zip(&design.members)
            .map(|(declared, designed)| member_design(declared, designed))
            .collect::<Result<Vec<GroupMemberDesign>, Failure>>()?,
        decoupler: Some(GroupDecoupler {
            matrix: design.matrix,
            input_channel_ids: design.input_ids,
            member_instances: design.member_ids,
            block: design.block,
        }),
        reason: READY_DECOUPLED.to_owned(),
    })
}

/// The answer of a group's Tune (C7, B4.5): the candidate, its values as the settings, and the outcome that is not predicted. A group
/// has no single loop's margins, so none are given.
fn answer(
    candidate: &GroupCandidate,
    section: &TuneSection,
    objective: TuningObjective,
    evidence: StageEvidence,
) -> TuneResult {
    TuneResult {
        group: Some(candidate.clone()),
        feedforward: None,
        state: None,
        candidate: None,
        project_candidate: None,
        settings: candidate.values.clone(),
        method: None,
        method_title: AUTOMATIC_TITLE.to_owned(),
        method_description: METHOD_DESCRIPTION.to_owned(),
        reason: section.reason.clone(),
        objective,
        closed_loop_time_seconds: None,
        attempts: Vec::new(),
        gain_units: TuneGainUnits {
            proportional: String::new(),
            integral: String::new(),
            derivative: String::new(),
        },
        requirements: section.requirements.clone(),
        outcome: CandidateOutcome::Unpredicted {
            reason: UNPREDICTED.to_owned(),
        },
        response: None,
        prediction_unavailable: Some(UNPREDICTED.to_owned()),
        settling_time_seconds: None,
        overshoot_percent: None,
        delay_margin_seconds: None,
        margins: None,
        margins_unavailable: Some(MARGINS_UNAVAILABLE.to_owned()),
        comparison: Vec::new(),
        stages: stage_availability(&evidence),
        stage_evidence: evidence,
        schedule: None,
    }
}

#[cfg(test)]
#[path = "pid_group_tests.rs"]
mod tests;
