use serde_json::{json, Value as JsonValue};
use text_size::TextSize;
use trust_hir::db::{FileId, SourceDatabase};
use trust_hir::Database;
use trust_ide::rename::{rename_checked, RenameError, RenameResult};
use verification_cases::{CaseRecord, TraceStep};

use super::trace_support::{run_trace_cases, scenario, trace_string};

const LOCAL_TEST_ID: &str = "TEST_EDITOR_RENAME_LOCAL_TRACE_001";
const LOCAL_CASE_FILE: &str = "verification/cases/editor_safety/EDIT_RENAME_001.toml";
const LOCAL_CASE_FILE_DIGEST: &str =
    "sha256:c1bc6d86827343289709f5ebb207ac334a92c19b3da917870cce86d1b8618468";
const PROJECT_TEST_ID: &str = "TEST_EDITOR_RENAME_PROJECT_TRACE_001";
const PROJECT_CASE_FILE: &str = "verification/cases/editor_safety/EDIT_RENAME_002.toml";
const PROJECT_CASE_FILE_DIGEST: &str =
    "sha256:2645d42c25592c5244f591759c3c3415345ce112ed5095c176e6d0c533234320";

#[test]
fn editor_rename_local_trace_cases() {
    run_trace_cases(
        LOCAL_TEST_ID,
        LOCAL_CASE_FILE,
        LOCAL_CASE_FILE_DIGEST,
        execute_local_case,
    );
}

#[test]
fn editor_rename_project_trace_cases() {
    run_trace_cases(
        PROJECT_TEST_ID,
        PROJECT_CASE_FILE,
        PROJECT_CASE_FILE_DIGEST,
        execute_project_case,
    );
}

fn execute_local_case(case: &CaseRecord, step: &TraceStep) -> Result<JsonValue, String> {
    let replacement = trace_string(step, "replacement")?;
    match scenario(case)? {
        "DECLARING_SCOPE_COLLISION" | "CASE_INSENSITIVE_SCOPE_COLLISION" => {
            let source = "VAR_GLOBAL\n    Speed : INT;\n    Limit : INT;\nEND_VAR\n";
            observe_rename(single_file(source, "Speed : INT", &replacement))
        }
        "CASE_ONLY_SELF_RENAME" => {
            let source = "PROGRAM Main\nVAR\n    Speed : INT;\nEND_VAR\nSpeed := 1;\nEND_PROGRAM\n";
            observe_rename(single_file(source, "Speed : INT", &replacement))
        }
        "STRUCT_FIELD_COLLISION" => {
            let source = "TYPE MotorState : STRUCT\n    Speed : DINT;\n    Limit : DINT;\nEND_STRUCT\nEND_TYPE\n\nPROGRAM Main\nVAR\n    State : MotorState;\nEND_VAR\nState.Speed := 1;\nEND_PROGRAM\n";
            observe_rename(single_file(source, "Speed : DINT", &replacement))
        }
        "INVALID_IDENTIFIER" | "RESERVED_KEYWORD" => {
            let source = "PROGRAM Main\nVAR\n    Speed : INT;\nEND_VAR\nEND_PROGRAM\n";
            observe_rename(single_file(source, "Speed : INT", &replacement))
        }
        other => Err(format!("unreviewed local rename scenario {other}")),
    }
}

fn execute_project_case(case: &CaseRecord, step: &TraceStep) -> Result<JsonValue, String> {
    let replacement = trace_string(step, "replacement")?;
    match scenario(case)? {
        "IMPORTED_ORIGIN_COLLISION" => {
            let globals = "VAR_GLOBAL\n    Speed : INT;\n    Limit : INT;\nEND_VAR\n";
            let main =
                "PROGRAM Main\nVAR\n    Value : INT;\nEND_VAR\nValue := Speed;\nEND_PROGRAM\n";
            let mut db = Database::new();
            db.set_source_text(FileId(0), globals.to_string());
            db.set_source_text(FileId(1), main.to_string());
            let position = TextSize::from(main.find("Speed;").expect("imported use") as u32);
            observe_rename(rename_checked(&db, FileId(1), position, &replacement))
        }
        "PROJECT_POU_COLLISION" => {
            let primary = "FUNCTION_BLOCK LevelController\nEND_FUNCTION_BLOCK\n";
            let conflict = "FUNCTION_BLOCK BackupController\nEND_FUNCTION_BLOCK\n";
            let mut db = Database::new();
            db.set_source_text(FileId(0), primary.to_string());
            db.set_source_text(FileId(1), conflict.to_string());
            let position = TextSize::from(primary.find("LevelController").unwrap() as u32);
            observe_rename(rename_checked(&db, FileId(0), position, &replacement))
        }
        "CROSS_FILE_REFERENCE_CAPTURE" => {
            let globals = "VAR_GLOBAL\n    Speed : INT;\nEND_VAR\n";
            let main = "PROGRAM Main\nVAR\n    Temp : INT;\n    Observed : INT;\nEND_VAR\nObserved := Speed;\nEND_PROGRAM\n";
            observe_cross_file(globals, main, &replacement)
        }
        "SAFE_CROSS_FILE_RENAME" => {
            let globals = "VAR_GLOBAL\n    Speed : INT;\nEND_VAR\n";
            let main = "PROGRAM Main\nVAR\n    Observed : INT;\nEND_VAR\nObserved := Speed;\nEND_PROGRAM\n";
            observe_cross_file(globals, main, &replacement)
        }
        other => Err(format!("unreviewed project rename scenario {other}")),
    }
}

fn single_file(source: &str, needle: &str, replacement: &str) -> Result<RenameResult, RenameError> {
    let mut db = Database::new();
    db.set_source_text(FileId(0), source.to_string());
    let position = TextSize::from(source.find(needle).expect("rename target") as u32);
    rename_checked(&db, FileId(0), position, replacement)
}

fn observe_cross_file(
    declaration: &str,
    usage: &str,
    replacement: &str,
) -> Result<JsonValue, String> {
    let mut db = Database::new();
    db.set_source_text(FileId(0), declaration.to_string());
    db.set_source_text(FileId(1), usage.to_string());
    let position = TextSize::from(declaration.find("Speed : INT").unwrap() as u32);
    observe_rename(rename_checked(&db, FileId(0), position, replacement))
}

fn observe_rename(result: Result<RenameResult, RenameError>) -> Result<JsonValue, String> {
    match result {
        Ok(result) => {
            let replacement = result
                .edits
                .values()
                .flatten()
                .next()
                .map(|edit| edit.new_text.clone())
                .unwrap_or_default();
            Ok(json!({
                "accepted": true,
                "edit_count": result.edit_count(),
                "error": "none",
                "replacement": replacement,
            }))
        }
        Err(error) => Ok(json!({
            "accepted": false,
            "edit_count": 0,
            "error": rename_error_name(&error),
        })),
    }
}

fn rename_error_name(error: &RenameError) -> &'static str {
    match error {
        RenameError::NoTarget => "no_target",
        RenameError::InvalidIdentifier { .. } => "invalid_identifier",
        RenameError::ReservedKeyword { .. } => "reserved_keyword",
        RenameError::UnsupportedNamespaceRename { .. } => "unsupported_namespace_rename",
        RenameError::DeclaringScopeConflict { .. } => "declaring_scope_conflict",
        RenameError::CrossFileImportedSymbolConflict { .. } => {
            "cross_file_imported_symbol_conflict"
        }
        RenameError::ReferenceSiteRebind { .. } => "reference_site_rebind",
    }
}
