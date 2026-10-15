//! The criteria of a switching controller on a recorded run of Validate (specification 35, C3, Validate): every recorded change of the
//! requested state happens at its threshold (the transition rule), the requested state keeps inside its band (the retained band), and an
//! abort is recorded separately and never counts as a normal dwell completion. The levels and the completed dwells are judged by the
//! plan's requirements (`validation/on_off_case.rs`); this module judges the three criteria that need the latch, the setpoint and the
//! applied output at every scan. A scan that lacks one of those signals is Inconclusive, never a pass (B5.4).
//!
//! The thresholds are those of the declared band (D-8, D-10): the ON side is reached above the setpoint plus the half-width for a reverse
//! action and below the setpoint minus it for a direct one, and the OFF side the other way. The threshold policy decides whether reaching
//! a threshold is enough (`at-or-beyond`, the default) or the measurement must be strictly beyond it (`beyond`).

use std::collections::BTreeMap;

use crate::identification::connected_record::{ValidationRun, ValidationSample};
use crate::identification::independent_validation::RequirementStatus;
use crate::loop_definition::{Action, LoopDefinition, OnOffController, ThresholdEquality};
use crate::recording::channels::ChannelQuality;
use crate::validation::requirement_record::RunRequirementResult;
use crate::validation::run_validation::RecordedRunEvaluation;

/// The row of the transition rule: every change of the requested state happens at its threshold (C3).
pub(crate) const TRANSITION_ID: &str = "switching-transitions";
/// The row of the retained band: the requested state keeps inside its band (C3).
pub(crate) const BAND_ID: &str = "switching-band";
/// The row of the aborts: an abort is recorded separately (C3).
pub(crate) const ABORT_ID: &str = "switching-aborts";
/// The dwell rows of the plan, which an abort makes inconclusive (`validation/on_off_case.rs`).
const DWELL_IDS: [&str; 2] = ["switching-on-dwell", "switching-off-dwell"];
/// The sentence of a dwell row that an abort has made inconclusive.
const ABORT_ENDS_INTERVAL: &str = "An abort ended a switching interval, which is not a normal dwell completion, so the normal dwell is not judged (C3).";
/// The sentence of a transition row that no scan breaks.
const TRANSITIONS_HOLD: &str =
    "Every change of the requested state happened at its threshold (C3).";
/// The sentence of a band row that no scan breaks.
const BAND_HOLDS: &str = "The requested state stayed inside its band at every scan (C3).";
/// The sentence of an abort row without an abort.
const NO_ABORT: &str =
    "No abort: every change of the applied output followed its requested state (C3).";
/// The sentence of a run without any scan.
const NO_SCANS: &str = "The run holds no scan, so the switching criteria have no evidence (B5.4).";
/// The sentence of a run that averages its scans: the transition rule, the band and the aborts need every execution (A4.2, C3).
const AVERAGED_SCANS: &str = "The run averages its scans, so the transition rule, the retained band and the aborts cannot be judged on every execution (A4.2, C3).";
/// The sentence of a run whose settings hold no hysteresis.
const NO_HYSTERESIS: &str = "The run holds no value for the switching hysteresis, so the retained band has no threshold (C3).";

/// The symbols and the output's midpoint that the criteria read from a run.
struct Symbols<'a> {
    latch: &'a str,
    applied: &'a str,
    process: &'a str,
    setpoint: &'a str,
    midpoint: f64,
}

/// What one scan holds for the switching criteria: the requested state, the applied state, and the measurement and the setpoint.
#[derive(Debug, Clone, Copy)]
struct Scan {
    time: f64,
    requested_on: bool,
    applied_on: bool,
    process: f64,
    setpoint: f64,
}

/// The thresholds of the declared band (C3): the half-width, the action's direction, and whether a threshold must be passed strictly.
#[derive(Debug, Clone, Copy)]
struct Band {
    reverse: bool,
    half_width: f64,
    strict: bool,
}

impl Band {
    /// The threshold at which the measurement requests ON: above the setpoint for a reverse action, below it for a direct one.
    fn on_threshold(self, setpoint: f64) -> f64 {
        if self.reverse {
            setpoint + self.half_width
        } else {
            setpoint - self.half_width
        }
    }

    /// The threshold at which the measurement requests OFF: the other side of the setpoint.
    fn off_threshold(self, setpoint: f64) -> f64 {
        if self.reverse {
            setpoint - self.half_width
        } else {
            setpoint + self.half_width
        }
    }

    /// The threshold of a requested state.
    fn threshold_of(self, requested_on: bool, setpoint: f64) -> f64 {
        if requested_on {
            self.on_threshold(setpoint)
        } else {
            self.off_threshold(setpoint)
        }
    }

    /// Whether the measurement has reached the ON threshold at a scan.
    fn reaches_on(self, process: f64, setpoint: f64) -> bool {
        self.reached(process, self.on_threshold(setpoint), self.reverse)
    }

    /// Whether the measurement has reached the OFF threshold at a scan.
    fn reaches_off(self, process: f64, setpoint: f64) -> bool {
        self.reached(process, self.off_threshold(setpoint), !self.reverse)
    }

    /// Whether a measurement reaches a threshold: on the side that `above` names, at or beyond it, or strictly beyond it.
    fn reached(self, process: f64, threshold: f64, above: bool) -> bool {
        match (above, self.strict) {
            (true, false) => process >= threshold,
            (true, true) => process > threshold,
            (false, false) => process <= threshold,
            (false, true) => process < threshold,
        }
    }
}

/// The first scan that breaks a criterion: its time, the measurement there, the threshold it was judged against, and its sentence.
#[derive(Debug, Clone)]
struct Finding {
    process: f64,
    threshold: f64,
    reason: String,
}

/// The judgement of a run's scans: the first breach of the transition rule, the first breach of the band, and the aborts.
#[derive(Debug, Clone, Default)]
struct Judgement {
    transition: Option<Finding>,
    band: Option<Finding>,
    aborts: usize,
    first_abort: Option<f64>,
}

/// A good value of a signal in a scan, when the scan holds one.
fn good_value(sample: &ValidationSample, symbol: &str) -> Option<f64> {
    sample
        .values
        .get(symbol)
        .and_then(|value| match value.quality {
            ChannelQuality::Good => value.value,
            _ => None,
        })
}

/// The scans of a run, in order. The first scan that lacks a signal ends the reading, and its time and sentence are returned.
fn read_scans(run: &ValidationRun, symbols: &Symbols<'_>) -> (Vec<Scan>, Option<String>) {
    let mut scans = Vec::with_capacity(run.samples.len());
    for sample in &run.samples {
        let latch = good_value(sample, symbols.latch);
        let applied = good_value(sample, symbols.applied);
        let process = good_value(sample, symbols.process);
        let setpoint = good_value(sample, symbols.setpoint);
        let (Some(latch), Some(applied), Some(process), Some(setpoint)) =
            (latch, applied, process, setpoint)
        else {
            let reason = format!(
                "The requested state, the applied output, the measurement or the setpoint was not recorded at {:.3} s (B5.4).",
                sample.time_seconds
            );
            return (scans, Some(reason));
        };
        scans.push(Scan {
            time: sample.time_seconds,
            requested_on: latch >= 0.5,
            applied_on: applied >= symbols.midpoint,
            process,
            setpoint,
        });
    }
    (scans, None)
}

/// The judgement of the scans against the band: the first change of the requested state that its threshold does not reach, the first
/// scan whose measurement has passed the other threshold while the request holds, and the changes of the applied output that do not
/// follow the requested state (aborts). The scans are judged from the second one, since the first holds the initial request.
fn judge(scans: &[Scan], band: Band) -> Judgement {
    let mut judgement = Judgement::default();
    for pair in scans.windows(2) {
        let (before, now) = (pair[0], pair[1]);
        if now.requested_on != before.requested_on {
            let reached = if now.requested_on {
                band.reaches_on(now.process, now.setpoint)
            } else {
                band.reaches_off(now.process, now.setpoint)
            };
            if !reached && judgement.transition.is_none() {
                judgement.transition = Some(Finding {
                    process: now.process,
                    threshold: band.threshold_of(now.requested_on, now.setpoint),
                    reason: format!(
                        "The requested state changed to {} at {:.3} s before the measurement reached its threshold (C3).",
                        on_off_word(now.requested_on),
                        now.time
                    ),
                });
            }
        }
        let beyond_other = if now.requested_on {
            band.reaches_off(now.process, now.setpoint)
        } else {
            band.reaches_on(now.process, now.setpoint)
        };
        if beyond_other && judgement.band.is_none() {
            judgement.band = Some(Finding {
                process: now.process,
                threshold: band.threshold_of(!now.requested_on, now.setpoint),
                reason: format!(
                    "The requested state stayed {} at {:.3} s although the measurement had passed the other threshold (C3).",
                    on_off_word(now.requested_on),
                    now.time
                ),
            });
        }
        if now.applied_on != before.applied_on && now.applied_on != now.requested_on {
            judgement.aborts += 1;
            judgement.first_abort.get_or_insert(now.time);
        }
    }
    judgement
}

/// The word of a requested or applied state, for a sentence.
fn on_off_word(on: bool) -> &'static str {
    if on {
        "ON"
    } else {
        "OFF"
    }
}

/// The band of the run: the declared hysteresis of its settings as a half-width, with the action and the threshold policy of the loop.
fn band_of(
    loop_definition: &LoopDefinition,
    block: &OnOffController,
    settings: &BTreeMap<String, f64>,
) -> Option<Band> {
    let declared = settings.get(&block.hysteresis_symbol).copied()?;
    Some(Band {
        reverse: loop_definition.controller.action == Action::Reverse,
        half_width: block.band_convention.half_width(declared),
        strict: block.threshold_equality == ThresholdEquality::Beyond,
    })
}

/// The three rows of the switching criteria of a run (C3), before any dwell row is changed: the transition rule, the retained band and
/// the aborts. A run without the signals, the scans or the hysteresis has Inconclusive rows.
#[must_use]
pub(crate) fn switching_results(
    loop_definition: &LoopDefinition,
    block: &OnOffController,
    settings: &BTreeMap<String, f64>,
    run: &ValidationRun,
) -> Vec<RunRequirementResult> {
    let unit = loop_definition.signals.process_variable.unit.clone();
    let Some(latch) = block.latch_symbol() else {
        return inconclusive_rows(
            &unit,
            "The switching controller declares no output latch (C3).",
        );
    };
    let symbols = Symbols {
        latch,
        applied: loop_definition.signals.manipulated_variable.symbol.as_str(),
        process: loop_definition.signals.process_variable.symbol.as_str(),
        setpoint: loop_definition
            .signals
            .setpoint
            .as_ref()
            .map_or("", |setpoint| setpoint.symbol.as_str()),
        midpoint: f64::midpoint(block.low_output, block.high_output),
    };
    let (scans, missing) = read_scans(run, &symbols);
    if let Some(reason) = missing {
        return inconclusive_rows(&unit, &reason);
    }
    if scans.is_empty() {
        return inconclusive_rows(&unit, NO_SCANS);
    }
    let Some(band) = band_of(loop_definition, block, settings) else {
        return inconclusive_rows(&unit, NO_HYSTERESIS);
    };
    rows_of(&judge(&scans, band), &unit)
}

/// The rows of a judgement: a finding makes its row a failure, and an abort makes its row a failure with the count.
fn rows_of(judgement: &Judgement, unit: &str) -> Vec<RunRequirementResult> {
    let transition = match &judgement.transition {
        Some(finding) => failed(TRANSITION_ID, finding, unit),
        None => passed(TRANSITION_ID, TRANSITIONS_HOLD, unit),
    };
    let band = match &judgement.band {
        Some(finding) => failed(BAND_ID, finding, unit),
        None => passed(BAND_ID, BAND_HOLDS, unit),
    };
    let abort = if judgement.aborts == 0 {
        RunRequirementResult {
            id: ABORT_ID.to_owned(),
            kind: "switching-abort".to_owned(),
            status: RequirementStatus::Pass,
            value: Some(0.0),
            limit: Some(0.0),
            unit: "count".to_owned(),
            reason: NO_ABORT.to_owned(),
        }
    } else {
        RunRequirementResult {
            id: ABORT_ID.to_owned(),
            kind: "switching-abort".to_owned(),
            status: RequirementStatus::Fail,
            value: Some(u32::try_from(judgement.aborts).map_or(f64::MAX, f64::from)),
            limit: Some(0.0),
            unit: "count".to_owned(),
            reason: format!(
                "{} abort(s) of the applied output, first at {:.3} s: the output changed to a level its requested state did not hold. An abort is recorded separately and never counts as a normal dwell completion (C3).",
                judgement.aborts,
                judgement.first_abort.unwrap_or(0.0)
            ),
        }
    };
    vec![transition, band, abort]
}

/// A row that a finding breaks.
fn failed(id: &str, finding: &Finding, unit: &str) -> RunRequirementResult {
    RunRequirementResult {
        id: id.to_owned(),
        kind: id.to_owned(),
        status: RequirementStatus::Fail,
        value: Some(finding.process),
        limit: Some(finding.threshold),
        unit: unit.to_owned(),
        reason: finding.reason.clone(),
    }
}

/// A row that no scan breaks.
fn passed(id: &str, reason: &str, unit: &str) -> RunRequirementResult {
    RunRequirementResult {
        id: id.to_owned(),
        kind: id.to_owned(),
        status: RequirementStatus::Pass,
        value: None,
        limit: None,
        unit: unit.to_owned(),
        reason: reason.to_owned(),
    }
}

/// The three rows of a run whose evidence is missing: each is Inconclusive with the sentence that names the missing evidence.
fn inconclusive_rows(unit: &str, reason: &str) -> Vec<RunRequirementResult> {
    [TRANSITION_ID, BAND_ID, ABORT_ID]
        .into_iter()
        .map(|id| RunRequirementResult {
            id: id.to_owned(),
            kind: id.to_owned(),
            status: RequirementStatus::Inconclusive,
            value: None,
            limit: None,
            unit: unit.to_owned(),
            reason: reason.to_owned(),
        })
        .collect()
}

/// The overall status of a set of requirement verdicts: a failure fails it, and an inconclusive or empty set is inconclusive.
fn overall(statuses: impl Iterator<Item = RequirementStatus>) -> RequirementStatus {
    let statuses: Vec<RequirementStatus> = statuses.collect();
    if statuses.contains(&RequirementStatus::Fail) {
        RequirementStatus::Fail
    } else if statuses.is_empty() || statuses.contains(&RequirementStatus::Inconclusive) {
        RequirementStatus::Inconclusive
    } else {
        RequirementStatus::Pass
    }
}

/// Adds the switching criteria to a completed run's evaluation (C3, Validate): their three rows, and the overall status with them. When an
/// abort occurred, the normal dwell rows that it may have ended are made inconclusive, since an abort is never a normal completion. A
/// switching-free loop keeps its evaluation.
pub(crate) fn add_switching_results(
    loop_definition: &LoopDefinition,
    settings: &BTreeMap<String, f64>,
    run: &ValidationRun,
    averaged: bool,
    evaluation: &mut RecordedRunEvaluation,
) {
    let Some(block) = loop_definition.controller.on_off.as_ref() else {
        return;
    };
    let rows = if averaged {
        // A run that averages its scans (A4.2) cannot show the state of each execution.
        inconclusive_rows(
            &loop_definition.signals.process_variable.unit,
            AVERAGED_SCANS,
        )
    } else {
        switching_results(loop_definition, block, settings, run)
    };
    let aborted = rows
        .iter()
        .any(|row| row.id == ABORT_ID && row.status == RequirementStatus::Fail);
    if aborted {
        for row in evaluation
            .requirements
            .iter_mut()
            .filter(|row| DWELL_IDS.contains(&row.id.as_str()))
            .filter(|row| row.status != RequirementStatus::Fail)
        {
            row.status = RequirementStatus::Inconclusive;
            ABORT_ENDS_INTERVAL.clone_into(&mut row.reason);
        }
    }
    evaluation.requirements.extend(rows);
    evaluation.status = overall(evaluation.requirements.iter().map(|row| row.status));
}

#[cfg(test)]
#[path = "on_off_criteria_tests.rs"]
mod tests;
