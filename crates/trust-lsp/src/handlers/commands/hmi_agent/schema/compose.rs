//! The composition inputs of `hmi_compose_page` (AT-04): one page or several, with their objects.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use super::super::error::{AgentError, AgentResult};
use super::super::patterns::PatternId;
use super::{
    check_grid_region, check_object, decode_nested, grid_step, page_file, text_length,
    unique_items, Bounded, ObjectContext, MAX_ASSUMPTIONS, MAX_BATCH, MAX_GAP, MAX_PAGES, MIN_SIZE,
};

/// A component's binding to a PLC path (`bind`, and `bind` operations).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)] // Validated by its types; the owning edit and compose modules read the same JSON.
pub(in super::super) struct BindingInput {
    /// The exact PLC path, as `hmi_get_context` returns it.
    path: String,
    /// The direction the object may use.
    direction: Direction,
    /// The unit to show.
    unit: Option<String>,
    /// The range of the value: two numbers.
    range: Option<[f64; 2]>,
}

/// The direction of a binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)] // Validated by its type; the owning modules read the same JSON.
pub(in super::super) enum Direction {
    /// The object only shows the value.
    Read,
    /// The object only writes the value.
    Write,
    /// The object shows and writes the value.
    ReadWrite,
}

/// The limits of a value object.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)] // Validated by its types; the owning modules read the same JSON.
pub(in super::super) struct LimitsInput {
    /// The low-low limit.
    #[serde(rename = "low-low")]
    low_low: Option<f64>,
    /// The low limit.
    low: Option<f64>,
    /// The high limit.
    high: Option<f64>,
    /// The high-high limit.
    #[serde(rename = "high-high")]
    high_high: Option<f64>,
}

/// The role that may operate an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // Validated by its type; the owning modules read the same JSON.
pub(in super::super) enum Role {
    /// An operator.
    Operator,
    /// An engineer.
    Engineer,
}

/// The action of a button or a value object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // Validated by its type; the owning modules read the same JSON.
pub(in super::super) enum ObjectAction {
    /// Sets the value.
    Set,
    /// Resets the value.
    Reset,
    /// Toggles the value.
    Toggle,
}

/// How a row of repeated objects is arranged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // Validated by its type; the owning modules read the same JSON.
pub(in super::super) enum Arrange {
    /// One row.
    Row,
    /// One column.
    Column,
    /// A grid.
    Grid,
}

/// The layout region a composed object goes in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // Validated by its type; the owning compose module reads the same JSON.
pub(in super::super) enum RegionRole {
    /// The operating process.
    Process,
    /// The measurements beside it.
    Measurements,
    /// The trends.
    Trends,
    /// The alarms.
    Alarms,
    /// The controls.
    Controls,
}

/// A flow direction, or the axis of a distribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // Validated by its type; the owning modules read the same JSON.
pub(in super::super) enum Axis {
    /// Left to right, or across.
    Horizontal,
    /// Top to bottom, or down.
    Vertical,
}

/// One component or object as a composition or an edit gives it (`compose` items, `add` objects,
/// `update` properties). Its keys are the schema's; a composition and an edit forbid the others.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[allow(dead_code)] // Validated by its keys and types; the owning modules read the same JSON.
pub(in super::super) struct ObjectInput {
    /// The object's ID.
    pub(super) id: Option<String>,
    /// The exact component type from `hmi_browse_library`.
    #[serde(rename = "type")]
    kind: Option<String>,
    /// The size in pixels, each side at least 8.
    pub(super) size: Option<[i64; 2]>,
    /// The rotation: 0, 90, 180 or 270 degrees.
    pub(super) rotation: Option<i64>,
    /// The bindings, by component property.
    bind: Option<BTreeMap<String, BindingInput>>,
    /// The position of an added object.
    pub(super) at: Option<[i64; 2]>,
    /// The position and size an object keeps.
    pub(super) pinned_at: Option<[i64; 2]>,
    /// The layout region of a composed object.
    region: Option<RegionRole>,
    /// The tag.
    tag: Option<String>,
    /// The label.
    label: Option<String>,
    /// The text.
    text: Option<String>,
    /// The unit.
    unit: Option<String>,
    /// The variant.
    variant: Option<String>,
    /// The confirmation text.
    confirm: Option<String>,
    /// The instrument's attachment.
    attach: Option<String>,
    /// The group.
    group: Option<String>,
    /// The project image.
    image: Option<String>,
    /// The faceplate.
    faceplate: Option<String>,
    /// The faceplate instance.
    instance: Option<String>,
    /// The minimum.
    min: Option<f64>,
    /// The maximum.
    max: Option<f64>,
    /// The span.
    span: Option<f64>,
    /// The gap.
    gap: Option<f64>,
    /// The pulse length.
    pulse: Option<f64>,
    /// The timeout.
    timeout: Option<f64>,
    /// The decimal places.
    pub(super) decimals: Option<u32>,
    /// The limits.
    limits: Option<LimitsInput>,
    /// The role that may operate it.
    role: Option<Role>,
    /// The action it performs.
    action: Option<ObjectAction>,
    /// The expected value.
    expect: Option<Scalar>,
    /// The options.
    options: Option<Vec<String>>,
    /// Whether it is locked.
    locked: Option<bool>,
    /// How its repeats are arranged.
    arrange: Option<Arrange>,
    /// The click targets, by kind.
    click: Option<BTreeMap<String, String>>,
}

/// A value a property takes.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
#[allow(dead_code)] // Validated by its type; the owning modules read the same JSON.
pub(in super::super) enum Scalar {
    /// A flag.
    Flag(bool),
    /// A number.
    Number(f64),
    /// A text.
    Text(String),
}

/// A connection between two ports (`connections`, `connect` and `disconnect`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)] // Validated by its type; the owning modules read the same JSON.
pub(in super::super) struct Connection {
    /// The port the pipe starts at.
    from: String,
    /// The port the pipe ends at.
    to: String,
}

/// A grid region of the design, on the 8-pixel grid (`layout.regions`, `arrange`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)] // Validated by its fields; the owning modules read the same JSON.
pub(in super::super) struct GridRegion {
    /// The left edge.
    pub(super) x: i64,
    /// The top edge.
    pub(super) y: i64,
    /// The width.
    pub(super) width: i64,
    /// The height.
    pub(super) height: i64,
}

/// The named regions of a layout.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)] // Validated by its fields; the owning compose module reads the same JSON.
pub(in super::super) struct RegionsInput {
    /// The process region.
    process: Option<GridRegion>,
    /// The measurements region.
    measurements: Option<GridRegion>,
    /// The trends region.
    trends: Option<GridRegion>,
    /// The alarms region.
    alarms: Option<GridRegion>,
    /// The controls region.
    controls: Option<GridRegion>,
}

/// A composition's layout constraints (`layout`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[allow(dead_code)] // Validated by its fields; the owning compose module reads the same JSON.
pub(in super::super) struct LayoutInput {
    /// The regions by role.
    regions: Option<RegionsInput>,
    /// The gap between regions.
    gap: Option<i64>,
    /// The fill direction.
    flow: Option<Axis>,
    /// The inner padding of a region.
    padding: Option<i64>,
    /// Which size equal objects share.
    equal_size: Option<EqualSize>,
}

/// The axis a set of objects shares a size on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // Validated by its type; the owning modules read the same JSON.
pub(in super::super) enum EqualSize {
    /// The same width.
    Width,
    /// The same height.
    Height,
    /// The same width and height.
    Both,
}

/// The properties a page edit may change (`page` operation).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[allow(dead_code)] // Validated by its fields; the owning edit module reads the same JSON.
pub(in super::super) struct PageProperties {
    /// The title, from 1 to 1000 characters.
    pub(super) title: Option<String>,
    /// The position in the navigation.
    pub(super) order: Option<u32>,
    /// The icon.
    pub(super) icon: Option<String>,
    /// The navigation group.
    pub(super) group: Option<String>,
    /// Whether the navigation hides the page.
    pub(super) hidden: Option<bool>,
    /// Whether the page is still marked incomplete.
    pub(super) incomplete: Option<bool>,
}

/// One composition request: the source of a new or existing page, with its key (`hmi_compose_page`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[allow(dead_code)] // The raw pieces are checked by `check_page_fields`; the port reads the same JSON.
pub(in super::super) struct PageComposition {
    /// The destination page, `hmi/pages/<name>.toml`.
    file: String,
    /// The creation key.
    pub(in super::super) key: String,
    /// The page title.
    title: Option<String>,
    /// The composition pattern.
    pattern: Option<String>,
    /// The components, checked by `ObjectInput`.
    items: Option<Vec<Value>>,
    /// The connections, checked by `Connection`.
    connections: Option<Vec<Value>>,
    /// The existing page to copy into the draft.
    source_file: Option<String>,
    /// The layout constraints, checked by `LayoutInput`.
    layout: Option<Value>,
    /// The operator task.
    operator_task: Option<String>,
    /// The rationale.
    rationale: Option<String>,
    /// The assumptions for the customer to review.
    assumptions: Option<Vec<String>>,
}

/// One destination of a multi-page composition (`pages`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[allow(dead_code)] // The raw pieces are checked by `check_page_fields`; the port reads the same JSON.
pub(in super::super) struct PageDestination {
    /// The destination page.
    file: String,
    /// The page title.
    title: Option<String>,
    /// The composition pattern.
    pattern: Option<String>,
    /// The components.
    items: Option<Vec<Value>>,
    /// The connections.
    connections: Option<Vec<Value>>,
    /// The existing page to copy.
    source_file: Option<String>,
    /// The layout constraints.
    layout: Option<Value>,
}

/// A multi-page composition (`pages`, at most 32 unique destinations).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[allow(dead_code)] // The pages are checked by `check_page_fields`; the port reads the same JSON.
pub(in super::super) struct PagesComposition {
    /// The creation key.
    pub(in super::super) key: String,
    /// The destinations.
    pages: Vec<PageDestination>,
    /// The operator task of every page.
    operator_task: Option<String>,
    /// The rationale.
    rationale: Option<String>,
    /// The assumptions.
    assumptions: Option<Vec<String>>,
}

/// The input of `hmi_compose_page` (AT-04): one page, or several with `pages`.
#[derive(Debug)]
pub(in super::super) enum ComposeInput {
    /// One destination page.
    Page(PageComposition),
    /// Several destination pages prepared together.
    Pages(PagesComposition),
}

impl<'de> Deserialize<'de> for ComposeInput {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let multi = value
            .as_object()
            .is_some_and(|object| object.contains_key("pages"));
        if multi {
            PagesComposition::deserialize(value)
                .map(Self::Pages)
                .map_err(serde::de::Error::custom)
        } else {
            PageComposition::deserialize(value)
                .map(Self::Page)
                .map_err(serde::de::Error::custom)
        }
    }
}

impl Bounded for ComposeInput {
    fn bounds(&self) -> AgentResult<()> {
        match self {
            Self::Page(page) => {
                page_file("input.file", &page.file)?;
                text_length("input.key", &page.key, 1, 200)?;
                check_page_fields(
                    "input",
                    page.source_file.as_deref(),
                    PageFields {
                        title: page.title.is_some(),
                        pattern: page.pattern.as_deref(),
                        items: page.items.as_deref(),
                        layout: page.layout.as_ref(),
                        connections: page.connections.as_deref(),
                    },
                )?;
                if let Some(source) = &page.source_file {
                    page_file("input.sourceFile", source)?;
                }
                composition_notes(
                    page.operator_task.as_deref(),
                    page.rationale.as_deref(),
                    page.assumptions.as_deref(),
                )
            }
            Self::Pages(pages) => {
                text_length("input.key", &pages.key, 1, 200)?;
                if pages.pages.is_empty() || pages.pages.len() > MAX_PAGES {
                    return Err(AgentError::limit(format!(
                        "input.pages must name 1 to {MAX_PAGES} destinations."
                    )));
                }
                let files: Vec<String> = pages.pages.iter().map(|page| page.file.clone()).collect();
                unique_items("input.pages.file", &files)?;
                for (index, page) in pages.pages.iter().enumerate() {
                    let field = format!("input.pages[{index}]");
                    page_file(&format!("{field}.file"), &page.file)?;
                    check_page_fields(
                        &field,
                        page.source_file.as_deref(),
                        PageFields {
                            title: page.title.is_some(),
                            pattern: page.pattern.as_deref(),
                            items: page.items.as_deref(),
                            layout: page.layout.as_ref(),
                            connections: page.connections.as_deref(),
                        },
                    )?;
                    if let Some(source) = &page.source_file {
                        page_file(&format!("{field}.sourceFile"), source)?;
                    }
                }
                composition_notes(
                    pages.operator_task.as_deref(),
                    pages.rationale.as_deref(),
                    pages.assumptions.as_deref(),
                )
            }
        }
    }
}

/// The page fields a composition checks together.
struct PageFields<'a> {
    /// Whether a title is given.
    title: bool,
    /// The pattern name.
    pattern: Option<&'a str>,
    /// The raw components.
    items: Option<&'a [Value]>,
    /// The raw layout.
    layout: Option<&'a Value>,
    /// The raw connections.
    connections: Option<&'a [Value]>,
}

/// The shape rules of one composition: a copy of a source page takes no items, pattern or layout; a
/// new composition takes a title, a pattern and items (`oneOf` of the schema).
fn check_page_fields(field: &str, source: Option<&str>, fields: PageFields<'_>) -> AgentResult<()> {
    if source.is_some() {
        if fields.items.is_some() || fields.pattern.is_some() || fields.layout.is_some() {
            return Err(AgentError::invalid(format!(
                "{field}: choose an existing source page or a new composition, not both."
            )));
        }
    } else {
        if !fields.title {
            return Err(AgentError::invalid(format!(
                "{field}.title is required for a new composition."
            )));
        }
        let Some(pattern) = fields.pattern else {
            return Err(AgentError::invalid(format!(
                "{field}.pattern is required for a new composition."
            )));
        };
        if PatternId::parse(pattern).is_none() {
            return Err(AgentError::invalid(format!(
                "{field}.pattern must be process-overview, equipment-detail, multi-unit, investigation or settings."
            )));
        }
        if fields.items.is_none() {
            return Err(AgentError::invalid(format!(
                "{field}.items is required for a new composition."
            )));
        }
    }
    if let Some(items) = fields.items {
        if items.len() > MAX_BATCH {
            return Err(AgentError::limit(format!(
                "{field}.items may hold at most {MAX_BATCH} objects."
            )));
        }
        for (index, item) in items.iter().enumerate() {
            let item_field = format!("{field}.items[{index}]");
            let object: ObjectInput = decode_nested(&item_field, item)?;
            check_object(&item_field, item, &object, ObjectContext::Composition)?;
        }
    }
    if let Some(connections) = fields.connections {
        for (index, connection) in connections.iter().enumerate() {
            let _: Connection =
                decode_nested(&format!("{field}.connections[{index}]"), connection)?;
        }
    }
    if let Some(layout) = fields.layout {
        check_layout(&format!("{field}.layout"), layout)?;
    }
    Ok(())
}

/// The notes a composition records (`operatorTask`, `rationale`, `assumptions`).
fn composition_notes(
    operator_task: Option<&str>,
    rationale: Option<&str>,
    assumptions: Option<&[String]>,
) -> AgentResult<()> {
    if let Some(task) = operator_task {
        text_length("input.operatorTask", task, 1, 1000)?;
    }
    if let Some(text) = rationale {
        text_length("input.rationale", text, 1, 2000)?;
    }
    if let Some(list) = assumptions {
        assumption_list("input", list)?;
    }
    Ok(())
}

/// Checks a layout's regions, gap and padding (`layout`, `arrange`).
fn check_layout(field: &str, raw: &Value) -> AgentResult<()> {
    let layout: LayoutInput = decode_nested(field, raw)?;
    if let Some(gap) = layout.gap {
        grid_step(&format!("{field}.gap"), gap, MIN_SIZE, MAX_GAP)?;
    }
    if let Some(padding) = layout.padding {
        grid_step(&format!("{field}.padding"), padding, 0, MAX_GAP)?;
    }
    let Some(regions) = raw.get("regions").and_then(Value::as_object) else {
        return Ok(());
    };
    for (role, region) in regions {
        let region_field = format!("{field}.regions.{role}");
        let decoded: GridRegion = decode_nested(&region_field, region)?;
        check_grid_region(&region_field, &decoded)?;
    }
    Ok(())
}

/// The assumptions a composition records: at most 12 unique texts of 1 to 300 characters.
fn assumption_list(field: &str, list: &[String]) -> AgentResult<()> {
    if list.len() > MAX_ASSUMPTIONS {
        return Err(AgentError::limit(format!(
            "{field}.assumptions may hold at most {MAX_ASSUMPTIONS} entries."
        )));
    }
    unique_items(&format!("{field}.assumptions"), list)?;
    for text in list {
        text_length(&format!("{field}.assumptions"), text, 1, 300)?;
    }
    Ok(())
}
