# Conformance Known Gaps

This list is explicit by contract and updated with suite revisions.

## Current Gaps

- No vendor-specific dialect conformance in this phase.
  - Siemens SCL and Mitsubishi GX Works3 compatibility are outside Deliverable 1.

- Arithmetic scope is limited to selected corner cases.
  - Additional numeric conversion/overflow matrices are planned.

- Compile-error matching currently compares full emitted error text.
  - Diagnostic-code-only matching is planned to reduce formatting sensitivity.

- External adapter harness is documented but not bundled.
  - Third-party runtimes currently implement their own adapter runner.

- Suite does not yet model wall-clock jitter/fault injection.
  - Deterministic cycle replay is covered; stochastic timing fault profiles are deferred.

- The retain matrix exposes hot/fault/download as conformance labels mapped to
  the runtime's current cold/warm restart primitives.
  - A future runtime with distinct restart primitives must add dedicated cases
    before changing those mappings.

- Communication determinism is simulated/loopback only.
  - Live broker, PLC, or fieldbus hardware acceptance belongs to the
    device-in-the-loop gates, not this deterministic suite.

## Non-goals In Deliverable 1

- Functional safety certification claims.
- Hardware-in-the-loop certification claims.
- Vendor project import/export parity claims.
