//! The variables of a function's or a method's frame and the `VAR_TEMP` of a function block start
//! at each call at their declared initial value or at their type's default (IEC §6.5.1, §6.5.2.1,
//! Table 13 3d; specification 44 §6.7): the values are worked out by hand, and the same program
//! runs in lockstep with the runtime.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

#[path = "support/native_lockstep.rs"]
#[allow(dead_code)]
mod native_lockstep;

use std::sync::atomic::{AtomicUsize, Ordering};

use native_lockstep::compare_sources;
use trust_runtime::harness::SourceFile;
use trust_runtime::native::{
    build, load, Device, NativeProgram, NoHooks, Options, Target, Toolchain,
};
use trust_runtime::value::{Duration, Value};

/// Frame variables of every kind: an enumeration whose first value is 3, a subrange from 5, a
/// structure with initial values in its type, initial values in declarations, an array initialized
/// with a repetition, and an omitted input with a default.
const FRAMES: &str = "
TYPE
    Mode : (Idle := 3, Run := 5);
    Pos : INT(5..10);
    Rec : STRUCT
        a : INT := 7;
        b : REAL := 1.5;
        s : STRING[8] := 'ab';
    END_STRUCT;
END_TYPE

FUNCTION Probe : INT
VAR_OUTPUT
    mode : Mode;
    pos : Pos;
    rec : Rec;
    tab : ARRAY[0..5] OF INT;
    count : INT;
END_VAR
VAR
    n : INT := 2 * 21;
    t : ARRAY[0..5] OF INT := [1, 2, 4(9)];
    k : INT;
END_VAR
k := k + 1;
n := n + k;
tab := t;
count := k;
Probe := n;
END_FUNCTION

FUNCTION Result : Mode
VAR_INPUT
    go : BOOL;
END_VAR
IF go THEN
    Result := Run;
END_IF;
END_FUNCTION

FUNCTION Sum : INT
VAR_INPUT
    a : INT;
    b : INT := 7;
END_VAR
Sum := a + b;
END_FUNCTION

FUNCTION_BLOCK Block
VAR_OUTPUT
    last : INT;
    mode : Mode;
    named : Mode;
END_VAR
VAR_TEMP
    t : INT := 5;
    tm : Mode;
END_VAR

METHOD PUBLIC Name : Mode
VAR
    seen : INT := 3;
END_VAR
seen := seen + 1;
IF seen = 4 THEN
    Name := Run;
END_IF;
END_METHOD

t := t + 1;
last := t;
mode := tm;
named := Name();
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    v : INT;
    m : Mode := Run;
    p : Pos := 10;
    r : Rec;
    tb : ARRAY[0..5] OF INT;
    c : INT;
    m2 : Mode := Run;
    s : INT;
    fb : Block;
END_VAR
r.a := 0;
r.b := 0.0;
r.s := '';
v := Probe(mode => m, pos => p, rec => r, tab => tb, count => c);
m2 := Result(go := FALSE);
s := Sum(a := 1);
fb();
END_PROGRAM
";

fn program(source: &str) -> NativeProgram<'static, trust_runtime::native::HostCode> {
    static BUILDS: AtomicUsize = AtomicUsize::new(0);
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let host = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!(
        "trust-native-frames-{}-{}",
        std::process::id(),
        BUILDS.fetch_add(1, Ordering::Relaxed)
    ));
    let built = build(
        &[SourceFile::new(source)],
        &Options {
            target: host,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the program compiles");
    let _ = std::fs::remove_dir_all(&work);
    let device = Device {
        target: host,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 10_000,
    };
    let image: &'static [u8] = Box::leak(built.image.into_boxed_slice());
    let mut program = load(image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    program
}

/// The number an integer or an enumeration value holds.
fn number(value: &Value) -> i64 {
    match value {
        Value::Int(value) => i64::from(*value),
        Value::Enum(value) => value.numeric_value(),
        other => panic!("{other:?} is no number"),
    }
}

/// Each call starts its frame anew (IEC §6.5.2.1). Over three scans `Probe` returns 43 each
/// time: `n` starts at 42 and `k` at 0, which counts to 1; a frame kept from call to call would
/// give 48 by the third scan, and one that ignores the initial value 1. `Probe` never assigns its
/// outputs, so they hand on their starting values: the mode `Idle`, 3, the type's first value,
/// where zero is no value of `Mode`; the position 5, the subrange's lower bound; the record's 7,
/// 1.5 and 'ab' from its type, over the zeros the program wrote; the table 1, 2, 9, 9, 9, 9; and
/// `count` 1. `Result` returns `Idle`; `Sum(a := 1)` is 1 + 7 with `b`'s default. The block's
/// `VAR_TEMP` `t` starts at 5 at each call, so `last` is 6, and `tm` at `Idle`; the method's
/// `seen` starts at 3, so it is 4 and `Name` is `Run`, 5.
#[test]
fn frames_start_at_each_call_at_their_initial_values() {
    let mut program = program(FRAMES);
    for _ in 0..3 {
        program
            .scan(Duration::from_millis(10), &mut NoHooks)
            .expect("the scan runs");
    }
    let read = |name: &str| program.value(name).unwrap_or_else(|| panic!("{name}"));
    for (name, expected) in [
        ("MAIN.V", 43),
        ("MAIN.M", 3),
        ("MAIN.P", 5),
        ("MAIN.C", 1),
        ("MAIN.M2", 3),
        ("MAIN.S", 8),
        ("MAIN.FB.LAST", 6),
        ("MAIN.FB.MODE", 3),
        ("MAIN.FB.NAMED", 5),
    ] {
        assert_eq!(number(&read(name)), expected, "{name}");
    }
    let Value::Struct(record) = read("MAIN.R") else {
        panic!("MAIN.R is no structure");
    };
    assert_eq!(record.field("a"), Some(&Value::Int(7)));
    assert_eq!(record.field("b"), Some(&Value::Real(1.5)));
    assert_eq!(record.field("s"), Some(&Value::String("ab".into())));
    let Value::Array(table) = read("MAIN.TB") else {
        panic!("MAIN.TB is no array");
    };
    let table: Vec<i64> = table.elements().iter().map(number).collect();
    assert_eq!(table, [1, 2, 9, 9, 9, 9]);
}

/// The same program in lockstep with the runtime, which starts frames the same way.
#[test]
fn frames_start_as_the_runtime_starts_them() {
    match compare_sources("frames", &[SourceFile::new(FRAMES)], 10, 20) {
        Ok(matched) => println!("frames: {matched} values matched over 20 scans"),
        Err(divergence) => panic!("frames: {divergence}"),
    }
}
