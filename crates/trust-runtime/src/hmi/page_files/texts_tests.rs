//! Specification 39, §14.5: the project's text files, `hmi/texts/<tag>.toml`.

use super::*;

const FILE: &str = "hmi/texts/es.toml";
const ES: &str = "format = 1\nlanguage = \"es\"\n\n[texts]\n\"Running\" = \"En marcha\"\n\"Tank T-101\" = \"Tanque T-101\"\n";

#[test]
fn a_text_file_maps_each_text_as_written_to_the_text_shown() {
    let texts = parse_texts(FILE, ES).expect("the file loads");
    assert_eq!(texts.language, "es");
    assert_eq!(
        texts.texts.get("Running").map(String::as_str),
        Some("En marcha")
    );
    assert_eq!(
        texts.texts.get("Tank T-101").map(String::as_str),
        Some("Tanque T-101")
    );
}

#[test]
fn a_text_file_is_written_back_byte_for_byte() {
    let texts = parse_texts(FILE, ES).expect("the file loads");
    assert_eq!(write_texts(&texts), ES);
}

#[test]
fn a_text_file_holds_the_language_of_its_file_name() {
    let text = ES.replace("language = \"es\"", "language = \"de\"");
    let errors = parse_texts(FILE, &text).expect_err("a file of another language is refused");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].line, 2);
    assert!(errors[0].message.contains("es"), "{}", errors[0].message);
}

#[test]
fn a_text_that_is_not_text_is_reported_with_its_line() {
    let text = format!("{ES}\"Speed\" = 3\n");
    let errors = parse_texts(FILE, &text).expect_err("a number is not a text");
    assert_eq!(errors[0].line, 7, "{errors:?}");
}
