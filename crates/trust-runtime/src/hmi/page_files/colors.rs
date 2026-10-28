//! Project colours (specification 39, §14.1): the colour tokens a project may set.

/// Names of the colour tokens, the same as the drawing module's (a VS Code test checks both).
pub const COLOR_TOKENS: &[&str] = &[
    "page",
    "surface",
    "raised",
    "border",
    "line",
    "text",
    "muted",
    "accent",
    "accent-2",
    "accent-3",
    "pen-1",
    "pen-2",
    "pen-3",
    "pen-4",
    "unknown",
    "stale",
    "pending",
    "warning",
    "alarm",
    "alarm-high",
    "alarm-medium",
    "alarm-low",
];

/// Whether a value is a hex colour: `#rgb`, `#rrggbb` or `#rrggbbaa`.
pub fn is_hex_colour(value: &str) -> bool {
    value.strip_prefix('#').is_some_and(|digits| {
        matches!(digits.len(), 3 | 6 | 8) && digits.chars().all(|c| c.is_ascii_hexdigit())
    })
}
