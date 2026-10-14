//! The drawn geometry of page objects (specification 39, §5 and §6): the box an object is drawn in
//! and the containment and overlap tests the authoring checks use.
//!
//! Port of `editor/editing.ts` (`drawnBox`) and of the rectangle tests in `agent/layout.ts`
//! (`overlaps`, `contains`). `js_round` reproduces JavaScript's `Math.round`, which the TypeScript
//! engine used to move objects: it rounds a half toward positive infinity.

use serde::Serialize;
use trust_runtime::hmi::page_files::HmiObject;

/// An axis-aligned box in design pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(super) struct Rect {
    /// Left edge.
    pub(super) x: f64,
    /// Top edge.
    pub(super) y: f64,
    /// Width.
    pub(super) width: f64,
    /// Height.
    pub(super) height: f64,
}

/// The box an object is drawn in. A quarter turn (any rotation that is not a multiple of 180)
/// swaps its width and height about its centre (`drawnBox`).
#[must_use]
pub(super) fn drawn_box(object: &HmiObject) -> Rect {
    box_of(
        [coord(object.at[0]), coord(object.at[1])],
        [f64::from(object.size[0]), f64::from(object.size[1])],
        object.rotation,
    )
}

/// The box drawn for a position, a size and a rotation, before an object is placed there
/// (`drawnBox` applied to `{ ...item, at, size, rotation }` in `layout.ts` `boxAt`).
#[must_use]
pub(super) fn box_of(at: [f64; 2], size: [f64; 2], rotation: u16) -> Rect {
    let [x, y] = at;
    let [width, height] = size;
    if rotation.is_multiple_of(180) {
        return Rect {
            x,
            y,
            width,
            height,
        };
    }
    Rect {
        x: x + width / 2.0 - height / 2.0,
        y: y + height / 2.0 - width / 2.0,
        width: height,
        height: width,
    }
}

/// Whether `inner` lies wholly inside `outer`, edges included (`contains` in `layout.ts`).
#[must_use]
pub(super) fn contains(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

/// Whether two boxes overlap once `gap` is added around the first (`overlaps` in `layout.ts`).
/// Touching edges alone do not overlap.
#[must_use]
pub(super) fn overlaps(first: Rect, second: Rect, gap: f64) -> bool {
    first.x < second.x + second.width + gap
        && first.x + first.width + gap > second.x
        && first.y < second.y + second.height + gap
        && first.y + first.height + gap > second.y
}

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod tests;

/// JavaScript's `Math.round`: halves go toward positive infinity, so `-2.5` becomes `-2`.
#[must_use]
pub(super) fn js_round(value: f64) -> f64 {
    // `value + 0.5` can round up for 0.49999999999999994, so compare the fraction instead.
    let floor = value.floor();
    if value - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    }
}

/// A computed position rounded to whole pixels, as the model stores it. Positions are finite
/// and far below 2^53 in every caller.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub(super) fn whole_pixel(value: f64) -> i64 {
    js_round(value) as i64
}

/// A page coordinate as a float. Page coordinates are far below 2^53, so the conversion is exact.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub(super) fn coord(value: i64) -> f64 {
    value as f64
}
