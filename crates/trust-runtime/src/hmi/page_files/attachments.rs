//! An instrument's attachment (specification 39, §6 item 5): a reference to the equipment or the
//! pipe it measures, `<object id>` or `pipe:<n>`, which must be on the same page; and a loop
//! controller's signal lines, `measures` and `acts_on`, each naming another object of the page.
//! Which objects may attach or carry signal lines, and to what, the binding checks decide with the
//! catalogue.

use super::load::{error_at, line_at, PageFileError, RawObject};

/// Every attachment of a file's objects names an object of the file other than itself, or one of
/// its `pipes` pipes.
pub(super) fn check_attachments(
    file: &str,
    text: &str,
    objects: &[RawObject],
    pipes: usize,
) -> Result<(), Vec<PageFileError>> {
    let mut errors: Vec<PageFileError> = objects
        .iter()
        .filter_map(|object| {
            let attach = object.attach.as_ref()?;
            let target = attach.get_ref();
            let message = problem(target, object.id.get_ref(), objects, pipes)?;
            Some(error_at(file, line_at(text, attach.span().start), message))
        })
        .collect();
    for object in objects {
        for (key, signal) in [("measures", &object.measures), ("acts_on", &object.acts_on)] {
            let Some(signal) = signal else { continue };
            let target = signal.get_ref();
            let message = if target == object.id.get_ref() {
                Some(format!("`{key}` names the object itself"))
            } else if objects.iter().all(|other| other.id.get_ref() != target) {
                Some(format!(
                    "`{key}` names `{target}`, which is not on this page"
                ))
            } else {
                None
            };
            if let Some(message) = message {
                errors.push(error_at(file, line_at(text, signal.span().start), message));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn problem(target: &str, own: &str, objects: &[RawObject], pipes: usize) -> Option<String> {
    if let Some(number) = target.strip_prefix("pipe:") {
        return match number.parse::<usize>() {
            Err(_) => Some(format!(
                "attaches to `{target}`; a pipe is written `pipe:<n>`, counting from 0"
            )),
            Ok(index) if index >= pipes => Some(format!(
                "attaches to pipe {index}, but the page has {pipes} pipe{}",
                if pipes == 1 { "" } else { "s" }
            )),
            Ok(_) => None,
        };
    }
    if target == own {
        return Some("an object cannot attach to itself".to_string());
    }
    if objects.iter().all(|object| object.id.get_ref() != target) {
        return Some(format!("attaches to `{target}`, which is not on this page"));
    }
    None
}
