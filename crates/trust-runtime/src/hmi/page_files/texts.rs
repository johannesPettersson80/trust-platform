//! The project's text files (specification 39, §14.5): `hmi/texts/<tag>.toml`, each text as
//! written to the text shown in one language.

use std::collections::BTreeMap;

use serde::Deserialize;
use toml::Spanned;

use super::load::{check_format, error_at, line_at, toml_error, PageFileError};
use super::types::HmiTexts;
use super::write::string;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTexts {
    #[allow(dead_code)]
    format: i64,
    language: Spanned<String>,
    #[serde(default)]
    texts: BTreeMap<String, Spanned<toml::Value>>,
}

/// Parses and validates one text file. Its `language` is the tag of its file name.
pub fn parse_texts(file: &str, text: &str) -> Result<HmiTexts, Vec<PageFileError>> {
    check_format(file, text)?;
    let raw: RawTexts =
        toml::from_str(text).map_err(|error| vec![toml_error(file, text, &error)])?;
    let language = raw.language.get_ref().clone();
    let mut errors = Vec::new();
    let language_line = line_at(text, raw.language.span().start);
    let named = file
        .rsplit(['/', '\\'])
        .next()
        .and_then(|name| name.strip_suffix(".toml"));
    if !is_language_tag(&language) {
        errors.push(error_at(file, language_line, not_a_tag(&language)));
    } else if let Some(named) = named.filter(|named| *named != language) {
        errors.push(error_at(
            file,
            language_line,
            format!("this file holds the texts in `{named}`, so its language is \"{named}\", not \"{language}\""),
        ));
    }
    let mut texts = BTreeMap::new();
    for (key, value) in raw.texts {
        match value.get_ref() {
            toml::Value::String(shown) => {
                texts.insert(key, shown.clone());
            }
            _ => errors.push(error_at(
                file,
                line_at(text, value.span().start),
                format!("the text for \"{key}\" must be text in quotes"),
            )),
        }
    }
    if errors.is_empty() {
        Ok(HmiTexts { language, texts })
    } else {
        Err(errors)
    }
}

/// Writes a text file in its canonical form: the texts in the order of their keys.
pub fn write_texts(texts: &HmiTexts) -> String {
    let mut out = format!(
        "format = 1\nlanguage = {}\n\n[texts]\n",
        string(&texts.language)
    );
    for (key, shown) in &texts.texts {
        out.push_str(&format!("{} = {}\n", string(key), string(shown)));
    }
    out
}

/// Whether a text is a BCP 47 language tag in the shape the HMI takes: a two- or three-letter
/// language, then subtags of two to eight letters or digits, such as `sv`, `pt-BR` or `zh-Hans`.
pub fn is_language_tag(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let language_ok = parts.next().is_some_and(|language| {
        (2..=3).contains(&language.len()) && language.bytes().all(|byte| byte.is_ascii_alphabetic())
    });
    language_ok
        && parts.all(|part| {
            (2..=8).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

/// The message for a language that is no tag.
pub(super) fn not_a_tag(language: &str) -> String {
    format!("`{language}` is not a language tag; write a BCP 47 tag such as \"sv\", \"pt-BR\" or \"zh-Hans\"")
}
