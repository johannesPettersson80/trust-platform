# Phase 6 EtherCAT DC/CoE Non-Start Evidence

Date: 2026-07-05

Scope: `RTCONN-P6-005` and `RTCONN-P6-006`.

## Decision

EtherCAT distributed clocks and CoE startup were not started in Phase 6.

Reason: the currently shipped EtherCAT backend remains the v1 deterministic
process-image module-chain profile. Advanced motion profile support is still an
explicit v1 non-goal, and no accepted motion roadmap item in this checklist
requires EtherCAT DC/CoE implementation now.

## Evidence

- `docs/guides/ETHERCAT_BACKEND_V1.md` lists included v1 behavior as
  deterministic module-chain mapping, diagnostics, cycle-time health telemetry,
  EtherCrab hardware transport, and deterministic mock mode.
- The same guide lists advanced motion profile support as out of scope.
- `docs/specs/11-runtime-engine.md` repeats the EtherCAT v1 non-goal: no
  advanced motion profile support.
- No Phase 6 code changes were made under `crates/trust-runtime/src/io/ethercat*`.

## Follow-Up Rule

If a future motion roadmap explicitly accepts EtherCAT distributed clocks or CoE
startup, create a dedicated DC/CoE checklist before implementation. That
checklist must include hardware topology, safety boundaries, timing acceptance
criteria, and device-in-the-loop gates.
