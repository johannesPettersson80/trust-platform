//! Align and distribute for the agent's edits (specification 39, §3.3 item 8; port of
//! `editor/align.ts`).
//!
//! Positions are the drawn boxes of `geometry::drawn_box`, so a quarter turn counts as the box it is
//! drawn in. Each moved position is rounded to whole pixels as `Math.round` rounds it (`js_round`).

use std::cmp::Ordering;

use rustc_hash::FxHashMap;
use trust_runtime::hmi::page_files::HmiPage;

use super::geometry::{coord, drawn_box, js_round, Rect};

/// The edge or centre that the selection aligns to (`AlignEdge`, `align.ts` line 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AlignEdge {
    /// The left edges line up with the selection's left edge.
    Left,
    /// The centres line up at the middle of the selection's width.
    Centre,
    /// The right edges line up with the selection's right edge.
    Right,
    /// The top edges line up with the selection's top edge.
    Top,
    /// The middles line up at the middle of the selection's height.
    Middle,
    /// The bottom edges line up with the selection's bottom edge.
    Bottom,
}

impl AlignEdge {
    /// The edge a string names: `left`, `centre`, `right`, `top`, `middle` or `bottom`, the strings of
    /// `AlignEdge` in `align.ts`. Any other text names no edge.
    #[must_use]
    pub(super) fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "left" => Self::Left,
            "centre" => Self::Centre,
            "right" => Self::Right,
            "top" => Self::Top,
            "middle" => Self::Middle,
            "bottom" => Self::Bottom,
            _ => return None,
        })
    }
}

/// The axis that objects are distributed along (`DistributeAxis`, `align.ts` line 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DistributeAxis {
    /// From left to right, along the x axis.
    Horizontal,
    /// From top to bottom, along the y axis.
    Vertical,
}

impl DistributeAxis {
    /// The axis a string names: `horizontal` or `vertical`, the strings of `DistributeAxis` in
    /// `align.ts`. Any other text names no axis.
    #[must_use]
    pub(super) fn parse(text: &str) -> Option<Self> {
        match text {
            "horizontal" => Some(Self::Horizontal),
            "vertical" => Some(Self::Vertical),
            _ => None,
        }
    }
}

/// The objects of `page` named in `ids`, in the page's order, each with its drawn box. The page is
/// filtered rather than the names, so a name on no object adds nothing and a repeated name counts once
/// (`alignObjects` and `distributeObjects` both filter `page.object` this way).
fn selected_boxes<'a>(page: &'a HmiPage, ids: &[String]) -> Vec<(&'a str, Rect)> {
    page.objects
        .iter()
        .filter(|object| ids.contains(&object.id))
        .map(|object| (object.id.as_str(), drawn_box(object)))
        .collect()
}

/// Rounds a position to whole pixels, as `Math.round` does (`js_round`), and converts it to `i64`.
#[allow(clippy::cast_possible_truncation)] // js_round gives an integral value far below 2^53, so the cast is exact.
fn whole_pixels(value: f64) -> i64 {
    js_round(value) as i64
}

/// A count of objects as a float. A page holds far fewer than 2^53 objects, so the conversion is exact.
#[allow(clippy::cast_precision_loss)] // See above: the count is far below 2^53.
fn count_as_f64(count: usize) -> f64 {
    count as f64
}

/// The page with each object of `deltas` moved by its offset, the new position rounded to whole
/// pixels (`moved`, `align.ts` line 13). An offset applies to every object that has its id.
#[must_use]
fn moved(page: &HmiPage, deltas: &FxHashMap<&str, [f64; 2]>) -> HmiPage {
    let mut next = page.clone();
    for object in &mut next.objects {
        if let Some(delta) = deltas.get(object.id.as_str()) {
            object.at = [
                whole_pixels(coord(object.at[0]) + delta[0]),
                whole_pixels(coord(object.at[1]) + delta[1]),
            ];
        }
    }
    next
}

/// `alignObjects` (`align.ts` line 26): with two or more of `ids` on the page, lines their edges or
/// centres up with the outer edges of the selection (specification 39, §3.3 item 8). Fewer than two
/// leave the page as it was.
#[must_use]
pub(super) fn align_objects(page: &HmiPage, ids: &[String], edge: AlignEdge) -> HmiPage {
    let boxes = selected_boxes(page, ids);
    if boxes.len() < 2 {
        return page.clone();
    }
    let left = boxes
        .iter()
        .map(|(_, rect)| rect.x)
        .fold(f64::INFINITY, f64::min);
    let right = boxes
        .iter()
        .map(|(_, rect)| rect.x + rect.width)
        .fold(f64::NEG_INFINITY, f64::max);
    let top = boxes
        .iter()
        .map(|(_, rect)| rect.y)
        .fold(f64::INFINITY, f64::min);
    let bottom = boxes
        .iter()
        .map(|(_, rect)| rect.y + rect.height)
        .fold(f64::NEG_INFINITY, f64::max);
    let mut deltas = FxHashMap::default();
    for (id, rect) in &boxes {
        let delta = [
            match edge {
                AlignEdge::Left => left - rect.x,
                AlignEdge::Centre => left.midpoint(right) - rect.x - rect.width / 2.0,
                AlignEdge::Right => right - rect.x - rect.width,
                AlignEdge::Top | AlignEdge::Middle | AlignEdge::Bottom => 0.0,
            },
            match edge {
                AlignEdge::Top => top - rect.y,
                AlignEdge::Middle => top.midpoint(bottom) - rect.y - rect.height / 2.0,
                AlignEdge::Bottom => bottom - rect.y - rect.height,
                AlignEdge::Left | AlignEdge::Centre | AlignEdge::Right => 0.0,
            },
        ];
        deltas.insert(*id, delta);
    }
    moved(page, &deltas)
}

/// `distributeObjects` (`align.ts` line 59): with three or more of `ids` on the page, spreads them
/// along `axis` with equal gaps between them, the first and the last staying where they are
/// (specification 39, §3.3 item 8). The objects are taken in the order of their start on the axis,
/// and equal starts keep the page's order. Fewer than three leave the page as it was.
#[must_use]
pub(super) fn distribute_objects(page: &HmiPage, ids: &[String], axis: DistributeAxis) -> HmiPage {
    let horizontal = axis == DistributeAxis::Horizontal;
    let start = |rect: &Rect| if horizontal { rect.x } else { rect.y };
    let length = |rect: &Rect| if horizontal { rect.width } else { rect.height };
    let mut boxes = selected_boxes(page, ids);
    boxes.sort_by(|(_, a), (_, b)| start(a).partial_cmp(&start(b)).unwrap_or(Ordering::Equal));
    if boxes.len() < 3 {
        return page.clone();
    }
    let first = boxes[0].1;
    let last = boxes[boxes.len() - 1].1;
    let span = start(&last) + length(&last) - start(&first);
    let total = boxes.iter().fold(0.0, |sum, (_, rect)| sum + length(rect));
    let gap = (span - total) / count_as_f64(boxes.len() - 1);
    let mut deltas = FxHashMap::default();
    let mut at = start(&first);
    for (id, rect) in &boxes {
        let shift = at - start(rect);
        deltas.insert(
            *id,
            if horizontal {
                [shift, 0.0]
            } else {
                [0.0, shift]
            },
        );
        at += length(rect) + gap;
    }
    moved(page, &deltas)
}

#[cfg(test)]
#[path = "align_tests.rs"]
mod tests;
