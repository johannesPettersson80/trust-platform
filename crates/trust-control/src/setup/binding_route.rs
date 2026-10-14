//! The rules of the three-page binding route (specification 35, B1.2, B1.3 and C0; `controlStudio/threePageBindingRoute.ts`): the
//! shape of a binding selection, the controller family it asks for, and the project that holds the loop. The message protocol, its
//! replies, the session and the adoption of a study model belong to the extension and the engine; they are not ported here.

use serde_json::Value;

use super::discovery::{
    select_binding, BindingSelection, BindingTarget, ControllerCompatibility, Discovery,
    LoopFileRead, LoopFileText, MAXIMUM_SELECTED_CHANNELS,
};
use super::texts::discovery as texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::json::parse_text;
use crate::loop_definition::LoopDefinition;

/// The controller families the setup names (`CONTROLLER_FAMILIES`, `controllerFamilies.ts`). Only the structures the adapters realize
/// can be chosen; the others are named so that the choice is refused, not misread.
pub const CONTROLLER_FAMILIES: [&str; 23] = [
    "ON/OFF",
    "state-feedback",
    "LQR",
    "LQG",
    "P",
    "PI",
    "PD",
    "PID",
    "cascade",
    "feedforward",
    "ratio",
    "split-range",
    "override",
    "Smith",
    "paired-PI",
    "decoupled-PI",
    "gain-scheduled",
    "MPC",
    "adaptive",
    "robust",
    "feedback-linearization",
    "observer-feedback",
    "lead-lag",
];

/// Whether a text names one of the controller families (`isControllerFamily`).
#[must_use]
pub fn is_controller_family(family: &str) -> bool {
    CONTROLLER_FAMILIES.contains(&family)
}

/// The shape of a binding selection (`selection` in `threePageBindingRoute.ts`): one to sixteen identities on each side, each a
/// non-empty text. A chosen family, when there is one, must be a controller family (`payload.family`).
///
/// # Errors
/// A side names no identity, more than sixteen, or an empty one, or the family is not a controller family.
pub fn check_selection(selection: &BindingSelection, family: Option<&str>) -> Result<(), Failure> {
    let valid = |ids: &[String]| {
        (1..=MAXIMUM_SELECTED_CHANNELS).contains(&ids.len()) && ids.iter().all(|id| !id.is_empty())
    };
    let family_known = family.is_none_or(is_controller_family);
    if valid(&selection.input_ids) && valid(&selection.output_ids) && family_known {
        Ok(())
    } else {
        Err(Failure::new(
            texts::selection_shape(),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ))
    }
}

/// The selected loop's adapter realizes the controller family the selector chose (C0, `threePageBindingRoute.ts`). `discovery` is the
/// one the selector lists, already in scope.
///
/// # Errors
/// The selected loop is not in the discovery, or its adapter does not realize the family.
pub fn check_family(
    discovery: &Discovery,
    selection: &BindingSelection,
    family: &str,
) -> Result<(), Failure> {
    let compatibility = discovery
        .targets
        .iter()
        .find(|target| target.loop_definition.id == selection.loop_id)
        .map(|target| &target.controller_compatibility);
    match compatibility {
        Some(ControllerCompatibility::Accepted { families })
            if families.iter().any(|known| known == family) =>
        {
            Ok(())
        }
        _ => Err(Failure::new(
            texts::family_not_supported(),
            Cause::ControllerCapability,
            NextAction::ChooseMethod,
        )),
    }
}

/// The target a binding selection names, once the engine has checked it (B1.3, C0): the selection's shape, the chosen family, the loop
/// the discovery lists, the controller it runs in and its declared channels. `discovery` is the one the selector lists.
///
/// # Errors
/// The selection's shape or the family is refused (`check_selection`); no listed loop has the selection's id, the controller is
/// another one, or a side names no channel, a partial set, an undeclared channel or one channel twice (`select_binding`); or the
/// loop's adapter does not realize the chosen family (`check_family`).
pub fn answer_selection<'a>(
    discovery: &'a Discovery,
    selection: &BindingSelection,
    family: Option<&str>,
) -> Result<&'a BindingTarget, Failure> {
    check_selection(selection, family)?;
    let target = select_binding(discovery, selection)?;
    if let Some(family) = family {
        check_family(discovery, selection, family)?;
    }
    Ok(target)
}

/// The loop files of one workspace project (the multi-root rule of `threePageBindingRoute.ts`).
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectLoopFiles {
    /// The project's root folder.
    pub root: String,
    /// Its loop files, as the engine read them.
    pub files: Vec<LoopFileText>,
}

/// The project that holds the loop (`project` in `threePageBindingRoute.ts`): the only project when there is one; otherwise the one
/// project whose loop files declare the loop's id, project and runtime.
///
/// # Errors
/// No project, or more than one project, declares the loop.
pub fn identify_project<'a>(
    loop_definition: &LoopDefinition,
    projects: &'a [ProjectLoopFiles],
) -> Result<&'a ProjectLoopFiles, Failure> {
    if let [only] = projects {
        return Ok(only);
    }
    let mut matches = projects.iter().filter(|project| {
        project
            .files
            .iter()
            .any(|file| declares_loop(file, loop_definition))
    });
    match (matches.next(), matches.next()) {
        (Some(only), None) => Ok(only),
        _ => Err(Failure::new(
            texts::project_not_identified(),
            Cause::Settings,
            NextAction::None,
        )),
    }
}

/// Whether one loop file declares the loop's id, project and runtime. A file that is too large, unreadable or not JSON declares
/// nothing: the discovery reports it.
fn declares_loop(file: &LoopFileText, loop_definition: &LoopDefinition) -> bool {
    let LoopFileRead::Text(text) = &file.read else {
        return false;
    };
    let Ok(value) = parse_text(text) else {
        return false;
    };
    let field = |key: &str| value.get(key).and_then(Value::as_str);
    field("id") == Some(loop_definition.id.as_str())
        && field("projectId") == Some(loop_definition.project_id.as_str())
        && field("runtimeId") == Some(loop_definition.runtime_id.as_str())
}

#[cfg(test)]
#[path = "binding_route_tests.rs"]
mod tests;
