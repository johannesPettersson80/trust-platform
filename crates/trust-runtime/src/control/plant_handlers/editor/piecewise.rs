//! The piecewise-linear editor's rules (specification 47 §9; specification 46 §7.7): paired input
//! and output points, at least two, every value finite and the inputs strictly increasing; a new
//! point one input step after the last; and a move that keeps the inputs increasing.

use serde_json::{Map, Value};

use super::issues::{Issue, Target};
use super::js;

/// One point of the curve; a value that is not a finite number is `NaN`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Point {
    pub(super) input: f64,
    pub(super) output: f64,
}

/// The points the parameters give: one per input point, paired with the output at the same index.
pub(super) fn points(parameters: &Map<String, Value>) -> Vec<Point> {
    let value = |values: Option<&Vec<Value>>, index: usize| {
        values
            .and_then(|values| values.get(index))
            .and_then(|value| js::num(Some(value)).finite())
            .unwrap_or(f64::NAN)
    };
    let inputs = parameters.get("input_points").and_then(Value::as_array);
    let outputs = parameters.get("output_points").and_then(Value::as_array);
    (0..inputs.map_or(0, Vec::len))
        .map(|index| Point {
            input: value(inputs, index),
            output: value(outputs, index),
        })
        .collect()
}

/// The parameters that hold `points`, the other parameters unchanged.
pub(super) fn with_points(parameters: &Map<String, Value>, points: &[Point]) -> Map<String, Value> {
    let mut next = parameters.clone();
    next.insert(
        "input_points".to_string(),
        Value::Array(
            points
                .iter()
                .map(|point| js::js_number(point.input))
                .collect(),
        ),
    );
    next.insert(
        "output_points".to_string(),
        Value::Array(
            points
                .iter()
                .map(|point| js::js_number(point.output))
                .collect(),
        ),
    );
    next
}

/// The point checks: at least two points, every input and output finite, each input greater than
/// the one before, and as many output points as input points.
pub(super) fn checks(parameters: &Map<String, Value>) -> Vec<Issue> {
    let points = points(parameters);
    let mut issues = Vec::new();
    let issue = |row: usize, field: &'static str, message: String| {
        let path = format!(
            "parameters.{}[{row}]",
            if field == "input" {
                "input_points"
            } else {
                "output_points"
            }
        );
        Issue::new(
            "invalid_piecewise_point",
            path,
            message,
            Target::Piecewise(row, field),
        )
    };
    if points.len() < 2 {
        issues.push(issue(
            points.len().saturating_sub(1),
            "input",
            "Add at least two input/output points.".into(),
        ));
    }
    for (row, point) in points.iter().enumerate() {
        if !point.input.is_finite() {
            issues.push(issue(
                row,
                "input",
                format!("Input point {} must be a finite number.", row + 1),
            ));
        }
        if !point.output.is_finite() {
            issues.push(issue(
                row,
                "output",
                format!("Output point {} must be a finite number.", row + 1),
            ));
        }
        if row > 0
            && point.input.is_finite()
            && points[row - 1].input.is_finite()
            && point.input <= points[row - 1].input
        {
            issues.push(issue(
                row,
                "input",
                format!(
                    "Input point {} must be greater than input point {row}.",
                    row + 1
                ),
            ));
        }
    }
    let inputs = parameters
        .get("input_points")
        .and_then(Value::as_array)
        .map(Vec::len);
    let outputs = parameters
        .get("output_points")
        .and_then(Value::as_array)
        .map(Vec::len);
    if let (Some(inputs), Some(outputs)) = (inputs, outputs)
        && inputs != outputs
    {
        // Shown at the first point without its output, or at the last point.
        issues.push(Issue::new(
            "invalid_piecewise_point",
            "parameters.output_points",
            "Input and output points must contain the same number of values.",
            Target::Piecewise(outputs.min(inputs.saturating_sub(1)), "output"),
        ));
    }
    issues
}

/// The points with a new last point: one input step after the last point at its output, or the
/// origin for an empty curve.
pub(super) fn append(points: &[Point]) -> Vec<Point> {
    let mut next = points.to_vec();
    next.push(points.last().map_or(
        Point {
            input: 0.0,
            output: 0.0,
        },
        |last| Point {
            input: last.input + 1.0,
            output: last.output,
        },
    ));
    next
}

/// The points with point `index` exchanged with its neighbour `delta` away, when the neighbour
/// exists and the inputs then still increase strictly; `None` otherwise.
pub(super) fn moved(points: &[Point], index: usize, delta: isize) -> Option<Vec<Point>> {
    let target = index.checked_add_signed(delta)?;
    if index >= points.len() || target >= points.len() {
        return None;
    }
    let mut next = points.to_vec();
    next.swap(index, target);
    let increasing = next
        .iter()
        .all(|point| point.input.is_finite() && point.output.is_finite())
        && next.len() >= 2
        && next.windows(2).all(|pair| pair[1].input > pair[0].input);
    increasing.then_some(next)
}
