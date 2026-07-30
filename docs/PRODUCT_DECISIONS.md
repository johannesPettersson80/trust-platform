# truST Product Decisions Log

This file records reviewed truST product and tooling decisions that are not
interpretations of IEC 61131-3 or PLCopen. IEC interpretation decisions belong
in `IEC_DECISIONS.md`; PLCopen profile decisions belong in
`PLCOPEN_DECISIONS.md`. Product behavior recorded here MUST NOT be copied to an
IEC or PLCopen deviation log merely because truST owns the behavior.

## 2026-07-28 - Runtime symbolic boundary and queued-write failure authority

- Area: Runtime test harness, storage, and debug mutation
- Decision:
  - In-process symbolic reads accept reviewed indexed and dotted paths, reject
    unknown and ambiguous unqualified paths visibly, and distinguish a
    declared null reference value from a missing name.
  - Harness input and direct-binding typos fail before fallback global or
    binding creation; a following snapshot retains the declared target and an
    explicit unresolved result for the typo.
  - Queued debug global and lvalue writes resolve existing targets at the cycle
    boundary. An unresolved target faults the cycle without creating a global,
    and the reviewed lvalue route publishes a matching runtime fault event.
- Authority:
  - `docs/specs/10-runtime-semantics.md`
- Reason:
  - Error codes, fallback-creation policy, candidate reporting, harness
    snapshots, and runtime fault events are truST host-product behavior outside
    IEC 61131-3. IEC 61131-3 Ed.3 §6.6.6.5.1 separately governs the initial
    NULL value of an uninitialized interface variable; that normative rule is
    implemented here without an IEC decision or deviation.

## 2026-07-28 - Integer initializer diagnostic identity

- Area: HIR variable initializer diagnostics
- Decision:
  - An integer variable initializer that depends on an ordinary mutable
    variable is rejected with category `E202` and the message `variable
    initializer must be a literal or constant expression`.
  - Literal and supported declared-constant integer expressions remain
    accepted.
- Authority:
  - `docs/specs/03-variables.md#initialization-table-14`
- Reason:
  - IEC 61131-3 Section 6.5.1.3 supplies the semantic rejection rule, but it
    does not prescribe truST's diagnostic category or wording. The stable
    diagnostic identity is therefore a product decision, not an IEC
    deviation.

## 2026-07-28 - Tutorial examples are executable product contracts

- Area: Shipped Structured Text tutorials
- Decision:
  - All nine single-file tutorials must compile through both the runtime
    harness and the current source-to-bytecode path.
  - The blinker, traffic-light, and motor-starter tutorials retain the exact
    clean-cycle timing and I/O sequences specified in
    `docs/specs/22-developer-workflows.md`.
  - These are bounded fixture contracts. They do not generalize the examples
    into IEC timer, traffic-control, or motor-safety requirements.
- Authority:
  - `docs/specs/22-developer-workflows.md#executable-tutorial-examples`
- Reason:
  - Tutorials are user-facing product assets and should fail visibly when
    compiler or runtime changes make their documented behavior stale. IEC
    61131-3 does not define these truST example files, so the decision is not
    an IEC deviation.

## 2026-07-28 - Qualified program retain persistence

- Area: Runtime retain snapshots and restart
- Decision:
  - File-backed snapshots include retentive variables declared inside a
    `PROGRAM`, not only configuration/global retained variables.
  - Program-variable entries use the reserved internal key
    `@program/<program-identity>/<variable-identity>`. The reserved prefix
    cannot collide with an IEC identifier; namespace punctuation in a program
    identity remains part of that identity.
  - Snapshot creation follows runtime program order and declaration order.
    Warm load resolves program and variable identity case-insensitively,
    validates and migrates every entry before applying any retained target,
    then restores the selected instance variable.
  - A cold restart through the test harness initializes retained variables and
    does not load the configured retain store.
  - Plain destruction of a runtime or test harness is not a requested graceful
    stop and does not implicitly publish dirty retained state.
- Authority:
  - IEC 61131-3 Ed.3, Section 6.5.6 and Figure 9.
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - IEC defines the warm/cold retentive behavior, while persisted key encoding,
    deterministic snapshot order, and the harness/store boundary are truST
    product choices. These choices implement the IEC behavior and are not IEC
    deviations.

## 2026-07-28 - Runtime logical clock and non-pacing tick boundary

- Area: Runtime clock injection and scheduler stepping
- Decision:
  - `StdClock::now` exposes nondecreasing elapsed monotonic time.
  - A direct `ResourceRunner::tick` is one non-pacing step: it samples the
    injected clock and executes one scheduler cycle without calling
    `Clock::sleep_until`. Pacing belongs to the spawned resource loop.
  - The zero-argument truST compatibility call `TIME()` returns the runtime's
    current injected logical elapsed time as a `TIME` value. Harness and
    simulation time affect it only through the runtime/manual clock.
  - Task readiness is derived from the injected clock. Holding a manual clock
    fixed does not make a periodic task ready; explicitly advancing it to the
    deadline does.
  - `CURRENT_DT` remains a separate host-clock function. It samples UTC
    `SystemTime` independently on each call, returns timezone-naive `DT` ticks
    from the Unix epoch at fixed one-millisecond resolution, truncates positive
    sub-millisecond fractions, and accepts tick `0..=i64::MAX`.
  - A pre-epoch or larger host value fails with `RuntimeError::Overflow`.
    Runtime/manual clocks, scheduler scaling, simulation, and replay do not
    control the result; host-clock rollback is visible rather than clamped.
    Deterministic replay therefore excludes programs that call `CURRENT_DT`
    unless the environment controls the host clock.
- Authority:
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - Clock injection, direct-step pacing, and the compatibility `TIME()` call
    are truST runtime behavior outside IEC's standard function set.
    Separating them from `CURRENT_DT` prevents deterministic elapsed time from
    inheriting an unrelated civil-clock specification gap. This is product
    authority, not an IEC deviation.

## 2026-07-28 - Namespace-qualified multifile runtime assembly

- Area: Runtime source assembly and namespace identity
- Decision:
  - A direct member imported by `USING` remains callable when its namespace and
    consuming program are supplied as separate source units.
  - Runtime registries preserve namespace-qualified identities for programs,
    functions, function blocks, classes, and interfaces. A namespaced program
    remains an executable scan entry point.
  - Within one namespace, sibling functions, function blocks, classes, and
    interfaces resolve as one semantic graph and execute without dropping the
    namespace identity.
  - Program identities are unique under ASCII-case-insensitive comparison
    across the merged source set. A collision rejects assembly and identifies
    both the duplicate condition and the reviewed original spelling.
- Authority:
  - IEC 61131-3 Ed.3, Section 6.9 and Tables 64-66.
  - `docs/specs/04-pou-declarations.md`
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - Namespace membership and `USING` resolution follow IEC. Runtime registry
    naming, scan-entry selection, merged-source collision handling, and
    diagnostic identity are observable truST product integration rules. They
    require product authority but do not conflict with IEC and are not an IEC
    deviation.

## 2026-07-28 - Native CI fixture and performance authority boundary

- Area: Developer test CI compatibility
- Decision:
  - The runtime compatibility entrypoint preserves the reviewed version-1
    build, validate, and test JSON fields plus green and broken JUnit fixture
    outcomes documented in `docs/specs/22-developer-workflows.md`.
  - The broken fixture returns test-failure class 12, emits a JUnit failure
    entry, and identifies failed ST tests on stderr.
  - The existing elapsed-time assertions are not product performance oracles
    until a reference environment defines hardware, operating system,
    prebuilt-versus-clean state, cache state, concurrency, repetitions, and
    aggregation. In particular, the test named “clean setup” uses already
    built binaries and is not clean-install evidence.
- Authority:
  - `docs/specs/22-developer-workflows.md`
- Reason:
  - Machine-report compatibility is deterministic product behavior.
    Unqualified wall-clock thresholds are environment observations and cannot
    honestly establish portable performance budgets.

## 2026-07-28 - Syntax-kind classifier authority

- Area: Lexer and concrete syntax tree
- Decision:
  - Lexer token kinds convert to same-named syntax token kinds, with `Eof` as
    the final token-kind boundary and later `SyntaxKind` values classified as
    composite nodes.
  - Parser trivia is whitespace, line comments, block comments, and pragmas.
  - Generic expression nodes and aggregate initializer nodes remain distinct;
    initializer position accepts their union.
  - The POU-like semantic-owner set contains program, function, function
    block, class, method, property, and interface nodes in that canonical
    iteration order. Accessors, configuration objects, namespaces, and
    resources are excluded.
- Authority:
  - `docs/specs/01-lexical-elements.md`
  - `docs/specs/03-variables.md`
  - `docs/specs/04-pou-declarations.md`
- Reason:
  - These classifier boundaries are stable truST compiler APIs used by parser
    and semantic consumers. They describe internal representation and
    ownership; they do not redefine IEC source syntax and are not IEC
    deviations.

## 2026-07-28 - Resource scheduler cycle, fault, and trace semantics

- Area: Runtime resource scheduler
- Decision:
  - A resource tick samples the current injected clock and task inputs, then
    executes each due periodic or rising-edge event program once in the
    scheduler's deterministic order.
  - A backward manual-clock sample neither replays periodic work nor creates an
    overrun; execution resumes from the unchanged periodic baseline.
  - A task execution error is returned, latches the runtime fault state, and
    makes a later tick reject as `ResourceFaulted` until an explicit recovery
    transition occurs.
  - Replaying the same source, manual-clock steps, and input transitions
    produces the same ordered runtime-event vector.
- Authority:
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - IEC defines task triggers and priority, while resource-cycle realization,
    fault latching, and debug-event trace reproducibility are truST runtime
    policy. These choices are product authority, not IEC deviations.

## 2026-07-28 - Direct restart staging and recursive retain identity

- Area: Runtime restart and retained-state migration
- Decision:
  - An in-process warm restart stages its complete replacement storage and
    instance image before commit. Instance-construction failure preserves the
    live executable, storage, queued debug force, and cycle counter.
  - Compatible retained enum, structure, and one-dimensional array values are
    rebuilt against the current declared type, variant, field, and inclusive
    bounds identities. Incompatible nested fields or elements reject
    application.
- Supersedes:
  - Any interpretation that successful scalar migration alone proves recursive
    aggregate migration, or that a failed direct restart may partially replace
    live state.
- Authority:
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - These staging and identity-rebuild rules are truST runtime policy outside
    IEC 61131-3 and therefore are not IEC deviations.

## 2026-07-28 - In-process test-harness control semantics

- Area: Runtime testing API
- Decision:
  - `TestHarness::from_source` constructs a fresh initialized runtime before
    any cycle, with zero simulated time and zero completed cycles.
  - Symbolic and bound direct I/O are applied through the harness cycle
    boundary. `run_cycles` executes the requested count, and `run_until` checks
    its predicate before the first cycle and returns only cycles it executed.
  - Bounded `run_until_max` panics after exhausting its reviewed limit with
    `run_until exceeded <N> cycles`; an already-true predicate executes no
    cycle.
- Supersedes:
  - Any interpretation that the in-process Rust API proves the JSON-line
    transport, wall-clock advancement, physical I/O, or arbitrary runtime
    execution beyond the reviewed fixtures.
- Authority:
  - `docs/specs/10-runtime-semantics.md`
- Reason:
  - These are deterministic truST testing-API choices outside IEC 61131-3 and
    therefore belong in product authority rather than `IEC_DEVIATIONS.md`.

## 2026-07-28 - Runtime hot-reload transaction boundary

- Area: Runtime online change
- Decision:
  - A reload commits only after the in-flight scan completes. A successful warm
    reload preserves reviewed retained values, warm-initializes non-retained
    storage and instances, rebinds I/O, restarts at the new entry point, and
    resets the scan counter.
  - Preparation failure preserves the live runtime. For the reviewed retained
    `INT(0..10)` value `100`, rejection retains the cause text
    `outside declared subrange 0..10`; malformed bytes `[00, 01, 02, 03]`
    return the typed `RuntimeError::Bytecode` failure.
- Supersedes:
  - Any interpretation that a reload may report success before the complete
    cycle-boundary commit, or that these focused tests prove reload latency,
    external I/O hardware, DAP state preservation, or every malformed module.
- Authority:
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - Online change is a truST runtime transaction outside IEC 61131-3. Its
    commit, restart, and failure identities belong here rather than in
    `IEC_DEVIATIONS.md`.

## 2026-07-28 - Explicit VOID function bytecode rejection

- Area: Source-to-bytecode lowering
- Decision:
  - A function declared with the reviewed explicit `: VOID` return type is
    rejected during bytecode construction with a diagnostic containing
    `unsupported generic type`; no partial module is returned.
- Supersedes:
  - Any interpretation that the two reviewed `expr_calls` regressions prove
    `VAR_IN_OUT` conversion or copy-back behavior when lowering fails earlier
    on the explicit return type.
- Authority:
  - `docs/specs/12-bytecode.md`
- Reason:
  - This is a fail-closed truST encoder boundary. IEC no-result function syntax
    omits the colon and return type, so the reviewed explicit `: VOID` form is
    not an IEC deviation and must not be recorded in `IEC_DEVIATIONS.md`.

## 2026-07-28 - Browser and WebAssembly analysis projection

- Area: Browser-hosted Structured Text analysis
- Decision:
  - The browser engine replaces one complete in-memory document snapshot and
    reports the accepted URI identities without rewriting virtual or plain
    source keys as filesystem paths.
  - For the reviewed diagnostics, hover, and completion cases, browser results
    preserve native analysis results for the same documents and request.
  - Definition, references, rename, and document highlight preserve supplied
    source identities and the asserted targets or counts, and support the
    reviewed identifier-boundary and punctuation-adjacent cursor positions.
  - The WebAssembly JSON surface fails explicitly on malformed input and
    returns parseable apply, status, and diagnostic projections of the same
    engine state.
- Authority:
  - `docs/specs/14-lsp.md`
- Supersedes:
  - Any interpretation that filename-only source keys are incomplete file URIs
    or that adapter success may hide malformed JSON behind an empty result.
- Reason:
  - Browser and JSON hosting must not change source identity or fabricate
    success. This is product adapter behavior outside IEC 61131-3, so it belongs
    here rather than in `IEC_DEVIATIONS.md`. Performance and memory budgets
    remain separate evidence and are not implied by this decision. Native
    facade tests also do not imply `wasm32`, JavaScript, or rendered-browser
    integration proof.

## 2026-07-27 - Simulation model activation and scan-boundary effects

- Area: Runtime simulation
- Decision:
  - An explicit `[simulation].enabled` value is authoritative. When it is
    omitted, a nonempty coupling or disturbance model, or enabled physics with
    at least one joint, enables simulation implicitly.
  - Coupling and physics outputs are queued after a scan and become input-image
    changes only at a following pre-cycle boundary when due.
  - File-backed disturbances execute in configured time order, and a scripted
    fault becomes a visible runtime fault when its due time is reached.
  - Fixed-step physics and coupling replay are deterministic for the same
    accepted model and inputs. The seed is retained as replay identity but no
    claim is made that changing it affects a model with no stochastic element.
  - Accelerated simulation scheduling does not reinterpret scaled model time
    as active watchdog execution time or manufacture a watchdog fault.
- Authority:
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - A declared model must not silently remain inert merely because the optional
    enable flag was omitted, while an explicit disable must remain respected.
    Scan-boundary application prevents simulation from bypassing process-image
    ordering. These are truST runtime choices outside IEC 61131-3 and are not
    IEC deviations.

## 2026-07-27 - ADS server symbol snapshot identity

- Area: ADS server symbol table
- Decision:
  - Accepted symbols are sorted by name before contiguous offset assignment,
    duplicate names reject, and capability flags are preserved.
  - Deterministic serialization identifies the complete snapshot for the local
    symbol-version counter; equal serialization retains the version and a
    changed serialization advances it.
- Authority:
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - Stable address assignment and complete-snapshot version identity prevent
    input-order drift and ambiguous change detection. This is ADS product
    behavior, not an IEC deviation, and does not claim atomic publication.

## 2026-07-27 - ADS service-port routing and registration endpoint binding

- Area: ADS server TCP listener
- Decision:
  - Runtime, system, router, and TCOM service ports form a closed configured
    routing set; unknown target ports reject with `AccessDenied`.
  - System and router service ports expose the documented compatibility
    responses, including two-layer router metadata errors.
  - A notification receiver endpoint is fixed when registration succeeds and
    later multiplexed frames cannot retarget it.
  - TCP bind conflict fails server startup without returning a usable handle.
- Authority:
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - These ADS compatibility and lifecycle rules are truST product behavior.
    The unresolved router TCP/IP entry-length exception remains open instead
    of being accepted implicitly. None of these host rules is an IEC
    deviation.

## 2026-07-27 - ADS notification sampling and invalidation

- Area: ADS server notification sampler
- Decision:
  - Cyclic samples emit at every due boundary; on-change samples emit first,
    suppress equal bytes, and emit after change.
  - Symbol, symbol-version, and static-system sources honor the registered
    watch length without crossing unrelated runtime-value boundaries.
  - Failed or undersized reads preserve the handle in an invalidated empty
    sample, and representable timestamps use Windows FILETIME.
- Authority:
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - These wire-sampling and failure semantics are truST ADS product behavior.
    They require explicit authority for catalog oracles but are outside IEC
    61131-3 and are not IEC deviations.

## 2026-07-27 - Deterministic ready-task and background-program order

- Area: Runtime task scheduler
- Decision:
  - Ready tasks run by lower numeric priority, earlier due time, then stable
    declaration index.
  - Registering a task while its `SINGLE` input is already high initializes the
    saved edge state without a spurious activation.
  - Programs without a task association execute once after ready-task work in
    each resource cycle.
- Authority:
  - `docs/specs/10-runtime-semantics.md`
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - IEC defines priority and task association but permits the deterministic
    host tie-break and cycle realization. Recording the exact truST choice
    resolves conflicting documentation without misclassifying a permitted
    implementation policy as an IEC deviation.

## 2026-07-27 - Runtime cycle deadlines and automatic-restart backoff

- Area: Runtime scheduler safety
- Decision:
  - An enabled watchdog arms execution and output-commit deadlines for each
    cycle and restores the preceding values after every contained outcome.
  - A disabled watchdog does not arm either deadline.
  - Automatic restart uses an exponential delay based on the cycle interval,
    clamped to a one-millisecond floor and one-second cap.
- Authority:
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - Deadline restoration prevents one scan's temporary safety boundary from
    leaking into later work. Bounded restart delay prevents a persistent fault
    from becoming a busy retry loop while preserving deterministic recovery.
    These are truST host-runtime choices outside IEC 61131-3 and therefore are
    product decisions, not IEC deviations.

## 2026-07-27 - ADS AMS/TCP frame-length boundary

- Area: ADS server wire codec
- Decision:
  - The AMS/TCP prefix is six bytes: two zero reserved bytes and a
    little-endian length.
  - Both that length and `max_frame_bytes` cover the 32-byte AMS header plus
    payload and exclude the six-byte prefix.
  - Parsing and serialization require exact header/payload length correlation;
    response construction swaps endpoints while preserving command and invoke
    identity.
- Authority:
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - ADS framing is a truST product/protocol contract outside IEC 61131-3.
    Naming the exact configured-length boundary resolves the otherwise
    ambiguous phrase "AMS/TCP frame size" and gives codec tests one precise
    oracle without claiming transport or interoperability proof.

## 2026-07-27 - Mesh data and control-plane authority

- Area: Runtime mesh transport
- Decision:
  - Mesh envelopes preserve source identity and sequence metadata, and inbound
    subscription keys use a trimmed nonempty `peer:key` grammar.
  - Numeric payloads are decoded against the configured local type without
    narrowing, wrapping, non-finite substitution, or partial update.
  - Control-plane readiness requires session, liveliness, identity-queryable,
    and catalog-queryable authority. Bounded readiness and snapshot waits fail
    visibly instead of manufacturing ready or empty-success state.
  - QoS uses canonical slash-delimited `active`, `cfg`, and `diag` segments in
    that precedence order; other keys use the fast data profile.
  - This decision does not establish the unresolved Zenoh version-family policy
    and does not treat a TLS flag or queryable-ready bit as transport-security
    or query/reply proof.
- Authority:
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - Mesh behavior is a truST runtime contract outside IEC 61131-3. Recording
    the existing envelope, mapping, readiness, liveliness, QoS, and snapshot
    boundaries gives focused tests exact authority without overstating TLS,
    interoperability, or payload-exchange evidence.

## 2026-07-27 - OpenOT translation, publication, and reconciliation boundary

- Area: OpenOT compiler/runtime integration
- Decision:
  - Accepted OpenOT attributes are translated deterministically into one hidden
    producer per attributed program, a canonical definition document, and
    typed producer calls before bytecode generation.
  - Explicit identities remain pinned, implicit identities follow deterministic
    declaration order, and any validation or translation failure rejects the
    compilation instead of falling back to uninstrumented bytecode.
  - Runtime heartbeat and ST-producer publication are part of the scan result:
    a required append failure fails the cycle and never claims a record was
    published.
  - Fenced consumers reconcile delivery and loss per run and source. Unfenced
    operation remains proof-only and carries no equivalent reconciliation
    claim.
- Authority:
  - `docs/specs/27-openot-authoring.md`
- Reason:
  - OpenOT is a truST product integration outside IEC 61131-3. Freezing the
    existing compiler, producer, ring-publication, event, and reconciliation
    behavior in the product contract gives its tests a real oracle without
    misclassifying any choice as an IEC deviation.

## 2026-07-27 - Side-effect-free debug evaluation call boundary

- Area: Debug Adapter Protocol expression evaluation
- Decision:
  - `EvaluateRequest` accepts value and type expressions plus the explicitly
    implemented whitelist of pure standard functions, conversions, and
    split-date/time helpers.
  - User-defined, unknown, or impure calls remain rejected before execution.
    The whitelist does not grant general call execution in hover or watch
    evaluation.
- Authority:
  - `docs/specs/13-debug-adapter.md`
  - `docs/specs/14-lsp.md`
- Reason:
  - DAP is a product/tooling protocol outside IEC 61131-3. The accepted pure
    calls are useful for inspecting values without allowing debug evaluation
    to mutate the running program, and recording the closed boundary prevents
    either an inaccurate all-calls ban or an unsafe general-call claim.

## 2026-07-26 - Runtime process-image and connector boundary

- Area: Runtime process image, I/O drivers, industrial protocols, and connector
  reporting
- Decision:
  - Each process-image area is independently capped at 16 MiB. Oversized
    metadata, bindings, resizes, and writes fail before partial mutation, while
    unallocated reads return zero without allocating.
  - Built-in I/O drivers retain stable canonical names and aliases. Driver
    composition follows cycle-boundary ordering, and GPIO configuration,
    Modbus/MQTT worker lifecycle, EtherCAT image-shape failure, and safe-state
    handoff follow the contracts in `docs/specs/11-runtime-engine.md`.
  - MQTT TLS/mTLS validation and the outbound Sparkplug B node profile are
    closed product contracts. Transport construction or mock traffic does not
    by itself claim an authenticated broker or external interoperability.
  - Connector reports use the versioned schema and projection rules in
    `docs/specs/23-connector-status.md`. A serializer lock proves wire shape
    only; mock, TCP reachability, and directly constructed states do not prove
    live protocol or physical-device health.
  - Communication discovery is bounded by caller-selected scope and origin.
    Browsing projects configured or observed symbols into stable authoring
    trees without converting cached or transport-only evidence into live
    protocol proof.
  - Runtime mDNS uses the truST PLC service type and preserves the reviewed
    runtime identity, endpoint, host-group, and removal semantics.
  - The OPC UA server exports only the reviewed scalar and enum projection,
    starts before its first runtime snapshot without claiming ready data, and
    remains read-only. Its secure profile is signed-and-encrypted with
    anonymous access disabled unless configuration explicitly selects another
    accepted profile.
  - ADS generated interfaces are deterministic, type-faithful, collision-free
    Structured Text artifacts whose offline validation is exact. ADS web
    routes derive credential-channel authority from the observed request and
    may use an internal local token only for in-process control dispatch; that
    token is never forwarded by the control proxy.
- Authority:
  - `docs/specs/11-runtime-engine.md`
  - `docs/specs/23-connector-status.md`
- Reason:
  - IEC 61131-3 defines PLC language and directly represented variables but not
    host allocation caps, GPIO backends, MQTT/Sparkplug, Modbus worker
    lifecycle, EtherCAT transport, mDNS, OPC UA or ADS transport/tooling, web
    proxy credentials, or connector JSON. Keeping these choices in the product
    log prevents them from being mislabeled as IEC deviations and makes the
    proof ceiling of structural tests explicit.

## 2026-07-26 - Structured Text parser compatibility boundary

- Area: Structured Text type references, expression calls, declaration
  initializers, and parser recovery
- Decision:
  - `STRING(<constant-expression>)` and
    `WSTRING(<constant-expression>)` are accepted as vendor-compatible aliases
    of the IEC bracketed length form. They have the same constant, positive,
    and bound-validation rules as `STRING[...]` and `WSTRING[...]`.
  - The parser accepts an `ARRAY[*]` type shape wherever a variable type
    reference is syntactically allowed, including as the target of the
    `POINTER TO` product extension. Semantic validation separately enforces the
    IEC variable-length-array declaration locations.
  - `TIME()` is retained as a zero-argument `CallExpr` for source
    compatibility. This syntax-only acceptance does not assign conversion or
    runtime semantics to the call.
  - Parenthesized declaration initializers use aggregate-initializer syntax
    only in declaration or type-default positions. Enum value assignments and
    formal call arguments retain their own syntax.
  - Positional and empty structure initializers are rejected. Recovery is
    bounded at the following declaration, `END_VAR`, or end of file, and the
    positional form uses the stable diagnostic documented in
    `docs/specs/03-variables.md`.
- Authority:
  - `docs/specs/02-data-types.md`
  - `docs/specs/03-variables.md`
  - `docs/specs/05-expressions.md`
- Reason:
  - These accepted compatibility forms and recovery guarantees are observable
    product behavior. Recording the parser-versus-semantic boundary prevents
    syntax acceptance from being mistaken for IEC semantic acceptance or an
    IEC deviation.

## 2026-07-26 - HIR product authority and verification-inventory boundary

- Area: HIR semantic services, OpenOT authoring, diagnostics, allocation
  validation, and verification governance
- Decision:
  - The stable HIR identity, invalidation, translation, diagnostic-context,
    declaration-catalog, concurrency, and fail-closed primitive-model
    contracts are normative truST product behavior.
  - OpenOT pragma semantics, the exact HIR warning policy, and truST validation
    of `SIZEOF`, `NEW`, and `__DELETE` are normative product contracts. They do
    not become IEC requirements or IEC deviations merely because the HIR
    implements them alongside IEC language semantics.
  - IEC `REF(...)` lifetime requirements remain IEC authority. The
    method-result lifetime rule and the `ADR(...)`/`POINTER TO`
    slot-versus-pointee rules are truST product constraints around addressable
    storage. Rejection of `REF(...)` for CONSTANT-qualified variables is the
    separate normative conflict recorded as DEV-020.
  - The IEC table test inventory is verification governance only. It proves
    that its listed names resolve and that its rendered report is
    deterministic, review-locked, and fail-closed. It does not prove that every
    relevant IEC table or test is listed, IEC product behavior, or conformance.
- Authority:
  - `docs/specs/02-data-types.md`
  - `docs/specs/26-hir-semantic-kernel.md`
  - `docs/specs/27-openot-authoring.md`
  - `docs/specs/28-hir-warning-policy.md`
  - `docs/specs/29-hir-sizeof-and-allocation.md`
  - `docs/specs/30-verification-inventory.md`
- Reason:
  - Existing HIR tests cover both IEC-derived semantics and truST-owned
    behavior. Recording the ownership boundary prevents product choices and
    verification mechanics from being mislabeled as normative IEC
    nonconformance.

## 2026-07-26 - VS Code shell consolidation and retired surfaces

- Area: VS Code commands, navigation, and product packaging
- Decision:
  - `Devices & Connections`, Live Values, native Testing, and the truST sidebar
    are the supported user-facing entry points for their workflows.
  - The legacy Communication and ADS panels are removed from the contributed
    product surface.
  - The VS Code 3D-twin panel is retired and is not contributed, activated,
    packaged, or exposed as a language-model tool by this extension. A future
    digital-twin product may ship through a separately reviewed surface.
- Supersedes:
  - `docs/internal/design/vscode-ux-overhaul-plan.md` section 5's
    “demote, do not cut” decision for the VS Code trust-twin panel.
- Reason:
  - The shipped extension has one visible route for each workflow. Retaining
    hidden or partially packaged retired surfaces makes the command model and
    release contents ambiguous.

## 2026-07-26 - Deploy and Compile product language

- Area: VS Code truST sidebar
- Decision:
  - The fixed fourth sidebar action is named `Deploy`.
  - Until a real deployment backend exists, Deploy remains visible in its
    stable location but disabled with the reason
    `Deploy is not available for this target yet.`
  - No deploy command is contributed to the command palette before the backend
    exists.
  - The authoritative project-validation action is named `Compile`, invokes
    `trust-lsp.checkProgram`, and remains a fixed sidebar action plus a palette
    escape hatch.
- Supersedes:
  - `docs/internal/design/vscode-ux-overhaul-plan.md` sections 0.5.6 and 0.6.5
    where `Send to PLC` was selected as the user-facing label.
  - Sections 0.5.6 and 0.5.17 where `Check program` was selected as the
    user-facing label. The backend-authority requirements remain applicable.
- Reason:
  - Stable, concise sidebar labels match the shipped product. Disabled reasons
    and result wording state capability truth without advertising a command
    that cannot perform the operation.

## 2026-07-26 - Live Values placement and lifecycle ownership

- Area: VS Code Live Values and Devices & Connections
- Decision:
  - Live Values owns values, write, force, release, and value-operation
    feedback. It does not own Start, Stop, Connect, Disconnect, target-mode
    selection, compile diagnostics, or debugging controls.
  - When opened from Devices & Connections, Live Values reuses the active
    editor group. Other launch routes use the normal secondary editor group.
  - Devices & Connections and the truST sidebar own runtime lifecycle actions.
- Supersedes:
  - The unresolved placement choice in
    `docs/internal/design/vscode-ux-overhaul-plan.md` section 0.5.5.
  - Public wording that presents abbreviated `W`/`F`/`R` controls or lifecycle
    buttons inside Live Values.
- Reason:
  - Runtime lifecycle and process-value mutation are separate safety contexts.
    Context-sensitive placement preserves usable table width without
    destroying the graph layout.

## 2026-07-26 - Runtime token compatibility boundary

- Area: VS Code runtime authentication
- Decision:
  - Per-endpoint runtime tokens are written to and read from VS Code
    SecretStorage.
  - SecretStorage wins when both secure and fallback values exist.
  - The product may read the explicitly named
    `trust.runtime.authTokenFallback` plaintext setting as a bounded legacy
    compatibility input. It is labelled legacy/not recommended and directs
    users to the OS secret store.
  - Runtime code does not write plaintext tokens, does not read the retired
    `trust-lsp.runtime.controlAuthToken` setting directly, and does not
    contribute that retired key to native Settings.
  - A managed runtime project MAY contain the runtime-generated bootstrap
    control token in its own `runtime.toml`. The extension reads that token only
    to import it into per-endpoint SecretStorage before attach; this is distinct
    from an extension setting, example fixture, or log, and the extension never
    writes it into Settings.
- Supersedes:
  - Absolute “never plaintext settings” wording in
    `docs/internal/design/vscode-ux-overhaul-plan.md` sections 0.6.4 and 0.6.8
    only to the extent needed for this explicit read-only compatibility input.
- Reason:
  - Existing installations remain connectable while all new and endpoint-
    specific credentials use the OS secret store.

## 2026-07-26 - Starter gallery and runnable scaffold

- Area: VS Code onboarding and examples
- Decision:
  - `Start from example` opens the searchable, filterable example gallery as
    the primary selection surface rather than a plain QuickPick.
  - The curated set contains Empty simulator, Conveyor, TwinCAT ADS,
    Raspberry Pi, HMI starter, and PLCopen Motion single axis.
  - A runnable bundled scaffold contains `trust-lsp.toml`, `runtime.toml`,
    `io.toml`, `.vscode/launch.json`, `.vscode/settings.json`, and
    `src/Main.st`.
- Supersedes:
  - The five-item and QuickPick-only descriptions in
    `docs/internal/design/vscode-ux-overhaul-plan.md` sections 0.5.6 and
    0.5.12.
- Reason:
  - The gallery supports a growing curated set and makes hardware requirements
    visible before copying. The six-file scaffold is the actual zero-manual-
    setup contract.

## 2026-07-26 - Devices addition and visual-editor runtime placement

- Area: Devices & Connections and VS Code visual editors
- Decision:
  - Devices & Connections exposes `+ Add` as a first-class action; adding a
    device or connection does not require entering a hidden Edit mode.
  - SFC, Statechart, Ladder, and Blockly do not embed private runtime, I/O,
    runtime-settings, or compile-diagnostics panels. Runtime lifecycle remains
    in the truST sidebar and values remain in Live Values.
  - Visual editors retain the shared Structured Text generation, launch,
    attach, and debug command route.
- Supersedes:
  - Public add-device instructions that require Edit mode before `+ Add`.
  - `docs/specs/17-visual-editors-runtime-unification.md` section 5's required
    right-pane runtime controls. Sections 1 through 4 remain authoritative.
- Reason:
  - Addition is a primary workflow. Duplicate runtime panes create conflicting
    lifecycle and safety controls while adding no execution capability.

## 2026-07-26 - Devices and Connections link certainty

- Area: VS Code Devices & Connections graph
- Decision:
  - Dashing represents missing live topology proof, not connection health.
  - Only an established connected, degraded, or error status proves topology
    and renders solid. Every other status, including an unrecognized future
    status, fails closed and renders dashed.
  - A mesh fabric remains dashed until every peer has an established connected,
    degraded, or error status. Other and unrecognized statuses are unproven.
- Supersedes:
  - The narrower wording in `docs/specs/25-vscode-product-contract.md` that
    did not separate connection proof from connection health.
  - The implementation behavior that dashed established degraded and error
    links while rendering configured-only links as solid.
- Reason:
  - Line style answers whether a connection has been proven to exist. Semantic
    status answers whether that proven connection is healthy. Keeping those
    dimensions separate prevents a real degraded connection from looking like
    an uncommitted draft.

## 2026-07-26 - Final project I/O driver removal preserves safety policy

- Area: Runtime communication authoring and project I/O configuration
- Decision:
  - Removing the final configured project I/O driver MUST keep the project
    `io.toml`.
  - The retained file uses explicit `driver = "none"` with empty parameters,
    preserves safe-state values and project comments outside rewritten
    driver/safe-state items, and contains no active driver.
  - Deleting the last driver MUST NOT implicitly opt the project into system
    I/O fallback. Selecting system I/O requires a separate explicit action.
  - A later Add replaces the `none` sentinel with the selected driver while
    preserving the retained project policy.
- Supersedes:
  - The control-path behavior and tests that deleted the complete `io.toml`
    after removing its last driver.
- Reason:
  - `io.toml` owns both driver selection and fault safe-state policy. Deleting
    it silently discards safety configuration and may activate machine-wide
    fallback I/O, which is not equivalent to turning project I/O off.

## 2026-07-26 - Communication and fleet status vocabularies

- Area: Runtime communication capabilities, fleet topology, and connector
  status
- Decision:
  - `docs/specs/23-connector-status.md` remains the canonical vocabulary for a
    protocol connector report's `state`, `health`, discovery confidence, and
    point quality.
  - Communication capability `health` and fleet topology runtime, endpoint,
    and link status are separate versioned projection vocabularies. They
    express build availability, configured-only policy, simulation, runtime
    reachability, live connection, degradation, and error for product
    authoring and topology rendering.
  - Consumers MUST NOT reinterpret communication or fleet projection values as
    connector-report health, or promote an unknown value to healthy.
- Supersedes:
  - Any reading that treats every field named `health` as the connector-report
    health vocabulary irrespective of its versioned response type.
- Reason:
  - The surfaces answer different questions. Connector reports normalize live
    protocol evidence; authoring capabilities and fleet topology must also
    represent build and configuration states that are not connector health.

## 2026-07-26 - Canonical VM semantics and optimized-backend fallback

- Area: Runtime bytecode execution
- Decision:
  - The stack executor is the canonical oracle for observable VM semantics.
  - Register-IR and tier-1 execution must preserve values, declared runtime
    types, storage mutations, traps, instruction-budget accounting, and
    deadline behavior.
  - An optimized backend may decline and use the stack executor only before it
    has made an observable mutation.
  - Cache policy, allocation reuse, profiling counters, polling stride, and
    diagnostic prose remain implementation details rather than product
    semantics.
  - An uninitialized interface-typed local or function return slot
    materializes as `Value::Null`; it does not manufacture an interface
    instance or a distinct empty-interface value.
- Supersedes:
  - Any interpretation that treats successful optimized execution as
    sufficient without parity to the stack executor, or treats exact internal
    counters and diagnostic strings as stable product behavior.
- Reason:
  - Optimization must not create a second language runtime. A single semantic
    authority permits safe fallback while keeping internal performance policy
    evolvable.

## 2026-07-26 - Runtime operator-surface authority and focused budgets

- Area: Runtime HMI, standalone Web IDE, terminal dashboard, and deployment
- Decision:
  - Runtime HMI schema, values, writes, descriptor reload, alarms, trends, and
    events are one typed product contract. Read-only mode and the explicit
    write allowlist remain authoritative across in-process and JSON routes.
  - The standalone Web IDE is a contained project authoring surface with
    Viewer/Editor sessions, sliding idle expiry, optimistic file versions,
    project-relative filesystem operations, local bundled assets, and the same
    language-analysis semantics as the LSP.
  - Its focused regression budgets are the exact limits recorded in
    `docs/specs/11-runtime-engine.md`; they are reference-environment gates,
    not universal real-time guarantees.
  - The terminal dashboard projects the runtime control state without creating
    a second status vocabulary.
  - Enabled deployment-signature policy validates the complete payload and
    trusted-key lifetime before any install or pointer mutation, and failure
    output never exposes key material.
- Supersedes:
  - Any interpretation that treats HMI, Web IDE, terminal UI, or deployment
    tests as implementation-only merely because IEC 61131-3 does not define
    those product surfaces.
  - Any interpretation that promotes focused Web IDE timing tests to guarantees
    for arbitrary hardware or project size.
- Reason:
  - These surfaces are intentionally shipped product behavior and need stable
    authority for their existing tests. Keeping timing scope and write/security
    boundaries explicit prevents both false proof and accidental weakening.

## 2026-07-26 - VS Code authoring workflow authority

- Area: VS Code testing, protocol authoring, visual editors, HMI tools, and
  project workflows
- Decision:
  - Native Testing, snippets, OPC UA browse-to-save identity, PLCopen
    import/export, deterministic visual-model transformations, HMI
    language-model artifacts, library manifest editing, project creation, and
    focused recovery behavior are shipped product contracts.
  - Visual execution remains authoritative only through generated Structured
    Text. Retained Ladder and Statechart component engines and retired embedded
    runtime/I/O panels do not define shipped runtime semantics.
  - Right-pane persistence applies to authoring panes only. Runtime lifecycle
    remains in the truST sidebar and values remain in Live Values.
  - Weak tests that only observe initial diagnostics, compare interchange
    counts, or accept a component fallback do not prove parser completion,
    semantic round-trip fidelity, or runtime behavior.
- Supersedes:
  - `docs/specs/16-ladder-profile-trust.md` sections 1, 4 through 6, and 9
    where they described LadderEngine and embedded runtime/I/O panes as
    primary runtime evidence.
  - Any interpretation that promotes editor-component behavior over the shared
    Structured Text runtime contract.
- Reason:
  - These workflows already define user-visible data integrity and recovery.
    Grouping their authority prevents false catalogue mappings while keeping
    legacy helpers and retired panels runnable without treating them as product
    proof.

## 2026-07-26 - LSP client aliases and editor projection

- Area: LSP and IDE product behavior
- Decision:
  - CamelCase client settings are canonical and win over snake_case aliases;
    wrong-typed aliases are ignored rather than coerced.
  - HMI commands and descriptor diagnostics validate independently of runtime
    compilation and rank only high-confidence recovery suggestions.
  - Completion cancellation returns no payload, and learner hints remain
    confidence-gated.
  - OpenOT completion, inlay, and code-action projection is limited to
    documented typed behavior.
  - Hover falls back to written declaration authority, runtime inline values
    merge by resolved identity, call hierarchy respects allowed files, and
    namespace relocation creates and edits before deleting.
  - Platform-gated URI behavior is never inferred from a Linux-only test run.
- Supersedes:
  - Any client-alias merge that lets a legacy snake_case value override the
    canonical camelCase setting.
  - Any false-success interpretation of cancelled completion, low-confidence
    learner hints, unavailable platform syntax, or HMI diagnostics that happen
    to coincide with a successful compile.
- Reason:
  - These choices define deterministic editor recovery and prevent stale,
    cross-platform, or partial state from being presented as authoritative.

## 2026-07-27 - Executable STBC 1.1 opcode authority

- Area: Runtime bytecode format and source lowering
- Decision:
  - The accepted STBC 1.1 instruction set is the set documented in
    `docs/specs/12-bytecode.md` section 7.3 and implemented by the fail-closed
    validator and canonical stack executor.
  - `CALL_NATIVE`, self/super/null loads, size queries, partial access, and the
    debug marker are part of that accepted set.
  - Legacy or previously proposed values `0x05`, `0x07`, `0x08`, `0x14`,
    `0x15`, `0x16`, `0x4A`, `0x4B`, `0x4D`, and `0x4E` are not accepted in
    STBC 1.1 and must fail validation before dispatch.
  - Source lowering must preserve the typed section, metadata, process-image,
    resource/task identity, control-flow, and dynamic-reference projections
    specified in section 7.5; it must fail instead of inventing placeholder
    bytecode for unsupported source constructs.
- Supersedes:
  - The stale baseline instruction table that assigned executable semantics to
    unimplemented values while omitting opcodes already emitted and executed
    by the product.
- Reason:
  - A versioned bytecode contract must describe the wire format that the
    validator and runtime actually accept. Keeping proposed and executable
    opcodes in one table created false specification authority for tests and
    made valid emitted modules appear undocumented.

## 2026-07-27 - Variable-storage reference and layout authority

- Area: Runtime variable storage and dynamic references
- Decision:
  - Global, current-local-frame, and instance slots share one reference
    contract; direct-slot, empty-path, borrowed-reference, and owned-reference
    helpers must address the same logical value.
  - Instance-field resolution prefers the requested instance before its parent
    chain. Declared offsets exclude inherited fields and remain consistent for
    instances with the same declared layout.
  - Lookup caches may accelerate resolution but must invalidate a direct miss
    after insertion and an inherited hit after child shadowing. Recursive
    parent-chain misses are not cached.
  - Nested aggregate writes are copy-on-write and checked array-offset
    arithmetic must reject overflow rather than wrap.
  - Cache structure, capacity, eviction, and synchronization remain internal
    implementation details.
- Supersedes:
  - Any interpretation that treats cache contents as storage authority, places
    inherited fields in a child's declared layout, or permits a nested write
    to mutate another slot that shared the old aggregate value.
- Reason:
  - Debug, configuration, initializer, and helper-evaluation paths all use
    `VariableStorage`. A single fail-closed reference contract prevents those
    surfaces from disagreeing about identity, inheritance, or nested mutation.

## 2026-07-27 - Retain codec compatibility and migration authority

- Area: Runtime retain persistence and warm-load migration
- Decision:
  - Version-2 retain snapshots preserve supported scalar and aggregate values,
    names, and order; supported version-1 `STRN` snapshots remain readable.
  - A missing file means an empty snapshot, while other filesystem failures
    remain visible. Atomic rename publishes before parent-directory sync, so a
    sync failure returns an error even if the renamed snapshot is immediately
    readable, and the manager retains retry eligibility.
  - Safe numeric widening, bounded-string canonicalization, declared structure
    additions/removals, and orphan removal are the supported warm-load
    migrations. Successful migrations and orphan removal emit their typed
    runtime events.
  - Snapshot application validates and stages every entry before changing any
    retained target.
- Supersedes:
  - Any interpretation that treats every load failure as an empty snapshot,
    treats a parent-sync error as durable success, silently discards migration
    evidence, or exposes a prefix of a rejected snapshot.
- Reason:
  - IEC 61131-3 defines retention across restart but not the file format,
    filesystem transaction, compatibility versions, or online migration
    policy. These shipped runtime choices need one explicit fail-closed
    authority and are not IEC deviations.

## 2026-07-27 - Host lvalue writes fail closed

- Area: Runtime host-helper storage mutation
- Decision:
  - The reviewed host lvalue helper paths update only an already-resolved
    current-local name or global-root dereference, array element, structure
    field, or nested aggregate target.
  - An unresolved root returns `RuntimeError::UndefinedVariable` and creates no
    fallback global slot.
- Supersedes:
  - Any interpretation that an assignment helper may create an undeclared
    global as a fallback.
- Reason:
  - Silent slot creation converts a misspelled or stale target into successful
    mutation of unrelated state. The helper boundary must fail closed.

## 2026-07-27 - Host helper standard-library capability

- Area: Runtime constant, debugger, and configuration expression helpers
- Decision:
  - The reviewed helper evaluates `ABS(DINT#-1)` to `DINT#1` when the caller
    supplies a `StandardLibrary` capability.
  - The same reviewed expression attempted without that capability returns
    `RuntimeError::TypeMismatch`.
- Supersedes:
  - Any interpretation that resolver-less helper evaluation inherits the
    production runtime's standard-library surface automatically.
- Reason:
  - Capability injection keeps this bounded helper behavior explicit. Other
    standard functions require separate specification behavior and proof.

## 2026-07-27 - Test-only host evaluator compatibility observations

- Area: Portable runtime program-model construction and host-stage evaluation
- Decision:
  - The test-only host evaluator retains only the exact expression, statement,
    call-binding, POU, and debug-hook observations specified in
    `docs/specs/10-runtime-semantics.md` sections 5.4, 5.5, 6.12, and 7.6.
  - Those observations include rejection of the reviewed mixed
    signed/unsigned numeric operation and the string-selector `CASE` result.
    They are compatibility authority for the test-only host model, not
    source-admission authority and not an alternative production execution
    backend.
  - Semantic analysis and validated STBC/VM execution remain authoritative for
    shipped IEC behavior. A host-only observation cannot widen their accepted
    type matrix or statement grammar.
- Supersedes:
  - Any interpretation that one host-evaluator unit test certifies a general
    language rule or makes that evaluator a supported production backend.
- Reason:
  - The portable model is still used by focused construction, initializer,
    debugger, and behavior-lock tests. Narrow compatibility authority keeps
    those tests meaningful without converting host-only differences into IEC
    deviations or silently changing the production language contract.

## 2026-07-27 - Numeric exponent result contract

- Area: Structured Text runtime numeric operations
- Decision:
  - The host evaluator accepts the intentional integer-base `**` extension;
    the reviewed representative result is `INT#2 ** INT#3 = INT#8`.
  - Its reviewed mixed-real result is `LREAL#2.0 ** REAL#3.0 = LREAL#8.0`.
    Other exponent type combinations and boundary failures require separate
    specification behaviors and focused proof.
- Supersedes:
  - The prior product-spec implication that the host evaluator accepted no
    integer-base `**` case.
- Reason:
  - IEC 61131-3 Ed.3 section 6.6.2.5.8 and Table 29 require `IN1` to be
    `ANY_REAL`. Accepting an integer base is therefore the intentional
    normative conflict recorded in
    `docs/IEC_DEVIATIONS.md#2026-07-27---integer-base-exponentiation`; the
    two representative typed results above are the bounded host-evaluator
    contract established by this decision.

## 2026-07-27 - Boolean comparison ordering

- Area: Structured Text runtime comparison operations
- Decision:
  - The reviewed host-evaluator comparison `TRUE >= FALSE` returns `TRUE`.
  - This decision does not use that one comparison to certify every Boolean
    comparison operator or a complete total-order contract.
- Supersedes:
  - The product-spec implication that the host evaluator had no reviewed
    ordered-Boolean result beyond `=` and `<>`.
- Reason:
  - IEC 61131-3 Ed.3 Table 33 admits `ANY_ELEMENTARY` comparison operands.
    Recording this exact result removes an internal product-spec contradiction
    and does not create an IEC deviation.

## 2026-07-27 - Developer test workflow compatibility authority

- Area: `trust-dev test`, CI classification, and API documentation
- Decision:
  - Every selected test case begins from a cold-restarted prepared runtime and
    must enter its declared program or function-block body.
  - Assertion failures and deadlines remain distinct failed and timeout
    outcomes. Non-assertion runtime-error classification remains explicit
    test debt.
  - JSON, TAP, JUnit, human, and list rendering preserve the bounded contracts
    in `docs/specs/22-developer-workflows.md`.
  - The JUnit suite retains the historical `trust-runtime` name as a
    compatibility identifier after command ownership moved to `trust-dev`.
  - The `trust-dev` copy of CI classification retains the runtime entrypoint's
    stable public codes and precedence.
  - Tagged ST documentation extraction and deterministic Markdown/HTML
    rendering are shipped product behavior. The existing snapshots establish
    only their reviewed fixtures, not every malformed or escaping partition.
- Supersedes:
  - Any interpretation that compiling or discovering a test without entering
    its body is a passing execution result.
  - Any interpretation that the historical JUnit suite name proves current
    binary ownership.
  - Any interpretation that product runner, reporting, Git, agent, or
    documentation behavior belongs in the IEC deviation register.
- Reason:
  - These behaviors are already observable automation contracts. Recording
    their bounded authority lets existing tests be catalogued without
    converting compatibility strings or narrow snapshots into broader claims.

## 2026-07-27 - Runtime standard-function result authority

- Area: Runtime standard-library values and truST assertion extensions
- Decision:
  - The runtime result contract for the IEC-named conversion, numerical, bit,
    selection, comparison, string, and validation functions is the bounded
    behavior written in `docs/specs/07-standard-functions.md`. Static HIR
    signature acceptance alone does not prove those runtime results.
  - The reviewed text-conversion representatives are
    `DINT_TO_STRING(DINT#42) = '42'`,
    `REAL_TO_STRING(REAL#1.25) = '1.25'`,
    `DWORD_TO_STRING(DWORD#42) = '42'`, and
    `STRING_TO_DINT('42') = DINT#42`. These examples do not define formatting
    for every elementary value.
  - Text-to-`REAL`/`LREAL` conversion rejects the reviewed non-finite spellings
    `NaN` and `inf` without assigning a stable runtime-error variant to that
    rejection.
  - A successful truST `ASSERT_*` runtime call returns the internal
    `Value::Null` representation of its IEC-facing `VOID` result. A failed
    assertion returns `RuntimeError::AssertionFailed` with the bounded
    user-facing message forms specified in
    `docs/specs/07-standard-functions.md`.
  - The reviewed assertion comparisons admit lossless `INT`/`DINT`
    comparison and finite `REAL`/`LREAL` tolerance comparison. This does not
    authorize every mixed elementary-type pairing.
- Supersedes:
  - Any interpretation that a standard-function HIR signature establishes its
    runtime result.
  - Any interpretation that non-finite text parsing has a catalog-stable error
    variant merely because it must reject.
  - Any interpretation that truST assertion success or diagnostic formatting
    is governed by IEC 61131-3.
- Reason:
  - Runtime result values and assertion diagnostics are observable product
    behavior. The assertion API and its messages are truST test-workflow
    contracts outside the IEC standard, so they belong in product authority
    rather than the IEC deviation register.

## 2026-07-28 - Realtime T0 shared-memory and cycle boundary

- Area: Same-host realtime communication
- Decision:
  - `T0HardRt` is a same-host, pre-bound, fixed-layout shared-memory route.
    `T0HardRt` traffic never falls back to generic mesh/IP transport. The
    `T1Fast`, `T2Ops`, and `T3Diag` tiers use the mesh/IP route and are not
    admitted through the T0 route.
  - T0 channel registration fails before readiness when the shared-memory root
    cannot be provisioned, required page pinning is unavailable, or an
    existing shared-memory header does not match the requested channel,
    schema, layout, policy, and ownership contract.
  - Publisher and subscriber binding requires the configured channel, the T0
    route, a matching schema hash, a fixed positive payload layout within the
    registered slot, and initialized pinned shared-memory state.
  - The T0 data path publishes one latest fixed-size payload. Readers report
    skipped committed updates and cumulative overruns, distinguish bounded
    no-update from stale data, and surface bounded-spin exhaustion as stale
    rather than spinning without limit.
  - The T0 cycle helper permits one pre-task exchange followed by one post-task
    exchange per cycle. Each cycle resets its noncritical cloud-work budget;
    excess work is denied and accumulated visibly without consuming the T0
    exchange path.
  - Processes that open the same root and exact channel contract observe a
    successfully committed payload through the shared mapping.
  - These contracts define functional state, ordering, and accounting. They do
    not establish worst-case latency, a deadline guarantee, PREEMPT_RT
    readiness, production page-lock success, or a hard-real-time certification.
- Supersedes:
  - Any interpretation that mesh/IP is a deterministic fallback for a failed
    T0 bind.
  - Any interpretation that the `HardRt` name or a software shared-memory test
    proves a physical scheduling or latency guarantee.
- Authority:
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - Route admission, shared-memory layout, pinning, counters, and cycle-budget
    policy are observable truST host-runtime behavior outside IEC 61131-3.
    Recording them as product authority allows exact catalog oracles without
    misclassifying the host transport as an IEC deviation or overstating its
    timing proof.

## 2026-07-28 - Live cyclic breakpoint eligibility

- Area: Runtime debugger breakpoint lifecycle
- Decision:
  - A breakpoint installed after launch, including while cyclic execution is
    already running, becomes eligible at the next matching statement boundary.
    It does not retroactively stop work completed before installation.
  - After continue, an unchanged breakpoint remains eligible on later scans
    until it is cleared or filtered by its condition or hit-count rule. One
    breakpoint identity emits at most one stop per scan even when multiple VM
    debug-map entries overlap its source range.
- Supersedes:
  - Any interpretation that breakpoints are frozen at launch or become
    one-shot merely because execution continued after a stop.
- Authority:
  - `docs/specs/13-debug-adapter.md`
- Reason:
  - Live breakpoint mutation and cyclic re-hit behavior are observable truST
    debugger lifecycle rules outside IEC 61131-3. They therefore belong in
    product authority, not the IEC deviation register.

## 2026-07-28 - ADS core fixed-layout value codec and descriptor size

- Area: Shared ADS value and symbol metadata
- Decision:
  - The reviewed fixed-width scalar matrix uses declared one-, two-, four-, or
    eight-byte layouts and canonical little-endian encoding. The reviewed
    `BOOL` true representation is byte `1`.
  - The reviewed `STRING(8)` layout has eight payload bytes plus one terminator
    byte. Decoding stops at the first NUL, and encoding the reviewed value
    zero-fills the remainder of that fixed layout.
  - The reviewed one-dimensional scalar-array layout preserves inclusive
    bounds, element order, scalar encoding, and exact total extent.
  - Byte-length, scalar-versus-array value-type, and array-shape disagreements
    reject as distinct mapping-error classes.
  - ADS symbol metadata is admissible at the reviewed boundary only when its
    endpoint byte size equals the descriptor-computed extent; an observed
    mismatch reports the computed expected and endpoint actual sizes.
- Supersedes:
  - Any interpretation that generic test names such as “every supported type”
    prove unreviewed scalar values or edge cases.
  - Any interpretation that this shared codec decision selects runtime ADS
    server publication of `STRING` or arrays.
  - Any interpretation that native codec unit tests prove ADS wire, external
    endpoint, or TwinCAT interoperability.
- Authority:
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - Fixed byte layout, mismatch classes, and endpoint metadata admission are
    observable truST product/protocol behavior outside IEC 61131-3. Product
    authority permits exact test-catalog oracles without misclassifying the
    behavior as an IEC deviation.

## 2026-07-28 - ADS core point-quality field transitions

- Area: Shared ADS point quality
- Decision:
  - A reviewed cold-start point preserves its supplied name and begins stale.
    An explicitly timestamped stale record preserves its supplied timestamp and
    detail.
  - In the reviewed transition sequence, marking stale quality good records the
    supplied timestamp and clears stale detail; marking it failed records error
    state, failure timestamp, and failure detail; and subsequently marking it
    stale preserves that timestamp while replacing the detail with the current
    stale reason.
- Supersedes:
  - Any interpretation that entering stale necessarily erases the last known
    update timestamp.
  - Any interpretation that these in-memory constructor tests establish clock,
    worker, transport, serialization, or external endpoint behavior.
- Authority:
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - Quality state and field-transition truth are observable truST product
    behavior outside IEC 61131-3 and require exact catalog oracles without an
    IEC deviation entry.

## 2026-07-28 - Runtime bundle source and local dependency resolution

- Area: Runtime project build
- Decision:
  - The canonical implicit project source root is `src/`; a legacy
    `sources/`-only project is rejected instead of silently selected.
  - The reviewed local dependency graph includes direct and transitive source
    files and reports its resolved dependency identities. Missing paths,
    dependency cycles, and requested-version mismatches reject with bounded
    diagnostic identity.
  - Root-project types, globals, and field access may be split across the three
    reviewed source files and compile into one `program.stbc` artifact.
- Supersedes:
  - Any interpretation that `sources/` remains an implicit runtime build root.
  - Any interpretation that a missing, cyclic, or version-incompatible local
    dependency may be omitted while the root build succeeds.
  - Any interpretation that these focused native tests prove general ordering,
    external registries, filesystem races, or cross-platform path behavior.
- Authority:
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - Project layout, dependency resolution, and build-report behavior are truST
    host-product contracts outside IEC 61131-3.

## 2026-07-28 - Runtime backend, historian, and metrics authority

- Area: Host-runtime execution and observability
- Decision:
  - The direct execution-backend selector trims and ASCII-case-folds input,
    accepts only `vm`, rejects empty or unknown input with the bounded
    `runtime.execution_backend` diagnostic, and rejects the retired
    `interpreter` spelling with explicit `vm` migration guidance.
  - A fresh runtime uses the VM backend, zero logical time and cycle count, and
    the default epoch-zero, one-millisecond date/time profile. VM selection may
    precede bytecode loading, and the reviewed source-built runtime may
    materialize its VM module on the first successful cycle.
  - The historian's reviewed all-mode, sample-interval, exact/`retain.*`
    allowlist, file-reload, typed-query, and Prometheus projection behaviors are
    product contracts. They do not claim general filesystem durability or the
    complete wildcard/export surface.
  - Runtime profiling begins enabled, clears and suppresses entries while
    disabled, resumes from a clean collection, and ranks the reviewed calls by
    cycle contribution. The reviewed five-sample latency window has the bounded
    percentile observations specified in the runtime engine contract.
- Supersedes:
  - Any interpretation that the existence of a VM enum or a no-panic smoke test
    establishes backend-selection or lazy-materialization behavior.
  - Any interpretation that an in-memory historian or metrics unit test proves
    crash durability, concurrent collection, percentile rollover, performance
    budgets, or a complete monitoring schema.
- Authority:
  - `docs/specs/11-runtime-engine.md`
- Reason:
  - Backend selection, persistence/export policy, and observability aggregation
    are observable truST host-product choices outside IEC 61131-3. Exact product
    authority permits bounded catalog oracles without inventing an IEC
    decision or deviation.
