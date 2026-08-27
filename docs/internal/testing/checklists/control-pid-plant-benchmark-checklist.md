# Control.PID plant benchmark checklist

This checklist extends the executed `Control.PID` evaluation from the paper's
nominal process to representative plant classes already provided by the truST
simulation system. Specification 36 is the product authority.

Passing this checklist demonstrates bounded performance only for the declared
plants, tunings, sample intervals, and acceptance limits. A failed case shall
be classified as an implementation defect, unsuitable tuning, unsupported
plant class, or simulation limitation before production code is changed.

## 1. Contract

- [x] PIDBENCH-001 Specify the integrating-process plant, tuning, duration, and
  quantitative stability and performance gates in Specification 36.
- [x] PIDBENCH-002 Specify the underdamped second-order plant, tuning, duration,
  and quantitative stability and performance gates in Specification 36.
- [x] PIDBENCH-003 Specify the delay-dominant plant, tuning, duration, and
  quantitative stability and performance gates in Specification 36.
- [x] PIDBENCH-004 Specify the nonlinear-actuator plant, tuning, duration, and
  quantitative stability, tracking, and actuator gates in Specification 36.
- [x] PIDBENCH-005 Require every time-domain benchmark to execute the public
  Structured Text `Control.PID` through the runtime and normal `%Q/%I` process
  image, without a duplicate controller equation.

## 2. Native executable benchmarks

- [x] PIDBENCH-006 Add a native integrating-process closed-loop benchmark and
  report overshoot, settling time, final error, IAE, and extrema.
- [x] PIDBENCH-007 Add a native underdamped second-order closed-loop benchmark
  and report overshoot, settling time, final error, IAE, and extrema.
- [x] PIDBENCH-008 Add a native delay-dominant closed-loop benchmark and report
  overshoot, settling time, final error, IAE, and extrema.
- [x] PIDBENCH-009 Add a native nonlinear-actuator closed-loop benchmark that
  exercises saturation, slew limiting, lag, and deadband while proving the
  controller output and process response remain bounded.
- [x] PIDBENCH-010 Keep the paper nominal, robustness, sampling, disturbance,
  anti-windup, rate-limit, noise, and margin benchmarks green.

## 3. Verification and interpretation

- [x] PIDBENCH-011 Run the focused native benchmark on `trust-builder` and
  record the exact command, individual measurements, and result.
- [x] PIDBENCH-012 Run the released native `Control.PID` conformance fixture,
  existing PID/FOPDT integration test, and simulation workflow suite.
- [x] PIDBENCH-013 Run formatting and focused lint validation; record unrelated
  repository blockers without changing unrelated production code.
- [x] PIDBENCH-014 Record an evidence-backed conclusion that distinguishes
  controller implementation quality from tuning quality and simulation proof
  from physical-plant proof.

## Evidence

- Contract: Specification 36, Section 11.
- Native tests: `crates/trust-runtime/tests/control_pid_performance.rs`.
- Focused command: `cargo test -p trust-runtime --test
  control_pid_performance pid_benchmark_ -- --nocapture`; `4 passed`.
- Complete benchmark command: `cargo test -p trust-runtime --test
  control_pid_performance -- --nocapture`; `9 passed`.
- Regression commands: native Control.PID conformance `1 passed`; existing
  PID/FOPDT integration `1 passed`; simulation workflow `10 passed`.
- Formatting: `cargo fmt --all -- --check` passed.
- Focused Clippy reached `trust-runtime` but remains blocked by the recorded
  unrelated Rust 1.98 `chunks_exact_to_as_chunks` findings in bytecode and
  Modbus production files. No warning is emitted by the new benchmark.
- Measurements and interpretation:
  `docs/internal/testing/evidence/vscode-control-studio/control-pid-performance-evaluation-2026-08-26.md`.
