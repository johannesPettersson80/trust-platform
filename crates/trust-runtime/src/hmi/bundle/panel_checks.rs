//! Validate the actual bound panels a page can open, including commands and settings.
use super::{
    checks::{BundleProblem, Program},
    HmiBundle,
};
use crate::hmi::{
    access::{config::WriteRule, page_policy::check_page_policy},
    binding_checks::check_page_with_remote,
    catalogue::Catalogue,
    faceplate_fit::{bind_fields, choose_pair, common_instance, requires_instance},
    member_paths::{resolve_member, MemberPath},
    page_files::{click_line, object_key_line, HmiClick, HmiPage, PLACED_FACEPLATE},
};
use crate::value::Value;
use std::collections::BTreeMap;

#[allow(clippy::too_many_arguments)]
pub(super) fn check(
    bundle: &HmiBundle,
    file: &str,
    text: &str,
    page: &HmiPage,
    catalogue: &Catalogue,
    program: &Program<'_>,
    policy: &[WriteRule],
    remote: &BTreeMap<String, serde_json::Value>,
) -> Vec<BundleProblem> {
    let mut problems = Vec::new();
    for object in &page.objects {
        let target = if object.kind == PLACED_FACEPLATE {
            object
                .faceplate
                .clone()
                .map(|name| (name, object.instance.clone()))
        } else {
            match object.effective_click() {
                Some(HmiClick::Faceplate { name, instance }) => {
                    Some((name, instance.or_else(|| common_instance(object))))
                }
                _ => None,
            }
        };
        let Some((name, instance)) = target else {
            continue;
        };
        let mut report = |message: String| {
            problems.push(BundleProblem {
                file: file.into(),
                line: click_line(text, &object.id),
                message: format!("{} / {name}: {message}", object.id),
            })
        };
        let panel = match bundle.faceplate(&name) {
            Some(Ok(panel)) => panel,
            Some(Err(errors)) => {
                for error in errors {
                    report(error.message);
                }
                continue;
            }
            None => {
                report("panel definition is missing".into());
                continue;
            }
        };
        if panel
            .popup
            .page
            .objects
            .iter()
            .any(|object| object.kind == PLACED_FACEPLATE)
        {
            report("a reusable panel cannot contain another placed panel".into());
            continue;
        }
        let panel = if let Some(instance) = instance {
            let Some(Value::Instance(id)) = MemberPath::parse(&instance)
                .ok()
                .and_then(|path| resolve_member(&path, program.storage))
            else {
                report("selected equipment instance is unavailable".into());
                continue;
            };
            let Some(instance_data) = program.storage.get_instance(*id) else {
                report("selected equipment instance is unavailable".into());
                continue;
            };
            let pairs = bundle.pairs_of(&name);
            let members = choose_pair(&pairs, Some(&instance_data.type_name))
                .map(|pair| pair.fields.clone())
                .unwrap_or_default();
            bind_fields(panel, &instance, &members)
        } else {
            if requires_instance(&panel) {
                report("choose an equipment instance".into());
                continue;
            }
            panel
        };
        for problem in check_page_with_remote(
            &panel.popup.page,
            catalogue,
            program.metadata,
            program.storage,
            remote,
        )
        .into_iter()
        .chain(check_page_policy(&panel.popup.page, catalogue, policy))
        {
            report(format!("{}: {}", problem.object, problem.message));
        }
    }
    problems
}

/// An equipment object's `block` is the type of the instance it shows, when that instance is
/// declared (specification 39, §21.1 item 2). An instance not declared yet is the design's intent
/// for Create PLC code, not a problem.
pub(super) fn check_blocks(
    file: &str,
    text: &str,
    page: &HmiPage,
    program: &Program<'_>,
) -> Vec<BundleProblem> {
    let mut problems = Vec::new();
    for object in &page.objects {
        let (Some(block), Some(instance)) = (&object.block, &object.instance) else {
            continue;
        };
        let Some(Value::Instance(id)) = MemberPath::parse(instance)
            .ok()
            .and_then(|path| resolve_member(&path, program.storage))
        else {
            continue;
        };
        let Some(declared) = program.storage.get_instance(*id) else {
            continue;
        };
        if !declared.type_name.eq_ignore_ascii_case(block) {
            problems.push(BundleProblem {
                file: file.into(),
                line: object_key_line(text, &object.id, "block"),
                message: format!(
                    "{} is a {}, not a {block}: change the block or choose another instance",
                    instance, declared.type_name
                ),
            });
        }
    }
    problems
}
