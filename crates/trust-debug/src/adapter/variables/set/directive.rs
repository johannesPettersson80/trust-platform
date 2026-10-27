//! Set variable directive parsing.
//! - SetDirective: write/force/release variants
//! - parse_set_directive: parse user input into directive

pub(in crate::adapter) enum SetDirective {
    Write,
    Force(String),
    Release,
}

pub(in crate::adapter) fn parse_set_directive(raw: &str) -> Result<SetDirective, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("value cannot be empty".to_string());
    }
    let lower = trimmed.to_ascii_lowercase();
    if matches!(lower.as_str(), "release" | "unforce" | "auto") {
        return Ok(SetDirective::Release);
    }
    if let Some(rest) = trimmed.strip_prefix('!') {
        let rest = rest.trim();
        if rest.is_empty() {
            return Err("force value cannot be empty".to_string());
        }
        return Ok(SetDirective::Force(rest.to_string()));
    }
    if lower.starts_with("force:") {
        let rest = trimmed[6..].trim();
        if rest.is_empty() {
            return Err("force value cannot be empty".to_string());
        }
        return Ok(SetDirective::Force(rest.to_string()));
    }
    if lower.starts_with("force ") {
        let rest = trimmed[5..].trim();
        if rest.is_empty() {
            return Err("force value cannot be empty".to_string());
        }
        return Ok(SetDirective::Force(rest.to_string()));
    }
    Ok(SetDirective::Write)
}
