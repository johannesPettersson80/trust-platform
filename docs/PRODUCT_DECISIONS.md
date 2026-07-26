# truST Product Decisions Log

This file records reviewed truST product and tooling decisions that are not
interpretations of IEC 61131-3 or PLCopen. IEC interpretation decisions belong
in `IEC_DECISIONS.md`; PLCopen profile decisions belong in
`PLCOPEN_DECISIONS.md`. Product behavior recorded here MUST NOT be copied to an
IEC or PLCopen deviation log merely because truST owns the behavior.

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
