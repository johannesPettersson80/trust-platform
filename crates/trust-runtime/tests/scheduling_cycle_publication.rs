//! Coincident task batches and output publication (spec 34 CYCLE-02 and CYCLE-03, SCHED-03; spec 11
//! section 6.2 "Resource-runner cycle transaction"). A cycle with two due tasks is observed through
//! the borrowed cycle hooks at logical instants. The expected phase order and output images are
//! written from the specification, not from the scheduler.

use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::Duration;
use trust_runtime_core::vm::{
    CycleAccess, CycleHooks, CyclePhase, PreparationLimits, PreparedModule,
};

/// Two tasks on one resource. Fast (declaration index 0) has priority 1 and Slow (index 1) has
/// priority 0, so Slow runs first although it is declared second. Both write the same output byte.
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

#[derive(Default)]
struct PhaseRecorder {
    phases: Vec<CyclePhase>,
    output_seen: Vec<(CyclePhase, u8)>,
}

impl CycleHooks for PhaseRecorder {
    fn on_phase(
        &mut self,
        phase: CyclePhase,
        access: &mut CycleAccess<'_, '_>,
    ) -> Result<(), RuntimeError> {
        self.phases.push(phase);
        if matches!(
            phase,
            CyclePhase::TaskEnd(_) | CyclePhase::AfterOutputSample
        ) {
            self.output_seen.push((phase, access.outputs()[0]));
        }
        Ok(())
    }
}

#[test]
fn coincident_tasks_run_in_priority_order_and_publish_one_output_image_after_the_batch() {
    let bytes = CompileSession::from_source(COINCIDENT)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted ST source")
        .encode()
        .expect("encode STBC");
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let mut recorder = PhaseRecorder::default();

    // First cycle at 50 ms. Fast (10 ms) has missed 4 deadlines and Slow (25 ms) has missed 1, so
    // both are due in this batch. Both overrun reports come before any task starts, in declaration
    // order, and the batch then runs by priority: Slow (priority 0), then Fast (priority 1).
    state
        .execute_cycle_with_hooks(Duration::from_millis(50), &mut recorder)
        .unwrap();

    let through_output = recorder
        .phases
        .iter()
        .position(|phase| *phase == CyclePhase::AfterOutputSample)
        .expect("the cycle reaches output sampling")
        + 1;
    assert_eq!(
        &recorder.phases[..through_output],
        &[
            CyclePhase::CycleStart,
            CyclePhase::BeforeInputSample,
            CyclePhase::AfterInputSample,
            CyclePhase::TaskOverrun {
                index: 0,
                missed: 4,
            },
            CyclePhase::TaskOverrun {
                index: 1,
                missed: 1,
            },
            CyclePhase::TaskStart(1),
            CyclePhase::TaskEnd(1),
            CyclePhase::TaskStart(0),
            CyclePhase::TaskEnd(0),
            CyclePhase::BeforeOutputSample,
            CyclePhase::AfterOutputSample,
        ],
        "phase order of one coincident batch"
    );

    // Slow ends having written 2 and Fast ends having written 1, but neither end publishes an image:
    // the output stays at the previous image (0) until the batch completes. The published image is
    // the batch's final value, from Fast, which ran last.
    assert_eq!(
        recorder.output_seen,
        vec![
            (CyclePhase::TaskEnd(1), 0),
            (CyclePhase::TaskEnd(0), 0),
            (CyclePhase::AfterOutputSample, 1),
        ]
    );
}
