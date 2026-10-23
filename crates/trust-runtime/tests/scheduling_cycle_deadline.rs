//! Physical execution deadline between two tasks of one coincident batch (spec 34 CYCLE-02 and
//! CYCLE-03; spec 11 section 6.2 resource-runner cycle transaction and section 6.6 watchdog
//! handling). A failed execution shall not publish partially computed normal outputs. Here the
//! first task of the batch ends with its write staged, the deadline then elapses before the second
//! task completes, and the batch returns the timeout: no driver publication runs and the committed
//! output image keeps the previous value. The deadline is a test-controlled platform service, so
//! the expiry is deterministic and no wall clock is read.

use std::cell::Cell;

use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::{DateTimeValue, Duration};
use trust_runtime_core::vm::{
    CycleAccess, CycleHooks, CyclePhase, ExecutionServices, PreparationLimits, PreparedModule,
};

/// Two tasks on one resource, as in `scheduling_cycle_publication.rs`. Slow (declaration index 1,
/// priority 0) runs first and writes 2; Fast (index 0, priority 1) runs second and writes 1.
const COINCIDENT: &str = r#"
VAR_GLOBAL outgoing AT %QB0 : BYTE; END_VAR
PROGRAM WriteFast outgoing := BYTE#1; END_PROGRAM
PROGRAM WriteSlow outgoing := BYTE#2; END_PROGRAM
CONFIGURATION Conf RESOURCE Controller ON PLC
TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
TASK Slow (INTERVAL := T#25ms, PRIORITY := 0);
PROGRAM P1 WITH Fast : WriteFast;
PROGRAM P2 WITH Slow : WriteSlow;
END_RESOURCE
END_CONFIGURATION
"#;

/// Platform services whose physical deadline is the shared flag the test controls.
struct ControlledDeadline<'a> {
    expired: &'a Cell<bool>,
}

impl ExecutionServices for ControlledDeadline<'_> {
    fn deadline_exceeded(&self) -> bool {
        self.expired.get()
    }

    fn has_wall_clock(&self) -> bool {
        false
    }

    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

/// Records the cycle phases and driver publications, and lets the deadline elapse once Slow ends.
struct ExpireAfterSlow<'a> {
    expired: &'a Cell<bool>,
    phases: Vec<CyclePhase>,
    publications: usize,
}

impl CycleHooks for ExpireAfterSlow<'_> {
    fn on_phase(
        &mut self,
        phase: CyclePhase,
        _access: &mut CycleAccess<'_, '_>,
    ) -> Result<(), RuntimeError> {
        self.phases.push(phase);
        if phase == CyclePhase::TaskEnd(1) {
            self.expired.set(true);
        }
        Ok(())
    }

    fn publish_outputs(&mut self, _access: &CycleAccess<'_, '_>) -> Result<(), RuntimeError> {
        self.publications += 1;
        Ok(())
    }
}

#[test]
fn deadline_expiry_between_coincident_tasks_publishes_no_partial_outputs() {
    let bytes = CompileSession::from_source(COINCIDENT)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted ST source")
        .encode()
        .expect("encode STBC");
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let expired = Cell::new(false);
    let services = ControlledDeadline { expired: &expired };
    let mut state = prepared
        .instantiate_with_services(0, &services)
        .expect("instantiate resource 0");
    let mut hooks = ExpireAfterSlow {
        expired: &expired,
        phases: Vec::new(),
        publications: 0,
    };

    // Both tasks are due at 50 ms. Slow runs and ends with 2 staged. The deadline then elapses,
    // so Fast's completion fails and the batch returns the timeout before output sampling.
    let error = state
        .execute_cycle_with_hooks(Duration::from_millis(50), &mut hooks)
        .expect_err("the batch must fail when the deadline elapses between its tasks");
    assert_eq!(error, RuntimeError::ExecutionTimeout);

    assert!(hooks.phases.contains(&CyclePhase::TaskEnd(1)));
    assert!(!hooks.phases.contains(&CyclePhase::AfterOutputSample));
    assert_eq!(
        hooks.publications, 0,
        "a failed batch never reaches driver publication"
    );
    assert_eq!(
        state.outputs(),
        [0u8].as_slice(),
        "the committed output image keeps its previous value"
    );

    // The fault is latched, so a later cycle is refused until an explicit recovery (spec 11 6.2).
    assert_eq!(
        state.execute_cycle_with_hooks(Duration::from_millis(60), &mut hooks),
        Err(RuntimeError::ResourceFaulted)
    );
}
