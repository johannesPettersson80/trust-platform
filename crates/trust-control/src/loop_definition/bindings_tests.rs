//! Specification 35 A6.1: a loop's mode binding is a BOOL with its Manual value, or an enumeration with its Manual and its Auto
//! member, and Control Studio reads a runtime's reading of either as TRUE meaning Manual where the BOOL's Manual value is TRUE.

use serde_json::json;

use super::ModeBinding;

/// Catches an enumeration binding that a loop file cannot hold or loses its Auto member on the way back, and a BOOL binding whose
/// file changes (an `auto_value` written for it would change every BOOL loop's revision).
#[test]
fn a_mode_binding_keeps_its_file_form() {
    let enumeration = json!({ "symbol": "PRG_Tank.eMode", "manual_value": "Manual", "auto_value": "Auto" });
    let decoded: ModeBinding = serde_json::from_value(enumeration.clone()).expect("an enumeration binding decodes");
    assert_eq!(serde_json::to_value(&decoded).expect("it encodes"), enumeration);

    let boolean = json!({ "symbol": "P1.Manual", "manual_value": true });
    let decoded: ModeBinding = serde_json::from_value(boolean.clone()).expect("a BOOL binding decodes");
    assert_eq!(serde_json::to_value(&decoded).expect("it encodes"), boolean);
}

/// Catches an enumeration read as a BOOL whose Manual value is FALSE: the runtime reads an enumeration TRUE in its Manual member
/// (protocol annex E6.3), so a reading of TRUE must mean Manual for it, as it does for a BOOL whose Manual value is TRUE.
#[test]
fn a_reading_means_manual_where_it_equals_the_manual_reading() {
    let enumeration = ModeBinding {
        symbol: "PRG_Tank.eMode".to_owned(),
        manual_value: super::ModeValue::Member("Manual".to_owned()),
        auto_value: Some("Auto".to_owned()),
    };
    assert!(enumeration.manual_reading());
    assert!(enumeration.is_enumeration());
    assert!(!ModeBinding::boolean("P1.AutoMode", false).manual_reading());
    assert!(!ModeBinding::boolean("P1.AutoMode", false).is_enumeration());
}
