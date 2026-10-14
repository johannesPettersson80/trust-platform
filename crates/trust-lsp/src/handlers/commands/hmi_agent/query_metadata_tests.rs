//! Tests of the provenance an inspection reports (specification 43, AT-03): the unit an object shows and where
//! it comes from, the bindings as authored with their format keys, and the range the object authors.
//!
//! Port of the rules of `editors/vscode/src/hmiBuilder/agent/queryMetadata.ts` (`inspectMetadata`) and of
//! `editors/vscode/src/hmiBuilder/drawing/valueFormats.ts` (`withValueFormat`, `formatPath`), restricted to the
//! inspection, which has no runtime values. The TypeScript suite has no test of these rules, so the expected
//! values are those the TypeScript code gives for the same objects.

use serde_json::{json, Value};

use super::*;

/// A tank whose value is bound to `Main.Level`, with the given unit on the object or on its binding.
fn tank(object_unit: Option<&str>, binding_unit: Option<&str>) -> Value {
    let mut binding = json!({ "path": "Main.Level", "direction": "read" });
    if let Some(unit) = binding_unit {
        binding["unit"] = json!(unit);
    }
    let mut object = json!({ "id": "tank", "type": "tank", "bind": { "value": binding } });
    if let Some(unit) = object_unit {
        object["unit"] = json!(unit);
    }
    object
}

/// The project format of `Main.Level`, which gives the unit `bar`.
fn formats() -> Value {
    json!({ "main.level": { "unit": "bar" } })
}

/// The object's own unit comes first, before the binding and the project format (`withValueFormat`:
/// `object.unit ?? binding.unit ?? equipment?.unit ?? format?.unit`).
#[test]
fn the_object_unit_wins_over_its_binding_and_the_project_format() {
    let provenance = provenance_of(&tank(Some("m"), Some("kg")), &formats());
    assert_eq!(
        provenance["unit"],
        json!({ "value": "m", "source": "object" })
    );
}

/// Without an object unit the value binding's unit decides, then the project format of the bound path, and
/// without any of them the unit is unspecified (`withValueFormat`; `unit.source` of `inspectMetadata`).
#[test]
fn the_binding_unit_comes_next_then_the_format_of_the_bound_path() {
    assert_eq!(
        provenance_of(&tank(None, Some("kg")), &formats())["unit"],
        json!({ "value": "kg", "source": "binding" })
    );
    assert_eq!(
        provenance_of(&tank(None, None), &formats())["unit"],
        json!({ "value": "bar", "source": "project-format" })
    );
    assert_eq!(
        provenance_of(&tank(None, None), &json!({}))["unit"],
        json!({ "value": null, "source": "unspecified" })
    );
}

/// Only the value binding supplies a unit beside the object: a unit on another bound property is not the
/// object's unit, and the format of its path does not apply (`withValueFormat` reads `bind.value`).
#[test]
fn only_the_value_binding_supplies_a_unit() {
    let pump = json!({
        "id": "pump",
        "type": "pump",
        "bind": { "speed": { "path": "Main.Speed", "direction": "read", "unit": "rpm" } },
    });
    let provenance = provenance_of(&pump, &json!({ "main.speed": { "unit": "Hz" } }));
    assert_eq!(
        provenance["unit"],
        json!({ "value": null, "source": "unspecified" })
    );
}

/// Each binding is reported as authored, with the format key of its path (`bindings` of `inspectMetadata`).
/// The token source names the shared theme and the project overrides (`tokens` of `inspectMetadata`).
#[test]
fn each_binding_is_reported_as_authored_with_its_format_key() {
    let provenance = provenance_of(&tank(None, None), &formats());
    assert_eq!(
        provenance["bindings"]["value"],
        json!({
            "path": "Main.Level",
            "direction": "read",
            "source": "authored",
            "formatKey": "main.level",
        })
    );
    assert_eq!(
        provenance["tokens"]["source"],
        json!("shared-theme-and-project-overrides")
    );
}

/// The range is the object's own authored minimum and maximum, with the source `authored-or-unspecified`; an
/// object that authors neither reports nulls (`range` of `inspectMetadata`, with no runtime range).
#[test]
fn the_range_is_the_authored_minimum_and_maximum() {
    let authored = json!({ "id": "bar", "type": "bar", "min": 0, "max": 100, "bind": {} });
    assert_eq!(
        provenance_of(&authored, &json!({}))["range"],
        json!({ "min": 0, "max": 100, "source": "authored-or-unspecified" })
    );
    let unauthored = json!({ "id": "bar", "type": "bar", "bind": {} });
    assert_eq!(
        provenance_of(&unauthored, &json!({}))["range"],
        json!({ "min": null, "max": null, "source": "authored-or-unspecified" })
    );
}

/// Only ASCII capitals of a bound path are lower-cased, which is `formatPath` in `valueFormats.ts`: it replaces
/// the letters A to Z and nothing else, so format lookups are case-insensitive for IEC member names.
#[test]
fn a_format_key_lowercases_ascii_capitals_only() {
    assert_eq!(format_path("Main.Speed_1"), "main.speed_1");
    assert_eq!(format_path("Main.Ünit"), "main.Ünit");
}
