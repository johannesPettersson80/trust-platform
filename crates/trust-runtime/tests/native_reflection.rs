//! A compiled program's variables reached by path and its function blocks by type, through the
//! image's tables alone (specification 44 §5.8, §13.3), as Control Studio, the HMI and `Comm.Link`
//! reach them on a computer.

use trust_runtime::harness::TestHarness;
use trust_runtime::runtime_core::native::memory::{read_place, ProgramMemory};
use trust_runtime::runtime_core::native::reflect::{
    resolve_path, type_base, type_implements, type_interfaces, type_is_a, walk_instances,
    PathError, Walk,
};
use trust_runtime::value::Value;
use trust_runtime::CompiledState;

const SOURCE: &str = r#"
NAMESPACE Lib
INTERFACE IRun
METHOD Start
END_METHOD
END_INTERFACE

INTERFACE IRunMore EXTENDS IRun
METHOD Halt
END_METHOD
END_INTERFACE
END_NAMESPACE

FUNCTION_BLOCK Device
VAR_OUTPUT
    Running : BOOL;
END_VAR
VAR
    Starts : DINT := 3;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Pump EXTENDS Device IMPLEMENTS Lib.IRunMore
VAR
    Speed : REAL;
END_VAR
METHOD PUBLIC Start
    Speed := Speed + 10.0;
END_METHOD
METHOD PUBLIC Halt
    Speed := 0.0;
END_METHOD
    Speed := Speed + 1.0;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Station
VAR_OUTPUT
    Levels : ARRAY[1..3] OF INT;
END_VAR
VAR
    Feed : Pump;
END_VAR
    Levels[3] := 7;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Pumps : ARRAY[1..2, 0..1] OF Pump;
    Line : Station;
    Count : INT;
END_VAR
    Pumps[2, 1]();
    Pumps[2, 1].Start();
    Line();
    Count := Count + 1;
END_PROGRAM
"#;

/// The program after one scan, as a computer's services read it.
fn scanned() -> CompiledState {
    let mut harness = TestHarness::from_source(SOURCE).expect("the program compiles");
    harness.cycle();
    harness
        .runtime()
        .snapshot()
        .expect("a computer keeps the program's image")
}

fn read(state: &CompiledState, path: &str) -> Value {
    let located = resolve_path(&state.image(), path)
        .unwrap_or_else(|error| panic!("{path} resolves: {error}"));
    read_place(state, located.place, located.type_index)
        .unwrap_or_else(|error| panic!("{path} reads: {error}"))
}

#[test]
fn a_path_reaches_members_of_array_elements_and_structures_inherited_members_included() {
    // Specification 44 §13.3: a path's first name is found in the symbol table, then each member
    // and each element through the type table, without regard to case. After one scan the element
    // `Pumps[2, 1]` has run its body once and its `Start` once (Speed 1 + 10 = 11.0), every element
    // holds the initial value 3 of `Starts`, which it inherits from `Device`, and
    // `Line.Levels[3]` holds 7. Catches an element offset that is not row-major or ignores a
    // dimension's lower bound (`0..1`), an inherited member a path cannot reach, and a
    // case-sensitive lookup.
    let state = scanned();
    assert_eq!(read(&state, "Main.Pumps[2, 1].Speed"), Value::Real(11.0));
    assert_eq!(read(&state, "Main.Pumps[2,1].Starts"), Value::DInt(3));
    assert_eq!(read(&state, "Main.Pumps[1, 0].Starts"), Value::DInt(3));
    assert_eq!(read(&state, "Main.Pumps[1, 0].Running"), Value::Bool(false));
    assert_eq!(read(&state, "Main.Pumps[2, 0].Speed"), Value::Real(0.0));
    assert_eq!(read(&state, "main.line.levels[3]"), Value::Int(7));
    assert_eq!(read(&state, "Main.Line.Levels[1]"), Value::Int(0));
    assert_eq!(read(&state, "Main.Count"), Value::Int(1));
}

#[test]
fn a_path_that_names_nothing_says_why() {
    // Specification 44 §13.3: a path outside the program is refused with its reason, never read
    // from a neighbouring place. Catches an index past a dimension's bounds or of the wrong number of
    // dimensions that is read anyway, a member of a value without members, and a malformed path.
    let state = scanned();
    let image = state.image();
    assert!(matches!(resolve_path(&image, "Main.Pumps[3, 0]"), Err(PathError::Index(_))));
    assert!(matches!(resolve_path(&image, "Main.Pumps[1, 2]"), Err(PathError::Index(_))));
    assert!(matches!(resolve_path(&image, "Main.Pumps[1]"), Err(PathError::Index(_))));
    assert!(matches!(resolve_path(&image, "Main.Count[1]"), Err(PathError::Index(_))));
    assert!(matches!(resolve_path(&image, "Main.Count.Bit"), Err(PathError::NoMember(_))));
    assert!(matches!(resolve_path(&image, "Main.Nothing"), Err(PathError::NoMember(_))));
    assert!(matches!(resolve_path(&image, "Nothing"), Err(PathError::Unknown(_))));
    assert!(matches!(resolve_path(&image, "Main..Count"), Err(PathError::Syntax(_))));
    assert!(matches!(resolve_path(&image, "Main.Pumps[x]"), Err(PathError::Syntax(_))));
}

#[test]
fn a_block_type_names_its_base_and_every_interface_it_implements() {
    // Specification 44 §5.8: a function block's type record names the type it extends and every
    // interface it implements, itself, through its base or through an interface that extends one
    // (IEC 61131-3 §6.6.6.4), interfaces by their qualified names. `Pump` extends `Device` and
    // implements `Lib.IRunMore`, which extends `Lib.IRun`. Catches a base the image loses, an
    // interface reached only through another interface that is missing, and a comparison that
    // depends on case.
    let state = scanned();
    let image = state.image();
    let pump = resolve_path(&image, "Main.Pumps[1, 1]").expect("an element").type_index;
    assert!(type_is_a(&image, pump, "Pump").unwrap());
    assert!(type_is_a(&image, pump, "device").unwrap());
    assert!(!type_is_a(&image, pump, "Station").unwrap());
    assert!(type_implements(&image, pump, "Lib.IRunMore").unwrap());
    assert!(type_implements(&image, pump, "LIB.IRUN").unwrap());
    assert_eq!(type_interfaces(&image, pump).unwrap().len(), 2);
    let device = type_base(&image, pump).unwrap().expect("Pump extends a base");
    assert!(type_is_a(&image, device, "Device").unwrap());
    assert_eq!(type_base(&image, device).unwrap(), None);
    assert!(!type_implements(&image, device, "Lib.IRun").unwrap());
    let station = resolve_path(&image, "Main.Line").expect("an instance").type_index;
    assert_eq!(type_base(&image, station).unwrap(), None);
    assert!(type_interfaces(&image, station).unwrap().is_empty());
}

#[test]
fn the_walk_visits_every_instance_in_declaration_order_and_stops_when_told() {
    // Specification 44 §13.3: the instances of the program, found where they are declared: the
    // elements of an array of instances in row-major order, then an instance and the instance it
    // holds, which the visitor descends into. Catches an element order that is not row-major, a
    // nested instance that is missed, and a walk that goes on after the visitor stops it.
    let state = scanned();
    let image = state.image();
    let mut paths = Vec::new();
    walk_instances(&image, &mut |path, _| {
        paths.push(path.to_string());
        Walk::Descend
    })
    .expect("the walk reads the tables");
    assert_eq!(
        paths,
        [
            "Main.Pumps[1,0]",
            "Main.Pumps[1,1]",
            "Main.Pumps[2,0]",
            "Main.Pumps[2,1]",
            "Main.Line",
            "Main.Line.Feed",
        ]
    );
    let mut first = Vec::new();
    walk_instances(&image, &mut |path, _| {
        first.push(path.to_string());
        if first.len() == 2 {
            Walk::Stop
        } else {
            Walk::Descend
        }
    })
    .expect("the walk reads the tables");
    assert_eq!(first, ["Main.Pumps[1,0]", "Main.Pumps[1,1]"]);
}

