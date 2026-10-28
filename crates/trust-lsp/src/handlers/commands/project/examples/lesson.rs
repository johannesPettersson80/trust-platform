//! The engineering contract of a control lesson's `lesson.json` (specification 38; specification 25
//! §3.1). A lesson of schema 2 is a summary: it keeps its place in the course but cannot be copied.
//! A lesson of schema 3 or 4 is copyable when every part of its contract holds: its signals, plant,
//! baseline and change, experiment, acceptance metrics, safety limits, workflow stages, proof and
//! video, and for schema 4 its objective, next lesson and learning sequence.
//!
//! The first check that fails names the lesson's problem; the gallery shows it, as a sentence, as
//! the reason the lesson is unavailable.

use serde_json::{json, Map, Value};

/// The signal roles a lesson may bind.
const SIGNAL_ROLES: [&str; 6] = ["SP", "PV", "MV", "disturbance", "state", "quality"];
/// The Control Studio workflow stages a lesson may name.
const WORKFLOW_STAGES: [&str; 5] = ["Monitor", "Test", "Identify", "Tune", "Validate"];
/// The experiment kinds a lesson may run.
const EXPERIMENT_KINDS: [&str; 8] = [
    "observation",
    "setpoint_step",
    "load_step",
    "mv_step",
    "mv_pulse",
    "relay",
    "prbs",
    "multisine",
];
/// The comparators of an acceptance metric.
const COMPARATORS: [&str; 5] = ["<", "<=", "==", ">=", ">"];
/// Where the evidence of an acceptance metric comes from.
const EVIDENCE_CLASSES: [&str; 4] = ["simulation", "physical", "recorded", "model"];
/// The publication states of a lesson's video.
const PUBLICATION_STATES: [&str; 7] = [
    "not_started",
    "scripted",
    "captured",
    "edited",
    "reviewed",
    "unlisted",
    "published",
];
/// The longest unit label, in UTF-16 code units as the gallery counts it.
const UNIT_LABEL_LIMIT: usize = 80;

/// A lesson that passed its checks.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Lesson {
    /// Schema 2: only the summary that orders the course.
    Summary(Value),
    /// Schema 3 or 4: the complete contract, as the file gives it.
    Contract(Value),
}

/// Whether a lesson can be copied, and why not.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Availability {
    /// The lesson can be copied as a runnable project.
    pub(super) available: bool,
    /// Why it cannot, as a sentence.
    pub(super) reason: Option<String>,
    /// The lesson as the gallery shows it: the contract, or the summary of one that cannot be
    /// copied, when that summary can be read.
    pub(super) lesson: Option<Value>,
}

/// The availability of the lesson `raw` that the manifest entry `manifest_id` names: available with
/// its complete contract, else unavailable with the reason and, when it can be read, its summary.
pub(super) fn availability(raw: &Value, manifest_id: &str) -> Availability {
    let check = Check { id: manifest_id };
    match check.contract(raw) {
        Ok(Lesson::Contract(lesson)) => Availability {
            available: true,
            reason: None,
            lesson: Some(lesson),
        },
        Ok(Lesson::Summary(summary)) => Availability {
            available: false,
            reason: Some("Requires the complete version 3 engineering contract.".to_owned()),
            lesson: Some(summary),
        },
        Err(Problem(reason)) => Availability {
            available: false,
            reason: Some(if reason == "missing object 'plant'" {
                "Missing the executable plant contract.".to_owned()
            } else {
                sentence(&reason)
            }),
            lesson: check
                .object(Some(raw), "metadata")
                .and_then(|lesson| check.summary(lesson))
                .ok(),
        },
    }
}

/// The lesson `raw` checked against its contract, or the error naming the first failed check as
/// `control lesson '<id>' <reason>`.
#[cfg(test)]
pub(super) fn parse_contract(raw: &Value, manifest_id: &str) -> Result<Lesson, String> {
    Check { id: manifest_id }
        .contract(raw)
        .map_err(|Problem(reason)| format!("control lesson '{manifest_id}' {reason}"))
}

/// `reason` as a sentence: without its final dots, with a capital and one final dot.
fn sentence(reason: &str) -> String {
    let normalized = reason.trim().trim_end_matches('.');
    let mut chars = normalized.chars();
    match chars.next() {
        Some(first) => format!("{}{}.", first.to_uppercase(), chars.as_str()),
        None => "The lesson contract is invalid.".to_owned(),
    }
}

/// The first failed check of a lesson, without the lesson's name.
#[derive(Debug)]
struct Problem(String);

type Checked<T> = Result<T, Problem>;

/// The checks of the lesson the manifest names `id`.
struct Check<'a> {
    id: &'a str,
}

/// A signal of a lesson: what the other parts may name.
struct Signal<'a> {
    role: &'a str,
    name: &'a str,
}

impl Check<'_> {
    fn contract(&self, raw: &Value) -> Checked<Lesson> {
        let lesson = self.object(Some(raw), "metadata")?;
        let summary = self.summary(lesson)?;
        let version = lesson.get("schemaVersion").and_then(Value::as_f64);
        if version == Some(2.0) {
            return Ok(Lesson::Summary(summary));
        }
        if version != Some(3.0) && version != Some(4.0) {
            return fail("unsupported schemaVersion");
        }
        for field in ["id", "title", "physicalPurpose"] {
            self.string(lesson.get(field), field)?;
        }
        if lesson.get("id").and_then(Value::as_str) != Some(self.id) {
            return fail("id does not match manifest");
        }
        let signals = self.signals(lesson.get("signals"))?;
        self.plant(lesson.get("plant"), &signals)?;
        self.executable_change(lesson.get("baseline"), "baseline", "limitation")?;
        self.executable_change(lesson.get("change"), "change", "decision")?;
        self.experiment(lesson.get("experiment"), &signals)?;
        self.acceptance(lesson.get("acceptance"), &signals)?;
        self.safety(lesson.get("safety"), &signals)?;
        self.workflow_stages(lesson.get("workflowStages"))?;
        self.proof(lesson.get("proof"))?;
        self.video(lesson.get("video"))?;
        if version == Some(4.0) {
            self.string(lesson.get("objective"), "objective")?;
            if !lesson.get("nextLesson").is_some_and(Value::is_null) {
                self.string(lesson.get("nextLesson"), "nextLesson")?;
            }
            self.learning(lesson.get("learning"))?;
        }
        Ok(Lesson::Contract(raw.clone()))
    }

    /// The summary that orders the course: level, equipment, technique, sequence, prerequisites.
    fn summary(&self, lesson: &Map<String, Value>) -> Checked<Value> {
        for field in ["level", "equipment", "principalTechnique"] {
            self.string(lesson.get(field), field)?;
        }
        let sequence = self.finite(lesson.get("sequence"), "sequence")?;
        if sequence.fract() != 0.0 || sequence < 1.0 {
            return fail("has invalid sequence");
        }
        let prerequisites = self.string_array(lesson.get("prerequisites"), "prerequisites")?;
        Ok(json!({
            "sequence": lesson.get("sequence"),
            "level": lesson.get("level"),
            "equipment": lesson.get("equipment"),
            "principalTechnique": lesson.get("principalTechnique"),
            "prerequisites": prerequisites,
        }))
    }

    fn signals<'v>(&self, value: Option<&'v Value>) -> Checked<Vec<Signal<'v>>> {
        let Some(items) = value
            .and_then(Value::as_array)
            .filter(|items| !items.is_empty())
        else {
            return fail("has invalid signals");
        };
        let mut signals = Vec::with_capacity(items.len());
        let mut bindings: Vec<&str> = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let signal = self.object(Some(item), &format!("signals[{index}]"))?;
            let role = self.string(signal.get("role"), &format!("signals[{index}].role"))?;
            if !SIGNAL_ROLES.contains(&role) {
                return fail(&format!("signal has unsupported role '{role}'"));
            }
            let name = self.string(signal.get("name"), &format!("signals[{index}].name"))?;
            self.unit(signal.get("unit"), &format!("signal '{role}'"))?;
            let binding =
                self.string(signal.get("binding"), &format!("signals[{index}].binding"))?;
            let minimum =
                self.finite(signal.get("minimum"), &format!("signals[{index}].minimum"))?;
            let maximum =
                self.finite(signal.get("maximum"), &format!("signals[{index}].maximum"))?;
            let resolution = self.finite(
                signal.get("resolution"),
                &format!("signals[{index}].resolution"),
            )?;
            if minimum >= maximum {
                return fail(&format!("signal '{role}' has invalid range"));
            }
            if resolution <= 0.0 {
                return fail(&format!("signal '{role}' has invalid resolution"));
            }
            if bindings.contains(&binding) {
                return fail(&format!("signals have duplicate binding '{binding}'"));
            }
            bindings.push(binding);
            signals.push(Signal { role, name });
        }
        // Open-loop identification and plant-classification lessons have no setpoint; PV and MV
        // remain mandatory.
        for required in ["PV", "MV"] {
            if !signals.iter().any(|signal| signal.role == required) {
                return fail(&format!("signals missing required role '{required}'"));
            }
        }
        Ok(signals)
    }

    fn plant(&self, value: Option<&Value>, signals: &[Signal<'_>]) -> Checked<()> {
        let plant = self.object(value, "plant")?;
        self.string_with(
            plant.get("modelId"),
            "plant missing string 'modelId'".to_owned(),
        )?;
        self.string(plant.get("executable"), "plant.executable")?;
        let sample_time = self.finite(plant.get("sampleTimeSeconds"), "plant.sampleTimeSeconds")?;
        if sample_time <= 0.0 {
            return fail("plant has invalid sampleTimeSeconds");
        }
        self.finite_record(plant.get("initialState"), "plant.initialState")?;
        let seed = self.finite(plant.get("seed"), "plant.seed")?;
        if seed.fract() != 0.0 {
            return fail("plant has invalid seed");
        }
        // The keys are checked in name order: serde_json keeps an object's members sorted.
        let operating_point =
            self.finite_record(plant.get("operatingPoint"), "plant.operatingPoint")?;
        for signal in operating_point.keys() {
            if !is_known(signals, signal) {
                return fail(&format!(
                    "plant operating point names unknown signal '{signal}'"
                ));
            }
        }
        let Some(ranges) = plant
            .get("validityRange")
            .and_then(Value::as_array)
            .filter(|ranges| !ranges.is_empty())
        else {
            return fail("plant has invalid validityRange");
        };
        for item in ranges {
            let range = self.object(Some(item), "plant validity range")?;
            let signal = self.string(range.get("signal"), "plant validity range signal")?;
            if !is_known(signals, signal) {
                return fail(&format!(
                    "plant validity range names unknown signal '{signal}'"
                ));
            }
            let minimum = self.finite(
                range.get("minimum"),
                &format!("plant validity range '{signal}' minimum"),
            )?;
            let maximum = self.finite(
                range.get("maximum"),
                &format!("plant validity range '{signal}' maximum"),
            )?;
            if minimum >= maximum {
                return fail(&format!("plant validity range '{signal}' is reversed"));
            }
            self.unit(
                range.get("unit"),
                &format!("plant validity range '{signal}'"),
            )?;
        }
        self.string_array(plant.get("simplifications"), "plant.simplifications")?;
        let Some(parameters) = plant
            .get("parameters")
            .and_then(Value::as_array)
            .filter(|parameters| !parameters.is_empty())
        else {
            return fail("plant has invalid parameters");
        };
        for item in parameters {
            let parameter = self.object(Some(item), "plant parameter")?;
            let name = self.string(parameter.get("name"), "plant parameter name")?;
            self.unit(parameter.get("unit"), &format!("plant parameter '{name}'"))?;
            if !parameter.get("value").is_some_and(Value::is_number) {
                return fail(&format!("plant parameter '{name}' has nonfinite value"));
            }
        }
        Ok(())
    }

    /// The baseline or the change: an executable, its settings and the reason for it. A JSON
    /// number is always finite, so the settings need no further check.
    fn executable_change(
        &self,
        value: Option<&Value>,
        field: &str,
        explanation: &str,
    ) -> Checked<()> {
        let change = self.object(value, field)?;
        self.string(change.get("executableId"), &format!("{field}.executableId"))?;
        self.object(change.get("settings"), &format!("{field}.settings"))?;
        self.string(change.get(explanation), &format!("{field}.{explanation}"))?;
        Ok(())
    }

    fn experiment(&self, value: Option<&Value>, signals: &[Signal<'_>]) -> Checked<()> {
        let experiment = self.object(value, "experiment")?;
        let kind = self.string(experiment.get("kind"), "experiment.kind")?;
        if !EXPERIMENT_KINDS.contains(&kind) {
            return fail(&format!("experiment has unsupported kind '{kind}'"));
        }
        let signal = self.string(experiment.get("signal"), "experiment.signal")?;
        if !is_known(signals, signal) {
            return fail(&format!("experiment names unknown signal '{signal}'"));
        }
        for field in [
            "baseline",
            "amplitude",
            "durationSeconds",
            "settlingSeconds",
            "seed",
            "restorationValue",
        ] {
            self.finite(experiment.get(field), &format!("experiment.{field}"))?;
        }
        let duration = self.finite(
            experiment.get("durationSeconds"),
            "experiment.durationSeconds",
        )?;
        let settling = self.finite(
            experiment.get("settlingSeconds"),
            "experiment.settlingSeconds",
        )?;
        if duration <= 0.0 {
            return fail("experiment has invalid durationSeconds");
        }
        if settling < 0.0 || settling > duration {
            return fail("experiment has invalid settlingSeconds");
        }
        self.string_array(experiment.get("disturbances"), "experiment.disturbances")?;
        Ok(())
    }

    fn acceptance(&self, value: Option<&Value>, signals: &[Signal<'_>]) -> Checked<()> {
        let Some(metrics) = value
            .and_then(Value::as_array)
            .filter(|metrics| !metrics.is_empty())
        else {
            return fail("has invalid acceptance");
        };
        for item in metrics {
            let acceptance = self.object(Some(item), "acceptance metric")?;
            let metric = self.string(acceptance.get("metric"), "acceptance.metric")?;
            let signal = self.string(
                acceptance.get("signal"),
                &format!("acceptance metric '{metric}' signal"),
            )?;
            if !is_known(signals, signal) {
                return fail(&format!(
                    "acceptance metric '{metric}' names unknown signal '{signal}'"
                ));
            }
            self.string(
                acceptance.get("window"),
                &format!("acceptance metric '{metric}' window"),
            )?;
            let comparator = self.string(
                acceptance.get("comparator"),
                &format!("acceptance metric '{metric}' comparator"),
            )?;
            if !COMPARATORS.contains(&comparator) {
                return fail(&format!(
                    "acceptance metric '{metric}' has unsupported comparator '{comparator}'"
                ));
            }
            if !acceptance.get("limit").is_some_and(Value::is_number) {
                return fail(&format!("acceptance metric '{metric}' has nonfinite limit"));
            }
            let tolerance = self.finite(
                acceptance.get("tolerance"),
                &format!("acceptance metric '{metric}' tolerance"),
            )?;
            if tolerance < 0.0 {
                return fail(&format!(
                    "acceptance metric '{metric}' has invalid tolerance"
                ));
            }
            self.unit(
                acceptance.get("unit"),
                &format!("acceptance metric '{metric}'"),
            )?;
            let evidence_class = self.string(
                acceptance.get("evidenceClass"),
                &format!("acceptance metric '{metric}' evidenceClass"),
            )?;
            if !EVIDENCE_CLASSES.contains(&evidence_class) {
                return fail(&format!(
                    "acceptance metric '{metric}' has unsupported evidenceClass '{evidence_class}'"
                ));
            }
        }
        Ok(())
    }

    fn safety(&self, value: Option<&Value>, signals: &[Signal<'_>]) -> Checked<()> {
        let safety = self.object(value, "safety")?;
        let Some(limits) = safety
            .get("hardLimits")
            .and_then(Value::as_array)
            .filter(|limits| !limits.is_empty())
        else {
            return fail("safety has invalid hardLimits");
        };
        for item in limits {
            let limit = self.object(Some(item), "safety limit")?;
            let signal = self.string(limit.get("signal"), "safety limit signal")?;
            if !is_known(signals, signal) {
                return fail(&format!("safety limit names unknown signal '{signal}'"));
            }
            let minimum = self.finite(
                limit.get("minimum"),
                &format!("safety limit '{signal}' minimum"),
            )?;
            let maximum = self.finite(
                limit.get("maximum"),
                &format!("safety limit '{signal}' maximum"),
            )?;
            if minimum >= maximum {
                return fail(&format!("safety limit '{signal}' has invalid range"));
            }
            self.unit(limit.get("unit"), &format!("safety limit '{signal}'"))?;
        }
        if self
            .string_array(safety.get("abortPredicates"), "safety.abortPredicates")?
            .is_empty()
        {
            return fail("safety has no abortPredicates");
        }
        self.finite(safety.get("safeMv"), "safety.safeMv")?;
        self.string(safety.get("restorationMode"), "safety.restorationMode")?;
        self.string(
            safety.get("physicalApprovalRole"),
            "safety.physicalApprovalRole",
        )?;
        self.string_array(
            safety.get("simulationOnlyRestrictions"),
            "safety.simulationOnlyRestrictions",
        )?;
        Ok(())
    }

    fn workflow_stages(&self, value: Option<&Value>) -> Checked<()> {
        let stages = self.string_array(value, "workflowStages")?;
        let mut seen: Vec<&str> = Vec::new();
        for stage in stages {
            if !WORKFLOW_STAGES.contains(&stage) {
                return fail(&format!(
                    "workflowStages contains unsupported stage '{stage}'"
                ));
            }
            if seen.contains(&stage) {
                return fail(&format!(
                    "workflowStages contains duplicate stage '{stage}'"
                ));
            }
            seen.push(stage);
        }
        Ok(())
    }

    fn proof(&self, value: Option<&Value>) -> Checked<()> {
        let proof = self.object(value, "proof")?;
        for field in [
            "specification",
            "nativeTest",
            "nativeCommand",
            "projectPath",
            "renderedJourneyId",
        ] {
            self.string(proof.get(field), &format!("proof.{field}"))?;
        }
        let native_test = proof
            .get("nativeTest")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let native_command = proof
            .get("nativeCommand")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !native_command.contains(native_test) {
            return fail(&format!(
                "proof nativeCommand does not name nativeTest '{native_test}'"
            ));
        }
        if !native_command
            .split_whitespace()
            .any(|argument| argument == native_test)
        {
            return fail(&format!(
                "proof nativeCommand does not name nativeTest '{native_test}' as an exact argument"
            ));
        }
        if self
            .string_array(proof.get("runArtifactFields"), "proof.runArtifactFields")?
            .is_empty()
        {
            return fail("proof has no runArtifactFields");
        }
        Ok(())
    }

    fn video(&self, value: Option<&Value>) -> Checked<()> {
        let video = self.object(value, "video")?;
        for field in [
            "viewerOutcome",
            "durationClass",
            "scriptPath",
            "captionPath",
            "thumbnailPath",
        ] {
            self.string(video.get(field), &format!("video.{field}"))?;
        }
        if self
            .string_array(
                video.get("requiredVisibleEvidence"),
                "video.requiredVisibleEvidence",
            )?
            .is_empty()
        {
            return fail("video has no requiredVisibleEvidence");
        }
        let state = self.string(video.get("publicationState"), "video.publicationState")?;
        if !PUBLICATION_STATES.contains(&state) {
            return fail(&format!("video has unsupported publicationState '{state}'"));
        }
        Ok(())
    }

    fn learning(&self, value: Option<&Value>) -> Checked<()> {
        let learning = self.object(value, "learning")?;
        for field in [
            "context",
            "reason",
            "workedPrediction",
            "baseline",
            "change",
            "comparison",
            "validation",
            "explanation",
            "transfer",
        ] {
            self.string(learning.get(field), &format!("learning.{field}"))?;
        }
        for field in ["theory", "practicalPreparation"] {
            if self
                .string_array(learning.get(field), &format!("learning.{field}"))?
                .is_empty()
            {
                return fail(&format!("has empty learning.{field}"));
            }
        }
        Ok(())
    }

    /// An object, or `missing object '<field>'` (`metadata is not an object` for the lesson).
    fn object<'v>(&self, value: Option<&'v Value>, field: &str) -> Checked<&'v Map<String, Value>> {
        match value {
            Some(Value::Object(object)) => Ok(object),
            _ if field == "metadata" => fail("metadata is not an object"),
            _ => fail(&format!("missing object '{field}'")),
        }
    }

    /// A text that is not blank, or `missing string '<field>'`.
    fn string<'v>(&self, value: Option<&'v Value>, field: &str) -> Checked<&'v str> {
        self.string_with(value, format!("missing string '{field}'"))
    }

    fn string_with<'v>(&self, value: Option<&'v Value>, message: String) -> Checked<&'v str> {
        match value.and_then(Value::as_str) {
            Some(text) if !is_blank(text) => Ok(text),
            _ => Err(Problem(message)),
        }
    }

    /// A number, or `has invalid <field>`. A JSON number is always finite.
    fn finite(&self, value: Option<&Value>, field: &str) -> Checked<f64> {
        match value.and_then(Value::as_f64) {
            Some(number) if number.is_finite() => Ok(number),
            _ => fail(&format!("has invalid {field}")),
        }
    }

    /// A list of texts none of which is blank, or `has invalid <field>`.
    fn string_array<'v>(&self, value: Option<&'v Value>, field: &str) -> Checked<Vec<&'v str>> {
        let texts: Option<Vec<&str>> = value.and_then(Value::as_array).and_then(|items| {
            items
                .iter()
                .map(|item| item.as_str().filter(|text| !is_blank(text)))
                .collect()
        });
        texts.ok_or_else(|| Problem(format!("has invalid {field}")))
    }

    /// An object of numbers that is not empty.
    fn finite_record<'v>(
        &self,
        value: Option<&'v Value>,
        field: &str,
    ) -> Checked<&'v Map<String, Value>> {
        let record = self.object(value, field)?;
        if record.is_empty() {
            return fail(&format!("{field} is empty"));
        }
        for (key, item) in record {
            if !item.is_number() {
                return fail(&format!("{field}.{key} is not finite"));
            }
        }
        Ok(record)
    }

    /// An engineering unit label: a text of at most 80 UTF-16 code units without control
    /// characters. Any unit is accepted; the gallery suggests common ones.
    fn unit<'v>(&self, value: Option<&'v Value>, owner: &str) -> Checked<&'v str> {
        let unit = self.string(value, &format!("{owner} unit"))?;
        if unit.encode_utf16().count() > UNIT_LABEL_LIMIT
            || unit.chars().any(|ch| ch < ' ' || ch == '\u{7f}')
        {
            return fail(&format!("{owner} has an invalid unit label"));
        }
        Ok(unit)
    }
}

/// Whether `name` is a role or a name of one of `signals`.
fn is_known(signals: &[Signal<'_>], name: &str) -> bool {
    signals
        .iter()
        .any(|signal| signal.role == name || signal.name == name)
}

/// Whether `text` is empty once blanks are trimmed, with the blanks the gallery's former checks
/// trimmed (Unicode white space and the byte-order mark, but not U+0085).
fn is_blank(text: &str) -> bool {
    text.chars()
        .all(|ch| (ch.is_whitespace() && ch != '\u{85}') || ch == '\u{feff}')
}

fn fail<T>(reason: &str) -> Checked<T> {
    Err(Problem(reason.to_owned()))
}

#[cfg(test)]
#[path = "lesson_tests.rs"]
mod tests;
