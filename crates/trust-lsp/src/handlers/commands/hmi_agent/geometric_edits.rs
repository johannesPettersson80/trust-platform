//! Geometric edits of an agent draft (specification 43, AT-05): equal sizing and regional
//! arrangement of a selection.
//!
//! Port of `agent/geometricEdits.ts`. Both return a new page; the caller replaces the draft only
//! when the whole batch succeeds.

use serde_json::{json, Value};
use trust_runtime::hmi::page_files::{HmiPage, PLACED_FACEPLATE};

use super::error::{AgentError, AgentResult};
use super::geometry::{drawn_box, js_round};
use super::layout::{
    constrained_regions, gap_and_flow, place_region, CompositionRole, Obstacle, Region, SizedItem,
};

/// Makes the selected objects share a width, a height or both, taken from `reference`
/// (`equalSize`). A faceplate keeps its proportions; a choice that would break them is refused.
///
/// # Errors
///
/// Returns `INVALID_ARGUMENT` for an axis other than width, height or both, and
/// `VALIDATION_FAILED` for a faceplate that cannot keep its proportions or an object that would
/// become smaller than 8 pixels.
pub(super) fn equal_size(
    page: &HmiPage,
    ids: &[String],
    reference: &str,
    axis: Option<&Value>,
) -> AgentResult<HmiPage> {
    let axis = axis
        .and_then(Value::as_str)
        .filter(|axis| matches!(*axis, "width" | "height" | "both"))
        .ok_or_else(|| AgentError::invalid("equal-size requires width, height or both."))?;
    let reference_object = page
        .objects
        .iter()
        .find(|object| object.id == reference)
        .ok_or_else(|| AgentError::invalid(format!("unknown object {reference}.")))?;
    let mut sizes: Vec<(String, [u32; 2])> = Vec::with_capacity(ids.len());
    for id in ids {
        let object = page
            .objects
            .iter()
            .find(|object| object.id == *id)
            .ok_or_else(|| AgentError::invalid(format!("unknown object {id}.")))?;
        let mut width = f64::from(if axis == "height" {
            object.size[0]
        } else {
            reference_object.size[0]
        });
        let mut height = f64::from(if axis == "width" {
            object.size[1]
        } else {
            reference_object.size[1]
        });
        if object.kind == PLACED_FACEPLATE {
            let ratio = f64::from(object.size[1]) / f64::from(object.size[0]);
            if axis == "both" && (height - js_round(width * ratio)).abs() > 1.0 {
                return Err(AgentError::validation(format!(
                    "{} cannot match both dimensions without changing faceplate proportions.",
                    object.id
                )));
            }
            if axis == "width" {
                height = js_round(width * ratio);
            }
            if axis == "height" {
                width = js_round(height / ratio);
            }
        }
        if width < 8.0 || height < 8.0 {
            return Err(AgentError::validation(format!(
                "{} would become smaller than the minimum dimensions.",
                object.id
            )));
        }
        sizes.push((object.id.clone(), [side(width), side(height)]));
    }
    let mut result = page.clone();
    for object in &mut result.objects {
        if let Some((_, size)) = sizes.iter().find(|(id, _)| *id == object.id) {
            object.size = *size;
        }
    }
    Ok(result)
}

/// Arranges the selected objects in a region of the page (`arrangeRegion`). Pinned and locked
/// objects keep their position; the other objects take free cells of the region in the flow
/// direction, avoiding every object that is not part of the selection and is not a panel or a
/// group.
///
/// # Errors
///
/// Returns `INVALID_ARGUMENT` when the project's design size is unknown, the region or the pinned
/// IDs are malformed, and `VALIDATION_FAILED` when the items do not fit at readable sizes.
pub(super) fn arrange_region(
    page: &HmiPage,
    ids: &[String],
    operation: &Value,
    size: Option<[u32; 2]>,
) -> AgentResult<HmiPage> {
    let size = size.ok_or_else(|| {
        AgentError::invalid("regional arrangement needs the project's design size.")
    })?;
    let (gap, flow) = gap_and_flow(operation.get("gap"), operation.get("flow"))?;
    let regions = json!({
        "process": operation.get("region").cloned().unwrap_or(Value::Null),
    });
    let bounds = Region {
        x: 0,
        y: 0,
        width: i64::from(size[0]),
        height: i64::from(size[1]),
    };
    let areas = constrained_regions(Vec::new(), bounds, Some(&regions))?;
    let region = areas
        .iter()
        .find(|(role, _)| *role == CompositionRole::Process)
        .map(|(_, region)| *region)
        .ok_or_else(|| AgentError::invalid("process region is missing."))?;
    let pinned = pinned_ids(operation.get("pinnedIds"), ids)?;
    let obstacles: Vec<Obstacle> = page
        .objects
        .iter()
        .filter(|object| {
            !ids.contains(&object.id) && object.kind != "group" && object.kind != "panel"
        })
        .map(|object| Obstacle {
            id: object.id.clone(),
            boxed: drawn_box(object),
        })
        .collect();
    let mut items = Vec::with_capacity(ids.len());
    for id in ids {
        let object = page
            .objects
            .iter()
            .find(|object| object.id == *id)
            .ok_or_else(|| AgentError::invalid(format!("unknown object {id}.")))?;
        let fixed = object.locked || pinned.contains(id);
        items.push(SizedItem {
            id: id.clone(),
            size: [i64::from(object.size[0]), i64::from(object.size[1])],
            rotation: object.rotation,
            pinned_at: fixed.then(|| json!([object.at[0], object.at[1]])),
        });
    }
    let placed = place_region(
        &items,
        region,
        CompositionRole::Process,
        gap,
        flow,
        &obstacles,
    )?;
    let mut result = page.clone();
    for object in &mut result.objects {
        if let Some(at) = placed.get(&object.id) {
            object.at = *at;
        }
    }
    Ok(result)
}

/// The pinned IDs of an arrangement: unique members of the selection (`pinnedIds`; null means
/// none).
fn pinned_ids(value: Option<&Value>, ids: &[String]) -> AgentResult<Vec<String>> {
    let refused =
        || AgentError::invalid("pinned IDs must be unique members of the arranged selection.");
    let list = match value.filter(|value| !value.is_null()) {
        None => return Ok(Vec::new()),
        Some(value) => value.as_array().ok_or_else(refused)?,
    };
    let mut pinned: Vec<String> = Vec::with_capacity(list.len());
    for entry in list {
        let id = entry
            .as_str()
            .filter(|id| ids.iter().any(|selected| selected.as_str() == *id))
            .ok_or_else(refused)?;
        if pinned.iter().any(|known| known.as_str() == id) {
            return Err(refused());
        }
        pinned.push(id.to_owned());
    }
    Ok(pinned)
}

/// A rounded side back as the model stores it. Sides are checked to be at least 8 pixels and
/// finite before this is called.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn side(value: f64) -> u32 {
    value as u32
}
