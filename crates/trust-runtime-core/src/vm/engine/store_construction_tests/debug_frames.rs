//! The call stack a debugger reads (specification 34 §6.3, specification 11 §6.13.1; C1-17, C1-19e).
//!
//! At every instruction the engine gives the statement observer the active frame, and through it the
//! frames suspended below it. A debugger acts only where a statement starts, as the module's
//! `LINE_TABLE` names the starts (specification 12 §6.13). On the flat store a frame's typed locals lie
//! on the activation stack and the frame's own entries for them are `Null`, so an observer reads each
//! local with [`StatementFrame::local`]; on the value store the frame held them. The test checks what an
//! observer reads at every statement start of the corpus programs against what it read on the `Value`
//! store, which is recorded (`expected.rs`).

use super::expected::Facts;
use super::{build, prepare, EngineState, CORPUS};
use crate::bytecode::BytecodeModule;
use crate::debug::StatementIndex;
use crate::error::RuntimeError;
use crate::memory::VariableStorage;
use crate::value::{DateTimeValue, Duration};
use crate::vm::engine::cycle_port::NoopCycleHooks;
use crate::vm::{ExecutionServices, StatementFrame};
use core::cell::RefCell;
use core::fmt::Write;

/// The scans each program runs, 10 ms apart.
const SCANS: i64 = 3;

/// Records, at every statement start, the call stack an observer sees: for each frame its activation,
/// its POU and every local as [`StatementFrame::local`] reads it.
struct FrameRecorder {
    statements: StatementIndex,
    lines: RefCell<Vec<String>>,
}

impl ExecutionServices for FrameRecorder {
    fn deadline_exceeded(&self) -> bool {
        false
    }

    fn has_wall_clock(&self) -> bool {
        false
    }

    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }

    fn on_statement_with_frame_checked(
        &self,
        pou_id: u32,
        code_offset: usize,
        depth: u32,
        _storage: &VariableStorage,
        _logical_time: Duration,
        frame: StatementFrame<'_>,
    ) -> Result<(), RuntimeError> {
        let start = u32::try_from(code_offset)
            .ok()
            .and_then(|offset| self.statements.statement_at_start(pou_id, offset));
        let Some(statement) = start else {
            return Ok(());
        };
        let mut line = format!("statement {statement} of POU {pou_id}, depth {depth}:");
        for invocation in frame.call_stack() {
            let _ = write!(
                line,
                " [{:?} {:?}",
                invocation.activation(),
                invocation.pou_id()
            );
            for slot in 0..invocation.locals().len() {
                let _ = write!(line, " {:?}", invocation.local(slot));
            }
            line.push(']');
        }
        self.lines.borrow_mut().push(line);
        Ok(())
    }
}

/// What an observer reads at every statement start of `SCANS` scans of the program `bytes`, and how
/// each scan ended; `None` for a program without a `LINE_TABLE`.
fn observed(bytes: &[u8]) -> Option<Vec<String>> {
    let module = BytecodeModule::decode(bytes).expect("the corpus program decodes");
    let table = module.line_table()?;
    let recorder = FrameRecorder {
        statements: StatementIndex::new(table),
        lines: RefCell::new(Vec::new()),
    };
    let prepared = prepare(bytes);
    let mut data = build(&prepared).expect("the program constructs");
    {
        let mut state = EngineState::borrow(&mut data, &prepared, &recorder);
        for scan in 1..=SCANS {
            let result = state
                .execute_cycle_with_hooks(Duration::from_millis(10 * scan), &mut NoopCycleHooks);
            recorder
                .lines
                .borrow_mut()
                .push(format!("scan {scan}: {result:?}"));
        }
    }
    Some(recorder.lines.take())
}

/// What an observer reads in every corpus program that carries a `LINE_TABLE`.
fn every_call_stack(facts: &mut Facts) {
    let mut deepest = 0;
    for (name, bytes) in CORPUS {
        let Some(lines) = observed(bytes) else {
            assert!(name.starts_with("saved/"), "{name} carries a LINE_TABLE");
            continue;
        };
        if name == "calls" {
            deepest = lines
                .iter()
                .map(|line| line.matches(" [").count())
                .max()
                .unwrap_or(0);
        }
        facts.check(&format!("call stacks/{name}"), &lines);
    }
    assert_eq!(deepest, 3, "calls: the program, Twice and Scale");
}

#[test]
fn an_observer_reads_the_call_stack_the_value_store_showed() {
    // Specification 34 §6.3 and specification 11 §6.13.1 (C1-17): the debugger shows the values a
    // program holds, whichever store holds them, and a program cannot tell the stores apart (§6.2).
    // Each corpus program runs three scans with an observer that records, at every statement start
    // (where a debugger stops, evaluates a condition or writes a logpoint), each frame of the call
    // stack: its activation, its POU and all its locals read with `StatementFrame::local`. The value
    // store's record is the expected one. A flat store whose observer read a typed local from the
    // frame's own entry would record `Null` where the value store recorded the value; one that read
    // another activation's bytes would record the caller's or the callee's values in the wrong frame;
    // one that lost the reader in a suspended frame would record `Null` for the callers' locals. The
    // record reaches below the active frame: in `calls` the program calls `Twice`, which calls `Scale`,
    // so three frames are live at `Scale`'s statement.
    // The saved STBC 2.0 programs predate the LINE_TABLE; every bounded-C program carries one.
    let mut facts = Facts::compared();
    every_call_stack(&mut facts);
    facts.finish("call stacks/");
}

/// Record the facts of every test of this file.
pub(super) fn record(facts: &mut Facts) {
    every_call_stack(facts);
}
