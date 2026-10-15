//! The save of a confirmed setup (specification 35, B1.3, §0.1b and A11; `loopSetup/loopSetupSave.ts`, and the id check of
//! `projectLoop.ts`). The draft is read, its loop definitions are generated and checked, and the refusals are decided here: a file
//! that exists, an id that another file declares, a file that cannot be read, and a structure the Rust engine does not run yet.
//! Nothing here writes a file: the engine writes every planned loop, or none.
//!
//! File names are relative to the project root with forward slashes on both sides of the comparison: `control-loop.json` for the
//! root file and `control-loops/<name>.json` for each file of the folder. A root file named like a loop therefore never collides
//! with a planned file by accident.

use serde::de::DeserializeOwned;
use serde_json::Value;

use super::catalog::CompiledCatalog;
use super::feedforward_loop::{feedforward_loop_definition, FeedforwardLoopDraft};
use super::identify_loop::{identify_loop_definition, IdentifyLoopDraft};
use super::own_law_loop::{own_law_loop_definition, OwnLawLoopDraft};
use super::pid_loop::{pid_loop_definition, PidLoopDraft};
use super::texts::save as texts;
use super::trajectory_loop::{trajectory_loop_definition, TrajectoryLoopDraft};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{LoopDefinition, LoopRejection, LoopScope};

/// The confirmed draft of one setup, by its structure (§0.1b).
#[derive(Debug)]
pub enum SetupDraft {
    /// A PID loop on a library controller's parameter record (B1.3, structure `pid`).
    Pid(PidLoopDraft),
    /// A loop that only identifies the plant (B1.3, structure `identify`).
    Identify(IdentifyLoopDraft),
    /// A loop on the program's own law (C2, structure `own-law`).
    OwnLaw(OwnLawLoopDraft),
    /// A loop on a trajectory-following law (C11, structure `trajectory-feedforward`).
    TrajectoryFeedforward(TrajectoryLoopDraft),
    /// A loop on a measured-disturbance feedforward (C8, structure `feedforward`).
    Feedforward(FeedforwardLoopDraft),
    /// A structure the Rust engine does not run yet (stage V7). The save refuses it by name.
    Parked {
        /// The structure as the draft names it.
        structure: String,
    },
}

/// One loop the save writes (B1.3).
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedLoop {
    /// The file's path relative to the project root, with forward slashes: `control-loops/<slug>.json`, where the slug is the loop
    /// id without its `loop:` prefix.
    pub file_name: String,
    /// The loop as the save writes it.
    pub loop_definition: LoopDefinition,
}

/// What the engine found in one loop file that already exists (B1.3's refusals).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopFileId {
    /// The file declares this loop id.
    Id(String),
    /// The file is JSON without an id.
    NoId,
    /// The file is not JSON, or it cannot be read.
    Unreadable,
}

/// One loop file of the project, as the save's refusals need it (B1.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingLoopFile {
    /// The file's path relative to the project root, with forward slashes: `control-loop.json` for the root file, and
    /// `control-loops/<name>.json` for each file of the folder.
    pub file_name: String,
    /// The loop id the file declares.
    pub id: LoopFileId,
}

/// Reads a confirmed draft by its `structure` field (§0.1b). A draft without a structure is a PID loop, as the setup form writes it.
///
/// # Errors
/// The draft is not an object, its structure is not a text or not one this version knows, or its fields do not decode.
pub fn parse_setup_draft(value: &Value) -> Result<SetupDraft, Failure> {
    let Value::Object(fields) = value else {
        return Err(Failure::invalid_request(texts::draft_not_object()));
    };
    let mut fields = fields.clone();
    let structure = match fields.remove("structure") {
        None => "pid".to_string(),
        Some(Value::String(structure)) => structure,
        Some(_) => return Err(Failure::invalid_request(texts::structure_not_text())),
    };
    let body = Value::Object(fields);
    match structure.as_str() {
        "pid" => decode(&structure, body).map(SetupDraft::Pid),
        "identify" => decode(&structure, body).map(SetupDraft::Identify),
        "own-law" => decode(&structure, body).map(SetupDraft::OwnLaw),
        "trajectory-feedforward" => decode(&structure, body).map(SetupDraft::TrajectoryFeedforward),
        "feedforward" => decode(&structure, body).map(SetupDraft::Feedforward),
        other if texts::parked_label(other).is_some() => Ok(SetupDraft::Parked {
            structure: other.to_string(),
        }),
        other => Err(Failure::invalid_request(&texts::unknown_structure(other))),
    }
}

/// The loop definitions of one confirmed draft, generated for the scope and checked (B1.3, §0.1b). The project and runtime come from
/// the scope, never from the draft. Nothing is written.
///
/// # Errors
/// The structure is one the Rust engine does not run yet, a generator refuses the draft, or a generated loop fails the checks that
/// need no runtime.
pub fn plan_setup_save(
    catalog: &CompiledCatalog,
    scope: &LoopScope,
    draft: &SetupDraft,
) -> Result<Vec<PlannedLoop>, Failure> {
    let loops = match draft {
        SetupDraft::Pid(draft) => vec![pid_loop_definition(catalog, scope, draft)?],
        SetupDraft::Identify(draft) => vec![identify_loop_definition(catalog, scope, draft)?],
        SetupDraft::OwnLaw(draft) => vec![own_law_loop_definition(catalog, scope, draft)?],
        SetupDraft::TrajectoryFeedforward(draft) => {
            vec![trajectory_loop_definition(catalog, scope, draft)?]
        }
        SetupDraft::Feedforward(draft) => vec![feedforward_loop_definition(catalog, scope, draft)?],
        SetupDraft::Parked { structure } => {
            let label = texts::parked_label(structure).unwrap_or(structure.as_str());
            return Err(Failure::new(
                &texts::parked_structure(label),
                Cause::ControllerCapability,
                NextAction::None,
            ));
        }
    };
    loops
        .into_iter()
        .map(|loop_definition| plan(loop_definition, scope))
        .collect()
}

/// Checks that the planned loop files do not overwrite a file of the project, and that no planned loop id is declared by a file of
/// the project (B1.3: an existing definition is never overwritten; `assertControlLoopIdAvailable`). The refusals are checked in
/// the order the setup writes the loops, and the first refusal is returned. Nothing is written.
///
/// # Errors
/// A planned file already exists, a planned id is declared by an existing file, or an existing file cannot be read.
pub fn check_setup_targets(
    existing: &[ExistingLoopFile],
    planned: &[PlannedLoop],
) -> Result<(), Failure> {
    for planned_loop in planned {
        if existing
            .iter()
            .any(|file| file.file_name == planned_loop.file_name)
        {
            return Err(Failure::new(
                &texts::already_exists(&planned_loop.loop_definition.name),
                Cause::Settings,
                NextAction::OpenLoopSetup,
            ));
        }
        check_id_available(existing, &planned_loop.loop_definition.id)?;
    }
    Ok(())
}

/// The id of a planned loop is not declared by any loop file of the project, and no loop file is unreadable (B1.2, B1.3).
fn check_id_available(existing: &[ExistingLoopFile], id: &str) -> Result<(), Failure> {
    for file in existing {
        let name = base_name(&file.file_name);
        match &file.id {
            LoopFileId::Id(declared) if declared == id => {
                return Err(Failure::new(
                    &texts::duplicate_id(id, name),
                    Cause::Settings,
                    NextAction::OpenLoopSetup,
                ));
            }
            LoopFileId::Unreadable => {
                return Err(Failure::new(
                    &texts::repair_before_adding(name),
                    Cause::Implementation,
                    NextAction::None,
                ));
            }
            LoopFileId::Id(_) | LoopFileId::NoId => {}
        }
    }
    Ok(())
}

/// The name a rule of a loop definition has, as the save's sentence shows it (`validation_reason_code`). Each rejection maps to the
/// rule's name of the `TypeScript` discovery, so the sentences keep their wording.
#[must_use]
pub fn validation_reason_code(rejection: &LoopRejection) -> &'static str {
    match rejection {
        LoopRejection::Malformed(_) => "malformed_loop",
        LoopRejection::UnsupportedVersion(_) => "unsupported_version",
        LoopRejection::CrossRuntime => "cross_runtime_loop",
        LoopRejection::IncompatibleControllerDesign => "incompatible_controller_design",
        LoopRejection::IncompatiblePidInitialization(_) => "incompatible_pid_initialization",
        LoopRejection::IncompatibleTopology => "incompatible_topology",
        LoopRejection::InvalidSignalDirection(_) => "invalid_signal_direction",
        LoopRejection::IncompatibleSignalType(_) => "incompatible_signal_type",
        LoopRejection::IncompatibleUnits => "incompatible_units",
        LoopRejection::DuplicateSignalId(_) => "duplicate_signal_id",
        LoopRejection::DuplicateSignalRole(_) => "duplicate_signal_role",
        LoopRejection::AmbiguousSignalAddress(_, _) => "ambiguous_signal_address",
        LoopRejection::IncompatibleAuxiliaryUnits(_) => "incompatible_auxiliary_units",
    }
}

/// The planned file of one generated loop, after the checks that need no runtime (B1.4).
fn plan(loop_definition: LoopDefinition, scope: &LoopScope) -> Result<PlannedLoop, Failure> {
    if let Err(rejection) = loop_definition.validate(scope) {
        return Err(Failure::new(
            &texts::incompatible(validation_reason_code(&rejection)),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    let slug = loop_definition
        .id
        .strip_prefix("loop:")
        .unwrap_or(loop_definition.id.as_str());
    let file_name = format!("control-loops/{slug}.json");
    Ok(PlannedLoop {
        file_name,
        loop_definition,
    })
}

/// Decodes the fields of one draft structure.
fn decode<T: DeserializeOwned>(structure: &str, body: Value) -> Result<T, Failure> {
    crate::json::decode_value(body).map_err(|error| {
        Failure::invalid_request(&texts::draft_not_complete(structure, &error.to_string()))
    })
}

/// The last segment of a relative file name: the file's own name in the sentences.
fn base_name(file_name: &str) -> &str {
    file_name.rsplit('/').next().unwrap_or(file_name)
}

#[cfg(test)]
#[path = "save_tests.rs"]
mod tests;
