//! The control endpoint of a project's `runtime.toml` (specification 35, §0.1b; runtime protocol
//! annex, E0.1). No TOML parser is a dependency of this crate, so this reads the two spellings that
//! the project files use: `endpoint` in the `[runtime.control]` table, and the dotted key
//! `runtime.control.endpoint` at the top of the file. A file that spells the endpoint another way
//! counts as configuring none, and the command line says so.

/// The endpoint that the text of a `runtime.toml` configures, when it is a string.
#[must_use]
pub fn control_endpoint(text: &str) -> Option<String> {
    let mut section = "";
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            section = name.trim();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let is_endpoint = matches!(
            (section, key.trim()),
            ("runtime.control", "endpoint") | ("", "runtime.control.endpoint")
        );
        if is_endpoint {
            return string_value(value);
        }
    }
    None
}

/// The line without its comment: everything from a `#` that is not inside a string.
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    for (index, character) in line.char_indices() {
        match (quote, character) {
            (None, '#') => return &line[..index],
            (None, '"' | '\'') => quote = Some(character),
            (Some(open), close) if open == close => quote = None,
            _ => {}
        }
    }
    line
}

/// The text of a TOML string: a basic string without escapes, or a literal string.
fn string_value(value: &str) -> Option<String> {
    let value = value.trim();
    let inner = value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|rest| rest.strip_suffix('\''))
        })?;
    (!inner.is_empty() && !inner.contains(['"', '\\'])).then(|| inner.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_endpoint_of_the_control_table_or_the_dotted_key() {
        assert_eq!(
            control_endpoint(
                "[resource]\nname = \"Res\"\n\n[runtime.control]\nendpoint = \"tcp://127.0.0.1:5195\" # the port\nmode = \"debug\"\n"
            ),
            Some("tcp://127.0.0.1:5195".to_string())
        );
        assert_eq!(
            control_endpoint("runtime.control.endpoint = 'unix:///run/trust.sock'\n"),
            Some("unix:///run/trust.sock".to_string())
        );
    }

    #[test]
    fn another_table_or_another_spelling_configures_no_endpoint() {
        assert_eq!(
            control_endpoint("[runtime.log]\nendpoint = \"tcp://127.0.0.1:1\"\n"),
            None
        );
        assert_eq!(
            control_endpoint("[runtime]\ncontrol.endpoint = \"tcp://127.0.0.1:1\"\n"),
            None,
            "a dotted key inside a table is not read"
        );
        assert_eq!(
            control_endpoint("[runtime.control]\nendpoint = 5195\n"),
            None,
            "a number is not an endpoint"
        );
        assert_eq!(
            control_endpoint("[runtime.control]\nendpoint = \"a\\\\b\"\n"),
            None,
            "an escape is not read"
        );
    }
}
