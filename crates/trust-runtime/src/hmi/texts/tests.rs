//! Specification 39, §14.5: truST's own words in 11 languages, and a project's texts over them.

use super::*;

/// The values a text names in braces, such as `{minutes}`, in order.
fn placeholders(text: &str) -> Vec<&str> {
    text.match_indices('{')
        .filter_map(|(start, _)| {
            text[start..]
                .find('}')
                .map(|end| &text[start..start + end + 1])
        })
        .collect()
}

#[test]
fn the_eleven_languages_are_built_in_with_their_own_names() {
    assert_eq!(
        BUILT_IN_LANGUAGES.as_slice(),
        ["en", "zh-Hans", "de", "es", "ja", "pt-BR", "fr", "ko", "it", "tr", "sv"]
    );
    let names: Vec<&str> = BUILT_IN_LANGUAGES
        .iter()
        .map(|tag| language_name(tag))
        .collect();
    assert_eq!(
        names,
        [
            "English",
            "简体中文",
            "Deutsch",
            "Español",
            "日本語",
            "Português (Brasil)",
            "Français",
            "한국어",
            "Italiano",
            "Türkçe",
            "Svenska"
        ]
    );
    assert_eq!(language_name("fi"), "fi");
}

#[test]
fn every_built_in_language_has_every_word_with_its_values() {
    let english = builtin("en").expect("English is built in");
    assert!(english.texts.len() >= 100, "{} words", english.texts.len());
    for (key, shown) in &english.texts {
        assert_eq!(key, shown, "English shows each word as it is keyed");
    }
    for tag in BUILT_IN_LANGUAGES {
        let table = builtin(tag).unwrap_or_else(|| panic!("{tag} is built in"));
        assert_eq!(table.language, tag);
        let keys: Vec<&String> = table.texts.keys().collect();
        let english_keys: Vec<&String> = english.texts.keys().collect();
        assert_eq!(keys, english_keys, "{tag} has every word and no other");
        for (key, shown) in &table.texts {
            assert!(!shown.trim().is_empty(), "{tag}: {key} is empty");
            let mut wanted = placeholders(key);
            let mut given = placeholders(shown);
            wanted.sort_unstable();
            given.sort_unstable();
            assert_eq!(given, wanted, "{tag}: {key} → {shown} keeps its values");
        }
    }
}

#[test]
fn equipment_navigation_has_words_in_every_built_in_language() {
    for tag in BUILT_IN_LANGUAGES {
        let table = builtin(tag).unwrap();
        for word in [
            "Show equipment",
            "Finding equipment…",
            "Equipment is not shown on a page.",
            "Unable to find equipment.",
        ] {
            assert!(
                table.texts.get(word).is_some_and(|text| !text.is_empty()),
                "{tag} lacks {word}"
            );
        }
    }
}

#[test]
fn a_project_text_replaces_the_built_in_word_and_adds_its_own() {
    let project = HmiTexts {
        language: "sv".to_string(),
        texts: [
            ("Running".to_string(), "Pumpar".to_string()),
            ("Tank T-101".to_string(), "Tank T-101 norr".to_string()),
        ]
        .into(),
    };
    let swedish = merged_texts("sv", Some(&project));
    assert_eq!(swedish.get("Running").map(String::as_str), Some("Pumpar"));
    assert_eq!(
        swedish.get("Tank T-101").map(String::as_str),
        Some("Tank T-101 norr")
    );
    assert_eq!(
        swedish.get("Stopped").map(String::as_str),
        builtin("sv")
            .and_then(|table| table.texts.get("Stopped"))
            .map(String::as_str)
    );
    assert_ne!(
        swedish.get("Stopped").map(String::as_str),
        Some("Stopped"),
        "truST's Swedish word"
    );
    // A language truST does not ship has only the project's texts; the rest shows as written.
    assert!(merged_texts("fi", None).is_empty());
}
