//! Tests of the switching controller's setup draft (C3, B1.3): the `onoff` structure is read as a switching draft and is no longer
//! parked by the save. The generator itself needs a compiled catalog of the program, which these tests do not build; its loop is
//! decoded by the loop contract as it is generated.

use serde_json::{json, Value};

use super::check_equipment_times;
use crate::setup::save::{parse_setup_draft, SetupDraft};
use crate::setup::texts::save::parked_label;

/// The cold room's draft as the setup form sends it (`control-studio-loop-setup-form.test.ts`).
fn cold_room_draft() -> Value {
    json!({
        "structure": "onoff", "name": "Cold room", "action": "reverse", "instance": "P1",
        "processVariable": { "symbol": "P1.RoomC", "unit": "degC", "minimum": -10, "maximum": 20 },
        "setpoint": { "symbol": "P1.SetpointC" },
        "manipulatedVariable": { "symbol": "P1.CompressorPct", "unit": "%", "minimum": 0, "maximum": 100 },
        "mode": { "symbol": "P1.AutoMode", "whenTrue": "Auto" }, "manualOutput": "P1.ManualPct",
        "hysteresis": "P1.HysteresisC", "bandConvention": "half-band", "minimumOn": "P1.MinimumOnS",
        "minimumOff": "P1.MinimumOffS", "latch": "P1.CoolingOn", "dwellTimer": "P1.DwellS",
        "equipmentMinimumOnSeconds": 20.0, "equipmentMinimumOffSeconds": 30.0
    })
}

#[test]
fn the_onoff_draft_is_read_as_a_switching_draft_with_its_equipment_limits() {
    match parse_setup_draft(&cold_room_draft()).expect("the draft reads") {
        SetupDraft::OnOff(draft) => {
            assert_eq!(draft.instance, "P1");
            assert_eq!(draft.dwell_timer.as_deref(), Some("P1.DwellS"));
            assert_eq!(draft.equipment_minimum_on_seconds, Some(20.0));
            assert_eq!(draft.equipment_minimum_off_seconds, Some(30.0));
            assert_eq!(
                draft.low_output, None,
                "the output levels default to the output's limits"
            );
        }
        other => panic!("a switching draft is expected, not {other:?}"),
    }
}

#[test]
fn the_switching_structure_is_no_longer_parked() {
    assert!(
        parked_label("onoff").is_none(),
        "the Rust engine runs the switching controller"
    );
}

#[test]
fn a_negative_equipment_minimum_time_is_refused_under_its_field() {
    // `equipmentSeconds` of `loopSetupForm.ts`: zero or more seconds, or blank when the equipment declares none.
    let check = |draft: &Value| match parse_setup_draft(draft).expect("the draft reads") {
        SetupDraft::OnOff(parsed) => check_equipment_times(&parsed),
        other => panic!("a switching draft is expected, not {other:?}"),
    };
    let mut draft = cold_room_draft();
    draft["equipmentMinimumOnSeconds"] = json!(-1.0);
    let failure = check(&draft).expect_err("a negative minimum ON time is refused");
    assert_eq!(
        failure.message,
        "The equipment minimum ON time must be zero or more seconds; leave it blank when the equipment has none."
    );
    assert_eq!(failure.field.as_deref(), Some("onoff.equipmentMinimumOn"));
    draft["equipmentMinimumOnSeconds"] = json!(0.0);
    check(&draft).expect("zero seconds is an equipment that needs no time");
    draft["equipmentMinimumOnSeconds"] = json!(null);
    check(&draft).expect("a blank time declares none");
}

#[test]
fn a_draft_with_an_unknown_field_is_refused_by_its_fields() {
    let mut draft = cold_room_draft();
    draft["unexpected"] = json!(true);
    assert!(
        parse_setup_draft(&draft).is_err(),
        "the draft's fields are closed"
    );
}
