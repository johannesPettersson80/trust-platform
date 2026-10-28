//! A page's place in the navigation (specification 39, §4.6): its order, icon, group and whether
//! it is hidden.

use toml::Spanned;

use super::load::{error_at, line_at, PageFileError};

/// The navigation icons truST ships.
pub const ICONS: [&str; 14] = [
    "overview", "process", "trend", "alarm", "settings", "pump", "valve", "tank", "motor", "heat",
    "flow", "power", "list", "info",
];

/// The P&ID a page shares its process objects with is named as its file is, `hmi/pid/<name>.toml`
/// (§18 phase 4).
pub(super) fn check_pid(
    file: &str,
    text: &str,
    pid: Option<&Spanned<String>>,
) -> Result<(), Vec<PageFileError>> {
    let named = |name: &str| {
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    match pid.filter(|pid| !named(pid.get_ref())) {
        Some(pid) => Err(vec![error_at(
            file,
            line_at(text, pid.span().start),
            format!(
                "pid names a P&ID of hmi/pid/ by its file name, letters, digits, - and _, not \"{}\"",
                pid.get_ref()
            ),
        )]),
        None => Ok(()),
    }
}

/// The order is a whole number from 0, the icon a shipped one and the group a name; each
/// problem is reported on its line.
pub(super) fn check(
    file: &str,
    text: &str,
    order: Option<&Spanned<i64>>,
    icon: Option<&Spanned<String>>,
    group: Option<&Spanned<String>>,
) -> Result<(), Vec<PageFileError>> {
    let mut errors = Vec::new();
    if let Some(order) = order.filter(|order| u32::try_from(*order.get_ref()).is_err()) {
        errors.push(error_at(
            file,
            line_at(text, order.span().start),
            "order must be a whole number from 0".to_string(),
        ));
    }
    if let Some(icon) = icon.filter(|icon| !ICONS.contains(&icon.get_ref().as_str())) {
        errors.push(error_at(
            file,
            line_at(text, icon.span().start),
            format!(
                "icon must be one of {}, not \"{}\"",
                ICONS.join(", "),
                icon.get_ref()
            ),
        ));
    }
    if let Some(group) = group.filter(|group| group.get_ref().trim().is_empty()) {
        errors.push(error_at(
            file,
            line_at(text, group.span().start),
            "group must name a group".to_string(),
        ));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
