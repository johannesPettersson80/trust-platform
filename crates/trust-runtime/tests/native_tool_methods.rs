//! A tool's call of a method of a live function block instance of a compiled program between scans
//! (specification 44 §5.4, §5.7, §7.1, §13.3; specification 39 §21.2): the method a call through
//! the interface reaches runs on the instance the path names, the instance's body does not, an
//! input not given takes its initial value, the work is bounded by the tool's allowance, and a
//! fault of the method faults the resource. Every expected value is worked out by hand.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::{SourceFile, TestHarness};
use trust_runtime::native::{build, Options, Target, Toolchain};
use trust_runtime::runtime_core::native::image::{
    AreaRecord, EntryKind, EntryRecord, Image, SymbolRecord, NONE, SYMBOL_INPUT,
};
use trust_runtime::runtime_core::native::memory::{read_place, ProgramMemory};
use trust_runtime::runtime_core::native::reflect::resolve_path;
use trust_runtime::runtime_core::native::tool::ONLY_INPUTS;
use trust_runtime::value::Value;
use trust_runtime::CompiledRuntime;

/// The program of the bytecode engine's HMI method tests, with methods that take an output, inputs
/// with initial values, a text, a real and a division, and a derived block that overrides a method.
const PROGRAM: &str = r#"
INTERFACE ICounter
METHOD Add : DINT
VAR_INPUT Amount : DINT; END_VAR
END_METHOD
METHOD Peek : DINT
VAR_OUTPUT Doubled : DINT; END_VAR
END_METHOD
METHOD Advance : DINT
VAR_INPUT Amount : DINT := 5; Times : DINT := 2; END_VAR
END_METHOD
METHOD Rename : BOOL
VAR_INPUT Name : STRING[16] := 'pump'; Enable : BOOL := TRUE; END_VAR
END_METHOD
METHOD Scale : REAL
VAR_INPUT Factor : REAL := 1.5; END_VAR
END_METHOD
METHOD Divide : DINT
VAR_INPUT Divisor : DINT; END_VAR
END_METHOD
END_INTERFACE

INTERFACE IReset
METHOD Reset
END_METHOD
END_INTERFACE

FUNCTION_BLOCK Counter IMPLEMENTS ICounter
VAR_OUTPUT Total : DINT; BodyCalls : DINT; Label : STRING[16]; END_VAR
BodyCalls := BodyCalls + 1;
METHOD PUBLIC Add : DINT
VAR_INPUT Amount : DINT; END_VAR
    Total := Total + Amount;
    Add := Total;
END_METHOD
METHOD PUBLIC Peek : DINT
VAR_OUTPUT Doubled : DINT; END_VAR
    Doubled := Total * 2;
    Peek := Total;
END_METHOD
METHOD PUBLIC Advance : DINT
VAR_INPUT Amount : DINT := 5; Times : DINT := 2; END_VAR
    Total := Total + Amount * Times;
    Advance := Total;
END_METHOD
METHOD PUBLIC Rename : BOOL
VAR_INPUT Name : STRING[16] := 'pump'; Enable : BOOL := TRUE; END_VAR
    Label := Name;
    Rename := Enable;
END_METHOD
METHOD PUBLIC Scale : REAL
VAR_INPUT Factor : REAL := 1.5; END_VAR
    Scale := DINT_TO_REAL(Total) * Factor;
END_METHOD
METHOD PUBLIC Divide : DINT
VAR_INPUT Divisor : DINT; END_VAR
    Total := Total + 1;
    Divide := Total / Divisor;
END_METHOD
METHOD PUBLIC HiddenFromCapability : DINT
    Total := 999;
    HiddenFromCapability := Total;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FastCounter EXTENDS Counter
METHOD PUBLIC OVERRIDE Add : DINT
VAR_INPUT Amount : DINT; END_VAR
    Total := Total + 2 * Amount;
    Add := Total;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Resettable IMPLEMENTS IReset
VAR_OUTPUT Resets : DINT; END_VAR
METHOD PUBLIC Reset
    Resets := Resets + 1;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR Items : ARRAY[1..2] OF Counter; Fast : FastCounter; Other : Resettable; END_VAR
END_PROGRAM
"#;

/// The program after one scan, whose body calls no block, as a tool finds it between scans.
fn scanned() -> TestHarness {
    let mut harness = TestHarness::from_source(PROGRAM).expect("the program compiles");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
}

/// The value at `path` as the program holds it now.
fn read(runtime: &CompiledRuntime, path: &str) -> Value {
    let state = runtime
        .snapshot()
        .expect("a computer keeps the program's image");
    let located = resolve_path(&state.image(), path)
        .unwrap_or_else(|error| panic!("{path} resolves: {error}"));
    read_place(&state, located.place, located.type_index)
        .unwrap_or_else(|error| panic!("{path} reads: {error}"))
}

/// The bytes of every area of the program but the stack, which holds a call's block.
fn memory(runtime: &CompiledRuntime) -> Vec<Vec<u8>> {
    let state = runtime
        .snapshot()
        .expect("a computer keeps the program's image");
    let image = state.image();
    (1..=6u8)
        .map(|id| {
            let length = image
                .area_length(id)
                .and_then(|length| usize::try_from(length).ok())
                .unwrap_or(0);
            let mut bytes = vec![0; length];
            assert!(state.read_area(id, 0, &mut bytes), "area {id} reads");
            bytes
        })
        .collect()
}

fn dint(name: &str, value: i32) -> (smol_str::SmolStr, Value) {
    (name.into(), Value::DInt(value))
}

#[test]
fn typed_method_runs_on_selected_instance_without_executing_fb_body() {
    // Specification 44 §13.3 (spec 39 §21.2): `ICounter.Add` runs on `Main.Items[2]` alone:
    // its Total goes from 0 to 3 and the call returns 3, `Items[1]` keeps 0, and the block's body,
    // which counts BodyCalls, does not run. The method's two statements take two of the allowance
    // in this debug image (NX-10). Catches a call on the wrong element of the array, a body run
    // around the method, and a wrong count of the statements used.
    let mut harness = scanned();
    let runtime = harness.runtime_mut();
    let called = runtime
        .call_method(
            "Main.Items[2]",
            "ICounter",
            "Add",
            &[dint("Amount", 3)],
            10_000,
        )
        .expect("the method runs");
    assert_eq!(called, (Value::DInt(3), 2));
    assert_eq!(read(runtime, "Main.Items[1].Total"), Value::DInt(0));
    assert_eq!(read(runtime, "Main.Items[2].Total"), Value::DInt(3));
    assert_eq!(read(runtime, "Main.Items[2].BodyCalls"), Value::DInt(0));
    assert!(!runtime.faulted());
}

#[test]
fn methods_outside_capability_and_exhausted_budget_do_not_execute() {
    // Specification 44 §13.3: each of these calls is refused and leaves every area of the program
    // as it was, and the resource runs on. `Main.Items[1]` is a Counter, which does not implement
    // IReset although Resettable does; ICounter declares no HiddenFromCapability, which would set
    // Total to 999; Peek takes an output; Add runs two statements, so an allowance of 1 is used
    // up after its first statement had added 3, which is undone; Add has no input Count and takes
    // no text; Scale takes no NaN (specification 34 §4.3.1); `Main.Items[3]` is outside the array.
    // Catches a method a call through the interface cannot reach, a call cut short whose first
    // store stays or that faults the resource, and a value converted past its input's type.
    let mut harness = scanned();
    let before = memory(harness.runtime());
    let runtime = harness.runtime_mut();
    let path = "Main.Items[1]";
    let refused = |result: Result<(Value, u32), RuntimeError>| result.expect_err("refused");
    let error = refused(runtime.call_method(path, "IReset", "Reset", &[], 10_000));
    assert!(matches!(error, RuntimeError::ControlError(_)), "{error}");
    let error = refused(runtime.call_method(path, "IUnknown", "Add", &[], 10_000));
    assert!(matches!(error, RuntimeError::ControlError(_)), "{error}");
    let error = refused(runtime.call_method(path, "ICounter", "HiddenFromCapability", &[], 10_000));
    assert!(matches!(error, RuntimeError::UndefinedField(_)), "{error}");
    let error = refused(runtime.call_method(path, "ICounter", "Peek", &[], 10_000));
    assert_eq!(error, RuntimeError::InvalidConfig(ONLY_INPUTS.into()));
    let error = refused(runtime.call_method(path, "ICounter", "Add", &[dint("Amount", 3)], 0));
    assert!(
        matches!(error, RuntimeError::ExecutionLimit { limit: 0, .. }),
        "{error}"
    );
    let error = refused(runtime.call_method(path, "ICounter", "Add", &[dint("Amount", 3)], 1));
    assert!(
        matches!(
            error,
            RuntimeError::ExecutionLimit {
                used: 1,
                limit: 1,
                ..
            }
        ),
        "{error}"
    );
    let error = refused(runtime.call_method(path, "ICounter", "Add", &[dint("Count", 3)], 10_000));
    assert_eq!(error, RuntimeError::InvalidArgumentName("Count".into()));
    let text = [("Amount".into(), Value::String("three".into()))];
    let error = refused(runtime.call_method(path, "ICounter", "Add", &text, 10_000));
    assert_eq!(error, RuntimeError::TypeMismatch);
    let not_a_number = [("Factor".into(), Value::Real(f32::NAN))];
    let error = refused(runtime.call_method(path, "ICounter", "Scale", &not_a_number, 10_000));
    assert_eq!(error, RuntimeError::NonFiniteValue);
    let error = refused(runtime.call_method("Main.Items[3]", "ICounter", "Add", &[], 10_000));
    assert!(
        matches!(error, RuntimeError::UndefinedVariable(_)),
        "{error}"
    );
    assert!(!runtime.faulted());
    assert_eq!(memory(runtime), before);
    assert_eq!(read(runtime, "Main.Items[1].Total"), Value::DInt(0));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
}

#[test]
fn an_input_not_given_takes_its_initial_value_at_every_call() {
    // IEC 61131-3 §6.6.1: a method's inputs start from their initial values at each call, so an
    // input a call does not give is its declared initial value, not what an earlier call gave.
    // Advance adds Amount * Times to Total: with neither given 5 * 2 = 10; with Times 3 alone
    // 10 + 5 * 3 = 25; with Amount 1 alone 25 + 1 * 2 = 27. Rename sets Label from a text and
    // returns a BOOL: none given, 'pump' and TRUE; 'valve' and FALSE given; none given again,
    // 'pump' and TRUE. Scale returns the REAL Total * Factor: 27 * 1.5 = 40.5 with Factor's initial
    // value. Catches an input that keeps the last call's value or starts at zero, a text or a BOOL
    // read from the block as another value, and a result written into the wrong place.
    let mut harness = scanned();
    let runtime = harness.runtime_mut();
    let path = "Main.Items[1]";
    let mut call = |method: &str, inputs: &[(smol_str::SmolStr, Value)]| {
        runtime
            .call_method(path, "ICounter", method, inputs, 10_000)
            .unwrap_or_else(|error| panic!("{method}: {error}"))
            .0
    };
    assert_eq!(call("Advance", &[]), Value::DInt(10));
    assert_eq!(call("Advance", &[dint("Times", 3)]), Value::DInt(25));
    assert_eq!(call("Advance", &[dint("amount", 1)]), Value::DInt(27));
    assert_eq!(call("Rename", &[]), Value::Bool(true));
    let label = read(runtime, "Main.Items[1].Label");
    assert_eq!(label, Value::String("pump".into()));
    let given = [
        ("Name".into(), Value::String("valve".into())),
        ("Enable".into(), Value::Bool(false)),
    ];
    assert_eq!(
        runtime
            .call_method(path, "ICounter", "Rename", &given, 10_000)
            .unwrap()
            .0,
        Value::Bool(false)
    );
    assert_eq!(
        read(runtime, "Main.Items[1].Label"),
        Value::String("valve".into())
    );
    assert_eq!(
        runtime
            .call_method(path, "ICounter", "Rename", &[], 10_000)
            .unwrap()
            .0,
        Value::Bool(true)
    );
    assert_eq!(
        read(runtime, "Main.Items[1].Label"),
        Value::String("pump".into())
    );
    assert_eq!(
        runtime
            .call_method(path, "ICounter", "Scale", &[], 10_000)
            .unwrap()
            .0,
        Value::Real(40.5)
    );
}

#[test]
fn an_override_in_a_derived_type_is_the_one_called() {
    // IEC 61131-3 §6.6.8 and specification 44 §6.4: a call through an interface binds by the
    // instance's class. FastCounter overrides Add to add twice the amount: Add 3 on `Main.Fast`
    // gives 6, on the Counter `Main.Items[1]` 3. Catches a call bound to the method of the type
    // that declares the interface's implementation instead of the instance's own.
    let mut harness = scanned();
    let runtime = harness.runtime_mut();
    let fast = runtime
        .call_method("Main.Fast", "ICounter", "Add", &[dint("Amount", 3)], 10_000)
        .expect("the override runs");
    assert_eq!(fast.0, Value::DInt(6));
    assert_eq!(read(runtime, "Main.Fast.Total"), Value::DInt(6));
    let plain = runtime
        .call_method(
            "Main.Items[1]",
            "ICounter",
            "Add",
            &[dint("Amount", 3)],
            10_000,
        )
        .expect("the method runs");
    assert_eq!(plain.0, Value::DInt(3));
}

#[test]
fn a_fault_in_the_method_faults_the_runtime() {
    // Specification 44 §13.3, §8.6: a fault of the method faults the resource as a faulted scan
    // does. Divide adds 1 to Total and divides it by 0: the call returns the division by zero, the
    // fault is latched, the store of the statement before it stays (Total 1), and neither a scan
    // nor another call runs until a restart. Catches a fault reported as a refusal, which leaves
    // the resource running, and a fault that is undone like a used-up allowance.
    let mut harness = scanned();
    let runtime = harness.runtime_mut();
    let error = runtime
        .call_method(
            "Main.Items[1]",
            "ICounter",
            "Divide",
            &[dint("Divisor", 0)],
            10_000,
        )
        .expect_err("the method faults");
    assert_eq!(error, RuntimeError::DivisionByZero);
    assert!(runtime.faulted());
    assert_eq!(runtime.last_fault(), Some(RuntimeError::DivisionByZero));
    assert_eq!(read(runtime, "Main.Items[1].Total"), Value::DInt(1));
    assert_eq!(
        runtime.call_method(
            "Main.Items[1]",
            "ICounter",
            "Add",
            &[dint("Amount", 3)],
            10_000
        ),
        Err(RuntimeError::ResourceFaulted)
    );
    let cycle = harness.cycle();
    assert_eq!(cycle.errors, [RuntimeError::ResourceFaulted]);
}

/// A namespaced interface, an interface that extends it, a method with an output, and a block
/// that implements them.
const IMAGE_PROGRAM: &str = r#"
NAMESPACE Lib
INTERFACE IRun
METHOD Start : BOOL
VAR_INPUT Speed : INT; END_VAR
END_METHOD
METHOD Report
VAR_OUTPUT Hours : DINT; END_VAR
END_METHOD
END_INTERFACE
INTERFACE IRunMore EXTENDS IRun
METHOD Halt
END_METHOD
END_INTERFACE
END_NAMESPACE

FUNCTION_BLOCK Pump IMPLEMENTS Lib.IRunMore
VAR Running : BOOL; Rpm : INT; Worked : DINT; END_VAR
METHOD PUBLIC Start : BOOL
VAR_INPUT Speed : INT; END_VAR
    Rpm := Speed;
    Running := TRUE;
    Start := Running;
END_METHOD
METHOD PUBLIC Report
VAR_OUTPUT Hours : DINT; END_VAR
    Hours := Worked;
END_METHOD
METHOD PUBLIC Halt
    Running := FALSE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR P : Pump; END_VAR
END_PROGRAM
"#;

/// The image of `source` for this computer, a debug image with `hooks`, else a release image.
fn image_of(source: &str, hooks: bool) -> Vec<u8> {
    let toolchain = Toolchain::find().expect("the toolchain of specification 44 §4.6");
    let target = Target::this_cpu().expect("this computer's CPU runs program images");
    let work =
        std::env::temp_dir().join(format!("trust-tool-methods-{}-{hooks}", std::process::id()));
    let built = build(
        &[SourceFile::new(source)],
        &Options { target, hooks },
        &toolchain,
        &work,
    );
    let _ = std::fs::remove_dir_all(&work);
    built.expect("the program builds").image
}

/// The symbol of canonical name `canonical` with its index.
fn symbol(image: &Image<'_>, canonical: &str) -> (u32, SymbolRecord) {
    image
        .find_symbol(canonical)
        .unwrap_or_else(|| panic!("the image has the symbol {canonical}"))
}

/// The tool entries' indexes, the symbols they name.
fn tool_entries(image: &Image<'_>) -> Vec<u32> {
    image
        .table::<EntryRecord>()
        .expect("the entry table reads")
        .iter()
        .filter(|entry| entry.kind == EntryKind::Method as u32)
        .map(|entry| entry.index)
        .collect()
}

#[test]
fn release_and_debug_images_carry_each_methods_tool_symbol_entry_and_block() {
    // Specification 44 §5.4, §5.7, §7.1: Pump implements Lib.IRunMore and, through it, Lib.IRun.
    // Each method of each has a tool symbol `@TOOL.<interface>.<method>` of area 0; Start and Halt,
    // inputs only, have a tool entry whose index is their symbol's, and Report, with an output, has
    // its symbol without parts and no entry. Start's block, laid out as a structure (§7.2): THIS, 16
    // bytes at 0; __GIVEN, one byte at 16; the input Speed, an INT at 18; the result Start, a BOOL
    // at 20; 21 bytes padded to 24 by THIS's alignment of 4. Halt's is THIS and __GIVEN, 17 bytes
    // padded to 20, so the stack area is 24 bytes, the largest block. A release image calls Start
    // as a debug image does, and counts no statement of it (NX-10). Catches a tool symbol named
    // otherwise, an entry that names another symbol, a block whose parts are not where the entry
    // reads them, a stack area too small for a block, and a release image without its tools.
    for hooks in [true, false] {
        let bytes = image_of(IMAGE_PROGRAM, hooks);
        let image = Image::parse(&bytes).expect("the image parses");
        let mut entries = tool_entries(&image);
        entries.sort_unstable();
        let mut expected: Vec<u32> = [
            "@TOOL.LIB.IRUN.START",
            "@TOOL.LIB.IRUNMORE.START",
            "@TOOL.LIB.IRUNMORE.HALT",
        ]
        .iter()
        .map(|name| symbol(&image, name).0)
        .collect();
        expected.sort_unstable();
        assert_eq!(entries, expected, "hooks {hooks}");
        for report in ["@TOOL.LIB.IRUN.REPORT", "@TOOL.LIB.IRUNMORE.REPORT"] {
            let (index, record) = symbol(&image, report);
            assert_eq!(record.area, 0);
            assert!(!entries.contains(&index), "{report} has no entry");
            assert!(image.find_symbol(&format!("{report}.THIS")).is_none());
        }
        let (start, record) = symbol(&image, "@TOOL.LIB.IRUN.START");
        assert_eq!((record.area, record.parent, record.size), (0, NONE, 24));
        for (part, offset, size, input) in [
            ("THIS", 0, 16, false),
            ("__GIVEN", 16, 1, false),
            ("SPEED", 18, 2, true),
            ("START", 20, 1, false),
        ] {
            let (_, record) = symbol(&image, &format!("@TOOL.LIB.IRUN.START.{part}"));
            assert_eq!(
                (record.area, record.offset, record.size, record.parent),
                (7, offset, size, start),
                "{part}"
            );
            assert_eq!(record.flags & SYMBOL_INPUT != 0, input, "{part}");
        }
        let (_, halt) = symbol(&image, "@TOOL.LIB.IRUNMORE.HALT");
        assert_eq!(halt.size, 20);
        let stack = image
            .table::<AreaRecord>()
            .expect("the area table reads")
            .iter()
            .find(|area| area.id == 7)
            .expect("the stack area");
        assert_eq!(stack.length, 24, "hooks {hooks}");
        let mut runtime =
            CompiledRuntime::load(bytes.clone(), "RESOURCE").expect("the computer runs the image");
        runtime
            .restart(trust_runtime::RestartMode::Cold)
            .expect("a cold restart");
        let speed = [("Speed".into(), Value::Int(1500))];
        let called = runtime
            .call_method("Main.P", "Lib.IRun", "Start", &speed, 100)
            .expect("Start runs");
        assert_eq!(called, (Value::Bool(true), if hooks { 3 } else { 0 }));
        assert_eq!(read(&runtime, "Main.P.Rpm"), Value::Int(1500));
        assert_eq!(read(&runtime, "Main.P.Running"), Value::Bool(true));
        let error = runtime
            .call_method("Main.P", "Lib.IRun", "Report", &[], 100)
            .expect_err("Report takes an output");
        assert_eq!(error, RuntimeError::InvalidConfig(ONLY_INPUTS.into()));
    }
}
