# IEC Deviations Log

Authoritative location:
- This tracked file is the repository source of truth for IEC deviations/extensions.
- Do not point tracked docs or code comments at legacy internal IEC deviation-log paths.

This file tracks known, intentional deviations/extensions from strict IEC 61131-3 behavior.

Classification note: an entry in this log is not automatically a claim of IEC
non-conformance. Per the repository IEC-first rules, this file also records
behavior that IEC explicitly leaves implementer-specific and truST product
extensions outside IEC's scope. Each entry must state which category applies
in its IEC reference.

## 2026-07-22 - Enabled watchdog policy normalization

- ID: DEV-062
- Area: Portable watchdog policy installation
- IEC reference: IEC 61131-3 Ed.3 section 6.8 defines resource and task
  execution but leaves watchdog thresholds and host-control actions
  implementer-specific. This entry records truST's portable policy boundary;
  it does not claim that installing a value starts a watchdog deadline.
- Deviation/extension:
  - An enabled non-positive timeout normalizes to one millisecond.
  - An enabled positive timeout is preserved.
  - A disabled timeout is inert and remains unchanged.
  - The configured watchdog action is always preserved.
  - Installing policy stores the normalized value without executing a timeout
    action or mutating resource state.
- Impact:
  - The scheduler cannot receive an enabled always-expired deadline from this
    boundary, while disabled and already-valid policies retain their values.
- Mitigation:
  - Configuration validation should still reject operator mistakes; the
    one-millisecond floor is a fail-safe runtime boundary, not silent approval
    of an invalid production configuration.

## 2026-07-22 - Portable fault recording and policy replacement

- ID: DEV-061
- Area: Portable runtime fault-holder mutation
- IEC reference: IEC 61131-3 Ed.3 section 6.8 defines resource and task
  execution but does not define truST's host-side fault holder, stored runtime
  error, or policy setter. This is a product extension outside the IEC language
  model.
- Deviation/extension:
  - Recording sets the fault flag and stores the supplied error exactly.
  - A later recording replaces the prior last error and preserves policy.
  - Replacing policy preserves the fault flag and stored error.
  - Neither mutation executes halt, safe-state, restart, clear, or recovery.
- Impact:
  - Status surfaces observe the latest fault without policy drift, and control
    paths can change future handling without erasing the current fault record.
- Mitigation:
  - Owning host-control code must separately execute policy actions and enforce
    authorization, safe-state, restart, and recovery contracts.

## 2026-07-22 - Deterministic portable fault-policy decisions

- ID: DEV-060
- Area: Portable runtime fault-policy projection
- IEC reference: IEC 61131-3 Ed.3 section 6.8 defines resource and task
  execution but does not define truST's host-side `FaultPolicy`,
  `FaultDecision`, safe-state instruction flag, or restart-control API. This
  entry records a truST product extension outside the IEC language model; it
  does not claim that returning a decision executes a resource transition.
- Deviation/extension:
  - `Halt` projects to action `Halt` without requesting safe-state application.
  - `SafeHalt` projects to action `SafeHalt` and requests safe-state
    application.
  - `Restart` projects to action `Restart` without requesting safe-state
    application.
  - Projection reads only the configured policy, does not depend on the current
    fault flag or stored error, and does not mutate the fault holder.
- Impact:
  - Callers receive one deterministic instruction for each configured policy
    without accidentally executing recovery at the portable value boundary.
- Mitigation:
  - The owning host-control path must separately apply safe outputs, halt or
    restart the resource, enforce restart bounds, and report any failed action.

## 2026-07-22 - Deterministic portable fault-state clearing

- ID: DEV-059
- Area: Portable runtime fault-state lifecycle
- IEC reference: IEC 61131-3 Ed.3 section 6.8 defines resource and task
  execution, but it does not define truST's host-side `FaultSubsystem`, stored
  runtime error, recovery authorization, or clear API. This entry records a
  truST product extension outside the IEC language model; it does not treat
  clearing bookkeeping as evidence that a resource may safely resume.
- Deviation/extension:
  - Clearing a recorded fault atomically sets the holder to not faulted and
    removes the stored last error.
  - Clearing does not alter the configured `FaultPolicy`.
  - Clearing an already healthy holder is idempotent.
  - The portable clear operation does not restart or resume a resource, apply
    safe outputs, reset runtime storage, or authorize recovery.
- Impact:
  - Status consumers cannot observe a healthy flag paired with a stale last
    fault after the clear transition, and recovery policy cannot silently drift.
- Mitigation:
  - Invoke clear only from an owning recovery/control path after separately
    handling safe state, storage transition, authorization, and fault repair.

## 2026-07-22 - Deterministic task readiness and overrun accounting

- ID: DEV-058
- Area: Portable runtime task readiness
- IEC reference: IEC 61131-3 Ed.3 section 6.8.2 a-b requires scheduling on a
  `SINGLE` rising edge and periodic scheduling for a non-zero `INTERVAL` while
  `SINGLE` is zero. The standard makes interval resolution and deadline
  information implementer-specific and does not define truST's due timestamp,
  skipped-interval counter, or logical-clock discontinuity policy. This entry
  records those implementation choices without changing the IEC trigger
  conditions.
- Deviation/extension:
  - Each sampled FALSE-to-TRUE `SINGLE` transition produces one event
    activation at the sampled logical time. Held TRUE does not repeat and
    suppresses periodic readiness; FALSE rearms the next edge.
  - A non-positive interval produces no periodic activation at the portable
    boundary. Valid task configuration uses zero to disable or a positive
    duration to enable periodic scheduling; negative portable metadata is
    conservatively disabled.
  - A backward logical-clock sample leaves the periodic baseline and overrun
    count unchanged. A later sample must reach the next deadline relative to
    that unchanged baseline before the task is due.
  - A forward jump coalesces all elapsed intervals into one activation. The
    due time is the first nominal deadline, the missed count is the number of
    additional complete intervals, cumulative overruns add with saturation,
    and the baseline advances to the sampled time.
  - Event activation does not advance the periodic baseline or overrun count;
    nominal-deadline arithmetic saturates rather than wrapping.
- Impact:
  - Host pauses, deterministic simulation, and explicit test-clock jumps do
    not replay an unbounded burst of stale task executions or corrupt task
    accounting.
- Mitigation:
  - Use a positive configured interval for periodic tasks, treat recorded
    overruns as missed deadlines requiring supervision, and use an injected
    clock for deterministic simulation or replay.

## 2026-07-22 - Deterministic retained snapshot insertion

- ID: DEV-057
- Area: Portable retained snapshot construction
- IEC reference: IEC 61131-3 Ed.3 section 6.5.6 and Figure 9 define which
  variable classes retain values across restart. They do not define the
  ordering, duplicate-key, or validation semantics of an implementation's
  internal retained snapshot container. This entry records a truST product
  policy at that internal boundary; it does not change source-level IEC
  identifier matching or retentive-variable qualification.
- Deviation/extension:
  - Callers pass an already-resolved retained snapshot name. The insertion
    boundary does not canonicalize or reinterpret that name.
  - The first insertion of a name appends one entry. Inserting the same
    resolved name again replaces only its value at the original position;
    it does not append a duplicate or reorder sibling entries.
  - Insertion preserves the complete runtime `Value`, including its concrete
    variant and nested contents. Declared-type, finiteness, migration, and
    persistence validation remain owned by the later retain save/load/apply
    boundaries; successful insertion alone is not acceptance evidence.
- Impact:
  - Snapshot serialization, comparison, and replay observe deterministic
    entry order and one current value per resolved retained name.
- Mitigation:
  - Resolve names before insertion and continue to use the owning retain
    persistence or reload transaction for validation and fail-closed apply.

## 2026-07-22 - Reject unsigned integer result and conversion overflow

- ID: DEV-056
- Area: Runtime unsigned integer arithmetic and numeric conversion
- IEC reference: IEC 61131-3 Ed.3 section 6.4.2.1, Table 10 defines the
  unsigned integer widths and ranges. Section 7.3.2 requires a numerical
  result outside its result-type range to be treated as an error. Sections
  6.6.2.5.2 and 6.6.2.5.3 make conversion execution errors and the target
  value for an out-of-range conversion implementer-specific. This entry
  chooses the truST fault and no-result policy; it does not add an implicit
  source-level signed/unsigned conversion.
- Deviation/extension:
  - Materializing a representable result as `USINT`, `UINT`, `UDINT`, or
    `ULINT` preserves the mathematical value exactly and assigns the
    destination's runtime value tag.
  - A result above the destination maximum returns `RuntimeError::Overflow`
    before storage. The runtime does not wrap, saturate, truncate, or
    substitute a value.
  - Passing a signed or floating destination category to the unsigned
    materialization boundary returns `RuntimeError::TypeMismatch` without a
    value.
- Impact:
  - Unsigned arithmetic and conversions fail visibly instead of silently
    changing a safety-relevant integer value.
- Mitigation:
  - Select a wider unsigned destination or range-check before an explicit
    narrowing conversion, and handle the runtime fault at the owning control
    boundary.

## 2026-07-22 - Fail closed while normalizing integer runtime operands

- ID: DEV-055
- Area: Runtime integer operand normalization
- IEC reference: IEC 61131-3 Ed.3 section 6.4.2.1, Table 10 defines the signed
  and unsigned integer value ranges. The standard does not define truST's
  internal host-width operand extraction helper. This entry records an
  implementation safety contract outside the IEC source-language interface;
  it does not add an implicit source-level signed/unsigned conversion.
- Deviation/extension:
  - `SINT`, `INT`, `DINT`, and `LINT` runtime values are represented as `i64`
    without changing their mathematical value.
  - `USINT`, `UINT`, `UDINT`, and representable `ULINT` values are represented
    exactly; a `ULINT` value above `i64::MAX` returns
    `RuntimeError::Overflow` instead of wrapping or reinterpreting its sign.
  - Every non-integer runtime tag returns `RuntimeError::TypeMismatch` without
    a substitute value.
- Impact:
  - Internal arithmetic and comparison operand extraction cannot silently
    change an integer's value or interpret an invalid runtime category.
- Mitigation:
  - Route unsigned operations through their owning unsigned boundary and
    reject oversized values before a signed host-width representation is used.

## 2026-07-21 - Reject signed integer result and conversion overflow

- ID: DEV-054
- Area: Runtime signed integer arithmetic and numeric conversion
- IEC reference: IEC 61131-3 Ed.3 section 6.4.2.1, Table 10 defines the signed
  integer widths and ranges. Section 7.3.2 requires a numerical result outside
  its result-type range to be treated as an error. Sections 6.6.2.5.2 and
  6.6.2.5.3 make conversion execution errors and the target value for an
  out-of-range conversion implementer-specific. This entry chooses the truST
  fault and no-result policy; it does not weaken the IEC arithmetic-error rule.
- Deviation/extension:
  - Materializing a representable result as `SINT`, `INT`, `DINT`, or `LINT`
    preserves the mathematical value exactly and assigns the destination's
    runtime value tag.
  - A result below the destination minimum or above its maximum returns
    `RuntimeError::Overflow` before storage. The runtime does not wrap,
    saturate, truncate, or substitute a value.
  - Passing a non-signed destination category to the signed materialization
    boundary returns `RuntimeError::TypeMismatch` without a value.
- Impact:
  - Signed arithmetic and conversions fail visibly instead of silently
    changing a safety-relevant integer value.
- Mitigation:
  - Select a wider signed destination or range-check before an explicit
    narrowing conversion, and handle the runtime fault at the owning control
    boundary.

## 2026-07-18 - Reject non-finite explicit conversion results

- ID: DEV-053
- Area: Standard numeric and bit-to-numeric conversion functions
- IEC reference: IEC 61131-3 Ed.3 section 6.6.2.5.2 makes conversion accuracy
  effects and execution errors implementer-specific; section 6.6.2.5.3 Table 23
  specifically makes `LREAL_TO_REAL` value-range errors implementer-specific.
  Section 6.6.2.5.5 Table 25 defines `DWORD_TO_REAL` and `LWORD_TO_LREAL` as
  binary transfers. This entry records the product's implementer-specific
  exceptional-value policy, not a contradiction of a required finite result.
- Deviation/extension:
  - Every explicit conversion targeting `REAL` or `LREAL` returns
    `RuntimeError::Overflow` when parsing, narrowing, or binary transfer would
    produce NaN, positive infinity, or negative infinity.
  - Finite `DWORD_TO_REAL` and `LWORD_TO_LREAL` values preserve the IEC binary
    transfer. No non-finite value is clamped or replaced.
  - The error occurs before assignment storage, leaving the target unchanged.
- Impact:
  - PLC logic cannot synthesize a stored non-finite value through standard
    explicit conversion functions.
- Mitigation:
  - Range-check before narrowing and represent exceptional state with an
  explicit finite value and status rather than IEEE exceptional payloads.

## 2026-07-14 - Connector status and discovery truth vocabulary

- ID: DEV-051
- Area: Runtime connector supervision and editor discovery presentation
- IEC reference: IEC 61131-3 does not define truST's protocol connector
  lifecycle, supervisory health, discovery probes, or editor fleet topology.
  This entry records a truST product extension outside the IEC language model,
  not an IEC non-conformance.
- Deviation/extension:
  - Connector lifecycle, health, discovery confidence, and point-quality values
    use the closed vocabularies in the runtime-engine specification.
  - Unknown values fail visibly. An invalid peer status report cannot be
    substituted with a healthy value or remove the peer's raw topology.
- Impact:
  - Operators can distinguish configured, reachable, protocol-confirmed,
    degraded, stale, and faulted connectors without treating reachability as
    protocol health.
- Mitigation:
  - Keep producers and consumers on the shared closed vocabulary and preserve
    topology with an explicit error when a version mismatch is encountered.

## 2026-07-14 - EtherCAT hardware unavailability is terminal per driver

- ID: DEV-052
- Area: EtherCAT connector initialization and recovery
- IEC reference: IEC 61131-3 does not define EtherCAT adapter discovery,
  EtherCrab process-data allocation, or truST's driver recovery lifecycle. This
  entry records a truST product extension outside the IEC language model, not
  an IEC non-conformance.
- Deviation/extension:
  - Missing hardware does not block project construction or runtime startup;
    the first wire operation reports the unavailable resource.
  - A post-allocation initialization failure is terminal for that driver
    instance and requires rebuilding it before another allocation attempt.
- Impact:
  - Missing hardware fails visibly without an unbounded scan-cycle retry or
    repeated process-data allocation.
- Mitigation:
  - Rebuild the driver after correcting adapter or topology configuration and
    use the mock adapter only as software evidence, never hardware proof.

## 2026-07-14 - Runtime control and debugger role authorization

- ID: DEV-050
- Area: Runtime control, debugger, I/O, HMI, configuration, and pairing access
- IEC reference: IEC 61131-3 does not define truST's host control protocol,
  pairing tokens, debugger transport, or product role hierarchy. This entry
  records a truST security extension outside the IEC language model, not an IEC
  non-conformance.
- Deviation/extension:
  - Control roles are ordered Viewer, Operator, Engineer, and Admin, with every
    supported request assigned an explicit minimum role.
  - Debug writes, forces, and releases require Engineer; enabling debug or
    changing control mode requires Admin.
  - Unclassified future request types use an Admin fail-safe rather than
    inheriting Viewer authority.
- Impact:
  - Lower-role clients cannot activate the debug surface or mutate PLC state;
    adding a new request without updating the matrix cannot silently expose it
    to a read-only client.
- Mitigation:
  - Commission tokens and pairing roles explicitly, keep production debug
    disabled, and review the role matrix whenever a control operation is added.

## 2026-07-14 - Exclude debugger pause dwell from watchdog execution time

- ID: DEV-049
- Area: Runtime debugger pause and watchdog accounting
- IEC reference: IEC 61131-3 does not define truST's debugger protocol,
  statement-boundary pause mechanism, or host watchdog accounting while an
  operator inspects a stopped program. This entry records a truST product
  extension outside the IEC language model, not a contradiction of an IEC
  requirement.
- Deviation/extension:
  - A watchdog measures active cycle execution. Time spent waiting at a
    debugger statement-boundary pause is excluded from the current cycle's
    execution and output-commit deadlines.
  - Resume continues the suspended cycle with the remaining active-execution
    budget rather than resetting the watchdog to a new full interval.
  - A resource pause between cycles starts no new scan or watchdog interval.
- Impact:
  - An operator may inspect a paused program longer than the configured
    watchdog timeout without manufacturing a watchdog fault, while genuine
    active-execution overruns remain bounded.
- Mitigation:
  - Production deployments should keep debug attach disabled unless explicitly
    commissioned. Stop or fault the resource when safe-state handling is
    required instead of relying on a debugger pause.

## 2026-07-14 - Automatic runtime fault restart uses warm storage semantics

- ID: DEV-048
- Area: Runtime fault recovery and retain lifecycle
- IEC reference: IEC 61131-3 Ed.3 section 6.5.6 defines retentive-variable
  storage, but it does not define truST's host `FaultPolicy`, watchdog retry
  action, or automatic recovery loop.
- Deviation/extension:
  - A non-panic cycle fault or watchdog timeout configured for automatic
    restart uses the runtime's warm-restart transition.
  - `RETAIN` values are preserved; `NON_RETAIN` and ordinary initialized
    storage are reinitialized.
  - A contained resource panic is not retried automatically, and automatic
    non-panic retries are limited to three consecutive attempts.
- Impact:
  - Recovery preserves declared retained process state, but ordinary working
    state is reinitialized before execution resumes.
- Mitigation:
  - Declare state `RETAIN` only when it must survive automatic recovery, and
    treat restart-limit exhaustion or a resource panic as an operator-visible
    fault requiring intervention.

## 2026-07-14 - Bound debug writes and forces to a runtime lifecycle

- ID: DEV-047
- Area: Runtime and debug-adapter write/force lifecycle
- IEC reference: IEC 61131-3 does not define truST's DAP or runtime-control
  write, force, release, detach, authorization, or process-lifecycle protocol.
  This entry records a truST product extension outside the IEC language model,
  not a contradiction of an IEC requirement.
- Deviation/extension:
  - Debug writes are one-shot requests consumed at the next scan boundary.
  - Forces persist across ordinary scans, pause/resume, and non-terminating
    debugger disconnect until explicit release.
  - Deliberate stop, fault handling, and warm or cold restart clear pending
    writes and active forces before safe-state handling or execution resumes.
  - Safe-state output has precedence over forcing at stop and fault boundaries.
  - Authorization changes govern later commands and do not silently alter an
    existing force.
- Impact:
  - A pre-restart or pre-fault debug mutation cannot be replayed into a new
    runtime lifecycle. Detaching a client does not unexpectedly change the
    running PLC state.
- Mitigation:
  - Operators must explicitly release forces during an ordinary attached
    session and must reapply any intended force after a restart.

## 2026-07-14 - Require confirmed safe-state driver handoff

- ID: DEV-046
- Area: Runtime safe-state output handoff
- IEC reference: IEC 61131-3 does not define truST's host I/O worker queues,
  protocol reconnect policy, driver-health model, or physical safe-state
  acknowledgement boundary. This entry records an implementer-specific host
  runtime policy, not a contradiction of an IEC requirement.
- Deviation/extension:
  - A configured safe-state attempt succeeds only when every configured driver
    accepts the output image and immediately reports `Ok` health.
  - `Degraded` or `Faulted` health is an unconfirmed physical handoff even when
    a driver's ordinary `warn` or `ignore` policy suppresses its transport
    error.
  - The runtime attempts all configured drivers, reports the first failure, and
    does not let a deliberate stop enter `Stopped` when safe-state delivery is
    unconfirmed.
  - Worker handoff remains bounded. Queued, reconnecting, and timed-out writes
    fail the confirmation attempt instead of blocking indefinitely or being
    reported as physically safe.
- Impact:
  - A runtime may report a safe-state failure even if a queued worker write
    reaches the device later. This is the conservative outcome because delivery
    was not confirmed at the stop/fault boundary.
- Mitigation:
  - Keep protocol workers healthy, configure explicit safe outputs, and treat a
    safe-state failure or `Faulted` stop as requiring operator or supervisory
    recovery before assuming outputs are safe.

## 2026-07-14 - Reject non-finite values at typed runtime admission boundaries

- ID: DEV-045
- Area: Runtime typed I/O, protocol, API, configuration, and retain boundaries
- IEC reference: IEC 61131-3 Ed.3 Section 6.4.2.1, Table 10 defines
  `REAL`/`LREAL` using IEC 60559 and makes results involving infinity or
  not-a-number implementer-specific; Section 6.6.2.5.15, Table 39 defines
  `IS_VALID` but does not define external admission policy. This entry records
  an implementer-specific product policy, not a contradiction of an IEC
  requirement.
- Deviation/extension:
  - truST rejects NaN, positive infinity, negative infinity, and declared-width
    overflow at every typed runtime admission boundary enumerated in
    `docs/specs/11-runtime-engine.md#floating-point-boundary-admission-policy`.
  - Rejection occurs before the external value changes PLC storage or an
    accepted driver snapshot. Multi-point Modbus reads and MQTT payload batches
    are transactional: a rejected point cannot leak an earlier point from the
    same rejected batch into a later scan.
  - Untyped raw-byte/register transport remains possible, but a declared
    `REAL`/`LREAL` binding validates the representation before exposing it as a
    typed value. Rejection never clamps, normalizes, or substitutes a default.
- Impact:
  - External systems cannot use non-finite IEEE values as ordinary PLC process
    inputs through a typed truST boundary.
- Mitigation:
  - Validate values at the producing system and represent exceptional state as
    an explicit finite value/status pair. PLC programs may use `IS_VALID` for
    in-process values where the applicable operation permits them.

## 2026-07-13 - Reject non-finite typed process-image outputs

- ID: DEV-044
- Area: Runtime typed `%Q`/`%M` process-image egress
- IEC reference: IEC 61131-3 Ed.3 defines directly represented variables and
  `REAL`/`LREAL`, but does not define truST's process-image serialization and
  driver-commit transaction.
- Deviation/extension:
  - Typed `%Q` and `%M` `REAL`/`LREAL` bindings reject values that are not
    finite at their declared width. Conversion to `REAL` also rejects a finite
    wider value when basic-single narrowing becomes non-finite.
  - Every eligible binding is validated before any output or marker image
    mutation. Rejection leaves the pending image unchanged and prevents the
    normal driver-output commit.
  - The runtime does not clamp, normalize, substitute, or emit non-finite IEEE
    output bits. Configured safe-state fault handling remains separate.
- Impact:
  - Non-finite floating state cannot silently cross the typed process-image
    boundary or partially update a physical-output transaction.
- Mitigation:
  - Validate and range-check process values before the scan reaches its output
    phase, and model exceptional state with an explicit status value.

## 2026-07-13 - Fault on non-finite REAL named-function results

- ID: DEV-043
- Area: Runtime `REAL` numerical functions
- IEC reference: IEC 61131-3 Ed.3 Tables 28-29 define `EXP` and `EXPT`, while
  §6.4.2.1 Table 10 footnote e defines exceptional basic-single
  floating-point results as implementer-specific.
- Deviation/extension:
  - For finite `REAL` operands, `EXP` and `EXPT` return
    `RuntimeError::Overflow` when the result is not finite at basic-single
    width.
  - The error occurs before assignment storage, so the target retains its
    previous value. The runtime does not clamp or store infinity or NaN.
  - This decision does not define `LREAL`, non-finite operands, subnormal
    underflow, signed zero, explicit conversions, or domain behavior for other
    numerical functions.
- Impact:
  - Programs cannot use infinity or NaN produced by these `REAL` functions as
    ordinary stored process state.
- Mitigation:
  - Scale or range-check operands before the call and represent exceptional
    state with an explicit status value.

## 2026-07-13 - Fault on non-finite REAL binary results

- ID: DEV-042
- Area: Runtime `REAL` binary arithmetic
- IEC reference: IEC 61131-3 Ed.3 §6.4.2.1, Table 10 footnote e defines
  exceptional results for basic-single floating-point arithmetic as
  implementer-specific.
- Deviation/extension:
  - For finite `REAL` operands, binary `+`, `-`, `*`, `/`, and `**` return
    `RuntimeError::Overflow` when the result is not finite at basic-single
    width.
  - The error occurs before assignment storage, so the target retains its
    previous value. The runtime does not clamp or store infinity or NaN.
  - This decision does not define `LREAL`, subnormal underflow, signed zero,
    non-finite operands, explicit conversions, or named numerical functions.
- Impact:
  - Programs cannot use infinity or NaN produced by these `REAL` binary
    operators as ordinary stored process state.
- Mitigation:
  - Scale or range-check operands before the operation and represent exceptional
    state with an explicit status value.

## 2026-07-13 - Reject non-finite simulation coupling thresholds

- ID: DEV-041
- Area: File-backed simulation configuration
- IEC reference: IEC 61131-3 defines `REAL`/`LREAL` using IEC 60559, but it
  does not define truST's implementer-specific `simulation.toml` coupling
  policy.
- Deviation/extension:
  - The `simulation.toml` loader rejects a `[[couplings]].threshold` containing
    `NaN`, positive infinity, or negative infinity before activating the
    configuration.
  - Rejection does not normalize, clamp, or substitute a threshold. Finite
    threshold comparison behavior is unchanged.
- Impact:
  - Simulation files cannot use a non-finite value as an ordinary coupling
    decision threshold.
- Mitigation:
  - Model exceptional simulation state explicitly with finite values and
    scripted disturbances.

## 2026-07-13 - Reject non-finite managed-debug I/O values

- ID: DEV-040
- Area: Debug adapter Live Values floating-point ingress
- IEC reference: IEC 61131-3 defines `REAL`/`LREAL` using IEC 60559, but it
  does not define truST's implementer-specific DAP `stIoWrite` and `stIoForce`
  requests.
- Deviation/extension:
  - In managed local debug sessions, values submitted for declared `REAL` and
    `LREAL` addresses are interpreted as semantic decimal floating-point
    values, not as raw `DWORD`/`LWORD` bit patterns.
  - The adapter rejects `NaN`, positive infinity, negative infinity, and
    values that overflow the declared floating-point width before queuing,
    forcing, or mutating the process image.
  - Rejection preserves the previous process-image value and force state and
    returns a failed DAP response. Attach-mode requests remain governed by the
    separate runtime control endpoint contract.
- Impact:
  - A managed debugger cannot use non-finite values or their integer bit
    encodings as ordinary `REAL`/`LREAL` Live Values inputs.
- Mitigation:
  - Use an explicit finite value/status pair when non-finite state must be
    represented to a PLC program.

## 2026-07-13 - Reject non-finite file-backed retained values

- ID: DEV-039
- Area: Runtime retain persistence
- IEC reference: IEC 61131-3 defines `REAL`/`LREAL` using IEC 60559 and defines
  retentive-variable restart behavior, but it does not define truST's
  implementer-specific file-backed retain image format or admission policy.
- Deviation/extension:
  - truST rejects retained `REAL`/`LREAL` values containing `NaN`, positive
    infinity, or negative infinity when saving or loading a file-backed retain
    snapshot. The rule applies recursively to arrays and structures.
  - A rejected save leaves the last durable snapshot unchanged. A rejected
    load applies no entries from the snapshot and leaves the runtime's retained
    values unchanged.
  - The runtime does not clamp, normalize, or replace a non-finite retained
    value with a default.
- Impact:
  - Projects cannot persist non-finite values through truST's file-backed retain
    store as ordinary process state.
- Mitigation:
  - Use `IS_VALID` and project logic to convert intentional non-finite internal
    state into an explicit finite status/value pair before persistence.

## 2026-07-13 - Reject out-of-range runtime mesh numeric inputs

- ID: DEV-038
- Area: Runtime mesh numeric ingress
- IEC reference: IEC 61131-3 defines the ranges of elementary integer types and
  defines `REAL`/`LREAL` using IEC 60559, but it does not define truST's
  implementer-specific runtime mesh transport.
- Deviation/extension:
  - truST decodes an inbound mesh number against the configured local target
    type and rejects integers outside that type's range.
  - A mesh number targeting `REAL` or `LREAL` is rejected when conversion would
    produce `NaN`, positive infinity, or negative infinity.
  - Rejection queues no mesh update and leaves the local PLC value unchanged;
    the runtime does not clamp, wrap, normalize, or substitute a default.
- Impact:
  - Mesh peers cannot rely on Rust-style narrowing casts or non-finite values as
    ordinary process-data inputs.
- Mitigation:
  - Validate and scale values at the publishing peer so they fit the subscribed
    IEC target type.

## 2026-07-13 - Reject non-finite ADS input values

- ID: DEV-037
- Area: Runtime ADS floating-point ingress
- IEC reference: IEC 61131-3 defines `REAL`/`LREAL` using IEC 60559 and exposes
  `IS_VALID` for detecting `NaN` and infinity, but it does not define how an
  ADS client or server must admit those values into PLC storage.
- Deviation/extension:
  - truST rejects ADS `REAL`/`LREAL` client reads, notifications, and server
    writes containing `NaN`, positive infinity, or negative infinity.
  - Rejection occurs before client cache acceptance, server write queuing, or
    PLC storage mutation and also applies to non-finite array elements.
- Impact:
  - ADS peers cannot use non-finite values as ordinary floating-point process
    inputs in truST.
- Mitigation:
  - Validate or normalize the value at the ADS peer when a project intentionally
    uses non-finite values for out-of-band signaling.

## 2026-07-13 - Reject non-finite OPC UA client input samples

- ID: DEV-036
- Area: Runtime OPC UA client floating-point ingress
- IEC reference: IEC 61131-3 defines `REAL`/`LREAL` using IEC 60559 and exposes
  `IS_VALID` for detecting `NaN` and infinity, but it does not define how a
  host OPC UA client must admit those values into PLC storage.
- Deviation/extension:
  - truST rejects OPC UA client `Float`/`Double` input samples containing
    `NaN`, positive infinity, or negative infinity before cache acceptance or
    PLC storage mutation.
  - The affected point becomes faulted and the previous PLC value is retained.
  - The value is not normalized, clamped, or replaced with a default.
- Impact:
  - An OPC UA server cannot use non-finite scalar values as ordinary
    `REAL`/`LREAL` process inputs in truST.
- Mitigation:
  - Validate or normalize the source value at the OPC UA server when a project
    intentionally uses non-finite values for out-of-band signaling.

## 2026-07-13 - Watchdog output-commit and empty safe-state policy

- ID: DEV-035
- Area: Runtime watchdog and physical output commit
- IEC reference: IEC 61131-3 does not standardize truST's host-runtime watchdog
  thresholds, fault action, or physical driver commit boundary.
- Deviation/extension:
  - truST checks the watchdog deadline before the physical output-driver write.
  - An expired deadline prevents the pending process-image output from being
    written.
  - Fault handling writes configured safe-state values when at least one value
    is configured. With an empty safe-state configuration, it performs no
    physical output write.
- Impact:
  - Without configured safe outputs, the device remains at its last physically
    committed state after the watchdog fault; the uncommitted current-cycle
    image is never used as an implicit safe state.
- Mitigation:
  - Configure explicit safe-state outputs for installations that require a
    physical transition on watchdog fault.

## 2026-04-27 - `UNION` aggregate initialization as truST extension

- Area: Structured Text data types and initializers
- IEC reference: IEC 61131-3 Ed.3 defines structured aggregate initialization
  for structures and FB declarations; `UNION` is not a core Ed.3 data-type
  construct in the same way as `STRUCT`.
- Deviation:
  - truST accepts `UNION` / `END_UNION` as a vendor-style data type and routes
    named union aggregate initializers through the same checked initializer
    machinery as structures.
- Impact:
  - Projects can use named union defaults in truST, but strict IEC exporters may
    need to lower or reject these declarations.
- Mitigation:
  - The behavior is explicit in parser/HIR/runtime tests and uses the same
    unknown/duplicate-field diagnostics as structure initialization.

## 2026-02-25 - CTUD single-input profile in LD v2 node model

- Area: Ladder Diagram counter node representation
- IEC reference: Counter FBs (IEC 61131-3 Ed.3, counter FB tables)
- Deviation:
  - LD schema v2 `counter` node currently exposes one power input.
  - `CTUD` is executed as CU-driven (rising-edge increment) in this profile; separate CD/QD wiring is not represented in node schema yet.
- Impact:
  - Full dual-input CTUD semantics are not available in current LD node contract.
- Mitigation:
  - Behavior is explicit in tests and docs; future schema extension can add dedicated CU/CD/R/LD pins.

## 2026-02-25 - TP/TOF ET exposure uses internal millisecond state

- Area: Ladder Diagram timer diagnostics/state exposure
- IEC reference: Timer FBs (IEC 61131-3 Ed.3, timer timing tables)
- Deviation:
  - Internal ET storage for TP/TOF diagnostics is represented as implementation-facing millisecond state in `%MW_LD_TIMER_<instance>_ET`.
- Impact:
  - Exposed ET key is engine-internal and not a normative IEC variable contract.
- Mitigation:
  - Runtime behavior (`Q` transitions) is tested; ET key is documented as implementation detail.

## 2026-02-25 - PLCopen LD interop subset

- Area: PLCopen LD import/export
- IEC reference: PLCopen XML graphical-body interchange profiles (vendor ecosystem variance)
- Deviation:
  - LD import/export currently targets the supported LD network-body subset used by `editors/vscode/src/ladder/plcopenLdInterop.ts`.
  - Unsupported graphical/vendor constructs are skipped with explicit diagnostics.
- Impact:
  - Not all vendor-specific graphical metadata/layout constructs are round-tripped.
- Mitigation:
  - Unsupported constructs are reported deterministically and covered by interop tests.

## 2026-02-27 - LD node operands use free-form string references

- Area: LD schema v2 operand contract
- IEC reference: Section 8.2 LD operands with declaration-driven typing and scope
- Deviation:
  - Node operands (`contact.variable`, `coil.variable`, compare/math operands) are represented as plain strings in schema v2.
  - Schema v2 does not yet provide explicit `symbolRef` vs `directAddress` discriminators.
- Impact:
  - Symbolic and direct-address references are syntactically mixed at profile level.
  - Additional validation is required to enforce strict declaration-driven addressing policies.
- Mitigation:
  - Normative spec defines symbolic-first policy; profile constraints are documented in `docs/specs/16-ladder-profile-trust.md`.

## 2026-02-27 - Runtime forcing path symbolic support closure

- Area: LD runtime I/O write/force operations
- IEC reference: Implementation-specific external I/O binding around LD execution model
- Previous deviation:
  - Runtime write/force/release operations were direct-address centric.
- Current status:
  - Closed in this stream. Runtime write/force/release now resolve declared symbols
    (including scoped references) in addition to direct `%IX*` addressing.
- Impact:
  - Symbol-first LD projects can be exercised from runtime controls without mandatory
    direct-address operands in node fields.

## 2026-02-27 - LD contact/coil symbol subset (Table 75/76)

- Area: Ladder Diagram symbol set exposed in schema v2/editor tooling
- IEC reference: IEC 61131-3 Ed.3 Table 75 (Contacts), Table 76 (Coils)
- Deviation:
  - Current schema v2/editor profile implements static contacts (`NO`, `NC`) and coil
    variants (`NORMAL`, `NEGATED`, `SET`, `RESET`).
  - Transition-sensing contact/coil variants from Table 75/76 are not yet represented in
    node schema.
- Impact:
  - Users cannot model transition-sensing LD symbols directly in the current profile.
- Mitigation:
  - Unsupported symbol forms are not silently coerced; they are rejected with explicit
    diagnostics.

## 2026-04-11 - Numeric hazard diagnostics for ST expressions

- Area: Structured Text diagnostics
- IEC reference: IEC 61131-3 Ed.3 defines expression evaluation and runtime fault behavior, but does not require warnings for floating-point equality or literal zero divisors.
- Deviation:
  - The type checker emits `W013` for `=`/`<>` comparisons when either operand is `REAL`/`LREAL`.
  - The type checker emits `W014` for `DIV`/`MOD` expressions whose right-hand operand is a literal zero.
- Impact:
  - truST reports additional proactive diagnostics beyond strict IEC conformance.
- Mitigation:
  - These are configurable tooling warnings under `[diagnostics].warn_numeric_hazards`, and severities can still be overridden per code.

## 2026-04-11 - File-scope `VAR_GLOBAL` as vendor-style GVL

- Area: Structured Text global-variable declarations
- IEC reference: IEC 61131-3 Ed.3 models globals through `PROGRAM`/`CONFIGURATION`/`RESOURCE`; vendor ecosystems such as CODESYS/TwinCAT also use standalone GVL source files.
- Deviation:
  - truST accepts top-level file-scope `VAR_GLOBAL ... END_VAR` blocks and treats them as global variable libraries (GVLs).
- Impact:
  - CODESYS/TwinCAT-style GVL source files compile directly in truST without wrapping them in a `CONFIGURATION`.
- Mitigation:
  - Duplicate global names in the same effective namespace are rejected.
  - Strict-IEC reshaping remains available as an adapter/export concern rather than a core-language requirement.

## 2026-04-11 - Namespaced vendor-style GVLs

- Area: Structured Text global-variable declarations
- IEC reference: `NAMESPACE`-scoped global-variable libraries are a vendor extension rather than an IEC Ed.3 construct.
- Deviation:
  - truST accepts `NAMESPACE ... VAR_GLOBAL ... END_NAMESPACE`.
  - Qualified access such as `GVL.shared` resolves against the namespaced global directly.
  - CODESYS `{attribute 'qualified_only'}` is not enforced as a semantic restriction in core truST yet.
- Impact:
  - Vendor-style namespaced GVLs compile directly in truST, including qualified reads/writes.
  - Projects imported from vendor tooling may still allow bare access where CODESYS would require qualification.
- Mitigation:
  - Strict import/export paths may keep wrapper or injected-`VAR_EXTERNAL` transforms for external consumers that need them, including PLCopen import calls that opt into `PlcopenImportGlobalVarMode::StrictIecAdapter`.
  - Documentation calls out the current `qualified_only` limitation explicitly.

## 2026-04-11 - Optional `VAR_EXTERNAL` for vendor-parity global access

- Area: Structured Text global-variable access
- IEC reference: IEC 61131-3 Ed.3 §6.5.2.2 / Figure 8 requires explicit `VAR_EXTERNAL` linkage for external global access.
- Deviation:
  - truST accepts direct global access without requiring a matching `VAR_EXTERNAL` declaration.
  - `VAR_EXTERNAL` remains supported and type-checked when authors choose to declare it.
  - This vendor-parity path applies to configuration/resource globals, file-scope GVLs, and qualified namespaced GVL access.
- Impact:
  - CODESYS/TwinCAT-style ST authored without injected `VAR_EXTERNAL` blocks compiles directly in truST.
- Mitigation:
  - Undefined bare names still diagnose as errors.
  - Strict-IEC export/adapter flows may still synthesize `VAR_EXTERNAL` declarations when targeting stricter consumers, including the optional PLCopen strict-adapter import mode.

## 2026-04-11 - `VAR_STAT` runtime semantics

- Area: Structured Text static variables
- IEC reference: `VAR_STAT` is a vendor extension and is not defined by IEC 61131-3 Ed.3.
- Deviation:
  - truST accepts `VAR_STAT` and gives it persistent storage semantics.
  - In `FUNCTION`, `VAR_STAT` persists across calls to that function definition.
  - In `METHOD`, `VAR_STAT` persists per enclosing instance and per method.
  - In `PROGRAM`, `FUNCTION_BLOCK`, and `CLASS`, `VAR_STAT` behaves as ordinary instance storage in the enclosing instance-bearing scope.
- Impact:
  - Vendor-authored code using `VAR_STAT` compiles and preserves static state without rewriting to IEC-only forms.
- Mitigation:
  - `VAR_STAT` remains an explicit vendor extension in docs/specs.
  - Strict-IEC export/adapter paths may rewrite or reject `VAR_STAT` for consumers that do not support it.

## 2026-04-17 - ADR / SIZEOF built-ins

- ID: DEV-016
- Area: ADR / SIZEOF built-ins
- IEC reference: Not specified in IEC 61131-3 Ed3; vendor extension.
- Deviation:
  - truST parses and executes `ADR(...)` and `SIZEOF(...)` as built-in expressions.
  - `SIZEOF(...)` returns a `DINT` byte count for the static storage representation of either an explicit `type_ref` or a storage operand (`var`, field/index access, dereference, `THIS.field`) without evaluating the operand.
  - Bare identifiers resolve variables before type names, matching common CODESYS shadowing behavior.
  - `SIZEOF(...)` is const-foldable when the static type is known.
  - `STRING[n]` reports `n`, `WSTRING[n]` reports `2n`.
  - `POINTER TO` / `REF_TO` operands report the platform pointer word size (`sizeof(usize)`), not truST's internal runtime handle layout.
  - Open arrays, unsized strings/WSTRINGs, and whole FB/class/interface instances are rejected.
- Impact:
  - Common vendor ST patterns like `ARRAY[0..SIZEOF(packet)-1] OF BYTE` compile and fold deterministically.
  - Pointer `SIZEOF` matches platform pointer width rather than leaking the size of truST's private runtime reference handle.
  - `ADR(str)` yields a typed pointer/reference-compatible handle for string dereference/indexing, not a raw byte-array reinterpretation of string storage.
- Mitigation:
  - This behavior is documented as a vendor extension rather than an IEC core feature.

## 2026-04-17 - Runtime STRING / WSTRING element semantics

- ID: DEV-017
- Area: Runtime string indexing, character access, and stdlib string element operations
- IEC reference:
  - IEC 61131-3 Ed.3 Table 10 defines `STRING` as single-byte and `WSTRING` as double-byte character strings.
  - IEC examples and common vendor practice use 1-based element access for `str[idx]`.
- Deviation:
  - truST stores `STRING` as UTF-8 text and `WSTRING` as Rust `String` text rather than raw single-byte / UCS-2 buffers.
  - Public runtime element access is 1-based for both `STRING` and `WSTRING`.
  - `STRING[idx]`, `LEN`, `LEFT`, `RIGHT`, `MID`, `INSERT`, `DELETE`, `REPLACE`, and `FIND` operate on Unicode scalar elements, not raw UTF-8 bytes.
  - `WSTRING[idx]` and the same stdlib helpers operate on the same Unicode scalar element model rather than raw 16-bit code units.
  - Materializing a `STRING` element as `CHAR` still requires the selected scalar value to fit in `u8`; otherwise the runtime reports overflow.
  - Materializing a `WSTRING` element as `WCHAR` requires the scalar value to fit in `u16`.
  - `SIZEOF(STRING[n])` and `SIZEOF(WSTRING[n])` remain storage-oriented (`n` and `2n` respectively), while runtime value sizing uses the same scalar-element counts described above.
- Impact:
  - Non-ASCII text behaves consistently across VM ref indexing and the shipped string stdlib, but the behavior is not raw IEC byte/code-unit indexing.
  - Existing projects that feed UTF-8 strings through file, MQTT, or fieldbus paths get stable element access semantics instead of mixed byte/scalar behavior.
- Mitigation:
- The runtime and docs now use one explicit element model end-to-end.
- A future raw-byte/raw-code-unit storage rewrite would be a separate compatibility project because it would require changing the underlying runtime value representation.

## 2026-04-20 - POINTER TO support beyond IEC REF_TO

- ID: DEV-018
- Area: Structured Text pointer types and runtime pointer operations
- IEC reference: `REF_TO` is standardized in IEC 61131-3 Ed.3 §6.4.4.10.2;
  `POINTER TO`, `ADR`, and dereference-style pointer workflows are vendor
  extensions.
- Deviation:
  - truST supports typed `POINTER TO` declarations in the language/type system.
  - `ADR(...)`, dereference (`^`), `NULL`, and `?=` work for pointers in both
    `trust-hir` and `trust-runtime`.
  - `POINTER TO` shares the runtime reference storage model used by `REF_TO`.
  - Pointer arithmetic is not supported.
- Impact:
  - Vendor-authored pointer-oriented ST can compile and execute in truST
    without rewriting to IEC-only `REF_TO` forms.
- Mitigation:
  - The extension is documented explicitly in specs and remains typed and
    non-arithmetic.

## 2026-04-20 - Assertion helper functions for test POUs

- ID: DEV-019
- Area: Standard-library assertion helpers
- IEC reference: IEC 61131-3 Ed.3 Tables 22-36 do not define assertion
  functions such as `ASSERT_TRUE` / `ASSERT_FALSE`.
- Deviation:
  - truST ships `ASSERT_TRUE`, `ASSERT_FALSE`, `ASSERT_EQUAL`,
    `ASSERT_NOT_EQUAL`, `ASSERT_GREATER`, `ASSERT_LESS`,
    `ASSERT_GREATER_OR_EQUAL`, `ASSERT_LESS_OR_EQUAL`, and `ASSERT_NEAR`.
  - These functions are executed by the runtime test framework and report
    assertion context in failure output.
- Impact:
  - Test POUs can use built-in assertion helpers instead of encoding
    expectations manually in ST control flow.
- Mitigation:
  - The helpers are documented as non-IEC test extensions and scoped to the
    testing/runtime workflow.

## 2026-04-20 - Reserved SFC keywords without textual SFC body syntax

- ID: DEV-020
- Area: Lexing and SFC authoring profile
- IEC reference: IEC 61131-3 Ed.3 reserves SFC keywords and defines SFC
  constructs, but truST currently ships visual SFC authoring rather than a
  textual SFC body syntax inside the ST parser.
- Deviation:
  - truST reserves SFC keywords such as `STEP`, `TRANSITION`, and `ACTION`.
  - truST ships a visual SFC editor/profile in the public docs.
  - Textual SFC body syntax is not currently accepted as part of the ST parser.
- Impact:
  - SFC-related words remain unavailable as identifiers even though textual SFC
    bodies are not yet part of the shipped ST grammar.
- Mitigation:
  - The reserved-keyword scope and authoring workflow are documented explicitly
    in the lexer spec and visual-editor docs.

## 2026-04-20 - Simplified `VAR_ACCESS` / `VAR_CONFIG` path validation

- ID: DEV-003
- Area: Structured Text access-path and configuration-variable validation
- IEC reference: IEC 61131-3 Ed.3 §6.5.2.2, Tables 13-16
- Deviation:
  - truST validates declared access-path shape and target-type compatibility.
  - Cross-resource / cross-program-instance mapping is not modeled completely in
    `trust-hir`.
- Impact:
  - Full IEC communication-service topology is not statically proven in the
    language layer.
- Mitigation:
  - The supported subset is documented in the variables spec and enforced
    consistently for the accepted forms.

## 2026-04-20 - Assignment-attempt compatibility is runtime-oriented

- ID: DEV-006
- Area: `?=` assignment-attempt semantics
- IEC reference: IEC 61131-3 Ed.3 §6.6.6.7.2, Table 71
- Deviation:
  - truST accepts `?=` for typed reference-style assignment attempts.
  - `trust-hir` does not fully enforce inheritance/interface compatibility for
    the source/target pair during static analysis.
- Impact:
  - Some compatibility failures are deferred to runtime null-result behavior
    instead of being diagnosed statically.
- Mitigation:
  - The operator remains typed, null-producing behavior is documented, and
    callers must check before dereference.

## 2026-04-20 - `PERSISTENT` behaves like `RETAIN`

- ID: DEV-007
- Area: Variable persistence qualifiers
- IEC reference: `PERSISTENT` is a vendor extension, not an IEC Ed.3 storage
  qualifier.
- Deviation:
  - truST accepts `PERSISTENT` and validates it with the same storage semantics
    as `RETAIN`.
- Impact:
  - Vendor-authored code using `PERSISTENT` compiles without rewriting to IEC
    core qualifiers.
- Mitigation:
  - The qualifier is documented explicitly as a vendor extension.

## 2026-04-20 - Standard FB semantic ownership split

- ID: DEV-010
- Area: Standard function-block modeling
- IEC reference: IEC 61131-3 Ed.3 §6.6.3.5, Tables 43-46, Figure 15
- Deviation:
  - `trust-hir` validates standard FB signatures and static types only.
  - `trust-runtime` executes the stateful timer/counter/trigger behavior.
- Impact:
  - Static docs/specs must distinguish language-level signature knowledge from
    runtime behavior/state.
- Mitigation:
  - The standard-FB spec now documents the split explicitly and cross-links the
    runtime-owned deviations.

## 2026-04-20 - ASCII-only identifier validation

- ID: DEV-013
- Area: Lexical identifier support
- IEC reference: IEC 61131-3 Ed.3 §6.1.1-6.1.2 permits broader character sets
  than truST currently accepts.
- Deviation:
  - truST validates identifiers using ASCII letters, digits, and `_` only.
  - Unicode identifiers are not currently accepted.
- Impact:
  - Some IEC-valid identifiers from broader character sets are rejected.
- Mitigation:
  - The lexer spec calls out the current limitation explicitly.

## 2026-04-20 - Extra non-IEC conversion helpers

- ID: DEV-021
- Area: Standard-library conversion functions
- IEC reference: IEC 61131-3 Ed.3 Tables 22-27 do not define `TIME_TO_DWORD`,
  `DWORD_TO_TIME`, or direct character-to-bitstring conversions such as
  `CHAR_TO_BYTE`.
- Deviation:
  - truST ships `TIME_TO_DWORD` / `DWORD_TO_TIME` using millisecond units.
  - truST accepts direct character-to-bitstring conversions such as
    `CHAR_TO_BYTE` and `WCHAR_TO_WORD`.
- Impact:
  - Vendor-oriented conversion workflows compile without custom wrappers.
- Mitigation:
  - These helpers are documented as non-IEC conversion extensions in the
    standard-functions spec.

## 2026-04-20 - Debugger visibility is broader than access-specifier rules

- ID: DEV-023
- Area: Debug-adapter variable visibility
- IEC reference: IEC 61131-3 Ed.3 §6.5.2.3 access-specifier rules
- Deviation:
  - The debugger may expose `PRIVATE`, `PROTECTED`, and `INTERNAL` members for
    inspection even when normal source-level access rules would hide them.
- Impact:
  - Debug inspection is more permissive than source-level member access.
- Mitigation:
  - The behavior is documented explicitly as a debug-surface deviation.

## 2026-04-20 - Per-resource hot reload with retained globals

- ID: DEV-024
- Area: Runtime online change / hot reload
- IEC reference: IEC 61131-3 does not standardize truST's runtime reload API.
- Deviation:
  - truST hot reload operates per resource rather than as a single-file-only
    mechanism.
  - Retained globals are preserved across the warm-restart style reload path.
  - Reload preparation is transactional: a fallible bytecode, resource, or
    retained-state preparation step cannot replace the live program or state.
- Impact:
  - The reload scope is broader and more runtime-oriented than earlier
    single-file wording suggested.
- Mitigation:
  - The engine/LSP/debug docs define the per-resource transaction boundary and
    require failed preparation to leave the old program executable.

## 2026-04-20 - Debug forcing includes outputs as well as inputs

- ID: DEV-025
- Area: Runtime debug I/O forcing
- IEC reference: IEC 61131-3 does not define truST's DAP/control forcing
  surface.
- Deviation:
  - truST allows debugger/control forcing for output areas in addition to input
    areas.
- Impact:
  - The debug/runtime control path can simulate or override both sides of the
    process image.
- Mitigation:
  - The behavior is documented explicitly in runtime-engine/debug docs.

## 2026-04-20 - Test-oriented POU declarations

- ID: DEV-033
- Area: POU declarations
- IEC reference: IEC 61131-3 Ed.3 does not define `TEST_PROGRAM` or
  `TEST_FUNCTION_BLOCK`.
- Deviation:
  - truST accepts `TEST_PROGRAM` and `TEST_FUNCTION_BLOCK` as test-oriented
    declaration forms for the built-in test workflow.
- Impact:
  - Test POUs can be authored directly without wrapping test logic into regular
    IEC POUs plus external harness glue.
- Mitigation:
  - The declarations are documented as explicit non-IEC extensions and paired
    with the assertion-helper runtime workflow.

## 2026-04-20 - Siemens SCL `#local` reference prefix

- ID: DEV-034
- Area: Structured Text name references
- IEC reference: IEC 61131-3 Ed.3 does not define `#identifier` as a local-name
  reference operator in ST expressions/statements.
- Deviation:
  - truST accepts `#identifier` and lowers it as the same local name reference
    as `identifier`.
  - Malformed uses still diagnose with `expected identifier after '#'`.
- Impact:
  - Siemens-authored SCL that prefixes local names with `#` compiles without
    source rewriting.
- Mitigation:
  - The behavior is documented as a vendor-compatibility extension rather than
    IEC core syntax.
