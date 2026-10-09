//! The editor's documents are the runtime's page files (specification 39, §21.1): parsed, checked
//! and written by the runtime's one implementation. This module only converts them to and from
//! the JSON model the editor edits.
use serde_json::Value;
use std::collections::BTreeMap;
use trust_runtime::hmi::page_files::{self as native, PageFileError};

fn failure(file: &str, message: impl ToString) -> Vec<PageFileError> {
    vec![PageFileError {
        file: file.into(),
        line: 1,
        message: message.to_string(),
    }]
}

// Optional JSON fields may be null; the file expresses the same absence by omitting the key.
fn omit_null_fields(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            fields.retain(|_, value| !value.is_null());
            for value in fields.values_mut() {
                omit_null_fields(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                omit_null_fields(value);
            }
        }
        _ => {}
    }
}

/// The JSON model of a page, popup, panel or screen file.
pub(crate) fn parse(kind: &str, file: &str, text: &str) -> Result<Value, Vec<PageFileError>> {
    let model = match kind {
        "page" => serde_json::to_value(native::parse_page(file, text)?),
        "popup" => serde_json::to_value(native::parse_popup(file, text)?),
        "panel" | "faceplate" => serde_json::to_value(native::parse_faceplate(file, text)?),
        "screen" => serde_json::to_value(native::parse_screen(file, text)?),
        _ => return Err(failure(file, "Unknown document kind")),
    };
    let mut model = model.map_err(|error| failure(file, error))?;
    omit_null_fields(&mut model);
    if matches!(kind, "panel" | "faceplate")
        && model["object"].as_array().is_some_and(|objects| {
            objects
                .iter()
                .any(|object| object["type"] == native::PLACED_FACEPLATE)
        })
    {
        return Err(failure(
            file,
            "A reusable panel contains objects, not nested panels",
        ));
    }
    Ok(model)
}

/// The canonical text of an edited model; the text is loaded again, so what is written loads.
pub(crate) fn write(kind: &str, mut model: Value) -> Result<String, Vec<PageFileError>> {
    let file = "(edited model)";
    omit_null_fields(&mut model);
    macro_rules! write_as {
        ($writer:path) => {
            $writer(&serde_json::from_value(model).map_err(|error| failure(file, error))?)
        };
    }
    let text = match kind {
        "page" => write_as!(native::write_page),
        "popup" => write_as!(native::write_popup),
        "panel" | "faceplate" => write_as!(native::write_faceplate),
        "screen" => write_as!(native::write_screen),
        _ => return Err(failure(file, "Unknown document kind")),
    };
    parse(kind, file, &text)?;
    Ok(text)
}

/// A page or popup and the block each equipment object records, by object id.
pub(crate) fn page(
    file: &str,
    text: &str,
    popup: bool,
) -> Result<(native::HmiPage, BTreeMap<String, String>), Vec<PageFileError>> {
    let page = if popup {
        native::parse_popup(file, text)?.page
    } else {
        native::parse_page(file, text)?
    };
    let blocks = page
        .objects
        .iter()
        .filter_map(|object| Some((object.id.clone(), object.block.clone()?)))
        .collect();
    Ok((page, blocks))
}

/// The JSON model of a panel the runtime has bound to an instance (§8.9), as `parse` gives a file.
pub(crate) fn faceplate_model(faceplate: &native::HmiFaceplate) -> Value {
    let mut model = serde_json::to_value(faceplate).unwrap_or(Value::Null);
    omit_null_fields(&mut model);
    model
}

/// A panel file.
pub(crate) fn faceplate(
    file: &str,
    text: &str,
) -> Result<native::HmiFaceplate, Vec<PageFileError>> {
    native::parse_faceplate(file, text)
}

/// Whether a panel can only be shown bound to an instance: it has fields, offers settings, or
/// commands its `$instance`.
pub(crate) fn requires_instance(model: &Value) -> bool {
    model["settings"].as_bool() == Some(true)
        || model["fields"]
            .as_object()
            .is_some_and(|fields| !fields.is_empty())
        || model["object"].as_array().is_some_and(|objects| {
            objects.iter().any(|object| {
                object["command"]["instance"]
                    .as_str()
                    .is_some_and(|instance| instance.starts_with("$instance"))
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    const PAGE: &str = "format = 1\ntitle = 'Process'\n[[object]]\nid = 'pump'\ntype = 'pump'\nblock = 'Plant.FB_Pump'\nfaceplate = 'plant-run'\ninstance = 'Main.P101'\nat = [40, 60]\nsize = [80, 80]\n[object.bind]\nrunning = { path = 'Main.P101.Running', quality = 'Main.P101.Quality' }\n";

    #[test]
    fn an_equipment_object_survives_the_editor_round_trip_in_the_one_format() {
        let model = parse("page", "hmi/pages/main.toml", PAGE).unwrap();
        assert_eq!(model["object"][0]["block"], "Plant.FB_Pump");
        assert_eq!(model["object"][0]["faceplate"], "plant-run");
        assert_eq!(model["object"][0]["at"], serde_json::json!([40, 60]));
        assert_eq!(
            model["object"][0]["bind"]["running"]["quality"],
            "Main.P101.Quality"
        );
        let written = write("page", model.clone()).unwrap();
        assert_eq!(
            parse("page", "hmi/pages/main.toml", &written).unwrap(),
            model
        );
        let (native, blocks) = page("hmi/pages/main.toml", &written, false).unwrap();
        assert_eq!(native.objects[0].instance.as_deref(), Some("Main.P101"));
        assert_eq!(blocks["pump"], "Plant.FB_Pump");
        assert_eq!(
            written,
            native::write_page(&native),
            "the editor writes exactly what the runtime writes"
        );
    }

    #[test]
    fn a_malformed_block_and_an_unknown_key_are_refused() {
        assert!(parse(
            "page",
            "main",
            &PAGE.replace("Plant.FB_Pump", "Plant.FB Pump")
        )
        .is_err());
        assert!(parse(
            "page",
            "main",
            &PAGE.replace("type = 'pump'", "type = 'pump'\nunknown_setting = true")
        )
        .is_err());
    }

    #[test]
    fn a_placed_panel_without_an_instance_keeps_none() {
        let text = "format=1\ntitle='Overview'\n[[object]]\nid='navigation'\ntype='faceplate'\nfaceplate='navigation'\nat=[0,0]\nsize=[200,720]\n";
        let model = parse("page", "overview", text).unwrap();
        assert!(model["object"][0].get("instance").is_none());
        let saved = write("page", model).unwrap();
        assert!(!saved.contains("instance ="));
    }
}
