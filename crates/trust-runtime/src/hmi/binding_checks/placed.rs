//! A faceplate placed on a page (specification 39, §8.9 item 2): it is bound to a function block
//! instance of the program; its objects are bound to that instance's members as its popup's are.

use super::{problem, BindingProblem};
use crate::hmi::bundle::HmiBundle;
use crate::hmi::faceplate_fit::misfits;
use crate::hmi::member_paths::{resolve_member, MemberPath};
use crate::hmi::page_files::{HmiObject, HmiPage, PLACED_FACEPLATE};
use crate::memory::VariableStorage;
use crate::value::Value;

/// The problems of a page's placed faceplates against the faceplates and pairs of the bundle: a
/// faceplate it lacks, at the `faceplate` key, and each field the instance's block lacks, at the
/// `instance` key. An instance that is none is [`check_placed`]'s.
pub fn check_placements(
    page: &HmiPage,
    bundle: &HmiBundle,
    storage: &VariableStorage,
) -> Vec<BindingProblem> {
    let mut problems = Vec::new();
    for object in &page.objects {
        if object.kind == PLACED_FACEPLATE && object.instance.is_none() {
            if let Some(name) = &object.faceplate {
                match bundle.faceplate(name) {
                    Some(Ok(faceplate))
                        if crate::hmi::faceplate_fit::requires_instance(&faceplate) =>
                    {
                        problems.push(problem(
                            object,
                            Some("instance"),
                            "choose an equipment instance for this panel".into(),
                        ))
                    }
                    None => problems.push(problem(
                        object,
                        Some("faceplate"),
                        format!("panel {name} is missing"),
                    )),
                    _ => {}
                }
            }
            continue;
        }
        let (Some(name), Some(instance)) = (&object.faceplate, &object.instance) else {
            continue;
        };
        if object.kind != PLACED_FACEPLATE || !is_instance(instance, storage) {
            continue;
        }
        match bundle.faceplate(name) {
            None => problems.push(problem(
                object,
                Some("faceplate"),
                format!("the faceplate \"{name}\" is not in the project or its libraries"),
            )),
            // The faceplate file's own errors are reported at that file.
            Some(Err(_)) => {}
            Some(Ok(faceplate)) => problems.extend(
                misfits(&faceplate, &bundle.pairs_of(name), instance, storage)
                    .into_iter()
                    .map(|message| problem(object, Some("instance"), message)),
            ),
        }
    }
    problems
}

/// The problem with a placed faceplate's instance, at its `instance` key; none for another object.
pub(super) fn check_placed(
    object: &HmiObject,
    storage: &VariableStorage,
) -> Option<BindingProblem> {
    if object.kind != PLACED_FACEPLATE {
        return None;
    }
    let instance = object.instance.as_ref()?;
    (!is_instance(instance, storage)).then(|| {
        problem(
            object,
            Some("instance"),
            format!("\"{instance}\" is not an instance of the program"),
        )
    })
}

/// Whether a path names a function block instance of the program.
fn is_instance(instance: &str, storage: &VariableStorage) -> bool {
    let found = MemberPath::parse(instance)
        .ok()
        .and_then(|path| resolve_member(&path, storage));
    matches!(found, Some(Value::Instance(_)))
}
