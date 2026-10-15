//! The loop definition of a PID group (specification 35, C7 and §17.9, §17.15): several library `Control.PID`s, each on one model input
//! and one model output, tuned and applied together under one Auto/Manual signal. Ported from `pidGroupGenerator.ts`. The project and
//! the runtime come from the scope, never from the draft. Each member's PI is designed on the group model, or, behind the program's
//! static decoupler `D`, starts at its share of the plant inputs.

#![allow(clippy::module_name_repetitions)] // `GroupLoopDraft` names the module's subject, as the TypeScript does.

use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::json;

use super::catalog::{pid_candidates, CompiledCatalog, CompiledSymbol, CompiledTask, PID_TYPE};
use super::loop_bindings::{
    binding, compiled, cyclic_task, identification_duration, loop_id, manual_output_type,
    measured_signal, measurement_binding, real_type, stated_signal, with_revision, FormSignal,
    MeasuredSignal,
};
use super::pid_bindings::{manual_output_tracking, pid_parameters, pid_states, startup_state};
use super::pid_loop::{setpoint_signal, ModeChoice, SignalChoice};
use super::state_feedback_loop::check_startup;
use super::texts::generators as texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    Action, AuxiliaryRole, AuxiliarySignal, ChannelInput, ChannelOutput, Controller,
    ControllerProject, Design, Direction, Execution, Experiment, ExperimentKind, InitialSource,
    InitialValue, InitializedState, Limits, LoopChannels, LoopDefinition, LoopScope, LoopSignal,
    ModeBinding, ModeWhenTrue, Parameter, ParameterBinding, PidControllerGroup, PidGroupMember,
    SampleTiming, Signals, Topology, LOOP_SCHEMA_VERSION,
};
use crate::model::{ActuatorCapabilities, ActuatorKind};

/// The fewest members a group has (`pidGroupGenerator.ts`).
const MINIMUM_MEMBERS: usize = 2;

/// The gain fields of a member's parameter record, in the order the group's parameters follow (`pidGroupLoopDefinition`).
const GAIN_FIELDS: [&str; 3] = ["ProportionalGain", "IntegralGain", "DerivativeGain"];

/// The bound of each static decoupler entry, in its unit (numerics annex N10, *Static decoupler entry D*: ±1,000).
const DECOUPLER_BOUND: f64 = 1_000.0;

/// The refusal of a group with fewer than two members (`pidGroupGenerator.ts`).
const FEW_MEMBERS: &str = "A PID group needs at least two controllers.";
/// The refusal of a group that names one controller twice (`pidGroupGenerator.ts`).
const REPEATED_MEMBER: &str = "Choose each controller only once.";
/// The refusal of members that do not run in one task (`pidGroupGenerator.ts`).
const SAME_TASK: &str = "All members of a PID group must run in the same task.";
/// The refusal of a decoupler that is not one variable per plant input and member, each used once (`pidGroupGenerator.ts`).
const DECOUPLER_SHAPE: &str =
    "A decoupler needs one variable per plant input and member, each used once.";
/// The refusal of a startup-done flag that is one of the group's signals (C1; Specification 35 §17.9). The flag is an application state of
/// the first member's PID, so it cannot also be a signal of the loop.
const STARTUP_IS_SIGNAL: &str =
    "The startup-done flag is one of the loop's signals, so it cannot mark the startup as done.";

/// A PID group as the engineer confirmed it (`PidGroupDraft`, C7). The step and the duration are optional: Identify chooses what is
/// left (B3.3, B3.4). The project and the runtime are not in the draft; the save removes them before it posts the draft.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupLoopDraft {
    /// The loop's name, from which its id is made.
    pub name: String,
    /// Whether each output rises (`direct`) or falls (`reverse`) while its measurement is below its setpoint.
    pub action: Action,
    /// The Auto/Manual signal that every member shares, and the mode its TRUE value means.
    pub mode: ModeChoice,
    /// The group's identification step, in the outputs' unit: the loop's own step, and the step of each member without its own.
    pub experiment_step: Option<f64>,
    /// How long identification runs, in s; with the group's step.
    pub experiment_duration_seconds: Option<f64>,
    /// The BOOL the program sets when its startup code has run.
    pub startup_flag: Option<String>,
    /// The members: at least two, each a library `Control.PID` that drives one model input and measures one model output.
    pub members: Vec<GroupMemberDraft>,
    /// The program's static decoupler `D[i][j]`, for plant input `i` and member `j`: one row and one column per member.
    pub decoupler: Option<Vec<Vec<String>>>,
}

/// One member of a PID group as the engineer confirmed it (`PidGroupMemberDraft`, C7).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupMemberDraft {
    /// The member's instance path: a compiled `Control.PID`.
    pub instance: String,
    /// The record the member's PID is configured from.
    pub parameter_record: String,
    /// The member's measurement.
    pub process_variable: FormSignal,
    /// The output the member drives.
    pub manipulated_variable: FormSignal,
    /// The member's setpoint; the measurement's unit and range where it gives none.
    pub setpoint: SignalChoice,
    /// The manual output that Manual writes for the member.
    pub manual_output: String,
    /// The member's own identification step, in its output's unit; the group's step when absent.
    pub experiment_step: Option<f64>,
}

/// A member as the generator uses it: its instance and record, its signals with their stated units and ranges, its setpoint, its manual
/// output, and the step of its own identification (its own step, else the group's).
#[derive(Debug)]
struct Member {
    instance: String,
    record: String,
    process: MeasuredSignal,
    manipulated: MeasuredSignal,
    setpoint: MeasuredSignal,
    manual: CompiledSymbol,
    step: Option<f64>,
}

impl Member {
    /// The member's three gains as parameters of its record (C0): `pidParameters` with the gain fields only.
    fn gains(&self) -> Vec<Parameter> {
        let fields: Vec<String> = GAIN_FIELDS
            .iter()
            .map(|field| format!("{}.{field}", self.record))
            .collect();
        pid_parameters(&self.record, &self.process, &self.manipulated)
            .into_iter()
            .filter(|parameter| fields.contains(&parameter.symbol))
            .collect()
    }
}

/// The loop definition of a PID group (`pidGroupLoopDefinition`): each member's PI is designed on its own path, the members share one
/// Auto/Manual signal, and the channels are named after their signals, as Identify names the channels of the model it fits.
///
/// # Errors
/// Fewer than two members; a controller named twice; a member that is not a compiled `Control.PID`, or does not run in exactly one
/// cyclic task; members in different tasks; a decoupler that is not one variable per plant input and member, each used once; a signal
/// or manual output that is not compiled or is not `REAL` or `LREAL`; behind a decoupler, a member whose plant input is its controller's
/// own output; a duration outside the identification limits; a startup-done flag
/// that the program needs and the draft does not name, that is not a compiled `BOOL`, or that is one of the loop's signals; or a
/// generated group that the loop file reader refuses.
pub fn group_loop_definition(
    catalog: &CompiledCatalog,
    scope: &LoopScope,
    draft: &GroupLoopDraft,
) -> Result<LoopDefinition, Failure> {
    let task = group_task(catalog, draft)?;
    let members = resolve_members(catalog, draft)?;
    let decoupler = decoupler_rows(draft)?;
    check_plant_inputs(catalog, draft, &members)?;
    let states = member_states(&members, decoupler);
    check_group_startup(catalog, draft, &members, &states)?;
    let controller = group_controller(
        catalog,
        draft,
        &members,
        &states,
        decoupler,
        task.interval_ms,
    )?;
    let first = &members[0];
    let id = loop_id(&draft.name);
    let loop_definition = LoopDefinition {
        schema_version: LOOP_SCHEMA_VERSION,
        id: id.clone(),
        name: draft.name.clone(),
        revision: "sha256:pending".to_owned(),
        project_id: scope.project_id.clone(),
        runtime_id: scope.runtime_id.clone(),
        controller,
        signals: group_signals(catalog, &members)?,
        limits: Limits {
            output_minimum: Some(first.manipulated.minimum),
            output_maximum: Some(first.manipulated.maximum),
            rate_up_per_s: None,
            rate_down_per_s: None,
            expected_settling_seconds: None,
        },
        observed_sample_timing: SampleTiming {
            nominal_period_ms: task.interval_ms,
            maximum_jitter_ms: None,
        },
        parameter_binding: Some(ParameterBinding {
            loop_id: id,
            task_name: task.name.clone(),
            mode: ModeBinding {
                symbol: draft.mode.symbol.clone(),
                manual_value: draft.mode.when_true == ModeWhenTrue::Manual,
            },
            parameters: group_parameters(catalog, &members, decoupler)?,
        }),
        parameter_purpose: None,
        topology: Some(Topology::ClosedLoop),
        experiment: group_experiment(draft)?,
        plant_id: None,
        operating_point: None,
        identification_policy: None,
        verification_scenarios: None,
        channels: Some(group_channels(&members)?),
        analysis: None,
        analysis_parameter_symbols: None,
        cascade: None,
    };
    let loop_definition = with_revision(loop_definition);
    check_group(&loop_definition)?;
    Ok(loop_definition)
}

/// The one cyclic task that the members share, after the members' own checks (`pidGroupLoopDefinition`, the first checks): at least two
/// members, each named once, each a compiled `Control.PID` in exactly one cyclic task, all in that task.
///
/// # Errors
/// One of these rules is broken; the sentence names the rule.
fn group_task<'a>(
    catalog: &'a CompiledCatalog,
    draft: &GroupLoopDraft,
) -> Result<&'a CompiledTask, Failure> {
    if draft.members.len() < MINIMUM_MEMBERS {
        return Err(refuse_settings(FEW_MEMBERS));
    }
    let mut instances = BTreeSet::new();
    if !draft
        .members
        .iter()
        .all(|member| instances.insert(member.instance.to_uppercase()))
    {
        return Err(refuse_settings(REPEATED_MEMBER));
    }
    let tasks = draft
        .members
        .iter()
        .map(|member| member_task(catalog, &member.instance))
        .collect::<Result<Vec<_>, _>>()?;
    let first = tasks[0];
    if tasks.iter().any(|task| task.name != first.name) {
        return Err(refuse_settings(SAME_TASK));
    }
    Ok(first)
}

/// The one cyclic task of the compiled `Control.PID` that a member names.
///
/// # Errors
/// No compiled `Control.PID` has the instance, or it does not run in exactly one cyclic task.
fn member_task<'a>(
    catalog: &'a CompiledCatalog,
    instance: &str,
) -> Result<&'a CompiledTask, Failure> {
    let controller = catalog
        .controllers
        .iter()
        .find(|entry| entry.instance == instance && entry.type_name.to_uppercase() == PID_TYPE)
        .ok_or_else(|| {
            Failure::new(
                &texts::not_compiled_pid(instance),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })?;
    cyclic_task(controller)
}

/// The members with their signals resolved: each signal with its stated unit and range, the setpoint with the measurement's unit and
/// range where it gives none, the manual output, and the member's own step.
///
/// # Errors
/// A member's manual output is not compiled.
fn resolve_members(
    catalog: &CompiledCatalog,
    draft: &GroupLoopDraft,
) -> Result<Vec<Member>, Failure> {
    draft
        .members
        .iter()
        .map(|member| {
            let process = stated_signal(&measured_signal(
                &member.process_variable,
                &format!("{} measurement", member.instance),
                None,
            )?);
            let manipulated = stated_signal(&measured_signal(
                &member.manipulated_variable,
                &format!("{} output", member.instance),
                None,
            )?);
            let setpoint = setpoint_signal(&member.setpoint, &process);
            Ok(Member {
                instance: member.instance.clone(),
                record: member.parameter_record.clone(),
                process,
                manipulated,
                setpoint,
                manual: compiled(catalog, &member.manual_output)?.clone(),
                step: member.experiment_step.or(draft.experiment_step),
            })
        })
        .collect()
}

/// The decoupler's rows, when the draft names one: one row per member, each with one variable per member, and no variable named twice
/// (compared in upper case), as `pidGroupLoopDefinition` checks it.
///
/// # Errors
/// The decoupler is not square in the members, or a variable is named twice.
fn decoupler_rows(draft: &GroupLoopDraft) -> Result<Option<&[Vec<String>]>, Failure> {
    let Some(rows) = draft.decoupler.as_deref() else {
        return Ok(None);
    };
    let size = draft.members.len();
    let square = rows.len() == size && rows.iter().all(|row| row.len() == size);
    let distinct: BTreeSet<String> = rows
        .iter()
        .flatten()
        .map(|symbol| symbol.to_uppercase())
        .collect();
    if square && distinct.len() == size * size {
        Ok(Some(rows))
    } else {
        Err(refuse_settings(DECOUPLER_SHAPE))
    }
}

/// Behind a decoupler a member's plant input is the one the program writes from `D·v`, not its controller's own output. A member whose
/// plant input is its controller's `Update` output is refused, and the setup asks for the plant input (Specification 35 §17.9).
///
/// # Errors
/// Behind a decoupler, a member's plant input is its controller's own output; the failure names that member's plant input.
fn check_plant_inputs(
    catalog: &CompiledCatalog,
    draft: &GroupLoopDraft,
    members: &[Member],
) -> Result<(), Failure> {
    if draft.decoupler.is_none() {
        return Ok(());
    }
    let candidates = pid_candidates(catalog);
    for (index, member) in members.iter().enumerate() {
        let own = candidates
            .iter()
            .find(|candidate| candidate.instance == member.instance)
            .and_then(|candidate| candidate.output.as_deref())
            .filter(|output| output.eq_ignore_ascii_case(&member.manipulated.symbol));
        if let Some(own) = own {
            return Err(refuse_settings(&plant_input_refusal(&member.instance, own))
                .with_field(&format!("group.{index}.manipulatedVariable")));
        }
    }
    Ok(())
}

/// The refusal of a member whose plant input, behind a decoupler, is its controller's own output (Specification 35 §17.9): it names the
/// instance and the output, and asks for the plant input.
fn plant_input_refusal(instance: &str, own: &str) -> String {
    format!(
        "Behind a decoupler, {instance}'s output is the plant input the program writes from D·v, \
         not the controller's own output {own}. Choose the plant input."
    )
}

/// Each member's seven retained states (`pidStates`, C1). Behind a decoupler each member's PID starts at its share of the present plant
/// inputs, `v = D⁻¹·u` (§17.15): its nominal and saturated outputs start at the decoupled input of its own row.
fn member_states(
    members: &[Member],
    decoupler: Option<&[Vec<String>]>,
) -> Vec<Vec<InitializedState>> {
    members
        .iter()
        .enumerate()
        .map(|(index, member)| {
            let mut states = pid_states(
                &member.instance,
                &member.process,
                &member.manipulated,
                index,
            );
            if let Some(matrix) = decoupler {
                for state in states
                    .iter_mut()
                    .filter(|state| is_output_state(&state.symbol))
                {
                    state.initial = InitialValue::From(InitialSource::DecoupledOutput {
                        index,
                        matrix_symbols: matrix.to_vec(),
                    });
                }
            }
            states
        })
        .collect()
}

/// Whether a state is the nominal or the saturated output of a PID (`pidGroupLoopDefinition`).
fn is_output_state(symbol: &str) -> bool {
    symbol.ends_with(".StateValue.NominalOutput") || symbol.ends_with(".StateValue.SaturatedOutput")
}

/// The startup-done flag the draft names, when it names one: a blank name is no flag (`startupFlag` is read as a truthy text).
fn startup_flag(draft: &GroupLoopDraft) -> Option<&str> {
    draft
        .startup_flag
        .as_deref()
        .filter(|flag| !flag.is_empty())
}

/// The group's identification experiment (B3.4; Specification 35 §17.9). The group's own step is an output step of that amplitude; without
/// one, the group asks for the automatic request, an observation of amplitude 0. A member's step is its input's step, not the loop's
/// (`channel_input`). With no step, no duration and no member step, the group writes no experiment.
///
/// # Errors
/// The duration is outside the identification limits.
fn group_experiment(draft: &GroupLoopDraft) -> Result<Option<Experiment>, Failure> {
    let asked = draft.experiment_step.is_some()
        || draft.experiment_duration_seconds.is_some()
        || draft
            .members
            .iter()
            .any(|member| member.experiment_step.is_some());
    if !asked {
        return Ok(None);
    }
    let (kind, amplitude) = match draft.experiment_step {
        Some(step) => (ExperimentKind::MvStep, step),
        None => (ExperimentKind::Observation, 0.0),
    };
    let duration_seconds = draft
        .experiment_duration_seconds
        .map(|seconds| identification_duration(&draft.name, Some(seconds)))
        .transpose()?;
    Ok(Some(Experiment {
        execution: Execution::ControlStudio,
        kind,
        amplitude,
        relay_direction: None,
        duration_seconds,
        baseline_seconds: None,
        hold_parameter: None,
        // The seed that every Control Studio experiment of a setup gets (`explicitExperiment`).
        seed: 1,
        activation: None,
        abort: None,
    }))
}

/// The group's startup-done flag, checked as C1 checks every loop's (`check_startup`): the program of each member needs the flag when it
/// initializes that member's PID or writes a value that validation seeds for the group. A flag the draft names is a compiled `BOOL`, and
/// it is none of the group's signals.
///
/// # Errors
/// The program needs a flag and the draft names none; the flag is not a compiled `BOOL`; or the flag is one of the group's signals.
fn check_group_startup(
    catalog: &CompiledCatalog,
    draft: &GroupLoopDraft,
    members: &[Member],
    states: &[Vec<InitializedState>],
) -> Result<(), Failure> {
    let seeded = seeded_symbols(draft, members, states);
    for member in members {
        check_startup(catalog, &member.instance, &seeded, startup_flag(draft))?;
    }
    match startup_flag(draft) {
        Some(flag) => check_flag(draft, members, flag),
        None => Ok(()),
    }
}

/// The symbols that validation seeds for the group (C1 Setup): the Auto/Manual signal; each member's setpoint, manual output and retained
/// states, its measurement excepted, as a PID's are; and the gains and decoupler entries that the group binds.
fn seeded_symbols(
    draft: &GroupLoopDraft,
    members: &[Member],
    states: &[Vec<InitializedState>],
) -> Vec<String> {
    let mut seeded = vec![draft.mode.symbol.clone()];
    for (member, member_states) in members.iter().zip(states) {
        seeded.push(member.setpoint.symbol.clone());
        seeded.push(member.manual.symbol.clone());
        seeded.extend(
            member_states
                .iter()
                .map(|state| state.symbol.clone())
                .filter(|symbol| !symbol.eq_ignore_ascii_case(&member.process.symbol)),
        );
        seeded.extend(member.gains().into_iter().map(|parameter| parameter.symbol));
    }
    seeded.extend(draft.decoupler.iter().flatten().flatten().cloned());
    seeded
}

/// A startup-done flag that is none of the group's signals: the Auto/Manual signal, and each member's setpoint, measurement, output and
/// manual output. The flag's type is checked first, by `check_startup`.
///
/// # Errors
/// The flag is one of the group's signals.
fn check_flag(draft: &GroupLoopDraft, members: &[Member], flag: &str) -> Result<(), Failure> {
    let mut signals = vec![draft.mode.symbol.as_str()];
    for member in members {
        signals.extend([
            member.setpoint.symbol.as_str(),
            member.process.symbol.as_str(),
            member.manipulated.symbol.as_str(),
            member.manual.symbol.as_str(),
        ]);
    }
    if signals
        .iter()
        .any(|signal| signal.eq_ignore_ascii_case(flag))
    {
        return Err(refuse_settings(STARTUP_IS_SIGNAL));
    }
    Ok(())
}

/// The controller of the group (B1.4, C7): the first member's instance and output, the program's law as `controller.project`, and the
/// members as `controller.group`.
///
/// # Errors
/// The first member's manual output is not `REAL` or `LREAL`, or the program's law breaks a rule of its project block.
fn group_controller(
    catalog: &CompiledCatalog,
    draft: &GroupLoopDraft,
    members: &[Member],
    states: &[Vec<InitializedState>],
    decoupler: Option<&[Vec<String>]>,
    period_ms: f64,
) -> Result<Controller, Failure> {
    let first = &members[0];
    let project = group_project(draft, first, states, decoupler)?;
    Ok(Controller {
        instance: first.instance.clone(),
        type_name: "Control.PID".to_owned(),
        revision: "sha256:unverified".to_owned(),
        action: draft.action,
        task_period_ms: period_ms,
        design: Some(Design::Project),
        law: None,
        mode_signal: Some(draft.mode.symbol.clone()),
        mode_when_true: Some(draft.mode.when_true),
        tracking_signal: None,
        manual_output_signal: Some(first.manual.symbol.clone()),
        manual_output_address: first
            .manual
            .address
            .clone()
            .filter(|address| !address.is_empty()),
        manual_output_type: Some(manual_output_type(real_type(&first.manual)?)),
        manual_output_follows_output: manual_output_tracking(
            catalog,
            &first.instance,
            &first.manual.symbol,
        )
        .then_some(true),
        pid_initialization: None,
        state_feedback: None,
        on_off: None,
        project: Some(project),
        group: Some(group_block(draft, members, states)),
    })
}

/// The program's law of the group as `controller.project` holds it (C7): the family, the declared states of every member with the startup
/// flag, the seed that names the decoupler or the decentralized design, the objective, and the budgets (`pidGroupLoopDefinition`).
///
/// # Errors
/// The block breaks a rule of the project decoder: its states, its seed, or the decoupler's shape.
fn group_project(
    draft: &GroupLoopDraft,
    first: &Member,
    states: &[Vec<InitializedState>],
    decoupler: Option<&[Vec<String>]>,
) -> Result<ControllerProject, Failure> {
    let mut declared: Vec<InitializedState> = states.iter().flatten().cloned().collect();
    declared.extend(startup_flag(draft).map(startup_state));
    let (family, seed) = match decoupler {
        Some(matrix) => (
            "decoupled-PI",
            json!({ "kind": "decoupled-pid", "family": "PI", "matrixSymbols": matrix }),
        ),
        None => (
            "paired-PI",
            json!({ "kind": "decentralized-pid", "family": "PI" }),
        ),
    };
    // These records always serialize; the default (null) could only be refused by the project decoder, never written silently.
    let block = json!({
        "family": family,
        "convention": "project-owned-native-controller",
        "initialization": {
            "kind": "declared-state-v1",
            "states": serde_json::to_value(&declared).unwrap_or_default(),
        },
        "seed": seed,
        // The group has no declared grid: each member's PI is designed on the group model, so there are no tuning axes.
        "axes": [],
        "objective": {
            "requirementId": format!("{}:target-error", first.process.symbol),
            "direction": "minimize",
        },
        "maximumCandidates": 1,
        "maximumCycles": 50_000,
        "wallBudgetMs": 60_000,
    });
    ControllerProject::from_value(&block).map_err(|message| refuse_settings(&message))
}

/// The members as the group declares them (C7): each member's instance, its channels, the group's shared mode, its three gain variables
/// and its retained states. The first member also owns the startup flag (`pidGroupLoopDefinition`).
fn group_block(
    draft: &GroupLoopDraft,
    members: &[Member],
    states: &[Vec<InitializedState>],
) -> PidControllerGroup {
    let startup = startup_flag(draft);
    let declared: Vec<PidGroupMember> = members
        .iter()
        .zip(states)
        .enumerate()
        .map(|(index, (member, member_states))| {
            let mut application: Vec<String> = member_states
                .iter()
                .map(|state| state.symbol.clone())
                .collect();
            if index == 0 {
                application.extend(startup.map(str::to_owned));
            }
            PidGroupMember {
                instance: member.instance.clone(),
                type_name: "Control.PID".to_owned(),
                input_channel_id: member.manipulated.symbol.clone(),
                output_channel_id: member.process.symbol.clone(),
                mode_symbol: draft.mode.symbol.clone(),
                parameter_symbols: member
                    .gains()
                    .into_iter()
                    .map(|parameter| parameter.symbol)
                    .collect(),
                application_state_symbols: Some(application),
            }
        })
        .collect();
    PidControllerGroup { members: declared }
}

/// The loop's signals (B1.4): the first member's setpoint, measurement and output are the loop's primary signals, and each later member
/// adds its output, measurement and setpoint as signals of its own channel `member-N`.
///
/// # Errors
/// A signal is not compiled, is not `REAL` or `LREAL`, or is a measurement the program computes.
fn group_signals(catalog: &CompiledCatalog, members: &[Member]) -> Result<Signals, Failure> {
    let first = &members[0];
    let mut additional = Vec::new();
    for (index, member) in members.iter().enumerate().skip(1) {
        additional.extend(member_signals(catalog, index, member)?);
    }
    Ok(Signals {
        setpoint: Some(binding(catalog, &first.setpoint, Direction::Read)?),
        process_variable: measurement_binding(catalog, &first.process)?,
        manipulated_variable: binding(catalog, &first.manipulated, Direction::Write)?,
        actuator: None,
        additional: Some(additional),
    })
}

/// The further signals of a member after the first, on its own channel: its output (`mv-N`), its measurement (`pv-N`) and its setpoint
/// (`sp-N`), where N is its place in the group.
///
/// # Errors
/// One of its signals is not compiled, is not `REAL` or `LREAL`, or is a measurement the program computes.
fn member_signals(
    catalog: &CompiledCatalog,
    index: usize,
    member: &Member,
) -> Result<[AuxiliarySignal; 3], Failure> {
    let channel = format!("member-{}", index + 1);
    Ok([
        auxiliary(
            signal_id("manipulatedVariable", "mv", index),
            channel.clone(),
            AuxiliaryRole::ManipulatedVariable,
            binding(catalog, &member.manipulated, Direction::Write)?,
        ),
        auxiliary(
            signal_id("processVariable", "pv", index),
            channel.clone(),
            AuxiliaryRole::ProcessVariable,
            measurement_binding(catalog, &member.process)?,
        ),
        auxiliary(
            signal_id("setpoint", "sp", index),
            channel,
            AuxiliaryRole::Setpoint,
            binding(catalog, &member.setpoint, Direction::Read)?,
        ),
    ])
}

/// A further signal of the loop, in its channel and with its role.
fn auxiliary(
    id: String,
    channel: String,
    role: AuxiliaryRole,
    signal: LoopSignal,
) -> AuxiliarySignal {
    AuxiliarySignal {
        id,
        channel,
        role,
        symbol: signal.symbol,
        address: signal.address,
        iec_type: signal.iec_type,
        direction: signal.direction,
        unit: signal.unit,
        range: signal.range,
    }
}

/// The identifier of a member's signal in the loop: the first member's is the loop's own primary signal, and a later member's is `mv-N`,
/// `pv-N` or `sp-N`, where N is its place in the group (`pidGroupLoopDefinition`).
fn signal_id(primary: &str, prefix: &str, index: usize) -> String {
    if index == 0 {
        primary.to_owned()
    } else {
        format!("{prefix}-{}", index + 1)
    }
}

/// The channels of the group (C7): one input per member, which drives its output, and one output per member, which measures its process
/// variable against its setpoint.
///
/// # Errors
/// A later member's manual output is not `REAL` or `LREAL`.
fn group_channels(members: &[Member]) -> Result<LoopChannels, Failure> {
    let inputs = members
        .iter()
        .enumerate()
        .map(|(index, member)| channel_input(index, member))
        .collect::<Result<Vec<_>, _>>()?;
    let outputs = members
        .iter()
        .enumerate()
        .map(|(index, member)| channel_output(index, member))
        .collect();
    Ok(LoopChannels {
        inputs,
        outputs,
        initial_state: None,
    })
}

/// One member's input: its output, its own identification step, and for a later member the Manual command that Identify steps.
///
/// # Errors
/// The member's manual output is not `REAL` or `LREAL`.
fn channel_input(index: usize, member: &Member) -> Result<ChannelInput, Failure> {
    let (manual_signal, manual_type) = if index == 0 {
        (None, None)
    } else {
        (
            Some(member.manual.symbol.clone()),
            Some(manual_output_type(real_type(&member.manual)?)),
        )
    };
    Ok(ChannelInput {
        model_channel_id: member.manipulated.symbol.clone(),
        signal_id: signal_id("manipulatedVariable", "mv", index),
        manual_output_signal: manual_signal,
        manual_output_type: manual_type,
        applied_signal_id: None,
        actuator: continuous_actuator(&member.manipulated),
        experiment_step: member.step,
    })
}

/// A continuous actuator over a signal's range: the plant input of a PID group (C7).
fn continuous_actuator(signal: &MeasuredSignal) -> ActuatorCapabilities {
    ActuatorCapabilities {
        kind: ActuatorKind::Continuous,
        minimum: signal.minimum,
        maximum: signal.maximum,
        rate_up_per_s: None,
        rate_down_per_s: None,
        minimum_on_seconds: None,
        minimum_off_seconds: None,
    }
}

/// One member's output: its measurement, and the setpoint that the member's signals name.
fn channel_output(index: usize, member: &Member) -> ChannelOutput {
    ChannelOutput {
        model_channel_id: member.process.symbol.clone(),
        signal_id: signal_id("processVariable", "pv", index),
        setpoint_signal_id: Some(signal_id("setpoint", "sp", index)),
        requirements: None,
    }
}

/// The bound parameters of the group (`parameterBinding`): every member's three gains, then the decoupler's entries.
///
/// # Errors
/// A decoupler variable is not compiled, or it is not `REAL` or `LREAL`.
fn group_parameters(
    catalog: &CompiledCatalog,
    members: &[Member],
    decoupler: Option<&[Vec<String>]>,
) -> Result<Vec<Parameter>, Failure> {
    let mut parameters: Vec<Parameter> = members.iter().flat_map(Member::gains).collect();
    parameters.extend(decoupler_parameters(catalog, members, decoupler)?);
    Ok(parameters)
}

/// The decoupler's entries as parameters (C7; numerics annex N10): each variable is a real of its own type, with the unit of the ratio
/// of its two outputs (`1` for one unit), and is bounded by ±1,000.
///
/// # Errors
/// A variable is not compiled, or it is not `REAL` or `LREAL`.
fn decoupler_parameters(
    catalog: &CompiledCatalog,
    members: &[Member],
    decoupler: Option<&[Vec<String>]>,
) -> Result<Vec<Parameter>, Failure> {
    let mut parameters = Vec::new();
    for (row, input) in decoupler.unwrap_or_default().iter().zip(members) {
        for (symbol, from) in row.iter().zip(members) {
            parameters.push(Parameter {
                symbol: symbol.clone(),
                iec_type: real_type(compiled(catalog, symbol)?)?,
                unit: unit_ratio(&input.manipulated.unit, &from.manipulated.unit),
                convention: format!(
                    "Decoupler entry for {} from {}",
                    input.manipulated.symbol, from.instance
                ),
                minimum: -DECOUPLER_BOUND,
                maximum: DECOUPLER_BOUND,
            });
        }
    }
    Ok(parameters)
}

/// The unit of a decoupler entry: `1` when the two outputs share a unit, else the ratio of the input's unit to the other output's.
fn unit_ratio(numerator: &str, denominator: &str) -> String {
    if numerator == denominator {
        "1".to_owned()
    } else {
        format!("{numerator}/{denominator}")
    }
}

/// The generated loop is read back as the loop file reader reads it (`LoopDefinition::decode`): its fields, its experiment, its channels
/// and its group. The save runs `LoopDefinition::validate` only, which does not run the reader's rules, so a group that the reader
/// refuses is refused here rather than written as a file that does not load.
///
/// # Errors
/// The loop file reader refuses the loop; the sentence is the reader's.
fn check_group(loop_definition: &LoopDefinition) -> Result<(), Failure> {
    let text = serde_json::to_string(loop_definition)
        .map_err(|error| refuse_settings(&error.to_string()))?;
    LoopDefinition::decode(&text)
        .map(|_| ())
        .map_err(|rejection| refuse_settings(&rejection.to_string()))
}

/// A settings refusal that the setup form answers by opening the loop setup (A11.2).
fn refuse_settings(text: &str) -> Failure {
    Failure::new(text, Cause::Settings, NextAction::OpenLoopSetup)
}

#[cfg(test)]
#[path = "group_loop_tests.rs"]
mod tests;
