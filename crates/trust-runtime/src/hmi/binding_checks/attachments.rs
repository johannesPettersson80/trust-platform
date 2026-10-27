//! A page's attachments against the catalogue (specification 39, §6 item 5): only an instrument
//! attaches, and only to equipment that is no instrument itself, or to a pipe; only a loop
//! controller has signal lines, from the instrument it `measures` and to the equipment it
//! `acts_on`. That the target is on the page, the file check already ensures.

use super::BindingProblem;
use crate::hmi::catalogue::{Catalogue, ComponentCategory};
use crate::hmi::page_files::HmiPage;

pub(super) fn check_attachments(page: &HmiPage, catalogue: &Catalogue) -> Vec<BindingProblem> {
    let mut problems = attachment_problems(page, catalogue);
    problems.extend(signal_problems(page, catalogue));
    problems
}

/// The type of the loop controller, the one component with signal lines.
const CONTROLLER: &str = "controller";

fn signal_problems(page: &HmiPage, catalogue: &Catalogue) -> Vec<BindingProblem> {
    let mut problems = Vec::new();
    for object in &page.objects {
        for (key, target) in [
            ("measures", object.measures.as_deref()),
            ("acts_on", object.acts_on.as_deref()),
        ] {
            let Some(target) = target else { continue };
            let Some(kind) = page
                .objects
                .iter()
                .find(|candidate| candidate.id == target)
                .map(|candidate| candidate.kind.as_str())
            else {
                continue;
            };
            let definition = catalogue.component(kind);
            let message = if object.kind != CONTROLLER {
                Some(format!(
                    "{} has no signal lines; a loop controller has",
                    named(&object.kind)
                ))
            } else if key == "measures" && !definition.is_some_and(|component| component.attaches) {
                Some(format!(
                    "a controller measures an instrument, not {}",
                    named(kind)
                ))
            } else if key == "acts_on"
                && !definition.is_some_and(|component| {
                    component.category == ComponentCategory::Equipment && !component.attaches
                })
                // A project's own symbol stands for equipment too (§14.4).
                && !kind.starts_with("my:")
            {
                Some(format!(
                    "a controller acts on equipment, not {}",
                    named(kind)
                ))
            } else {
                None
            };
            if let Some(message) = message {
                problems.push(BindingProblem {
                    object: object.id.clone(),
                    property: Some(key.to_string()),
                    message,
                    point: None,
                });
            }
        }
    }
    problems
}

fn attachment_problems(page: &HmiPage, catalogue: &Catalogue) -> Vec<BindingProblem> {
    page.objects
        .iter()
        .filter_map(|object| {
            let target = object.attach.as_deref()?;
            let component = catalogue.component(&object.kind)?;
            let message = if !component.attaches {
                format!(
                    "{} does not attach to equipment; instruments do",
                    named(&object.kind)
                )
            } else if target.starts_with("pipe:") {
                return None;
            } else {
                let kind = &page
                    .objects
                    .iter()
                    .find(|candidate| candidate.id == target)?
                    .kind;
                // Equipment that does not itself attach: an instrument measures equipment, not
                // another instrument.
                if catalogue.component(kind).is_some_and(|definition| {
                    definition.category == ComponentCategory::Equipment && !definition.attaches
                }) {
                    return None;
                }
                format!(
                    "an instrument attaches to equipment or a pipe, not to {}",
                    named(kind)
                )
            };
            Some(BindingProblem {
                object: object.id.clone(),
                property: Some("attach".to_string()),
                message,
                point: None,
            })
        })
        .collect()
}

/// A component type as words with its article: "a value", "an alarm list".
fn named(kind: &str) -> String {
    let words = kind.replace('-', " ");
    let article = if words.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    format!("{article} {words}")
}
