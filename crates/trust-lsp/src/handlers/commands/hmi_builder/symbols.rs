//! **HMI: Import Symbol** (specification 39, §14.4 items 1 and 5): an SVG drawing becomes one of
//! the project's own symbols. The extension asks for the file and the name, copies the drawing and
//! writes the definition this command answers.
//!
//! - `suggest { title }` → `{ name }`: the name offered for a drawing titled `title` (its file name).
//! - `checkName { name }` → `{ problem }`: why `name` cannot name a symbol, or null.
//! - `definition { title, svg }` → `{ text }`: the definition to complete.

use serde::Deserialize;
use serde_json::{json, Value};
use trust_runtime::hmi::my_symbols::{is_symbol_name, starter_definition};

use super::decode;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum Args {
    Suggest { title: String },
    CheckName { name: String },
    Definition { title: String, svg: String },
}

/// The answer of `trust-lsp.hmiImportSymbol`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    match decode::<Args>(argument)? {
        Args::Suggest { title } => Ok(json!({ "ok": true, "name": suggested(&title) })),
        Args::CheckName { name } => Ok(json!({
            "ok": true,
            "problem": (!is_symbol_name(&name)).then_some("Use letters, digits, - and _"),
        })),
        Args::Definition { title, svg } => {
            Ok(json!({ "ok": true, "text": starter_definition(&title, &svg)? }))
        }
    }
}

/// The name offered for a drawing: its title in lower case, each run of other characters than
/// letters, digits, `-` and `_` a `-`, without `-` at either end; `symbol` when nothing is left.
fn suggested(title: &str) -> String {
    let mut name = String::new();
    let mut other = false;
    for c in title.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' {
            if other {
                name.push('-');
                other = false;
            }
            name.push(c);
        } else {
            other = true;
        }
    }
    if other {
        name.push('-');
    }
    let name = name.trim_matches('-');
    if name.is_empty() {
        "symbol".to_owned()
    } else {
        name.to_owned()
    }
}

#[cfg(test)]
#[path = "symbols_tests.rs"]
mod tests;
