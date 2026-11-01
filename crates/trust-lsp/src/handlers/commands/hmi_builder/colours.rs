//! Project colours (specification 39, §14.1 item 2): one colour token of the screen's dark or light
//! theme chosen in `hmi/screen.toml`, or reset so that truST's default applies again.
//!
//! `{ text, theme, token, value? }` → `{ text, model }`: the file's text with `[colors.<theme>]`
//! holding `token` = `value`, or without `token` when no value is given, and the screen model the
//! editor draws with. A theme table left empty is not written. The text is the runtime's own
//! writing of the screen and must load again, so an unknown token and a value that is not a hex
//! colour are refused with the file's error, as is a file that does not load.

use serde::Deserialize;
use serde_json::{json, Value};
use trust_runtime::hmi::page_files::{parse_screen, write_screen, PageFileError};

use super::decode;

/// The screen layout's file in the project.
const SCREEN_FILE: &str = "hmi/screen.toml";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    text: String,
    theme: String,
    token: String,
    #[serde(default)]
    value: Option<String>,
}

/// `{ ok: true, text, model }` for the colour in `argument`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    let args: Args = decode(argument)?;
    let mut screen =
        parse_screen(SCREEN_FILE, &args.text).map_err(|errors| first_problem(&errors))?;
    let table = match args.theme.as_str() {
        "dark" => &mut screen.colors.dark,
        "light" => &mut screen.colors.light,
        other => {
            return Err(format!(
                "A project colour belongs to the dark or the light theme, not \"{other}\"."
            ))
        }
    };
    match args.value {
        Some(value) => table.insert(args.token, value),
        None => table.remove(&args.token),
    };
    let text = write_screen(&screen);
    let model = super::super::hmi_authoring::documents::parse("screen", SCREEN_FILE, &text)
        .map_err(|errors| first_problem(&errors))?;
    Ok(json!({ "ok": true, "text": text, "model": model }))
}

fn first_problem(errors: &[PageFileError]) -> String {
    errors.first().map_or_else(
        || format!("{SCREEN_FILE} does not load."),
        |error| error.message.clone(),
    )
}

#[cfg(test)]
#[path = "colours_tests.rs"]
mod tests;
