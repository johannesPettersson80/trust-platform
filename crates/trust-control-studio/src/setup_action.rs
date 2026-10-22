//! The loop setup methods `controlStudio/setupDiscover` and `controlStudio/setupSave` (specification 35, B1.2 and B1.3, and the
//! `setup` verb of §0.1b).
//!
//! `setupDiscover` answers the compiled project's setup form and the configured loops, each checked against the compiled
//! catalog, with every rejection and its reason. `setupSave` writes a confirmed draft as loop files, all or none, and never over
//! an existing file or loop identifier. Every rule is trust-control's (`setup`). This module compiles the project through
//! `trust-lsp command` (§0.1a), reads the loop files, creates the new ones and forwards the answers.

use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::json::parse_text;
use trust_control::loop_definition::{LoopDefinition, LoopScope};
use trust_control::model::GeneralModel;
use trust_control::setup::binding_route::answer_selection;
use trust_control::setup::catalog::{decode_catalog_answer, CatalogAnswer};
use trust_control::setup::discovery::{
    discover_bindings, estimator_model_files, refused, BindingTarget, LoopFileRead, LoopFileText,
    ModelFileText, MAXIMUM_LOOP_BYTES,
};
use trust_control::setup::form::setup_form_model;
use trust_control::setup::mapping_match::{match_mapping, own_mapping};
use trust_control::setup::messages::{
    ConfiguredLoopRequest, LoopSummary, SavedLoop, SetupDiscoverRequest, SetupDiscoverResult,
    SetupSaveRequest, SetupSaveResult,
};
use trust_control::setup::save::{
    check_setup_targets, decode_estimator_model, estimator_model_path, parse_setup_draft,
    plan_setup_save_with_estimator, ExistingLoopFile, LoopFileId, PlannedLoop, SetupDraft,
};
use trust_control::setup::step_answers;
use trust_control::setup::texts::save as texts;

use crate::busy_loops::{LoopClaim, LoopClaims};
use crate::commands::to_value;
use crate::language_tool::{LanguageTool, ProjectTool};
use crate::loops;

/// The operation name a binding change holds its loop under, which a refusal of another request names (A8.5).
const BINDING_OPERATION: &str = "Binding change";

/// The command that answers the compiled project's control catalog (specification 14 §6.6.2 and §7.8). Its argument is empty:
/// the project is read from disk.
const CATALOG_COMMAND: &str = "trust-lsp.hmiBindings";

/// Answers `controlStudio/setupDiscover` (B1.2, B1.3): the setup form of the compiled project, and the configured loops that
/// the compiled catalog confirms, with each rejected definition and its reason.
///
/// # Errors
/// The compiled project cannot be read: the compiler's own reason, as `trust-lsp command` answers it.
pub fn answer_discover(
    request: &SetupDiscoverRequest,
    claims: &LoopClaims,
) -> Result<Value, Failure> {
    let project = Path::new(&request.project_root);
    let catalog = compiled_catalog(request.language_tool.as_deref(), project)?;
    let discovery = match loop_file_texts(project) {
        Ok(files) => {
            discover_bindings(&request.project_root, &catalog, &request.runtime_id, &files)
        }
        // The folder cannot be read: nothing is discovered, and the rejection keeps the reason (B1.2, A11.1).
        Err(failure) => refused(
            &project.join("control-loops").display().to_string(),
            &failure.message,
        ),
    };
    // The binding the three-page panel chose is checked against the loops this discovery lists (B1.3, C0).
    let selected = request
        .selection
        .as_ref()
        .map(|selection| answer_selection(&discovery, selection, request.family.as_deref()))
        .transpose()?;
    // A binding change is refused while an operation of this engine works on the selected loop; the claim lasts for this answer (B1.3, A8.5).
    let _claim = selected
        .map(|target| claim_binding(claims, &target.loop_definition))
        .transpose()?;
    let selected_file = selected.map(|target| target.configuration_path.clone());
    let selected_request =
        selected.map(|target| ConfiguredLoopRequest::of(target.loop_definition.clone()));
    // The model files an LQG estimator can implement (C4 Setup), listed by the rules of the discovery.
    let estimator_models = estimator_model_files(&model_file_texts(project));
    // The mapping the selectors name, or, for a panel that names none, the choices of the loop it shows (B1.2, B3.6).
    let mapping = match (&request.mapping, &request.loop_id) {
        (Some(mapping), _) => Some(match_mapping(&discovery, mapping)),
        (None, Some(loop_id)) => own_mapping(&discovery, loop_id),
        (None, None) => None,
    };
    to_value(&SetupDiscoverResult {
        form_model: setup_form_model(&catalog.control),
        loops: discovery.targets.iter().map(loop_summary).collect(),
        rejected: discovery.rejected,
        selected_file,
        estimator_models,
        selected_request,
        mapping,
    })
}

/// Answers `controlStudio/setupSave` (B1.3, §0.1b): generates the loops of the confirmed draft, checks them with the loop
/// contract, and writes the files all or none. A file that exists or a loop identifier already declared refuses the save and
/// changes nothing.
///
/// # Errors
/// The draft is not a setup this engine saves (its structure is parked or unknown, or a choice is incomplete); a step-by-step answer is
/// not of its question's kind or names no compiled entry; the compiled project cannot be read; the loops do not satisfy the loop
/// contract; a file or identifier is taken; or a file cannot be written.
pub fn answer_save(request: &SetupSaveRequest) -> Result<Value, Failure> {
    let project = Path::new(&request.project_root);
    let scope = LoopScope {
        project_id: project_id(project),
        runtime_id: request.runtime_id.clone(),
    };
    // The step-by-step draft is the answers of its questions: the engine builds its loop from them (`configure_loop`, B1.3).
    let planned = if step_answers::is_step_by_step(&request.draft) {
        let catalog = compiled_catalog(request.language_tool.as_deref(), project)?;
        vec![step_answers::plan(&request.draft, &catalog, &scope)?]
    } else {
        let draft = parse_setup_draft(&request.draft)?;
        let catalog = compiled_catalog(request.language_tool.as_deref(), project)?;
        let estimator = estimator_model_for(project, &draft)?;
        plan_setup_save_with_estimator(&catalog.control, &scope, &draft, estimator.as_ref())?
    };
    check_setup_targets(&existing_loop_files(project)?, &planned)?;
    write_all_or_none(project, &planned)?;
    to_value(&SetupSaveResult {
        loops: planned
            .iter()
            .map(|plan| SavedLoop {
                id: plan.loop_definition.id.clone(),
                name: plan.loop_definition.name.clone(),
                file: plan.file_name.clone(),
            })
            .collect(),
        // The page opens Control Studio on the first loop written (B1.3).
        open_request: planned
            .first()
            .map(|plan| ConfiguredLoopRequest::of(plan.loop_definition.clone())),
    })
}

/// Claims the loop a binding change selects (B1.3, A8.5): a loop that a Test or another operation of this engine is working on refuses the
/// change, and the refusal names that operation.
///
/// # Errors
/// An operation of this engine is working on the loop.
fn claim_binding(
    claims: &LoopClaims,
    loop_definition: &LoopDefinition,
) -> Result<LoopClaim, Failure> {
    claims.claim(
        &loop_definition.project_id,
        &loop_definition.id,
        BINDING_OPERATION,
    )
}

/// The compiled project's control catalog, from `trust-lsp command` (§0.1a). The language tool is the given executable, or the one
/// beside the engine or on the PATH.
pub(crate) fn compiled_catalog(
    language_tool: Option<&str>,
    project: &Path,
) -> Result<CatalogAnswer, Failure> {
    let tool = LanguageTool::new(language_tool.map(PathBuf::from));
    let answer = tool.answer(CATALOG_COMMAND, project, &json!({}))?;
    decode_catalog_answer(&answer)
}

/// The project's loop files as the discovery reads them (B1.2): the root file first, then `control-loops/` in name order. A file
/// that is missing is skipped; an over-large or unreadable one is passed on, and the discovery explains it.
fn loop_file_texts(project: &Path) -> Result<Vec<LoopFileText>, Failure> {
    let mut files = Vec::new();
    for path in loops::loop_files(project)? {
        if let Some(read) = read_loop_file(&path) {
            files.push(LoopFileText {
                configuration_path: path.display().to_string(),
                read,
            });
        }
    }
    Ok(files)
}

/// One loop file as the discovery needs it; `None` when the file does not exist.
fn read_loop_file(path: &Path) -> Option<LoopFileRead> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return None,
        Err(_) => return Some(LoopFileRead::Unreadable),
    };
    if metadata.len() > MAXIMUM_LOOP_BYTES {
        return Some(LoopFileRead::TooLarge {
            bytes: metadata.len(),
        });
    }
    match fs::read_to_string(path) {
        Ok(text) => Some(LoopFileRead::Text(text)),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(_) => Some(LoopFileRead::Unreadable),
    }
}

/// The project's model files as the estimator list reads them (C4 Setup): the project folder, then `models/`, each with its `.json` files
/// in name order. A missing folder or file is skipped; an over-large or unreadable file is passed on, and the discovery leaves it out.
fn model_file_texts(project: &Path) -> Vec<ModelFileText> {
    let mut files = Vec::new();
    for folder in ["", "models"] {
        let directory = if folder.is_empty() {
            project.to_path_buf()
        } else {
            project.join(folder)
        };
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.to_lowercase().ends_with(".json"))
            .collect();
        names.sort();
        for name in names {
            let Some(read) = read_loop_file(&directory.join(&name)) else {
                continue;
            };
            let file_name = if folder.is_empty() {
                name
            } else {
                format!("{folder}/{name}")
            };
            files.push(ModelFileText { file_name, read });
        }
    }
    files
}

/// The model an LQG estimator implements, read from the project and decoded (C4 Setup; `estimatorModel` of `loopSetupSave.ts`). `None`
/// for a draft that names no model file: the generator refuses such a draft.
///
/// # Errors
/// The file lies outside the project, cannot be read, or is not a model the project accepts.
fn estimator_model_for(
    project: &Path,
    draft: &SetupDraft,
) -> Result<Option<GeneralModel>, Failure> {
    let Some(file) = draft.estimator_model_file() else {
        return Ok(None);
    };
    let relative = estimator_model_path(file)?;
    let text = fs::read_to_string(project.join(relative)).map_err(|_| {
        Failure::new(
            &texts::model_unreadable(file),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        )
    })?;
    decode_estimator_model(&text, file).map(Some)
}

/// The summary of one discovered loop, as the setup page lists it.
fn loop_summary(target: &BindingTarget) -> LoopSummary {
    LoopSummary {
        loop_id: target.loop_definition.id.clone(),
        name: target.loop_definition.name.clone(),
        configuration_path: target.configuration_path.clone(),
        controller: target.controller.clone(),
        inputs: target.inputs.clone(),
        outputs: target.outputs.clone(),
        controller_compatibility: target.controller_compatibility.clone(),
    }
}

/// The loop files of the project as the save's refusals need them: each file's relative name and the identifier it declares.
/// A file that is missing is skipped; a file whose JSON cannot be read is reported as unreadable (B1.2).
fn existing_loop_files(project: &Path) -> Result<Vec<ExistingLoopFile>, Failure> {
    let mut existing = Vec::new();
    for path in loops::loop_files(project)? {
        let id = match fs::read_to_string(&path) {
            Ok(text) => loop_file_id(&text),
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(_) => LoopFileId::Unreadable,
        };
        let file_name = path
            .strip_prefix(project)
            .unwrap_or(path.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        existing.push(ExistingLoopFile { file_name, id });
    }
    Ok(existing)
}

/// The identifier a loop file declares: `NoId` for JSON without one, `Unreadable` for text that is not JSON.
fn loop_file_id(text: &str) -> LoopFileId {
    match parse_text(text) {
        Ok(value) => value
            .get("id")
            .and_then(Value::as_str)
            .map_or(LoopFileId::NoId, |id| LoopFileId::Id(id.to_string())),
        Err(_) => LoopFileId::Unreadable,
    }
}

/// Writes every planned loop as a new file (B1.3). A file that appears meanwhile is refused, and when a write fails the files
/// this save already created are removed, so the project keeps none of them.
fn write_all_or_none(project: &Path, planned: &[PlannedLoop]) -> Result<(), Failure> {
    fs::create_dir_all(project.join("control-loops"))
        .map_err(|error| io_failure("The control-loops directory cannot be created", &error))?;
    let mut created: Vec<PathBuf> = Vec::new();
    for plan in planned {
        let path = project.join(&plan.file_name);
        if let Err(failure) = write_new_file(&path, plan) {
            for file in &created {
                let _ = fs::remove_file(file);
            }
            return Err(failure);
        }
        created.push(path);
    }
    Ok(())
}

/// Creates one loop file, failing when it exists, and writes the loop as the JSON of the loop contract: two-space indent and a
/// final line break. A file whose write fails is removed again.
fn write_new_file(path: &Path, plan: &PlannedLoop) -> Result<(), Failure> {
    let text = serde_json::to_string_pretty(&plan.loop_definition).map_err(|error| {
        Failure::new(
            &format!(
                "The loop {} cannot be written: {error}.",
                plan.loop_definition.name
            ),
            Cause::Implementation,
            NextAction::None,
        )
    })?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_failure(&format!("{} cannot be created", path.display()), &error))?;
    if let Err(error) = file.write_all(format!("{text}\n").as_bytes()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(io_failure(
            &format!("{} cannot be written", path.display()),
            &error,
        ));
    }
    Ok(())
}

/// The project's identifier: `project:` and the SHA-256 of its folder's file address, as the extension's setup forms it.
fn project_id(project: &Path) -> String {
    let folder = fs::canonicalize(project).unwrap_or_else(|_| project.to_path_buf());
    let digest = Sha256::digest(format!("file://{}", folder.display()).as_bytes());
    let mut id = String::from("project:");
    for byte in digest {
        let _ = write!(id, "{byte:02x}");
    }
    id
}

/// A file-system failure of the save, with the operating system's reason.
fn io_failure(text: &str, error: &std::io::Error) -> Failure {
    Failure::new(
        &format!("{text}: {error}."),
        Cause::Unclassified,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "setup_action_tests.rs"]
mod tests;
