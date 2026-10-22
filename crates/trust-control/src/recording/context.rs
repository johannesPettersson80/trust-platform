//! The context of the run that made a recording (specification 35, B10.2 and B10.3): the experiments the run wrote and how each one
//! ended, and the simulated plant the run ran against. A result keeps them when they apply: a run on a physical target has no
//! simulated plant, and a run that wrote nothing has no experiment. The recording file keeps the context beside the run, so a
//! recording from an aborted test stays labelled with its stop reason and its restoration after it is reloaded (B10.3), and the JSON
//! export writes it (B10.5).
//!
//! The context is a fact of the run, taken when the run happens: [`ExperimentContext::of`] reads the plan the engineer reviewed and
//! the runtime's last status of the experiment (annex E5.6), and [`PlantContext::of`] reads the runtime's saved plant configuration and
//! its active status (specification 34, §20.2 to §20.4). A saved result the engine opens keeps the context it states
//! ([`saved_context`]). The types hold no tagged, untagged or flattened enum, because a recording file decodes without
//! [`crate::json`].

#![allow(clippy::module_name_repetitions)] // The types name their concept: the run context.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::experiment::number_text::js_number;
use crate::experiment::{
    ControlExperimentKind, ControlExperimentPlan, ControlExperimentPoint, ExperimentTerminalStatus,
    ExperimentValue, PlanSource,
};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;
use crate::recording::EvidenceSource;
use crate::study::canonical_json;

/// The approval of an experiment on a simulated target: pressing the action is the confirmation (A2.5).
pub const SIMULATION_APPROVAL: &str = "Simulation confirmation";
/// The approval of an experiment on a physical target: the engineer's explicit approval of the reviewed topology (A2.5, annex E10.2).
pub const PHYSICAL_APPROVAL: &str = "Explicit physical approval after topology review";
/// What a plant context says of an actuator that the plant's configuration does not declare.
pub const NO_ACTUATOR: &str = "Not declared";

/// The most points a commanded trajectory keeps, as a stored plan keeps them.
const MAXIMUM_TRAJECTORY_POINTS: usize = 72_000;
/// The most frequencies a multisine keeps.
const MAXIMUM_FREQUENCIES: usize = 64;
/// The most stop conditions, uncertainties or scheduled events a context lists.
const MAXIMUM_TEXTS: usize = 128;

/// The context of the run that made a recording (B10.2): the experiments it wrote, in the order the campaign stepped them, and the
/// plant it ran against.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunContext {
    /// The experiments the run wrote. Absent where the run wrote none, as an observation does not.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub experiments: Vec<ExperimentContext>,
    /// The simulated plant the run ran against. Absent where the run had none to state: a physical target has no simulation
    /// configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plant: Option<PlantContext>,
}

/// One experiment of a run and how it ended (B10.2): what the plan asked for, and what the runtime reported at its end. The plan's
/// fields are absent for an experiment that a saved result states without them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExperimentContext {
    /// The kind of the experiment.
    pub kind: ControlExperimentKind,
    /// The signal the experiment wrote, by symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affected_signal: Option<String>,
    /// The signal's address; absent for a variable written by name (specification 35, §17.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// The value the plan asked for; a Boolean is 0 or 1.
    pub requested_value: f64,
    /// The value the runtime applied last and read back (annex E5.6, `last_applied`). Absent where the runtime reported none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_value: Option<f64>,
    /// The duration of the experiment, in ms of process time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
    /// The interval between the writes of a pseudo-random sequence, in ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_ms: Option<f64>,
    /// The seed of a pseudo-random sequence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,
    /// The frequencies of a multisine, in Hz.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequencies_hz: Option<Vec<f64>>,
    /// The complete commanded trajectory: each value written and the process time it was written at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trajectory: Option<Vec<ControlExperimentPoint>>,
    /// The conditions that stop the experiment early, as the engineer read them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_conditions: Option<Vec<String>>,
    /// The smallest value the experiment may write.
    pub minimum: f64,
    /// The largest value the experiment may write.
    pub maximum: f64,
    /// Who approved the experiment: [`SIMULATION_APPROVAL`] or [`PHYSICAL_APPROVAL`].
    pub approval: String,
    /// How the experiment ended (annex E5.6).
    pub status: ExperimentTerminalStatus,
    /// Why the experiment stopped (annex E5.7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    /// Whether the experiment was aborted.
    pub aborted: bool,
    /// The restoration of the written signal to its original value.
    pub restoration: RestorationContext,
}

/// The restoration of an experiment (annex E5.4, E5.6): the value restored, whether the runtime confirmed it, and why it did not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestorationContext {
    /// The value the restoring write returns the signal to; a Boolean is 0 or 1.
    pub value: f64,
    /// Whether the runtime confirmed the restoration.
    pub status: RestorationResult,
    /// Why the restoration failed, when the runtime says so (`confirmation_timeout`, or the stop reason of a reload or restart).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// How an experiment's restoration ended (annex E5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RestorationResult {
    /// The runtime confirmed that the signal reads its original value again.
    Confirmed,
    /// The restoration was not confirmed.
    Failed,
    /// The experiment wrote nothing, so there was nothing to restore.
    NotRequired,
}

/// The plant a run ran against (B10.2): its identity, the configuration the runtime proved it runs, the seed its randomness was
/// drawn with, and what the configuration says of its uncertainty, its scheduled events and its actuator. The evidence source of the
/// recording decides whether the plant is simulated (A9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlantContext {
    /// The plant's identifier, which the loop names.
    pub id: String,
    /// The revision of the saved configuration the plant runs under (specification 34, §20.3).
    pub configuration_revision: String,
    /// The plant's entry of the simulation configuration, as it was saved. Absent for a plant that is not simulated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<Value>,
    /// The reviewed physical topology the plant stands for. Present for a plant that is not simulated, and absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physical_topology_reference: Option<String>,
    /// The plant's model kind.
    pub model: String,
    /// The seed the simulation's randomness was drawn with, as a decimal text (specification 33, §3.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_seed: Option<String>,
    /// The plant's uncertainties, one canonical JSON text each.
    pub uncertainties: Vec<String>,
    /// The events scheduled for the plant, one canonical JSON text each.
    pub scheduled_events: Vec<String>,
    /// The plant's actuator behaviour as a canonical JSON text, or [`NO_ACTUATOR`].
    pub actuator: String,
    /// The range the loop declares the plant's model valid over: the measured output and the plant input.
    pub validity_range: String,
}

impl RunContext {
    /// Whether the context states nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.experiments.is_empty() && self.plant.is_none()
    }

    /// Checks every part of the context against the evidence source of its recording (B10.2, B10.4).
    ///
    /// # Errors
    /// A part breaks a rule of [`ExperimentContext::check`] or [`PlantContext::check`].
    pub fn check(&self, source: EvidenceSource) -> Result<(), Failure> {
        for experiment in &self.experiments {
            experiment.check()?;
        }
        self.plant
            .as_ref()
            .map_or(Ok(()), |plant| plant.check(source))
    }

    /// The context without the parts that break a rule, or `None` when nothing is left. A recording is kept whatever its context
    /// says: a part that cannot be kept is left out, and the recording stays (B10.3).
    #[must_use]
    pub fn checked(self, source: EvidenceSource) -> Option<Self> {
        let experiments: Vec<ExperimentContext> = self
            .experiments
            .into_iter()
            .filter(|experiment| experiment.check().is_ok())
            .collect();
        let plant = self.plant.filter(|plant| plant.check(source).is_ok());
        let context = Self { experiments, plant };
        (!context.is_empty()).then_some(context)
    }
}

impl ExperimentContext {
    /// The context of an experiment that has ended: the plan the engineer reviewed and the runtime's last status of it (annex E5.6).
    /// `None` while the status does not describe an ended experiment: a running one, or one whose restoration is still pending.
    #[must_use]
    pub fn of(plan: &ControlExperimentPlan, status: &Value) -> Option<Self> {
        let ended: ExperimentTerminalStatus =
            serde_json::from_value(status.get("status")?.clone()).ok()?;
        let restoration = match status.get("restoration")?.as_str()? {
            "restored" => RestorationResult::Confirmed,
            "not_required" => RestorationResult::NotRequired,
            "failed" => RestorationResult::Failed,
            _ => return None,
        };
        let applied_value = status
            .get("last_applied")
            .and_then(|applied| applied.get("value"))
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite());
        Some(Self {
            kind: plan.kind,
            affected_signal: Some(plan.affected_signal.clone()),
            address: plan.address.clone(),
            requested_value: number(plan.to),
            applied_value,
            duration_ms: Some(plan.duration_ms),
            interval_ms: plan.interval_ms,
            seed: plan.seed,
            frequencies_hz: plan.frequencies_hz.clone(),
            trajectory: Some(plan.trajectory.clone()),
            stop_conditions: Some(plan.stop_conditions.clone()),
            minimum: plan.minimum,
            maximum: plan.maximum,
            approval: match plan.source {
                PlanSource::Simulation => SIMULATION_APPROVAL,
                PlanSource::Physical => PHYSICAL_APPROVAL,
            }
            .to_owned(),
            status: ended,
            stop_reason: text_of(status, "stop_reason"),
            aborted: ended == ExperimentTerminalStatus::Aborted,
            restoration: RestorationContext {
                value: number(plan.restoration_value),
                status: restoration,
                reason: text_of(status, "restoration_reason"),
            },
        })
    }

    /// Checks the rules of an experiment's context (B10.2, B10.4): its numbers are finite and its bounds ordered, its durations,
    /// intervals and frequencies are positive, its trajectory and its texts are well formed, and its abort state follows from its
    /// status.
    ///
    /// # Errors
    /// The context breaks one of these rules; the failure names it.
    pub fn check(&self) -> Result<(), Failure> {
        let numbers = [
            self.requested_value,
            self.minimum,
            self.maximum,
            self.restoration.value,
        ];
        if numbers.iter().any(|number| !number.is_finite())
            || self.applied_value.is_some_and(|value| !value.is_finite())
            || self.minimum > self.maximum
        {
            return Err(invalid(
                "an experiment's values are not finite numbers, or its bounds are not ordered",
            ));
        }
        if self.duration_ms.is_some_and(|value| !positive(value))
            || self.interval_ms.is_some_and(|value| !positive(value))
            || self.frequencies_hz.as_ref().is_some_and(|frequencies| {
                frequencies.len() > MAXIMUM_FREQUENCIES
                    || frequencies.iter().any(|frequency| !positive(*frequency))
            })
        {
            return Err(invalid(
                "an experiment's duration, interval or frequencies are not positive finite numbers",
            ));
        }
        if self.trajectory.as_ref().is_some_and(|points| {
            points.len() > MAXIMUM_TRAJECTORY_POINTS
                || points.iter().any(|point| {
                    !point.value.is_finite() || !point.at_ms.is_finite() || point.at_ms < 0.0
                })
        }) {
            return Err(invalid(
                "an experiment's trajectory holds a point that is not a finite value at a time from zero",
            ));
        }
        if self.stop_conditions.as_ref().is_some_and(|conditions| {
            conditions.len() > MAXIMUM_TEXTS || conditions.iter().any(|text| blank(text))
        }) {
            return Err(invalid(
                "an experiment's stop conditions are too many or blank",
            ));
        }
        let texts = [
            self.affected_signal.as_deref(),
            self.address.as_deref(),
            self.stop_reason.as_deref(),
            self.restoration.reason.as_deref(),
        ];
        if blank(&self.approval) || texts.into_iter().flatten().any(blank) {
            return Err(invalid(
                "an experiment's approval, signal, address or reasons are blank",
            ));
        }
        if self.aborted != (self.status == ExperimentTerminalStatus::Aborted) {
            return Err(invalid(
                "an experiment's abort state contradicts the status it ended with",
            ));
        }
        Ok(())
    }
}

impl PlantContext {
    /// The simulated plant that `loop_definition` is tuned against, as the runtime proves it runs it (B10.2; specification 34, §20.2
    /// to §20.4). `read` is the runtime's `plant.read` answer and `status` its `plant.status` answer. `None` unless the loop names a
    /// plant, the saved configuration describes it, and the runtime reports the plant active under the revision of that saved
    /// configuration: a configuration the runtime does not run is not evidence of the run.
    #[must_use]
    pub fn of(loop_definition: &LoopDefinition, read: &Value, status: &Value) -> Option<Self> {
        let plant_id = loop_definition
            .plant_id
            .as_deref()
            .filter(|id| !blank(id))?;
        let revision = non_blank(read.get("configuration_revision"))?;
        if non_blank(status.get("active_configuration_revision"))? != revision {
            return None;
        }
        let effective_seed = non_blank(status.get("effective_seed"))?;
        let active = status
            .get("plants")?
            .as_array()?
            .iter()
            .any(|plant| named(plant, plant_id));
        if !active {
            return None;
        }
        let saved = read.get("saved_configuration")?;
        let entry = saved
            .get("plants")?
            .as_array()?
            .iter()
            .find(|plant| named(plant, plant_id))?;
        let model = non_blank(entry.get("model"))?;
        let configuration = crate::json::decode_value::<Value>(entry.clone()).ok()?;
        let uncertainties: Vec<String> = entry
            .get("uncertainties")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(canonical_json).collect())
            .unwrap_or_default();
        let scheduled_events: Vec<String> = saved
            .get("plant_events")
            .and_then(Value::as_array)
            .map(|events| {
                events
                    .iter()
                    .filter(|event| event.get("plant").and_then(Value::as_str) == Some(plant_id))
                    .map(canonical_json)
                    .collect()
            })
            .unwrap_or_default();
        let actuator = entry
            .get("actuator")
            .filter(|actuator| actuator.is_object())
            .map_or_else(|| NO_ACTUATOR.to_owned(), canonical_json);
        Some(Self {
            id: plant_id.to_owned(),
            configuration_revision: revision.to_owned(),
            configuration: Some(configuration),
            physical_topology_reference: None,
            model: model.to_owned(),
            effective_seed: Some(effective_seed.to_owned()),
            uncertainties,
            scheduled_events,
            actuator,
            validity_range: validity_range(loop_definition),
        })
    }

    /// Checks the rules of a plant's context against the evidence source of its recording (B10.2, B10.4, A9): its identity and texts
    /// are named, and a simulated plant states its configuration and no physical topology, as a plant that is not simulated states
    /// the topology and no configuration.
    ///
    /// # Errors
    /// The context breaks one of these rules; the failure names it.
    pub fn check(&self, source: EvidenceSource) -> Result<(), Failure> {
        let identity = [
            self.id.as_str(),
            self.configuration_revision.as_str(),
            self.model.as_str(),
            self.actuator.as_str(),
            self.validity_range.as_str(),
        ];
        if identity.into_iter().any(blank) || self.effective_seed.as_deref().is_some_and(blank) {
            return Err(invalid(
                "a plant's identifier, revision, model, seed, actuator or validity range is blank",
            ));
        }
        if self.uncertainties.len() > MAXIMUM_TEXTS
            || self.scheduled_events.len() > MAXIMUM_TEXTS
            || self
                .uncertainties
                .iter()
                .chain(&self.scheduled_events)
                .any(|text| blank(text))
        {
            return Err(invalid(
                "a plant's uncertainties or scheduled events are too many or blank",
            ));
        }
        let consistent = match source {
            EvidenceSource::SimulatedPlant => {
                self.configuration.as_ref().is_some_and(Value::is_object)
                    && self.physical_topology_reference.is_none()
            }
            EvidenceSource::PhysicalPlant => {
                self.configuration.is_none()
                    && self
                        .physical_topology_reference
                        .as_deref()
                        .is_some_and(|reference| !blank(reference))
            }
        };
        if consistent {
            Ok(())
        } else {
            Err(invalid(
                "a plant's configuration and topology do not fit the evidence source of its recording",
            ))
        }
    }
}

/// The context that a saved Control Studio result states of the run that made its recording (B10.2): the experiment and the plant of
/// the recording's own provenance when it has one, and else of the result itself. The result's `source` is the evidence source (A9),
/// and a plant context that names another source is not kept. A part that this engine does not read, or that breaks a rule, is left out
/// and never makes the file unreadable; `None` when nothing is left.
#[must_use]
pub fn saved_context(result: &Value, source: EvidenceSource) -> Option<RunContext> {
    let origin = result
        .pointer("/recording/provenance")
        .filter(|origin| origin.is_object())
        .unwrap_or(result);
    let experiments: Vec<ExperimentContext> = origin
        .get("experiment")
        .and_then(|experiment| {
            crate::json::decode_value::<ExperimentContext>(experiment.clone()).ok()
        })
        .into_iter()
        .collect();
    let plant = origin
        .get("plant")
        .and_then(|plant| saved_plant(plant, source));
    RunContext { experiments, plant }.checked(source)
}

/// The plant a saved result states, when it names the evidence source of the recording: the `source` key is the result's own word
/// (`Simulation` or `Physical`), which the context does not repeat.
fn saved_plant(plant: &Value, source: EvidenceSource) -> Option<PlantContext> {
    let word = match source {
        EvidenceSource::SimulatedPlant => "Simulation",
        EvidenceSource::PhysicalPlant => "Physical",
    };
    let mut fields = plant.as_object()?.clone();
    if fields.remove("source")?.as_str() != Some(word) {
        return None;
    }
    crate::json::decode_value::<PlantContext>(Value::Object(fields)).ok()
}

/// The range the loop declares its plant valid over (B10.2): the measured output and the plant input, with their units.
fn validity_range(loop_definition: &LoopDefinition) -> String {
    let output = &loop_definition.signals.process_variable;
    let input = &loop_definition.signals.manipulated_variable;
    format!(
        "PV {}…{} {}; MV {}…{} {}",
        js_number(output.range.minimum),
        js_number(output.range.maximum),
        output.unit,
        js_number(input.range.minimum),
        js_number(input.range.maximum),
        input.unit
    )
}

/// The number a plan's value holds; a Boolean is 0 or 1.
fn number(value: ExperimentValue) -> f64 {
    match value {
        ExperimentValue::Number(number) => number,
        ExperimentValue::Boolean(boolean) => f64::from(u8::from(boolean)),
    }
}

/// The text a status holds under `key`, when it is not blank.
fn text_of(status: &Value, key: &str) -> Option<String> {
    non_blank(status.get(key)).map(str::to_owned)
}

/// A JSON text that is not blank.
fn non_blank(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::as_str).filter(|text| !blank(text))
}

/// Whether the entry of a configuration or a status is the plant `plant_id`.
fn named(entry: &Value, plant_id: &str) -> bool {
    entry.get("id").and_then(Value::as_str) == Some(plant_id)
}

/// Whether a text is blank.
fn blank(text: &str) -> bool {
    text.trim().is_empty()
}

/// Whether a number is finite and above zero.
fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

/// The failure of a context that breaks a rule of the recording file.
fn invalid(reason: &str) -> Failure {
    Failure::new(
        &format!("The recording file is not consistent: {reason}."),
        Cause::Settings,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
