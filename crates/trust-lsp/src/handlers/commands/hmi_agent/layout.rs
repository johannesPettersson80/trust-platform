//! Region layout of a composition (specification 43, §6): the explicit regions, the default
//! regions of each pattern, pinned items and auto placement on the 8-pixel grid.
//!
//! Port of `agent/layout.ts`. The constraints are read from the request's JSON so that each
//! refusal names the field that failed, in the order the TypeScript checked them, with its text.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value};

use super::error::{AgentError, AgentResult};
use super::geometry::{box_of, contains, coord, overlaps, Rect};

/// A region role of a composition (`CompositionRole` in `layout.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum CompositionRole {
    /// The dominant operating area.
    Process,
    /// Supporting readings.
    Measurements,
    /// Trend area.
    Trends,
    /// Alarm area.
    Alarms,
    /// Controls and settings.
    Controls,
}

impl CompositionRole {
    /// Every role, in the order the TypeScript lists them when it validates a name.
    pub(super) const ALL: [Self; 5] = [
        Self::Process,
        Self::Measurements,
        Self::Trends,
        Self::Alarms,
        Self::Controls,
    ];

    /// The role named by `text`, if it is one of the five.
    #[must_use]
    pub(super) fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.as_str() == text)
    }

    /// The role's name as the tool contract writes it.
    #[must_use]
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Process => "process",
            Self::Measurements => "measurements",
            Self::Trends => "trends",
            Self::Alarms => "alarms",
            Self::Controls => "controls",
        }
    }
}

/// A region in design pixels: its top-left corner and its size, on the 8-pixel grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Region {
    /// Left edge.
    pub(super) x: i64,
    /// Top edge.
    pub(super) y: i64,
    /// Width.
    pub(super) width: i64,
    /// Height.
    pub(super) height: i64,
}

impl Region {
    /// The region as a box for the containment and overlap tests.
    #[must_use]
    pub(super) fn rect(self) -> Rect {
        Rect {
            x: coord(self.x),
            y: coord(self.y),
            width: coord(self.width),
            height: coord(self.height),
        }
    }
}

/// The direction in which auto placement fills cells (`flow`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Flow {
    /// Left to right, then down.
    Horizontal,
    /// Top to bottom, then right.
    Vertical,
}

/// Which dimension of the cards is made equal (`equalSize`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EqualSize {
    /// Equal widths.
    Width,
    /// Equal heights.
    Height,
    /// Equal widths and heights.
    Both,
}

/// The layout constraints of a composition request, checked as `layoutOptions` checks them.
/// The explicit regions stay raw until `constrained_regions` checks them, which happens later in
/// the composition, as in the TypeScript.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LayoutConstraints {
    /// The `regions` value exactly as the request wrote it, when it wrote one.
    pub(super) regions: Option<Value>,
    /// The gap between regions and cards, 8 to 128 on the grid (default 24).
    pub(super) gap: i64,
    /// The direction of auto placement (default horizontal).
    pub(super) flow: Flow,
    /// The inner margin of every region, when the request sets one.
    pub(super) padding: Option<i64>,
    /// The equal-size rule, when the request sets one.
    pub(super) equal_size: Option<EqualSize>,
}

/// The keys a composition's `layout` may carry.
const LAYOUT_KEYS: [&str; 5] = ["regions", "gap", "flow", "padding", "equalSize"];

/// Whether a value is on the 8-pixel grid.
fn on_grid(value: i64) -> bool {
    value % 8 == 0
}

/// The gap and flow of a layout, as `layoutOptions` reads them: a missing or null value takes its
/// default (24 pixels, horizontal), a present one must be valid.
///
/// # Errors
///
/// Returns `INVALID_ARGUMENT` when the gap is not a whole number of grid units from 8 to 128
/// pixels, or the flow is neither `horizontal` nor `vertical`.
pub(super) fn gap_and_flow(gap: Option<&Value>, flow: Option<&Value>) -> AgentResult<(i64, Flow)> {
    let gap = match gap.filter(|value| !value.is_null()) {
        None => 24,
        Some(value) => value
            .as_i64()
            .filter(|gap| on_grid(*gap) && (8..=128).contains(gap))
            .ok_or_else(|| {
                AgentError::invalid("layout gap must be 8–128 pixels on the 8-pixel grid.")
            })?,
    };
    let flow = match flow.filter(|value| !value.is_null()) {
        None => Flow::Horizontal,
        Some(value) => match value.as_str() {
            Some("horizontal") => Flow::Horizontal,
            Some("vertical") => Flow::Vertical,
            _ => {
                return Err(AgentError::invalid(
                    "layout flow must be horizontal or vertical.",
                ))
            }
        },
    };
    Ok((gap, flow))
}

/// Reads a composition's `layout` as `layoutOptions` does: the keys must be known, the gap,
/// flow, padding and equal-size rules must hold when present. An absent layout is the default.
///
/// # Errors
///
/// Returns `INVALID_ARGUMENT` with the TypeScript's text for the first field that fails.
pub(super) fn parse_layout(value: Option<&Value>) -> AgentResult<LayoutConstraints> {
    let unsupported = || AgentError::invalid("unsupported layout constraints.");
    let empty = Map::new();
    let object = match value {
        None => &empty,
        Some(value) => value.as_object().ok_or_else(unsupported)?,
    };
    if object
        .keys()
        .any(|key| !LAYOUT_KEYS.contains(&key.as_str()))
    {
        return Err(unsupported());
    }
    let (gap, flow) = gap_and_flow(object.get("gap"), object.get("flow"))?;
    let padding = match object.get("padding") {
        None => None,
        Some(value) => Some(
            value
                .as_i64()
                .filter(|padding| on_grid(*padding) && (0..=128).contains(padding))
                .ok_or_else(|| {
                    AgentError::invalid("padding must use the 8-pixel grid between 0 and 128.")
                })?,
        ),
    };
    let equal_size = match object.get("equalSize") {
        None => None,
        Some(value) => Some(match value.as_str() {
            Some("width") => EqualSize::Width,
            Some("height") => EqualSize::Height,
            Some("both") => EqualSize::Both,
            _ => {
                return Err(AgentError::invalid(
                    "equalSize must be width, height or both.",
                ))
            }
        }),
    };
    Ok(LayoutConstraints {
        regions: object.get("regions").cloned(),
        gap,
        flow,
        padding,
        equal_size,
    })
}

/// Resolves explicit regions over the default regions, as `constrainedRegions` does. Explicit
/// regions must have exactly `x`, `y`, `width` and `height` on the grid with positive size, lie
/// inside `bounds`, and may not overlap one another. A region that replaces a default keeps the
/// default's place in the order.
///
/// # Errors
///
/// Returns `INVALID_ARGUMENT` for a malformed region and `VALIDATION_FAILED` for a region outside
/// the bounds or overlapping another.
pub(super) fn constrained_regions(
    defaults: Vec<(CompositionRole, Region)>,
    bounds: Region,
    regions: Option<&Value>,
) -> AgentResult<Vec<(CompositionRole, Region)>> {
    let mut areas = defaults;
    if let Some(regions) = regions {
        let entries = regions
            .as_object()
            .ok_or_else(|| AgentError::invalid("layout regions must be an object."))?;
        for (name, value) in entries {
            let role = CompositionRole::parse(name)
                .ok_or_else(|| AgentError::invalid(format!("unknown layout region {name}.")))?;
            let region = parse_region(role, value)?;
            if !contains(bounds.rect(), region.rect()) {
                return Err(AgentError::validation(format!(
                    "{} region does not fit below the heading and inside page margins.",
                    role.as_str()
                )));
            }
            set_region(&mut areas, role, region);
        }
    }
    for (index, (first, first_area)) in areas.iter().enumerate() {
        for (second, second_area) in &areas[index + 1..] {
            if overlaps(first_area.rect(), second_area.rect(), 0.0) {
                return Err(AgentError::validation(format!(
                    "{} and {} regions overlap; move or resize the regions.",
                    first.as_str(),
                    second.as_str()
                )));
            }
        }
    }
    Ok(areas)
}

/// Sets a role's region, keeping its place in the order if it already has one.
fn set_region(areas: &mut Vec<(CompositionRole, Region)>, role: CompositionRole, region: Region) {
    match areas.iter_mut().find(|(existing, _)| *existing == role) {
        Some(entry) => entry.1 = region,
        None => areas.push((role, region)),
    }
}

/// One explicit region's shape: exactly four keys, whole grid values, positive size.
fn parse_region(role: CompositionRole, value: &Value) -> AgentResult<Region> {
    let shape = || {
        AgentError::invalid(format!(
            "{} region must use positive dimensions on the 8-pixel grid.",
            role.as_str()
        ))
    };
    let object = value.as_object().ok_or_else(shape)?;
    if object.len() != 4 {
        return Err(shape());
    }
    let field = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_i64)
            .filter(|value| on_grid(*value))
            .ok_or_else(shape)
    };
    let region = Region {
        x: field("x")?,
        y: field("y")?,
        width: field("width")?,
        height: field("height")?,
    };
    if region.width <= 0 || region.height <= 0 {
        return Err(shape());
    }
    Ok(region)
}

/// Rounds down to the 8-pixel grid (`snap` in `compose.ts`: `Math.floor(value / 8) * 8`).
#[must_use]
pub(super) fn snap(value: f64) -> i64 {
    count((value / 8.0).floor() * 8.0)
}

/// An item to place: its size, rotation and optional pinned position. The pinned value is kept
/// as the request wrote it, so a malformed pinned position is refused where `placeRegion` checks.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SizedItem {
    /// The object's ID.
    pub(super) id: String,
    /// Width and height.
    pub(super) size: [i64; 2],
    /// Rotation in degrees.
    pub(super) rotation: u16,
    /// The `pinnedAt` value, when the item has one.
    pub(super) pinned_at: Option<Value>,
}

/// A box that auto placement must avoid: an object already on the page.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Obstacle {
    /// The object's ID.
    pub(super) id: String,
    /// The box the object is drawn in.
    pub(super) boxed: Rect,
}

/// The placement a region's items receive, by item ID.
pub(super) type Placement = BTreeMap<String, [i64; 2]>;

/// Places items in a region (`placeRegion`). Pinned items keep their position, which must be on
/// the grid, inside the region, and clear of obstacles and of one another. Movable items fill
/// the free cells in the flow direction, cells sized to the largest item.
///
/// # Errors
///
/// Returns `INVALID_ARGUMENT` for a malformed pinned position and `VALIDATION_FAILED` when an item
/// does not fit at a readable size.
pub(super) fn place_region(
    items: &[SizedItem],
    area: Region,
    role: CompositionRole,
    gap: i64,
    flow: Flow,
    obstacles: &[Obstacle],
) -> AgentResult<Placement> {
    let mut occupied: Vec<(String, Rect)> = obstacles
        .iter()
        .map(|obstacle| (obstacle.id.clone(), obstacle.boxed))
        .collect();
    let mut placed = place_pinned(items, area, role, &mut occupied)?;
    let movable: Vec<&SizedItem> = items
        .iter()
        .filter(|entry| entry.pinned_at.is_none())
        .collect();
    if !movable.is_empty() {
        place_movable(&movable, area, role, gap, flow, &mut occupied, &mut placed)?;
    }
    Ok(placed)
}

/// Keeps every pinned item where it is, after checking its position, its fit and its clearance.
fn place_pinned(
    items: &[SizedItem],
    area: Region,
    role: CompositionRole,
    occupied: &mut Vec<(String, Rect)>,
) -> AgentResult<Placement> {
    let area_box = area.rect();
    let mut placed = Placement::new();
    for entry in items {
        let Some(pinned) = &entry.pinned_at else {
            continue;
        };
        let at = pinned_position(pinned).ok_or_else(|| {
            AgentError::invalid(format!(
                "{} pinned position must use the 8-pixel grid.",
                entry.id
            ))
        })?;
        let boxed = box_of(to_f(at), size_f(entry.size), entry.rotation);
        if !contains(area_box, boxed) {
            return Err(AgentError::validation(format!(
                "{} does not fit in the {} region.",
                entry.id,
                role.as_str()
            )));
        }
        if let Some((other, _)) = occupied
            .iter()
            .find(|(_, other)| overlaps(boxed, *other, 0.0))
        {
            return Err(AgentError::validation(format!(
                "pinned objects {} and {other} overlap.",
                entry.id
            )));
        }
        placed.insert(entry.id.clone(), at);
        occupied.push((entry.id.clone(), boxed));
    }
    Ok(placed)
}

/// Fills the free cells of the region with the movable items, in the flow direction.
fn place_movable(
    movable: &[&SizedItem],
    area: Region,
    role: CompositionRole,
    gap: i64,
    flow: Flow,
    occupied: &mut Vec<(String, Rect)>,
    placed: &mut Placement,
) -> AgentResult<()> {
    let area_box = area.rect();
    let gap_f = coord(gap);
    let boxes: Vec<Rect> = movable
        .iter()
        .map(|entry| box_of([0.0, 0.0], size_f(entry.size), entry.rotation))
        .collect();
    let cell_width = boxes
        .iter()
        .map(|boxed| boxed.width)
        .fold(f64::MIN, f64::max);
    let cell_height = boxes
        .iter()
        .map(|boxed| boxed.height)
        .fold(f64::MIN, f64::max);
    let single_column = matches!(
        role,
        CompositionRole::Measurements | CompositionRole::Controls
    );
    let columns = if single_column {
        1
    } else {
        count(((area_box.width + gap_f) / (cell_width + gap_f)).floor())
    };
    let rows = count(((area_box.height + gap_f) / (cell_height + gap_f)).floor());
    for entry in movable {
        let origin = box_of([0.0, 0.0], size_f(entry.size), entry.rotation);
        let shift_y = if role == CompositionRole::Process {
            (cell_height - origin.height) / 2.0
        } else {
            0.0
        };
        let mut found = false;
        for index in 0..rows * columns {
            let (column, row) = match flow {
                Flow::Horizontal => (index % columns, index / columns),
                Flow::Vertical => (index / rows, index % rows),
            };
            let at = [
                snap(area_box.x + coord(column) * (cell_width + gap_f) - origin.x),
                snap(area_box.y + coord(row) * (cell_height + gap_f) + shift_y - origin.y),
            ];
            let boxed = box_of(to_f(at), size_f(entry.size), entry.rotation);
            if !contains(area_box, boxed)
                || occupied
                    .iter()
                    .any(|(_, other)| overlaps(boxed, *other, gap_f))
            {
                continue;
            }
            placed.insert(entry.id.clone(), at);
            occupied.push((entry.id.clone(), boxed));
            found = true;
            break;
        }
        if !found {
            return Err(AgentError::validation(format!(
                "{} does not fit in {} at readable sizes; enlarge its region, reduce items or split the page.",
                entry.id,
                role.as_str()
            )));
        }
    }
    Ok(())
}

/// A pinned position: a pair of whole grid values.
fn pinned_position(value: &Value) -> Option<[i64; 2]> {
    let pair = value.as_array()?;
    if pair.len() != 2 {
        return None;
    }
    let x = pair[0].as_i64().filter(|x| on_grid(*x))?;
    let y = pair[1].as_i64().filter(|y| on_grid(*y))?;
    Some([x, y])
}

/// The size of an item as floats.
fn size_f(size: [i64; 2]) -> [f64; 2] {
    [coord(size[0]), coord(size[1])]
}

/// A position as floats.
fn to_f(at: [i64; 2]) -> [f64; 2] {
    [coord(at[0]), coord(at[1])]
}

/// A whole count or grid position from a float that is already integral.
#[allow(clippy::cast_possible_truncation)]
fn count(value: f64) -> i64 {
    value as i64
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
