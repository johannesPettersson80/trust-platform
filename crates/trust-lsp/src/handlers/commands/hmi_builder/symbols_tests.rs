//! Import Symbol's name and definition (specification 39, §14.4 items 1 and 5).

use serde_json::json;

use super::{answer, suggested};

#[test]
fn the_offered_name_is_the_titles_letters_and_digits_joined_by_dashes() {
    // §14.4 item 1: `<name>` has letters, digits, `-` and `_`; offering the title as it is would
    // propose a name the files cannot have.
    assert_eq!(suggested("Centrifugal Pump (v2)"), "centrifugal-pump-v2");
    assert_eq!(
        suggested("Ü-Ventil"),
        "ventil",
        "letters outside a-z become dashes, trimmed"
    );
    assert_eq!(suggested("hand_valve"), "hand_valve");
    assert_eq!(suggested(" (( ))"), "symbol", "nothing left");
}

#[test]
fn a_symbol_name_has_letters_digits_dashes_and_underscores_only() {
    let problem = |name: &str| {
        answer(json!({ "action": "checkName", "name": name })).unwrap()["problem"].clone()
    };
    assert_eq!(problem("my_pump-2"), json!(null));
    assert_eq!(problem("my pump"), json!("Use letters, digits, - and _"));
    assert_eq!(
        problem(""),
        json!("Use letters, digits, - and _"),
        "an empty name is no name"
    );
}

#[test]
fn the_definition_comes_from_the_runtimes_symbol_reader() {
    let answer = answer(json!({
        "action": "definition",
        "title": "Pump",
        "svg": "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 32\"><circle id=\"rotor\"/></svg>"
    }))
    .unwrap();
    let text = answer["text"].as_str().unwrap();
    assert!(
        text.starts_with("format = 1\ntitle = \"Pump\"\nsize = [128, 76]\n"),
        "{text}"
    );
    assert!(
        text.ends_with("\n# [[part]]\n# id = \"rotor\"\n# spin = \"running\"\n"),
        "{text}"
    );
    assert!(
        super::answer(json!({ "action": "definition", "title": "Pump", "svg": "not a drawing" }))
            .is_err(),
        "a file that is no drawing is refused"
    );
}
