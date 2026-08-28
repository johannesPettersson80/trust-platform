# MQTT Tag Mapping Review Follow-up Checklist

Scope: resolve the four valid automated review findings from PR #114 without
regressing raw MQTT mode or the traffic-light mapping workflow.

Branch: `fix/mqtt-review-followups`

Base: `5bca03b85cf5228c1027f52bab1cb5d39739f498`

## Product contract

- [x] Specify that an MQTT driver with no input direction leaves the shared
  input process image unchanged.
- [x] Specify that a mapping configuration with no output direction publishes
  no raw output fallback.
- [x] Specify numeric enum wire values, declared enum reconstruction, alias
  identity preservation, and fail-closed invalid-member handling.
- [x] Specify that conformance case execution checks inspect only the current
  invocation's log.

## Test-first behavior slices

- [x] Red: write-only MQTT leaves unrelated shared input bytes unchanged.
- [x] Green: disabled MQTT input is a no-op and reports healthy status.
- [x] Red: read-only mappings do not publish the raw shared output image.
- [x] Green: mapped output disablement is explicit and raw mode remains
  backward compatible when `mappings` is absent.
- [x] Red: outbound enum mapping publishes the enum's numeric IEC value.
- [x] Red: inbound enum mapping reconstructs the declared enum value.
- [x] Red: an undeclared inbound enum value fails atomically.
- [x] Red: an alias-to-enum mapping preserves the declared enum identity.
- [x] Green: enum-aware mapping conversion passes all four cases.
- [x] Red: a second conformance-gate run with the same `OUT_DIR` cannot reuse a
  previous passing line when its current filter selects zero tests.
- [x] Green: each conformance case log is fresh for its invocation.

## Implementation

- [x] Keep MQTT transport/session ownership separate from tag resolution and
  PLC value conversion.
- [x] Represent raw input and raw output enablement symmetrically in MQTT
  configuration.
- [x] Preserve declared enum TypeId separately from its MQTT wire scalar type.
- [x] Capture canonical enum identity and members from the runtime type
  registry, then rebuild inbound `Value::Enum` before storage mutation.
- [x] Reject invalid enum values before any input binding is committed.
- [x] Audit subranges sharing the scalar-layout path and document or test their
  existing representation contract.
- [x] Truncate only the active per-case conformance log before execution.

## SOLID, KISS, and DRY acceptance

- [x] `MqttIoDriver` owns transport direction behavior but does not resolve PLC
  symbols.
- [x] MQTT tag lowering owns declared-type metadata but does not publish or
  subscribe.
- [x] Enum conversion is implemented once at the process-image binding
  boundary and reused by read and write paths.
- [x] No protocol-specific enum conversion is added to the scheduler.
- [x] The conformance gate uses the existing progress runner and does not add a
  second log parser.
- [x] No touched source file crosses the repository's approximately 1,000-line
  split threshold because of this change.

## Focused and broad validation

- [x] Run every focused test red before its production change and record the
  exact assertion failure.
- [x] Run the same focused tests green after the minimal implementation.
- [x] Run the complete MQTT unit and runtime mapping suites on `trust-builder`.
- [x] Run runtime vertical tests: `api_smoke`, `debug_control`,
  `complete_program`, and `runtime_reliability`.
- [x] Run runtime networking checks, including the eight-iteration mesh/TLS
  stability gate and warnings-denied all-target check.
- [ ] Run remote `just fmt`, `just clippy`, and `just test-all` on the frozen
  candidate.
- [x] Run CI job-shape parity for the pull-request jobs, including the
  Windows-sensitive checks, before freezing the first candidate. Release-build
  checks remain part of the exact-SHA release-candidate and tag validation.

## Release hygiene

- [x] Update `CHANGELOG.md` under `Unreleased`.
- [x] Bump workspace and VS Code package versions together.
- [ ] Prepare the exact-SHA release-candidate artifact before push.
- [ ] Push one frozen candidate and wait for every GitHub check.
- [ ] Merge only through the guarded merge command.
- [ ] Create the annotated release tag from the exact green `main` SHA.
- [ ] Verify main CI, Release, GitHub Latest, checksums, and all Marketplace
  targets before completion.
