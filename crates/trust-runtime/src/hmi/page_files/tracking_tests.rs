//! Specification 39, §8.5 and §10.5 item 3: a controller's definition names how the controller
//! tracks the equipment its output drives, `[program.tracking]`, read and written back as it is,
//! and only with identifiers, because Create PLC code writes them into the program.

use super::*;

const FILE: &str = "hmi/pairs/plant-pid.toml";

const TRACKED: &str = "format = 1\nblock = \"Control.PID\"\nfaceplate = \"plant-pid\"\nsymbol = \"controller\"\n\n[program]\nposition = \"controllers\"\n\n[program.tracking]\ninterface = \"Plant.IControlledDevice\"\nfollows = \"FollowsAuto\"\nvalue = \"GetActualValue\"\n";

/// Catches a tracking table that is dropped on reading or on writing back.
#[test]
fn a_controllers_tracking_is_read_and_written_back() {
    let pair = parse_pair(FILE, TRACKED).expect("the definition loads");
    let program = pair.program.as_ref().expect("its program rules");
    assert_eq!(
        program.tracking,
        Some(HmiPairTracking {
            interface: "Plant.IControlledDevice".into(),
            follows: "FollowsAuto".into(),
            value: "GetActualValue".into(),
        })
    );
    assert_eq!(write_pair(&pair), TRACKED, "written back as read");
}

/// Catches a name that would put anything but a call into the generated program, and a table
/// with a missing or unknown key.
#[test]
fn tracking_names_only_identifiers_and_its_three_keys() {
    for (wrong, expected) in [
        (
            "follows = \"FollowsAuto\"",
            "follows = \"FollowsAuto(); Valve.Open\"",
        ),
        ("value = \"GetActualValue\"", "value = \"Get Value\""),
        (
            "interface = \"Plant.IControlledDevice\"",
            "interface = \"Plant..IControlledDevice\"",
        ),
    ] {
        let text = TRACKED.replace(wrong, expected);
        let errors = parse_pair(FILE, &text).expect_err(expected);
        assert!(
            (9..=12).contains(&errors[0].line),
            "at the tracking table: {errors:?}"
        );
        assert!(
            errors[0].message.contains("each an identifier"),
            "{}",
            errors[0].message
        );
    }
    let missing = TRACKED.replace("value = \"GetActualValue\"\n", "");
    assert!(parse_pair(FILE, &missing).is_err(), "value is required");
    let unknown = format!("{TRACKED}when = \"Auto\"\n");
    assert!(
        parse_pair(FILE, &unknown).is_err(),
        "an unknown key is refused"
    );
}
