use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use trust_hir::db::{Database, FileId, SemanticDatabase, SourceDatabase};
use trust_hir::diagnostics::{DiagnosticCode, DiagnosticSeverity};
use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::value::Value;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

const TEST_ID: &str = "TEST_IEC_STRING_BINDING_TRACE_001";
const CASE_FILE: &str = "verification/cases/compiler_iec/IEC_STRING_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:f88e46163f6a6367919bcdd5f74e16d5dd6990d2e7b698d4346f90aaf98aa289";

#[test]
fn iec_string_binding_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let mut probe = StringBindingProbe::default();
    let config = RunConfig::new(TEST_ID, workspace.join(CASE_FILE), CASE_FILE_DIGEST);
    let artifact = run_case_file(&config, &mut probe, run_string_case)
        .expect("STRING binding case artifact must be written");
    let failed = artifact
        .cases
        .iter()
        .filter(|case| case.result != CaseResult::Passed)
        .map(|case| {
            format!(
                "{}: {}",
                case.id,
                case.observed_error.as_deref().unwrap_or("not passed")
            )
        })
        .collect::<Vec<_>>();
    assert!(
        failed.is_empty(),
        "STRING binding failures: {}",
        failed.join("; ")
    );
}

fn run_string_case(
    case: &CaseRecord,
    probe: &mut StringBindingProbe,
) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    let failure = match scenario {
        "HIR_INOUT_EQUAL_ALIAS" => hir_equal_alias(probe),
        "HIR_INOUT_MISMATCH" => hir_inout_mismatch(probe),
        "STRING_WSTRING_CROSS_FAMILY" => hir_cross_family(probe),
        "IMPORTED_CAPACITY_EQUAL" => imported_capacity(probe, false),
        "IMPORTED_CAPACITY_MISMATCH" => imported_capacity(probe, true),
        "FB_OUTPUT_COPYBACK_STRING" => run_runtime(
            probe,
            FB_OUTPUT_COPYBACK_STRING,
            &[("narrow", ExpectedValue::String("ABCDE"))],
        ),
        "FUNCTION_OUTPUT_COPYBACK_STRING" => run_runtime(
            probe,
            FUNCTION_OUTPUT_COPYBACK_STRING,
            &[("narrow", ExpectedValue::String("ABCDE"))],
        ),
        "LOCAL_OUTPUT_COPYBACK_STRING" => run_runtime(
            probe,
            LOCAL_OUTPUT_COPYBACK_STRING,
            &[("observed_length", ExpectedValue::DInt(5))],
        ),
        "LOCAL_ALIAS_COPYBACK_WSTRING" => run_runtime(
            probe,
            LOCAL_ALIAS_COPYBACK_WSTRING,
            &[("observed_length", ExpectedValue::DInt(3))],
        ),
        "FB_INOUT_MISMATCH_STRING" => {
            compile_reject(probe, FB_INOUT_MISMATCH_STRING, "error[E205]:")
        }
        "FB_OUTPUT_COPYBACK_WSTRING" => run_runtime(
            probe,
            FB_OUTPUT_COPYBACK_WSTRING,
            &[("narrow", ExpectedValue::WString("ABC"))],
        ),
        "FB_INOUT_MISMATCH_WSTRING" => {
            compile_reject(probe, FB_INOUT_MISMATCH_WSTRING, "error[E205]:")
        }
        "UNICODE_LITERAL_BOUND" => run_runtime(
            probe,
            UNICODE_LITERAL_BOUND,
            &[
                ("narrow", ExpectedValue::String("Å")),
                ("wide", ExpectedValue::WString("Å")),
            ],
        ),
        "UNICODE_ASSIGNMENT_BOUND" => run_runtime(
            probe,
            UNICODE_ASSIGNMENT_BOUND,
            &[
                ("narrow", ExpectedValue::String("ÅB")),
                ("wide_narrow", ExpectedValue::WString("ÅB")),
            ],
        ),
        "FUNCTION_RECEIVER_BOUND" => run_runtime(
            probe,
            FUNCTION_RECEIVER_BOUND,
            &[("narrow", ExpectedValue::String("ABCDE"))],
        ),
        "FUNCTION_RESULT_BOUND" => run_runtime(
            probe,
            FUNCTION_RESULT_BOUND,
            &[
                ("result", ExpectedValue::String("ABCDE")),
                ("wide_result", ExpectedValue::WString("ABC")),
            ],
        ),
        "FUNCTION_INPUT_EQUAL_INOUT" => run_runtime(
            probe,
            FUNCTION_INPUT_EQUAL_INOUT,
            &[
                ("observed_length", ExpectedValue::DInt(5)),
                ("shared", ExpectedValue::String("ABZ")),
            ],
        ),
        "FUNCTION_INOUT_MISMATCH" => compile_reject(probe, FUNCTION_INOUT_MISMATCH, "error[E205]:"),
        "NESTED_FIELD_COPYBACK" => run_runtime(
            probe,
            NESTED_FIELD_COPYBACK,
            &[("observed", ExpectedValue::String("ABCDE"))],
        ),
        "FB_INPUT_EQUAL_INOUT" => run_runtime(
            probe,
            FB_INPUT_EQUAL_INOUT,
            &[
                ("observed", ExpectedValue::String("ABCDE")),
                ("shared", ExpectedValue::String("ABZ")),
            ],
        ),
        other => return Err(format!("unreviewed STRING binding scenario {other}")),
    };
    Ok(CaseExecution {
        result: if failure.is_none() {
            CaseResult::Passed
        } else {
            CaseResult::Failed
        },
        observed_error: failure,
        observed_status: Some(
            if probe.diagnostics.is_empty() {
                "binding_value_match"
            } else {
                "binding_rejection_match"
            }
            .to_string(),
        ),
    })
}

fn run_runtime(
    probe: &mut StringBindingProbe,
    source: &str,
    expected: &[(&str, ExpectedValue)],
) -> Option<String> {
    let mut harness = match TestHarness::from_source(source) {
        Ok(harness) => harness,
        Err(error) => return Some(format!("runtime fixture failed to compile: {error}")),
    };
    let cycle = harness.cycle();
    if !cycle.errors.is_empty() {
        return Some(format!("runtime fixture errors: {:?}", cycle.errors));
    }
    let mut observed = serde_json::Map::new();
    let mut mismatches = Vec::new();
    for (name, expected_value) in expected {
        let actual = harness.get_output(name);
        observed.insert(
            (*name).to_string(),
            serde_json::json!(format!("{actual:?}")),
        );
        if !expected_value.matches(actual.as_ref()) {
            mismatches.push(format!(
                "{name} expected {}, observed {actual:?}",
                expected_value.describe()
            ));
        }
    }
    probe.target = Some(serde_json::Value::Object(observed));
    (!mismatches.is_empty()).then(|| mismatches.join("; "))
}

fn compile_reject(probe: &mut StringBindingProbe, source: &str, expected: &str) -> Option<String> {
    match CompileSession::from_source(source).build_runtime() {
        Ok(_) => Some(format!("fixture compiled but expected {expected}")),
        Err(error) => {
            let rendered = error.to_string();
            probe.diagnostics.push(rendered.clone());
            (!rendered.contains(expected))
                .then(|| format!("expected rejection containing {expected:?}, got {rendered}"))
        }
    }
}

fn hir_errors(source: &str) -> Vec<DiagnosticCode> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn hir_equal_alias(probe: &mut StringBindingProbe) -> Option<String> {
    let errors = hir_errors(HIR_INOUT_EQUAL_ALIAS);
    probe.diagnostics = errors.iter().map(|code| format!("{code:?}")).collect();
    (!errors.is_empty()).then(|| format!("equal alias binding errors: {errors:?}"))
}

fn hir_inout_mismatch(probe: &mut StringBindingProbe) -> Option<String> {
    let errors = hir_errors(HIR_INOUT_MISMATCH);
    probe.diagnostics = errors.iter().map(|code| format!("{code:?}")).collect();
    let count = errors
        .iter()
        .filter(|code| **code == DiagnosticCode::InvalidArgumentType)
        .count();
    (count != 3).then(|| format!("expected 3 InvalidArgumentType errors, got {errors:?}"))
}

fn hir_cross_family(probe: &mut StringBindingProbe) -> Option<String> {
    let errors = hir_errors(HIR_CROSS_FAMILY);
    probe.diagnostics = errors.iter().map(|code| format!("{code:?}")).collect();
    let count = errors
        .iter()
        .filter(|code| **code == DiagnosticCode::IncompatibleAssignment)
        .count();
    (count != 2).then(|| format!("expected 2 IncompatibleAssignment errors, got {errors:?}"))
}

fn imported_capacity(probe: &mut StringBindingProbe, mismatch: bool) -> Option<String> {
    let mut db = Database::new();
    let globals = FileId(32);
    let library = FileId(33);
    let main = FileId(34);
    db.set_source_text(
        globals,
        "VAR_GLOBAL CONSTANT\n    STRING_LENGTH : INT := INT#12;\nEND_VAR\n".to_string(),
    );
    db.set_source_text(
        library,
        "FUNCTION Touch : BOOL\nVAR_IN_OUT\n    Text : STRING[STRING_LENGTH];\nEND_VAR\nTouch := TRUE;\nEND_FUNCTION\n\nFUNCTION_BLOCK Bind\nVAR_IN_OUT\n    Text : STRING[STRING_LENGTH];\nEND_VAR\nEND_FUNCTION_BLOCK\n".to_string(),
    );
    let main_source = if mismatch {
        "PROGRAM Main\nVAR text : STRING[11]; ok : BOOL; END_VAR\nok := Touch(Text := text);\nEND_PROGRAM\n"
    } else {
        "PROGRAM Main\nVAR fb : Bind; text : STRING[STRING_LENGTH]; ok : BOOL; END_VAR\nok := Touch(Text := text);\nfb(Text := text);\nEND_PROGRAM\n"
    };
    db.set_source_text(main, main_source.to_string());
    let analysis = db.analyze(main);
    let errors = analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();
    probe.diagnostics = errors.iter().map(|code| format!("{code:?}")).collect();
    if mismatch {
        let count = errors
            .iter()
            .filter(|code| **code == DiagnosticCode::InvalidArgumentType)
            .count();
        (count != 1).then(|| format!("expected imported-capacity rejection, got {errors:?}"))
    } else {
        (!errors.is_empty()).then(|| format!("equal imported capacity errors: {errors:?}"))
    }
}

#[derive(Clone, Copy)]
enum ExpectedValue {
    DInt(i32),
    String(&'static str),
    WString(&'static str),
}

impl ExpectedValue {
    fn matches(self, actual: Option<&Value>) -> bool {
        match (self, actual) {
            (Self::DInt(expected), Some(Value::DInt(actual))) => expected == *actual,
            (Self::String(expected), Some(Value::String(actual))) => expected == actual.as_str(),
            (Self::WString(expected), Some(Value::WString(actual))) => expected == actual.as_str(),
            _ => false,
        }
    }

    fn describe(self) -> String {
        match self {
            Self::DInt(value) => format!("DINT#{value}"),
            Self::String(value) => format!("STRING {value:?}"),
            Self::WString(value) => format!("WSTRING {value:?}"),
        }
    }
}

#[derive(Default)]
struct StringBindingProbe {
    diagnostics: Vec<String>,
    target: Option<serde_json::Value>,
    next_snapshot_is_after: bool,
}

impl StateProbe for StringBindingProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        if !self.next_snapshot_is_after {
            self.diagnostics.clear();
            self.target = None;
        }
        self.next_snapshot_is_after = !self.next_snapshot_is_after;
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: None,
            target: self.target.clone(),
            siblings: BTreeMap::new(),
            diagnostics: self.diagnostics.clone(),
        })
    }
}

const HIR_INOUT_EQUAL_ALIAS: &str = r#"
TYPE ShortText : STRING[5]; ShortWideText : WSTRING[5]; END_TYPE
FUNCTION_BLOCK Bind
VAR_IN_OUT narrow : STRING[5]; wide_narrow : WSTRING[5]; END_VAR
END_FUNCTION_BLOCK
PROGRAM Test
VAR fb : Bind; text : ShortText; wide_text : ShortWideText; END_VAR
fb(narrow := text, wide_narrow := wide_text);
END_PROGRAM
"#;

const HIR_INOUT_MISMATCH: &str = r#"
FUNCTION_BLOCK Bind
VAR_IN_OUT narrow : STRING[5]; wide_narrow : WSTRING[5]; END_VAR
END_FUNCTION_BLOCK
PROGRAM Test
VAR fb : Bind; text : STRING[20]; unbounded_text : STRING; wide_text : WSTRING[20]; wide_equal : WSTRING[5]; END_VAR
fb(narrow := text, wide_narrow := wide_text);
fb(narrow := unbounded_text, wide_narrow := wide_equal);
END_PROGRAM
"#;

const HIR_CROSS_FAMILY: &str = r#"
PROGRAM Main
VAR text : STRING[5]; wide : WSTRING[5]; END_VAR
text := wide;
wide := text;
END_PROGRAM
"#;

const FB_OUTPUT_COPYBACK_STRING: &str = r#"
FUNCTION_BLOCK Producer VAR_OUTPUT text : STRING[20]; END_VAR text := 'ABCDEFGHIJKLMNOPQRST'; END_FUNCTION_BLOCK
PROGRAM Main VAR fb : Producer; narrow : STRING[5] := 'OLD'; END_VAR fb(text => narrow); END_PROGRAM
"#;

const FUNCTION_OUTPUT_COPYBACK_STRING: &str = r#"
FUNCTION Produce : BOOL VAR_OUTPUT text : STRING[20]; END_VAR text := 'ABCDEFGHIJKLMNOPQRST'; Produce := TRUE; END_FUNCTION
PROGRAM Main VAR narrow : STRING[5] := 'OLD'; END_VAR Produce(narrow); END_PROGRAM
"#;

const LOCAL_OUTPUT_COPYBACK_STRING: &str = r#"
FUNCTION Produce : BOOL VAR_OUTPUT text : STRING[20]; END_VAR text := 'ABCDEFGHIJKLMNOPQRST'; Produce := TRUE; END_FUNCTION
FUNCTION CallProduce : DINT VAR narrow : STRING[5] := 'OLD'; END_VAR Produce(narrow); CallProduce := LEN(narrow); END_FUNCTION
PROGRAM Main VAR observed_length : DINT; END_VAR observed_length := CallProduce(); END_PROGRAM
"#;

const LOCAL_ALIAS_COPYBACK_WSTRING: &str = r#"
TYPE NarrowWideText : WSTRING[3]; END_TYPE
FUNCTION ProduceWide : BOOL VAR_OUTPUT text : WSTRING[6]; END_VAR text := "ABCDEF"; ProduceWide := TRUE; END_FUNCTION
FUNCTION CallProduceWide : DINT VAR narrow : NarrowWideText := "OLD"; END_VAR ProduceWide(narrow); CallProduceWide := LEN(narrow); END_FUNCTION
PROGRAM Main VAR observed_length : DINT; END_VAR observed_length := CallProduceWide(); END_PROGRAM
"#;

const FB_INOUT_MISMATCH_STRING: &str = r#"
FUNCTION_BLOCK Observe VAR_IN_OUT text : STRING[5]; END_VAR END_FUNCTION_BLOCK
PROGRAM Main VAR fb : Observe; caller_text : STRING[20] := 'ABCDEFGHIJKLMNOPQRST'; END_VAR fb(text := caller_text); END_PROGRAM
"#;

const FB_OUTPUT_COPYBACK_WSTRING: &str = r#"
FUNCTION_BLOCK Producer VAR_OUTPUT text : WSTRING[6]; END_VAR text := "ABCDEF"; END_FUNCTION_BLOCK
PROGRAM Main VAR fb : Producer; narrow : WSTRING[3] := "OLD"; END_VAR fb(text => narrow); END_PROGRAM
"#;

const FB_INOUT_MISMATCH_WSTRING: &str = r#"
FUNCTION_BLOCK Observe VAR_IN_OUT text : WSTRING[3]; END_VAR END_FUNCTION_BLOCK
PROGRAM Main VAR fb : Observe; caller_text : WSTRING[6] := "ABCDEF"; END_VAR fb(text := caller_text); END_PROGRAM
"#;

const UNICODE_LITERAL_BOUND: &str = r#"
PROGRAM Main VAR narrow : STRING[1] := 'Å'; wide : WSTRING[1] := "Å"; END_VAR END_PROGRAM
"#;

const UNICODE_ASSIGNMENT_BOUND: &str = r#"
PROGRAM Main
VAR source : STRING[3] := 'ÅBC'; wide_source : WSTRING[3] := "ÅBC"; narrow : STRING[2]; wide_narrow : WSTRING[2]; END_VAR
narrow := source; wide_narrow := wide_source;
END_PROGRAM
"#;

const FUNCTION_RECEIVER_BOUND: &str = r#"
FUNCTION ProduceText : STRING[20] ProduceText := 'ABCDEFGHIJKLMNOPQRST'; END_FUNCTION
PROGRAM Main VAR narrow : STRING[5]; END_VAR narrow := ProduceText(); END_PROGRAM
"#;

const FUNCTION_RESULT_BOUND: &str = r#"
FUNCTION BoundText : STRING[5] VAR_INPUT source : STRING[20]; END_VAR BoundText := source; END_FUNCTION
FUNCTION BoundWideText : WSTRING[3] VAR_INPUT source : WSTRING[6]; END_VAR BoundWideText := source; END_FUNCTION
PROGRAM Main
VAR source : STRING[20] := 'ABCDEFGHIJKLMNOPQRST'; wide_source : WSTRING[6] := "ABCDEF"; result : STRING[20]; wide_result : WSTRING[6]; END_VAR
result := BoundText(source); wide_result := BoundWideText(wide_source);
END_PROGRAM
"#;

const FUNCTION_INPUT_EQUAL_INOUT: &str = r#"
FUNCTION EchoText : DINT VAR_INPUT input_text : STRING[5]; END_VAR VAR_IN_OUT shared_text : STRING[5]; END_VAR
shared_text := CONCAT(shared_text, 'Z'); EchoText := LEN(input_text); END_FUNCTION
PROGRAM Main VAR long_text : STRING[20] := 'ABCDEFGHIJKLMNOPQRST'; shared : STRING[5] := 'AB'; observed_length : DINT; END_VAR
observed_length := EchoText(input_text := long_text, shared_text := shared); END_PROGRAM
"#;

const FUNCTION_INOUT_MISMATCH: &str = r#"
FUNCTION ObserveText : BOOL VAR_IN_OUT text : STRING[5]; END_VAR ObserveText := TRUE; END_FUNCTION
PROGRAM Main VAR caller_text : STRING[20] := 'ABCDEFGHIJKLMNOPQRST'; accepted : BOOL; END_VAR accepted := ObserveText(caller_text); END_PROGRAM
"#;

const NESTED_FIELD_COPYBACK: &str = r#"
TYPE TextHolder : STRUCT text : STRING[5]; END_STRUCT END_TYPE
FUNCTION ProduceText : BOOL VAR_OUTPUT text : STRING[20]; END_VAR text := 'ABCDEFGHIJKLMNOPQRST'; ProduceText := TRUE; END_FUNCTION
PROGRAM Main VAR holder : TextHolder; observed : STRING[5]; END_VAR ProduceText(holder.text); observed := holder.text; END_PROGRAM
"#;

const FB_INPUT_EQUAL_INOUT: &str = r#"
FUNCTION_BLOCK Echo
VAR_INPUT input_text : STRING[5]; END_VAR VAR_IN_OUT shared_text : STRING[5]; END_VAR VAR_OUTPUT observed_input : STRING[5]; END_VAR
observed_input := input_text; shared_text := CONCAT(shared_text, 'Z'); END_FUNCTION_BLOCK
PROGRAM Main VAR fb : Echo; long_text : STRING[20] := 'ABCDEFGHIJKLMNOPQRST'; shared : STRING[5] := 'AB'; observed : STRING[5]; END_VAR
fb(input_text := long_text, shared_text := shared, observed_input => observed); END_PROGRAM
"#;
