//! A translation in its language file (specification 39, §14.5).

use serde_json::json;

use super::answer;

#[test]
fn a_first_translation_creates_the_language_file_in_its_canonical_form() {
    assert_eq!(
        answer(json!({ "language": "sv", "text": "Start", "translation": "Starta" })).unwrap(),
        json!({ "ok": true, "text": "format = 1\nlanguage = \"sv\"\n\n[texts]\n\"Start\" = \"Starta\"\n" })
    );
}

#[test]
fn a_translation_joins_the_others_in_key_order_and_an_empty_one_removes_the_text() {
    // §14.5 item 8: an empty translation is no translation; written as "" the display would show
    // nothing instead of falling back to the text as written.
    let current = "format = 1\nlanguage = \"de\"\n\n[texts]\n\"Stop\" = \"Halt\"\n";
    assert_eq!(
        answer(json!({ "language": "de", "text": "Start", "translation": "Starten", "current": current }))
            .unwrap()["text"],
        json!("format = 1\nlanguage = \"de\"\n\n[texts]\n\"Start\" = \"Starten\"\n\"Stop\" = \"Halt\"\n")
    );
    assert_eq!(
        answer(
            json!({ "language": "de", "text": "Stop", "translation": "   ", "current": current })
        )
        .unwrap()["text"],
        json!("format = 1\nlanguage = \"de\"\n\n[texts]\n")
    );
}

#[test]
fn a_language_that_is_no_tag_and_a_file_that_does_not_load_are_refused() {
    assert_eq!(
        answer(json!({ "language": "Swedish!", "text": "Start", "translation": "Starta" }))
            .unwrap(),
        json!({ "ok": false, "errors": [{ "message": "\"Swedish!\" is not a language tag." }] })
    );
    let refused = answer(json!({
        "language": "sv", "text": "Start", "translation": "Starta",
        "current": "format = 1\nlanguage = \"sv\"\n\n[texts]\n\"Start\" = 5\n"
    }))
    .unwrap();
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["errors"][0]["file"], json!("hmi/texts/sv.toml"));
    assert_eq!(
        refused["errors"][0]["line"],
        json!(5),
        "the problem on its line"
    );
}
