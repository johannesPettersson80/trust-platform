//! Specification 10 §6.7 and the Plant library specification §11: RETAIN variables declared inside
//! function block instances, including a base part's and a nested instance's, reach the retain
//! store and come back after a power loss, exactly as RETAIN program variables do. An instance
//! initializer may set a member the block inherits.

use trust_runtime::harness::TestHarness;
use trust_runtime::retain::FileRetainStore;
use trust_runtime::value::{Duration, Value};
use trust_runtime::RestartMode;

const SOURCE: &str = r#"
FUNCTION_BLOCK Base
VAR PUBLIC RETAIN
    Limit : TIME := T#5s;
END_VAR
VAR_OUTPUT RETAIN
    Count : UDINT;
END_VAR
VAR NON_RETAIN
    Transient : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Totaliser
VAR_OUTPUT RETAIN
    Total : DINT;
END_VAR
    Total := Total + 2;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Device EXTENDS Base
VAR RETAIN
    Hours : LREAL;
END_VAR
VAR
    Inner : Totaliser;
END_VAR
    Count := Count + 1;
    Hours := Hours + 0.5;
    Transient := Transient + 1;
    Inner();
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Pump : Device;
    Spare : Device := (Limit := T#7s);
    Arm : BOOL;
END_VAR
    IF Arm THEN
        Pump.Limit := T#9s;
    END_IF;
    Pump();
END_PROGRAM
"#;

fn value(harness: &TestHarness, path: &str) -> Value {
    harness
        .get_output(path)
        .unwrap_or_else(|| panic!("{path:?} exists"))
}

fn scan(harness: &mut TestHarness) {
    harness.advance_time(Duration::from_millis(10));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
}

fn store_path(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "trust_retain_instance_members_{}_{name}.bin",
        std::process::id()
    ));
    path
}

#[test]
fn instance_retain_members_survive_a_power_loss() {
    let path = store_path("power_loss");
    let _ = std::fs::remove_file(&path);
    let mut before = TestHarness::from_source(SOURCE).expect("compile and initialize");
    before
        .runtime_mut()
        .set_retain_store(
            Some(Box::new(FileRetainStore::new(&path))),
            Some(Duration::from_millis(1)),
        )
        .expect("set the retain store");
    before.set_input("Arm", Value::Bool(true));
    scan(&mut before);
    before.set_input("Arm", Value::Bool(false));
    scan(&mut before);
    scan(&mut before);
    before
        .runtime_mut()
        .save_retain_store()
        .expect("save the retain store");
    drop(before);

    // A power loss: a new runtime of the same program reads the store at start.
    let mut after = TestHarness::from_source(SOURCE).expect("compile and initialize again");
    after
        .runtime_mut()
        .set_retain_store(
            Some(Box::new(FileRetainStore::new(&path))),
            Some(Duration::from_millis(1)),
        )
        .expect("set the retain store");
    after
        .restart_with_retain(RestartMode::Warm)
        .expect("load the retain store");
    assert_eq!(
        value(&after, "Main.Pump.Limit"),
        Value::Time(Duration::from_millis(9_000)),
        "a RETAIN setting of a base part comes back"
    );
    assert_eq!(
        value(&after, "Main.Pump.Count"),
        Value::UDInt(3),
        "a RETAIN output of a base part comes back"
    );
    assert_eq!(
        value(&after, "Main.Pump.Hours"),
        Value::LReal(1.5),
        "a RETAIN variable of the derived block comes back"
    );
    assert_eq!(
        value(&after, "Main.Pump.Inner.Total"),
        Value::DInt(6),
        "a RETAIN output of a nested instance comes back"
    );
    assert_eq!(
        value(&after, "Main.Pump.Transient"),
        Value::Int(0),
        "a NON_RETAIN variable starts over"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn an_instance_initializer_sets_an_inherited_member() {
    let harness = TestHarness::from_source(SOURCE).expect("compile and initialize");
    assert_eq!(
        value(&harness, "Main.Spare.Limit"),
        Value::Time(Duration::from_millis(7_000)),
        "the initializer reaches the member Device inherits from Base"
    );
    assert_eq!(
        value(&harness, "Main.Pump.Limit"),
        Value::Time(Duration::from_millis(5_000)),
        "an instance without initializer keeps the declared default"
    );
}
