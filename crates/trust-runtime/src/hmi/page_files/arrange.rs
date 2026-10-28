//! How a panel or group rearranges its objects on a smaller screen (specification 39, §4 item 5,
//! §5.1): `arrange` and `gap`.

use super::load::{error_at, line_at, PageFileError, RawObject};
use super::types::HmiArrange;

/// An arrangement by its name.
pub(super) fn arrange(text: &str) -> Option<HmiArrange> {
    match text {
        "row" => Some(HmiArrange::Row),
        "column" => Some(HmiArrange::Column),
        "grid" => Some(HmiArrange::Grid),
        _ => None,
    }
}

/// `arrange` names one of the three, and both keys belong to panels and groups.
pub(super) fn check(
    file: &str,
    text: &str,
    objects: &[RawObject],
) -> Result<(), Vec<PageFileError>> {
    let mut errors = Vec::new();
    for object in objects {
        let container = matches!(object.kind.as_str(), "panel" | "group");
        if let Some(value) = &object.arrange {
            let message = if !container {
                Some("arrange is for panels and groups".to_string())
            } else if arrange(value.get_ref()).is_none() {
                Some(format!(
                    "arrange must be row, column or grid, not \"{}\"",
                    value.get_ref()
                ))
            } else {
                None
            };
            if let Some(message) = message {
                errors.push(error_at(file, line_at(text, value.span().start), message));
            }
        }
        if let (Some(gap), false) = (&object.gap, container) {
            errors.push(error_at(
                file,
                line_at(text, gap.span().start),
                "gap is for panels and groups".to_string(),
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
