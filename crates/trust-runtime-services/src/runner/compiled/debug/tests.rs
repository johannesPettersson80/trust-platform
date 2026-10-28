//! A debugger's write at a held scan of a compiled program (specification 44 §13.2): the hold
//! writes through the stop it lends to the reads, so the reads at the hold and the held statement
//! see the write, and a refused write leaves the memory as it was; and a device's loop takes
//! `debug.set` while it holds the scan, as trust-runtime does (specification 34: a board works
//! exactly like trust-runtime). The program is the stand-in debug body of the cycle's tests: at
//! 10 ms statement 0 counts `MAIN.COUNT` from 5 to 6, and statement 1, on line 2 of `main.st`,
//! copies the count's lowest bit to `MAIN.OUT`.

use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use serde_json::{json, Value as Json};
use trust_runtime_core::cycle::CycleHooks;
use trust_runtime_core::deployment::ControlMode;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::native::memory::{read_place, symbol_place};
use trust_runtime_core::native::reflect::Located;
use trust_runtime_core::native::KERNEL_FUNCTIONS;
use trust_runtime_core::value::{Duration, Value};

use super::super::tests::{cycle_with, image_of};
use crate::debug::expression::Names;
use crate::runner::cycle::ResourceCycle;
use crate::runner::debugger::{Boundary, StatementStart, StopPoints};
use crate::runner::hold::{CompiledHeld, HoldHandle, HoldPass};
use crate::runner::Runner;
use crate::tests::runner_platform::TestPlatform;
use crate::tests::runner_ports::{ms, TestClock, TestLink, TestWait};

/// A hold at the statement on line 2 that writes each of `writes` to `MAIN.COUNT` through the held
/// scan, and records each write's result with the value the reads see after it.
struct WritingHold {
    writes: Vec<Value>,
    seen: RefCell<Vec<(Result<(), RuntimeError>, Value)>>,
}

impl HoldPass for WritingHold {
    fn scan_begins(&self) -> bool {
        true
    }

    fn at_statement(
        &self,
        _at: &StatementStart<'_>,
        _names: Option<&dyn Names>,
    ) -> Result<Boundary, RuntimeError> {
        Ok(Boundary::Hold)
    }

    fn hold_compiled(
        &self,
        _at: &StatementStart<'_>,
        view: &CompiledHeld<'_>,
    ) -> Result<(), RuntimeError> {
        let image = view.memory.image();
        let (index, _) = image
            .find_symbol("MAIN.COUNT")
            .expect("MAIN.COUNT has a symbol");
        let (place, type_index) = symbol_place(&image, index)?;
        for value in &self.writes {
            let written = view
                .writes
                .write_held(&Located { place, type_index }, value);
            let read = read_place(view.memory, place, type_index)?;
            self.seen.borrow_mut().push((written, read));
        }
        Ok(())
    }

    fn stop_points(&self) -> StopPoints {
        StopPoints {
            every: false,
            starts: vec![(0, (2, 1))],
        }
    }

    fn cells_free(&self) -> bool {
        true
    }
}

/// An observer that does nothing at the scan's boundaries.
struct Quiet;

impl CycleHooks for Quiet {}

#[test]
fn a_write_at_the_hold_reaches_the_held_statement_and_a_refused_one_changes_nothing() {
    // Specification 44 §13.2, worked by hand: at 10 ms statement 0 counts 5 to 6 and the scan
    // holds at statement 1. A BOOL is no USINT and is refused, and the reads at the hold still see
    // 6; 41 is written into the held scan at once and the reads see 41; when the hold ends,
    // statement 1 copies 41's lowest bit, 1, to `MAIN.OUT`. Catches a write that waits for the
    // hold's end (the reads at the hold would see 6), a write into a copy that the scan never sees
    // (`MAIN.OUT` would be 6 & 1 = 0), and a refused write that changed the count.
    let bytes = image_of(48, true);
    let hold = Rc::new(WritingHold {
        writes: vec![Value::Bool(true), Value::USInt(41)],
        seen: RefCell::new(Vec::new()),
    });
    let (mut cycle, _, _) = cycle_with(&bytes, Vec::new(), &KERNEL_FUNCTIONS);
    let pass: Rc<dyn HoldPass> = Rc::clone(&hold) as Rc<dyn HoldPass>;
    cycle.attach_hold(HoldHandle(pass));
    for time in [0, 10] {
        cycle
            .run_scan(Duration::from_millis(time), None, &mut Quiet)
            .expect("the scan runs");
    }
    assert_eq!(
        *hold.seen.borrow(),
        [
            (Err(RuntimeError::TypeMismatch), Value::USInt(6)),
            (Ok(()), Value::USInt(41)),
        ]
    );
    assert_eq!(cycle.program().value("MAIN.COUNT"), Some(Value::USInt(41)));
    assert_eq!(cycle.program().value("MAIN.OUT"), Some(Value::USInt(1)));
}

/// The replies the link has sent, by their request's number.
fn reply(replies: &[Json], id: u64) -> Json {
    replies
        .iter()
        .find(|reply| reply["id"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("a reply to request {id}: {replies:?}"))
}

#[test]
fn a_device_loop_takes_a_write_while_it_holds_the_scan() {
    // Specification 34 (a board works exactly like trust-runtime) with specification 44 §13.2: a
    // breakpoint on line 2 holds the scan at 10 ms with the count at 6; `debug.set` of `MAIN.COUNT`
    // := 41, served by the loop during the hold at 15 ms, answers 41 written, and `resume` at 16 ms
    // releases the hold, so statement 1 copies 41's lowest bit, 1, to `MAIN.OUT`, which a read
    // between scans at 17 ms reports, before the scan at 20 ms. Catches a loop that queues the
    // write for the hold's end or refuses it (`MAIN.OUT` would be 6 & 1 = 0), and a reply sent
    // before the write was made (it would report 6).
    let bytes = image_of(48, true);
    let clock = TestClock::default();
    let link = TestLink::new(4_096, clock.clone());
    let wait = TestWait::new(clock.clone());
    let (cycle, _, _) = cycle_with(&bytes, Vec::new(), &KERNEL_FUNCTIONS);
    let platform = TestPlatform {
        debug_enabled: true,
        control_mode: ControlMode::Debug,
        ..TestPlatform::default()
    };
    let mut runner = Runner::new(
        link.clone(),
        wait.clone(),
        cycle,
        platform,
        clock.clone(),
        64,
    );
    runner.run_pass().expect("the test wait does not fail");
    link.send(&json!({
        "id": 1, "type": "breakpoints.set", "params": {"source": "main.st", "lines": [1]}
    }));
    runner.run_pass().expect("the test wait does not fail");
    let mut replies = link.take_replies();
    wait.advance_time();
    for (at, request) in [
        (
            15,
            json!({"id": 2, "type": "debug.set", "params": {"path": "MAIN.COUNT", "value": "41"}}),
        ),
        (16, json!({"id": 3, "type": "resume"})),
        (
            17,
            json!({"id": 4, "type": "debug.set", "params": {"path": "MAIN.OUT"}}),
        ),
    ] {
        let link = link.clone();
        wait.at(ms(at), move || link.send(&request));
    }
    // The breakpoint stays armed, so no pass may reach the scan at 20 ms, which would hold again.
    for _ in 0..8 {
        runner.run_pass().expect("the test wait does not fail");
        replies.extend(link.take_replies());
        if replies.iter().any(|reply| reply["id"] == 4) {
            break;
        }
    }
    assert_eq!(reply(&replies, 1)["ok"], json!(true), "{replies:?}");
    assert_eq!(
        reply(&replies, 2)["result"],
        json!({"path": "MAIN.COUNT", "type": "USINT", "value": "41", "written": true}),
        "{replies:?}"
    );
    assert_eq!(reply(&replies, 3)["ok"], json!(true), "{replies:?}");
    assert_eq!(
        reply(&replies, 4)["result"],
        json!({"path": "MAIN.OUT", "type": "USINT", "value": "1", "written": false}),
        "{replies:?}"
    );
}
