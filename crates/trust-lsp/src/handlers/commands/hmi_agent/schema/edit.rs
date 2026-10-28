//! The edit input of `hmi_edit_draft` (AT-05) and its operations, each checked by its own fields.

use serde::Deserialize;
use serde_json::Value;

use super::super::error::{AgentError, AgentResult};
use super::compose::{Axis, BindingInput, EqualSize, GridRegion, ObjectInput, PageProperties};
use super::{
    check_grid_region, check_object, decode_nested, grid_step, page_file, text_length,
    unique_items, Bounded, ObjectContext, MAX_BATCH, MAX_GAP, MAX_WAYPOINTS, MIN_SIZE,
};

/// The input of `hmi_edit_draft` (AT-05). The operations stay JSON; each is checked by its type.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(in super::super) struct EditInput {
    /// The draft to edit.
    pub(in super::super) draft_id: String,
    /// The revision the edit starts from.
    pub(in super::super) revision: String,
    /// The idempotency key of this edit.
    pub(in super::super) key: String,
    /// The page to edit, when the draft holds more than one.
    pub(in super::super) file: Option<String>,
    /// Whether unresolved process-first bindings may stay in the private draft.
    pub(in super::super) allow_unresolved: Option<bool>,
    /// The operations, applied in order.
    pub(in super::super) operations: Vec<Value>,
}

impl Bounded for EditInput {
    fn bounds(&self) -> AgentResult<()> {
        text_length("input.draftId", &self.draft_id, 1, usize::MAX)?;
        text_length("input.revision", &self.revision, 1, usize::MAX)?;
        text_length("input.key", &self.key, 1, 200)?;
        if let Some(file) = &self.file {
            page_file("input.file", file)?;
        }
        if self.operations.is_empty() || self.operations.len() > MAX_BATCH {
            return Err(AgentError::limit(format!(
                "input.operations must hold 1 to {MAX_BATCH} operations."
            )));
        }
        for (index, operation) in self.operations.iter().enumerate() {
            let field = format!("input.operations[{index}]");
            let decoded: EditOperation = decode_nested(&field, operation)?;
            decoded.bounds(&field, operation)?;
        }
        Ok(())
    }
}

/// One edit operation (`operations`), discriminated by `op`. Each kind takes only its own keys.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
#[allow(dead_code)] // Each operation is validated by its fields; `edits.rs` applies the same JSON.
pub(in super::super) enum EditOperation {
    /// Moves the objects by a delta.
    Move { ids: Vec<String>, delta: [i64; 2] },
    /// Resizes one object.
    Resize { id: String, size: [i64; 2] },
    /// Changes the properties of one object.
    Update { id: String, properties: ObjectInput },
    /// Adds an object.
    Add { object: ObjectInput },
    /// Removes objects, and optionally what depends on them.
    Remove {
        ids: Vec<String>,
        #[serde(rename = "removeDependents")]
        remove_dependents: Option<bool>,
    },
    /// Copies objects with new IDs.
    Duplicate {
        ids: Vec<String>,
        #[serde(rename = "newIds")]
        new_ids: Vec<String>,
        delta: [i64; 2],
    },
    /// Aligns objects to an edge.
    Align { ids: Vec<String>, edge: AlignEdge },
    /// Distributes objects along an axis.
    Distribute { ids: Vec<String>, axis: Axis },
    /// Puts objects in a group.
    Group { ids: Vec<String>, group: String },
    /// Takes objects out of their group.
    Ungroup { ids: Vec<String> },
    /// Binds one property of an object.
    Bind {
        id: String,
        property: String,
        binding: BindingInput,
    },
    /// Removes one property's binding.
    Unbind { id: String, property: String },
    /// Connects two ports with a pipe.
    Connect { from: String, to: String },
    /// Removes the pipe between two ports.
    Disconnect { from: String, to: String },
    /// Moves objects to the front or the back.
    Reorder {
        ids: Vec<String>,
        position: Position,
    },
    /// Gives objects the size of a reference object.
    EqualSize {
        ids: Vec<String>,
        reference: String,
        axis: EqualSize,
    },
    /// Arranges objects in a grid region, keeping pinned ones.
    Arrange {
        ids: Vec<String>,
        region: GridRegion,
        #[serde(rename = "pinnedIds")]
        pinned_ids: Option<Vec<String>>,
        gap: Option<i64>,
        flow: Option<Axis>,
    },
    /// Sets the waypoints of one connection.
    Reroute {
        from: String,
        to: String,
        waypoints: Vec<[i64; 2]>,
    },
    /// Moves the ends of one connection.
    Reconnect {
        from: String,
        to: String,
        #[serde(rename = "newFrom")]
        new_from: String,
        #[serde(rename = "newTo")]
        new_to: String,
    },
    /// Changes the page's own properties.
    Page { properties: PageProperties },
}

/// The edge an alignment uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // Validated by its type; `align.rs` reads the same JSON.
pub(in super::super) enum AlignEdge {
    /// The left edge.
    Left,
    /// The centre.
    Centre,
    /// The right edge.
    Right,
    /// The top edge.
    Top,
    /// The middle.
    Middle,
    /// The bottom edge.
    Bottom,
}

/// Whether an object goes to the front or the back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // Validated by its type; the owning module reads the same JSON.
pub(in super::super) enum Position {
    /// Drawn last.
    Front,
    /// Drawn first.
    Back,
}

impl EditOperation {
    /// Checks the bounds of one operation, naming the operation's field.
    fn bounds(&self, field: &str, raw: &Value) -> AgentResult<()> {
        let ids = match self {
            Self::Move { ids, .. }
            | Self::Duplicate { ids, .. }
            | Self::Remove { ids, .. }
            | Self::Align { ids, .. }
            | Self::Distribute { ids, .. }
            | Self::Group { ids, .. }
            | Self::Ungroup { ids }
            | Self::Reorder { ids, .. }
            | Self::EqualSize { ids, .. }
            | Self::Arrange { ids, .. } => Some(ids),
            _ => None,
        };
        if let Some(ids) = ids {
            if ids.is_empty() {
                return Err(AgentError::invalid(format!(
                    "{field}.ids must name at least one object."
                )));
            }
            unique_items(&format!("{field}.ids"), ids)?;
        }
        match self {
            Self::Resize { size, .. } => {
                if size.iter().any(|side| *side < MIN_SIZE) {
                    return Err(AgentError::invalid(format!(
                        "{field}.size must be at least {MIN_SIZE} pixels per side."
                    )));
                }
            }
            Self::Update { properties, .. } => {
                let raw_properties = raw.get("properties").cloned().unwrap_or(Value::Null);
                check_object(
                    &format!("{field}.properties"),
                    &raw_properties,
                    properties,
                    ObjectContext::Update,
                )?;
            }
            Self::Add { object } => {
                let raw_object = raw.get("object").cloned().unwrap_or(Value::Null);
                check_object(
                    &format!("{field}.object"),
                    &raw_object,
                    object,
                    ObjectContext::Add,
                )?;
            }
            Self::Duplicate { new_ids, .. } => {
                if new_ids.is_empty() {
                    return Err(AgentError::invalid(format!(
                        "{field}.newIds must name at least one new ID."
                    )));
                }
                unique_items(&format!("{field}.newIds"), new_ids)?;
            }
            Self::Arrange {
                region,
                pinned_ids,
                gap,
                ..
            } => {
                check_grid_region(&format!("{field}.region"), region)?;
                if let Some(pinned) = pinned_ids {
                    if pinned.len() > MAX_BATCH {
                        return Err(AgentError::limit(format!(
                            "{field}.pinnedIds may hold at most {MAX_BATCH} IDs."
                        )));
                    }
                    unique_items(&format!("{field}.pinnedIds"), pinned)?;
                }
                if let Some(gap) = gap {
                    grid_step(&format!("{field}.gap"), *gap, MIN_SIZE, MAX_GAP)?;
                }
            }
            Self::Reroute { waypoints, .. } => {
                if waypoints.len() > MAX_WAYPOINTS {
                    return Err(AgentError::limit(format!(
                        "{field}.waypoints may hold at most {MAX_WAYPOINTS} points."
                    )));
                }
            }
            Self::Page { properties } => {
                if let Some(title) = &properties.title {
                    text_length(&format!("{field}.properties.title"), title, 1, 1000)?;
                }
                let any = [
                    properties.title.is_some(),
                    properties.order.is_some(),
                    properties.icon.is_some(),
                    properties.group.is_some(),
                    properties.hidden.is_some(),
                    properties.incomplete.is_some(),
                ];
                if !any.contains(&true) {
                    return Err(AgentError::invalid(format!(
                        "{field}.properties must change at least one property."
                    )));
                }
            }
            _ => {}
        }
        Ok(())
    }
}
