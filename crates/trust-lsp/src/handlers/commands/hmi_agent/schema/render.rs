//! The validation, render and preview inputs (AT-06 to AT-08) and their journey steps.

use serde::Deserialize;
use serde_json::Value;

use super::super::error::{AgentError, AgentResult};
use super::compose::Scalar;
use super::{
    page_file, size_between, text_length, unique_items, Bounded, Theme, MAX_ISSUE_OBJECTS,
    MAX_PREVIEW, MAX_VIEWPORTS,
};

/// The input of `hmi_validate_draft` (AT-06).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(in super::super) struct ValidateInput {
    /// The draft to validate.
    pub(in super::super) draft_id: String,
    /// The revision it is validated at.
    pub(in super::super) revision: String,
    /// The target viewports, at most three. The geometry reads the same JSON through `viewport_profiles`.
    #[allow(dead_code)] // Checked here; the answer reads the raw viewports.
    pub(in super::super) viewports: Option<Vec<Viewport>>,
    /// Whether to return separate PLC edit proposals.
    pub(in super::super) propose_program: Option<bool>,
}

/// A target viewport: the available HMI content area, in pixels.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Viewport {
    /// The identifier, unique within the request.
    pub(in super::super) id: String,
    /// The width, from 1 to 4096.
    pub(in super::super) width: i64,
    /// The height, from 1 to 4096.
    pub(in super::super) height: i64,
}

impl Bounded for ValidateInput {
    fn bounds(&self) -> AgentResult<()> {
        text_length("input.draftId", &self.draft_id, 1, usize::MAX)?;
        text_length("input.revision", &self.revision, 1, usize::MAX)?;
        if let Some(viewports) = &self.viewports {
            viewport_list(viewports)?;
        }
        Ok(())
    }
}

/// Checks the viewports of a validation or a render: at most three, unique identifiers, sizes 1 to 4096.
fn viewport_list(viewports: &[Viewport]) -> AgentResult<()> {
    if viewports.len() > MAX_VIEWPORTS {
        return Err(AgentError::limit(format!(
            "input.viewports may name at most {MAX_VIEWPORTS} viewports."
        )));
    }
    let ids: Vec<String> = viewports
        .iter()
        .map(|viewport| viewport.id.clone())
        .collect();
    unique_items("input.viewports.id", &ids)?;
    for viewport in viewports {
        viewport_id(&viewport.id)?;
        size_between("input.viewports", viewport.width, viewport.height)?;
    }
    Ok(())
}

/// Checks a viewport identifier: a letter, then at most 63 letters, digits, `_` or `-`.
fn viewport_id(id: &str) -> AgentResult<()> {
    let mut characters = id.chars();
    let valid = characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && id.len() <= 64
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        });
    if !valid {
        return Err(AgentError::invalid(
            "input.viewports.id must start with a letter and hold at most 64 letters, digits, '_' or '-'.",
        ));
    }
    Ok(())
}

/// A native-pixel crop of a rendered page: existing object IDs, or a rectangle.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(in super::super) enum Crop {
    /// The objects to show, 1 to 500 of them.
    Ids(CropIds),
    /// A rectangle of the page.
    Rect(CropRect),
}

/// A crop by object IDs.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct CropIds {
    /// The object IDs.
    pub(in super::super) ids: Vec<String>,
}

/// A crop by rectangle, in whole native pixels.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct CropRect {
    /// The left edge, from 0.
    pub(in super::super) x: i64,
    /// The top edge, from 0.
    pub(in super::super) y: i64,
    /// The width, at least 1.
    pub(in super::super) width: i64,
    /// The height, at least 1.
    pub(in super::super) height: i64,
}

/// The input of `hmi_render_draft` (AT-07). Rendering stays in the extension; the service checks here.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[allow(dead_code)] // The extension renders from the same input; the service only checks it.
pub(in super::super) struct RenderInput {
    /// The draft to render.
    draft_id: String,
    /// The revision to render.
    revision: String,
    /// The theme.
    theme: Option<Theme>,
    /// The page to render.
    file: Option<String>,
    /// The crop, with the whole page returned too.
    crop: Option<Crop>,
    /// The language the project declares.
    language: Option<String>,
    /// The viewport to render.
    viewport: Option<ViewportSize>,
}

/// A viewport size without an identifier.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)] // Read through `RenderInput` and `PreviewInput`, which the check decodes.
pub(in super::super) struct ViewportSize {
    /// The width, from 1 to 4096.
    width: i64,
    /// The height, from 1 to 4096.
    height: i64,
}

impl Bounded for RenderInput {
    fn bounds(&self) -> AgentResult<()> {
        text_length("input.draftId", &self.draft_id, 1, usize::MAX)?;
        text_length("input.revision", &self.revision, 1, usize::MAX)?;
        if let Some(file) = &self.file {
            page_file("input.file", file)?;
        }
        if let Some(language) = &self.language {
            text_length("input.language", language, 1, usize::MAX)?;
        }
        if let Some(viewport) = self.viewport {
            size_between("input.viewport", viewport.width, viewport.height)?;
        }
        match &self.crop {
            Some(Crop::Ids(crop)) => {
                if crop.ids.is_empty() || crop.ids.len() > MAX_ISSUE_OBJECTS {
                    return Err(AgentError::limit(format!(
                        "input.crop.ids must name 1 to {MAX_ISSUE_OBJECTS} objects."
                    )));
                }
            }
            Some(Crop::Rect(rect))
                if rect.x < 0 || rect.y < 0 || rect.width < 1 || rect.height < 1 =>
            {
                return Err(AgentError::invalid(
                    "input.crop needs whole x and y from 0 and a width and height of at least 1.",
                ));
            }
            Some(Crop::Rect(_)) | None => {}
        }
        Ok(())
    }
}

/// The input of `hmi_preview_states` (AT-08): named states of the draft, or a simulator journey.
#[derive(Debug)]
pub(in super::super) enum PreviewInput {
    /// Synthetic states of the draft's page.
    States(StatesPreview),
    /// A named simulator session's journey over deployed pages.
    Simulator(SimulatorPreview),
}

impl<'de> Deserialize<'de> for PreviewInput {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let simulator = value
            .as_object()
            .is_some_and(|object| object.contains_key("mode"));
        if simulator {
            SimulatorPreview::deserialize(value)
                .map(Self::Simulator)
                .map_err(serde::de::Error::custom)
        } else {
            StatesPreview::deserialize(value)
                .map(Self::States)
                .map_err(serde::de::Error::custom)
        }
    }
}

/// Synthetic states of the draft's page.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[allow(dead_code)] // Read by the extension's renderer; the service checks the shape here.
pub(in super::super) struct StatesPreview {
    /// The draft.
    draft_id: String,
    /// The revision.
    revision: String,
    /// The state names, 1 to 12.
    states: Vec<String>,
    /// The theme.
    theme: Option<Theme>,
    /// The page.
    file: Option<String>,
    /// The language.
    language: Option<String>,
    /// The viewport.
    viewport: Option<ViewportSize>,
}

/// The mode of a simulator journey; the only value is `simulator`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // Validated by its only value.
pub(in super::super) enum PreviewMode {
    /// The simulator.
    Simulator,
}

/// A simulator journey: a session and up to 12 steps over deployed pages.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[allow(dead_code)] // Read by the extension's simulator; the service checks the shape here.
pub(in super::super) struct SimulatorPreview {
    /// The draft.
    draft_id: String,
    /// The revision.
    revision: String,
    /// The mode, `simulator`.
    mode: PreviewMode,
    /// The named simulator session.
    session_id: String,
    /// The steps, 1 to 12.
    steps: Vec<JourneyStep>,
}

/// One step of a simulator journey.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[allow(dead_code)] // Read by the extension's simulator; the service checks the shape here.
pub(in super::super) struct JourneyStep {
    /// The deployed page, by name.
    page: String,
    /// The object to act on.
    object: Option<String>,
    /// The action on the object.
    action: Option<StepAction>,
    /// The value to write.
    value: Option<Scalar>,
    /// The readback path.
    path: Option<String>,
    /// The expected readback value.
    expect: Option<Scalar>,
}

/// The action a journey step performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // Validated by its type.
pub(in super::super) enum StepAction {
    /// Writes a value.
    Write,
    /// Sets a value.
    Set,
    /// Resets a value.
    Reset,
    /// Toggles a value.
    Toggle,
    /// Pulses a value.
    Pulse,
}

impl Bounded for PreviewInput {
    fn bounds(&self) -> AgentResult<()> {
        match self {
            Self::States(preview) => {
                text_length("input.draftId", &preview.draft_id, 1, usize::MAX)?;
                text_length("input.revision", &preview.revision, 1, usize::MAX)?;
                if preview.states.is_empty() || preview.states.len() > MAX_PREVIEW {
                    return Err(AgentError::limit(format!(
                        "input.states must name 1 to {MAX_PREVIEW} states."
                    )));
                }
                if let Some(file) = &preview.file {
                    page_file("input.file", file)?;
                }
                if let Some(viewport) = preview.viewport {
                    size_between("input.viewport", viewport.width, viewport.height)?;
                }
                Ok(())
            }
            Self::Simulator(journey) => {
                text_length("input.draftId", &journey.draft_id, 1, usize::MAX)?;
                text_length("input.revision", &journey.revision, 1, usize::MAX)?;
                text_length("input.sessionId", &journey.session_id, 1, usize::MAX)?;
                if journey.steps.is_empty() || journey.steps.len() > MAX_PREVIEW {
                    return Err(AgentError::limit(format!(
                        "input.steps must name 1 to {MAX_PREVIEW} steps."
                    )));
                }
                for (index, step) in journey.steps.iter().enumerate() {
                    check_step(&format!("input.steps[{index}]"), step)?;
                }
                Ok(())
            }
        }
    }
}

/// Checks one journey step's page name, its lengths and the keys that depend on each other.
fn check_step(field: &str, step: &JourneyStep) -> AgentResult<()> {
    let page_name = !step.page.is_empty()
        && step.page.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        });
    if !page_name {
        return Err(AgentError::invalid(format!(
            "{field}.page must hold letters, digits, '_' or '-'."
        )));
    }
    if let Some(object) = &step.object {
        text_length(&format!("{field}.object"), object, 0, 200)?;
    }
    if let Some(path) = &step.path {
        text_length(&format!("{field}.path"), path, 0, 1000)?;
    }
    if (step.object.is_some()) != step.action.is_some() {
        return Err(AgentError::invalid(format!(
            "{field}: object and action go together."
        )));
    }
    if step.value.is_some() && step.object.is_none() {
        return Err(AgentError::invalid(format!(
            "{field}: value needs an object."
        )));
    }
    if step.expect.is_some() && step.path.is_none() {
        return Err(AgentError::invalid(format!(
            "{field}: expect needs a path."
        )));
    }
    Ok(())
}
