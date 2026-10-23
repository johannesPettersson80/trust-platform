//! Parsing of optional register-executor configuration values.

use core::str::FromStr;

pub fn parse_bool(value: Option<&str>, default: bool) -> bool {
    match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("1" | "true" | "yes" | "on") => true,
        Some("0" | "false" | "no" | "off") => false,
        _ => default,
    }
}

pub fn parse_number<T: FromStr>(value: Option<&str>, default: T) -> T {
    value
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(default)
}
