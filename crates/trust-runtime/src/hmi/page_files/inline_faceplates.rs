//! A panel placed on a page (specification 39, §8.9 item 3): one object of type `faceplate` that
//! names the panel and the instance it is bound to. Equipment objects name the panel they open,
//! their instance and their function block type the same way (`faceplate`, `instance`, `block`,
//! §21.1).

use super::load::{error_at, line_at, PageFileError, RawObject};

/// The type of a placed faceplate.
pub const PLACED_FACEPLATE: &str = "faceplate";

/// A placed panel names its panel, reported at its id; a `block` must be a type name, reported at
/// that key.
pub(super) fn check(
    file: &str,
    text: &str,
    objects: &[RawObject],
) -> Result<(), Vec<PageFileError>> {
    let mut errors = Vec::new();
    for object in objects {
        if object.kind == PLACED_FACEPLATE && object.faceplate.is_none() {
            errors.push(error_at(
                file,
                line_at(text, object.id.span().start),
                "a placed faceplate names its faceplate: faceplate = \"<name>\"".to_string(),
            ));
        }
        if let Some(block) = &object.block
            && !is_type_name(block.get_ref())
        {
            errors.push(error_at(
                file,
                line_at(text, block.span().start),
                format!(
                    "`{}` is not a function block type name such as `Plant.FB_Pump`",
                    block.get_ref()
                ),
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// A possibly qualified identifier: `FB_Pump` or `Plant.FB_Pump`.
fn is_type_name(text: &str) -> bool {
    !text.is_empty()
        && text.split('.').all(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}
