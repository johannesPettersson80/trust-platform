//! A `restart` request to the simulator's control server restarts its program (specification 13; specification 11
//! §6.7), as trust-runtime's scheduler takes one. The test needs the toolchain of specification 44 §4.6 at
//! `TRUST_TOOLCHAIN`.

use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;
use trust_runtime::control::ControlState;
use trust_runtime::value::Value;

use super::simulation_speed_tests::{project_with, published_output, simulator};
use super::tests_part_02::{assert_control_ok, reserve_loopback_addr, send_control_request};

/// Counts its scans at `%QD0`, from its starting value 0.
const COUNTER: &str =
    "PROGRAM Main\nVAR\n    Count AT %QD0 : DINT := 0;\nEND_VAR\nCount := Count + 1;\nEND_PROGRAM\n";

/// The count at `%QD0` in the last I/O snapshot the simulator published.
fn published_count(state: &ControlState) -> Option<i64> {
    match published_output(state, "%QD0")? {
        Value::DInt(count) => Some(i64::from(count)),
        Value::DWord(bits) => Some(i64::from(bits.cast_signed())),
        _ => None,
    }
}

/// Waits up to ten seconds for a published count that `wanted` takes, and returns it.
fn wait_for_count(state: &ControlState, wanted: impl Fn(i64) -> bool) -> Option<i64> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Some(count) = published_count(state).filter(|count| wanted(*count)) {
            return Some(count);
        }
        thread::sleep(Duration::from_millis(5));
    }
    None
}

/// Specification 13 and specification 11 §6.7, worked by hand: the program counts its scans from 0, so without a restart
/// its count only grows; a cold restart gives `Count` its starting value 0 again, and the scans after it publish a count
/// below the one reached before. Catches a restart that the control server answers "restart queued" but that the
/// simulator's scheduler never takes.
#[test]
fn a_restart_request_restarts_the_simulators_program() {
    let root = project_with("restart", COUNTER, None, None);
    let addr = reserve_loopback_addr();
    let auth = "restart-token";
    let (_session, simulator) = simulator(&root, addr, auth);
    let state = simulator.control_server().state();

    let reached = wait_for_count(&state, |count| count >= 20);
    let answer = send_control_request(
        addr,
        auth,
        json!({"id": 7, "type": "restart", "params": {"mode": "cold"}}),
    );
    let restarted = reached.and_then(|reached| wait_for_count(&state, |count| count < reached));
    simulator.stop();
    let reached = reached.expect("the program counts its scans");
    assert_control_ok(&answer, "restart");
    assert!(
        restarted.is_some(),
        "the count went on from {reached} without falling back after the restart"
    );
    fs::remove_dir_all(root).ok();
}
