//! The geometry findings of a draft review (specification 43, §4 and §7 items 2 and 4): objects that
//! reach past the design page, unintended overlaps, and, at each target viewport, the adapted
//! layout's scale and each control's size there.
//!
//! Port of `editors/vscode/src/hmiBuilder/agent/geometryQuality.ts` (`viewportProfiles` and
//! `geometryQuality`). Geometry is a review aid, not a beauty score or a proof of a rendered
//! physical panel. This module defines `Finding`, the record the routing review shares.

use std::collections::HashSet;

use serde::Serialize;
use serde_json::Value;
use trust_runtime::hmi::catalogue::{Catalogue, ComponentCategory};
use trust_runtime::hmi::page_files::{HmiObject, HmiPage};

use super::adaptive::adapt_page;
use super::error::{AgentError, AgentResult};
use super::geometry::{contains, drawn_box, Rect};

/// The most findings one review reports; the rest are counted as omitted (§7).
pub(super) const FINDING_LIMIT: usize = 200;
/// The least width and height of a touch target, in pixels (§4 item 2, §7 item 4).
const TOUCH_TARGET: f64 = 44.0;

/// One review finding, serialized with the keys of the `TypeScript` records. Optional keys are left
/// out when the finding has no such value.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct Finding {
    /// `warning` or `error`.
    pub(super) severity: &'static str,
    /// The stable name of the rule that found it.
    pub(super) rule: &'static str,
    /// The index of the pipe, for a finding about one pipe.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) pipe: Option<usize>,
    /// The object, for a finding about one object.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) object: Option<String>,
    /// Every object the finding names.
    pub(super) objects: Vec<String>,
    /// The viewport, for a finding about one target viewport.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) viewport: Option<String>,
    /// The rectangle the finding is about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) rectangle: Option<Rect>,
    /// What was measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) measured: Option<Measure>,
    /// What the rule requires.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) required: Option<Measure>,
    /// The sentence that names the values involved and the change to make.
    pub(super) message: String,
}

impl Finding {
    /// A warning with its rule and message. The caller sets the objects and measures it reports:
    /// `Finding { objects, ..Finding::warning(rule, message) }`.
    #[must_use]
    pub(super) fn warning(rule: &'static str, message: String) -> Self {
        Self {
            severity: "warning",
            rule,
            pipe: None,
            object: None,
            objects: Vec::new(),
            viewport: None,
            rectangle: None,
            measured: None,
            required: None,
            message,
        }
    }
}

/// What a finding measured or requires. Serialized as the bare values, as the `TypeScript` records
/// are.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub(super) enum Measure {
    /// A rectangle: an object's box, or the page's bounds (`outside-page`).
    Rectangle(Rect),
    /// A measured width and height in viewport pixels (`viewport-touch-target`).
    Size {
        /// The width.
        width: f64,
        /// The height.
        height: f64,
    },
    /// The required touch target of 44 × 44 pixels, which serializes as whole numbers.
    Target {
        /// The width.
        width: u32,
        /// The height.
        height: u32,
    },
    /// The top and bottom edges of a row of repeated cards (`repeated-equipment-alignment`).
    Edges {
        /// The least top edge.
        top: i64,
        /// The greatest top edge.
        bottom: i64,
    },
    /// The gaps between the cards of a row, in order (`repeated-equipment-spacing`).
    Gaps {
        /// The gap between each card and the next.
        gaps: Vec<i64>,
    },
}

/// A target viewport a draft is reviewed at (`ViewportProfile` in `geometryQuality.ts`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ViewportProfile {
    /// The identifier, unique within the request.
    pub(super) id: String,
    /// The width of the available content area in pixels, from 1 to 4096.
    pub(super) width: u32,
    /// The height of the available content area in pixels, from 1 to 4096.
    pub(super) height: u32,
}

/// An object's rectangle in one viewport, serialized as `{ id, x, y, width, height }`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct ViewportObject {
    /// The object's id.
    pub(super) id: String,
    /// Its rectangle in viewport pixels.
    #[serde(flatten)]
    pub(super) rectangle: Rect,
}

/// The review of the page at one target viewport (`viewports` of `geometryQuality`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct ViewportReview {
    /// The viewport's identifier.
    pub(super) id: String,
    /// Its width in pixels.
    pub(super) width: u32,
    /// Its height in pixels.
    pub(super) height: u32,
    /// The factor the adapted page is drawn at in the viewport.
    pub(super) scale: f64,
    /// Which layout the page is reviewed in.
    pub(super) origin: &'static str,
    /// The size of the adapted content in design pixels, width then height.
    #[serde(rename = "contentSize")]
    pub(super) content_size: [f64; 2],
    /// Whether the content scrolls: it is not scaled to fit.
    pub(super) scroll: bool,
    /// Each object's rectangle in the viewport.
    pub(super) objects: Vec<ViewportObject>,
}

/// The geometry review of a page (`geometryQuality`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct GeometryReview {
    /// The findings, up to the limit.
    pub(super) findings: Vec<Finding>,
    /// The review at each target viewport, in request order.
    pub(super) viewports: Vec<ViewportReview>,
    /// Whether findings were left out past the limit.
    pub(super) truncated: bool,
    /// How many findings were left out.
    pub(super) omitted: usize,
}

/// The findings kept for one review, and how many were left out past the limit (the `add` of
/// `geometryQuality`).
#[derive(Default)]
struct Report {
    /// The findings kept, in the order they were found.
    findings: Vec<Finding>,
    /// How many were found past the limit.
    omitted: usize,
}

impl Report {
    /// Keeps a finding while the review is under the limit, and counts it as omitted past it.
    fn add(&mut self, finding: Finding) {
        if self.findings.len() < FINDING_LIMIT {
            self.findings.push(finding);
        } else {
            self.omitted += 1;
        }
    }
}

/// The target viewports of a request: none when the field is absent; at most three, each with a
/// unique identifier and whole dimensions from 1 to 4096 pixels (`viewportProfiles`).
///
/// # Errors
///
/// `INVALID_ARGUMENT` when the value is not an array of at most three viewports, or a viewport is
/// malformed, repeats an identifier, or has a dimension out of range.
pub(super) fn viewport_profiles(value: Option<&Value>) -> AgentResult<Vec<ViewportProfile>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .filter(|items| items.len() <= 3)
        .ok_or_else(|| AgentError::invalid("provide at most three target content viewports."))?;
    let mut ids: HashSet<String> = HashSet::new();
    let mut profiles = Vec::with_capacity(items.len());
    for item in items {
        let profile = parse_profile(item).ok_or_else(profiles_refused)?;
        if !ids.insert(profile.id.clone()) {
            return Err(profiles_refused());
        }
        profiles.push(profile);
    }
    Ok(profiles)
}

/// The refusal of a malformed viewport (`viewportProfiles`).
fn profiles_refused() -> AgentError {
    AgentError::invalid("viewports need unique IDs and positive dimensions up to 4096 pixels.")
}

/// A viewport from its JSON object: exactly the keys `height`, `id` and `width`, an identifier that
/// matches `^[A-Za-z][A-Za-z0-9_-]{0,63}$`, and whole dimensions from 1 to 4096.
fn parse_profile(value: &Value) -> Option<ViewportProfile> {
    let object = value.as_object()?;
    let exact_keys = object.len() == 3
        && ["height", "id", "width"]
            .iter()
            .all(|key| object.contains_key(*key));
    if !exact_keys {
        return None;
    }
    let id = object.get("id")?.as_str()?;
    if !is_viewport_id(id) {
        return None;
    }
    Some(ViewportProfile {
        id: id.to_owned(),
        width: dimension(object.get("width")?)?,
        height: dimension(object.get("height")?)?,
    })
}

/// Whether an identifier matches `^[A-Za-z][A-Za-z0-9_-]{0,63}$`: a letter, then up to 63 letters,
/// digits, underscores or hyphens.
fn is_viewport_id(id: &str) -> bool {
    id.chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && id.len() <= 64
        && id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
}

/// A whole pixel count from 1 to 4096: `Number.isInteger(size) && size > 0 && size <= 4096`. A JSON
/// number written with a zero fraction, such as `1280.0`, is whole, as it is in `JavaScript`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Whole and 1..=4096 here.
fn dimension(value: &Value) -> Option<u32> {
    let number = value.as_f64()?;
    if number.fract() != 0.0 || !(1.0..=4096.0).contains(&number) {
        return None;
    }
    Some(number as u32)
}

/// The review of a page: the objects that reach past the design page, the unintended overlaps, and,
/// at each target viewport, the adapted layout's scale and offsets, each object's rectangle, and each
/// control that is smaller than 44 px there (`geometryQuality`). `size` is the design size.
#[must_use]
pub(super) fn geometry_quality(
    page: &HmiPage,
    size: [u32; 2],
    catalogue: &Catalogue,
    profiles: &[ViewportProfile],
) -> GeometryReview {
    let mut report = Report::default();
    let boxes: Vec<Rect> = page.objects.iter().map(drawn_box).collect();
    let page_box = Rect {
        x: 0.0,
        y: 0.0,
        width: f64::from(size[0]),
        height: f64::from(size[1]),
    };
    for (object, body) in page.objects.iter().zip(&boxes) {
        if !contains(page_box, *body) {
            report.add(Finding {
                object: Some(object.id.clone()),
                objects: vec![object.id.clone()],
                rectangle: Some(*body),
                measured: Some(Measure::Rectangle(*body)),
                required: Some(Measure::Rectangle(page_box)),
                ..Finding::warning(
                    "outside-page",
                    format!(
                        "{} extends beyond the design page; move or resize it within the page bounds.",
                        object.id
                    ),
                )
            });
        }
    }
    add_overlaps(page, &boxes, &mut report);
    let viewports: Vec<ViewportReview> = profiles
        .iter()
        .map(|profile| viewport_review(page, size, catalogue, profile, &mut report))
        .collect();
    GeometryReview {
        findings: report.findings,
        viewports,
        truncated: report.omitted > 0,
        omitted: report.omitted,
    }
}

/// The unintended overlaps of the page's objects (§7 item 2). A background that holds another object
/// wholly, and an instrument attached to its equipment, are intentional; touching edges are not an
/// overlap, so only a positive area is reported.
fn add_overlaps(page: &HmiPage, boxes: &[Rect], report: &mut Report) {
    let background = |object: &HmiObject| matches!(object.kind.as_str(), "group" | "panel");
    for (index, (first, first_box)) in page.objects.iter().zip(boxes).enumerate() {
        for (second, second_box) in page.objects.iter().zip(boxes).skip(index + 1) {
            let intentional = (background(first) && contains(*first_box, *second_box))
                || (background(second) && contains(*second_box, *first_box))
                || first.attach.as_deref() == Some(second.id.as_str())
                || second.attach.as_deref() == Some(first.id.as_str());
            if intentional {
                continue;
            }
            let x = first_box.x.max(second_box.x);
            let y = first_box.y.max(second_box.y);
            let width = (first_box.x + first_box.width).min(second_box.x + second_box.width) - x;
            let height = (first_box.y + first_box.height).min(second_box.y + second_box.height) - y;
            if width > 0.0 && height > 0.0 {
                report.add(Finding {
                    objects: vec![first.id.clone(), second.id.clone()],
                    rectangle: Some(Rect {
                        x,
                        y,
                        width,
                        height,
                    }),
                    ..Finding::warning(
                        "object-overlap",
                        format!(
                            "{} and {} overlap; separate them or review the intended overlay.",
                            first.id, second.id
                        ),
                    )
                });
            }
        }
    }
}

/// The review at one target viewport: the adapted layout fitted to the viewport when it is at least
/// the design size, else shown at scale 1 and scrolled; each object's rectangle there; and each touch
/// target smaller than 44 px (the per-viewport part of `geometryQuality`).
fn viewport_review(
    page: &HmiPage,
    size: [u32; 2],
    catalogue: &Catalogue,
    profile: &ViewportProfile,
    report: &mut Report,
) -> ViewportReview {
    let adapted = adapt_page(page, size, [profile.width, profile.height]);
    let (scale, offset_x, offset_y) = if adapted.fit {
        let scale = (f64::from(profile.width) / adapted.size[0])
            .min(f64::from(profile.height) / adapted.size[1]);
        (
            scale,
            (f64::from(profile.width) - adapted.size[0] * scale) / 2.0,
            (f64::from(profile.height) - adapted.size[1] * scale) / 2.0,
        )
    } else {
        (1.0, 0.0, 0.0)
    };
    let mut objects = Vec::with_capacity(adapted.page.objects.len());
    for (object, body) in adapted.page.objects.iter().zip(&adapted.boxes) {
        let rectangle = Rect {
            x: offset_x + body.x * scale,
            y: offset_y + body.y * scale,
            width: body.width * scale,
            height: body.height * scale,
        };
        if is_touch_target(catalogue, object)
            && (rectangle.width < TOUCH_TARGET || rectangle.height < TOUCH_TARGET)
        {
            report.add(Finding {
                viewport: Some(profile.id.clone()),
                object: Some(object.id.clone()),
                objects: vec![object.id.clone()],
                rectangle: Some(rectangle),
                measured: Some(Measure::Size {
                    width: rectangle.width,
                    height: rectangle.height,
                }),
                required: Some(Measure::Target {
                    width: 44,
                    height: 44,
                }),
                ..Finding::warning(
                    "viewport-touch-target",
                    format!(
                        "{} is {} × {} px in {}; use a larger control, readable scrolling or another page to retain a 44 × 44 px target.",
                        object.id,
                        to_fixed_one(rectangle.width),
                        to_fixed_one(rectangle.height),
                        profile.id
                    ),
                )
            });
        }
        objects.push(ViewportObject {
            id: object.id.clone(),
            rectangle,
        });
    }
    ViewportReview {
        id: profile.id.clone(),
        width: profile.width,
        height: profile.height,
        scale,
        origin: "shared-adaptive-layout",
        content_size: adapted.size,
        scroll: !adapted.fit,
        objects,
    }
}

/// Whether an object is a touch control: a control component, a page button, or an object with a
/// click (the `touch` test of `geometryQuality`).
fn is_touch_target(catalogue: &Catalogue, object: &HmiObject) -> bool {
    catalogue
        .component(&object.kind)
        .is_some_and(|component| component.category == ComponentCategory::Control)
        || object.kind == "page-button"
        || object.click.is_some()
}

/// `Number.prototype.toFixed(1)` for a measured size: the nearest tenth, with the sign kept for a
/// negative value. An exact tie, a quarter such as `x.25`, goes to the larger tenth as `ECMAScript`
/// rounds it; Rust's formatting rounds an exact tie to the even tenth.
#[allow(clippy::float_cmp)] // Exact on purpose: only an exact quarter is a tie.
fn to_fixed_one(value: f64) -> String {
    let magnitude = value.abs();
    let whole = magnitude.trunc();
    let tenths = if magnitude - whole == 0.25 {
        format!("{whole:.0}.3")
    } else {
        format!("{magnitude:.1}")
    };
    if value < 0.0 {
        format!("-{tenths}")
    } else {
        tenths
    }
}

#[cfg(test)]
#[path = "geometry_quality_tests.rs"]
mod tests;
