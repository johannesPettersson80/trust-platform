//! Project colours chosen and reset in `hmi/screen.toml` (specification 39, §14.1 item 2), asked
//! through `trust-lsp.hmiScreenColour` as the editor asks.

use serde_json::{json, Value};

use super::super::{execute, HMI_SCREEN_COLOUR_COMMAND};

/// The screen example of specification 39, §15.3.
const SCREEN_EXAMPLE: &str = r##"format = 1
size = [1920, 1080]
theme = "light"
symbols = "flat"

[top_bar]
title = "Feed station"
clock = true
alarms = true

[navigation]
style = "side"

[colors.light]
accent = "#0e7490"
"##;

fn colour(text: &str, theme: &str, token: &str, value: Option<&str>) -> Value {
    let mut argument = json!({ "text": text, "theme": theme, "token": token });
    if let Some(value) = value {
        argument["value"] = json!(value);
    }
    execute(HMI_SCREEN_COLOUR_COMMAND, vec![argument])
}

fn written(answer: &Value) -> &str {
    assert_eq!(answer["ok"], true, "{answer}");
    answer["text"].as_str().expect("the new text")
}

/// A chosen colour goes into its theme's table; the other theme's colours and the rest of the
/// screen stay. Catches the other theme's table dropped and the screen's other settings lost.
#[test]
fn a_chosen_colour_is_written_to_its_theme_table() {
    let answer = colour(SCREEN_EXAMPLE, "dark", "accent", Some("#f97316"));
    let text = written(&answer);
    assert!(
        text.contains("[colors.dark]\naccent = \"#f97316\"\n"),
        "{text}"
    );
    assert!(
        text.contains("[colors.light]\naccent = \"#0e7490\"\n"),
        "{text}"
    );
    assert_eq!(
        answer["model"]["colors"],
        json!({ "dark": { "accent": "#f97316" }, "light": { "accent": "#0e7490" } })
    );
    assert_eq!(answer["model"]["top_bar"]["title"], "Feed station");
    assert_eq!(answer["model"]["theme"], "light");
}

/// Reset removes the project's colour so the default applies again, and a table left empty is not
/// written; the theme's other colours stay, and resetting a colour the project does not set
/// changes nothing. Catches an empty `[colors.light]` left behind and a reset that clears the
/// whole theme.
#[test]
fn reset_removes_the_colour_and_leaves_no_empty_table() {
    let answer = colour(SCREEN_EXAMPLE, "light", "accent", None);
    let text = written(&answer);
    assert!(!text.contains("[colors"), "{text}");
    assert!(!text.contains("accent"), "{text}");
    assert_eq!(answer["model"].get("colors"), None, "{}", answer["model"]);

    let two = format!("{SCREEN_EXAMPLE}pen-1 = \"#ff8800\"\n");
    let answer = colour(&two, "light", "accent", None);
    assert!(
        written(&answer).contains("[colors.light]\npen-1 = \"#ff8800\"\n"),
        "{answer}"
    );
    assert_eq!(
        answer["model"]["colors"],
        json!({ "light": { "pen-1": "#ff8800" } })
    );

    let answer = colour(SCREEN_EXAMPLE, "dark", "accent", None);
    written(&answer);
    assert_eq!(
        answer["model"]["colors"],
        json!({ "light": { "accent": "#0e7490" } })
    );
}

/// What is written must load, so an unknown token and a value that is not a hex colour are
/// refused with the file's own error and no text. Catches the editor writing a `screen.toml` that
/// no longer loads.
#[test]
fn an_unknown_token_or_a_value_that_is_not_hex_is_refused() {
    let answer = colour(SCREEN_EXAMPLE, "dark", "acent", Some("#f97316"));
    assert_eq!(answer["ok"], false, "{answer}");
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| error.contains("`acent` is not a colour token")),
        "{answer}"
    );
    assert_eq!(answer.get("text"), None, "{answer}");

    let answer = colour(SCREEN_EXAMPLE, "dark", "accent", Some("red"));
    assert_eq!(answer["ok"], false, "{answer}");
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| error.contains("`accent` must be a hex colour")),
        "{answer}"
    );
}

/// A colour belongs to the dark or the light theme, and a file that does not load is refused
/// with its error. Catches a third theme table written and a broken file overwritten.
#[test]
fn another_theme_and_a_file_that_does_not_load_are_refused() {
    let answer = colour(SCREEN_EXAMPLE, "blue", "accent", Some("#f97316"));
    assert_eq!(
        answer,
        json!({
            "ok": false,
            "error": "A project colour belongs to the dark or the light theme, not \"blue\"."
        })
    );

    let broken = SCREEN_EXAMPLE.replacen("size = [1920, 1080]", "size = \"wide\"", 1);
    let answer = colour(&broken, "dark", "accent", Some("#f97316"));
    assert_eq!(answer["ok"], false, "{answer}");
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| !error.is_empty()),
        "{answer}"
    );
    assert_eq!(answer.get("text"), None, "{answer}");
}
