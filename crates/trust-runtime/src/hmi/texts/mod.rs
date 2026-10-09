//! truST's own words in the languages it ships (specification 39, §14.5), and the text table a
//! screen draws with: the project's texts for a language over truST's words for it.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::page_files::{parse_texts, HmiTexts};

mod project;

pub use project::{read_project_texts, LanguageEntry, ProjectText, ProjectTexts};

#[cfg(test)]
mod project_tests;
#[cfg(test)]
mod tests;

/// The languages truST ships its own words in, as BCP 47 tags, English first.
pub const BUILT_IN_LANGUAGES: [&str; 11] = [
    "en", "zh-Hans", "de", "es", "ja", "pt-BR", "fr", "ko", "it", "tr", "sv",
];

/// Each built-in table as shipped, keyed by the English wording of truST's words.
const TABLES: [(&str, &str); 11] = [
    ("en", include_str!("builtin/en.toml")),
    ("zh-Hans", include_str!("builtin/zh-Hans.toml")),
    ("de", include_str!("builtin/de.toml")),
    ("es", include_str!("builtin/es.toml")),
    ("ja", include_str!("builtin/ja.toml")),
    ("pt-BR", include_str!("builtin/pt-BR.toml")),
    ("fr", include_str!("builtin/fr.toml")),
    ("ko", include_str!("builtin/ko.toml")),
    ("it", include_str!("builtin/it.toml")),
    ("tr", include_str!("builtin/tr.toml")),
    ("sv", include_str!("builtin/sv.toml")),
];

fn tables() -> &'static BTreeMap<&'static str, HmiTexts> {
    static TABLES_PARSED: OnceLock<BTreeMap<&'static str, HmiTexts>> = OnceLock::new();
    TABLES_PARSED.get_or_init(|| {
        TABLES
            .iter()
            .map(|(tag, text)| {
                let parsed = parse_texts(&format!("builtin/{tag}.toml"), text)
                    .unwrap_or_else(|errors| panic!("truST's {tag} words are valid: {errors:?}"));
                (*tag, parsed)
            })
            .collect()
    })
}

/// truST's own words in a language it ships, keyed by their English wording.
pub fn builtin(language: &str) -> Option<&'static HmiTexts> {
    tables().get(language)
}

/// A language's own name, as the language button lists it; a tag truST does not know shows as
/// it is.
pub fn language_name(language: &str) -> &str {
    match language {
        "en" => "English",
        "zh-Hans" => "简体中文",
        "de" => "Deutsch",
        "es" => "Español",
        "ja" => "日本語",
        "pt-BR" => "Português (Brasil)",
        "fr" => "Français",
        "ko" => "한국어",
        "it" => "Italiano",
        "tr" => "Türkçe",
        "sv" => "Svenska",
        other => other,
    }
}

/// The table a screen draws with in a language: the project's texts over truST's words. A text
/// in neither shows as written.
pub fn merged_texts(language: &str, project: Option<&HmiTexts>) -> BTreeMap<String, String> {
    let mut texts = builtin(language)
        .map(|table| table.texts.clone())
        .unwrap_or_default();
    if let Some(project) = project.filter(|project| project.language == language) {
        texts.extend(
            project
                .texts
                .iter()
                .map(|(key, shown)| (key.clone(), shown.clone())),
        );
    }
    texts
}
