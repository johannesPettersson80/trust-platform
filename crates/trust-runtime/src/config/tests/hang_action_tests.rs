//! `runtime.watchdog.hang_action` parsing and validation (spec 11 section 6.6, hosted hang
//! supervision).

use super::*;

#[test]
fn runtime_watchdog_hang_action_defaults_to_fault_and_accepts_abort() {
    let omitted = parse_runtime_toml_from_text(&runtime_toml(), "runtime.toml")
        .expect("runtime fixture without hang_action parses");
    assert_eq!(
        omitted.watchdog_hang_action,
        crate::watchdog::HangAction::Fault
    );

    let text = runtime_toml().replace(
        "action = \"halt\"\n",
        "action = \"halt\"\nhang_action = \"abort\"\n",
    );
    let configured =
        parse_runtime_toml_from_text(&text, "runtime.toml").expect("hang_action = abort parses");
    assert_eq!(
        configured.watchdog_hang_action,
        crate::watchdog::HangAction::Abort
    );
}

#[test]
fn runtime_watchdog_hang_action_rejects_unknown_values() {
    let text = runtime_toml().replace(
        "action = \"halt\"\n",
        "action = \"halt\"\nhang_action = \"exit\"\n",
    );
    let err = validate_runtime_toml_text(&text).expect_err("unknown hang_action must fail");
    assert!(
        err.to_string()
            .contains("invalid watchdog hang action 'exit'"),
        "{err}"
    );
}
