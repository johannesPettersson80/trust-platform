//! New visual programs (specification 25 §11.4): the name rule and the file of a new Ladder,
//! Blockly, Statechart or SFC program.
//!
//! A name is trimmed and may end with the suffix of its kind, which is cut. What remains must be
//! one or more letters, digits, hyphens or underscores, so it is a file name on every platform
//! and the companion function block can be named after it. The new file is the template of its
//! kind with the name (and, for Ladder and SFC, the creation time) filled in.

use crate::js;
use crate::kind::VisualKind;

/// The file of a new program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewProgram {
    /// The file name: the name and the suffix of its kind.
    pub file_name: String,
    /// The file text.
    pub text: String,
}

const LADDER_TEMPLATE: &str = r#"{
  "schemaVersion": 2,
  "networks": [
    {
      "id": "rung_1",
      "order": 0,
      "nodes": [
        {
          "id": "contact_1",
          "type": "contact",
          "contactType": "NO",
          "variable": "Input1",
          "position": {
            "x": 100,
            "y": 100
          }
        },
        {
          "id": "coil_1",
          "type": "coil",
          "coilType": "NORMAL",
          "variable": "Output1",
          "position": {
            "x": 300,
            "y": 100
          }
        }
      ],
      "edges": [
        {
          "id": "edge_1",
          "fromNodeId": "contact_1",
          "toNodeId": "coil_1"
        }
      ],
      "layout": {
        "y": 100
      }
    }
  ],
  "variables": [
    {
      "name": "Input1",
      "type": "BOOL",
      "initialValue": "FALSE"
    },
    {
      "name": "Output1",
      "type": "BOOL",
      "initialValue": "FALSE"
    }
  ],
  "metadata": {
    "name": "{{name}}",
    "description": "Ladder Logic program",
    "version": "1.0.0",
    "created": "{{created}}"
  }
}
"#;

const SFC_TEMPLATE: &str = r#"{
  "name": "{{name}}",
  "steps": [
    {
      "id": "step_init",
      "name": "Init",
      "initial": true,
      "x": 200,
      "y": 50,
      "actions": []
    },
    {
      "id": "step_1",
      "name": "Step1",
      "x": 200,
      "y": 150,
      "actions": []
    }
  ],
  "transitions": [
    {
      "id": "trans_1",
      "name": "T1",
      "condition": "TRUE",
      "sourceStepId": "step_init",
      "targetStepId": "step_1"
    }
  ],
  "variables": [],
  "metadata": {
    "author": "",
    "version": "1.0.0",
    "description": "Sequential Function Chart program",
    "created": "{{created}}"
  }
}
"#;

const STATECHART_TEMPLATE: &str = r#"{
  "id": "{{name}}",
  "initial": "Initial",
  "states": {
    "Initial": {
      "entry": [],
      "exit": [],
      "on": {
        "START": {
          "target": "Running",
          "actions": []
        }
      }
    },
    "Running": {
      "entry": [],
      "exit": [],
      "on": {
        "STOP": {
          "target": "Initial",
          "actions": []
        }
      }
    }
  }
}
"#;

const BLOCKLY_TEMPLATE: &str = r#"{
  "blocks": {
    "languageVersion": 0,
    "blocks": [
      {
        "type": "controls_if",
        "id": "initial_block",
        "x": 50,
        "y": 50
      }
    ]
  },
  "variables": [],
  "metadata": {
    "name": "{{name}}",
    "description": "PLC program created with Blockly",
    "version": "1.0.0"
  }
}
"#;

/// The template of a kind; `{{name}}` and `{{created}}` are filled in.
fn template(kind: VisualKind) -> &'static str {
    match kind {
        VisualKind::Ladder => LADDER_TEMPLATE,
        VisualKind::Blockly => BLOCKLY_TEMPLATE,
        VisualKind::Statechart => STATECHART_TEMPLATE,
        VisualKind::Sfc => SFC_TEMPLATE,
    }
}

/// The name without surrounding space and without the suffix of its kind (any case).
fn base(kind: VisualKind, name: &str) -> &str {
    let trimmed = js::trim(name);
    let suffix = kind.suffix();
    let cut = trimmed.len().saturating_sub(suffix.len());
    if trimmed.len() > suffix.len()
        && trimmed.is_char_boundary(cut)
        && trimmed[cut..].eq_ignore_ascii_case(suffix)
    {
        &trimmed[..cut]
    } else {
        trimmed
    }
}

/// Why `name` cannot name a new program of `kind`, or `None` when it can.
#[must_use]
pub fn check_name(kind: VisualKind, name: &str) -> Option<String> {
    let title = kind.title();
    let base = base(kind, name);
    if base.is_empty() {
        return Some(format!("{title} program name is required."));
    }
    if base.contains(['/', '\\']) {
        return Some(format!(
            "{title} program name must not contain path separators."
        ));
    }
    if base == "." || base == ".." {
        return Some(format!("{title} program name is invalid."));
    }
    if !base
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Some(format!(
            "{title} program name must only contain letters, numbers, hyphens, and underscores."
        ));
    }
    None
}

/// A JSON string body (without its quotes) holding `text`.
fn json_text(text: &str) -> String {
    let quoted = crate::Json::str(text).to_compact();
    quoted[1..quoted.len() - 1].to_owned()
}

/// The file of a new program of `kind` called `name`, created at `created` (an ISO 8601 time).
///
/// # Errors
/// The name breaks the name rule ([`check_name`]).
pub fn new_program(kind: VisualKind, name: &str, created: &str) -> Result<NewProgram, String> {
    if let Some(problem) = check_name(kind, name) {
        return Err(problem);
    }
    let base = base(kind, name);
    let text = template(kind)
        .replace("{{name}}", &json_text(base))
        .replace("{{created}}", &json_text(created));
    Ok(NewProgram {
        file_name: format!("{base}{}", kind.suffix()),
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Json;

    #[test]
    fn one_name_rule_for_every_kind() {
        // Specification 25 §11.4: letters, digits, hyphens and underscores, the kind's suffix may
        // be typed and is cut. Catches the Statechart copy, which took spaces and quotes into a
        // file name and the template's JSON.
        assert_eq!(check_name(VisualKind::Ladder, "  conveyor-1_a "), None);
        assert_eq!(
            check_name(VisualKind::Statechart, "lights.statechart.json"),
            None
        );
        assert_eq!(
            check_name(VisualKind::Statechart, "my chart"),
            Some("Statechart program name must only contain letters, numbers, hyphens, and underscores.".to_owned())
        );
        assert_eq!(
            check_name(VisualKind::Sfc, " "),
            Some("SFC program name is required.".to_owned())
        );
        assert_eq!(
            check_name(VisualKind::Blockly, "a/b"),
            Some("Blockly program name must not contain path separators.".to_owned())
        );
        assert_eq!(
            check_name(VisualKind::Ladder, ".."),
            Some("Ladder program name is invalid.".to_owned())
        );
        assert_eq!(
            check_name(VisualKind::Ladder, ".ladder.json"),
            Some(
                "Ladder program name must only contain letters, numbers, hyphens, and underscores."
                    .to_owned()
            )
        );
    }

    #[test]
    fn a_new_ladder_program_is_a_valid_schema_v2_file() {
        // The template must pass the Ladder schema check (specification 16 §2), or the editor
        // opens it as "Invalid Ladder Program"; name and time are filled in once.
        let program =
            new_program(VisualKind::Ladder, "conveyor", "2026-10-26T10:00:00.000Z").expect("new");
        assert_eq!(program.file_name, "conveyor.ladder.json");
        let file = Json::parse(&program.text).expect("JSON");
        assert_eq!(crate::ladder::validate(&file), Ok(()));
        let metadata = file.get("metadata").expect("metadata");
        assert_eq!(metadata.str_at("name"), Some("conveyor"));
        assert_eq!(metadata.str_at("created"), Some("2026-10-26T10:00:00.000Z"));
        assert!(program.text.ends_with("}\n"));
    }

    #[test]
    fn every_template_is_json_that_its_companion_translates() {
        // Each new program must get a companion at once (it opens beside the editor). Catches a
        // template the generator refuses.
        for kind in VisualKind::ALL {
            let program = new_program(kind, "fresh", "T").expect("new program");
            assert_eq!(VisualKind::of_file(&program.file_name), Some(kind));
            let files = crate::companion::companion(&program.file_name, &program.text);
            assert!(files.is_ok(), "{kind:?}: {files:?}");
        }
        let chart =
            new_program(VisualKind::Statechart, "lights.statechart.json", "T").expect("chart");
        assert_eq!(chart.file_name, "lights.statechart.json");
        assert!(chart.text.starts_with("{\n  \"id\": \"lights\",\n"));
        assert!(new_program(VisualKind::Blockly, "bad name", "T").is_err());
    }
}
