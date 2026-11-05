//! The draft a simulator journey exercises (specification 43, AT-08): the pages of the draft and
//! revision that the tool call names, while the program still has the revision the draft was
//! composed against. The journey's thread reads them, since the program's revision can take a
//! compilation.
//!
//! Port of `simulatorJourney` in `editors/vscode/src/hmiBuilder/agent/previews.ts`.

use serde_json::{Map, Value};

use crate::state::ServerState;

use super::argument::Argument;
use super::drafts::{require_retained_pages, DraftStore};
use super::error::{AgentError, AgentResult};
use super::native;
use super::service::{at_revision, owned_draft};

/// The models of the draft's pages by page name, the stem of each page file
/// (`hmi/pages/pumps.toml` holds the page `pumps`), as a journey's steps name the deployed pages.
///
/// # Errors
///
/// The refusals of a draft read: `EXPIRED_DRAFT` for a draft that is unavailable, discarded or
/// applied, `PROJECT_UNAVAILABLE` for another workspace's; `CONFLICT` for a draft at another
/// revision or composed against another program revision than the program's; `COMPILATION_FAILED`
/// when the program does not compile; `VALIDATION_FAILED` for a page that does not load.
pub(super) fn draft_models(
    state: &ServerState,
    argument: &Argument,
    draft_id: &str,
    revision: &str,
) -> AgentResult<Map<String, Value>> {
    let store = DraftStore::open(&argument.storage)?;
    let draft = owned_draft(&store, argument, draft_id)?;
    require_retained_pages(&draft)?;
    at_revision(&draft, revision)?;
    let bindings = native::bindings(state, &argument.root_uri)?;
    let program = bindings.get("sourceRevision").and_then(Value::as_str);
    if draft.program_revision.is_none() || draft.program_revision.as_deref() != program {
        return Err(AgentError::conflict(
            "the program changed since this simulation proposal was created.",
        ));
    }
    draft
        .files
        .iter()
        .map(|(file, text)| {
            let model = native::parse_model(state, "page", text, file)?;
            Ok((page_name(file).to_owned(), model))
        })
        .collect()
}

/// The page that a page file holds: the file's stem.
fn page_name(file: &str) -> &str {
    let name = file.rsplit('/').next().unwrap_or(file);
    name.strip_suffix(".toml").unwrap_or(name)
}

#[cfg(test)]
#[path = "simulator_draft_tests.rs"]
mod tests;
