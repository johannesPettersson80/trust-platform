//! The ADS client of a compiled program (specification 44 §13.3 with specification 11's ADS
//! client): a point's declared type is checked against the shape of its global's type, and its
//! samples and outputs reach the global by name at the boundaries of the scan.

use std::collections::BTreeMap;

use trust_ads_core::{ArrayDimension, PointAccess, UpdateMode};

use super::*;
use crate::ads::{AdsNotificationMode, AdsPointAddress};
use crate::runtime::NamedGlobals;

/// The descriptor of an ADS `iec_type` declared as `name`.
fn descriptor(name: &str, iec_type: IecDataType) -> AdsDataTypeDescriptor {
    AdsDataTypeDescriptor::scalar(name, iec_type)
}

/// Specification 11's ADS client checks a point's declared type against its global's, here the
/// compiled program's type shapes: an `INT` against `INT` and not `DINT`; a `STRING(20)` against a
/// `STRING[20]` and not a `STRING[10]`; an `INT` against an enumeration of 2 bytes, whose base the
/// image does not name, and not a `DINT`; `ARRAY[1..3] OF INT` against the same bounds and not
/// `[0..2]`; never a `WSTRING` or a structure. Catches a compiled program that binds a point to a
/// global of another type, or refuses one of the same type.
#[test]
fn a_points_declared_type_is_checked_against_the_globals_shape() {
    let int = TypeShape::Elementary(type_kind::INT);
    assert!(ads_descriptor_matches_shape(
        &descriptor("INT", IecDataType::Int),
        &int
    ));
    assert!(!ads_descriptor_matches_shape(
        &descriptor("INT", IecDataType::Int),
        &TypeShape::Elementary(type_kind::DINT)
    ));

    let string = AdsDataTypeDescriptor {
        string_len: Some(20),
        ..descriptor("STRING(20)", IecDataType::String)
    };
    let text = |capacity| TypeShape::Text {
        wide: false,
        capacity,
    };
    assert!(ads_descriptor_matches_shape(&string, &text(20)));
    assert!(!ads_descriptor_matches_shape(&string, &text(10)));

    let mode = TypeShape::Enumeration {
        name: SmolStr::new("Mode"),
        size: 2,
    };
    assert!(ads_descriptor_matches_shape(
        &descriptor("INT", IecDataType::Int),
        &mode
    ));
    assert!(!ads_descriptor_matches_shape(
        &descriptor("DINT", IecDataType::Dint),
        &mode
    ));

    let array = AdsDataTypeDescriptor {
        dimensions: vec![ArrayDimension { lower: 1, upper: 3 }],
        ..descriptor("ARRAY[1..3] OF INT", IecDataType::Int)
    };
    let shape = |lower, upper| TypeShape::Array {
        element: Box::new(int.clone()),
        dimensions: vec![(lower, upper)],
    };
    assert!(ads_descriptor_matches_shape(&array, &shape(1, 3)));
    assert!(!ads_descriptor_matches_shape(&array, &shape(0, 2)));

    assert!(!ads_descriptor_matches_shape(
        &string,
        &TypeShape::Text {
            wide: true,
            capacity: 20
        }
    ));
    assert!(!ads_descriptor_matches_shape(
        &descriptor("INT", IecDataType::Int),
        &TypeShape::Other
    ));
}

/// Specification 44 §13.3 with specification 11's ADS client: a compiled program's point reaches
/// its global by name. The good sample 2 of the read-write point is written to the global `Speed`
/// after the inputs, and its quality `Good` to `Speed_quality`, while `Other` keeps 0; after the
/// scan, when `Speed` holds 7, 7 is queued for the PLC. Catches a compiled program whose inputs
/// never reach its globals, whose quality is not written, whose outputs are never queued, or a
/// point that reaches another global.
#[test]
fn a_compiled_programs_point_reaches_its_global_by_name() {
    let binding = AdsBinding {
        point: AdsPointConfig {
            point_name: "Speed".into(),
            address: AdsPointAddress::Symbol("Remote.Speed".into()),
            data_type: descriptor("INT", IecDataType::Int),
            access: PointAccess::ReadWrite,
            mode: UpdateMode::Poll,
            notification_mode: AdsNotificationMode::OnChange,
            allow_retain_read: false,
        },
        target: BoundGlobal::Compiled("Speed".into()),
        quality_target: Some(BoundGlobal::Compiled("Speed_quality".into())),
        retain: false,
    };
    let mut bridge = AdsConnectionBridge::new(vec![binding]).unwrap();
    bridge.shared.set_value("Speed", Value::Int(2));
    bridge.shared.set_quality("Speed", PointQuality::good(10));
    let mut globals = NamedGlobals(BTreeMap::from([
        ("Speed", Value::Int(0)),
        ("Speed_quality", quality_value(QualityState::Stale)),
        ("Other", Value::Int(0)),
    ]));

    bridge.apply_inputs_compiled(&mut globals, 10).unwrap();
    assert_eq!(globals.0["Speed"], Value::Int(2));
    assert_eq!(
        globals.0["Speed_quality"],
        quality_value(QualityState::Good)
    );
    assert_eq!(globals.0["Other"], Value::Int(0));

    globals.0.insert("Speed", Value::Int(7));
    bridge.capture_outputs_compiled(&mut globals, 20).unwrap();
    assert_eq!(bridge.pending_write("Speed"), Some(Value::Int(7)));
}
