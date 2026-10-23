//! The typed inputs of the nine authoring tools (specification 43, §5, AT-01 to AT-09), decoded with
//! `deny_unknown_fields`, with the bounds the published `package.json` schemas and `schema.ts` state.
//!
//! Port of `agent/schema.ts` (`validateToolInput`). A malformed input is the tool contract's typed
//! refusal, `INVALID_ARGUMENT`, and its message names the tool and the invalid field (specification 43,
//! §4), not every alternative of a union. The operation payloads (`compose` items, layout, edit
//! operations) are checked here by their types and bounds, and the owning modules then read the same
//! JSON, so no rule is stated twice.

use serde::Deserialize;
use serde_json::Value;

use super::error::{AgentError, AgentResult};

mod apply;
mod compose;
mod edit;
mod render;

pub(super) use self::apply::{ApplyAction, ApplyInput};
pub(super) use self::compose::{ComposeInput, GridRegion, ObjectInput};
pub(super) use self::edit::EditInput;
pub(super) use self::render::{PreviewInput, RenderInput, ValidateInput};

/// The most viewports a validation names (AT-06).
const MAX_VIEWPORTS: usize = 3;
/// The most pages one composition creates (specification 43, §4).
const MAX_PAGES: usize = 32;
/// The most objects, operations and states one request names (specification 43, §4 and §5).
const MAX_BATCH: usize = 500;
/// The most states one preview names, and the most steps of one simulator journey.
const MAX_PREVIEW: usize = 12;
/// The most waypoints one reroute supplies (AT-05).
const MAX_WAYPOINTS: usize = 64;
/// The most assumptions one composition records (AT-04).
const MAX_ASSUMPTIONS: usize = 12;
/// The most issues one visual review names.
const MAX_ISSUES: usize = 100;
/// The most object IDs one visual review issue names.
const MAX_ISSUE_OBJECTS: usize = 500;
/// The smallest size of an object or region in design pixels.
const MIN_SIZE: i64 = 8;
/// The largest coordinate, size or padding the schemas accept, in design pixels.
const MAX_DESIGN: i64 = 4096;
/// The largest gap or padding of a layout, in design pixels.
const MAX_GAP: i64 = 128;

/// A decoded tool input whose bounds go beyond its types. `read` runs the check after decoding.
pub(super) trait Bounded {
    /// Checks the bounds the schema states, naming the field that breaks one.
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` for a field outside its bounds; `LIMIT_EXCEEDED` for a batch over its limit.
    fn bounds(&self) -> AgentResult<()>;
}

/// Decodes the input of `tool` and checks its bounds (`validateToolInput`).
///
/// # Errors
///
/// `INVALID_ARGUMENT` for an unknown field, a wrong type or a field outside its bounds, named with the
/// tool. `LIMIT_EXCEEDED` for a batch over its limit.
pub(super) fn read<T>(tool: &str, input: Value) -> AgentResult<T>
where
    T: for<'de> Deserialize<'de> + Bounded,
{
    let value: T = serde_json::from_value(input)
        .map_err(|error| AgentError::invalid(format!("{tool}: {error}.")))?;
    value.bounds().map_err(|error| AgentError {
        code: error.code,
        message: format!("{tool}: {}", error.message),
    })?;
    Ok(value)
}

/// Checks the input of any of the nine tools without running it (`validateToolInput`). The rendering
/// and preview tools are checked here although their rendering stays in the extension.
///
/// # Errors
///
/// The refusals of [`read`]; `INVALID_ARGUMENT` for a tool name this service does not serve.
pub(super) fn check_tool_input(tool: &str, input: Value) -> AgentResult<()> {
    match tool {
        "hmi_get_context" => read::<ContextInput>(tool, input).map(drop),
        "hmi_browse_library" => read::<LibraryInput>(tool, input).map(drop),
        "hmi_inspect_page" => read::<InspectInput>(tool, input).map(drop),
        "hmi_compose_page" => read::<ComposeInput>(tool, input).map(drop),
        "hmi_edit_draft" => read::<EditInput>(tool, input).map(drop),
        "hmi_validate_draft" => read::<ValidateInput>(tool, input).map(drop),
        "hmi_render_draft" => read::<RenderInput>(tool, input).map(drop),
        "hmi_preview_states" => read::<PreviewInput>(tool, input).map(drop),
        "hmi_apply_draft" => read::<ApplyInput>(tool, input).map(drop),
        _ => Err(AgentError::invalid("unknown authoring tool.")),
    }
}

/// A theme name (`theme`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Theme {
    /// The dark theme, the default.
    Dark,
    /// The light theme.
    Light,
}

/// The sections a context request may name (AT-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum ContextSection {
    /// The screen design size, theme and settings.
    Screen,
    /// The project texts and languages.
    Texts,
    /// The languages alone.
    Languages,
    /// The compiled program's variables.
    Variables,
    /// The alarm declarations.
    Alarms,
    /// The component catalogue.
    Catalogue,
    /// The project's own symbols.
    Symbols,
    /// The display formats.
    Formats,
    /// The faceplates.
    Faceplates,
    /// The page navigation.
    Pages,
    /// The connections the project declares.
    Topology,
    /// The remote runtime's variables.
    RemoteCatalogue,
    /// The open simulator sessions.
    SimulatorSessions,
    /// The text and unsaved state of each page and popup: the snapshot of a review with `explore: true`. Only a request that
    /// names it gets it.
    Sources,
}

/// The input of `hmi_get_context` (AT-01).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ContextInput {
    /// The sections to return; each once.
    pub(super) sections: Option<Vec<ContextSection>>,
    /// A page to select in the answer's `selection`.
    pub(super) file: Option<String>,
    /// The first page of the page list.
    pub(super) cursor: Option<u64>,
    /// The number of pages in one answer, from 1 to 100.
    pub(super) limit: Option<u64>,
}

impl Bounded for ContextInput {
    fn bounds(&self) -> AgentResult<()> {
        if let Some(sections) = &self.sections {
            unique_items("input.sections", sections)?;
        }
        if let Some(file) = &self.file {
            page_file("input.file", file)?;
        }
        if let Some(limit) = self.limit {
            count_between("input.limit", limit, 1, 100)?;
        }
        Ok(())
    }
}

/// The input of `hmi_browse_library` (AT-02).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct LibraryInput {
    /// Text the component type, category or group must contain.
    pub(super) query: Option<String>,
    /// The first component of the page.
    pub(super) cursor: Option<u64>,
    /// The number of components in one page, from 1 to 100 (12 by default).
    pub(super) limit: Option<u64>,
    /// The theme of the reference images, drawn by the extension's renderer.
    #[allow(dead_code)] // Read by the renderer; the library answer does not depend on it.
    pub(super) theme: Option<Theme>,
    /// Only components of this category.
    pub(super) category: Option<String>,
    /// The synthetic state to show; the extension renders it.
    pub(super) state: Option<String>,
}

impl Bounded for LibraryInput {
    fn bounds(&self) -> AgentResult<()> {
        if let Some(limit) = self.limit {
            count_between("input.limit", limit, 1, 100)?;
        }
        if let Some(category) = &self.category {
            text_length("input.category", category, 0, 100)?;
        }
        if let Some(state) = &self.state {
            text_length("input.state", state, 1, 64)?;
        }
        Ok(())
    }
}

/// A rectangle of the design, in pixels: `x` and `y` from 0, `width` and `height` above 0 (AT-03).
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ViewRegion {
    /// The left edge.
    pub(super) x: f64,
    /// The top edge.
    pub(super) y: f64,
    /// The width.
    pub(super) width: f64,
    /// The height.
    pub(super) height: f64,
}

/// The input of `hmi_inspect_page` (AT-03). `draftId` and `revision` go together.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct InspectInput {
    /// The page, popup or faceplate, project-relative.
    pub(super) file: String,
    /// The object IDs to inspect. Use `ids` or `region`, never both.
    pub(super) ids: Option<Vec<String>>,
    /// The rectangle whose overlapping objects are inspected.
    pub(super) region: Option<ViewRegion>,
    /// The draft to inspect instead of the source page.
    pub(super) draft_id: Option<String>,
    /// The draft revision that names the draft.
    pub(super) revision: Option<String>,
}

impl Bounded for InspectInput {
    fn bounds(&self) -> AgentResult<()> {
        page_or_popup_or_panel("input.file", &self.file)?;
        if let Some(ids) = &self.ids {
            text_items("input.ids", ids)?;
        }
        if let Some(region) = &self.region {
            let finite = [region.x, region.y, region.width, region.height]
                .iter()
                .all(|value| value.is_finite());
            if !finite
                || region.x < 0.0
                || region.y < 0.0
                || region.width <= 0.0
                || region.height <= 0.0
            {
                return Err(AgentError::invalid(
                    "input.region needs finite x and y from 0 and a positive width and height.",
                ));
            }
        }
        match (&self.draft_id, &self.revision) {
            (Some(draft), Some(revision)) => {
                text_length("input.draftId", draft, 1, usize::MAX)?;
                text_length("input.revision", revision, 1, usize::MAX)?;
            }
            (None, None) => {}
            _ => {
                return Err(AgentError::invalid(
                    "input.draftId and input.revision must be given together.",
                ));
            }
        }
        Ok(())
    }
}

/// Where a component's object is used; each use forbids and requires other keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectContext {
    /// A new component of a composition: it takes a type, and a layout region, not an `at`.
    Composition,
    /// An object added by an edit: it takes a type, `at` and `size`.
    Add,
    /// The properties an edit updates: no identity, geometry or bindings.
    Update,
}

/// Checks the required and forbidden keys of one object, by its use.
fn check_object(
    field: &str,
    raw: &Value,
    object: &ObjectInput,
    context: ObjectContext,
) -> AgentResult<()> {
    let keys = raw.as_object();
    let has = |key: &str| keys.is_some_and(|map| map.contains_key(key));
    let required: &[&str] = match context {
        ObjectContext::Composition => &["id", "type"],
        ObjectContext::Add => &["id", "type", "at", "size"],
        ObjectContext::Update => &[],
    };
    for key in required {
        if !has(key) {
            return Err(AgentError::invalid(format!("{field}.{key} is required.")));
        }
    }
    let forbidden: &[&str] = match context {
        ObjectContext::Composition => &["at"],
        ObjectContext::Add => &["region", "pinnedAt"],
        ObjectContext::Update => &[
            "id", "type", "size", "rotation", "bind", "at", "region", "pinnedAt",
        ],
    };
    for key in forbidden {
        if has(key) {
            return Err(AgentError::invalid(format!(
                "{field}.{key} is not allowed here."
            )));
        }
    }
    if let Some(id) = &object.id
        && !is_object_id(id)
    {
        return Err(AgentError::invalid(format!(
            "{field}.id must start with a letter and hold letters, digits, '_' or '-'."
        )));
    }
    if let Some([width, height]) = object.size
        && (width < MIN_SIZE || height < MIN_SIZE)
    {
        return Err(AgentError::invalid(format!(
            "{field}.size must be at least {MIN_SIZE} pixels per side."
        )));
    }
    if let Some(rotation) = object.rotation
        && ![0, 90, 180, 270].contains(&rotation)
    {
        return Err(AgentError::invalid(format!(
            "{field}.rotation must be 0, 90, 180 or 270."
        )));
    }
    if let Some([x, y]) = object.pinned_at {
        grid_position(&format!("{field}.pinnedAt"), x, y)?;
    }
    if let Some(at) = object.at
        && at.iter().any(|value| *value < -MAX_DESIGN)
    {
        return Err(AgentError::invalid(format!(
            "{field}.at is outside the design."
        )));
    }
    if let Some(decimals) = object.decimals
        && decimals > 8
    {
        return Err(AgentError::invalid(format!(
            "{field}.decimals must be at most 8."
        )));
    }
    Ok(())
}

/// Whether an object ID is a letter, then letters, digits, `_` or `-`.
fn is_object_id(id: &str) -> bool {
    let mut characters = id.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
}

/// Checks a grid position: each coordinate from 0 to 4096, on the 8-pixel grid.
fn grid_position(field: &str, x: i64, y: i64) -> AgentResult<()> {
    for (name, value) in [("x", x), ("y", y)] {
        if !(0..=MAX_DESIGN).contains(&value) || value % MIN_SIZE != 0 {
            return Err(AgentError::invalid(format!(
                "{field}.{name} must be 0 to {MAX_DESIGN} on the 8-pixel grid."
            )));
        }
    }
    Ok(())
}

/// Checks one region's coordinates and size on the grid.
fn check_grid_region(field: &str, region: &GridRegion) -> AgentResult<()> {
    grid_position(field, region.x, region.y)?;
    for (name, value) in [("width", region.width), ("height", region.height)] {
        if !(MIN_SIZE..=MAX_DESIGN).contains(&value) || value % MIN_SIZE != 0 {
            return Err(AgentError::invalid(format!(
                "{field}.{name} must be {MIN_SIZE} to {MAX_DESIGN} on the 8-pixel grid."
            )));
        }
    }
    Ok(())
}

/// Checks a value on the grid between `min` and `max`, both on the grid.
fn grid_step(field: &str, value: i64, min: i64, max: i64) -> AgentResult<()> {
    if !(min..=max).contains(&value) || value % MIN_SIZE != 0 {
        return Err(AgentError::invalid(format!(
            "{field} must be {min} to {max} on the 8-pixel grid."
        )));
    }
    Ok(())
}

/// Decodes one nested payload, naming its field in the refusal.
fn decode_nested<T: for<'de> Deserialize<'de>>(field: &str, raw: &Value) -> AgentResult<T> {
    serde_json::from_value(raw.clone())
        .map_err(|error| AgentError::invalid(format!("{field}: {error}.")))
}

/// The page file a tool names: `hmi/pages/<name>.toml`, with a name of letters, digits, `_` and `-`.
#[must_use]
pub(super) fn is_page_file(text: &str) -> bool {
    text.strip_prefix("hmi/pages/")
        .and_then(|rest| rest.strip_suffix(".toml"))
        .is_some_and(|name| {
            !name.is_empty()
                && name.chars().all(|character| {
                    character.is_ascii_alphanumeric() || character == '_' || character == '-'
                })
        })
}

/// Checks a page file name (`PAGE` in `service.ts`).
fn page_file(field: &str, file: &str) -> AgentResult<()> {
    if !is_page_file(file) {
        return Err(AgentError::invalid(format!(
            "{field} must be hmi/pages/<name>.toml with letters, digits, '_' or '-'."
        )));
    }
    Ok(())
}

/// Checks an inspected file: a page, a popup or a faceplate, under `hmi/pages/`, `hmi/popups/` or
/// `hmi/panels/`.
fn page_or_popup_or_panel(field: &str, file: &str) -> AgentResult<()> {
    let folder = ["hmi/pages/", "hmi/popups/", "hmi/panels/"]
        .iter()
        .find_map(|prefix| file.strip_prefix(prefix));
    let named = folder.is_some_and(|rest| {
        rest.strip_suffix(".toml")
            .is_some_and(|name| !name.is_empty() && !name.contains('/') && !name.contains('\\'))
    });
    if !named {
        return Err(AgentError::invalid(format!(
            "{field} must be a project-relative hmi/pages, hmi/popups or hmi/panels TOML path."
        )));
    }
    Ok(())
}

/// Checks that a text's length in characters lies between `min` and `max`.
fn text_length(field: &str, text: &str, min: usize, max: usize) -> AgentResult<()> {
    let length = text.chars().count();
    if !(min..=max).contains(&length) {
        return Err(AgentError::invalid(format!(
            "{field} must be {min} to {max} characters."
        )));
    }
    Ok(())
}

/// Checks that each text of a list is a non-empty name.
fn text_items(field: &str, items: &[String]) -> AgentResult<()> {
    if items.iter().any(|item| item.is_empty()) {
        return Err(AgentError::invalid(format!(
            "{field} must hold non-empty text."
        )));
    }
    Ok(())
}

/// Checks that no item of a list repeats (`uniqueItems`).
fn unique_items<T: PartialEq>(field: &str, items: &[T]) -> AgentResult<()> {
    for (index, item) in items.iter().enumerate() {
        if items[..index].contains(item) {
            return Err(AgentError::invalid(format!(
                "{field} must not repeat an entry."
            )));
        }
    }
    Ok(())
}

/// Checks that a count lies between `min` and `max`.
fn count_between(field: &str, count: u64, min: u64, max: u64) -> AgentResult<()> {
    if !(min..=max).contains(&count) {
        return Err(AgentError::invalid(format!(
            "{field} must be {min} to {max}."
        )));
    }
    Ok(())
}

/// Checks a width and height in pixels, each from 1 to 4096.
fn size_between(field: &str, width: i64, height: i64) -> AgentResult<()> {
    for value in [width, height] {
        if !(1..=MAX_DESIGN).contains(&value) {
            return Err(AgentError::invalid(format!(
                "{field} width and height must be 1 to {MAX_DESIGN} pixels."
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
