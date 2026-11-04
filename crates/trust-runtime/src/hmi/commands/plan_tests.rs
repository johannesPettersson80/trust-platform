//! Specification 39, §12.8 item 5: which commands the runtime follows after their first answer,
//! reporting their final outcome later on the value stream, and which are final as `applied`; and
//! how long after the answer it decides that outcome.

use std::time::Duration;

use serde_json::{json, Value as Json};

use super::{plan, Plan};
use crate::hmi::catalogue::Catalogue;
use crate::hmi::page_files::parse_page;

/// The plan of the command `action` (with `value`) to the only control of a page holding `object`.
fn plan_of(object: &str, action: &str, value: Option<Json>) -> Plan {
    let text = format!("format = 1\ntitle = \"Controls\"\n\n[[object]]\n{object}");
    let page = parse_page("hmi/pages/controls.toml", &text).expect("the page loads");
    let control = &page.objects[0];
    let component = Catalogue::shipped()
        .component(&control.kind)
        .expect("a shipped component");
    plan(control, component, action, value.as_ref(), None).expect("a command that fits")
}

/// Catches: a command whose control declares no confirmation followed for a later outcome that never
/// comes, so that whoever waits for it times out; such a command is final as applied.
#[test]
fn a_button_without_feedback_is_final_as_applied() {
    let button = r#"id = "stop"
type = "button"
at = [0, 0]
size = [120, 40]
action = "reset"

[object.bind]
target = { path = "Main.Run", direction = "write" }
"#;
    assert!(!plan_of(button, "reset", None).is_followed());
}

/// Catches: a command its control confirms by its feedback ("by `feedback` reaching `expect`") not
/// followed, so that its later `confirmed` or `timed-out` is never waited for.
#[test]
fn a_control_with_feedback_is_followed() {
    let button = r#"id = "start"
type = "button"
at = [0, 0]
size = [120, 40]
action = "set"

[object.bind]
feedback = "Main.Running"
target = { path = "Main.Start", direction = "write" }
"#;
    assert!(plan_of(button, "set", None).is_followed());
}

/// Catches: a written value not followed although the runtime confirms it by the value read back
/// ("for a written value without `feedback`, the target reads back the requested value").
#[test]
fn a_written_value_is_followed() {
    let setpoint = r#"id = "sp"
type = "setpoint"
at = [0, 0]
size = [200, 64]

[object.bind]
value = { path = "Main.SP", direction = "read-write" }
"#;
    assert!(plan_of(setpoint, "write", Some(json!(10))).is_followed());
}

/// Catches: a local pulse taken for a followed command, whose end the runtime writes and reports but
/// whose command is final as applied; and a pulse to a remote runtime's point not followed, although
/// the runtime observes its end there.
#[test]
fn a_pulse_is_followed_only_to_a_remote_point() {
    let pulse = |path: &str| {
        format!(
            "id = \"start\"\ntype = \"pulse-button\"\nat = [0, 0]\nsize = [120, 40]\npulse = 0.5\n\n\
             [object.bind]\ntarget = {{ path = \"{path}\", direction = \"write\" }}\n"
        )
    };
    assert!(!plan_of(&pulse("Main.Start"), "pulse", None).is_followed());
    assert!(plan_of(&pulse("Owner:Main.Start"), "pulse", None).is_followed());
}

/// Catches: a pulse's later outcome expected at its control's confirmation time, although the
/// runtime waits for the pulse's end, up to the longer of the pulse and the confirmation time and
/// the 2 s it allows to write the end, before it decides the confirmation; and a written value's
/// expected later than its confirmation time.
#[test]
fn a_pulse_is_decided_after_its_end() {
    let reset = |pulse: &str| {
        format!(
            "id = \"reset\"\ntype = \"pulse-button\"\nat = [0, 0]\nsize = [120, 40]\npulse = {pulse}\n\
             timeout = 0.3\n\n[object.bind]\ntarget = {{ path = \"Main.Reset\", direction = \"write\" }}\n\
             feedback = \"Main.Tripped\"\n"
        )
    };
    assert_eq!(
        plan_of(&reset("0.4"), "pulse", None).decided_within(),
        Duration::from_millis(2_400)
    );
    assert_eq!(
        plan_of(&reset("0.1"), "pulse", None).decided_within(),
        Duration::from_millis(2_300)
    );
    let setpoint = r#"id = "sp"
type = "setpoint"
at = [0, 0]
size = [200, 64]
timeout = 0.3

[object.bind]
value = { path = "Main.SP", direction = "read-write" }
"#;
    assert_eq!(
        plan_of(setpoint, "write", Some(json!(10))).decided_within(),
        Duration::from_millis(300)
    );
}
