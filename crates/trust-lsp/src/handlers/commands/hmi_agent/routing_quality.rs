//! The routing findings of a draft review (specification 43, §7 items 6 and 7): a pipe with an
//! unavailable endpoint, a pipe that runs through an object's body, and rows of repeated faceplate
//! cards that are not laid out alike.
//!
//! Port of `editors/vscode/src/hmiBuilder/agent/routingQuality.ts` (`routingQuality`). Crossings
//! between pipes are intentional, and the equipment at a pipe's ends, the instruments attached to
//! that pipe, and panels and groups are not body collisions. The routes are those of
//! `routes::pipe_routes`: they are inspected here, never replaced.

use trust_runtime::hmi::page_files::{HmiObject, HmiPage, HmiPipe};

use super::geometry::{drawn_box, Rect};
use super::geometry_quality::{Finding, Measure, FINDING_LIMIT};
use super::routes::{pipe_routes, Point};

/// The margin inside each edge of a body, in pixels: a route on the body's edge is not in it.
const BODY_TOLERANCE: f64 = 2.0;
/// The top edges of a row of cards may differ by this many pixels and still form a row (§7 item 7).
const ROW_TOP_TOLERANCE: i64 = 16;
/// The gaps between the cards of a row may differ by this many pixels (§7 item 7).
const ROW_GAP_TOLERANCE: i64 = 8;

/// The routing findings of a page, at most the limit: pipes without a route, pipes through an
/// object's body, and rows of repeated cards that differ in size, alignment or spacing
/// (`routingQuality`). `size` is the design size.
#[must_use]
pub(super) fn routing_quality(page: &HmiPage, size: [u32; 2]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (index, (route, pipe)) in pipe_routes(page, size).iter().zip(&page.pipes).enumerate() {
        let ends = [end_object(&pipe.from), end_object(&pipe.to)];
        match route {
            None => findings.push(Finding {
                severity: "error",
                pipe: Some(index),
                objects: ends.iter().map(|end| (*end).to_owned()).collect(),
                ..Finding::warning(
                    "disconnected-port",
                    format!(
                        "Connection {} → {} has an unavailable endpoint; reconnect it to a declared port.",
                        pipe.from, pipe.to
                    ),
                )
            }),
            Some(points) => collide(page, index, pipe, ends, points, &mut findings),
        }
    }
    let cards = repeated_cards(page);
    for (name, members) in &cards {
        if members.len() > 1 && members.iter().any(|member| member.size != members[0].size) {
            findings.push(Finding {
                objects: ids(members),
                ..Finding::warning(
                    "repeated-equipment-size",
                    format!(
                        "Repeated {name} cards have different sizes; use equal-size or record the intentional emphasis."
                    ),
                )
            });
        }
    }
    row_findings(&cards, &mut findings);
    findings.truncate(FINDING_LIMIT);
    findings
}

/// The object a pipe's end names: the text before its first dot (`split(".")[0]` in
/// `routingQuality.ts`).
fn end_object(written: &str) -> &str {
    written
        .split_once('.')
        .map_or(written, |(object, _)| object)
}

/// The collisions of one pipe's route with the bodies of the page's objects. The pipe's own ends, the
/// instruments attached to it, and panels and groups are skipped. The search stops once the findings
/// reach the limit, as the `TypeScript`'s loop returns (the loop of `routingQuality`).
fn collide(
    page: &HmiPage,
    index: usize,
    pipe: &HmiPipe,
    ends: [&str; 2],
    route: &[Point],
    findings: &mut Vec<Finding>,
) {
    let pipe_tag = format!("pipe:{index}");
    for object in &page.objects {
        if ends.contains(&object.id.as_str())
            || object.attach.as_deref() == Some(pipe_tag.as_str())
            || matches!(object.kind.as_str(), "panel" | "group")
        {
            continue;
        }
        let body = drawn_box(object);
        if crosses_body(route, body) {
            findings.push(Finding {
                pipe: Some(index),
                object: Some(object.id.clone()),
                objects: vec![object.id.clone()],
                rectangle: Some(body),
                ..Finding::warning(
                    "pipe-object-collision",
                    format!(
                        "Pipe {} → {} crosses {}; move the object or reroute the pipe outside its body.",
                        pipe.from, pipe.to, object.id
                    ),
                )
            });
        }
        if findings.len() >= FINDING_LIMIT {
            break;
        }
    }
}

/// Whether a route runs through a body along one of its segments: a horizontal or a vertical segment
/// that lies inside the body, more than 2 px from each edge, along its length (`intersects` in
/// `routingQuality.ts`). A segment on the edge, or in the 2 px margin, is not a collision.
#[allow(clippy::float_cmp)] // The segments are axis-aligned by construction: equal coordinates are exact.
fn crosses_body(route: &[Point], body: Rect) -> bool {
    route.windows(2).any(|pair| {
        let (from_x, from_y) = (pair[0][0], pair[0][1]);
        let (to_x, to_y) = (pair[1][0], pair[1][1]);
        if to_x == from_x {
            to_x > body.x + BODY_TOLERANCE
                && to_x < body.x + body.width - BODY_TOLERANCE
                && to_y.max(from_y) > body.y + BODY_TOLERANCE
                && to_y.min(from_y) < body.y + body.height - BODY_TOLERANCE
        } else if to_y == from_y {
            to_y > body.y + BODY_TOLERANCE
                && to_y < body.y + body.height - BODY_TOLERANCE
                && to_x.max(from_x) > body.x + BODY_TOLERANCE
                && to_x.min(from_x) < body.x + body.width - BODY_TOLERANCE
        } else {
            false
        }
    })
}

/// The faceplate cards grouped by their panel, the groups in the order their first card appears. A
/// card's group is its `faceplate`, else its type (`repeated` in `routingQuality.ts`).
fn repeated_cards(page: &HmiPage) -> Vec<(String, Vec<&HmiObject>)> {
    let mut groups: Vec<(String, Vec<&HmiObject>)> = Vec::new();
    for object in page
        .objects
        .iter()
        .filter(|object| object.kind == "faceplate")
    {
        let key = object
            .faceplate
            .as_deref()
            .unwrap_or(object.kind.as_str())
            .to_owned();
        match groups.iter().position(|(name, _)| *name == key) {
            Some(index) => groups[index].1.push(object),
            None => groups.push((key, vec![object])),
        }
    }
    groups
}

/// The rows of repeated cards that are not aligned or not evenly spaced. A row is inferred only from
/// at least three cards whose top edges lie within 16 px and which do not overlap horizontally;
/// separate process rows and deliberately stacked cards are not layout errors (the row part of
/// `routingQuality.ts`).
fn row_findings(cards: &[(String, Vec<&HmiObject>)], findings: &mut Vec<Finding>) {
    for (name, members) in cards {
        if members.len() < 3 {
            continue;
        }
        let mut row = members.clone();
        row.sort_by_key(|object| object.at[0]);
        let top = row
            .iter()
            .fold(i64::MAX, |best, object| best.min(object.at[1]));
        let bottom = row
            .iter()
            .fold(i64::MIN, |best, object| best.max(object.at[1]));
        let overlapping = row
            .windows(2)
            .any(|pair| pair[1].at[0] < pair[0].at[0] + i64::from(pair[0].size[0]));
        if bottom - top > ROW_TOP_TOLERANCE || overlapping {
            continue;
        }
        if bottom != top {
            findings.push(Finding {
                objects: ids(&row),
                measured: Some(Measure::Edges { top, bottom }),
                ..Finding::warning(
                    "repeated-equipment-alignment",
                    format!(
                        "Repeated {name} cards are nearly, but not exactly, aligned; align their top edges or record the intentional offset."
                    ),
                )
            });
        }
        let gaps: Vec<i64> = row
            .windows(2)
            .map(|pair| pair[1].at[0] - pair[0].at[0] - i64::from(pair[0].size[0]))
            .collect();
        let widest = gaps.iter().copied().fold(i64::MIN, i64::max);
        let narrowest = gaps.iter().copied().fold(i64::MAX, i64::min);
        if widest - narrowest > ROW_GAP_TOLERANCE {
            findings.push(Finding {
                objects: ids(&row),
                measured: Some(Measure::Gaps { gaps }),
                ..Finding::warning(
                    "repeated-equipment-spacing",
                    format!(
                        "Repeated {name} cards have uneven row spacing; distribute them evenly or record the intentional grouping."
                    ),
                )
            });
        }
    }
}

/// The ids of the given objects, in their order.
fn ids(members: &[&HmiObject]) -> Vec<String> {
    members.iter().map(|member| member.id.clone()).collect()
}

#[cfg(test)]
#[path = "routing_quality_tests.rs"]
mod tests;
