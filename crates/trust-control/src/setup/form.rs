//! The setup form's model (specification 35, B1.3 and C0; `loopSetup/setupFormModel.ts`): what the compiled program shows, and only
//! the choices a loop can use. The model is read from the compiled catalog alone; no name is guessed from the program's text.
//!
//! The model files an LQG estimator can implement are listed beside this model in the discovery answer (`estimatorModels`, C4
//! Setup; `discovery::estimator_model_files`), because the engine reads them from the project's folders, not from the catalog.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::Serialize;

use super::catalog::{
    cascade_pairs, own_measurement, pid_candidates, pid_parameter_records, CascadePair,
    CompiledCatalog, CompiledSymbol, PidCandidate, PID_TYPE,
};
use super::group_loop;
use super::lqg_loop;
use super::own_law_loop::{self, OwnLawFamily};
use super::pid_bindings::startup_reason;
use super::pid_loop::pid_seeded_symbols;
use super::search_loop;
use super::state_feedback_loop;
use super::texts::form as words_of;
use crate::loop_definition::{BandConvention, ModeWhenTrue, ThresholdEquality};
use crate::tuning::state_controller::StateControllerFamily;
use crate::tuning::structures::gain_schedule::{MAXIMUM_POINTS, MINIMUM_POINTS};

/// What the program states for one library PID, offered as the form's preselection (B1.3). Only the signals the form offers are
/// named, so the engineer confirms every prefilled binding.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PidPrefill {
    /// The PID's instance.
    pub instance: String,
    /// The parameter record the PID's Configure call reads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_record: Option<String>,
    /// The output the PID's Update call writes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manipulated_variable: Option<String>,
    /// The setpoint the PID's Update call reads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setpoint: Option<String>,
    /// The measurement: the PID's own, or a signal the program reads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_value: Option<String>,
    /// The manual output the program steps by name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_output: Option<String>,
    /// The Auto/Manual signal the code tests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// What the mode signal's TRUE value means.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode_when_true: Option<ModeWhenTrue>,
    /// The flag the code tests in `IF NOT <flag>` around the PID's Initialize.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup_flag: Option<String>,
    /// A measurement the program computes, which a simulated measurement could not reach.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed: Option<ComputedPrefill>,
}

/// A measurement the program computes for a PID (B1.3).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ComputedPrefill {
    /// The computed signal's symbol.
    pub process_value: String,
}

/// The setup form's model of one compiled program (B1.3).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetupFormModel {
    /// The structure the program shows: `cascade`, `group`, `pid` or `identify`.
    pub structure: String,
    /// The library PIDs of the program, in the compiled order.
    pub pids: Vec<PidCandidate>,
    /// The outer and inner PID of a cascade: the first pair the program shows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cascade: Option<CascadePair>,
    /// The REAL and LREAL signals with an I/O address.
    pub addressed_real: Vec<String>,
    /// The REAL and LREAL signals a loop can use as an output or a setpoint, addressed or not; a PID's own records and states are
    /// not signals.
    pub signals: Vec<String>,
    /// The REAL and LREAL signals a simulated plant can write as a measurement: inputs, program variables without an address that
    /// the program only reads, and the measurement each library PID holds.
    pub measurements: Vec<String>,
    /// The BOOL symbols.
    pub booleans: Vec<String>,
    /// The BOOL symbols without an I/O address: the only ones the controller transfer switches.
    pub modes: Vec<String>,
    /// The parameter records of complete library PIDs.
    pub records: Vec<String>,
    /// The program and function block instances, other than library PIDs, that run in one cyclic task.
    pub instances: Vec<String>,
    /// Every REAL and LREAL symbol: Studio writes controller settings directly, so they need no address.
    pub reals: Vec<String>,
    /// The symbols the program assigns, as the compiler reports them, in the order it reports them.
    pub writes: Vec<String>,
    /// The library PIDs whose Initialize the program calls.
    pub initializes: Vec<String>,
    /// What each library PID's own calls say, for the controller the engineer chooses.
    pub prefills: Vec<PidPrefill>,
    /// The only PID's prefill, when the structure is `pid`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefill: Option<PidPrefill>,
    /// The structures the setup page offers, in the order it lists them: those a library PID's program can use, then, when the program has
    /// an instance of its own, the structures that run on it, and identify last (`loopSetupTemplate.ts`, B1.3).
    pub structures: Vec<String>,
    /// The PID instances whose program needs the startup-done flag at the settings the form starts from (C1 Setup): the program calls the
    /// PID's Initialize, or writes a value the setup seeds with the prefilled setpoint, manual output, mode or parameter record. The page
    /// shows the flag for a chosen instance in this list; a refusal that names the flag shows it whatever the list says.
    pub startup_flag_instances: Vec<String>,
    /// The choices the form preselects when the engineer has not chosen one (B1.3): the page shows them and keeps no default of its own.
    pub defaults: SetupFormDefaults,
    /// The row counts the form draws and the setup accepts (B1.3): the generators' own limits.
    pub limits: SetupFormLimits,
    /// The operating-point rows of the gain schedule's form, in the order the form draws them (B1.3, C10): one for each point a schedule
    /// may have, each saying whether the setup asks for it. The form draws these rows and keeps no count or flag of its own.
    pub schedule_points: Vec<SchedulePointRow>,
    /// The shape of the static decoupler a PID group may declare (B1.3, C7): one row for each plant input and one column for each member.
    /// The members are the program's library PIDs, so the matrix is square in them. The form draws one entry for each cell.
    pub decoupler: DecouplerShape,
    /// The loop name the form starts from for each library PID, in the order of `pids` (B1.3).
    pub loop_names: Vec<OfferedLoopName>,
    /// The words the form shows around a value (B1.3).
    pub wording: SetupFormWording,
    /// The values each choice of the form offers, in the order the page lists them (B1.3); the words of each value are `labels`.
    pub choices: SetupFormChoices,
    /// The words of each value the form offers (B1.3): for `structures` and for each list of `choices`, the words the page shows for
    /// each of its values, keyed by the value. The page shows these words and keeps none of its own.
    pub labels: SetupFormLabels,
}

/// The values of the choices the setup form offers (B1.3): what the draft reads for each choice, in the order the page lists them. Each value
/// is one the draft reads; `form_tests.rs` pins the lists to the enums and the family list they come from. Their words are
/// [`SetupFormLabels`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetupFormChoices {
    /// What an Auto/Manual signal's TRUE value means (`modeWhenTrue`).
    pub mode_when_true: Vec<String>,
    /// The hysteresis bands of an ON/OFF controller (`bandConvention`).
    pub band_conventions: Vec<String>,
    /// The threshold-equality policies of an ON/OFF controller (`thresholdEquality`).
    pub threshold_equalities: Vec<String>,
    /// The designs of state feedback (`family` of the state-feedback structure).
    pub state_families: Vec<String>,
    /// The laws of a program's own controller (`law`).
    pub own_laws: Vec<String>,
    /// Where a program's own PID law takes its derivative (`derivativeOn`).
    pub derivative_on: Vec<String>,
    /// The laws a finite search may name (`family` of the search structure).
    pub search_families: Vec<String>,
    /// Where a kept value of a finite search starts (`start`).
    pub kept_starts: Vec<String>,
    /// The controller actions (`action`).
    pub actions: Vec<String>,
}

/// The words of each value the setup form offers (B1.3), one map per list, keyed by the value: the structures the program offers and the
/// values of [`SetupFormChoices`]. Every value a list offers has its words here, and only those values do (`form_tests.rs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetupFormLabels {
    /// The words of each structure the program offers (`structures`).
    pub structures: BTreeMap<String, String>,
    /// The words of what an Auto/Manual signal's TRUE value means.
    pub mode_when_true: BTreeMap<String, String>,
    /// The words of the hysteresis bands of an ON/OFF controller.
    pub band_conventions: BTreeMap<String, String>,
    /// The words of the threshold-equality policies of an ON/OFF controller.
    pub threshold_equalities: BTreeMap<String, String>,
    /// The words of the designs of state feedback.
    pub state_families: BTreeMap<String, String>,
    /// The words of the laws of a program's own controller.
    pub own_laws: BTreeMap<String, String>,
    /// The words of where a program's own PID law takes its derivative.
    pub derivative_on: BTreeMap<String, String>,
    /// The words of the laws a finite search may name.
    pub search_families: BTreeMap<String, String>,
    /// The words of where a kept value of a finite search starts.
    pub kept_starts: BTreeMap<String, String>,
    /// The words of the controller actions.
    pub actions: BTreeMap<String, String>,
}

/// The choices the setup form preselects when the engineer has not chosen one (B1.3). Each word is one the engine's own draft reads back;
/// `form_tests.rs` pins that.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetupFormDefaults {
    /// The structure the form preselects: the one the program shows, so a program with several library PIDs preselects `group`.
    pub structure: String,
    /// The state-feedback design the form preselects (C4): `LQR`.
    pub state_family: String,
    /// The law family the form preselects for a finite search (C5): `MPC`.
    pub search_family: String,
    /// Where a kept value of a finite search starts when the engineer does not choose it (C5): `zero`.
    pub search_kept_start: String,
    /// What an Auto/Manual signal's TRUE value means when the engineer does not choose it (B1.3): `Auto`.
    pub mode_when_true: String,
    /// The controller action the form preselects (B1.3): `direct`.
    pub action: String,
    /// The band convention of an ON/OFF controller the form preselects (C3): `half-band`.
    pub band_convention: String,
    /// The threshold-equality policy of an ON/OFF controller the form preselects (C3, D-10): `at-or-beyond`.
    pub threshold_equality: String,
    /// The law an own-law setup preselects (C2): `PI`.
    pub own_law: String,
    /// The law a trajectory feedforward's program computes, which the form does not offer to change (C11): `PID`.
    pub trajectory_own_law: String,
}

/// The row counts the setup form draws and the setup accepts (B1.3): the generators' own limits, so the page and the engine cannot disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetupFormLimits {
    /// The further measured outputs a state-feedback law takes (C4).
    pub further_outputs: usize,
    /// The states of an LQG estimator's model, one estimate row each (C4).
    pub estimator_states: usize,
    /// The coefficients a finite search varies (C5).
    pub search_coefficients: usize,
    /// The values a finite search keeps from scan to scan (C5).
    pub search_kept_values: usize,
    /// The fewest members of a PID group (C7). A group has no most members: its decoupler has one row and one column per member.
    pub group_members_minimum: usize,
}

/// One operating-point row of the gain schedule's form (B1.3, C10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SchedulePointRow {
    /// Whether the setup asks for this point. A schedule has two to four points: the form always shows the first two, and offers each
    /// further one once the point before it has a name.
    pub required: bool,
}

/// The shape of the static decoupler the form asks for (B1.3, C7): a matrix with a row for each plant input and a column for each member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DecouplerShape {
    /// The rows of the matrix: one for each plant input.
    pub rows: usize,
    /// The columns of the matrix: one for each member.
    pub columns: usize,
}

/// The words the setup form shows around a value (B1.3).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetupFormWording {
    /// What an Auto/Manual signal's TRUE value means: the label before the mode that the signal's TRUE value chooses.
    pub true_means: String,
    /// The line a found value shows when the code states none.
    pub not_found: String,
    /// The line a task shows when the code states none.
    pub not_stated: String,
}

/// The loop name the form starts from for a library PID (B1.3): the last segment of its instance's path. The engineer may type another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OfferedLoopName {
    /// The PID's instance.
    pub instance: String,
    /// The loop name the form starts from.
    pub name: String,
}

/// The setup form's model of a compiled program (B1.3, `setupFormModel`). The structure is `cascade` when one PID's output is the
/// other's setpoint, `group` for several PIDs, `pid` for one, and `identify` when the program has no library PID.
#[must_use]
#[allow(clippy::too_many_lines)] // One form model, built field by field in the order setupFormModel.ts builds it.
pub fn setup_form_model(catalog: &CompiledCatalog) -> SetupFormModel {
    let pids = pid_candidates(catalog);
    let pairs = cascade_pairs(&pids);
    let records = pid_parameter_records(catalog);
    let written: BTreeSet<String> = catalog
        .writes
        .iter()
        .map(|symbol| symbol.to_ascii_uppercase())
        .collect();
    let owners: Vec<&str> = records
        .iter()
        .map(String::as_str)
        .chain(pids.iter().map(|pid| pid.instance.as_str()))
        .collect();
    let internal = |symbol: &str| {
        owners.iter().any(|owner| {
            symbol
                .strip_prefix(*owner)
                .is_some_and(|rest| rest.starts_with('.'))
        })
    };
    let addressed_real = sorted(
        catalog
            .symbols
            .iter()
            .filter(|entry| is_real(entry) && has_address(entry))
            .map(symbol_of)
            .collect(),
    );
    let booleans = sorted(
        catalog
            .symbols
            .iter()
            .filter(|entry| entry.iec_type == "BOOL")
            .map(symbol_of)
            .collect(),
    );
    let modes = sorted(
        catalog
            .symbols
            .iter()
            .filter(|entry| entry.iec_type == "BOOL" && !has_address(entry))
            .map(symbol_of)
            .collect(),
    );
    let reals = sorted(
        catalog
            .symbols
            .iter()
            .filter(|entry| is_real(entry))
            .map(symbol_of)
            .collect(),
    );
    let signals = sorted(
        catalog
            .symbols
            .iter()
            .filter(|entry| is_real(entry) && !internal(&entry.symbol))
            .map(symbol_of)
            .collect(),
    );
    // A PID's own measurement is what its Update received, wherever the program computed it from (a sensor block, a filter).
    let mut measurements: Vec<String> = catalog
        .symbols
        .iter()
        .filter(|entry| {
            is_real(entry) && !internal(&entry.symbol) && is_measurement(entry, &written)
        })
        .map(symbol_of)
        .collect();
    measurements.extend(pids.iter().filter_map(|pid| pid.own_measurement.clone()));
    let measurements = sorted(measurements);
    let instances = sorted(
        catalog
            .controllers
            .iter()
            .filter(|controller| {
                controller.type_name.to_ascii_uppercase() != PID_TYPE
                    && matches!(controller.tasks.as_slice(), [task] if task.interval_ms > 0.0)
            })
            .map(|controller| controller.instance.clone())
            .collect(),
    );
    let initializes = sorted(
        pids.iter()
            .filter(|pid| {
                catalog.links.iter().any(|link| {
                    link.instance == pid.instance && link.method.eq_ignore_ascii_case("INITIALIZE")
                })
            })
            .map(|pid| pid.instance.clone())
            .collect(),
    );
    let structure = if !pairs.is_empty() {
        "cascade"
    } else if pids.len() > 1 {
        "group"
    } else if pids.len() == 1 {
        "pid"
    } else {
        "identify"
    };
    let lists = Lists {
        records: &records,
        signals: &signals,
        measurements: &measurements,
        modes: &modes,
        booleans: &booleans,
    };
    let prefills: Vec<PidPrefill> = pids.iter().map(|pid| prefill(pid, &lists)).collect();
    let single = if structure == "pid" {
        prefills.first().cloned()
    } else {
        None
    };
    let structures = offered_structures(&pids, &instances);
    let startup_flag_instances = startup_flag_instances(catalog, &prefills);
    let loop_names = offered_loop_names(&pids);
    let decoupler = decoupler_shape(pids.len());
    let choices = choices();
    let labels = labels(&structures, &choices);
    SetupFormModel {
        structure: structure.to_string(),
        pids,
        cascade: pairs.first().cloned(),
        addressed_real,
        signals,
        measurements,
        booleans,
        modes,
        records,
        instances,
        reals,
        writes: catalog.writes.clone(),
        initializes,
        prefills,
        prefill: single,
        structures,
        startup_flag_instances,
        defaults: defaults(structure),
        limits: limits(),
        schedule_points: schedule_point_rows(),
        decoupler,
        loop_names,
        wording: wording(),
        choices,
        labels,
    }
}

/// The structures the setup page offers, in its order (`loopSetupTemplate.ts`): those a library PID's program can use, then, when the program
/// has an instance of its own, the structures that run on it, and identify last. The engine decides which exist, and gives each its words
/// (`labels`).
fn offered_structures(pids: &[PidCandidate], instances: &[String]) -> Vec<String> {
    let mut structures = vec!["pid", "cascade", "feedforward", "smith", "schedule"];
    if pids.len() > 1 {
        structures.push("group");
    }
    if !instances.is_empty() {
        structures.extend([
            "onoff",
            "state-feedback",
            "own-law",
            "trajectory-feedforward",
            "search",
        ]);
    }
    structures.push("identify");
    structures.into_iter().map(str::to_owned).collect()
}

/// The PID instances whose program needs the startup-done flag at the settings the form starts from (C1 Setup): the program calls the
/// PID's Initialize, or writes a value the setup seeds with the prefilled setpoint, manual output, mode and parameter record. The rule is
/// `startup_reason`; the choices the engineer makes later are checked when the loop is saved.
fn startup_flag_instances(catalog: &CompiledCatalog, prefills: &[PidPrefill]) -> Vec<String> {
    prefills
        .iter()
        .filter(|prefill| {
            let own = prefill.process_value.as_deref().is_some_and(|value| {
                value.eq_ignore_ascii_case(&own_measurement(&prefill.instance))
            });
            let seeded = pid_seeded_symbols(
                &prefill.instance,
                [
                    prefill.setpoint.as_deref().unwrap_or_default(),
                    prefill.manual_output.as_deref().unwrap_or_default(),
                    prefill.mode.as_deref().unwrap_or_default(),
                ],
                own,
            );
            let records: Vec<String> = prefill.parameter_record.iter().cloned().collect();
            startup_reason(catalog, Some(prefill.instance.as_str()), &seeded, &records).is_some()
        })
        .map(|prefill| prefill.instance.clone())
        .collect()
}

/// The choices the form preselects (B1.3). The state-feedback and own-law words are the generators' own names; the others are the words
/// their drafts read.
fn defaults(structure: &str) -> SetupFormDefaults {
    SetupFormDefaults {
        structure: structure.to_string(),
        state_family: state_feedback_loop::family_name(StateControllerFamily::Lqr).to_string(),
        search_family: "MPC".to_string(),
        search_kept_start: "zero".to_string(),
        mode_when_true: "Auto".to_string(),
        action: "direct".to_string(),
        band_convention: BandConvention::HalfBand.word().to_string(),
        threshold_equality: ThresholdEquality::AtOrBeyond.word().to_string(),
        own_law: own_law_loop::family_name(OwnLawFamily::Pi).to_string(),
        trajectory_own_law: own_law_loop::family_name(OwnLawFamily::Pid).to_string(),
    }
}

/// The row counts the form draws and the setup accepts: the generators' own limits (B1.3).
fn limits() -> SetupFormLimits {
    SetupFormLimits {
        further_outputs: state_feedback_loop::MAXIMUM_FURTHER_OUTPUTS,
        estimator_states: lqg_loop::MAXIMUM_ESTIMATOR_STATES,
        search_coefficients: search_loop::MAXIMUM_COEFFICIENTS,
        search_kept_values: search_loop::MAXIMUM_KEPT_VALUES,
        group_members_minimum: group_loop::MINIMUM_MEMBERS,
    }
}

/// The operating-point rows of the gain schedule's form (C10): one for each point a schedule may have, the fewest of them required.
fn schedule_point_rows() -> Vec<SchedulePointRow> {
    (0..MAXIMUM_POINTS)
        .map(|index| SchedulePointRow {
            required: index < MINIMUM_POINTS,
        })
        .collect()
}

/// The decoupler a PID group may declare (C7): a row for each plant input and a column for each member, and a member for each library PID
/// the program holds.
fn decoupler_shape(members: usize) -> DecouplerShape {
    DecouplerShape {
        rows: members,
        columns: members,
    }
}

/// The choice lists of the form (B1.3): the values the draft reads, each list in the order the page lists it.
fn choices() -> SetupFormChoices {
    SetupFormChoices {
        mode_when_true: words(&["Auto", "Manual"]),
        band_conventions: words(&["half-band", "full-band"]),
        threshold_equalities: words(&["at-or-beyond", "beyond"]),
        state_families: words(&["LQR", "state-feedback", "LQG"]),
        own_laws: words(&["PI", "P", "PID"]),
        derivative_on: words(&["error", "measurement"]),
        search_families: words(&search_loop::SEARCH_FAMILIES),
        kept_starts: words(&["zero", "output", "measurement"]),
        actions: words(&["direct", "reverse"]),
    }
}

/// The owned words of a list of values.
fn words(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// The words of each value the form offers (B1.3): the offered `structures` and each list of `choices`, with the words of
/// `texts::form`. A value without words there is shown as itself, never left out; `form_tests.rs` pins that every value has its words.
fn labels(structures: &[String], choices: &SetupFormChoices) -> SetupFormLabels {
    SetupFormLabels {
        structures: labelled(structures, &words_of::STRUCTURES),
        mode_when_true: labelled(&choices.mode_when_true, &words_of::MODE_WHEN_TRUE),
        band_conventions: labelled(&choices.band_conventions, &words_of::BAND_CONVENTIONS),
        threshold_equalities: labelled(
            &choices.threshold_equalities,
            &words_of::THRESHOLD_EQUALITIES,
        ),
        state_families: labelled(&choices.state_families, &words_of::STATE_FAMILIES),
        own_laws: labelled(&choices.own_laws, &words_of::OWN_LAWS),
        derivative_on: labelled(&choices.derivative_on, &words_of::DERIVATIVE_ON),
        search_families: labelled(&choices.search_families, &words_of::SEARCH_FAMILIES),
        kept_starts: labelled(&choices.kept_starts, &words_of::KEPT_STARTS),
        actions: labelled(&choices.actions, &words_of::ACTIONS),
    }
}

/// Each of `values` with its words in `words`, or with itself when `words` has none.
fn labelled(values: &[String], words: &[(&str, &str)]) -> BTreeMap<String, String> {
    values
        .iter()
        .map(|value| {
            let text = words
                .iter()
                .find(|(word, _)| word == value)
                .map_or(value.as_str(), |(_, text)| *text);
            (value.clone(), text.to_owned())
        })
        .collect()
}

/// The words the form shows around a value (B1.3).
fn wording() -> SetupFormWording {
    SetupFormWording {
        true_means: "TRUE means".to_string(),
        not_found: "Not found in the code".to_string(),
        not_stated: "Not stated in the code".to_string(),
    }
}

/// The loop name the form starts from for each library PID: the last segment of its instance's path, so `P1.Controller` names
/// `Controller` (B1.3).
fn offered_loop_names(pids: &[PidCandidate]) -> Vec<OfferedLoopName> {
    pids.iter()
        .map(|pid| OfferedLoopName {
            instance: pid.instance.clone(),
            name: pid
                .instance
                .rsplit('.')
                .next()
                .unwrap_or_default()
                .to_string(),
        })
        .collect()
}

/// The lists a prefill checks its names against: only what the form offers is preselected.
struct Lists<'a> {
    records: &'a [String],
    signals: &'a [String],
    measurements: &'a [String],
    modes: &'a [String],
    booleans: &'a [String],
}

/// What a library PID's own calls say, restricted to what the form offers (`setupFormModel`, prefills).
fn prefill(pid: &PidCandidate, lists: &Lists<'_>) -> PidPrefill {
    let mut prefill = PidPrefill {
        instance: pid.instance.clone(),
        parameter_record: offered(pid.parameter_record.as_ref(), lists.records),
        manipulated_variable: offered(pid.output.as_ref(), lists.signals),
        setpoint: offered(pid.setpoint.as_ref(), lists.signals),
        process_value: None,
        manual_output: offered(pid.manual_output.as_ref(), lists.signals),
        mode: None,
        mode_when_true: None,
        startup_flag: offered(pid.startup_flag.as_ref(), lists.booleans),
        computed: None,
    };
    if let Some(own) = &pid.own_measurement {
        prefill.process_value = Some(own.clone());
    } else if let Some(value) = offered(pid.process_value.as_ref(), lists.measurements) {
        prefill.process_value = Some(value);
    } else if let Some(value) = offered(pid.process_value.as_ref(), lists.signals) {
        prefill.computed = Some(ComputedPrefill {
            process_value: value,
        });
    }
    if let Some(mode) = pid
        .mode
        .as_ref()
        .filter(|mode| lists.modes.contains(&mode.symbol))
    {
        prefill.mode = Some(mode.symbol.clone());
        prefill.mode_when_true = Some(mode.when_true);
    }
    prefill
}

/// The choice when the offered list names it.
fn offered(choice: Option<&String>, list: &[String]) -> Option<String> {
    choice.filter(|name| list.contains(*name)).cloned()
}

/// Whether a symbol is a REAL or LREAL (`setupFormModel`: the type is compared as the compiler writes it).
fn is_real(entry: &CompiledSymbol) -> bool {
    entry.iec_type == "REAL" || entry.iec_type == "LREAL"
}

/// Whether a symbol has an I/O address (an empty address is no address).
fn has_address(entry: &CompiledSymbol) -> bool {
    entry
        .address
        .as_deref()
        .is_some_and(|address| !address.is_empty())
}

/// Whether a REAL can be a simulated measurement: an input (an address starting `%I`), or a program variable without an address
/// that the program only reads.
fn is_measurement(entry: &CompiledSymbol, written: &BTreeSet<String>) -> bool {
    match entry.address.as_deref() {
        Some(address) if !address.is_empty() => address.to_ascii_uppercase().starts_with("%I"),
        _ => !written.contains(&entry.symbol.to_ascii_uppercase()),
    }
}

/// The symbol of a compiled entry.
fn symbol_of(entry: &CompiledSymbol) -> String {
    entry.symbol.clone()
}

/// The names in order.
fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

#[cfg(test)]
#[path = "form_tests.rs"]
mod tests;
