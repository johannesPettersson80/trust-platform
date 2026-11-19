//! The typed handles and the JSON forms a service writes (specification 48 RU-62; specification 11
//! §6.8.1, "Writing variables"). Every expected value is worked out by hand.

use std::time::Duration;

use serde_json::json;

use super::{ToVariable, Variable, Writable};

#[test]
fn each_rust_value_is_written_in_its_types_json_form() {
    // A bool and the integers as they are; 0.1 as an f32 is written as its shortest text 0.1, which
    // the runtime reads back as the same single-precision value, not as 0.10000000149011612; NaN
    // and the infinities as variables.read writes them; 1.5 s as 1,500,000,000 ns. Catches a REAL
    // written through f64's longer text and a TIME written in another unit.
    assert_eq!(true.to_variable(), json!(true));
    assert_eq!((-3_i16).to_variable(), json!(-3));
    assert_eq!(u64::MAX.to_variable(), json!(u64::MAX));
    assert_eq!(0.1_f32.to_variable(), json!(0.1));
    assert_eq!(21.5_f64.to_variable(), json!(21.5));
    assert_eq!(f64::NAN.to_variable(), json!("NaN"));
    assert_eq!(f32::NEG_INFINITY.to_variable(), json!("-Infinity"));
    assert_eq!("pump".to_variable(), json!("pump"));
    assert_eq!(String::from("pump").to_variable(), json!("pump"));
    assert_eq!(
        Duration::from_millis(1500).to_variable(),
        json!(1_500_000_000)
    );
}

#[test]
fn a_writable_handle_reads_as_its_variable() {
    // A writable handle and the variable it reads name the same path. Catches a handle whose read
    // names another variable than its write.
    const TARGET: Writable<f64> = Writable::new("Recipe.target");
    const LEVEL: Variable<f64> = Variable::new("P1.level");
    assert_eq!(TARGET.path(), "Recipe.target");
    assert_eq!(TARGET.variable().path(), "Recipe.target");
    assert_eq!(LEVEL.path(), "P1.level");
    assert_eq!(
        format!("{TARGET:?} {LEVEL:?}"),
        "Writable(\"Recipe.target\") Variable(\"P1.level\")"
    );
}
