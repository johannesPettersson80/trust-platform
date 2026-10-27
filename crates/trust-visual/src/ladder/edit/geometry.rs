//! The Ladder canvas geometry the stored edits use: the 20-unit grid, the rails, the rung pitch and
//! the connector points of each element kind (the same numbers the editor draws with).

use crate::json::js_round;
use crate::Json;

/// The grid every stored position snaps to.
pub const GRID: f64 = 20.0;
/// The x of the left power rail.
pub const LEFT_RAIL_X: f64 = 50.0;
/// The x of the right power rail.
pub const RIGHT_RAIL_X: f64 = 1100.0;
/// The vertical distance between rungs.
pub const RUNG_HEIGHT: f64 = 100.0;

/// `value` on the grid: `Math.round(value / 20) * 20`.
#[must_use]
pub fn snap(value: f64) -> f64 {
    js_round(value / GRID) * GRID
}

/// The x offset of the left and right connector of an element kind from its position.
#[must_use]
pub fn connector_offsets(kind: &str) -> (f64, f64) {
    match kind {
        "contact" | "coil" => (-20.0, 60.0),
        "timer" | "counter" | "compare" | "math" => (-20.0, 140.0),
        _ => (0.0, 0.0),
    }
}

/// The position of a node.
#[must_use]
pub fn position(node: &Json) -> (f64, f64) {
    let coordinate = |axis: &str| {
        node.get("position")
            .and_then(|position| position.get(axis))
            .and_then(Json::as_f64)
            .unwrap_or(0.0)
    };
    (coordinate("x"), coordinate("y"))
}

/// The orthogonal route of a wire from the right connector of `from` to the left connector of
/// `to`: four points with the vertical leg on the grid point between them. With `branch_legs`, a
/// wire into a merge turns at the merge and a wire out of a split turns at the split, so the legs
/// of a branch line up on its split and merge.
#[must_use]
pub fn route(from: &Json, to: &Json, branch_legs: bool) -> Json {
    let kind = |node: &Json| node.str_at("type").unwrap_or_default().to_owned();
    let (from_x, from_y) = position(from);
    let (to_x, to_y) = position(to);
    let from_x = from_x + connector_offsets(&kind(from)).1;
    let to_x = to_x + connector_offsets(&kind(to)).0;
    let elbow = if branch_legs && kind(to) == "branchMerge" {
        to_x
    } else if branch_legs && kind(from) == "branchSplit" {
        from_x
    } else {
        snap(f64::midpoint(from_x, to_x))
    };
    let point = |x: f64, y: f64| {
        Json::Object(vec![
            ("x".to_owned(), Json::Number(x)),
            ("y".to_owned(), Json::Number(y)),
        ])
    };
    Json::Array(vec![
        point(from_x, from_y),
        point(elbow, from_y),
        point(elbow, to_y),
        point(to_x, to_y),
    ])
}
