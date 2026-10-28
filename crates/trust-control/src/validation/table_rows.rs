//! The rows of the Validate table (specification 35, B5.4 and B9.9; the Validate page specification, "Results and plot"). The
//! engine decides what each row says, the order of the rows, which passing checks share one row, and each verdict's class. The
//! page lays the rows out and decides none of them. The words of a row, its measured and limit texts and its verdict cell are
//! written here as the table's renderer wrote them before (`engineValidate.ts`), so the table reads the same.

use crate::identification::independent_validation::RequirementStatus;
use crate::identification::recording_import::display_engineering_unit;
use crate::messages::{
    InitializationOutcome, LimitSide, RequirementChannel, RequirementOutcome, ScenarioOutcome,
    ValidateLabels, ValidateRow, ValueRange,
};
use crate::tuning::texts::engineering;

/// The identity the page gives the row that counts the passing guards (`data-studio-requirement`).
const GUARD_ROW: &str = "limit-and-controller-checks";
/// The identity the page gives the candidate's overall row (`data-studio-requirement`).
const OVERALL_ROW: &str = "overall";
/// The text of a value the table does not have.
const NO_VALUE: &str = "—";

/// One requirement of the table, with the words of its run in front of its label, and whether it is the independent model check.
struct Entry<'a> {
    prefix: String,
    requirement: &'a RequirementOutcome,
    model_agreement: bool,
}

/// The rows of the Validate table in the order the page shows them (B9.9). The independent model check comes first unless it
/// fails; then the rows that fail, the inconclusive rows and the passing rows, each group in the engine's order. The passing guards
/// are counted in one row after them, and the candidate's overall row closes the table when a scenario ran.
#[must_use]
pub fn rows(
    labels: &ValidateLabels,
    model_agreement: Option<&RequirementOutcome>,
    scenarios: &[ScenarioOutcome],
    initialization: Option<&InitializationOutcome>,
    status: RequirementStatus,
) -> Vec<ValidateRow> {
    let entries = collect_entries(labels, model_agreement, scenarios, initialization);
    let passing_guards = entries
        .iter()
        .filter(|entry| is_passing_guard(entry.requirement))
        .count();
    let mut ranked: Vec<(u8, ValidateRow)> = entries
        .iter()
        .filter(|entry| !is_passing_guard(entry.requirement))
        .map(|entry| (rank(entry), requirement_row(entry, labels)))
        .collect();
    ranked.sort_by_key(|(key, _)| *key);
    let mut table: Vec<ValidateRow> = ranked.into_iter().map(|(_, row)| row).collect();
    if passing_guards > 0 {
        table.push(guard_row(labels, passing_guards));
    }
    if !scenarios.is_empty() {
        table.push(overall_row(labels, status));
    }
    table
}

/// The requirements of the table with the words their rows carry: the model check, then each scenario's current settings' run and
/// candidate's run (named with the scenario when there is more than one), then the Manual-to-Auto entry of an Auto commissioning.
fn collect_entries<'a>(
    labels: &ValidateLabels,
    model_agreement: Option<&'a RequirementOutcome>,
    scenarios: &'a [ScenarioOutcome],
    initialization: Option<&'a InitializationOutcome>,
) -> Vec<Entry<'a>> {
    let mut entries: Vec<Entry<'a>> = model_agreement
        .into_iter()
        .map(|requirement| Entry {
            prefix: String::new(),
            requirement,
            model_agreement: true,
        })
        .collect();
    for scenario in scenarios {
        let scenario_prefix = if scenarios.len() > 1 {
            format!("{} · ", scenario.label)
        } else {
            String::new()
        };
        let runs = [
            (&scenario.current, &labels.current_settings),
            (&scenario.candidate, &labels.candidate),
        ];
        for (run, run_name) in runs {
            for requirement in &run.requirements {
                entries.push(Entry {
                    prefix: format!("{scenario_prefix}{run_name} · "),
                    requirement,
                    model_agreement: false,
                });
            }
        }
    }
    if let Some(case) = initialization {
        for requirement in &case.requirements {
            entries.push(Entry {
                prefix: format!("{} · ", case.label),
                requirement,
                model_agreement: false,
            });
        }
    }
    entries
}

/// A generated check that passed: the page counts it in the guard row, not in a row of its own (B9.9).
fn is_passing_guard(requirement: &RequirementOutcome) -> bool {
    requirement.guard && requirement.status == RequirementStatus::Pass
}

/// The place of a row: the model check first unless it fails, then the failing rows, the inconclusive rows and the passing rows.
fn rank(entry: &Entry<'_>) -> u8 {
    match (entry.model_agreement, entry.requirement.status) {
        (true, RequirementStatus::Pass | RequirementStatus::Inconclusive) => 0,
        (_, RequirementStatus::Fail) => 1,
        (_, RequirementStatus::Inconclusive) => 2,
        (_, RequirementStatus::Pass) => 3,
    }
}

/// One requirement as a row (B5.4, B9.9): its name with the Automatic mark and the channel's role and unit, the channel's symbol
/// that the page shows on hover, the source it shows under the name, its measured and limit texts, and its verdict.
fn requirement_row(entry: &Entry<'_>, labels: &ValidateLabels) -> ValidateRow {
    let requirement = entry.requirement;
    let mark = if requirement.automatic {
        format!(" · {}", labels.automatic)
    } else {
        String::new()
    };
    let channel = channel_text(&requirement.channel);
    ValidateRow {
        title: format!("{}{}{mark}{channel}", entry.prefix, requirement.label),
        symbol: requirement.channel.symbol.clone(),
        source: requirement.source.clone(),
        measured: measured_text(requirement),
        limit: limit_text(requirement),
        verdict: status_word(requirement.status).to_owned(),
        verdict_text: verdict_text(requirement.status, &requirement.reason),
        verdict_class: verdict_class(requirement.status).to_owned(),
        requirement: requirement.id.clone(),
    }
}

/// The passing guards as one row (B9.9): the engine's words for the row, the number of checks it counts, the words that they stayed
/// within their declared limits, and Pass.
fn guard_row(labels: &ValidateLabels, count: usize) -> ValidateRow {
    let checks = if count == 1 { "check" } else { "checks" };
    ValidateRow {
        title: labels.guards.clone(),
        symbol: String::new(),
        source: String::new(),
        measured: format!("{count} {checks}"),
        limit: labels.guards_limit.clone(),
        verdict: status_word(RequirementStatus::Pass).to_owned(),
        verdict_text: verdict_text(RequirementStatus::Pass, ""),
        verdict_class: verdict_class(RequirementStatus::Pass).to_owned(),
        requirement: GUARD_ROW.to_owned(),
    }
}

/// The overall row: the candidate's result, which decides Apply (N8.5, B5.4); it has no measured value and no limit.
fn overall_row(labels: &ValidateLabels, status: RequirementStatus) -> ValidateRow {
    ValidateRow {
        title: labels.candidate_overall.clone(),
        symbol: String::new(),
        source: String::new(),
        measured: NO_VALUE.to_owned(),
        limit: NO_VALUE.to_owned(),
        verdict: status_word(status).to_owned(),
        verdict_text: verdict_text(status, ""),
        verdict_class: verdict_class(status).to_owned(),
        requirement: OVERALL_ROW.to_owned(),
    }
}

/// The channel's role with its unit in parentheses, for example " · Measurement (°C)"; empty when the channel names no role.
fn channel_text(channel: &RequirementChannel) -> String {
    if channel.role.is_empty() {
        String::new()
    } else {
        format!(" · {}{}", channel.role, unit_parentheses(&channel.unit))
    }
}

/// The measured text of a check: its measured range for an operating-range or state-range check, and "—" for such a check
/// without a good value. Any other check shows its value with its unit, or "—" when the value is not a finite number.
fn measured_text(requirement: &RequirementOutcome) -> String {
    let unit = unit_suffix(&requirement.unit);
    if let Some(range) = requirement.measured_range {
        return range_text(range, &unit);
    }
    if requirement.limit_range.is_some() {
        return NO_VALUE.to_owned();
    }
    match requirement.value.and_then(page_number) {
        Some(value) => format!("{}{unit}", engineering(value, None)),
        None => NO_VALUE.to_owned(),
    }
}

/// The limit text of a check: its declared bounds for an operating-range or state-range check, with "= " before a single value.
/// Any other check shows its one-sided limit: "≤" for a value that must stay at or below it, "≥" for one at or above it.
fn limit_text(requirement: &RequirementOutcome) -> String {
    let unit = unit_suffix(&requirement.unit);
    if let Some(bounds) = requirement.limit_range {
        let equal = if page_number(bounds.minimum) == page_number(bounds.maximum) {
            "= "
        } else {
            ""
        };
        return format!("{equal}{}", range_text(bounds, &unit));
    }
    match requirement.limit.and_then(page_number) {
        Some(limit) => {
            let side = match requirement.limit_side {
                LimitSide::AtMost => "≤",
                LimitSide::AtLeast => "≥",
            };
            format!("{side} {}{unit}", engineering(limit, None))
        }
        None => NO_VALUE.to_owned(),
    }
}

/// A range as one number when its ends are equal, and as "lowest–highest" otherwise, with its unit.
pub(crate) fn range_text(range: ValueRange, unit: &str) -> String {
    let (low, high) = (page_number(range.minimum), page_number(range.maximum));
    if low == high {
        format!("{}{unit}", number_text(low))
    } else {
        format!("{}–{}{unit}", number_text(low), number_text(high))
    }
}

/// A number as the table prints it: the engine's engineering text, or "—" for a value the page does not have.
fn number_text(value: Option<f64>) -> String {
    value.map_or_else(|| NO_VALUE.to_owned(), |number| engineering(number, None))
}

/// A value as the page has it: the engine writes a number that is not finite as null, and the page shows null as "—".
fn page_number(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

/// The unit after a number: a space and the unit in the page's notation, or nothing for a blank unit or the unit "1".
pub(crate) fn unit_suffix(unit: &str) -> String {
    if is_dimensionless(unit) {
        String::new()
    } else {
        format!(" {}", display_engineering_unit(unit))
    }
}

/// The unit in parentheses, as a channel's role names it (" (°C)"); nothing for a blank unit or the unit "1".
fn unit_parentheses(unit: &str) -> String {
    if is_dimensionless(unit) {
        String::new()
    } else {
        format!(" ({})", display_engineering_unit(unit))
    }
}

/// Whether a unit is blank or "1", which the page does not write after a value.
fn is_dimensionless(unit: &str) -> bool {
    let trimmed = unit.trim();
    trimmed.is_empty() || trimmed == "1"
}

/// The engine's word for a verdict: Pass, Fail or Inconclusive (N8.4, N8.5).
fn status_word(status: RequirementStatus) -> &'static str {
    match status {
        RequirementStatus::Pass => "Pass",
        RequirementStatus::Fail => "Fail",
        RequirementStatus::Inconclusive => "Inconclusive",
    }
}

/// The class a verdict is styled by: the word in lower case.
fn verdict_class(status: RequirementStatus) -> &'static str {
    match status {
        RequirementStatus::Pass => "pass",
        RequirementStatus::Fail => "fail",
        RequirementStatus::Inconclusive => "inconclusive",
    }
}

/// The text of a verdict cell: the word alone for a pass, and the word, " · " and the reason for any other verdict that has one.
fn verdict_text(status: RequirementStatus, reason: &str) -> String {
    let word = status_word(status);
    if status != RequirementStatus::Pass && !reason.is_empty() {
        format!("{word} · {reason}")
    } else {
        word.to_owned()
    }
}

#[cfg(test)]
#[path = "table_rows_tests.rs"]
mod tests;
