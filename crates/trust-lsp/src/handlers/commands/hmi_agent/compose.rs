//! Deterministic initial composition of a page (specification 43, AT-04 and §6).
//!
//! Port of `agent/compose.ts`. The tool lays out the constraints the calling agent chose; it never
//! calls a model, never infers a process connection and never invents a binding. The items are
//! written as ordinary objects through `edit_page`, so the add rules (unique IDs, catalogue types,
//! sizes on the grid) are the same rules an edit meets.

use serde_json::{json, Map, Value};
use trust_runtime::hmi::catalogue::Catalogue;
use trust_runtime::hmi::page_files::{HmiPage, PLACED_FACEPLATE};

use super::edits::{edit_page, page_from_json};
use super::error::{AgentError, AgentResult};
use super::geometry::coord;
use super::layout::{
    constrained_regions, parse_layout, place_region, snap, CompositionRole, EqualSize,
    LayoutConstraints, Region, SizedItem,
};
use super::patterns::PatternId;

/// The refusal for a composition that fails its basic checks (`compose.ts`).
const INVALID_CONSTRAINTS: &str = "invalid composition constraints.";

/// The most items one composition may place.
const MAX_ITEMS: usize = 500;

/// One item of a composition request: its object properties and its layout fields.
struct Item {
    /// The object's ID.
    id: String,
    /// The component type.
    kind: String,
    /// The region the item goes in.
    region: CompositionRole,
    /// The `pinnedAt` value, when the request wrote one.
    pinned_at: Option<Value>,
    /// The `size` value, when the request wrote one.
    size: Option<Value>,
    /// Every other property of the item, as the object will store it.
    properties: Map<String, Value>,
}

/// A validated composition request (`CompositionInput` in `compose.ts`).
struct Input {
    /// The page title, which is also the heading's text.
    title: String,
    /// The pattern whose default regions place items that name none.
    pattern: PatternId,
    /// The layout constraints, checked.
    layout: LayoutConstraints,
    /// The design size in pixels.
    size: [i64; 2],
    /// The items in request order.
    items: Vec<Item>,
    /// Port connections, each written `object.port`, in request order.
    connections: Vec<(Value, Value)>,
}

/// An item with its final size, ready to place.
struct Sized {
    /// Index into the input's items.
    index: usize,
    /// Width and height.
    size: [i64; 2],
    /// Whether the request gave a size, which then stays as written.
    size_given: bool,
}

/// Composes a page from a composition request (`composeAgentPage`).
///
/// # Errors
///
/// Returns `INVALID_ARGUMENT` for a malformed request, an unknown component or region, duplicate
/// or reserved IDs and unusable sizes; `VALIDATION_FAILED` and `CONFLICT` for constraints that
/// cannot be satisfied, naming the objects or regions involved.
pub(super) fn compose_agent_page(request: &Value, catalogue: &Catalogue) -> AgentResult<HmiPage> {
    let input = parse_input(request, catalogue)?;
    let (width, height) = (input.size[0], input.size[1]);
    let inner = Region {
        x: 32,
        y: 88,
        width: snap(coord(width - 64)),
        height: snap(coord(height - 120)),
    };
    let groups = group_by_region(&input.items);
    let areas = regions_for(&groups, inner, &input.layout)?;
    let heading = json!({
        "id": "page-heading",
        "type": "text",
        "variant": "heading",
        "at": [32, 24],
        "size": [snap(coord(width - 64)), 40],
        "rotation": 0,
        "text": input.title,
        "bind": {},
    });
    let mut page = page_from_json(&json!({
        "title": input.title,
        "object": [heading],
        "pipe": [],
    }))?;
    let mut operations = Vec::new();
    for (role, members) in &groups {
        let outer = area_of(&areas, *role)?;
        let area = padded(outer, input.layout.padding.unwrap_or(0), *role)?;
        let sized = size_group(&input, members, area, catalogue, &input.layout)?;
        place_group(&input, &sized, *role, area, &input.layout, &mut operations)?;
    }
    if !operations.is_empty() {
        page = edit_page(&page, &operations, catalogue, None)?;
    }
    if !input.connections.is_empty() {
        let connections: Vec<Value> = input
            .connections
            .iter()
            .map(|(from, to)| json!({"op": "connect", "from": from, "to": to}))
            .collect();
        page = edit_page(&page, &connections, catalogue, None)?;
    }
    Ok(page)
}

/// Reads and checks a composition request, in the TypeScript's order.
fn parse_input(request: &Value, catalogue: &Catalogue) -> AgentResult<Input> {
    let invalid = || AgentError::invalid(INVALID_CONSTRAINTS);
    let object = request.as_object().ok_or_else(invalid)?;
    let title = object
        .get("title")
        .and_then(Value::as_str)
        .filter(|title| !title.trim().is_empty())
        .ok_or_else(invalid)?;
    let pattern = object
        .get("pattern")
        .and_then(Value::as_str)
        .and_then(PatternId::parse)
        .ok_or_else(invalid)?;
    let size = object
        .get("size")
        .and_then(Value::as_array)
        .filter(|size| size.len() == 2)
        .and_then(|size| Some([whole_size(&size[0])?, whole_size(&size[1])?]))
        .ok_or_else(invalid)?;
    let items = object
        .get("items")
        .and_then(Value::as_array)
        .filter(|items| items.len() <= MAX_ITEMS)
        .ok_or_else(invalid)?;
    let layout = parse_layout(object.get("layout"))?;
    let items = parse_items(items, pattern, catalogue)?;
    let connections = match object.get("connections") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(list)) => list
            .iter()
            .map(|connection| {
                let from = connection.get("from").cloned().unwrap_or(Value::Null);
                let to = connection.get("to").cloned().unwrap_or(Value::Null);
                (from, to)
            })
            .collect(),
        Some(_) => return Err(AgentError::invalid("connections must be a list.")),
    };
    Ok(Input {
        title: title.to_owned(),
        pattern,
        layout,
        size,
        items,
        connections,
    })
}

/// A design size: a whole number from 240 to 4096 pixels.
fn whole_size(value: &Value) -> Option<i64> {
    value.as_i64().filter(|size| (240..=4096).contains(size))
}

/// Reads the items: unique IDs that do not use the heading's, known component types and known
/// regions (the pattern's default region applies when an item names none).
fn parse_items(
    items: &[Value],
    pattern: PatternId,
    catalogue: &Catalogue,
) -> AgentResult<Vec<Item>> {
    let mut ids = Vec::with_capacity(items.len());
    for item in items {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::invalid("invalid object id."))?;
        ids.push(id);
    }
    let unique = ids
        .iter()
        .enumerate()
        .all(|(index, id)| !ids[..index].contains(id));
    if ids.contains(&"page-heading") || !unique {
        return Err(AgentError::invalid(
            "object ids must be unique and must not use page-heading.",
        ));
    }
    let mut parsed = Vec::with_capacity(items.len());
    for item in items {
        let object = item
            .as_object()
            .ok_or_else(|| AgentError::invalid(INVALID_CONSTRAINTS))?;
        let kind = object
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if catalogue.component(kind).is_none() && kind != PLACED_FACEPLATE {
            return Err(AgentError::invalid(format!("unknown component {kind}.")));
        }
        let region = match object.get("region").filter(|value| !value.is_null()) {
            None => default_region(pattern),
            Some(value) => value
                .as_str()
                .and_then(CompositionRole::parse)
                .ok_or_else(|| AgentError::invalid("unknown region."))?,
        };
        let mut properties = object.clone();
        let pinned_at = properties.remove("pinnedAt");
        properties.remove("region");
        parsed.push(Item {
            id: id_text(object),
            kind: kind.to_owned(),
            region,
            pinned_at,
            size: object.get("size").cloned(),
            properties,
        });
    }
    Ok(parsed)
}

/// The `id` of an item whose presence was checked in `parse_items`.
fn id_text(object: &Map<String, Value>) -> String {
    object
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// The region of an item that names none, by pattern (`compose.ts`).
fn default_region(pattern: PatternId) -> CompositionRole {
    match pattern {
        PatternId::Settings => CompositionRole::Controls,
        PatternId::Investigation => CompositionRole::Trends,
        PatternId::ProcessOverview | PatternId::EquipmentDetail | PatternId::MultiUnit => {
            CompositionRole::Process
        }
    }
}

/// The items of each region, in the order regions first appear (a `Map` in the TypeScript).
fn group_by_region(items: &[Item]) -> Vec<(CompositionRole, Vec<usize>)> {
    let mut groups: Vec<(CompositionRole, Vec<usize>)> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        match groups.iter_mut().find(|(role, _)| *role == item.region) {
            Some((_, members)) => members.push(index),
            None => groups.push((item.region, vec![index])),
        }
    }
    groups
}

/// The regions of the groups in use, then the explicit regions over them (`areas` in
/// `compose.ts`): the primary region takes about two thirds of the upper width, the side regions
/// stack beside it, and the lower regions share the bottom.
fn regions_for(
    groups: &[(CompositionRole, Vec<usize>)],
    inner: Region,
    layout: &LayoutConstraints,
) -> AgentResult<Vec<(CompositionRole, Region)>> {
    let gap = layout.gap;
    let used = |role: CompositionRole| groups.iter().any(|(group, _)| *group == role);
    let bottom: Vec<CompositionRole> = [CompositionRole::Trends, CompositionRole::Alarms]
        .into_iter()
        .filter(|role| used(*role))
        .collect();
    let upper: Vec<CompositionRole> = [
        CompositionRole::Process,
        CompositionRole::Measurements,
        CompositionRole::Controls,
    ]
    .into_iter()
    .filter(|role| used(*role))
    .collect();
    let has_both = !bottom.is_empty() && !upper.is_empty();
    let upper_height = if has_both {
        snap(coord(inner.height - gap) * 0.6)
    } else {
        inner.height
    };
    let bottom_area = if has_both {
        Region {
            y: inner.y + upper_height + gap,
            height: inner.height - upper_height - gap,
            ..inner
        }
    } else {
        inner
    };
    let mut areas: Vec<(CompositionRole, Region)> = Vec::new();
    if let Some(&first) = upper.first() {
        let primary = if used(CompositionRole::Process) {
            CompositionRole::Process
        } else {
            first
        };
        let aside: Vec<CompositionRole> = upper
            .iter()
            .copied()
            .filter(|role| *role != primary)
            .collect();
        let primary_width = if aside.is_empty() {
            inner.width
        } else {
            snap(coord(inner.width - gap) * 0.67)
        };
        set_area(
            &mut areas,
            primary,
            Region {
                width: primary_width,
                height: upper_height,
                ..inner
            },
        );
        let side_count = count(aside.len());
        for (index, role) in aside.iter().enumerate() {
            let index = count(index);
            let area_height =
                snap(coord(upper_height - gap * (side_count - 1)) / coord(side_count));
            set_area(
                &mut areas,
                *role,
                Region {
                    x: inner.x + primary_width + gap,
                    y: inner.y + index * (area_height + gap),
                    width: inner.width - primary_width - gap,
                    height: area_height,
                },
            );
        }
    }
    let bottom_count = count(bottom.len());
    for (index, role) in bottom.iter().enumerate() {
        let index = count(index);
        let area_width =
            snap(coord(bottom_area.width - gap * (bottom_count - 1)) / coord(bottom_count));
        set_area(
            &mut areas,
            *role,
            Region {
                x: bottom_area.x + index * (area_width + gap),
                width: area_width,
                ..bottom_area
            },
        );
    }
    constrained_regions(areas, inner, layout.regions.as_ref())
}

/// A length as a count (a number of regions or items, never more than a few hundred).
fn count(length: usize) -> i64 {
    i64::try_from(length).unwrap_or(i64::MAX)
}

/// Sets a region's area, keeping its place if it already has one.
fn set_area(areas: &mut Vec<(CompositionRole, Region)>, role: CompositionRole, region: Region) {
    match areas.iter_mut().find(|(existing, _)| *existing == role) {
        Some(entry) => entry.1 = region,
        None => areas.push((role, region)),
    }
}

/// The area a group is placed in.
fn area_of(areas: &[(CompositionRole, Region)], role: CompositionRole) -> AgentResult<Region> {
    areas
        .iter()
        .find(|(existing, _)| *existing == role)
        .map(|(_, region)| *region)
        .ok_or_else(|| AgentError::invalid(format!("unknown region {}.", role.as_str())))
}

/// The region less its padding on every side (`padding consumes the ... region`).
fn padded(outer: Region, padding: i64, role: CompositionRole) -> AgentResult<Region> {
    let area = Region {
        x: outer.x + padding,
        y: outer.y + padding,
        width: outer.width - padding * 2,
        height: outer.height - padding * 2,
    };
    if area.width <= 0 || area.height <= 0 {
        return Err(AgentError::validation(format!(
            "padding consumes the {} region.",
            role.as_str()
        )));
    }
    Ok(area)
}

/// Gives each item of a group its size: the request's size, else the catalogue's; a sparse
/// process overview is enlarged, readings and controls take the region's width, and equal sizes
/// are applied when asked.
fn size_group(
    input: &Input,
    members: &[usize],
    area: Region,
    catalogue: &Catalogue,
    layout: &LayoutConstraints,
) -> AgentResult<Vec<Sized>> {
    let mut sized = Vec::with_capacity(members.len());
    for &index in members {
        let item = &input.items[index];
        let size_given = item.size.as_ref().is_some_and(|value| !value.is_null());
        let size = match item.size.as_ref().filter(|value| !value.is_null()) {
            Some(value) => usable_size(value),
            None => catalogue
                .component(&item.kind)
                .map(|definition| [i64::from(definition.size[0]), i64::from(definition.size[1])]),
        }
        .ok_or_else(|| AgentError::invalid(format!("provide a usable size for {}.", item.id)))?;
        sized.push(Sized {
            index,
            size,
            size_given,
        });
    }
    let role = input.items[members[0]].region;
    if role == CompositionRole::Process
        && matches!(
            input.pattern,
            PatternId::ProcessOverview | PatternId::EquipmentDetail
        )
        && sized.len() <= 6
    {
        enlarge_sparse(input, &mut sized, area, layout.gap);
    }
    if matches!(
        role,
        CompositionRole::Measurements | CompositionRole::Controls
    ) {
        for entry in &mut sized {
            if !entry.size_given && !is_pinned(&input.items[entry.index]) {
                entry.size = [snap(coord(area.width)), entry.size[1].max(80)];
            }
        }
    }
    if let Some(equal) = layout.equal_size {
        equalize(input, &mut sized, equal)?;
    }
    Ok(sized)
}

/// Enlarges the cards of a sparse process overview by the largest factor that fits in the area,
/// at most 2.5 times their size (`compose.ts`, the process branch).
fn enlarge_sparse(input: &Input, sized: &mut [Sized], area: Region, gap: i64) {
    let total = sized.len();
    let columns = total.min(3);
    let rows = total.div_ceil(columns);
    let widest = sized.iter().map(|entry| entry.size[0]).max().unwrap_or(0);
    let tallest = sized.iter().map(|entry| entry.size[1]).max().unwrap_or(0);
    let columns_f = ratio(columns);
    let rows_f = ratio(rows);
    let across = coord(area.width - gap * (count(columns) - 1)) / (columns_f * coord(widest));
    let down = coord(area.height - gap * (count(rows) - 1)) / (rows_f * coord(tallest));
    let scale = 1.0_f64.max(2.5_f64.min(across).min(down));
    for entry in sized.iter_mut() {
        if !entry.size_given && !is_pinned(&input.items[entry.index]) {
            entry.size = [
                snap(coord(entry.size[0]) * scale),
                snap(coord(entry.size[1]) * scale),
            ];
        }
    }
}

/// A count as a float for a ratio of layout dimensions.
#[allow(clippy::cast_precision_loss)]
fn ratio(value: usize) -> f64 {
    value as f64
}

/// Applies the equal-size rule (`equalSize` in `compose.ts`): the largest width or height of the
/// group is shared, a faceplate keeps its proportions, and a fixed object keeps its size.
fn equalize(input: &Input, sized: &mut [Sized], equal: EqualSize) -> AgentResult<()> {
    let Some(first) = sized.first() else {
        return Ok(());
    };
    let target = sized.iter().fold(first.size, |largest, entry| {
        [largest[0].max(entry.size[0]), largest[1].max(entry.size[1])]
    });
    for entry in sized.iter_mut() {
        let item = &input.items[entry.index];
        let next = [
            if equal == EqualSize::Height {
                entry.size[0]
            } else {
                target[0]
            },
            if equal == EqualSize::Width {
                entry.size[1]
            } else {
                target[1]
            },
        ];
        if item.kind == PLACED_FACEPLATE && next[0] * entry.size[1] != next[1] * entry.size[0] {
            return Err(AgentError::validation(format!(
                "equal sizing would distort faceplate {}.",
                item.id
            )));
        }
        let fixed = is_pinned(item) || is_locked(item);
        if fixed && next != entry.size {
            return Err(AgentError::conflict(format!(
                "equal sizing would resize fixed object {}.",
                item.id
            )));
        }
        entry.size = next;
    }
    Ok(())
}

/// Places a group's items in its area and appends one `add` operation per item.
fn place_group(
    input: &Input,
    sized: &[Sized],
    role: CompositionRole,
    area: Region,
    layout: &LayoutConstraints,
    operations: &mut Vec<Value>,
) -> AgentResult<()> {
    let items: Vec<SizedItem> = sized
        .iter()
        .map(|entry| {
            let item = &input.items[entry.index];
            SizedItem {
                id: item.id.clone(),
                size: entry.size,
                rotation: rotation_of(item),
                pinned_at: item.pinned_at.clone(),
            }
        })
        .collect();
    let positions = place_region(&items, area, role, layout.gap, layout.flow, &[])?;
    for entry in sized {
        let item = &input.items[entry.index];
        let at = positions
            .get(&item.id)
            .copied()
            .ok_or_else(|| AgentError::validation(format!("{} has no position.", item.id)))?;
        operations.push(add_operation(item, at, entry.size));
    }
    Ok(())
}

/// The `add` operation of one item: its properties with its position, size, rotation and
/// bindings (`rotation` and `bind` default to 0 and none).
fn add_operation(item: &Item, at: [i64; 2], size: [i64; 2]) -> Value {
    let mut object = item.properties.clone();
    object.insert("at".to_owned(), json!(at));
    object.insert("size".to_owned(), json!(size));
    object.insert(
        "rotation".to_owned(),
        item.properties
            .get("rotation")
            .filter(|value| !value.is_null())
            .cloned()
            .unwrap_or_else(|| json!(0)),
    );
    object.insert(
        "bind".to_owned(),
        item.properties
            .get("bind")
            .filter(|value| !value.is_null())
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    json!({"op": "add", "object": Value::Object(object)})
}

/// The item's rotation for placement; a rotation that is not a whole number of degrees counts as
/// zero here and is refused when the object is read.
fn rotation_of(item: &Item) -> u16 {
    item.properties
        .get("rotation")
        .and_then(Value::as_u64)
        .and_then(|rotation| u16::try_from(rotation).ok())
        .unwrap_or(0)
}

/// A usable size: two whole numbers of at least 8 pixels.
fn usable_size(value: &Value) -> Option<[i64; 2]> {
    let pair = value.as_array()?;
    if pair.len() != 2 {
        return None;
    }
    let width = pair[0].as_i64().filter(|width| *width >= 8)?;
    let height = pair[1].as_i64().filter(|height| *height >= 8)?;
    Some([width, height])
}

/// Whether the item's `pinnedAt` is set to something other than null (the TypeScript's truthy
/// `pinnedAt` test).
fn is_pinned(item: &Item) -> bool {
    item.pinned_at
        .as_ref()
        .is_some_and(|value| !value.is_null())
}

/// Whether the item is locked (its `locked` property is `true`).
fn is_locked(item: &Item) -> bool {
    item.properties.get("locked").and_then(Value::as_bool) == Some(true)
}

#[cfg(test)]
#[path = "compose_tests.rs"]
mod tests;
