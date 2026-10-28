//! The state-space editor's rules (specification 47 §9; specification 46 §5.1.1, §7.6, §7.6.1): the
//! dimensions the matrices give, the check of every matrix cell and starting state, the per-input
//! delays with the transport-history limit at the effective plant step, the resize that keeps or
//! removes cells and delays, and a pasted matrix.

use serde_json::{Map, Value};

use super::issues::{Issue, Target};
use super::js::{self, Num};

/// The most transport-history entries a plant may reserve (specification 46 §5.1.1).
pub(super) const HISTORY_LIMIT: u128 = 100_000;

/// States, inputs and outputs of a state-space model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Dimensions {
    pub(super) states: usize,
    pub(super) inputs: usize,
    pub(super) outputs: usize,
}

/// A matrix as the editor holds it: rows of numbers, a cell that is not a finite number `NaN`.
pub(super) fn matrix(value: Option<&Value>) -> Vec<Vec<f64>> {
    value
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    row.as_array()
                        .map(|cells| {
                            cells
                                .iter()
                                .map(|cell| js::num(Some(cell)).finite().unwrap_or(f64::NAN))
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The dimensions the matrices give: the rows of A, the columns of B's first row, the rows of C,
/// each at least 1.
pub(super) fn dimensions(parameters: &Map<String, Value>) -> Dimensions {
    let a = matrix(parameters.get("a"));
    let b = matrix(parameters.get("b"));
    let c = matrix(parameters.get("c"));
    Dimensions {
        states: a.len().max(1),
        inputs: b.first().map_or(0, Vec::len).max(1),
        outputs: c.len().max(1),
    }
}

/// The cell checks: each matrix has the shape its dimensions give (A states × states, B states ×
/// inputs, C outputs × states, D outputs × inputs) with finite cells, and the initial state holds one
/// finite number per state.
pub(super) fn cells(draft: &Map<String, Value>) -> Vec<Issue> {
    let parameters = js::object(draft, "parameters");
    let dimensions = dimensions(parameters);
    let mut issues = Vec::new();
    for (name, rows, columns) in [
        ("a", dimensions.states, dimensions.states),
        ("b", dimensions.states, dimensions.inputs),
        ("c", dimensions.outputs, dimensions.states),
        ("d", dimensions.outputs, dimensions.inputs),
    ] {
        let value = matrix(parameters.get(name));
        let upper = name.to_ascii_uppercase();
        if value.len() != rows || value.iter().any(|row| row.len() != columns) {
            issues.push(Issue::new(
                "invalid_state_space",
                format!("parameters.{name}"),
                format!("{upper} must be {rows} × {columns}."),
                Target::Matrix(name_of(name), 0, 0),
            ));
            continue;
        }
        for (row, cells) in value.iter().enumerate() {
            for (column, cell) in cells.iter().enumerate() {
                if !cell.is_finite() {
                    issues.push(Issue::new(
                        "invalid_state_space",
                        format!("parameters.{name}[{row}][{column}]"),
                        format!(
                            "{upper}[{},{}] must be a finite number.",
                            row + 1,
                            column + 1
                        ),
                        Target::Matrix(name_of(name), row, column),
                    ));
                }
            }
        }
    }
    let initial = js::numbers(draft.get("initialState")).unwrap_or_default();
    if initial.len() != dimensions.states {
        issues.push(Issue::new(
            "invalid_state_space",
            "initial_state",
            format!("Initial state must contain {} values.", dimensions.states),
            Target::Matrix("initial_state", 0, 0),
        ));
    } else {
        for (column, value) in initial.iter().enumerate() {
            if value.finite().is_none() {
                issues.push(Issue::new(
                    "invalid_state_space",
                    format!("initial_state[{column}]"),
                    format!("Initial state[{}] must be a finite number.", column + 1),
                    Target::Matrix("initial_state", 0, column),
                ));
            }
        }
    }
    issues
}

fn name_of(name: &str) -> &'static str {
    match name {
        "a" => "a",
        "b" => "b",
        "c" => "c",
        _ => "d",
    }
}

/// The input-delay checks (specification 46 §7.6.1, §5.1.1): omitted delays are zero; otherwise
/// one delay per input, each a finite number of seconds, not negative and within the runtime's time
/// range; and at the effective plant step (the plant's override, else the project's step) the
/// delays reserve at most 100,000 history entries, `ceil(delay / step) + 3` per input. Without a
/// known step, or with no delay above zero, the limit is not judged.
pub(super) fn input_delays(draft: &Map<String, Value>, project_step_ms: Option<f64>) -> Vec<Issue> {
    let parameters = js::object(draft, "parameters");
    let Some(delays) = parameters.get("input_delays_s") else {
        return Vec::new();
    };
    let inputs = dimensions(parameters).inputs;
    let issue = |column: Option<usize>, message: String| {
        let path = column.map_or_else(
            || "parameters.input_delays_s".to_string(),
            |column| format!("parameters.input_delays_s[{column}]"),
        );
        Issue::new(
            "invalid_input_delay",
            path,
            message.clone(),
            Target::InputDelay(column),
        )
        .summarized(format!(
            "Input delays (s) ({}): {message}",
            js::shown(Some(delays))
        ))
    };
    let Some(values) = js::numbers(Some(delays)).filter(|values| values.len() == inputs) else {
        return vec![issue(
            None,
            format!("Input delays need exactly {inputs} values, one per input."),
        )];
    };
    let mut issues = Vec::new();
    let mut nanos = Vec::with_capacity(values.len());
    for (column, value) in values.iter().enumerate() {
        match value {
            Num::Finite(seconds) if *seconds < 0.0 => issues.push(issue(
                Some(column),
                format!("Input {} delay must be 0 s or greater.", column + 1),
            )),
            Num::Finite(seconds) => {
                let rounded = (seconds * 1e9).round();
                if !rounded.is_finite() || rounded >= 9_223_372_036_854_775_808.0 {
                    issues.push(issue(
                        Some(column),
                        format!(
                            "Input {} delay exceeds the supported time range.",
                            column + 1
                        ),
                    ));
                } else {
                    nanos.push(rounded as u128);
                }
            }
            Num::Absent | Num::NotFinite => issues.push(issue(
                Some(column),
                format!(
                    "Input {} delay must be a finite number of seconds.",
                    column + 1
                ),
            )),
        }
    }
    if !issues.is_empty() {
        return issues;
    }
    let step_ms = match js::field(draft, "stepMs") {
        Num::Absent => project_step_ms,
        other => other.finite(),
    };
    let Some(step_ms) = step_ms.and_then(js::safe_integer).filter(|step| *step > 0) else {
        return Vec::new();
    };
    if nanos.iter().all(|value| *value == 0) {
        return Vec::new();
    }
    let step = step_ms as u128 * 1_000_000;
    let entries: u128 = nanos
        .iter()
        .map(|value| value / step + u128::from(value % step != 0) + 3)
        .sum();
    if entries > HISTORY_LIMIT {
        return vec![issue(
            None,
            format!(
                "Input delays exceed the 100,000-entry history limit at a {step_ms} ms plant step. Reduce a delay or increase the plant step."
            ),
        )];
    }
    Vec::new()
}

/// The parameters and initial state at `requested` dimensions: every matrix keeps the cells both
/// shapes have and gives new cells 0, the initial state keeps its first values and starts new states
/// at 0, and explicit input delays follow a change of the input count, new inputs at 0.
pub(super) fn resize(
    parameters: &Map<String, Value>,
    initial_state: &[Value],
    requested: Dimensions,
) -> (Map<String, Value>, Vec<Value>) {
    let current = dimensions(parameters);
    let mut resized = parameters.clone();
    for (name, rows, columns) in [
        ("a", requested.states, requested.states),
        ("b", requested.states, requested.inputs),
        ("c", requested.outputs, requested.states),
        ("d", requested.outputs, requested.inputs),
    ] {
        let source = matrix(parameters.get(name));
        let cells: Vec<Value> = (0..rows)
            .map(|row| {
                Value::Array(
                    (0..columns)
                        .map(|column| {
                            source
                                .get(row)
                                .and_then(|cells| cells.get(column))
                                .map_or(Value::from(0), |cell| js::js_number(*cell))
                        })
                        .collect(),
                )
            })
            .collect();
        resized.insert(name.to_string(), Value::Array(cells));
    }
    // Only a vector of numbers follows the inputs; anything else stays for its check.
    if requested.inputs != current.inputs
        && let Some(delays) = parameters.get("input_delays_s").and_then(Value::as_array)
        && delays
            .iter()
            .all(|delay| delay.is_number() || delay.is_null())
    {
        let next = (0..requested.inputs)
            .map(|index| delays.get(index).cloned().unwrap_or(Value::from(0)))
            .collect();
        resized.insert("input_delays_s".to_string(), Value::Array(next));
    }
    let state = (0..requested.states)
        .map(|index| initial_state.get(index).cloned().unwrap_or(Value::from(0)))
        .collect();
    (resized, state)
}

/// Whether a resize to `requested` removes values, and the confirmation it then needs: fewer
/// states, inputs or outputs remove matrix or initial-state values, and fewer inputs remove the
/// nonzero delays of the removed inputs (specification 47 §9).
pub(super) fn resize_confirmation(
    parameters: &Map<String, Value>,
    requested: Dimensions,
) -> Option<String> {
    let current = dimensions(parameters);
    let removes = requested.states < current.states
        || requested.inputs < current.inputs
        || requested.outputs < current.outputs;
    if !removes {
        return None;
    }
    let removed_delays = if requested.inputs < current.inputs {
        parameters
            .get("input_delays_s")
            .and_then(Value::as_array)
            .map_or(0, |delays| {
                // A delay the editor could not read (`NaN`, sent as null) is not zero either.
                delays
                    .iter()
                    .skip(requested.inputs)
                    .filter(|delay| {
                        delay.is_null() || delay.as_f64().is_some_and(|value| value != 0.0)
                    })
                    .count()
            })
    } else {
        0
    };
    Some(if removed_delays > 0 {
        format!(
            "Reducing inputs removes matrix columns and {removed_delays} nonzero input delay{}.",
            if removed_delays == 1 { "" } else { "s" }
        )
    } else {
        "Reducing dimensions removes matrix or initial-state values.".to_string()
    })
}

/// A pasted block of numbers (specification 47 §9): rows by line, cells separated by runs of tabs,
/// commas or spaces, an empty cell 0; every row as wide as the first and every cell a finite number.
/// `None` for anything else.
pub(super) fn parse_paste(text: &str) -> Option<Vec<Vec<f64>>> {
    let rows: Vec<Vec<f64>> = text
        .trim()
        .split('\n')
        .map(|row| {
            split_cells(row.strip_suffix('\r').unwrap_or(row).trim())
                .into_iter()
                .map(js::number_from_text)
                .collect()
        })
        .collect();
    let width = rows.first().map_or(0, Vec::len);
    if rows.is_empty()
        || width == 0
        || rows
            .iter()
            .any(|row| row.len() != width || row.iter().any(|value| !value.is_finite()))
    {
        return None;
    }
    Some(rows)
}

/// A row split at each run of tabs, commas and spaces; a run at either end leaves an empty cell
/// there, and an empty row is one empty cell.
fn split_cells(row: &str) -> Vec<&str> {
    let separator = |ch: char| matches!(ch, '\t' | ',' | ' ');
    let mut cells = Vec::new();
    let mut start = 0;
    let mut in_run = false;
    for (index, ch) in row.char_indices() {
        if separator(ch) {
            if !in_run {
                cells.push(&row[start..index]);
                in_run = true;
            }
        } else if in_run {
            start = index;
            in_run = false;
        }
    }
    cells.push(if in_run { "" } else { &row[start..] });
    cells
}
