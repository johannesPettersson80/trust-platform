//! A faceplate's bindings (specification 39, §8.7 item 1): a binding to a field, `$Name`, is
//! checked as the field's kinds against the property it binds, since the instance is known only
//! when the faceplate opens; every other binding is checked against the program as a page's is.

use super::{check_page, BindingProblem};
use crate::hmi::catalogue::{Catalogue, ValueKind};
use crate::hmi::page_files::{HmiFaceplate, HmiPage};
use crate::memory::VariableStorage;
use crate::runtime::RuntimeMetadata;

/// The problems of a faceplate's objects, in their order on the page.
pub fn check_faceplate(
    faceplate: &HmiFaceplate,
    catalogue: &Catalogue,
    metadata: &RuntimeMetadata,
    storage: &VariableStorage,
) -> Vec<BindingProblem> {
    let page = &faceplate.popup.page;
    let mut problems = field_problems(faceplate, catalogue);
    problems.extend(check_page(
        &without_fields(page),
        catalogue,
        metadata,
        storage,
    ));
    let place = |object: &str| {
        page.objects
            .iter()
            .position(|candidate| candidate.id == object)
            .unwrap_or(usize::MAX)
    };
    problems.sort_by_key(|problem| place(&problem.object));
    problems
}

/// The page without its bindings to fields, for the checks that need the program or a policy.
pub fn without_fields(page: &HmiPage) -> HmiPage {
    let mut page = page.clone();
    for object in &mut page.objects {
        if object
            .command
            .as_ref()
            .is_some_and(|command| command.names_panel_instance())
        {
            object.command = None;
        }
        for binding in object.bind.values_mut() {
            if binding
                .quality
                .as_ref()
                .is_some_and(|path| path.starts_with('$'))
            {
                binding.quality = None;
            }
        }
        object
            .bind
            .retain(|_, binding| !binding.path.starts_with('$'));
    }
    for pipe in &mut page.pipes {
        pipe.bind
            .retain(|_, binding| !binding.path.starts_with('$'));
    }
    page
}

/// Check field-to-widget type compatibility without PLC instances or executable state.
pub fn field_problems(faceplate: &HmiFaceplate, catalogue: &Catalogue) -> Vec<BindingProblem> {
    let mut problems = Vec::new();
    for object in &faceplate.popup.page.objects {
        let Some(component) = catalogue.component(&object.kind) else {
            continue;
        };
        for (property, binding) in &object.bind {
            let Some(name) = binding.path.strip_prefix('$') else {
                continue;
            };
            let Some(field) = faceplate.fields.get(name) else {
                continue;
            };
            let Some(rule) = component.property(property) else {
                continue;
            };
            let taken: Vec<ValueKind> = field
                .accepts
                .iter()
                .filter_map(|kind| {
                    serde_json::from_value(serde_json::Value::String(kind.clone())).ok()
                })
                .collect();
            if taken.is_empty()
                || taken
                    .iter()
                    .any(|kind| rule.accepts.iter().any(|accepted| fits(*kind, *accepted)))
            {
                continue;
            }
            let takes = taken
                .iter()
                .map(|kind| kind.words())
                .collect::<Vec<_>>()
                .join(" or ");
            let needs = rule
                .accepts
                .iter()
                .map(|kind| kind.words())
                .collect::<Vec<_>>()
                .join(" or ");
            problems.push(BindingProblem {
                object: object.id.clone(),
                property: Some(property.clone()),
                message: format!(
                    "the field `{name}` takes {takes}; a {}'s {property} needs {needs}",
                    object.kind.replace('-', " ")
                ),
                point: None,
            });
        }
    }
    problems
}

/// Whether a field of one kind can bind a property accepting another.
fn fits(field: ValueKind, accepted: ValueKind) -> bool {
    field == accepted
        || (accepted == ValueKind::Value
            && !matches!(field, ValueKind::Structure | ValueKind::Array))
        || (field == ValueKind::Value
            && !matches!(accepted, ValueKind::Structure | ValueKind::Array))
}
