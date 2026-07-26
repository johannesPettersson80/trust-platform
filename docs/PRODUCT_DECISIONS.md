# truST Product Decisions Log

This file records reviewed truST product and tooling decisions that are not
interpretations of IEC 61131-3 or PLCopen. IEC interpretation decisions belong
in `IEC_DECISIONS.md`; PLCopen profile decisions belong in
`PLCOPEN_DECISIONS.md`. Product behavior recorded here MUST NOT be copied to an
IEC or PLCopen deviation log merely because truST owns the behavior.

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
