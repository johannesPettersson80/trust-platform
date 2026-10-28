//! Identify contracts (specification 11, "Identify"): the request, its refusals, the blink on a
//! fake clock and a fake output, the end of a run, and `status`. Every expected time is worked out
//! by hand from the specification: the output is on for 250 ms and then off for 250 ms, counted
//! from the first tick after the request.

use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use serde_json::{json, Value};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::scheduler::ResourceState;

use super::*;
use crate::access::AccessRole;
use crate::dispatch::dispatch_value;
use crate::tests::fake_host::{FakeHost, FakeIdentify, RecordingOutput};
use crate::tests::runner_ports::{ms, Fixture, TestClock};

/// A local client: it acts as `Admin` when the host has no token.
const LOCAL: Option<&str> = Some("unix");
/// A network client without a token: it acts as `Viewer`.
const NETWORK: Option<&str> = Some("127.0.0.1:9000");

/// The refusal text of every problem with the parameters (specification 11, "Identify").
const INVALID_DURATION: &str =
    "invalid params: duration_s must be a whole number of seconds from 1 to 60";

fn request(params: Value) -> Value {
    let mut asked = json!({"id": 1, "type": "identify"});
    asked["params"] = params;
    asked
}

fn identify_for(seconds: u64) -> Value {
    request(json!({"duration_s": seconds}))
}

fn result(response: &ControlResponse) -> Value {
    assert!(response.ok, "request failed: {:?}", response.error);
    response.result_value().expect("response result")
}

/// The `identifying` member of the host's `status`.
fn status_identifying(host: &FakeHost) -> bool {
    let status = dispatch_value(json!({"id": 9, "type": "status"}), host, LOCAL);
    result(&status)["identifying"]
        .as_bool()
        .expect("status reports identifying as a Boolean")
}

/// A running host whose identify output reads `clock`, with its state and the output's drives.
fn configured(clock: &TestClock) -> (FakeHost, Rc<RefCell<FakeIdentify>>, RecordingOutput) {
    FakeHost::running().with_identify_output(clock)
}

/// Ticks the identify state at each of `times` milliseconds.
fn tick_at(clock: &TestClock, identify: &Rc<RefCell<FakeIdentify>>, times: &[u64]) {
    for &at in times {
        clock.set(ms(at));
        identify
            .borrow_mut()
            .tick()
            .expect("the output can be driven");
    }
}

#[test]
fn identify_blinks_the_configured_output_for_its_duration() {
    // Specification 11, "Identify": the output is on for 250 ms and off for 250 ms (2 Hz, 50 %),
    // starting on, and is off when the duration has passed. A request of 1 s, ticked every 50 ms,
    // turns the output on at 0, off at 250, on at 500, off at 750, and the second is over at 1000,
    // where the output is already off. Catches a wrong rate (1 Hz would first turn it off at
    // 500), a blink that starts off, a duty cycle other than 50 %, and an output that is driven
    // again at the end although it is off.
    let clock = TestClock::default();
    let (host, identify, output) = configured(&clock);
    let response = dispatch_value(identify_for(1), &host, LOCAL);
    assert_eq!(
        result(&response),
        json!({"identifying": true, "duration_s": 1})
    );

    let every_50_ms: Vec<u64> = (0..=22).map(|step| step * 50).collect();
    tick_at(&clock, &identify, &every_50_ms);

    assert_eq!(
        output.drives(),
        vec![(0, true), (250, false), (500, true), (750, false)]
    );
    assert!(!identify.borrow().is_identifying());
}

#[test]
fn identify_ends_with_the_output_off_when_its_time_passes_while_it_is_on() {
    // Specification 11, "Identify", ending: the output is off from the first tick at or after the
    // end. Ticks at 0, 300, 600 and 1100 ms of a 1 s request turn the output on at 0, off at 300
    // (the first tick at or after the 250 edge), on at 600 (the next edge stays on the 250 ms
    // schedule, at 500) and, with the duration over, off at 1100. Catches an output left on after
    // the time has passed.
    let clock = TestClock::default();
    let (host, identify, output) = configured(&clock);
    let _ = dispatch_value(identify_for(1), &host, LOCAL);

    tick_at(&clock, &identify, &[0, 300, 600, 1100]);

    assert_eq!(
        output.drives(),
        vec![(0, true), (300, false), (600, true), (1100, false)]
    );
}

#[test]
fn a_cycle_longer_than_the_half_period_toggles_the_output_at_every_tick() {
    // Specification 11, "Identify", the blink: the runtime changes the output at the first
    // publication at or after each 250 ms edge and at most once at each, so with a 1 s cycle it
    // changes once per cycle. Ticks at 0, 1000, 2000 and 3000 ms turn the output on, off, on and
    // off. Catches a blink computed from the clock alone, which a 1 s cycle (a multiple of the
    // 500 ms period) would sample at the same phase every time and show as a constant level.
    let clock = TestClock::default();
    let (host, identify, output) = configured(&clock);
    let _ = dispatch_value(identify_for(5), &host, LOCAL);

    tick_at(&clock, &identify, &[0, 1000, 2000, 3000]);

    assert_eq!(
        output.drives(),
        vec![(0, true), (1000, false), (2000, true), (3000, false)]
    );
}

#[test]
fn the_first_tick_starts_the_blink_with_the_output_on_however_late_it_comes() {
    // Specification 11, "Identify", the blink: it starts on at the first publication after the
    // request, and the 250 ms count from there. A first tick at 400 ms turns the output on at 400
    // and off at 650. Catches a blink counted from the request, which would find the first edge
    // already due at 400 and never show the output on.
    let clock = TestClock::default();
    let (host, identify, output) = configured(&clock);
    let _ = dispatch_value(identify_for(5), &host, LOCAL);

    tick_at(&clock, &identify, &[400, 649, 650]);

    assert_eq!(output.drives(), vec![(400, true), (650, false)]);
}

#[test]
fn a_repeated_identify_replaces_the_run_and_starts_the_blink_again() {
    // Specification 11, "Identify", ending: a repeated request replaces the run; the end is the new
    // duration after the repeat, and the blink starts again with the output on. A request of 1 s at
    // 0, ticked every 50 ms, blinks as above up to 750 (off). Repeated with 1 s at 800, the output
    // turns on at 800, off at 1050 (250 after 800), on at 1300, off at 1550, and the run ends at
    // 1800, not at 1000. Catches a repeat that is ignored, one that keeps the old end, and one that
    // keeps the old phase.
    let clock = TestClock::default();
    let (host, identify, output) = configured(&clock);
    let _ = dispatch_value(identify_for(1), &host, LOCAL);
    let first: Vec<u64> = (0..=15).map(|step| step * 50).collect();
    tick_at(&clock, &identify, &first);
    assert_eq!(clock.now().since_origin().as_millis(), 750);

    clock.set(ms(800));
    let repeated = dispatch_value(identify_for(1), &host, LOCAL);
    assert_eq!(
        result(&repeated),
        json!({"identifying": true, "duration_s": 1})
    );
    let rest: Vec<u64> = (16..=38).map(|step| step * 50).collect();
    tick_at(&clock, &identify, &rest);

    assert_eq!(
        output.drives(),
        vec![
            (0, true),
            (250, false),
            (500, true),
            (750, false),
            (800, true),
            (1050, false),
            (1300, true),
            (1550, false),
        ]
    );
    assert!(!identify.borrow().is_identifying());
}

#[test]
fn cancel_drives_the_output_off_at_once_and_ends_identify() {
    // Specification 11, "Identify", ending: a stop or a fault of the resource ends identify at
    // once and drives the output off. Cancelled at 100 ms, while the output is on, the output goes
    // off at 100 and stays off at later ticks; cancelled again, or cancelled when nothing runs, it
    // is not driven. Catches an output left on after the resource stopped or faulted, and one
    // driven off twice.
    let clock = TestClock::default();
    let (host, identify, output) = configured(&clock);
    let _ = dispatch_value(identify_for(10), &host, LOCAL);
    tick_at(&clock, &identify, &[0]);

    clock.set(ms(100));
    identify
        .borrow_mut()
        .cancel()
        .expect("the output can be driven");
    assert!(!status_identifying(&host), "cancelled identify is over");
    identify.borrow_mut().cancel().expect("nothing to drive");
    tick_at(&clock, &identify, &[250, 500, 750]);

    assert_eq!(output.drives(), vec![(0, true), (100, false)]);
}

#[test]
fn identify_without_setting_answers_identify_not_configured() {
    // Specification 11, "Identify": without `[runtime.identify] output` the request answers the
    // error code `identify_not_configured`, and `status` reports `identifying: false`. Catches a
    // request that starts something or answers ok on a runtime without an output, a missing or
    // different code, and a `status` without the member.
    let host = FakeHost::running();
    let response = dispatch_value(identify_for(10), &host, LOCAL);

    assert!(!response.ok);
    assert_eq!(
        response.error_code.as_deref(),
        Some("identify_not_configured")
    );
    assert_eq!(
        response.error.as_deref(),
        Some("identify is not configured: set [runtime.identify] output in runtime.toml")
    );
    assert!(!status_identifying(&host));
}

#[test]
fn a_chip_answers_identify_not_configured() {
    // Specification 11, "Identify", and specification 34, SERVICE-07: a runtime on a chip answers
    // `identify_not_configured` and reports `identifying: false` until the chip's firmware drives
    // an output. Catches a loop that answers `unsupported request` for the new operation, one that
    // pretends to blink, and a `status` without the member.
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    // The first scan is due at once, and a request waits for it.
    fixture.pass_at(&mut runner, 0);

    let reply = fixture.call(&mut runner, &identify_for(10));
    assert_eq!(reply["ok"], json!(false));
    assert_eq!(reply["error_code"], json!("identify_not_configured"));

    let status = fixture.call(&mut runner, &json!({"id": 2, "type": "status"}));
    assert_eq!(status["result"]["identifying"], json!(false));
}

#[test]
fn identify_duration_outside_1_to_60_is_rejected() {
    // Specification 11, "Identify", request: the duration is a whole number of seconds from 1
    // through 60, and nothing else is accepted. 0 and 61 are refused and 1 and 60 are accepted;
    // 256 would be 0 if truncated to a byte; a string, a fraction, a negative number, null, a
    // missing or non-object parameter list and an extra member are refused with the same text,
    // and none of them makes the output ready or starts anything. Catches an off-by-one at either
    // end, a truncating conversion, a duration read leniently from a string or a fraction, and
    // an unknown member that is ignored.
    let clock = TestClock::default();
    let (host, _identify, output) = configured(&clock);

    for bad in [
        request(json!({"duration_s": 0})),
        request(json!({"duration_s": 61})),
        request(json!({"duration_s": 256})),
        request(json!({"duration_s": u64::MAX})),
        request(json!({"duration_s": -1})),
        request(json!({"duration_s": 10.5})),
        request(json!({"duration_s": 10.0})),
        request(json!({"duration_s": "10"})),
        request(json!({"duration_s": null})),
        request(json!({"duration_s": true})),
        request(json!({})),
        request(json!({"duration_s": 10, "extra": 1})),
        request(json!({"seconds": 10})),
        request(json!(10)),
        request(json!([10])),
        json!({"id": 1, "type": "identify"}),
    ] {
        let response = dispatch_value(bad.clone(), &host, LOCAL);
        assert!(!response.ok, "request {bad} is refused");
        assert_eq!(
            response.error.as_deref(),
            Some(INVALID_DURATION),
            "request {bad}"
        );
    }
    assert_eq!(
        output.prepares.get(),
        0,
        "no refused request prepares the output"
    );
    assert!(!status_identifying(&host));

    for seconds in [1_u64, 60] {
        let response = dispatch_value(identify_for(seconds), &host, LOCAL);
        assert_eq!(
            result(&response),
            json!({"identifying": true, "duration_s": seconds}),
            "seconds={seconds}"
        );
    }
}

#[test]
fn status_reports_identifying_until_it_ends() {
    // Specification 11, "Identify", status: `identifying` is true from the accepted request until
    // the duration has passed. A request of 2 s at 0 is identifying at 0 and at 1999 ms, and not at
    // 2000 ms, with no tick in between: the member follows the clock, not the ticks. Catches a
    // member that is false while the output blinks, one that stays true until a tick, and an
    // off-by-one at the end.
    let clock = TestClock::default();
    let (host, _identify, _output) = configured(&clock);
    assert!(!status_identifying(&host), "nothing was asked yet");

    let _ = dispatch_value(identify_for(2), &host, LOCAL);
    assert!(status_identifying(&host));
    clock.set(ms(1999));
    assert!(status_identifying(&host));
    clock.set(ms(2000));
    assert!(!status_identifying(&host));
}

#[test]
fn identify_needs_the_operator_role() {
    // Specification 11, §6.9.5, the role table: `identify` is an Operator operation. A Viewer is
    // refused with `insufficient_role` before anything is made ready; an Operator, an Engineer and
    // an Admin are served. Catches an operation registered as Viewer (anyone on the network could
    // blink an output) or left to the Admin fail-safe.
    for (role, allowed) in [
        (AccessRole::Viewer, false),
        (AccessRole::Operator, true),
        (AccessRole::Engineer, true),
        (AccessRole::Admin, true),
    ] {
        let clock = TestClock::default();
        let (mut host, _identify, output) = configured(&clock);
        host.control_token = Some("secret");
        host.requires_auth = true;
        host.pairing = Some(("paired", role));
        let mut asked = identify_for(5);
        asked["auth"] = json!("paired");

        let response = dispatch_value(asked, &host, NETWORK);

        assert_eq!(response.ok, allowed, "role={role:?}");
        if allowed {
            assert_eq!(
                result(&response),
                json!({"identifying": true, "duration_s": 5}),
                "role={role:?}"
            );
        } else {
            assert_eq!(
                response.error.as_deref(),
                Some("forbidden: requires role operator")
            );
            assert_eq!(response.error_code.as_deref(), Some("insufficient_role"));
            assert_eq!(output.prepares.get(), 0, "a refused request starts nothing");
        }
    }

    let viewer = dispatch_value(identify_for(5), &FakeHost::running(), NETWORK);
    assert_eq!(viewer.error_code.as_deref(), Some("insufficient_role"));
}

#[test]
fn identify_is_accepted_in_every_state_of_the_resource() {
    // Specification 11, "Identify", request: `identify` has no restriction on the state of the
    // resource; a stopped box is exactly when someone needs to find it (the owner's rule: no
    // restriction without his approval). In each of the five states the request is accepted with
    // the same result, makes the output ready once, and `status` reports `identifying`. Catches a
    // request that is refused (`resource_not_running`) while the resource is not running.
    for state in [
        ResourceState::Boot,
        ResourceState::Ready,
        ResourceState::Running,
        ResourceState::Paused,
        ResourceState::Faulted,
        ResourceState::Stopped,
    ] {
        let clock = TestClock::default();
        let (host, _identify, output) = configured(&clock);
        host.state.set(state);

        let response = dispatch_value(identify_for(5), &host, LOCAL);

        assert_eq!(
            result(&response),
            json!({"identifying": true, "duration_s": 5}),
            "state={state:?}"
        );
        assert_eq!(output.prepares.get(), 1, "state={state:?}");
        assert!(status_identifying(&host), "state={state:?}");
    }
}

#[test]
fn identify_blinks_while_the_resource_is_stopped() {
    // Specification 11, "Identify", the blink: a stopped resource scans no cycle, and the
    // platform ticks identify in its stopped wait instead (every ~10 ms on a computer). A request
    // on a Stopped resource, ticked every 10 ms for 1 s, turns the output on at 0, off at 250, on
    // at 500 and off at 750, as it does while running; at 1000 ms the run is over and the output is
    // already off. Catches a service that depends on the state of the resource for its blink.
    let clock = TestClock::default();
    let (host, identify, output) = configured(&clock);
    host.state.set(ResourceState::Stopped);
    let _ = dispatch_value(identify_for(1), &host, LOCAL);

    let every_10_ms: Vec<u64> = (0..=105).map(|step| step * 10).collect();
    tick_at(&clock, &identify, &every_10_ms);

    assert_eq!(
        output.drives(),
        vec![(0, true), (250, false), (500, true), (750, false)]
    );
    assert!(!status_identifying(&host), "the second is over");
}

#[test]
fn refusals_are_checked_in_the_order_the_specification_gives() {
    // Specification 11, "Identify", other refusals: the parameters first, then whether the
    // runtime has an output, then whether the output can be made ready. A bad duration on a runtime
    // without an output is `invalid params`; a good one is `identify_not_configured`; with an
    // output that cannot be made ready, a bad duration is still `invalid params` and a good one is
    // `identify_output_unavailable`. The state of the resource decides nothing: a faulted
    // resource answers the same. Catches checks in another order.
    let unconfigured = FakeHost::running();
    let bad = dispatch_value(identify_for(0), &unconfigured, LOCAL);
    assert_eq!(bad.error.as_deref(), Some(INVALID_DURATION));
    let good = dispatch_value(identify_for(5), &unconfigured, LOCAL);
    assert_eq!(good.error_code.as_deref(), Some("identify_not_configured"));

    let clock = TestClock::default();
    let (faulted, _identify, output) = configured(&clock);
    faulted.state.set(ResourceState::Faulted);
    *output.prepare_failure.borrow_mut() = Some(RuntimeError::IoDriver("busy".into()));
    let bad = dispatch_value(identify_for(0), &faulted, LOCAL);
    assert_eq!(bad.error.as_deref(), Some(INVALID_DURATION));
    let good = dispatch_value(identify_for(5), &faulted, LOCAL);
    assert_eq!(
        good.error_code.as_deref(),
        Some("identify_output_unavailable")
    );
}

#[test]
fn an_output_that_cannot_be_made_ready_refuses_identify() {
    // Specification 11, "Identify", other refusals: an output that cannot be made ready answers
    // `identify_output_unavailable` with the reason, and starts nothing. Catches an error that is
    // dropped (the request answers ok and nothing ever blinks) and one that leaves a run behind.
    let clock = TestClock::default();
    let (host, identify, output) = configured(&clock);
    *output.prepare_failure.borrow_mut() = Some(RuntimeError::IoDriver(
        "gpio chardev request line 17 failed".into(),
    ));

    let response = dispatch_value(identify_for(5), &host, LOCAL);

    assert_eq!(
        response.error_code.as_deref(),
        Some("identify_output_unavailable")
    );
    assert_eq!(
        response.error.as_deref(),
        Some("identify output unavailable: i/o driver error 'gpio chardev request line 17 failed'")
    );
    assert!(!status_identifying(&host));
    tick_at(&clock, &identify, &[0, 250]);
    assert_eq!(output.drives(), vec![], "nothing was started");
}

#[test]
fn a_failure_to_drive_the_output_ends_identify() {
    // Specification 11, "Identify", the blink: a failure to drive the output ends identify. The
    // failed tick reports the error, `status` is false afterwards, and a later tick drives
    // nothing. Catches a run that goes on after its output failed, which would report
    // `identifying: true` for an output that no longer blinks.
    let clock = TestClock::default();
    let (host, identify, output) = configured(&clock);
    let _ = dispatch_value(identify_for(5), &host, LOCAL);
    *output.drive_failure.borrow_mut() = Some(RuntimeError::IoDriver("write failed".into()));

    let failed = identify.borrow_mut().tick();

    assert_eq!(failed, Err(RuntimeError::IoDriver("write failed".into())));
    assert!(!status_identifying(&host));
    *output.drive_failure.borrow_mut() = None;
    tick_at(&clock, &identify, &[250, 500]);
    assert_eq!(output.drives(), vec![]);
}
