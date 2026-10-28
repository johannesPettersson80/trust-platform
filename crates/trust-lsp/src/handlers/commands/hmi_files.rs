//! HMI Builder page and screen files for the editor (specification 39, §3.1, §15, §17.3).
//!
//! Native presentation fields reuse the runtime parser. Editor intent is preserved separately
//! and must be projected to that unchanged deployment format.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use trust_runtime::hmi::my_symbols::{parse_symbol, write_symbol, HmiSymbol};
use trust_runtime::hmi::page_files::{parse_texts, write_texts, HmiTexts, PageFileError};

use super::hmi_authoring::pairs::{parse_pair, write_pair, Pair as HmiPair};

/// File name used for errors in text written from a model.
const MODEL_FILE: &str = "(edited model)";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ParseArgs {
    #[serde(default)]
    root_uri: Option<tower_lsp::lsp_types::Url>,
    #[serde(default)]
    open: std::collections::BTreeMap<String, String>,
    kind: String,
    text: String,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    drawing: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FormatArgs {
    kind: String,
    model: Value,
    #[serde(default)]
    drawing: Option<String>,
}

/// `trust-lsp.hmiFileParse`: `{ kind: "page" | "popup" | "screen" | "texts", text, file? }` to
/// the file's model.
pub(super) fn parse_value(args: Vec<Value>) -> Value {
    let args: ParseArgs = match single_argument(args, "trust-lsp.hmiFileParse") {
        Ok(args) => args,
        Err(message) => return refused(message),
    };
    let file = args.file.as_deref().unwrap_or("(unsaved)");
    match args.kind.as_str() {
        "composition" => {
            super::hmi_composition::compose(args.root_uri, file, &args.text, args.open)
        }
        "panel" | "page" | "popup" | "screen" | "faceplate" => loaded(
            &args.kind,
            super::hmi_authoring::documents::parse(&args.kind, file, &args.text),
        ),
        "texts" => loaded("texts", parse_texts(file, &args.text)),
        "pair" => loaded("pair", parse_pair(file, &args.text)),
        "symbol" => {
            let name = file
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(file)
                .strip_suffix(".toml")
                .unwrap_or(file);
            loaded(
                "symbol",
                parse_symbol(name, &args.text, args.drawing.as_deref().unwrap_or("")),
            )
        }
        other => refused(unknown_kind(other)),
    }
}

/// `trust-lsp.hmiFileFormat`: `{ kind: "page" | "popup" | "screen" | "texts", model }` to
/// canonical text.
///
/// The written text is read back before it is returned, so the editor never receives a file
/// with invalid native presentation fields or malformed editor intent.
pub(super) fn format_value(args: Vec<Value>) -> Value {
    let args: FormatArgs = match single_argument(args, "trust-lsp.hmiFileFormat") {
        Ok(args) => args,
        Err(message) => return refused(message),
    };
    match args.kind.as_str() {
        "panel" | "page" | "popup" | "screen" | "faceplate" => {
            match super::hmi_authoring::documents::write(&args.kind, args.model) {
                Ok(text) => json!({"ok":true,"kind":args.kind,"text":text}),
                Err(errors) => file_errors(&errors),
            }
        }
        "texts" => written::<HmiTexts>("texts", args.model, write_texts, |text| {
            parse_texts(MODEL_FILE, text).map(drop)
        }),
        "pair" => written::<HmiPair>("pair", args.model, write_pair, |text| {
            parse_pair(MODEL_FILE, text).map(drop)
        }),
        "symbol" => {
            let name = args
                .model
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            written::<HmiSymbol>("symbol", args.model, write_symbol, |text| {
                parse_symbol(&name, text, args.drawing.as_deref().unwrap_or("")).map(drop)
            })
        }
        other => refused(unknown_kind(other)),
    }
}

fn loaded<T: Serialize>(kind: &str, result: Result<T, Vec<PageFileError>>) -> Value {
    match result {
        Ok(model) => json!({ "ok": true, "kind": kind, "model": model }),
        Err(errors) => file_errors(&errors),
    }
}

fn written<T: for<'de> Deserialize<'de>>(
    kind: &str,
    model: Value,
    write: fn(&T) -> String,
    read_back: impl Fn(&str) -> Result<(), Vec<PageFileError>>,
) -> Value {
    let model: T = match serde_json::from_value(model) {
        Ok(model) => model,
        Err(error) => return refused(format!("the {kind} model is invalid: {error}")),
    };
    let text = write(&model);
    match read_back(&text) {
        Ok(()) => json!({ "ok": true, "kind": kind, "text": text }),
        Err(errors) => file_errors(&errors),
    }
}

pub(super) fn single_argument<T: for<'de> Deserialize<'de>>(
    args: Vec<Value>,
    command: &str,
) -> Result<T, String> {
    let [argument] = <[Value; 1]>::try_from(args)
        .map_err(|_| format!("{command} expects one argument object"))?;
    serde_json::from_value(argument)
        .map_err(|error| format!("invalid {command} arguments: {error}"))
}

fn unknown_kind(kind: &str) -> String {
    format!(
        "unknown HMI file kind \"{kind}\"; expected \"page\", \"popup\", \"screen\", \"texts\", \"faceplate\", \"pair\" or \"symbol\""
    )
}

pub(super) fn refused(message: String) -> Value {
    json!({ "ok": false, "errors": [{ "message": message }] })
}

pub(super) fn file_errors(errors: &[PageFileError]) -> Value {
    let errors = errors
        .iter()
        .map(|error| json!({ "file": error.file, "line": error.line, "message": error.message }))
        .collect::<Vec<_>>();
    json!({ "ok": false, "errors": errors })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANONICAL_PAGE: &str = r#"format = 1
title = "Feed"

[[object]]
id = "lt-101"
tag = "LT-101"
type = "value"
at = [40, 60]
size = [160, 48]
unit = "%"
decimals = 1
min = 0
max = 100

[object.bind]
setpoint = { path = "FeedArea.Setpoint", direction = "read-write" }
value = "FeedArea.Level"

[object.click]
page = "trends"

[[pipe]]
from = "p-101.out"
to = "v-101.in"
"#;

    const CANONICAL_SCREEN: &str = r#"format = 1
size = [1920, 1080]
theme = "dark"
symbols = "flat"

[top_bar]
title = "Feed area"
clock = true
alarms = true

[navigation]
style = "side"
position = "right"
"#;

    const CANONICAL_TEXTS: &str = "format = 1\nlanguage = \"sv\"\n\n[texts]\n\"Feed\" = \"Matning\"\n\"Pumps and valves\" = \"Pumpar och ventiler\"\n";

    #[test]
    fn a_text_file_is_read_into_its_model_and_written_back_as_canonical_text() {
        let parsed = parse_value(vec![
            json!({ "kind": "texts", "text": CANONICAL_TEXTS, "file": "hmi/texts/sv.toml" }),
        ]);
        assert_eq!(parsed["ok"], json!(true), "{parsed}");
        assert_eq!(parsed["kind"], json!("texts"));
        assert_eq!(
            parsed["model"],
            json!({ "language": "sv", "texts": { "Feed": "Matning", "Pumps and valves": "Pumpar och ventiler" } })
        );
        assert_eq!(
            format("texts", parsed["model"].clone()),
            json!({ "ok": true, "kind": "texts", "text": CANONICAL_TEXTS })
        );
    }

    #[test]
    fn a_text_file_that_does_not_load_is_reported_with_file_and_line() {
        let text = "format = 1\nlanguage = \"sv\"\n\n[texts]\n\"Feed\" = 3\n";
        let parsed = parse_value(vec![
            json!({ "kind": "texts", "text": text, "file": "hmi/texts/sv.toml" }),
        ]);
        assert_eq!(
            parsed,
            json!({ "ok": false, "errors": [{ "file": "hmi/texts/sv.toml", "line": 5, "message": "the text for \"Feed\" must be text in quotes" }] })
        );
    }

    fn parse(kind: &str, text: &str) -> Value {
        parse_value(vec![
            json!({ "kind": kind, "text": text, "file": "hmi/pages/feed.toml" }),
        ])
    }

    fn format(kind: &str, model: Value) -> Value {
        format_value(vec![json!({ "kind": kind, "model": model })])
    }

    const CANONICAL_POPUP: &str = r#"format = 1
title = "Pump P101"
size = [400, 240]
pinnable = true

[[object]]
id = "speed"
type = "value"
at = [24, 24]
size = [200, 96]

[object.bind]
value = "Main.P101.Speed"
"#;

    const CANONICAL_FACEPLATE: &str = r#"format = 1
title = "Pump"
description = "Transfer pump"
size = [400, 300]

[fields.Speed]
accepts = ["number"]

[[object]]
id = "speed"
type = "value"
at = [24, 24]
size = [200, 48]

[object.bind]
value = "$Speed"
"#;

    const CANONICAL_PAIR: &str = "format = 1\nblock = \"Equipment.Pump\"\nfaceplate = \"pump\"\nsymbol = \"pump\"\n\n[fields]\nSpeed = \"ActualSpeed\"\n\n[program]\nposition = \"actuators\"\n";

    /// Specification 39, §8.7: faceplate and pair files are read and written as the other HMI
    /// files are.
    #[test]
    fn a_faceplate_and_a_pair_are_read_into_their_models_and_written_back_as_canonical_text() {
        let faceplate = parse("faceplate", CANONICAL_FACEPLATE);
        assert_eq!(faceplate["ok"], json!(true), "{faceplate}");
        assert_eq!(
            faceplate["model"]["fields"]["Speed"]["accepts"],
            json!(["number"])
        );
        assert_eq!(faceplate["model"]["description"], json!("Transfer pump"));
        assert_eq!(
            format("faceplate", faceplate["model"].clone()),
            json!({ "ok": true, "kind": "faceplate", "text": CANONICAL_FACEPLATE })
        );
        let pair = parse("pair", CANONICAL_PAIR);
        assert_eq!(pair["ok"], json!(true), "{pair}");
        assert_eq!(pair["model"]["block"], json!("Equipment.Pump"));
        assert_eq!(pair["model"]["program"]["position"], json!("actuators"));
        assert_eq!(
            format("pair", pair["model"].clone()),
            json!({ "ok": true, "kind": "pair", "text": CANONICAL_PAIR })
        );
    }

    #[test]
    fn a_popup_is_read_into_its_model_and_written_back_as_canonical_text() {
        let parsed = parse("popup", CANONICAL_POPUP);
        assert_eq!(parsed["ok"], json!(true), "{parsed}");
        assert_eq!(parsed["kind"], json!("popup"));
        assert_eq!(
            (&parsed["model"]["size"], &parsed["model"]["pinnable"]),
            (&json!([400, 240]), &json!(true))
        );
        assert_eq!(
            format("popup", parsed["model"].clone()),
            json!({ "ok": true, "kind": "popup", "text": CANONICAL_POPUP })
        );
    }

    #[test]
    fn a_page_is_read_into_its_model_in_the_file_vocabulary() {
        assert_eq!(
            parse("page", CANONICAL_PAGE),
            json!({
                "ok": true,
                "kind": "page",
                "model": {
                    "title": "Feed",
                    "object": [{
                        "id": "lt-101",
                        "tag": "LT-101",
                        "type": "value",
                        "at": [40, 60],
                        "size": [160, 48],
                        "rotation": 0,
                        "unit": "%",
                        "decimals": 1,
                        "min": 0.0,
                        "max": 100.0,
                        "bind": {
                            "setpoint": { "path": "FeedArea.Setpoint", "direction": "read-write" },
                            "value": { "path": "FeedArea.Level", "direction": "read" }
                        },
                        "click": { "page": "trends" }
                    }],
                    "pipe": [{ "from": "p-101.out", "to": "v-101.in" }]
                }
            })
        );
    }

    #[test]
    fn a_file_that_does_not_load_is_reported_with_file_and_line() {
        let result = parse("page", "format = 1\ntitle = \"Feed\"\ncolour = \"red\"\n");
        assert_eq!(result["ok"], false);
        assert_eq!(result["errors"][0]["file"], "hmi/pages/feed.toml");
        assert_eq!(result["errors"][0]["line"], 3);
        assert!(result["errors"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("colour")));
    }

    #[test]
    fn a_model_is_written_as_canonical_text() {
        let model = parse("page", CANONICAL_PAGE)["model"].clone();
        assert_eq!(
            format("page", model),
            json!({ "ok": true, "kind": "page", "text": CANONICAL_PAGE })
        );
    }

    #[test]
    fn an_edited_model_changes_only_the_edited_line() {
        let mut model = parse("page", CANONICAL_PAGE)["model"].clone();
        model["object"][0]["at"] = json!([48, 60]);
        let text = format("page", model)["text"]
            .as_str()
            .expect("text")
            .to_string();
        let changed = CANONICAL_PAGE
            .lines()
            .zip(text.lines())
            .filter(|(before, after)| before != after)
            .collect::<Vec<_>>();
        assert_eq!(changed, vec![("at = [40, 60]", "at = [48, 60]")]);
    }

    #[test]
    fn a_model_that_breaks_the_file_rules_is_refused() {
        let mut model = parse("page", CANONICAL_PAGE)["model"].clone();
        let copy = model["object"][0].clone();
        model["object"].as_array_mut().expect("objects").push(copy);
        let result = format("page", model);
        assert_eq!(result["ok"], false);
        assert!(result["errors"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("lt-101")));

        let result = format(
            "page",
            json!({ "title": "Feed", "object": [], "pipe": [], "colour": 1 }),
        );
        assert_eq!(result["ok"], false);
        assert!(result["errors"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("colour")));
    }

    #[test]
    fn the_screen_file_round_trips() {
        let parsed = parse("screen", CANONICAL_SCREEN);
        assert_eq!(parsed["ok"], true, "{parsed}");
        assert_eq!(
            parsed["model"],
            json!({
                "size": [1920, 1080],
                "theme": "dark",
                "symbols": "flat",
                // Without `languages`, the screen's one language is English (§14.5 item 2).
                "languages": ["en"],
                "top_bar": {
                    "title": "Feed area",
                    "clock": true,
                    "alarms": true,
                    "user": false,
                    "page_title": false
                },
                "navigation": { "style": "side", "position": "right" }
            })
        );
        assert_eq!(
            format("screen", parsed["model"].clone())["text"],
            json!(CANONICAL_SCREEN)
        );
    }

    #[test]
    fn a_screen_with_project_colours_round_trips() {
        let text = format!(
            "{CANONICAL_SCREEN}\n[colors.dark]\naccent = \"#ff8800\"\npage = \"#000000\"\n"
        );
        let parsed = parse("screen", &text);
        assert_eq!(parsed["ok"], true, "{parsed}");
        assert_eq!(
            parsed["model"]["colors"],
            json!({ "dark": { "accent": "#ff8800", "page": "#000000" } })
        );
        assert_eq!(
            format("screen", parsed["model"].clone())["text"],
            json!(text)
        );
    }

    #[test]
    fn a_symbol_definition_is_editable_with_its_cleaned_drawing() {
        let drawing =
            r#"<svg viewBox="0 0 100 100"><circle id="rotor" cx="50" cy="50" r="20"/></svg>"#;
        let text = "format = 1\ntitle = \"Mixer\"\nsize = [128, 128]\n\n[properties]\nrunning = \"boolean\"\n\n[[part]]\nid = \"rotor\"\nspin = \"running\"\n";
        let parsed = parse_value(vec![json!({
            "kind": "symbol", "text": text, "file": "hmi/symbols/mixer.toml",
            "drawing": drawing
        })]);
        assert_eq!(parsed["ok"], true, "symbol definition must load: {parsed}");
        assert_eq!(parsed["model"]["parts"][0]["spin"], "running");
        assert_eq!(
            parsed["model"]["svg"]["children"][0]["attrs"]["id"],
            "my-mixer-rotor"
        );
        let mut model = parsed["model"].clone();
        model["parts"][0] = json!({"part": "rotor", "active": "running"});
        let written = format_value(vec![
            json!({"kind": "symbol", "model": model, "drawing": drawing}),
        ]);
        assert_eq!(
            written["ok"], true,
            "edited symbol must be writable: {written}"
        );
        let next = parse_value(vec![json!({"kind": "symbol", "text": written["text"],
            "file": "hmi/symbols/mixer.toml", "drawing": drawing})]);
        assert_eq!(next["ok"], true, "{next}");
        assert_eq!(next["model"]["parts"][0]["active"], "running");
        assert!(next["model"]["parts"][0].get("spin").is_none());
    }

    #[test]
    fn an_unknown_kind_is_refused() {
        let result = parse("unknown", CANONICAL_PAGE);
        assert_eq!(result["ok"], false);
        assert!(result["errors"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("unknown")));
    }
}
