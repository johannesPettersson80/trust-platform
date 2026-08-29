# MQTT Tag Mapping Review Follow-up Checklist

Scope: resolve all valid automated review findings on MQTT PR #115 and release
PR #116 without regressing raw MQTT mode, the traffic-light workflow, capture
lifecycle cleanup, or post-merge candidate cleanup.

Branch: `fix/mqtt-review-final`

Base: `f41e71183598491a1c0491321e1b3df157850056`

## Product contract

- [x] Make `docs/specs/32-mqtt-io.md` the authoritative MQTT communication
  specification and retain only runtime integration requirements in
  `docs/specs/11-runtime-engine.md`.
- [x] Specify that an MQTT driver with no input direction leaves the shared
  input process image unchanged.
- [x] Specify that a mapping configuration with no output direction publishes
  no raw output fallback.
- [x] Specify numeric enum wire values, declared enum reconstruction, alias
  identity preservation, and fail-closed invalid-member handling.
- [x] Specify that conformance case execution checks inspect only the current
  invocation's log.
- [x] Specify and test the complete MQTT mapping-presence/direction matrix:
  absent, explicitly empty, explicitly empty plus explicit points, read-only,
  write-only, and mixed.
- [x] Specify and test enum snapshot type metadata for value, error, and
  unresolved states.
- [x] Make the PR verification gate reject behavior-changing production diffs
  that have no written-specification or native-test companion.
- [x] Make the PR verification gate reject an MQTT production change paired
  only with an unrelated or generic runtime specification.
- [x] Specify that capture lifecycle assertion deadlines exceed the owned
  session's graceful-termination window.
- [x] Specify that stale prunable candidate worktree registrations are exact
  cleanup targets rather than dirty-worktree blockers.
- [x] Specify that optional broker-version reporting cannot override passing
  real-Mosquitto protocol assertions when a help command prints its version
  and returns a nonzero help status.

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
- [x] Red/green: `mappings = []` disables both raw MQTT directions.
- [x] Red/green: enum-backed I/O snapshot JSON retains the declared enum name
  for value, error, and unresolved states.
- [x] Red/green: the strict changed-file verification path rejects production
  changes missing either direct contract companion.
- [x] Red/green: `ctl status` renders the runtime's JSON-null fault as `none`.
- [x] Behavior-lock: the runtime communications conformance gate runs the
  traffic-light mapping against real Mosquitto and observes every light phase.
- [x] Red/green: a TERM-resistant owned child exercises the KILL fallback
  without exhausting the lifecycle assertion deadline.
- [x] Red/green: a missing candidate worktree directory with a prunable Git
  registration is reported as `prunable_worktree` cleanup work.
- [x] Red/green: Mosquitto 2.0.18-style help output with exit status 3 remains
  usable version evidence after all traffic-light assertions pass.

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
- [x] Neutralize only the informational Mosquitto help status while preserving
  the emitted version line and all authoritative protocol failures.

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
- [x] Changed-file conformance enforcement stays in a dedicated validator
  boundary and does not make planner or catalog metadata authoritative.
- [x] Real Mosquitto process orchestration stays in a dedicated executable
  conformance script instead of entering the MQTT worker or scheduler.
- [x] Release cleanup classification remains in the dedicated cleanup module;
  it does not mutate or prune worktrees during an audit.

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
