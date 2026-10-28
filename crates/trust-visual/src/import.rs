//! Imports of visual programs (specification 25 §11.4): which files an import accepts and the name
//! the imported copy gets.
//!
//! An import accepts a JSON file of the shape of its kind: a Ladder program has a `networks`
//! array, a Blockly workspace a `blocks.blocks` array, a Statechart `id` and `states`, an SFC
//! `name`, `steps` and `transitions`. The copy keeps the file name and gets the suffix of its kind
//! when the name lacks it (`.sfc` is an SFC suffix too), so the copy opens in its editor and gets
//! its companion.

use crate::kind::{file_name, VisualKind};
use crate::Json;

/// Why the text `text` is no file of `kind`, or `None` when an import of `kind` accepts it.
fn shape_problem(kind: VisualKind, text: &str) -> Option<String> {
    let Ok(file) = Json::parse(text) else {
        return Some(match kind {
            VisualKind::Blockly => "Invalid JSON format in Blockly file.".to_owned(),
            _ => shape_message(kind),
        });
    };
    let has = |key: &str| file.get(key).is_some();
    let fits = match kind {
        VisualKind::Ladder => file.get("networks").and_then(Json::as_array).is_some(),
        VisualKind::Blockly => crate::blockly::check_format(&file).is_ok(),
        VisualKind::Statechart => has("id") && has("states"),
        VisualKind::Sfc => has("name") && has("steps") && has("transitions"),
    };
    (!fits).then(|| shape_message(kind))
}

fn shape_message(kind: VisualKind) -> String {
    match kind {
        VisualKind::Ladder => "Invalid Ladder file. Must be JSON with 'networks' array.".to_owned(),
        VisualKind::Blockly => {
            "Invalid Blockly file. Must be JSON with a 'blocks.blocks' array.".to_owned()
        }
        VisualKind::Statechart => {
            "Invalid statechart file. Must be JSON with 'id' and 'states' properties.".to_owned()
        }
        VisualKind::Sfc => {
            "Invalid SFC file. Must be JSON with 'name', 'steps', and 'transitions' properties."
                .to_owned()
        }
    }
}

/// The file name the copy of `file` (a name or a path) gets when it is imported as `kind`.
///
/// # Errors
/// The text `text` of the file is not a file of `kind` (see the module documentation).
pub fn import_target(kind: VisualKind, file: &str, text: &str) -> Result<String, String> {
    if let Some(problem) = shape_problem(kind, text) {
        return Err(problem);
    }
    let name = file_name(file);
    let name = if name.is_empty() { "imported" } else { name };
    Ok(if kind.owns(name) {
        name.to_owned()
    } else {
        format!("{name}{}", kind.suffix())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_accepts_only_its_own_shape() {
        // Worked from the module rule. Catches the Blockly copy, which took any JSON, and a
        // Ladder check that accepts `networks` that is no array.
        assert!(import_target(VisualKind::Ladder, "a.json", r#"{"networks":[]}"#).is_ok());
        assert_eq!(
            import_target(VisualKind::Ladder, "a.json", r#"{"networks":{}}"#),
            Err("Invalid Ladder file. Must be JSON with 'networks' array.".to_owned())
        );
        assert!(
            import_target(VisualKind::Blockly, "b.json", r#"{"blocks":{"blocks":[]}}"#).is_ok()
        );
        assert_eq!(
            import_target(VisualKind::Blockly, "b.json", "{}"),
            Err("Invalid Blockly file. Must be JSON with a 'blocks.blocks' array.".to_owned())
        );
        assert_eq!(
            import_target(VisualKind::Blockly, "b.json", "{"),
            Err("Invalid JSON format in Blockly file.".to_owned())
        );
        assert!(import_target(
            VisualKind::Statechart,
            "c.json",
            r#"{"id":"x","states":{}}"#
        )
        .is_ok());
        assert!(import_target(VisualKind::Statechart, "c.json", r#"{"id":"x"}"#).is_err());
        assert!(import_target(
            VisualKind::Sfc,
            "d.sfc",
            r#"{"name":"","steps":[],"transitions":[]}"#
        )
        .is_ok());
        assert!(import_target(VisualKind::Sfc, "d.sfc", "[]").is_err());
    }

    #[test]
    fn the_copy_gets_the_suffix_of_its_kind() {
        // One rule for the four imports: the name is kept and the kind's suffix added when it is
        // missing. Catches the Statechart and Blockly copies, which kept `chart.json`, a name no
        // visual editor opens.
        let chart = r#"{"id":"x","states":{}}"#;
        assert_eq!(
            import_target(VisualKind::Statechart, "/in/chart.json", chart),
            Ok("chart.json.statechart.json".to_owned())
        );
        assert_eq!(
            import_target(
                VisualKind::Statechart,
                "C:\\in\\Lights.STATECHART.json",
                chart
            ),
            Ok("Lights.STATECHART.json".to_owned())
        );
        let sfc = r#"{"name":"","steps":[],"transitions":[]}"#;
        assert_eq!(
            import_target(VisualKind::Sfc, "/in/flow.sfc", sfc),
            Ok("flow.sfc".to_owned())
        );
        assert_eq!(
            import_target(VisualKind::Sfc, "/in/flow.json", sfc),
            Ok("flow.json.sfc.json".to_owned())
        );
        assert_eq!(
            import_target(
                VisualKind::Ladder,
                "/in/x.ladder.json",
                r#"{"networks":[]}"#
            ),
            Ok("x.ladder.json".to_owned())
        );
    }
}
