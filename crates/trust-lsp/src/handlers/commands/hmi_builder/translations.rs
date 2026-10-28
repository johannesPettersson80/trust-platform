//! A text's translation, written to its language's text file (specification 39, §14.5 items 2, 7
//! and 8): `hmi/texts/<language>.toml` in its canonical form, the file created when the language
//! has none; an empty translation removes the text. The extension passes the file's current text
//! (an open editor's, saved or not) and writes the answer.
//!
//! `{ language, text, translation, current? }` → `{ text }`, or `{ ok: false, errors }` with the
//! file's problems on their lines when it cannot be written.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{json, Value};
use trust_runtime::hmi::page_files::{
    is_language_tag, parse_texts, write_texts, HmiTexts, PageFileError,
};

use super::decode;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    /// The language's tag.
    language: String,
    /// The text as written in the project's first language.
    text: String,
    /// What it is in `language`; empty to remove it.
    translation: String,
    /// The language file's text; none when the file does not exist.
    #[serde(default)]
    current: Option<String>,
}

/// The answer of `trust-lsp.hmiTranslation`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    let args: Args = decode(argument)?;
    Ok(match translated(&args) {
        Ok(text) => json!({ "ok": true, "text": text }),
        Err(errors) => json!({ "ok": false, "errors": errors }),
    })
}

/// The language file's new text, or why it cannot be written.
fn translated(args: &Args) -> Result<String, Vec<Value>> {
    if !is_language_tag(&args.language) {
        return Err(vec![
            json!({ "message": format!("\"{}\" is not a language tag.", args.language) }),
        ]);
    }
    let file = format!("hmi/texts/{}.toml", args.language);
    let mut texts = match &args.current {
        Some(current) => parse_texts(&file, current).map_err(|errors| problems(&errors))?,
        None => HmiTexts {
            language: args.language.clone(),
            texts: BTreeMap::new(),
        },
    };
    if args.translation.trim().is_empty() {
        texts.texts.remove(&args.text);
    } else {
        texts
            .texts
            .insert(args.text.clone(), args.translation.clone());
    }
    let next = write_texts(&texts);
    // What is written loads.
    parse_texts(&file, &next).map_err(|errors| problems(&errors))?;
    Ok(next)
}

fn problems(errors: &[PageFileError]) -> Vec<Value> {
    errors
        .iter()
        .map(|error| json!({ "file": error.file, "line": error.line, "message": error.message }))
        .collect()
}

#[cfg(test)]
#[path = "translations_tests.rs"]
mod tests;
