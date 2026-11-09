//! Specification 02 §3.5.2: `{attribute 'pack_mode' := 'n'}` gives a structure's members the
//! addresses CODESYS packs them at, which `SIZEOF`, a `STRUCT OVERLAP`'s backing and a direct
//! address show. Every expected value is worked out by hand from CODESYS's two examples in its
//! help, "Attribute 'pack_mode'".
use trust_runtime::harness::{CompileSession, SourceFile, TestHarness};
use trust_runtime::io::IoAddress;
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;

/// CODESYS's examples: `BOOL, INT, BOOL, BOOL` packed by 1 and 2 and unpacked, `BOOL, BYTE, WORD,
/// BYTE, DWORD, BYTE, BYTE, DWORD` packed by 0, 2, 4 and 8 and unpacked, and `INT, BYTE` packed by 2,
/// whose size needs a padding byte at its end.
const TYPES: &str = r"
{attribute 'pack_mode' := '1'}
TYPE stFirst1 :
STRUCT
    b1 : BOOL;
    i : INT;
    b2 : BOOL;
    b3 : BOOL;
END_STRUCT
END_TYPE

{attribute 'pack_mode' := '2'}
TYPE stFirst2 :
STRUCT
    b1 : BOOL;
    i : INT;
    b2 : BOOL;
    b3 : BOOL;
END_STRUCT
END_TYPE

TYPE stFirst :
STRUCT
    b1 : BOOL;
    i : INT;
    b2 : BOOL;
    b3 : BOOL;
END_STRUCT
END_TYPE

{attribute 'pack_mode' := '0'}
TYPE stSecond0 :
STRUCT
    a : BOOL; b : BYTE; c : WORD; d : BYTE; e : DWORD; f : BYTE; g : BYTE; h : DWORD;
END_STRUCT
END_TYPE

{attribute 'pack_mode' := '2'}
TYPE stSecond2 :
STRUCT
    a : BOOL; b : BYTE; c : WORD; d : BYTE; e : DWORD; f : BYTE; g : BYTE; h : DWORD;
END_STRUCT
END_TYPE

{attribute 'pack_mode' := '4'}
TYPE stSecond4 :
STRUCT
    a : BOOL; b : BYTE; c : WORD; d : BYTE; e : DWORD; f : BYTE; g : BYTE; h : DWORD;
END_STRUCT
END_TYPE

{attribute 'pack_mode' := '8'}
TYPE stSecond8 :
STRUCT
    a : BOOL; b : BYTE; c : WORD; d : BYTE; e : DWORD; f : BYTE; g : BYTE; h : DWORD;
END_STRUCT
END_TYPE

TYPE stSecond :
STRUCT
    a : BOOL; b : BYTE; c : WORD; d : BYTE; e : DWORD; f : BYTE; g : BYTE; h : DWORD;
END_STRUCT
END_TYPE

{attribute 'pack_mode' := '2'}
TYPE stTail :
STRUCT
    w : INT;
    b : BYTE;
END_STRUCT
END_TYPE
";

/// The configuration that runs program `Main` as `P1`.
const CONFIGURATION: &str = r"
CONFIGURATION Plant
RESOURCE Res ON PLC
    TASK Cycle (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH Cycle : Main;
END_RESOURCE
END_CONFIGURATION
";

fn build(program: &str) -> Result<CompiledRuntime, String> {
    TestHarness::from_session(CompileSession::from_sources(vec![SourceFile::with_path(
        "src/Main.st",
        format!("{TYPES}\n{program}\n{CONFIGURATION}"),
    )]))
    .map(TestHarness::into_runtime)
    .map_err(|error| error.to_string())
}

/// One scan, 100 ms of logical time after the last.
fn cycle(runtime: &mut CompiledRuntime) {
    let now = runtime.current_time();
    runtime.set_current_time(Duration::from_nanos(
        now.as_nanos()
            .saturating_add(Duration::from_millis(100).as_nanos()),
    ));
    runtime.execute_cycle().expect("the cycle runs");
}

/// The integer in `P1.<name>`.
fn int(runtime: &CompiledRuntime, name: &str) -> i64 {
    match runtime.read_variable(&format!("P1.{name}")).unwrap() {
        Value::DInt(value) => i64::from(value),
        other => panic!("P1.{name} holds {other:?}"),
    }
}

/// Worked from the examples: `BOOL, INT, BOOL, BOOL` takes 5 bytes packed by 1 and 6 by 2 (0, 2, 4
/// and 5, rounded to 2); the second example 15 bytes packed by 0, 16 by 2 and 20 by 4 and 8; `INT,
/// BYTE` by 2 takes 4 (3 rounded to 2). Without the attribute `SIZEOF` adds the members' sizes, 5
/// and 15, as before (specification 29 §1). `SIZEOF` in an array bound is the same constant, so the
/// buffer of `SIZEOF(stFirst2)` bytes takes 6. Wrong behaviour caught: the attribute ignored (5 and
/// 15 for every mode), `0` read as no packing, a size left unrounded (5 for `stFirst2`, 3 for
/// `stTail`), and the constant evaluator and the compiled program disagreeing.
#[test]
fn sizeof_gives_the_size_the_pack_mode_lays_out() {
    let mut runtime = build(
        r"
PROGRAM Main
VAR
    first1 : DINT;
    first2 : DINT;
    first : DINT;
    second0 : DINT;
    second2 : DINT;
    second4 : DINT;
    second8 : DINT;
    second : DINT;
    tail : DINT;
    buffer : ARRAY[1..SIZEOF(stFirst2)] OF BYTE;
    bufferBytes : DINT;
END_VAR
first1 := SIZEOF(stFirst1);
first2 := SIZEOF(stFirst2);
first := SIZEOF(stFirst);
second0 := SIZEOF(stSecond0);
second2 := SIZEOF(stSecond2);
second4 := SIZEOF(stSecond4);
second8 := SIZEOF(stSecond8);
second := SIZEOF(stSecond);
tail := SIZEOF(stTail);
bufferBytes := SIZEOF(buffer);
END_PROGRAM
",
    )
    .unwrap_or_else(|error| panic!("the program compiles: {error}"));
    cycle(&mut runtime);
    for (name, bytes) in [
        ("first1", 5),
        ("first2", 6),
        ("first", 5),
        ("second0", 15),
        ("second2", 16),
        ("second4", 20),
        ("second8", 20),
        ("second", 15),
        ("tail", 4),
        ("bufferBytes", 6),
    ] {
        assert_eq!(int(&runtime, name), bytes, "SIZEOF for {name}");
    }
}

/// Worked from the examples, in a `STRUCT OVERLAP` whose byte array views the packed structure's
/// backing: packed by 1, `b1 := TRUE; i := 16#0201; b3 := TRUE` puts 1 at byte 0, the INT's 01 02 at
/// bytes 1 and 2, and 1 at byte 4; packed by 2, the INT lies at bytes 2 and 3 and `b3` at byte 5.
/// An array of `stTail` (`INT, BYTE` by 2) strides 4 bytes, so element 1's INT 16#0605 lies at bytes
/// 4 and 5. Wrong behaviour caught: the structure laid out at its natural offsets in the backing
/// (the INT at bytes 2 and 3 under `1`), and an array of a packed structure strided without its end
/// padding (the INT at bytes 3 and 4).
#[test]
fn an_overlap_sees_the_bytes_where_the_pack_mode_puts_them() {
    let mut runtime = build(
        r"
TYPE ovFirst1 : STRUCT OVERLAP
    tel AT %B0 : stFirst1;
    raw AT %B0 : ARRAY[0..4] OF BYTE;
END_STRUCT
END_TYPE

TYPE ovFirst2 : STRUCT OVERLAP
    tel AT %B0 : stFirst2;
    raw AT %B0 : ARRAY[0..5] OF BYTE;
END_STRUCT
END_TYPE

TYPE ovTails : STRUCT OVERLAP
    tails AT %B0 : ARRAY[0..1] OF stTail;
    raw AT %B0 : ARRAY[0..7] OF BYTE;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    one : ovFirst1;
    two : ovFirst2;
    tails : ovTails;
    r1 : ARRAY[0..4] OF BYTE;
    r2 : ARRAY[0..5] OF BYTE;
    rt : ARRAY[0..7] OF BYTE;
END_VAR
one.tel.b1 := TRUE;
one.tel.i := 16#0201;
one.tel.b3 := TRUE;
r1 := one.raw;
two.tel.b1 := TRUE;
two.tel.i := 16#0201;
two.tel.b3 := TRUE;
r2 := two.raw;
tails.tails[1].w := 16#0605;
rt := tails.raw;
END_PROGRAM
",
    )
    .unwrap_or_else(|error| panic!("the program compiles: {error}"));
    cycle(&mut runtime);
    assert_eq!(bytes(&runtime, "r1"), [1, 1, 2, 0, 1], "packed by 1");
    assert_eq!(bytes(&runtime, "r2"), [1, 0, 1, 2, 0, 1], "packed by 2");
    assert_eq!(
        bytes(&runtime, "rt"),
        [0, 0, 0, 0, 5, 6, 0, 0],
        "an array of stTail"
    );
}

/// The bytes of the `ARRAY OF BYTE` in `P1.<name>`.
fn bytes(runtime: &CompiledRuntime, name: &str) -> Vec<u8> {
    match runtime.read_variable(&format!("P1.{name}")).unwrap() {
        Value::Array(array) => array
            .elements()
            .iter()
            .map(|element| match element {
                Value::Byte(value) => *value,
                other => panic!("P1.{name} holds {other:?}"),
            })
            .collect(),
        other => panic!("P1.{name} holds {other:?}"),
    }
}

/// Worked from the first example packed by 2, located at `%QB0`: `b1` at bit 0 of byte 0, the INT
/// 16#0201 at bytes 2 and 3, `b2` at byte 4 and `b3` at bit 0 of byte 5. Wrong behaviour caught: the
/// members laid on the wire one after the other, as a structure without the attribute lies (the INT
/// at bytes 1 and 2, `b3` at bit 1 of byte 3).
#[test]
fn a_located_packed_structure_lies_on_the_wire_where_its_pack_mode_puts_it() {
    let mut runtime = build(
        r"
PROGRAM Main
VAR
    tel AT %QB0 : stFirst2;
END_VAR
tel.b1 := TRUE;
tel.i := 16#0201;
tel.b3 := TRUE;
END_PROGRAM
",
    )
    .unwrap_or_else(|error| panic!("the program compiles: {error}"));
    cycle(&mut runtime);
    let byte = |runtime: &CompiledRuntime, address: &str| match runtime
        .read_io(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::Byte(value) => value,
        other => panic!("{address} holds {other:?}"),
    };
    let wire: Vec<u8> = ["%QB0", "%QB1", "%QB2", "%QB3", "%QB4", "%QB5"]
        .into_iter()
        .map(|address| byte(&runtime, address))
        .collect();
    assert_eq!(wire, [1, 0, 1, 2, 0, 1]);
}

/// A pack mode CODESYS does not name is an error at build (E304). Wrong behaviour caught: `3`
/// accepted, or silently read as some other mode.
#[test]
fn a_value_that_is_no_pack_mode_is_refused() {
    let error = match TestHarness::from_session(CompileSession::from_sources(vec![
        SourceFile::with_path(
            "src/Main.st",
            format!(
                "{{attribute 'pack_mode' := '3'}}\nTYPE stOdd :\nSTRUCT\n    i : INT;\nEND_STRUCT\nEND_TYPE\n\
                 PROGRAM Main\nVAR\n    x : stOdd;\nEND_VAR\nx.i := 1;\nEND_PROGRAM\n{CONFIGURATION}"
            ),
        ),
    ])) {
        Ok(_) => panic!("the build must refuse pack_mode '3'"),
        Err(error) => error.to_string(),
    };
    assert!(error.contains("'3' is no pack mode"), "{error}");
}
