# Control.PID advanced benchmark checklist

## Required purpose checkpoint

Release-level gates and the final user walkthrough are tracked in the
[Control platform release-readiness checklist](control-platform-release-readiness.md).

Read the [Control Platform Purpose and Acceptance Contract](../../../specs/control-platform-purpose.md)
for every new or resumed task on this checklist. Record a fresh Start/Finish
entry using its Section 6; existing checkmarks retain only their stated evidence.

- [ ] **Start (current/next task):** identify the platform part, owning spec,
  existing worktree and implementation, and actual acceptance target.
- [ ] **Finish (current/next task):** name the controller/project/plant and what
  actually ran or was reviewed; link evidence and state gaps without substituting
  a simpler fixture for the intended goal.


This checklist extends Specification 36 with difficult but deterministic plant
conditions using the existing truST runtime, process image, and simulator. It
does not add another simulation or controller implementation.

A failed benchmark shall be classified before production code changes. Bad
tuning, an invalid oracle, and an unsupported plant are not PID implementation
defects.

## 1. Specification

- [x] PIDADV-001 Specify an open-loop unstable scalar state-space plant, fixed
  PI tuning, duration, and quantitative stability and tracking gates.
- [x] PIDADV-002 Specify a scheduled process-gain change, fixed PI tuning, and
  recovery gates without gain scheduling or retuning during execution.
- [x] PIDADV-003 Specify deterministic severe scan jitter, including the exact
  interval sequence and the requirement to pass every actual interval into
  `PID.Update`.
- [x] PIDADV-004 Specify a coupled 2×2 state-space process controlled by two
  independent public `Control.PID` instances through two `%Q/%I` channel pairs.
- [x] PIDADV-005 Define boundedness, final error, overshoot or excursion,
  settling or recovery, IAE, and controller-output extrema where applicable.
- [x] PIDADV-006 State explicitly that simulation evidence is neither universal
  robust-stability proof nor physical-plant evidence.

## 2. Native benchmarks

- [x] PIDADV-007 Add the unstable-plant executable closed-loop benchmark at the
  runtime/process-image boundary.
- [x] PIDADV-008 Add the scheduled-gain executable benchmark using a native
  plant event rather than directly modifying controller feedback.
- [x] PIDADV-009 Add the jitter executable benchmark with a deterministic
  repeated interval sequence and actual interval input to Structured Text.
- [x] PIDADV-010 Add the coupled-MIMO executable benchmark using the simulator's
  existing `state_space` model and two actual Structured Text PID instances.
- [x] PIDADV-011 Keep the original paper and representative plant-class
  benchmarks green without changing their acceptance limits.

## 3. Verification

- [x] PIDADV-012 Run the four focused advanced benchmarks on `trust-builder` and
  record individual measurements and the exact command.
- [x] PIDADV-013 Run the complete PID performance suite, native paper
  conformance, existing PID/FOPDT integration, and simulation workflow suite.
- [x] PIDADV-014 Run formatting and focused lint validation, recording any
  unrelated repository blocker without modifying unrelated code.
- [x] PIDADV-015 Update the existing PID performance evidence with results,
  failure classification, limitations, and the next justified test boundary.

## Evidence

- Contract: Specification 36, Section 12.
- SISO, gain-change, and jitter tests:
  `crates/trust-runtime/tests/control_pid_performance.rs`.
- Coupled-loop test:
  `crates/trust-runtime/tests/control_pid_mimo_performance.rs`.
- Complete performance commands: `control_pid_performance` `12 passed` and
  `control_pid_mimo_performance` `1 passed` on `trust-builder`.
- Regression commands: native Control.PID conformance `1 passed`; existing
  PID/FOPDT integration `1 passed`; simulation workflow `10 passed`.
- Formatting: `cargo fmt --all -- --check` passed.
- Focused Clippy was executed for both benchmark targets. It remains blocked by
  the unrelated Rust 1.98 `chunks_exact_to_as_chunks` findings in bytecode and
  Modbus production files; neither new benchmark emits a warning.
- Measurements, invalid-harness failure classification, tuning decisions, and
  limitations are recorded in
  `docs/internal/testing/evidence/vscode-control-studio/control-pid-performance-evaluation-2026-08-26.md`.
