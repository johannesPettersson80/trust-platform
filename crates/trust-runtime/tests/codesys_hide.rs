//! Specification 10 §4.8: a variable marked `{attribute 'hide'}`, and a `VAR` local of a POU marked
//! `{attribute 'hide_all_locals'}`, is left out of the debugger's lists of variables, as CODESYS
//! hides it from online monitoring and debugging, and code and an explicit name still reach it. The
//! lists come from the program image (specification 44 §5.7, §5.8), so a board lists the same.
//! Every expected value is worked out by hand from the program below.
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value as Json};
use trust_runtime::debug::DebugControl;
use trust_runtime::harness::SourceFile;
use trust_runtime::native::{build as build_image, Options, Target, Toolchain};
use trust_runtime::value::Value;
use trust_runtime::{CompiledDebugger, CompiledRuntime};
use trust_runtime_services::live_values::io_requests::IoSnapshotRead;
use trust_runtime_services::protocol::ControlRequest;

const PATH: &str = "src/main.st";

const WAIT: Duration = Duration::from_secs(10);

/// A global, a block's input and local, a program's variable, each once shown and once hidden, and a
/// block whose `VAR` locals `hide_all_locals` hides while its input stays shown.
const SOURCE: &str = r#"
VAR_GLOBAL
    gShown : DINT := 1;
    {attribute 'hide'}
    gHidden : DINT := 2;
END_VAR

FUNCTION_BLOCK FB_Hide
VAR_INPUT
    inShown : INT;
    {attribute 'hide'}
    inHidden : INT;
END_VAR
VAR_OUTPUT
    outCopy : INT;
END_VAR
VAR
    shown : INT := 3;
    {attribute 'hide'}
    hidden : INT := 4;
END_VAR
outCopy := hidden;
END_FUNCTION_BLOCK

{attribute 'hide_all_locals'}
FUNCTION_BLOCK FB_Private
VAR_INPUT
    visibleIn : INT;
END_VAR
VAR
    inside : INT := 5;
END_VAR
inside := inside + visibleIn;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : FB_Hide;
    priv : FB_Private;
    count : DINT := 0;
    {attribute 'hide'}
    secret : DINT := 7;
    readSecret : DINT := 0;
END_VAR
count := count + 1;
fb();
priv();
readSecret := secret + gHidden + INT_TO_DINT(fb.outCopy);
END_PROGRAM
"#;

/// The program of [`SOURCE`], built for this computer with debug hooks and loaded.
fn build() -> CompiledRuntime {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let work = trust_test_support::unique_temp_path("trust-hide");
    let built = build_image(
        &[SourceFile::with_path(PATH, SOURCE)],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the program compiles");
    let _ = std::fs::remove_dir_all(&work);
    CompiledRuntime::load(built.image, "RESOURCE").expect("the image loads")
}

/// The JSON of the debugger's answer to the debug operation `kind` with `params`.
fn ask(debugger: &CompiledDebugger, kind: &str, params: Option<Json>) -> Json {
    let request = ControlRequest {
        id: 1,
        r#type: kind.to_string(),
        params,
        auth: None,
        request_id: None,
    };
    serde_json::to_value(debugger.answer(&request, IoSnapshotRead::Missing))
        .expect("a response serializes")
}

/// Waits within [`WAIT`] for the debugger to hold the scan at a stop.
fn wait_for_stop(debugger: &CompiledDebugger) {
    let deadline = Instant::now() + WAIT;
    while ask(debugger, "debug.stops", None)["result"]["stops"]
        .as_array()
        .is_none_or(Vec::is_empty)
    {
        assert!(Instant::now() < deadline, "no stop within the wait");
        thread::sleep(Duration::from_millis(2));
    }
}

/// The names and references of the variables that `reference` lists.
fn listed(debugger: &CompiledDebugger, reference: u64) -> Vec<(String, u64)> {
    let answer = ask(
        debugger,
        "debug.variables",
        Some(json!({ "variables_reference": reference })),
    );
    assert_eq!(answer["ok"], json!(true), "{answer}");
    answer["result"]["variables"]
        .as_array()
        .expect("variables")
        .iter()
        .map(|variable| {
            (
                variable["name"].as_str().expect("a name").to_string(),
                variable["variablesReference"].as_u64().unwrap_or(0),
            )
        })
        .collect()
}

fn names(variables: &[(String, u64)]) -> Vec<&str> {
    variables.iter().map(|(name, _)| name.as_str()).collect()
}

fn reference_of(variables: &[(String, u64)], name: &str) -> u64 {
    variables
        .iter()
        .find(|(listed, _)| listed == name)
        .map(|(_, reference)| *reference)
        .unwrap_or_else(|| panic!("{name} is listed: {variables:?}"))
}

/// Worked from the program, held at its last statement in the second scan:
/// - the locals of `Main` list `count`, `fb`, `priv` and `readSecret`, in the symbol table's order
///   (by canonical name, specification 44 §5.7), and not `secret`;
/// - the globals list `gShown`, and not `gHidden`;
/// - `fb` lists `inShown`, `outCopy` and `shown`, and not `inHidden` or `hidden`; `priv` lists
///   `visibleIn` and not `inside`, a `VAR` local of a block marked `hide_all_locals`;
/// - the code still reads the hidden variables (`outCopy` copies `hidden` in the block), so
///   `readSecret` is 7 + 2 + 4 = 13 after the scan, and the hidden program variable reads 7 by its
///   name.
///
/// Wrong behaviour caught: a hidden variable listed by the debugger, a list that also drops what is
/// not hidden (an input of a `hide_all_locals` block, a shown sibling), and a hidden variable that
/// code or its name no longer reaches.
#[test]
fn the_debugger_lists_leave_out_hidden_variables_that_code_and_names_still_reach() {
    let mut runtime = build();
    let debug = DebugControl::new();
    runtime.set_debug_control(debug.clone());
    let debugger = debug.compiled().expect("the compiled program's debugger");
    runtime.execute_cycle().expect("the first scan completes");

    let line = SOURCE
        .lines()
        .position(|line| line.starts_with("readSecret :="))
        .expect("the last statement") as u64;
    let answer = ask(
        &debugger,
        "breakpoints.set",
        Some(json!({ "source": PATH, "lines": [line] })),
    );
    assert_eq!(answer["ok"], json!(true), "{answer}");
    let scan = thread::spawn(move || {
        let result = runtime.execute_cycle();
        (runtime, result)
    });
    wait_for_stop(&debugger);

    let stack = ask(&debugger, "debug.stack", None);
    let frame = stack["result"]["stack_frames"][0]["id"]
        .as_u64()
        .unwrap_or_else(|| panic!("the held frame: {stack}"));
    let scopes = ask(
        &debugger,
        "debug.scopes",
        Some(json!({ "frame_id": frame })),
    );
    assert_eq!(scopes["ok"], json!(true), "{scopes}");
    let scope = |name: &str| {
        scopes["result"]["scopes"]
            .as_array()
            .expect("scopes")
            .iter()
            .find(|scope| scope["name"] == name)
            .and_then(|scope| scope["variablesReference"].as_u64())
            .unwrap_or_else(|| panic!("a {name} scope: {scopes}"))
    };
    let locals = listed(&debugger, scope("Locals"));
    assert_eq!(names(&locals), ["count", "fb", "priv", "readSecret"]);
    let globals = listed(&debugger, scope("Globals"));
    assert_eq!(names(&globals), ["gShown"]);
    let fb = listed(&debugger, reference_of(&locals, "fb"));
    assert_eq!(names(&fb), ["inShown", "outCopy", "shown"]);
    let private = listed(&debugger, reference_of(&locals, "priv"));
    assert_eq!(names(&private), ["visibleIn"]);

    debugger.resume();
    let (runtime, result) = scan.join().expect("the scan thread ends");
    result.expect("the second scan completes");
    assert_eq!(
        runtime.read_variable("Main.readSecret"),
        Ok(Value::DInt(13))
    );
    assert_eq!(runtime.read_variable("Main.secret"), Ok(Value::DInt(7)));
}
