//! The Structured Text companion and runtime entry of a visual program (specification 17 §3,
//! specification 25 §11.4).
//!
//! Every visual program executes as Structured Text: a companion `<base>.st` holding the program as
//! a `FUNCTION_BLOCK`, and a runtime entry `<base>.visual.runtime.st` holding a `PROGRAM` that calls
//! one instance of it and the `CONFIGURATION` that runs that program in a task every 20 ms. The
//! runtime entry also declares, as globals, the program's variables and the direct addresses it
//! names. The extension writes both files beside the program; it never edits their text.

use crate::kind::{base_name, companion_file_name, file_name, runtime_entry_file_name};
use crate::naming::{comment_text, fb_name_for_source, sanitize_identifier};
use crate::{Generated, Global, Json, Problem, VisualKind};

/// The task interval of every runtime entry (specification 17 §3).
pub const RUNTIME_ENTRY_INTERVAL: &str = "T#20ms";

/// The two files that execute a visual program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Companion {
    /// The kind of the program.
    pub kind: VisualKind,
    /// The companion's file name, beside the program: `<base>.st`.
    pub companion_file: String,
    /// The companion's text: a banner and the `FUNCTION_BLOCK`.
    pub companion_text: String,
    /// The runtime entry's file name, beside the program: `<base>.visual.runtime.st`.
    pub runtime_entry_file: String,
    /// The runtime entry's text.
    pub runtime_entry_text: String,
    /// Parts of the program the companion could not express.
    pub warnings: Vec<String>,
}

/// Why a file has no companion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionError {
    /// The file is not a visual program (its name has no visual suffix).
    NotVisual,
    /// The program cannot be translated; each problem says why.
    Invalid(VisualKind, Vec<Problem>),
}

/// The companion and runtime entry of the visual program `file` (a name or a path) whose text is
/// `text`.
///
/// # Errors
/// [`CompanionError::NotVisual`] when `file` is not a visual program, and
/// [`CompanionError::Invalid`] when its text is not JSON or the program cannot be translated.
pub fn companion(file: &str, text: &str) -> Result<Companion, CompanionError> {
    let kind = VisualKind::of_file(file).ok_or(CompanionError::NotVisual)?;
    let program = Json::parse(text)
        .map_err(|message| CompanionError::Invalid(kind, vec![Problem::new(message)]))?;
    let base = base_name(file);
    let generated = generate(kind, &program, &base)
        .map_err(|problems| CompanionError::Invalid(kind, problems))?;
    let source = file_name(file);
    Ok(Companion {
        kind,
        companion_file: companion_file_name(file),
        companion_text: companion_text(kind, source, &generated.function_block),
        runtime_entry_file: runtime_entry_file_name(file),
        runtime_entry_text: runtime_entry_text(kind, source, &base, &generated.globals),
        warnings: generated.warnings,
    })
}

/// The companion function block of a program of `kind` read from its file `program`.
///
/// # Errors
/// The program cannot be translated; each problem says why.
pub fn generate(kind: VisualKind, program: &Json, base: &str) -> Result<Generated, Vec<Problem>> {
    match kind {
        VisualKind::Ladder => crate::ladder::generate(program, base),
        VisualKind::Blockly => crate::blockly::generate(program, base),
        VisualKind::Statechart => crate::statechart::generate(program, base),
        VisualKind::Sfc => crate::sfc::generate(program, base),
    }
}

/// The companion file: a banner naming the source, then the function block and one final newline.
fn companion_text(kind: VisualKind, source: &str, function_block: &str) -> String {
    format!(
        "(*\n  Auto-generated from {} source: {}\n  Source of truth is the visual file unless migrated fully to ST.\n*)\n{}\n",
        kind.name(),
        comment_text(source),
        function_block.trim_end()
    )
}

/// The runtime entry file (specification 17 §3): a `PROGRAM` calling one instance of the
/// companion and a `CONFIGURATION` declaring that instance and the program's globals and running
/// the program in one task.
fn runtime_entry_text(kind: VisualKind, source: &str, base: &str, globals: &[Global]) -> String {
    let base_id = sanitize_identifier(base, "Visual");
    let block = fb_name_for_source(base, kind.block_suffix());
    let program = sanitize_identifier(&format!("PRG_{base_id}_VISUAL"), "PRG_Visual");
    let instance = sanitize_identifier(&format!("fb_{base_id}"), "fb_visual");
    let configuration = sanitize_identifier(&format!("CFG_{base_id}_VISUAL"), "CFG_Visual");
    let resource = sanitize_identifier(&format!("RES_{base_id}_VISUAL"), "RES_Visual");
    let task = sanitize_identifier(&format!("TASK_{base_id}_VISUAL"), "TASK_Visual");
    let program_instance = sanitize_identifier(&format!("PLC_PRG_{base_id}"), "PLC_PRG_VISUAL");

    let mut lines =
        vec![
        "(*".to_owned(),
        format!("  Auto-generated runtime entry for {}.", comment_text(source)),
        "  This wrapper is used to run visual editors through the Structured Text debugger path."
            .to_owned(),
        "*)".to_owned(),
        String::new(),
        format!("PROGRAM {program}"),
        format!("{instance}();"),
        "END_PROGRAM".to_owned(),
        String::new(),
        format!("CONFIGURATION {configuration}"),
        "VAR_GLOBAL".to_owned(),
        format!("  {instance} : {block};"),
    ];
    lines.extend(globals.iter().map(Global::declaration));
    lines.extend([
        "END_VAR".to_owned(),
        format!("  RESOURCE {resource} ON PLC"),
        format!("    TASK {task}(INTERVAL := {RUNTIME_ENTRY_INTERVAL}, PRIORITY := 1);"),
        format!("    PROGRAM {program_instance} WITH {task} : {program};"),
        "  END_RESOURCE".to_owned(),
        "END_CONFIGURATION".to_owned(),
        String::new(),
    ]);
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ladder_program_gets_the_runtime_entry_of_specification_17() {
        // Worked from specification 17 §3: the PROGRAM calls the instance, the CONFIGURATION
        // declares it and the addressed globals, one task every 20 ms. Catches a wrapper that
        // declares a local VAR in the PROGRAM, drops a global or changes the interval.
        let text = r#"{
          "schemaVersion": 2,
          "networks": [],
          "variables": [
            { "name": "StartPB", "scope": "global", "type": "BOOL", "address": "%IX0.0" },
            { "name": "AlarmLamp", "scope": "global", "type": "BOOL", "address": "%QX0.1" },
            { "name": "RunLatch", "scope": "local", "type": "BOOL", "initialValue": false }
          ],
          "metadata": { "name": "simple-start-stop", "description": "test" }
        }"#;
        let files = companion("/tmp/demo/simple-start-stop.ladder.json", text).expect("companion");
        assert_eq!(files.kind, VisualKind::Ladder);
        assert_eq!(files.companion_file, "simple-start-stop.st");
        assert_eq!(
            files.runtime_entry_file,
            "simple-start-stop.visual.runtime.st"
        );
        assert_eq!(
            files.runtime_entry_text,
            "(*\n  Auto-generated runtime entry for simple-start-stop.ladder.json.\n  This wrapper is used to run visual editors through the Structured Text debugger path.\n*)\n\nPROGRAM PRG_simple_start_stop_VISUAL\nfb_simple_start_stop();\nEND_PROGRAM\n\nCONFIGURATION CFG_simple_start_stop_VISUAL\nVAR_GLOBAL\n  fb_simple_start_stop : FB_simple_start_stop_LADDER;\n  StartPB AT %IX0.0 : BOOL;\n  AlarmLamp AT %QX0.1 : BOOL;\nEND_VAR\n  RESOURCE RES_simple_start_stop_VISUAL ON PLC\n    TASK TASK_simple_start_stop_VISUAL(INTERVAL := T#20ms, PRIORITY := 1);\n    PROGRAM PLC_PRG_simple_start_stop WITH TASK_simple_start_stop_VISUAL : PRG_simple_start_stop_VISUAL;\n  END_RESOURCE\nEND_CONFIGURATION\n"
        );
        assert!(files.companion_text.starts_with(
            "(*\n  Auto-generated from ladder source: simple-start-stop.ladder.json\n  Source of truth is the visual file unless migrated fully to ST.\n*)\n(*\n  Auto-generated Ladder Diagram companion (schema v2)."
        ));
        assert!(files.companion_text.ends_with("END_FUNCTION_BLOCK\n"));
    }

    #[test]
    fn the_checked_in_ladder_example_is_reproduced_exactly() {
        // `examples/ladder/simple-start-stop.st` and `.visual.runtime.st` were written by the
        // TypeScript generator; its networks have one writing node each, so no decision of the
        // move changes them. Catches any drift of the companion or runtime entry text.
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/ladder/");
        let source = std::fs::read_to_string(format!("{root}simple-start-stop.ladder.json"))
            .expect("example source");
        let files = companion("simple-start-stop.ladder.json", &source).expect("companion");
        let companion_st =
            std::fs::read_to_string(format!("{root}simple-start-stop.st")).expect("companion");
        let entry_st =
            std::fs::read_to_string(format!("{root}simple-start-stop.visual.runtime.st"))
                .expect("runtime entry");
        assert_eq!(files.companion_text, companion_st);
        assert_eq!(files.runtime_entry_text, entry_st);
    }

    #[test]
    fn a_statechart_runtime_entry_declares_the_addresses_of_its_companion() {
        // Specification 25 §11.4: the runtime entry declares each direct address the companion
        // names, typed by its size (specification 03 §5). Catches the old scan of the raw JSON,
        // which also declared addresses that only a description mentioned.
        let text = r#"{
          "id": "traffic",
          "initial": "Idle",
          "states": { "Idle": { "on": { "START": { "target": "Run", "actions": ["turnOn"] } } }, "Run": {} },
          "actionMappings": { "turnOn": { "action": "WRITE_OUTPUT", "address": "%QX0.0", "value": true, "message": "not %QW9" } }
        }"#;
        let files = companion("traffic.statechart.json", text).expect("companion");
        assert!(files
            .runtime_entry_text
            .contains("  ld_io_QX0_0 AT %QX0.0 : BOOL;\n"));
        assert!(!files.runtime_entry_text.contains("%QW9"));
        assert!(files.companion_text.contains("ld_io_QX0_0 := TRUE;"));
    }

    #[test]
    fn every_checked_in_visual_example_gets_a_companion_and_runtime_entry() {
        // The examples are the programs users open first; each must translate (specification 17
        // §3). Catches an example whose operands the generator refuses, such as an undeclared
        // Ladder symbol.
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/");
        let mut translated = 0;
        for folder in ["ladder", "blockly", "statecharts", "sfc"] {
            let entries = std::fs::read_dir(format!("{root}{folder}")).expect("examples folder");
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path.to_string_lossy().into_owned();
                if VisualKind::of_file(&name).is_none() {
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("example");
                let files =
                    companion(&name, &text).unwrap_or_else(|error| panic!("{name}: {error:?}"));
                assert!(
                    files.companion_text.contains("FUNCTION_BLOCK FB_"),
                    "{name}"
                );
                assert!(
                    files.runtime_entry_text.contains("\nPROGRAM PRG_"),
                    "{name}"
                );
                translated += 1;
            }
        }
        assert!(translated >= 10, "only {translated} examples found");
    }

    #[test]
    fn a_file_that_is_no_visual_program_or_no_json_has_no_companion() {
        assert_eq!(
            companion("package.json", "{}"),
            Err(CompanionError::NotVisual)
        );
        match companion("a.ladder.json", "{") {
            Err(CompanionError::Invalid(VisualKind::Ladder, problems)) => {
                assert!(problems[0].message.starts_with("Invalid JSON at line 1"));
            }
            other => panic!("expected invalid JSON, got {other:?}"),
        }
    }
}
