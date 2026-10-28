//! A compiled program's debug operations through the resource loop and the shared dispatcher
//! (specification 44 §13.2): the loop answers them from the compiled cycle's debugger as it answers
//! them for the interpreter's program, and refuses them for a release image with §13.2's text. On
//! the C6 bench of 24 October 2026 `breakpoints.set`, `debug.state` and `debug.stack` answered
//! `unsupported request` for a debug image, because the loop asked only for the interpreter's debug
//! view; the cycle's own tests did not go through the loop.

use alloc::vec::Vec;

use serde_json::{json, Value as Json};
use trust_runtime_core::native::KERNEL_FUNCTIONS;

use super::tests::{cycle_with, image_of, image_with, Cycle};
use crate::debug::replies::NO_DEBUG_HOOKS;
use crate::runner::Runner;
use crate::tests::runner_platform::TestPlatform;
use crate::tests::runner_ports::{ms, TestClock, TestLink, TestWait};

type Loop<'i> = Runner<TestLink, TestWait, Cycle<'i>, TestPlatform, TestClock>;

/// The loop of the image `bytes` after its first pass, which runs the scan due at power-on, with its
/// link and its clock at 2 ms, and the debugger enabled on the platform.
fn running(bytes: &[u8]) -> (Loop<'_>, TestLink) {
    let clock = TestClock::default();
    let link = TestLink::new(4_096, clock.clone());
    let (cycle, _, _) = cycle_with(bytes, Vec::new(), &KERNEL_FUNCTIONS);
    let platform = TestPlatform {
        debug_enabled: true,
        ..TestPlatform::default()
    };
    let mut runner = Runner::new(
        link.clone(),
        TestWait::new(clock.clone()),
        cycle,
        platform,
        clock.clone(),
        64,
    );
    runner.run_pass().expect("the test wait does not fail");
    clock.set(ms(2));
    (runner, link)
}

/// Sends `request`, runs one pass and returns the one reply.
fn call(runner: &mut Loop<'_>, link: &TestLink, request: &Json) -> Json {
    link.send(request);
    runner.run_pass().expect("the test wait does not fail");
    let mut replies = link.take_replies();
    assert_eq!(replies.len(), 1, "one reply to {request}: {replies:?}");
    replies.remove(0)
}

#[test]
fn the_loop_answers_the_debug_operations_of_a_compiled_debug_image() {
    // Specification 44 §13.2 with specification 11 §6.13.3: the debug image's statements start at
    // 1:1 and 2:1 of `main.st` (one-based), so the zero-based line 1 resolves to the statement at
    // 2:1, reported zero-based as line 1, column 0, of file 0, in the file's first generation; and
    // `debug.state` answers between scans. Catches a loop that leaves a compiled program's debug
    // operations to the platform (`unsupported request`).
    let bytes = image_of(48, true);
    let (mut runner, link) = running(&bytes);
    let set = call(
        &mut runner,
        &link,
        &json!({"id": 1, "type": "breakpoints.set", "params": {"source": "main.st", "lines": [1]}}),
    );
    assert_eq!(
        set["result"],
        json!({"status": "ok", "file_id": 0, "resolved": [{"line": 1, "column": 0}], "generation": 1}),
        "{set}"
    );
    let state = call(&mut runner, &link, &json!({"id": 2, "type": "debug.state"}));
    assert_eq!(state["ok"], json!(true), "{state}");
}

#[test]
fn the_loop_refuses_the_debug_operations_of_a_release_image_with_its_reason() {
    // Specification 44 §13.2: a release image has no statement hooks, and every debug operation on
    // it is refused with the text that says so, not `unsupported request`. Catches a loop that
    // leaves them to the platform, and one that answers them as if the image could stop.
    let bytes = image_with(0, false, true);
    let (mut runner, link) = running(&bytes);
    let set = call(
        &mut runner,
        &link,
        &json!({"id": 1, "type": "breakpoints.set", "params": {"source": "main.st", "lines": [1]}}),
    );
    assert_eq!(set["ok"], json!(false), "{set}");
    assert_eq!(set["error"], json!(NO_DEBUG_HOOKS), "{set}");
}
